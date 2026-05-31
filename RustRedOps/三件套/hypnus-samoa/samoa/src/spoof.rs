#![allow(unused)]

use core::{ops::Add,ptr::null_mut,slice::from_raw_parts};

use obfstr::obfstr as s;
use crate::config::Config;
use crate::{debug_log,stealth_bail};
use crate::error::Result;// replace anyhow::Result
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

impl StackSpoof {
    
    #[inline]
  pub  fn new(cfg:&Config)->Result<Self> {
        todo!()
    }

    /// allocates memory required for spoof stack execution
pub fn alloc_memory(cfg:&Config)->Result<Self> {
    // Check that the algo module contains a gadget `call [rbp]` or `jmp [rbp]` from kernelbase.什么是kernelbase 见注释1
    
    todo!()
}




}




// 注释1
// win10以后,kernel32.dll已变为转发动态链接库(forwarder dll):其内大部分api只保留导出符号,其函数体实现/逻辑转到kernelbase.dll
// ntdll.dll是用户态进入内核的最后边界,是edr重点监控的核心.而kernelbase.dll作为win32业务逻辑承载模块,其内部控制流跳转比ntdll.dll频繁/复杂.edr很难对其内部所有通用寄存器跳转无差别的全阻断监听
// win64使用结构化异常处理SEH,其栈回溯依赖PE文件的.pdata节(Exception Directory)记录的栈展开元数据unwind codes.同时如果伪造的返回地址指向一个不具备有效展开元数据的函数段/指令序列与该地址在.pdata中注册栈帧释放规则不匹配,win的虚拟栈展开器(RtlVirtualUnwind)在回溯到该地址时会直接报错/触发异常
// kernelbase.dll是一个体积巨大的库,包含大量具有通用栈结构的帧函数,能够完美支持和匹配伪造栈帧的大小(如BaseThreadInitThunk对应的栈大小)