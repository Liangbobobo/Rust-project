// 禁用#[derive(Debug)]
// 禁用result<>,改用option<*mut c_void>

use alloc::{
    format, vec::Vec, vec,
    string::{String, ToString}, 
};
use core::{
    ffi::{CStr, c_void}, ptr::null_mut, slice::from_raw_parts
};

// use crate::{hash};
use crate::{types::EVENT_ALL_ACCESS};
use obfstr::obfbytes;
use crate::types::{HMODULE,LDR_DATA_TABLE_ENTRY};
use spin::Once;
use crate::winapis::{NtcurrentPeb};
use crate::helper::{PE};

/// crate a static variable to store the ntall.dll's address
/// 
/// 
static NTD:Once<u64>=Once::new();

// 区分retrieve_moudle_add传入的参数类型
pub enum MoudleType {
    // name(& 'a str), // 非特殊情况不应该传入模块名称 
    hash(u32),
    empty
}

/// 获取模块基址
/// 
///  let addr = retrieve_moudle_add(MoudleType::hash(0x12345678),Some(fnv1a_utf16) 
/// 
/// 使用Option(定义在core中)不需要引入std
#[inline(always)]
pub fn retrieve_moudle_add<T>(module:MoudleType,
hash_func:Option<fn(&[u16]) -> u32>)->Option<HMODULE>
// where T:ToString
{   
    // 成功会返回u32类型的hash值,并赋值给左侧的hash变量
    // 失败会返回None,并退出retrieve_moudle_add
    let hash = hash_func?;

    unsafe {
        let peb=NtcurrentPeb();
        let ldr = (*peb).Ldr;

        let mut InMemoryOrderModuleList_flink=(*ldr).InMemoryOrderModuleList.Flink;
        let mut InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY = (*ldr).InMemoryOrderModuleList.Flink as *const LDR_DATA_TABLE_ENTRY;
        
        if let MoudleType::empty=module
        {   
            return Some((*peb).ImageBaseAddress);
        }

        let  head_node= InMemoryOrderModuleList_flink;
        let mut addr = null_mut();

        while !(*InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY)
        .FullDllName
        .Buffer.is_null() {
            
            if (*InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY)
            .FullDllName
            .Length!=0
             {
                let buffer=from_raw_parts((*InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY).FullDllName.Buffer, 
            ((*InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY).FullDllName.Length/2) as usize);

                if let MoudleType::hash(dll_hash) = module {
                     if dll_hash == hash(buffer) {
                        addr = (*InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY).Reserved2[0];
                        break;
                }
            }

            InMemoryOrderModuleList_flink=(*InMemoryOrderModuleList_flink).Flink;
           
            if InMemoryOrderModuleList_flink==head_node{break;}

            InMemoryOrderModuleList_flink_LDR_DATA_TABLE_ENTRY=InMemoryOrderModuleList_flink as *const LDR_DATA_TABLE_ENTRY
        }

        }
      
      if addr.is_null(){
        None
    }
      else{
        Some(addr)
      }
         
      }
    }
    

// 与dinvk的原代码对比,重写及删除部分是否更优?
pub fn get_proc_address(
    h_moudle:Option<HMODULE>,
    function:MoudleType,
    hash_func:Option<fn(&[u16]) -> u32>
) -> Option<*mut c_void>{
    
// 使用? ,当Some会解出里面的内容并向左赋值,None会直接让整个 get_proc_address 函数返回None
let h_moudle_base=h_moudle?;

// initializes a new pe struct
let pe=PE::parse(h_moudle_base);
unsafe {
    // 这里的zip逻辑会在后续实现中完善
    // let Some((nt_header,export_dir))=pe.nt_header().zip(pe.exports)
}

todo!()
}