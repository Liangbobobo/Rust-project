#![allow(unused)]

use core::ptr::null_mut;
use puerto::hash::{fnv1a_utf16,fnv1a_utf16_from_u8};
use crate::{debug_log,stealth_bail};// replace anyhow
use puerto::winapis::{NtCurrentProcess,NT_SUCCESS};
use puerto::module::{get_module_address,get_proc_address,get_ntdll_address};
use crate::types::*;// crate代表本库(crate)的根目录




/// Stores resolved DLL base addresses and function pointers
/// 
/// 是对winapis.rs->Win
#[derive(Default,Debug,Clone,Copy)]
pub struct Config{
    
}