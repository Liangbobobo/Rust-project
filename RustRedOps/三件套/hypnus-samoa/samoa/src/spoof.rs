#![allow(unused)]

// 本模块在项目中的作用:




use core::{ops::Add,ptr::null_mut,slice::from_raw_parts};

use obfstr::obfstr as s;
use crate::config::Config;
use crate::{debug_log,stealth_bail};
use crate::error::Result;// replace anyhow::Result
use puerto::types::{CONTEXT, IMAGE_DIRECTORY_ENTRY_EXCEPTION, IMAGE_RUNTIME_FUNCTION, LeapSecondFlags};
use puerto::{winapis::{NtCurrentProcess,NT_SUCCESS},
helper::PE,
};
use crate::gadget::{GadgetKind};
use crate::winapis::{
NtLockVirtualMemory,
NtAllocateVirtualMemory,
NtProtectVirtualMemory
};

/// provides access to the unwind(exception handling)information of a pe image
/// 
/// 该pe专门用于处理exception handling
#[derive(Debug)]
pub struct Unwind{
    /// reference to the parsed pe image
pub pe:PE,
}

impl Unwind {
    pub fn new(pe:PE)->Self {
        Unwind {pe}
    }

/// return all runtime function entries
/// 
/// 语法:&[IMAGE_RUNTIME_FUNCTION]:使用&[T]详见注释2
pub fn entries(&self)->Option<&[IMAGE_RUNTIME_FUNCTION]> {
    
    /// pe->ntheader->ntheader.use PE的同时,也自动引入了对应的inherent methods.详见注释3
    let nt = self.pe.nt_header()?;

    /// ntheader->optionalheader->datadirectory(是一个16个元素的数组,每个元素是Image_Data_Directory类型,其中第3个指向Image_Runtime_Function,结构体为异常目录):异常目录是os用于stack walk栈回溯/SEH,详细记录了函数的栈/寄存器使用情况
    let dir = unsafe {
        (*nt).OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXCEPTION]
    };
    
    if dir.VirtualAddress==0 || dir.Size== 0 {
        return None;
    }
    
    /// 异常目录的地址
    let addr =(self.pe.base as usize + dir.VirtualAddress as usize) as *const IMAGE_RUNTIME_FUNCTION ;
    /// 异常目录的长度(Size是struct IMAGE_DATA_DIRECTORY的一个字段)
    let len = dir.Size as usize / size_of::<IMAGE_RUNTIME_FUNCTION>();

    /// Forms a slice of a pe's IMAGE_DATA_DIRECTORY[IMAGE_RUNTIME_FUNCTION]
Some(unsafe {
    from_raw_parts(addr, len)
})



}

/// Finds a runtime function by its RVA.
/// offset(RVA):使用puerto的函数算出来的VA-对应模块的基址
 pub fn function_by_offset(&self, offset: u32) -> Option<&IMAGE_RUNTIME_FUNCTION> {
        self.entries()?.iter().find(|f| f.BeginAddress == offset)
    }
}




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


// 注释2
// 语法:&[IMAGE_RUNTIME_FUNCTION]:1.该函数的目的是去找IMAGE_RUNTIME_FUNCTION(dll的.pdata节中已经存在),避免使用(Option<Vec<IMAGE_RUNTIME_FUNCTION>>),Vec版本会分配heap去存储.&[T]的本质是rust的胖指针(地址和元素数(不是字节数))
// 2. 使用&[T]方便使用迭代器



// 注释3
// pe的类型是通过use puerto::helper::PE引入的.所用通过impl PE实现的固有方法Inherent methods会自动引入该模块(不需要也不能单独use某个具体的方法)
// inherent methods:直接写在impl块中的.对于trait方法则必须通过use trait来实现