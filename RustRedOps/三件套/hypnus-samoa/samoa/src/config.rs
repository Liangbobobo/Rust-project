#![allow(unused)]

use core::ptr::null_mut;
use puerto::hash::{fnv1a_utf16,fnv1a_utf16_from_u8};
use crate::{debug_log,stealth_bail};// replace anyhow
use crate::error::Result; // replace anyhow::Result
use puerto::winapis::{NtCurrentProcess,NT_SUCCESS};
use puerto::module::{get_module_address,get_proc_address,get_ntdll_address};
use crate::types::*;// crate代表本库(crate)的根目录
use crate::spoof::{StackSpoof};
use crate::winapis::{WinApi,Modules};



/// Stores resolved DLL base addresses and function pointers
/// 
/// 执行混淆时用到的各种配件
#[derive(Default,Debug,Clone,Copy)]
pub struct Config{
 pub stack:StackSpoof,
 /// 休眠结束后,让thread pool触发ntcontinue,继续执行hyponus.rs中timer()/wait()中定义好的执行流
 pub callback:u64,

 /// 执行RtlCaptureContext的rx内存地址;在混淆链启动时获取快照
pub trampoline:u64,

 // 以下字段在config::new中调用crate::winapis::fn winapis初始化
 // 这些地址后续会被以(config.nt_continue)(...)  形式直接执行,如果在后续fn winapis()中使用了transmute将地址转为函数指针,在执行时cpu会使用call指令.所以config.rs  中的  WinApi  必须作为 “纯数据” 存在 详见注释1
    pub modules: Modules,
    pub wait_for_single: WinApi,
    pub base_thread: WinApi,
    pub enum_date: WinApi,
    pub system_function040: WinApi,
    pub system_function041: WinApi,
    pub nt_continue: WinApi,
    pub nt_set_event: WinApi,
    pub rtl_user_thread: WinApi,
    pub nt_protect_virtual_memory: WinApi,
    pub rtl_exit_user_thread: WinApi,
    pub nt_get_context_thread: WinApi,
    pub nt_set_context_thread: WinApi,
    pub nt_test_alert: WinApi,
    pub nt_wait_for_single: WinApi,
    pub rtl_acquire_lock: WinApi,
    pub tp_release_cleanup: WinApi,
    pub rtl_capture_context: WinApi,
    pub zw_wait_for_worker: WinApi,

}

impl Config {

    // 


    /// Create a new `Config`.
    pub fn new()->Result<Self> {
        
    }




}












// 注释1
// config.rs  中的  winapis() ：完全且不能使用  transmute:定义winapis()的结构体Config的字段类型是WinApi(u64).本质是被  transparent  包装的  u64  整数（纯数值），代表一个内存地址.
// 为什么这么设计:所有敏感高危api的调用(如内存保护属性修改、创建线程、休眠加密)绝不能在rust中直接执行call.
// 后续使用情况(待总结)
// 把这些敏感 API 的地址作为  u64  数据保存在  Config  里
// 