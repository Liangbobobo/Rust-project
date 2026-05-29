#![allow(unused)]

// cfg=control flow guard 安全机制



use core::{ffi::c_void,ptr::null_mut};
use crate::{debug_log,stealth_bail};
use obfstr::{obfcstr as s};
use puerto::winapis::{NtCurrentProcess,NT_SUCCESS};
use puerto::helper::PE;
