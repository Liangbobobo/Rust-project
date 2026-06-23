
#![allow(unused)]

//use alloc::string::String;//原项目hypnus中用于obfstr的宏展开,samoa中未使用obfstr



use puerto::winapis::NT_SUCCESS;
// uwd库中lib.rs使用了pub use uwd::*;=uwd::uwd::AsPointer
use uwd::AsPointer;

use crate::error::HypnusError::{InvalidArguments, NtCreateEventFailed, TpAllocPoolFailed};
use crate::types::{PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE,TP_POOL_STACK_INFORMATION};
use crate::winapis::{NtCreateEvent, TpAllocPool, TpSetPoolStackInformation};
use crate::{debug_log,stealth_bail};
use core::{ffi::c_void, mem::zeroed, ptr::null_mut, time};

use crate::config::{Config,init_config};
use crate::error::{HypnusError,Result};// 代替源码hyonus中anyhow的Result

use puerto::types::{EVENT_ALL_ACCESS,EVENT_TYPE};
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
    pub const None:Self=ObfMode(0b0000);


    // ObfMode结构体内部只有一个u32,后面的Heap/Rwx都是ObfMode这个结构体的不同值(封装了不同的u32)
    pub const Heap:Self=ObfMode(0b0001);

    pub const Rwx:Self=ObfMode(0b0010);

    /// Checks whether the flag contains another `ObfMode`.
    /// 
    /// 该函数参数传入self,但上面对ObfMode derive了copy.self从移动所有权变成了按位复制.不改变原所有权,把复制的数据给了函数
    fn contains(self,other:ObfMode)->bool {
        (self.0 & other.0)==other.0
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
#[derive(Clone,Copy,Debug)]
struct Hypnus{

    /// base memory pointer to be manipulated or operated on
    base:u64,

    size:u64,

    /// delay time in seconds
    time:u64,

    /// resolved winapi required for execution
    // cfg:&'static Config,
    cfg: &'static Config,

    /// Obfuscantion modes
    mode:ObfMode,
}

impl Hypnus {
    /// create a new Hypnus structure

    #[inline]
    fn new(base:u64,size:u64,time:u64,mode:ObfMode)->Result<Self> {
        if base==0 || size == 0 || time==0 {
        stealth_bail!(InvalidArguments,"invalid arguments")    
         }

        Ok(Self{
            base,
            size,
            time,
            // 在宏调用时赋予了实参.这是省略写法=mode:mode, rust中,实例化一个结构体时,如果当前作用域内存在一个与结构体字段同名的变量.可以省略:value的赋值部分
            mode,
            cfg:init_config()?
        })
    }

/// performs memory obfuscation using a thread-pool timer sequence
fn timer(&mut self)->Result<()> {
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
let mut events = [null_mut();3];

for event in events {
    // ffi的extern "system"方式调用win native api
    let status = NtCreateEvent(
        // 输出:成功的事件对象handle
        event,
         EVENT_ALL_ACCESS, 
         null_mut(), // 传空代表该事件是anonymous的.EDR对有名事件在扫描全局对象目录时很容易发现.anonymous对象只存于当前进程句柄表,隐匿性最高 
         EVENT_TYPE::NotificationEvent,//设置为有信号的通知型事件:会一直保持有信号状态,直到被重置(在hypnus的异步链中,一个事件可能被多个context同时等待,通知型事件能确保所有监听者都能收到信号)
        0 // 初始为无信号状态,意味着所有等待该事件的线程都会立即挂起,直到后续有指令发其他信号);
    );

if !NT_SUCCESS(status){
    stealth_bail!(NtCreateEventFailed,"NtCreateEvent Failed");// 宏后面到底需要加 ; 吗
}
}

// Allocate dedicated threadpool with one worker

// 用来表示指向TP_POOL的句柄:代表整个线程池的根,后续所有线程数量/大小都同各国这个poll指针进行挂载
let mut pool:*mut c_void=null_mut();

// 用TpAllocPool在用户态堆区分配并初始化一个TP_POOL结构体,并在内核中创建一个Worker Factory对象.但此时并没有产生真正的线程
let mut status = TpAllocPool(
    &mut pool, // 对应的参数类型是指针的指针,所以尽管pool本身是copy的,这里也需要用&
    null_mut());
if !NT_SUCCESS(status) {
    stealth_bail!(TpAllocPoolFailed,"TpAllocPool Failed")
}

// Configure threadpool stack size
// 0x80000=512kb,这个4kb是怎么计算得到的,见注释1
let mut stack = TP_POOL_STACK_INFORMATION{
    StackCommit: 0x80000,
    StackReserve: 0x80000 
};


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