use crate::module::{get_ntdll_address};
use crate::types::{CONTEXT, HANDLE, NTSTATUS, NtGetThreadContextFn, NtSetThreadContextFn, PEB};
use crate::{dinvok};
use crate::types::*;
/// Returns the default heap handle for the current process from the PEB.
#[inline(always)]
pub fn GetProcessHeap() -> HANDLE {
    let peb = NtCurrentPeb();
    (unsafe { *peb }).ProcessHeap
}


pub fn NtSetContextThread(
    hthread: HANDLE,// 目标线程句柄
    lpcontext: *const CONTEXT,// 新的上下文数据(只读指针)
) -> i32 {
    dinvok!(
        get_ntdll_address(),
        0xAD4FA23E,
        NtSetThreadContextFn,
        hthread,
        lpcontext
    )
    .unwrap_or(0)
}


#[inline(always)]
pub fn NtCurrentThread()->HANDLE {
    -2isize as HANDLE
}

pub fn NtGetContextThread(
    hthread: HANDLE,
    lpcontext: *mut CONTEXT
) ->i32{
dinvok!(
    // pue版本中
    get_ntdll_address(),
    0x0FFA8E6A,
    NtGetThreadContextFn,
    hthread,
    lpcontext
)
.unwrap_or(0)
}



#[inline(always)]
    /// 不调用windows api通过cpu的gs寄存器读取当前进程的peb地址
    /// 
    /// gs指向当前线程的TEB起始地址,其offset 0x60(win64)处指向peb地址
pub fn NtCurrentPeb()->*const PEB {

    #[cfg(target_arch = "x86_64")]
    // __readgsqword在msvc编译器中预定义为内联函数,用于读取GS的偏移
    // __代表该函数时一个极低层实现,是系统内核/编译器级别的逻辑
    return __readgsqword(0x60) as *const PEB;

    #[cfg(target_arch = "x86")]
    return __readfsdword(0x30) as *const PEB;

    #[cfg(target_arch = "aarch64")]
    return unsafe { *(__readx18(0x60) as *const *const PEB) };
    }
   
/// retrieve a pointer to the TEB of the current thread
#[inline(always)]
pub fn NtCurrentTeb()->*const TEB {
    #[cfg(target_arch = "x86_64")]
    return __readgsqword(0x30) as *const TEB;

     #[cfg(target_arch = "x86")]
    return __readfsdword(0x18) as *const TEB;

    #[cfg(target_arch = "aarch64")]
    return unsafe { *(__readx18(0x30) as *const *const TEB) };
}


#[inline(always)]
#[cfg(target_arch = "x86_64")]
pub fn __readgsqword(offset:u64)->u64 {
    let out: u64;
    unsafe {
        core::arch::asm!(

            // 详见win64中的汇编/Rust内联汇编
            // 将gs寄存器基址加offset内容作为内存地址,解引用后将对应的数据移到输出寄存器中(out)
            "mov {}, gs:[{:e}]",

            // 将一个空闲的通用寄存器分配给out,这个寄存器的值在汇编执行完毕后才写入.关于out和lateout区别,详见win64中的汇编/Rust内联汇编
            // reg是llvm/rust的寄存器类规范,表示从x86_64的通用寄存器挑一个.如果要操作浮点数或128位xmm向量/8位单字节,需要使用不同的标志.xmm_reg或reg_byte
            // out是前文定义的rust的局部变量
            lateout(reg) out,

            // 把offset传入一个通用寄存器.详见同上
            in(reg) offset,

            // options是编译器承诺集合,其内部是llvm的各项优化指令
            // nostack,此汇编代码不压栈出栈,编译器不需要调整栈指针(栈空间零侵入保证)
            // readonly.只读内存,不写入
            // pure 如果输入一样，输出就一样。这允许编译器进行优化（比如消除重复调用）
            options(nostack, pure, readonly),
        );
    }

    out
}

#[inline(always)]
pub fn NtCurrentProcess()->HANDLE {
    -1isize as HANDLE
}

/// Evaluates to TRUE if the return value specified by `nt_status` is a success
pub const fn NT_SUCCESS(nt_status:NTSTATUS)->bool {
    nt_status>=0
}