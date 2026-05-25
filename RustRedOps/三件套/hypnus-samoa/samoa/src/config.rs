#![allow(unused)]

use core::ptr::null_mut;
use puerto::hash::{fnv1a_utf16,fnv1a_utf16_from_u8};
use puerto::winapis::{NtCurrentProcess,NT_SUCCESS};

use puerto::module::{get_module_address,get_proc_address,get_ntdll_address};

// crate代表本库(crate)的根目录
use crate::types::*;


#[derive(Default,Debug,Clone,Copy)]
pub struct Config{
    
}