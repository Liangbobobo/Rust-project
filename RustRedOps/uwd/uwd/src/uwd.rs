use alloc::{string::String, vec::Vec};
use core::ffi::c_void;

use anyhow::{Context, Result, bail};
use obfstr::obfstring as s;
use dinvk::module::{get_module_address, get_proc_address};
use dinvk::types::IMAGE_RUNTIME_FUNCTION;
use dinvk::hash::murmur3;
use dinvk::helper::PE;

#[cfg(feature = "desync")]
use crate::util::find_base_thread_return_address;
use crate::util::{find_gadget, shuffle, find_valid_instruction_offset};
use crate::types::{Config, Registers, UNWIND_OP_CODES::{self, *}};
use crate::types::{UNW_FLAG_CHAININFO, UNW_FLAG_EHANDLER};
use crate::types::{UNWIND_CODE, UNWIND_INFO};
use crate::types::Unwind;

#[cfg(feature = "desync")]
unsafe extern "C" {
    /// Function responsible for Call Stack Spoofing (Desync)
    fn Spoof(config: &mut Config) -> *mut c_void;
}

#[cfg(not(feature = "desync"))]
unsafe extern "C" {
    /// Function responsible for Call Stack Spoofing (Synthetic)
    fn SpoofSynthetic(config: &mut Config) -> *mut c_void;
}

/// Invokes a function using a synthetic spoofed call stack.
///
/// # Examples
///
/// ```
/// use core::ptr;
/// use dinvk::module::{get_module_address, get_proc_address};
/// use uwd::spoof;
///
/// let kernel32 = get_module_address("kernel32.dll", None);
/// let virtual_alloc = get_proc_address(kernel32, "VirtualAlloc", None);
///
/// let addr = spoof!(
///     virtual_alloc,
///     ptr::null_mut::<core::ffi::c_void>(),
///     1 << 12,
///     0x3000,
///     0x04
/// ).unwrap();
///
/// assert!(!addr.is_null());
/// ```
#[macro_export]
macro_rules! spoof {
    // $告诉宏引擎,后面跟着的是一个宏变量或一个重复块的开始.$arg是一个元变量名称,在宏展开时,可以通过$arg来引用用户传进来的数据.宏是如何对应传进来的数据的?以这个宏举例.是通过$addr:expr, 这里的,分割的
    // ()重复块的边界;expr,片段分类符,约束这个变量必须是一个表达式expression(如1+2,&x,my_ptr)详见rust reference
    // 括号外的, 表示在重复匹配$arg时,用户必须用都好分开
    // + 代表一次或多次重复 *表示零次或多次重复 ?代表零次或一次
    // $($arg:expr),+ $(,)? 这里$($arg:expr)代表一个可重复的块的开始 紧接的,代表这个可重复的块中不同的参数之间用,分割 紧接的+代表用,分割的参数可以有一个或多个 紧接的$代表一个新的可重复块 紧接的(,)代表匹配的是,即重复块的内容本身就是, 紧接的?代表匹配的,可以是一个也可以没有
    //  这一段的含义是如果用户在最后一个参数后面加上一个逗号,这里的?就会匹配这个逗号,如果没有加也不会报错.这么做可以让宏自适应任何数量的参数,以灵活的对应win api的参数
    // 
    ($addr:expr, $($arg:expr),+ $(,)?) => {
        unsafe {
            // crate代表一个特殊的占位符,在宏展开时$crate会被替换为定义该宏的哪个库的绝对路径.即编译器会在本文件中(uwd.rs)查找这个宏定义
            // ::路径连接符path separator 是rust明明空间层级分割符
            // __双下划线,业界规范,代表是内部实现,不在文档中公开,步破坏公开接口
            // __private私有模块名(本文件中定义的另一个mod) .这里为啥要另外定义一个mod __private 因为宏是在调用者(用户)的仓库里展开的,在宏中调用的函数必须是pub的函数.但这里只希望使用spoof!宏 不去直接调用底层spoof函数.这么做是为了方便修改具体的实现外,还可以把调用和实现分割开
            // ::spoof 在__private模块中的spoof函数.
            // 宏是拷贝,如果调用10次宏,rustc会生成10次重复代码,增加二进制文件体积.因此宏应尽量少,具体实现放到函数中.函数的调用是指针,体积小的多
            $crate::__private::spoof(
                $addr,
                // &[] 数组引用,把所有转换后的参数放入一个固定大小的数组,以slice的方式使用.后续的spoof函数只需要接收一个参数的slice,而不是多个独立的参数
                // $(...),* 代表将宏捕获的()里面的内容以,分割 并重复捕获0或多次.并将捕获的内容在()中的代码中执行
                // ::core::mem::transmute($arg as usize) transmute将这块内存的数据当作另一种类型使用.由于后续spoof函数接收的是args: &[*const c_void] 所以这里将$arg当作*const c_void这种原始指针
                // ::core这里最前面的::代表从全局根命令空间开始查找后面的模块,而不是从当前模块或当前库中查找.防止“影子遮蔽” Shadowing
                // 在编写库crate时,尤其在宏中永远使用绝对路径::或$crate
                // transmute原型只需要src的类型,rustc会根据transmute的位置自动推断dst的类型
                &[$(::core::mem::transmute($arg as usize)),*],
                // SpoofKind是一个enum,代表要执行的操作类型
                $crate::SpoofKind::Function,
            )
        }
    };
}

/// Invokes a Windows native syscall using a spoofed stack.
///
/// # Examples
/// 
/// ```
/// use core::ptr;
/// use uwd::{AsPointer, syscall};
///
/// let mut addr = ptr::null_mut::<core::ffi::c_void>();
/// let mut size = (1 << 12) as usize;
///
/// let status = syscall!(
///     "NtAllocateVirtualMemory",
///     -1isize,
///     addr.as_ptr_mut(),
///     0,
///     size.as_ptr_mut(),
///     0x3000,
///     0x04
/// ).unwrap() as i32;
///
/// assert_eq!(status, 0);
/// ```
#[macro_export]
macro_rules! syscall {
    ($name:expr, $($arg:expr),* $(,)?) => {
        unsafe {
            $crate::__private::spoof(
                core::ptr::null_mut(),
                &[$(::core::mem::transmute($arg as usize)),*],
                $crate::SpoofKind::Syscall($name),
            )
        }
    };
}

/// 由于该mod在宏中展开了使用了spoof函数,而宏展开需要将spoof标记为pub.但标记为pub函数spoof会出现在docs.rs的官方文档中,导致调用这个库的用户以为这是一个可直接调用的api.而作为公开的api如果后续修改了函数,就会破坏语义化.导致原来的调用不适配更改后的函数定义
/// 
/// 解决方案是对这个mod使用#[doc(hidden)],生成的docs.rs文档会隐藏这个mod
/// 
/// 重新定义一个mod为了模块化重构的方便及形成一个功能模块
#[doc(hidden)]
pub mod __private {
    use core::ffi::c_void;
    // 将父mod中所有可见的内容,全部拉入到当前mod的作用域
    // 不仅仅是父mod中pub内容,父mod定义的所有内容在子mod中都可以用,以及父mod中use的的内容在子mod中也可以用.因为子mod对父mod是完全透明的,实质上不用use super::* 子mod也可以访问父mod的私有成员,但需要加上super:: 这里加上这个use后可以省略super::这个前缀
    // 例外1. 父mod中定义的宏没有使用#[macro_export]导出,那子mod中的super::* 抓不到这个宏,此外宏的可见性遵循从上而下,因此宏定义通常放在最前面;
    // 2. 同名冲突,编译器会优先使用子mod中的定义
    // 3. 只对上一级的mod有效,不能无限向上
    use super::*;

    /// Performs call stack spoofing in `synthetic` mode.
    #[cfg(not(feature = "desync"))]
    pub fn spoof(addr: *mut c_void, args: &[*const c_void], kind: SpoofKind) -> Result<*mut c_void> {
        // Max 11 args
        // 为什么限制在11个参数以内,详见源码解析/types.md
        if args.len() > 11 {
            bail!(s!("too many arguments"));
        }

        // Function pointer must be valid unless syscall spoof
        // 如果当前伪装的是普通函数调用(非系统调用),那么传入的addr不能为空.因为syscall是通过ssn定位的
        // s!是否足够安全?anyhow在cargon.toml中禁用默认特性(default-feature=false)是否可在no_std下运行
        // 初步结论是模拟NTSTATUS的状态来表示错误
        if let SpoofKind::Function = kind && addr.is_null() {
            bail!(s!("null function address"));
        }

        let mut config = Config::default();

        // Resolve kernelbase
        let kernelbase = get_module_address(2737729883u32, Some(murmur3));

        // Parse unwind table
        let pe_kernelbase = Unwind::new(PE::parse(kernelbase));
        let tables = pe_kernelbase
            .entries()
            .context(s!(
                "failed to read IMAGE_RUNTIME_FUNCTION entries from .pdata section"
            ))?;

        // Resolve APIs
        let ntdll = get_module_address(2788516083u32, Some(murmur3));
        if ntdll.is_null() {
            bail!(s!("ntdll.dll not found"));
        }

        let kernel32 = get_module_address(2808682670u32, Some(murmur3));
        let rlt_user_addr = get_proc_address(ntdll, 1578834099u32, Some(murmur3));
        let base_thread_addr = get_proc_address(kernel32, 4083630997u32, Some(murmur3));

        config.rtl_user_addr = rlt_user_addr;
        config.base_thread_addr = base_thread_addr;

        // Unwind lookup
        let pe_ntdll = Unwind::new(PE::parse(ntdll));
        let rtl_user_runtime = pe_ntdll
            .function_by_offset(rlt_user_addr as u32 - ntdll as u32)
            .context(s!("RtlUserThreadStart unwind info not found"))?;

        let pe_kernel32 = Unwind::new(PE::parse(kernel32));
        let base_thread_runtime = pe_kernel32
            .function_by_offset(base_thread_addr as u32 - kernel32 as u32)
            .context(s!("BaseThreadInitThunk unwind info not found"))?;

        // Stack sizes
        let rtl_user_size = ignoring_set_fpreg(ntdll, rtl_user_runtime)
            .context(s!("RtlUserThreadStart stack size not found"))?;
        
        let base_thread_size = ignoring_set_fpreg(kernel32, base_thread_runtime)
            .context(s!("BaseThreadInitThunk stack size not found"))?;

        config.rtl_user_thread_size = rtl_user_size as u64;
        config.base_thread_size = base_thread_size as u64;

        // First prologue
        let first_prolog = Prolog::find_prolog(kernelbase, tables)
            .context(s!("first prolog not found"))?;
        
        config.first_frame_fp = (first_prolog.frame + first_prolog.offset as u64) as *const c_void;
        config.first_frame_size = first_prolog.stack_size as u64;

        // Second prologue
        let second_prolog = Prolog::find_push_rbp(kernelbase, tables)
            .context(s!("second prolog not found"))?;
        
        config.second_frame_fp = (second_prolog.frame + second_prolog.offset as u64) as *const c_void;
        config.second_frame_size = second_prolog.stack_size as u64;
        config.rbp_stack_offset = second_prolog.rbp_offset as u64;

        // Gadget: `add rsp, 0x58; ret`
        let (add_rsp_addr, size) = find_gadget(kernelbase, &[0x48, 0x83, 0xC4, 0x58, 0xC3], tables)
            .context(s!("add rsp gadget not found"))?;
        
        config.add_rsp_gadget = add_rsp_addr as *const c_void;
        config.add_rsp_frame_size = size as u64;

        // Gadget: `jmp rbx`
        let (jmp_rbx_addr, size) = find_gadget(kernelbase, &[0xFF, 0x23], tables)
            .context(s!("jmp rbx gadget not found"))?;
        
        config.jmp_rbx_gadget = jmp_rbx_addr as *const c_void;
        config.jmp_rbx_frame_size = size as u64;

        // Prepare arguments
        let len = args.len();
        config.number_args = len as u64;
        
        for (i, &arg) in args.iter().take(len).enumerate() {
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

        // Handle syscall spoofing
        match kind {
            SpoofKind::Function => config.spoof_function = addr,
            SpoofKind::Syscall(name) => {
                let addr = get_proc_address(ntdll, name, None);
                if addr.is_null() {
                    bail!(s!("get_proc_address returned null"));
                }

                config.is_syscall = true as u32;
                config.ssn = dinvk::ssn(name, ntdll).context(s!("ssn not found"))?.into();
                config.spoof_function = dinvk::get_syscall_address(addr)
                    .context(s!("syscall address not found"))? as *const c_void;
            }
        }

        Ok(unsafe { SpoofSynthetic(&mut config) })
    }

    /// Performs call stack spoofing in `desync` mode.
    #[cfg(feature = "desync")]
    pub fn spoof(addr: *mut c_void, args: &[*const c_void], kind: SpoofKind) -> Result<*mut c_void> {
        // Max 11 args
        if args.len() > 11 {
            bail!(s!("too many arguments"));
        }

        // Function pointer must be valid unless syscall spoof
        if let SpoofKind::Function = kind && addr.is_null() {
            bail!(s!("null function address"));
        }

        let mut config = Config::default();

        // Resolve kernelbase
        let kernelbase = get_module_address(2737729883u32, Some(murmur3));

        // Parse unwind table
        let pe = Unwind::new(PE::parse(kernelbase));
        let tables = pe
            .entries()
            .context(s!(
                "failed to read IMAGE_RUNTIME_FUNCTION entries from .pdata section"
            ))?;

        // Locate a return address from BaseThreadInitThunk on the current stack
        config.return_address = find_base_thread_return_address()
            .context(s!("return address not found"))? as *const c_void;

        // First prologue
        let first_prolog = Prolog::find_prolog(kernelbase, tables)
            .context(s!("first prolog not found"))?;
        
        config.first_frame_fp = (first_prolog.frame + first_prolog.offset as u64) as *const c_void;
        config.first_frame_size = first_prolog.stack_size as u64;

        // Second prologue
        let second_prolog = Prolog::find_push_rbp(kernelbase, tables)
            .context(s!("second prolog not found"))?;
        
        config.second_frame_fp = (second_prolog.frame + second_prolog.offset as u64) as *const c_void;
        config.second_frame_size = second_prolog.stack_size as u64;
        config.rbp_stack_offset = second_prolog.rbp_offset as u64;

        // Gadget: `add rsp, 0x58; ret`
        let (add_rsp_addr, size) = find_gadget(kernelbase, &[0x48, 0x83, 0xC4, 0x58, 0xC3], tables)
            .context(s!("add rsp gadget not found"))?;

        config.add_rsp_gadget = add_rsp_addr as *const c_void;
        config.add_rsp_frame_size = size as u64;

        // Gadget: `jmp rbx`
        let (jmp_rbx_addr, size) = find_gadget(kernelbase, &[0xFF, 0x23], tables)
            .context(s!("jmp rbx gadget not found"))?;

        config.jmp_rbx_gadget = jmp_rbx_addr as *const c_void;
        config.jmp_rbx_frame_size = size as u64;

        // Prepare arguments
        let len = args.len();
        config.number_args = len as u64;
        
        for (i, &arg) in args.iter().take(len).enumerate() {
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

        // Handle syscall spoofing
        match kind {
            SpoofKind::Function => config.spoof_function = addr,
            SpoofKind::Syscall(name) => {
                let ntdll = get_module_address(2788516083u32, Some(murmur3));
                if ntdll.is_null() {
                    bail!(s!("ntdll.dll not found"));
                }

                let addr = get_proc_address(ntdll, name, None);
                if addr.is_null() {
                    bail!(s!("get_proc_address returned null"));
                }

                config.is_syscall = true as u32;
                config.ssn = dinvk::ssn(name, ntdll).context(s!("ssn not found"))?.into();
                config.spoof_function = dinvk::get_syscall_address(addr)
                    .context(s!("syscall address not found"))? as *const c_void;
            }
        }

        Ok(unsafe { Spoof(&mut config) })
    }
}

/// Metadata extracted from a function prologue that is suitable for spoofing.
#[derive(Copy, Clone, Default)]
struct Prolog {
    /// Address of the selected function frame.
    frame: u64,

    /// Total stack space reserved by the function.
    stack_size: u32,

    /// Offset inside the function where a valid instruction pattern was found.
    offset: u32,

    /// Offset in the stack where `rbp` is pushed or saved.
    rbp_offset: u32,
}

impl Prolog {
    /// Finds the first prologue in the unwind table that looks safe for spoofing.
    ///
    /// This scans the RUNTIME_FUNCTION entries for a function that:
    /// - Allocates a stack frame.
    /// - Has a predictable prologue layout.
    fn find_prolog(module_base: *mut c_void, runtime_table: &[IMAGE_RUNTIME_FUNCTION]) -> Option<Self> {
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

    /// Finds a prologue that uses `push rbp` and an RBP-based frame.
    ///
    /// This is useful when spoofing techniques rely on classic frame-pointer
    /// based layouts rather than purely RSP-based stack frames.
    fn find_push_rbp(module_base: *mut c_void, runtime_table: &[IMAGE_RUNTIME_FUNCTION]) -> Option<Self> {
        let mut prologs = runtime_table
            .iter()
            .filter_map(|runtime| {
                let (rbp_offset, stack_size) = rbp_offset(module_base, runtime)?;
                if rbp_offset == 0 || stack_size == 0 || stack_size <= rbp_offset {
                    return None;
                }

                let offset = find_valid_instruction_offset(module_base, runtime)?;
                let frame = module_base as u64 + runtime.BeginAddress as u64;
                Some(
                    Self {
                        frame,
                        stack_size,
                        offset,
                        rbp_offset,
                    }
                )
            })
            .collect::<Vec<Self>>();

        if prologs.is_empty() {
            return None;
        }

        // The first frame is often not suitable on many Windows versions.
        prologs.remove(0);

        // Shuffle to reduce pattern predictability.
        shuffle(&mut prologs);

        prologs.first().copied()
    }
}

/// Determines whether RBP is pushed or saved in a spoof-compatible manner and
/// computes the total stack size for a function.
///
/// This inspects the unwind codes associated with the `IMAGE_RUNTIME_FUNCTION`
/// entry to determine if the function frame uses a layout suitable for
/// call stack spoofing.
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
            // Accessing `UNWIND_CODE` based on the index
            let unwind_code = unwind_code.add(i);

            // Information used in operation codes
            let op_info = (*unwind_code).Anonymous.OpInfo() as usize;
            let unwind_op = (*unwind_code).Anonymous.UnwindOp();

            match UNWIND_OP_CODES::try_from(unwind_op) {
                // Saves a non-volatile register on the stack.
                //
                // Example: push <reg>
                Ok(UWOP_PUSH_NONVOL) => {
                    if Registers::Rsp == op_info {
                        return None;
                    }

                    if Registers::Rbp == op_info {
                        if rbp_pushed {
                            return None;
                        }

                        rbp_pushed = true;
                        stack_offset = total_stack;
                    }

                    total_stack += 8;
                    i += 1;
                }

                // Allocates large space on the stack.
                // - OpInfo == 0: The next slot contains the /8 size of the allocation (maximum 512 KB - 8).
                // - OpInfo == 1: The next two slots contain the full size of the allocation (up to 4 GB - 8).
                //
                // Example (OpInfo == 0): sub rsp, 0x100 ; Allocates 256 bytes
                // Example (OpInfo == 1): sub rsp, 0x10000 ; Allocates 65536 bytes (two slots used)
                Ok(UWOP_ALLOC_LARGE) => {
                    if (*unwind_code).Anonymous.OpInfo() == 0 {
                        // Case 1: OpInfo == 0 (Size in 1 slot, divided by 8)
                        // Multiplies by 8 to the actual value

                        let frame_offset = ((*unwind_code.add(1)).FrameOffset as i32) * 8;
                        total_stack += frame_offset as u32;

                        // Consumes 2 slots (1 for the instruction, 1 for the size divided by 8)
                        i += 2
                    } else {
                        // Case 2: OpInfo == 1 (Size in 2 slots, 32 bits)
                        let frame_offset = *(unwind_code.add(1) as *mut i32);
                        total_stack += frame_offset as u32;

                        // Consumes 3 slots (1 for the instruction, 2 for the full size)
                        i += 3
                    }
                }

                // Allocates small space in the stack.
                //
                // Example (OpInfo = 3): sub rsp, 0x20  ; Aloca 32 bytes (OpInfo + 1) * 8
                Ok(UWOP_ALLOC_SMALL) => {
                    total_stack += ((op_info + 1) * 8) as u32;
                    i += 1;
                }

                // UWOP_SAVE_NONVOL: Saves the contents of a non-volatile register in a specific position on the stack.
                // - Reg: Name of the saved register.
                // - FrameOffset: Offset indicating where the value of the register is saved.
                //
                // Example: mov [rsp + 0x40], rsi ; Saves the contents of RSI in RSP + 0x40
                Ok(UWOP_SAVE_NONVOL) => {
                    if Registers::Rsp == op_info {
                        return None;
                    }

                    if Registers::Rbp == op_info {
                        if rbp_pushed {
                            return None;
                        }

                        let offset = (*unwind_code.add(1)).FrameOffset * 8;
                        stack_offset = total_stack + offset as u32;
                        rbp_pushed = true;
                    }

                    i += 2;
                }

                // Saves a non-volatile register to a stack address with a long offset.
                // - Reg: Name of the saved register.
                // - FrameOffset: Long offset indicating where the value of the register is saved.
                //
                // Example: mov [rsp + 0x1040], rsi ; Saves the contents of RSI in RSP + 0x1040.
                Ok(UWOP_SAVE_NONVOL_BIG) => {
                    if Registers::Rsp == op_info {
                        return None;
                    }

                    if Registers::Rbp == op_info {
                        if rbp_pushed {
                            return None;
                        }

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

        // If there is a chain unwind structure, it too must be processed
        // recursively and included in the stack size calculation.
        if (flag & UNW_FLAG_CHAININFO) != 0 {
            let count = (*unwind_info).CountOfCodes as usize;
            let index = if count & 1 == 1 { count + 1 } else { count };
            let runtime = unwind_code.add(index) as *const IMAGE_RUNTIME_FUNCTION;
            if let Some((_, child_total)) = rbp_offset(module, &*runtime) {
                total_stack += child_total;
            } else {
                return None;
            }
        }

        Some((stack_offset, total_stack))
    }
}

/// Computes stack frame metadata while rejecting `setfp` frames.
///
/// Used when locating suitable prologues for spoofed call frames.
pub fn stack_frame(module: *mut c_void, runtime: &IMAGE_RUNTIME_FUNCTION) -> Option<(bool, u32)> {
    unsafe {
        let unwind_info = (module as usize + runtime.UnwindData as usize) as *mut UNWIND_INFO;
        let unwind_code = (unwind_info as *mut u8).add(4) as *mut UNWIND_CODE;
        let flag = (*unwind_info).VersionFlags.Flags();

        let mut i = 0usize;
        let mut set_fpreg_hit = false;
        let mut total_stack = 0i32;
        while i < (*unwind_info).CountOfCodes as usize {
            // Accessing `UNWIND_CODE` based on the index
            let unwind_code = unwind_code.add(i);

            // Information used in operation codes
            let op_info = (*unwind_code).Anonymous.OpInfo() as usize;
            let unwind_op = (*unwind_code).Anonymous.UnwindOp();

            match UNWIND_OP_CODES::try_from(unwind_op) {
                // Saves a non-volatile register on the stack.
                //
                // Example: push <reg>
                Ok(UWOP_PUSH_NONVOL) => {
                    if Registers::Rsp == op_info && !set_fpreg_hit {
                        return None;
                    }

                    total_stack += 8;
                    i += 1;
                }

                // Allocates small space in the stack.
                //
                // Example (OpInfo = 3): sub rsp, 0x20  ; Aloca 32 bytes (OpInfo + 1) * 8
                Ok(UWOP_ALLOC_SMALL) => {
                    total_stack += ((op_info + 1) * 8) as i32;
                    i += 1;
                }

                // Allocates large space on the stack.
                // - OpInfo == 0: The next slot contains the /8 size of the allocation (maximum 512 KB - 8).
                // - OpInfo == 1: The next two slots contain the full size of the allocation (up to 4 GB - 8).
                //
                // Example (OpInfo == 0): sub rsp, 0x100 ; Allocates 256 bytes
                // Example (OpInfo == 1): sub rsp, 0x10000 ; Allocates 65536 bytes (two slots used)
                Ok(UWOP_ALLOC_LARGE) => {
                    if (*unwind_code).Anonymous.OpInfo() == 0 {
                        // Case 1: OpInfo == 0 (Size in 1 slot, divided by 8)
                        // Multiplies by 8 to the actual value

                        let frame_offset = ((*unwind_code.add(1)).FrameOffset as i32) * 8;
                        total_stack += frame_offset;

                        // Consumes 2 slots (1 for the instruction, 1 for the size divided by 8)
                        i += 2
                    } else {
                        // Case 2: OpInfo == 1 (Size in 2 slots, 32 bits)
                        let frame_offset = *(unwind_code.add(1) as *mut i32);
                        total_stack += frame_offset;

                        // Consumes 3 slots (1 for the instruction, 2 for the full size)
                        i += 3
                    }
                }

                // UWOP_SAVE_NONVOL: Saves the contents of a non-volatile register in a specific position on the stack.
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
                // Subtract `FrameOffset << 4` from the stack total.
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

        // If there is a chain unwind structure, it too must be processed
        // recursively and included in the stack size calculation.
        if (flag & UNW_FLAG_CHAININFO) != 0 {
            let count = (*unwind_info).CountOfCodes as usize;
            let index = if count & 1 == 1 { count + 1 } else { count };
            let runtime = unwind_code.add(index) as *const IMAGE_RUNTIME_FUNCTION;
            if let Some((chained_fpreg_hit, chained_stack)) = stack_frame(module, &*runtime) {
                total_stack += chained_stack as i32;
                set_fpreg_hit |= chained_fpreg_hit;
            } else {
                return None;
            }
        }

        Some((set_fpreg_hit, total_stack as u32))
    }
}

/// Computes the total stack frame size of a function while ignoring any `setfp` frames. 
/// Useful for identifying spoof-compatible RUNTIME_FUNCTION entries.
pub fn ignoring_set_fpreg(module: *mut c_void, runtime: &IMAGE_RUNTIME_FUNCTION) -> Option<u32> {
    unsafe {

        // 指向UNWIND_INFO结构体
        let unwind_info = (module as usize + runtime.UnwindData as usize) as *mut UNWIND_INFO;

        // 跳过UNWIND_INFO前4个字节,指向真正的操作码数组
        let unwind_code = (unwind_info as *mut u8).add(4) as *mut UNWIND_CODE;
        // 以上,win规定,prolog中每一条修改rsp或保存寄存器的指令,都必须在此处有一个对应的操作码

        // *unwind_info内存中该结构体的首地址(原始指针在unsafe中用*解引用,将该指针指向区域具象化一个结构体实例)
        // Flags()由bitfield! 生成的方法,通过mask运算从VersionFlags这个8位的字段中提取高5位(标志位).不同标志位代表函数的不同特性,如0x4表示是否有链式回溯,0x1表示函数是否有异常处理
        let flag = (*unwind_info).VersionFlags.Flags();

        // 对UNWIND_CODE的计数
        let mut i = 0usize;
        // 该函数每解析一个涉及栈增长的操作码,将对应的字节数加到total_stack.最终会得到需要的函数栈帧深度,即增加了多少字节
        let mut total_stack = 0u32;

        // CountOfCodes 表示UNWIND_CODE数组中有多少元素
        while i < (*unwind_info).CountOfCodes as usize {
            // Accessing `UNWIND_CODE` based on the index
            // add(i)指针算数运算,将指针向后移动i个UNWIND_CODE结构体宽度()
            let unwind_code = unwind_code.add(i);

            // Information used in operation codes
            // Opinfo()和UnwindOp()这两个从一个UNWIND_CODE回溯操作码的高八位获取各自的位值,该位值代表不同的栈操作(Unwindop来表示压栈\分配\移动)以及栈操作的具体情况(Opinfo来表示谁被压栈\分配多少\移动到哪里)
            let op_info = (*unwind_code).Anonymous.OpInfo() as usize;
            let unwind_op = (*unwind_code).Anonymous.UnwindOp();

            // 根据不同的栈操作,调整对应的寄存器
            match UNWIND_OP_CODES::try_from(unwind_op) {
                // Saves a non-volatile register on the stack.
                //
                // 针对栈push操作.Example: push <reg>
                Ok(UWOP_PUSH_NONVOL) => {
                    if Registers::Rsp == op_info {
                        return None;
                    }

                    // 虽然UWOP_PUSH_NONVOL是16位,但此时函数栈的操作是push只针对寄存器,而寄存器是8位的
                    total_stack += 8;
                    i += 1;
                }

                // Allocates small space in the stack.即prolog对小规模栈空间分配的元数据记录情况
                //
                // Example (OpInfo = 3): sub rsp, 0x20  ; Aloca 32 bytes (OpInfo + 1) * 8
                Ok(UWOP_ALLOC_SMALL) => {
                    total_stack += ((op_info + 1) * 8) as u32;
                    i += 1;
                }

                // Allocates large space on the stack.
                // - OpInfo == 0: The next slot contains the /8 size of the allocation (maximum 512 KB - 8).
                // - OpInfo == 1: The next two slots contain the full size of the allocation (up to 4 GB - 8).
                //
                // Example (OpInfo == 0): sub rsp, 0x100 ; Allocates 256 bytes
                // Example (OpInfo == 1): sub rsp, 0x10000 ; Allocates 65536 bytes (two slots used)
                Ok(UWOP_ALLOC_LARGE) => {
                    if (*unwind_code).Anonymous.OpInfo() == 0 {
                        // Case 1: OpInfo == 0 (Size in 1 slot, divided by 8)
                        // Multiplies by 8 to the actual value

                        let frame_offset = ((*unwind_code.add(1)).FrameOffset as i32) * 8;
                        total_stack += frame_offset as u32;

                        // Consumes 2 slots (1 for the instruction, 1 for the size divided by 8)
                        i += 2
                    } else {
                        // Case 2: OpInfo == 1 (Size in 2 slots, 32 bits)
                        let frame_offset = *(unwind_code.add(1) as *mut i32);
                        total_stack += frame_offset as u32;

                        // Consumes 3 slots (1 for the instruction, 2 for the full size)
                        i += 3
                    }
                }

                // UWOP_SAVE_NONVOL: Saves the contents of a non-volatile register in a specific position on the stack.
                // - Reg: Name of the saved register.
                // - FrameOffset: Offset indicating where the value of the register is saved.
                //
                // Example: mov [rsp + 0x40], rsi ; Saves the contents of RSI in RSP + 0x40
                Ok(UWOP_SAVE_NONVOL) => {
                    if Registers::Rsp == op_info {
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
                    if Registers::Rsp == op_info {
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

                // Ignoring.
                Ok(UWOP_SET_FPREG) => i += 1,

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

        // If there is a chain unwind structure, it too must be processed
        // recursively and included in the stack size calculation.
        if (flag & UNW_FLAG_CHAININFO) != 0 {
            let count = (*unwind_info).CountOfCodes as usize;
            let index = if count & 1 == 1 { count + 1 } else { count };
            let runtime = unwind_code.add(index) as *const IMAGE_RUNTIME_FUNCTION;
            if let Some(chained_stack) = ignoring_set_fpreg(module, &*runtime) {
                total_stack += chained_stack;
            } else {
                return None;
            }
        }

        Some(total_stack)
    }
}

/// Trait for safely converting any reference or mutable reference into a raw
/// pointer usable in spoofing routines.
pub trait AsPointer {
    /// Returns a raw immutable pointer to `self`.
    fn as_ptr_const(&self) -> *const c_void;

    /// Returns a raw mutable pointer to `self`.
    fn as_ptr_mut(&mut self) -> *mut c_void;
}

impl<T> AsPointer for T {
    #[inline(always)]
    fn as_ptr_const(&self) -> *const c_void {
        self as *const _ as *const c_void
    }

    #[inline(always)]
    fn as_ptr_mut(&mut self) -> *mut c_void {
        self as *mut _ as *mut c_void
    }
}

/// Specifies the spoofing mode used by the engine.
pub enum SpoofKind<'a> {
    /// Spoofs a direct function call.
    Function,

    /// Spoofs a syscall using its name.
    Syscall(&'a str),
}

#[cfg(test)]
mod tests {
    use core::ptr;
    use alloc::boxed::Box;
    use super::*;

    #[test]
    fn test_spoof() -> Result<(), Box<dyn core::error::Error>> {
        let kernel32 = get_module_address("kernel32.dll", None);
        let virtual_alloc = get_proc_address(kernel32, "VirtualAlloc", None);   
        let addr = spoof!(virtual_alloc, ptr::null_mut::<c_void>(), 1 << 12, 0x3000, 0x04)?;
        assert_ne!(addr, ptr::null_mut());

        Ok(())
    }

    #[test]
    fn test_syscall() -> Result<(), Box<dyn core::error::Error>> {
        let mut addr = ptr::null_mut::<c_void>();
        let mut size = (1 << 12) as usize;
        let status = syscall!("NtAllocateVirtualMemory", -1isize, addr.as_ptr_mut(), 0, size.as_ptr_mut(), 0x3000, 0x04)? as i32;
        assert_eq!(status, 0);

        Ok(())
    }
}