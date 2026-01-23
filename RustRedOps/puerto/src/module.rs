
use alloc::{
    format, vec::Vec, vec,
    string::{String, ToString}, 
};
use core::{
    ffi::{CStr, c_void}, hash, ptr::null_mut, slice::from_raw_parts
};

use crate::{hash::fnv1a};
use crate::error::Error;
use obfstr::obfbytes;
use crate::types::{HMODULE};
use spin::Once;

/// crate a static variable to store the ntall.dll's address
/// 
/// 
static NTD:Once<u64>=Once::new();

#[inline(always)]
pub fn retrieve_moudle_add<T>(module:T,
hash:Option<fn(&str)->u32>)->Result<HMODULE,Error>
where T:ToString
{   
    // 成功会返回u32类型的hash值,并赋值给左侧的hash变量
    // 失败会返回匹配Result中Error类型的错误
    let hash = hash.ok_or(Error::HashFuncNotFound)?;

    // 不调用windows api通过cpu的gs寄存器读取当前进程的peb
    // 
    // gs指向当前线程的TEB起始地址,其offset 0x60(win64)处指向peb地址
    
    


    // 临时加入,避免出现mismatch type的错误,后续需要删除
     Err(Error::ModuleNotFound)
}



// #[inline(always)]
// pub fn retrieve_ntd_add() -> *mut c_void{
//     // call_once返回&T
//     *NTD.call_once(||retrieve_moudle_add(
//         3648013835u32,
//         Some(fnv1a(&str)
//         )
//     ))
// }