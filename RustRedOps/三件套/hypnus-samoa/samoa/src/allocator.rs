#![allow(unused)]

use core::{
    alloc::{GlobalAlloc, Layout},
    ffi::c_void,
    ptr::{NonNull, null_mut},
};

use puerto::types::HANDLE;
// = 0x00000002:RtlCreateHeap的flags参数,代表win的堆管理器可以在私有堆空间不够用时,自动向系统虚拟内存管理器申请新内存页
use crate::types::HEAP_GROWABLE;




/// Global handle to the custom heap used by `HypnusHeap`.
static mut HEAP_HANDLE:Option<NonNull<c_void>>=None;

/// A thread-safe wrapper for managing a Windows Heap.
pub struct HypnusHeap;

impl HypnusHeap {
    fn create_heap()->HANDLE {
        
    todo!()}

}




// 通过windows_targets::link!实现跨平台的static linking需要的函数
windows_targets::link!("ntdll" "system" fn RtlFreeHeap(heap:HANDLE,flags:u32,ptr: *mut c_void)->i8);
windows_targets::link!("ntdll" "system" fn RtlAllocateHeap(heap: HANDLE, flags: u32, size: usize) -> *mut c_void);
windows_targets::link!("ntdll" "system" fn RtlCreateHeap(
    flags: u32, 
    heap_base: *mut c_void, 
    reserve_size: usize, 
    commit_size: usize, 
    lock: *mut c_void, 
    parameters: *mut c_void
) -> HANDLE);