#![allow(unused)]

// cfg=control flow guard 安全机制



use core::{ffi::c_void,ptr::null_mut};
use crate::{debug_log,stealth_bail};
use crate::error::Result;
use obfstr::{obfcstr as s};
use puerto::winapis::{NtCurrentProcess,NT_SUCCESS};
use puerto::helper::PE;

use crate::config::Config;
use crate::winapis::{
    NtQueryInformationProcess,
    SetProcessValidCallTargets
};

use crate::types::{
    CFG_CALL_TARGET_INFO,
    EXTENDED_PROCESS_INFORMATION
};

/// flag indicating a valid indirect call target
/// 当调用SetProcessValidCallTargets去修改cfg位图bitmap时,需要传给内核一个CFG_CALL_TARGET_INFO结构体,其中有一个flags字段,该字段代表经bitmap中的bit置为1
const CFG_CALL_TARGET_VALID: usize = 1;

/// used internally by windows to identify per-process CFG state
/// 代码调用NtQueryInformationProcess查询进程的安全策略.该api的第二个参数是一个ProcessInformationClass的枚举值.为了查询安全缓解策略,系统规定必须传入枚举值52(对应  ProcessMitigationPolicy).但如果直接写 ProcessMitigationPolicy = 52被逆向的时候直接告诉对方这里在查询系统安全策略
/// 
/// 后续因该自己改变这两个做|运算的值,但要确保|后是52
const PROCESS_COOKIE: u32 = 36;
const PROCESS_USER_MODE_IOPL: u32 = 16;

/// mitigation policy id for cfg
/// 把52传给内核,表示要查询进程的mitigation policy环节策略.现代win有十几种(如aslr/dep等).PROCESS_MITIGATION_POLICY中的7就是cfg策略
/// 
/// 它被打包到一个结构体(申请表)中,这个结构体的内存地址(指针)作为第三个参数传给NtQueryInformationProcess
const ProcessControlFlowGuardPolicy: i32 = 7i32;

/// Checks if Control Flow Guard (CFG) is enabled for the current process:向win内核询问,当前进程是否开启CFG控制流防护
pub fn is_cfg_enforced()->Result<bool> {
    
// 调用NtQueryInformationProcess 需要传入的结构体
let mut proc_info=EXTENDED_PROCESS_INFORMATION{

    ExtendedProcessInfo:ProcessControlFlowGuardPolicy as u32,
    ..Default::default()


};


// 查询 .见注释2
let status =NtQueryInformationProcess(NtCurrentProcess(), PROCESS_COOKIE | PROCESS_USER_MODE_IOPL, 
// 将一个结构体引用转为一个原始指针.详见注释1
&mut proc_info as *mut _ as *mut c_void, 
size_of::<EXTENDED_PROCESS_INFORMATION>() as u32, null_mut()) ;

if !NT_SUCCESS(status) {
    stealth_bail!(crate::error::HypnusError::NtQueryInformationProcessFailed,"NtQueryInformationProcess Failed")
}


 Ok(proc_info.ExtendedProcessInfoBuffer != 0)


}


/// Adds a valid CFG call target for the given module base and target function.









// 注释1
// Rust as转换规则涉及到指针时:1. 引用转裸指针 &mut T -> *mut T(T的类型必须完全相同); 2. 裸指针转裸指针 *mut T -> *mut U(只要求T和U都是裸指针,T和U可以是不用类型)
// 因此不允许出现 &mut proc_info as *mut c_void这种&mut T -> *mut U情况
// rustc的规则过于死板,但可通过降级来解决:先用规则1,转为*mut T,再用规则2 转为*mut U.

// 注释2
// 这里没有用unsafe block:
// Rust中制造/转换裸指针是绝对安全的,即只要不真正去动内存,地址之间如何转换都可疑.
// &mut proc_info as *mut _ :制造指针 ; as *mut c_void:转换指针 这些都没有真正的读写内存,只有当动内存时 比如 解引用指针时,才要求必须用unsafe
// NtQueryInformationProcess是一个外部的C的ffi.Rust调用任何外部c函数,理论上必须放在unsafe中,因为Rust无法保证外部的c会不会破坏.
// 在winapis.rs中NtQueryInformationProcess的定义时,将真正的NtQueryInformationProcess api调用已经放入unsafe中了.这是Rust ffi中的安全包装