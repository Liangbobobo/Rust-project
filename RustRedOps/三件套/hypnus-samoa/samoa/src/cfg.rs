#![allow(unused)]

use core::{ffi::c_void,ptr::null_mut};
use crate::{debug_log,stealth_bail};
use puerto::winapis::{NtCurrentProcess,NT_SUCCESS};
use puerto::helper::PE;
