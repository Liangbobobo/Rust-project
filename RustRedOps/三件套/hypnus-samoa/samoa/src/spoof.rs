#![allow(unused)]

use core::{ops::Add,ptr::null_mut,slice::from_raw_parts};

use obfstr::obfstr as s;
use crate::{debug_log,stealth_bail};
use puerto::types::{CONTEXT,IMAGE_RUNTIME_FUNCTION,IMAGE_DIRECTORY_ENTRY_EXCEPTION};
use puerto::{winapis::{NtCurrentProcess,NT_SUCCESS},
helper::PE,
};
use crate::gadget::{GadgetKind};
use crate::winapis::{
NtLockVirtualMemory,
NtAllocateVirtualMemory,
NtProtectVirtualMemory
};


/// represent a reserved stack region for custom thread execution伪造的函数执行栈帧
/// 
#[derive(Debug,Default,Clone,Copy)]
pub struct StackSpoof{
/// address of a gadget_rbp,which realigns the stack(mov rsp,rbp; ret).将备份真实栈地址的rbp,重新对齐realign到rsp,然后ret继续执行真正的执行流
gadget_rbp:u64,

/// stack frame size for BaseThreadInitThunk
base_thread_size:u32,

/// stack frame size for RtUserThreadStart
rtl_user_thread_size:u32,

/// stack frame size for EnumResourcesW
enum_date_size:u32,

/// stack frame size for RtlAcquireSRWLockExclusive
rtl_acquire_srw_size:u32,

/// type of gadget(call [rbx] or jmp [rbx])
gadget:GadgetKind,

}