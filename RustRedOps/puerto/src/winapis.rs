use crate::types::{PEB};

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
   

#[inline(always)]
#[cfg(target_arch = "x86_64")]
pub fn __readgsqword(offset:u64)->u64 {
    let out: u64;
    unsafe {
        core::arch::asm!(

            // gs:[] 在gs寄存器中寻址
            // {:e} e代表只使用寄存器中的低32位
            // 将gs寄存器基址加offset内容移到输出寄存器中(out)
            "mov {}, gs:[{:e}]",

            // 将一个空闲的通用寄存器分配给out,这个寄存器的值在汇编执行完毕后才写入
            lateout(reg) out,

            // 把offset传入一个通用寄存器
            in(reg) offset,

            // nostack,此汇编不压栈出栈,编译器不需要调整栈指针
            // readonly.只读内存,不写入
            // pure 如果输入一样，输出就一样。这允许编译器进行优化（比如消除重复调用）
            options(nostack, pure, readonly),
        );
    }

    out
}