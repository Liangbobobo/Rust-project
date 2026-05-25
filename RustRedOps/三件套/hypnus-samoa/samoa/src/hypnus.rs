
#![allow(unused)]

//use alloc::string::String;//hypnus中用于obfstr的宏展开,samoa中未使用obfstr



// uwd库中lib.rs使用了pub use uwd::*;=uwd::uwd::AsPointer
use uwd::AsPointer;

use crate::{debug_log,stealth_bail};
use core::{ffi::c_void, mem::zeroed, ptr::null_mut};


/// Enumeration of supported memory obfuscation strategies
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
// 
#[repr(transparent)]
/// 元组结构体(包含一个匿名字段/成员)
/// 是Rust中的NewType模式
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

#[derive(Clone,Copy,Debug)]
struct Hypnus{

    /// base memory pointer to be manipulated or operated on
    base:u64,

    size:u64,

    /// delay time in seconds
    time:u64,

    /// resolved winapi required for execution
    // cfg:&'static Config,

    mode:ObfMode,
}