
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


// derive详见rust grammer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ObfMode(pub u32);