//! Windows API and NT system call wrappers.

// c_void对应c中的void类型,表示未知类型的指针
// null_mut返回一个 null mutable raw pointer(*mut T) 
use core::{ffi::c_void, ptr::null_mut};
// 使用如s!("NTDLL.DLL")形式,
use obfstr::obfstr as s;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use crate::breakpoint::{is_breakpoint_enabled, set_breakpoint, WINAPI, CURRENT_API};
use crate::module::{get_ntdll_address, get_module_address};
use crate::{types::*, dinvoke};

/// Wrapper for the `LoadLibraryA` function from `KERNEL32.DLL`.
/// 
pub fn LoadLibraryA(module: &str) -> *mut c_void {

    // 用户传入的字符串(如 user.dll)转为c格式,存入name
    let name = alloc::format!("{module}\0");

    // 找到LoadLibraryA所在的dll基址,即KERNEL32.dll基址
    let kernel32 = get_module_address(s!("KERNEL32.DLL"), None);

    // 无痕加载dll,不经过windows系统加载器,手动在内存中找到`LoadLibraryA` 函数的地址并执行它，从而加载一个新的 DLL 到当前进程中
    dinvoke!(
        kernel32, // 去哪找函数？去 KERNEL32.DLL 找
        // 编译时混淆LoadLibraryA这个字符串,在执行代码的瞬间解码到当前线程的栈内存,且用完就丢弃
        s!("LoadLibraryA"), // 找哪个函数？找真正的系统的 LoadLibraryA
        LoadLibraryAFn, // 函数长什么样？（函数原型定义）
        name.as_ptr().cast() // 给这个函数传什么参数？传的就是上面那个name
    )
    .unwrap_or(null_mut())
}

/// Wrapper for the `NtAllocateVirtualMemory` function from `NTDLL.DLL`.
#[allow(unused_mut)]
// 两个mut参数,就是即将修改进行欺骗的关键
pub fn NtAllocateVirtualMemory(
    mut process_handle: HANDLE,
    base_address: *mut *mut c_void,//双指针
    zero_bits: usize,
    region_size: *mut usize,//双指针
    allocation_type: u32,
    mut protect: u32,
) -> NTSTATUS {
    cfg_if::cfg_if! {
        if #[cfg(any(target_arch = "x86", target_arch = "x86_64"))] {
            // Handle debugging breakpoints, if enabled.
            if is_breakpoint_enabled() {
                
                // 保存真实意图需要用到的参数
                // 修改参数之前,必须保存一份真实意图的参数到CURRENT_API中
                // 之后触发硬件断点后,VEH(异常处理函数)会从这里读取要执行的参数,并填回寄存器或堆栈中,让内核执行
                unsafe {
                    CURRENT_API = Some(WINAPI::NtAllocateVirtualMemory {
                        ProcessHandle: process_handle,
                        Protect: protect,
                    });
                }
                
                // Argument tampering before syscall execution.
                // Modifies the memory protection to PAGE_READONLY.
                // 修改为无害的只读权限
                protect = 0x02;
        
                // Replaces the process handle with an arbitrary value.
                process_handle = -23isize as HANDLE; 
                // 将原本向申请的RWX改为R,原本指向自身的进程句柄-1改为-23(或其他任意值)
                // 如果spoof失败(断点未触发).系统会因调用无效句柄而直接返回错误,而不会导致程序崩溃或执行其他恶意内容.同时向EDR AV展示了一个意图无法判定的调用


                // Locate and set a breakpoint on the NtAllocateVirtualMemory syscall.
                // get_ntdll_address(),遍历PEB->Ldr->InMemoryOrderModuleList 找到ntdll.dll的基址
                // get_proc_address 解析ntdll.dll的导出表找到NtAllocateVirtualMemory函数地址
                let addr = super::module::get_proc_address(get_ntdll_address(), s!("NtAllocateVirtualMemory"), None);

                // get_syscall_address 返回syscall指令机器码的内存地址
                // set_breakpoint 设置断点
                if let Some(syscall_addr) = super::get_syscall_address(addr) {
                    set_breakpoint(syscall_addr);
                }
            }
        }
    }

    // 发起传入的虚假参数的调用,spoof EDR Av
    dinvoke!(
        get_ntdll_address(),
        s!("NtAllocateVirtualMemory"),
        NtAllocateVirtualMemoryFn,
        process_handle, // -23
        base_address,
        zero_bits,
        region_size,
        allocation_type, 
        protect         // 0x02(Read-Only)
    )
    .unwrap_or(STATUS_UNSUCCESSFUL)
}
// 1. 准备：保存真实参数（RWX）。
// 2. 伪装：修改参数为假参数（Read-Only）。
// 3. 设伏：在 syscall 处下断点。
// 4. 出发：调用函数。
// 5. EDR 检查：EDR 看到是 Read-Only，放行。
// 6. 触发断点：代码运行到 syscall，砰！触发异常。
// 7. 偷天换日 (VEH)：
//    * 程序的异常处理函数（在 src/breakpoint.rs 中）捕获异常。
//    * 它发现是 NtAllocateVirtualMemory 触发的。
//    * 它从 CURRENT_API 取出真实的 RWX 参数。
//    * 它修改 CPU 寄存器/堆栈，把假的 Read-Only 替换回 RWX。
// 8. 进内核：异常处理结束，恢复执行 syscall。此时进入内核的参数已经是 RWX 了。
// 9. 结果：你成功申请到了可执行内存，而 EDR 一无所知。


/// Wrapper for the `NtProtectVirtualMemory` function from `NTDLL.DLL`.
#[allow(unused_mut)]
pub fn NtProtectVirtualMemory(
    mut process_handle: *mut c_void,
    base_address: *mut *mut c_void,
    region_size: *mut usize,
    mut new_protect: u32,
    old_protect: *mut u32,
) -> NTSTATUS {
    cfg_if::cfg_if! {
        if #[cfg(any(target_arch = "x86", target_arch = "x86_64"))] {
            // Handle debugging breakpoints, if enabled.
            if is_breakpoint_enabled() {
                unsafe {
                    CURRENT_API = Some(WINAPI::NtProtectVirtualMemory {
                        ProcessHandle: process_handle,
                        NewProtect: new_protect,
                    });
                }
                
                // Modifies the memory protection to PAGE_READONLY.
                new_protect = 0x02;

                // Replaces the process handle with an arbitrary value.
                process_handle = -23isize as HANDLE; 

                // Locate and set a breakpoint on the NtProtectVirtualMemory syscall.
                let addr = super::module::get_proc_address(get_ntdll_address(), s!("NtProtectVirtualMemory"), None);
                if let Some(syscall_addr) = super::get_syscall_address(addr) {
                    set_breakpoint(syscall_addr);
                }
            }
        }
    }

    dinvoke!(
        get_ntdll_address(),
        s!("NtProtectVirtualMemory"),
        NtProtectVirtualMemoryFn,
        process_handle,
        base_address,
        region_size,
        new_protect, 
        old_protect
    )
    .unwrap_or(STATUS_UNSUCCESSFUL)
}

/// Wrapper for the `NtCreateThreadEx` function from `NTDLL.DLL`. 
#[allow(unused_mut)]
// 定义为mut的参数,即为实现硬件断点参数欺骗所必须使用的
pub fn NtCreateThreadEx(
    mut thread_handle: *mut HANDLE,// 双指针,
    mut desired_access: u32,
    mut object_attributes: *mut OBJECT_ATTRIBUTES,
    mut process_handle: HANDLE,
    start_routine: *mut c_void,
    argument: *mut c_void,
    create_flags: u32,
    zero_bits: usize,
    stack_size: usize,
    maximum_stack_size: usize,
    attribute_list: *mut PS_ATTRIBUTE_LIST
) -> NTSTATUS {
    cfg_if::cfg_if! {
        if #[cfg(any(target_arch = "x86", target_arch = "x86_64"))] {
            use alloc::boxed::Box;

            // Handle debugging breakpoints, if enabled.
            if is_breakpoint_enabled() {
                unsafe {
                    CURRENT_API = Some(WINAPI::NtCreateThreadEx {
                        ProcessHandle: process_handle,
                        ThreadHandle: thread_handle,
                        DesiredAccess: desired_access,
                        ObjectAttributes: object_attributes
                    });
                }
                
                // Replacing process handle and thread handle with arbitrary values.
                process_handle = -12isize as HANDLE;
                thread_handle = -43isize as *mut HANDLE;

                // Modifying desired access permissions.
                desired_access = 0x80;

                // Modifying object attributes before the syscall.
                object_attributes = Box::leak(Box::new(OBJECT_ATTRIBUTES::default()));

                // Locate and set a breakpoint on the NtCreateThreadEx syscall.
                let addr = super::module::get_proc_address(get_ntdll_address(), s!("NtCreateThreadEx"), None);
                if let Some(addr) = super::get_syscall_address(addr) {
                    set_breakpoint(addr);
                }
            }
        }
    }

    //  dinvoke! 宏底层是通过 core::mem::transmute将地址强制转换为你提供的函数指针类型 (`$ty`)
    // 所有的参数都需要在dinvk!使用时完整的传入
    dinvoke!(
        get_ntdll_address(),
        s!("NtCreateThreadEx"),
        NtCreateThreadExFn,
        thread_handle,
        desired_access,
        object_attributes,
        process_handle,
        start_routine,
        argument,
        create_flags,
        zero_bits,
        stack_size,
        maximum_stack_size,
        attribute_list
    )
    .unwrap_or(STATUS_UNSUCCESSFUL)
}

/// Wrapper for the `NtWriteVirtualMemory` function from `NTDLL.DLL`.
#[allow(unused_mut)]
pub fn NtWriteVirtualMemory(
    mut process_handle: HANDLE,
    base_address: *mut c_void,
    mut buffer: *mut c_void,
    mut number_of_bytes_to_write: usize,
    number_of_bytes_written: *mut usize,
) -> NTSTATUS {
    cfg_if::cfg_if! {
        if #[cfg(any(target_arch = "x86", target_arch = "x86_64"))] {
            // Handle debugging breakpoints, if enabled.
            if is_breakpoint_enabled() {
                unsafe {
                    CURRENT_API = Some(WINAPI::NtWriteVirtualMemory {
                        ProcessHandle: process_handle,
                        Buffer: buffer,
                        NumberOfBytesToWrite: number_of_bytes_written
                    });
                }

                // Replacing process handle with an arbitrary value.
                process_handle = -90isize as HANDLE;

                // Modifying buffer and size before syscall execution.
                let temp = [0u8; 10];
                buffer = temp.as_ptr().cast_mut().cast();
                number_of_bytes_to_write = temp.len();

                // Locate and set a breakpoint on the NtWriteVirtualMemory syscall.
                let addr = super::module::get_proc_address(get_ntdll_address(), s!("NtWriteVirtualMemory"), None);
                if let Some(addr) = super::get_syscall_address(addr) {
                    set_breakpoint(addr);
                }
            }
        }
    }
    
    dinvoke!(
        get_ntdll_address(),
        s!("NtWriteVirtualMemory"),
        NtWriteVirtualMemoryFn,
        process_handle,
        base_address,
        buffer,
        number_of_bytes_to_write,
        number_of_bytes_written
    )
    .unwrap_or(STATUS_UNSUCCESSFUL)
}

/// Wrapper for the `AddVectoredExceptionHandler` function from `KERNEL32.DLL`.
/// 
/// 注册一个全局异常处理函数（VEH）,当触发EXCEPTION_SINGLE_STEP 异常时,由自定义的veh_handler处理
pub fn AddVectoredExceptionHandler(
    first: u32,// 处理顺序
    handler: PVECTORED_EXCEPTION_HANDLER,// 自定义的处理异常的函数指针
) -> *mut c_void {
    let kernel32 = get_module_address(s!("KERNEL32.DLL"), None);

    // 这里调用dinvk!后,是得到了AddVectoredExceptionHandlerFn这个函数的地址并立即执行.最终会返回一个指向已注册异常处理程序的句柄
    // 
    dinvoke!(
        kernel32,// 在kernel32的基址中查找
        s!("AddVectoredExceptionHandler"),// 查找这个函数的地址
        AddVectoredExceptionHandlerFn,// 查找的函数的指针类型(定义了参数和返回值的类型)
        first,// 1最先调用,0最后调用
        handler// 你的异常处理函数的指针
    )
    .unwrap_or(null_mut())
}

/// Wrapper for the `RemoveVectoredExceptionHandler` function from `KERNEL32.DLL`.
/// 
/// 只要注册了（Add），无论任务成功与否，必须保证移除（Remove），否则就是留下了致命的隐患
/// 
/// AddVectoredExceptionHandler 的“撤销键”.虽然代码中用的是kernel32中的RemoveVectoredExceptionHandler,这只是一个wraper,真正的实现在ntdll中的RtlRemoveVectoredExceptionHandler 函数中
pub fn RemoveVectoredExceptionHandler(
    handle: *mut c_void,// 不是项目中自定义的veh_handler函数,而是AddVectoredExceptionHandler 执行成功后返回的那个不透明指针Opaque Pointer
) -> u32 {
    let kernel32 = get_module_address(s!("KERNEL32.DLL"), None);
    dinvoke!(
        kernel32,
        s!("RemoveVectoredExceptionHandler"),
        RemoveVectoredExceptionHandlerFn,
        handle
    )
    .unwrap_or(0)
}

/// Wrapper for the `NtGetContextThread` function from `NTDLL.DLL`.
/// 
/// 用于在修改调试寄存器(hardware breakpoints)之前,获取线程当前的全部状态,避免覆盖已有的关键配置
pub fn NtGetContextThread(
    hthread: HANDLE,
    lpcontext: *mut CONTEXT,
) -> i32 {
    dinvoke!(
        get_ntdll_address(),
        s!("NtGetContextThread"),
        NtGetThreadContextFn,
        hthread,
        lpcontext
    )
    .unwrap_or(0)
}

/// Wrapper for the `NtSetContextThread` function from `NTDLL.DLL`.
/// 
/// 该函数用于将指定的 CONTEXT 结构体内容强制写入目标线程的物理 CPU 寄存器状态（如 Rip、Rsp 或 Dr0-7），从而实现对线程执行流或硬件调试特性的直接操控
/// 
/// 在breakpoint.rs中的set_breakpoint中调用
pub fn NtSetContextThread(
    hthread: HANDLE,// 目标线程句柄
    lpcontext: *const CONTEXT,// 新的上下文数据(只读指针)
) -> i32 {
    dinvoke!(
        get_ntdll_address(),
        s!("NtSetContextThread"),
        NtSetThreadContextFn,
        hthread,
        lpcontext
    )
    .unwrap_or(0)
}

/// Wrapper for the `GetStdHandle` function from `KERNEL32.DLL`.
/// 
///  在 dinvk 中，它是你与控制台对话的唯一官方通道.用来获取 Windows “标准流句柄”（输入、输出、错误）的函数。在 dinvk项目中，它通常被底层宏（如println!）用来向控制台打印调试信息，或者在隐蔽操作中用于重定向输出
/// 
///  作为 `no_std` 环境下自定义输出系统（`println!`宏）的基石，通过动态解析调用的方式，隐蔽地获取控制台输出权限，从而为复杂的系统调用欺骗逻辑提供必要的运行时调试反馈。。
/// 
/// 是 dinvk 调试输出系统（User-Land Debugging System） 的源头,使用本项目中的println!时
pub fn GetStdHandle(handle: u32) -> HANDLE {
    let kernel32 = get_module_address(s!("KERNEL32.DLL"), None);
    dinvoke!(
        kernel32,
        s!("GetStdHandle"),
        GetStdHandleFn,
        handle
    )
    .unwrap_or(null_mut())
}

/// Returns a pseudo-handle to the current process ((HANDLE)-1).
/// 
/// 在 Windows内核对象管理器中，正常的句柄（Handle）是一个索引值，指向内核句柄表中的具体对象
/// 
/// `-1`: 永远代表 “调用者当前的进程”
#[inline(always)]
pub fn NtCurrentProcess() -> HANDLE {
    -1isize as HANDLE
}

/// Returns a pseudo-handle to the current thread ((HANDLE)-2).
/// 
/// `-2`: 永远代表 “调用者当前的线程”
#[inline(always)]
pub fn NtCurrentThread() -> HANDLE {
    -2isize as HANDLE
}

/// Returns the default heap handle for the current process from the PEB.
#[inline(always)]
pub fn GetProcessHeap() -> HANDLE {
    let peb = NtCurrentPeb();
    (unsafe { *peb }).ProcessHeap
}

/// Returns the process ID of the calling process from the TEB.
#[inline(always)]
pub fn GetCurrentProcessId() -> u32 {
    let teb = NtCurrentTeb();
    (unsafe { *teb }).Reserved1[8] as u32// 偏移0x40(根据TEB在rust中的映射内存布局)
}

/// Returns the thread ID of the calling thread from the TEB.
#[inline(always)]
pub fn GetCurrentThreadId() -> u32 {
    let teb = NtCurrentTeb();
    (unsafe { *teb }).Reserved1[9] as u32// 偏移0x48
}

/// Retrieves a pointer to the PEB of the current process.
/// 
///  Windows x64 架构下，CPU 的 GS 段寄存器 指向当前线程的 TEB (Thread Environment Block) 结构体基址。
#[inline(always)]
pub fn NtCurrentPeb() -> *const PEB {
    #[cfg(target_arch = "x86_64")]
    return __readgsqword(0x60) as *const PEB;

    #[cfg(target_arch = "x86")]
    return __readfsdword(0x30) as *const PEB;

    #[cfg(target_arch = "aarch64")]
    return unsafe { *(__readx18(0x60) as *const *const PEB) };
}

/// Retrieves a pointer to the TEB of the current thread.
#[inline(always)]
pub fn NtCurrentTeb() -> *const TEB {
    #[cfg(target_arch = "x86_64")]
    return __readgsqword(0x30) as *const TEB;

    #[cfg(target_arch = "x86")]
    return __readfsdword(0x18) as *const TEB;

    #[cfg(target_arch = "aarch64")]
    return unsafe { *(__readx18(0x30) as *const *const TEB) };
}

/// Reads a `u64` value from the GS segment at the specified offset.
#[inline(always)]
#[cfg(target_arch = "x86_64")]
pub fn __readgsqword(offset: u64) -> u64 {
    let out: u64;
    unsafe {
        core::arch::asm!(
            "mov {}, gs:[{:e}]",// 要生成的汇编指令原型
            lateout(reg) out,   // 对应第一个占位符{},out是u64,编译器会分配一个64为通用寄存器,如果编译器分配rax,这里就变成rax
            in(reg) offset,
            options(nostack, pure, readonly),
        );
    }

    out
}

/// Reads a `u32` value from the FS segment at the specified offset.
#[inline(always)]
#[cfg(target_arch = "x86")]
pub fn __readfsdword(offset: u32) -> u32 {
    let out: u32;
    unsafe {
        core::arch::asm!(
            "mov {:e}, fs:[{:e}]",
            lateout(reg) out,
            in(reg) offset,
            options(nostack, pure, readonly),
        );
    }

    out
}

/// Reads a `u64` value from the x18 register at the specified offset.
#[inline(always)]
#[cfg(target_arch = "aarch64")]
pub fn __readx18(offset: u64) -> u64 {
    let out: u64;
    unsafe {
        core::arch::asm!(
            "mov {}, x18",
            lateout(reg) out,
            options(nostack, pure, readonly),
        );
    }

    out + offset
}

/// Evaluates to TRUE if the return value specified by `nt_status` is a success
/// 
///  Windows 内核函数的返回值（NTSTATUS）是一套特殊的代码：
/// `0` 到 `0x3FFFFFFF`: 成功 (Success)。例如 STATUS_SUCCESS (0)。 
/// 
/// `0x40000000` 到 `0x7FFFFFFF`: 信息 (Informational)。
/// 
/// `0x80000000` 到 `0xBFFFFFFF`: 警告 (Warning)。
/// 
/// `0xC0000000` 到 `0xFFFFFFFF`: 错误 (Error)。例如 STATUS_ACCESS_DENIED (0xC0000022)。
/// 
///  由于 NTSTATUS 是 i32，所有错误代码（最高位为 1）都被视为负数.因此，nt_status >= 0 涵盖了所有成功和信息性的状态。
pub const fn NT_SUCCESS(nt_status: NTSTATUS) -> bool {
    nt_status >= 0
}