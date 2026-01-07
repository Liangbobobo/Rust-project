//! Hardware breakpoint management utilities.

use core::ffi::c_void;
use core::ptr::addr_of_mut;
use core::sync::atomic::{Ordering, AtomicBool};

use crate::winapis::{NtSetContextThread, NtGetContextThread, NtCurrentThread};
use crate::types::{
    CONTEXT, CONTEXT_DEBUG_REGISTERS_AMD64, EXCEPTION_SINGLE_STEP,
    EXCEPTION_CONTINUE_EXECUTION, EXCEPTION_CONTINUE_SEARCH, 
    EXCEPTION_POINTERS, HANDLE, OBJECT_ATTRIBUTES,
    CONTEXT_DEBUG_REGISTERS_X86
};

/// Global mutable static holding the current Windows API call.
/// 存储当前正在 Hook 的 API 信息
///  这是一个 unsafe 的全局变量，多线程下可能会有竞争，但在此类shellcode 加载器场景通常是单线程或受控的
pub static mut CURRENT_API: Option<WINAPI> = None;

/// Atomic variable to control the use of VEH.
/// 控制 VEH (异常处理函数) 是否应该响应异常.如果为 false，即使触发了异常，VEH也会直接忽略，交给系统或其他处理器处理
static USE_BREAKPOINT: AtomicBool = AtomicBool::new(false);

/// Enables or disables the use of hardware breakpoints globally.
/// 
/// # Examples
/// 
/// ```
/// // Enabling breakpoint hardware
/// set_use_breakpoint(true);
/// let handle = AddVectoredExceptionHandler(1, Some(veh_handler));
///
/// // Allocating memory and using breakpoint hardware
/// let mut addr = std::ptr::null_mut();
/// let mut size = 1 << 12;
/// let status = NtAllocateVirtualMemory(NtCurrentProcess(), &mut addr, 0, &mut size, 0x3000, 0x04);
/// if !NT_SUCCESS(status) {
///     eprintln!("[-] NtAllocateVirtualMemory Failed With Status: {}", status);
///     return;
/// }
///
/// // Disabling breakpoint hardware
/// set_use_breakpoint(false);
/// RemoveVectoredExceptionHandler(handle); 
/// ``

///  Ordering::SeqCst,在cpu指令层加一道锁(内存屏障),
///  它强制要求：这行代码之前的所有指令必须完成，这行代码之后的所有指令不准提前。 保证了逻辑执行顺序在所有 CPU核心看来都是一致的。
/// .store 原子存储,对应的是 CPU 的原子指令,且在改内存之后,负责通知cpu的内存一致性,让其他cpu核心知道,缓存的旧值失效了
/// 是原子世界的“赋值号（=）”，但它附带了硬件级的线程安全保障
#[inline(always)]
pub fn set_use_breakpoint(enabled: bool) {
    USE_BREAKPOINT.store(enabled, Ordering::SeqCst);
}

/// Checks if hardware breakpoints are currently enabled.
#[inline(always)]
pub fn is_breakpoint_enabled() -> bool {
    USE_BREAKPOINT.load(Ordering::SeqCst)
}

/// Configures a hardware breakpoint on the specified address.
pub(crate) fn set_breakpoint<T: Into<u64>>(address: T) {
    let mut ctx = CONTEXT {
        ContextFlags: if cfg!(target_arch = "x86_64") { CONTEXT_DEBUG_REGISTERS_AMD64 } else { CONTEXT_DEBUG_REGISTERS_X86 },
        ..Default::default()
    };

    NtGetContextThread(NtCurrentThread(), &mut ctx);

    cfg_if::cfg_if! {
        if #[cfg(target_arch = "x86_64")] {
            ctx.Dr0 = address.into();
            ctx.Dr6 = 0x00;
            ctx.Dr7 = set_dr7_bits(ctx.Dr7, 0, 1, 1);
        } else {
            ctx.Dr0 = address.into() as u32;
            ctx.Dr6 = 0x00;
            ctx.Dr7 = set_dr7_bits(ctx.Dr7 as u64, 0, 1, 1) as u32;
        }
    }

    NtSetContextThread(NtCurrentThread(), &ctx);
}

/// Modifies specific bits in the `DR7` register.
fn set_dr7_bits<T: Into<u64>>(current: T, start_bit: i32, nmbr_bits: i32, new_bit: u64) -> u64 {
    let current = current.into();
    let mask = (1u64 << nmbr_bits) - 1;
    (current & !(mask << start_bit)) | (new_bit << start_bit)
}

/// Enum representing different Windows API calls that can be used.
#[derive(Debug)]
pub enum WINAPI {
    /// Represents the `NtAllocateVirtualMemory` call.
    NtAllocateVirtualMemory {
        ProcessHandle: HANDLE,
        Protect: u32,
    },

    /// Represents the `NtProtectVirtualMemory` call.
    NtProtectVirtualMemory {
        ProcessHandle: HANDLE,
        NewProtect: u32,
    },

    /// Represents the `NtCreateThreadEx` call.
    NtCreateThreadEx {
        ProcessHandle: HANDLE,
        ThreadHandle: *mut HANDLE,
        DesiredAccess: u32,
        ObjectAttributes: *mut OBJECT_ATTRIBUTES
    },

    /// Represents the `NtWriteVirtualMemory` call.
    NtWriteVirtualMemory {
        ProcessHandle: HANDLE,
        Buffer: *mut c_void,
        NumberOfBytesToWrite: *mut usize,
    },
}

/// Handles exceptions triggered by hardware breakpoints (x64).
#[cfg(target_arch = "x86_64")]
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "system" fn veh_handler(exceptioninfo: *mut EXCEPTION_POINTERS) -> i32 {
    if !is_breakpoint_enabled() || (*(*exceptioninfo).ExceptionRecord).ExceptionCode != EXCEPTION_SINGLE_STEP {
        return EXCEPTION_CONTINUE_SEARCH;
    }

    let context = (*exceptioninfo).ContextRecord;
    if (*context).Rip == (*context).Dr0 && (*context).Dr7 & 1 == 1 {
        if let Some(current) = (*addr_of_mut!(CURRENT_API)).take() {
            match current {
                WINAPI::NtAllocateVirtualMemory {
                    ProcessHandle, 
                    Protect 
                } => {
                    (*context).R10 = ProcessHandle as u64;
                    *(((*context).Rsp + 0x30) as *mut u32) = Protect;
                },

                WINAPI::NtProtectVirtualMemory { 
                    ProcessHandle, 
                    NewProtect, 
                } => {
                    (*context).R10 = ProcessHandle as u64;
                    (*context).R9  = NewProtect as u64;
                },

                WINAPI::NtCreateThreadEx { 
                    ProcessHandle,
                    ThreadHandle,
                    DesiredAccess,
                    ObjectAttributes
                } => {
                    (*context).R10 = ThreadHandle as u64;
                    (*context).Rdx = DesiredAccess as u64;
                    (*context).R8  = ObjectAttributes as u64;
                    (*context).R9  = ProcessHandle as u64;
                },

                WINAPI::NtWriteVirtualMemory { 
                    ProcessHandle,
                    Buffer,
                    NumberOfBytesToWrite,
                } => {
                    (*context).R10 = ProcessHandle as u64;
                    (*context).R8  = Buffer as u64;
                    (*context).R9  = NumberOfBytesToWrite as u64;
                }
            }

            (*context).Dr0 = 0x00;
            (*context).Dr6 = 0x00;
            (*context).Dr7 = set_dr7_bits((*context).Dr7, 0, 1, 0);
        }

        return EXCEPTION_CONTINUE_EXECUTION;
    }

    EXCEPTION_CONTINUE_SEARCH
}

/// Handles exceptions triggered by hardware breakpoints (x86).
#[cfg(target_arch = "x86")]
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "system" fn veh_handler(exceptioninfo: *mut EXCEPTION_POINTERS) -> i32 {
    if !is_breakpoint_enabled() || (*(*exceptioninfo).ExceptionRecord).ExceptionCode != EXCEPTION_SINGLE_STEP {
        return EXCEPTION_CONTINUE_SEARCH;
    }

    let context = (*exceptioninfo).ContextRecord;
    if (*context).Eip == (*context).Dr0 && (*context).Dr7 & 1 == 1 {
        if let Some(current) = (*addr_of_mut!(CURRENT_API)).take() {
            match current {
                WINAPI::NtAllocateVirtualMemory { 
                    ProcessHandle, 
                    Protect 
                } => {
                    *(((*context).Esp + 0x4) as *mut u32) = ProcessHandle as u32;
                    *(((*context).Esp + 0x18) as *mut u32) = Protect;
                },

                WINAPI::NtProtectVirtualMemory { 
                    ProcessHandle, 
                    NewProtect, 
                } => {
                    *(((*context).Esp + 0x4) as *mut u32) = ProcessHandle as u32;
                    *(((*context).Esp + 0x10) as *mut u32) = NewProtect as u32;
                },

                WINAPI::NtCreateThreadEx { 
                    ProcessHandle,
                    ThreadHandle,
                    DesiredAccess,
                    ObjectAttributes
                } => {
                    *(((*context).Esp + 0x4) as *mut u32) = ThreadHandle as u32;
                    *(((*context).Esp + 0x8) as *mut u32) = DesiredAccess as u32;
                    *(((*context).Esp + 0xC) as *mut u32) = ObjectAttributes as u32;
                    *(((*context).Esp + 0x10) as *mut u32) = ProcessHandle as u32;
                },

                WINAPI::NtWriteVirtualMemory { 
                    ProcessHandle,
                    Buffer,
                    NumberOfBytesToWrite,
                } => {
                    *(((*context).Esp + 0x4) as *mut u32) = ProcessHandle as u32;
                    *(((*context).Esp + 0xC) as *mut u32) = Buffer as u32;
                    *(((*context).Esp + 0x10) as *mut u32) = NumberOfBytesToWrite as u32;
                }
            }

            (*context).Dr0 = 0x00;
            (*context).Dr6 = 0x00;
            (*context).Dr7 = set_dr7_bits((*context).Dr7, 0, 1, 0) as u32;
        }

        return EXCEPTION_CONTINUE_EXECUTION;
    }

    EXCEPTION_CONTINUE_SEARCH
}