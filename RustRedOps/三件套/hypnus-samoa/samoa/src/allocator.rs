#![allow(unused)]

use core::{
    alloc::{GlobalAlloc, Layout},
    ffi::c_void,
    ptr::{NonNull, null_mut},
};

use puerto::types::HANDLE;
// = 0x00000002:RtlCreateHeap的flags参数,代表win的堆管理器可以在私有堆空间不够用时,自动向系统虚拟内存管理器申请新内存页
use crate::types::HEAP_GROWABLE;


static mut HEAP_HANDLE:Option<NonNull<c_void>>=None;
