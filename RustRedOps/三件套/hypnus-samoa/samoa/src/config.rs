#![allow(unused)]

use core::ptr::null_mut;
use puerto::hash::{fnv1a_utf16,fnv1a_utf16_from_u8};
use crate::{debug_log,stealth_bail};// replace anyhow
use crate::error::Result; // replace anyhow::Result
use puerto::winapis::{NtCurrentProcess,NT_SUCCESS};
use puerto::module::{get_module_address,get_proc_address,get_ntdll_address};
use crate::types::*;// crate代表本库(crate)的根目录
use crate::spoof::{StackSpoof};
use crate::winapis::{WinApi,Modules,Dll};



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

 // 
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

/// Global configuration object
static CONFIG:spin::Once<Config>=spin::Once::new();

/// lazily initializes and returns a singletond单例(`Config` instance)
pub fn init_config()->Result<&'static Config> {
    CONFIG.try_call_once(Config::new)
}





impl Config {

    /// Create a new `Config`.
    pub fn new() -> Result<Self> {
        let modules = Self::modules();
        let config = Self::winapis(modules);

        // TODO: Resolve stack spoofing, callback, and trampoline if needed.
        
        Ok(config)
    }

    /// Resolves the base addresses of key Windows modules (`ntdll.dll`, `kernel32.dll`, etc).
    fn modules() -> Modules {
        /// ntdll/kernel32/kernelbase的基址
        let ntdll = get_ntdll_address();
        // 失败后通过unwrap_or(null_mut())降级返回空指针(不painc退出),契合源码逻辑
        let kernel32 = get_module_address(Some(1303842461u32), Some(fnv1a_utf16));
        let kernelbase = get_module_address(Some(3594687209u32), Some(fnv1a_utf16));

        // 处理cryptbase可能未加载的情况
        let load_library = get_proc_address(kernel32, Some(1290174399u32), Some(fnv1a_utf16)).unwrap_or(null_mut());
        let cryptbase = {
            let mut addr = get_module_address(Some(1145924862u32), Some(fnv1a_utf16)).unwrap_or(null_mut());

            if addr.is_null() {
                addr = uwd::spoof!(load_library, obfstr::obfcstr!(c"CryptBase").as_ptr())
                    .expect(obfstr::obfstr!("Error"));
            }

            addr
        };

        Modules {
            ntdll: Dll::from(ntdll),
            kernel32: Dll::from(kernel32.unwrap_or(null_mut())),
            cryptbase: Dll::from(cryptbase),
            kernelbase: Dll::from(kernelbase.unwrap_or(null_mut())),
        }
    }

    fn winapis(modules: Modules) -> Self {
        let ntdll = modules.ntdll.as_ptr();
        let kernel32 = modules.kernel32.as_ptr();
        let cryptbase = modules.cryptbase.as_ptr();

        Self {
            modules,
            wait_for_single: get_proc_address(Some(kernel32), Some(474226840u32), Some(fnv1a_utf16)).unwrap_or(null_mut()).into(),
            base_thread: get_proc_address(Some(kernel32), Some(4144453610u32), Some(fnv1a_utf16)).unwrap_or(null_mut()).into(),
            enum_date: get_proc_address(Some(kernel32), Some(2305293355u32), Some(fnv1a_utf16)).unwrap_or(null_mut()).into(),
            system_function040: get_proc_address(Some(cryptbase), Some(4252924884u32), Some(fnv1a_utf16)).unwrap_or(null_mut()).into(),
            system_function041: get_proc_address(Some(cryptbase), Some(2396840837u32), Some(fnv1a_utf16)).unwrap_or(null_mut()).into(),
            nt_continue: get_proc_address(Some(ntdll), Some(2043420876u32), Some(fnv1a_utf16)).unwrap_or(null_mut()).into(),
            rtl_capture_context: get_proc_address(Some(ntdll), Some(1541026118u32), Some(fnv1a_utf16)).unwrap_or(null_mut()).into(),
            nt_set_event: get_proc_address(Some(ntdll), Some(2314183347u32), Some(fnv1a_utf16)).unwrap_or(null_mut()).into(),
            rtl_user_thread: get_proc_address(Some(ntdll), Some(1924285810u32), Some(fnv1a_utf16)).unwrap_or(null_mut()).into(),
            nt_protect_virtual_memory: get_proc_address(Some(ntdll), Some(399609846u32), Some(fnv1a_utf16)).unwrap_or(null_mut()).into(),
            rtl_exit_user_thread: get_proc_address(Some(ntdll), Some(1491200690u32), Some(fnv1a_utf16)).unwrap_or(null_mut()).into(),
            nt_set_context_thread: get_proc_address(Some(ntdll), Some(2907677246u32), Some(fnv1a_utf16)).unwrap_or(null_mut()).into(),
            nt_get_context_thread: get_proc_address(Some(ntdll), Some(268078698u32), Some(fnv1a_utf16)).unwrap_or(null_mut()).into(),
            nt_test_alert: get_proc_address(Some(ntdll), Some(1663868085u32), Some(fnv1a_utf16)).unwrap_or(null_mut()).into(),
            nt_wait_for_single: get_proc_address(Some(ntdll), Some(1015357890u32), Some(fnv1a_utf16)).unwrap_or(null_mut()).into(),
            rtl_acquire_lock: get_proc_address(Some(ntdll), Some(105262326u32), Some(fnv1a_utf16)).unwrap_or(null_mut()).into(),
            tp_release_cleanup: get_proc_address(Some(ntdll), Some(1421224806u32), Some(fnv1a_utf16)).unwrap_or(null_mut()).into(),
            zw_wait_for_worker: get_proc_address(Some(ntdll), Some(2438784615u32), Some(fnv1a_utf16)).unwrap_or(null_mut()).into(),
            // 结构体更新语法（Struct Update Syntax）
            // 为结构体中所有未被显式赋值的字段生成一个“默认值”(空/零地址)
            // 风险:不会检查结构体字段是否被遗漏;如果运行时使用这个default的零/空地址调用函数,程序会crash(非法内存访问).这显然是运行时crash的温床,不要试图通过注释/记忆消除此风险,而是
            // 1.使用option/result wrap每个字段 2. 在config::new后面加一个对config的verify()方法用以检测是否完整解析环境
            ..Default::default()
        }
    }
}












// 注释1
// config.rs  中的  winapis() ：完全且不能使用  transmute:定义winapis()的结构体Config的字段类型是WinApi(u64).本质是被  transparent  包装的  u64  整数（纯数值），代表一个内存地址.
// 为什么这么设计:所有敏感高危api的调用(如内存保护属性修改、创建线程、休眠加密)绝不能在rust中直接执行call.
// 后续使用情况(待总结)
// 把这些敏感 API 的地址作为  u64  数据保存在  Config  里
// 