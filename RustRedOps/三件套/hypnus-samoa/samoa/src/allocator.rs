use core::{
    alloc::{GlobalAlloc, Layout},
    ffi::c_void,
    ptr::{NonNull, null_mut},
};

use puerto::types::HANDLE;
// = 0x00000002:RtlCreateHeap的flags参数,代表win的堆管理器可以在私有堆空间不够用时,自动向系统虚拟内存管理器申请新内存页
use crate::types::HEAP_GROWABLE;

/// Global handle to the custom heap used by `HypnusHeap`.
static mut HEAP_HANDLE: Option<NonNull<c_void>> = None;

/// A thread-safe wrapper for managing a Windows Heap.
pub struct HypnusHeap;

impl HypnusHeap {
    fn create_heap() -> HANDLE {
        let handle = unsafe {
            // 返回代表新堆内存的handle,win api中本质是*mut c_void
            RtlCreateHeap(
                // 下面的reserve_size/commit_size传入0:保留/提交大小使用系统默认值(保留1MB/提交64KB)
                // 这个flags代表初始堆很小,会自动向os要内存
                HEAP_GROWABLE,
                // heap_base:随机挑选堆内存基址
                null_mut(),
                0,
                0,
                null_mut(),
                null_mut(),
            )
        };

        let nonnull = unsafe {
            // 不检查是否为空,但还是用了NonNull为了rustc的空指针优化
            NonNull::new_unchecked(handle)
        };

        unsafe { HEAP_HANDLE = Some(nonnull) };

        handle
    }

    /// Returns the handle to the default process heap.
    ///
    ///
    pub fn get() -> HANDLE {
        // 此处读取了全局可变静态变量HEAP_HANDLE.而rust中多线程读取static mut会引发data race.所以必须用unsafe显示标明
        unsafe {
            // map : lazy init
            // HEAP_HANDLE只有两种状态:None(未初始化的没有指针状态),Some(NonNull)
            // None:直接短路并返回None;否则执行闭包中NonNull::as_ptr():将非空指针强转为可为空的*mut c_void
            HEAP_HANDLE.map(|p|p.as_ptr()).
        // 1. unwrap Some(T);2. 如果是None:调用传入的函数指针(这里是通过后续win api创建堆)
        unwrap_or_else(Self::create_heap)
        }
    }
}

// GlobalAlloc trait本身是unsafe:编译器无法保证自定义的堆内存大小/对齐等
unsafe impl GlobalAlloc for HypnusHeap {
    /// Allocates memory using the custom heap.
    ///
    ///
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // get()签名中未传入&self:不是一个方法而是关联函数,而关联函数调用方式是类名::函数名()即HypnusHeap::get()=Self::get()
        // 使用关联函数而不是方法:因为HEAP_HANDLE是一个static mut,调用get()不需要依赖HypnusHeap/其他实例,所以使用关联函数最合适
        let heap = Self::get();
        // layout.size()由编译器实现
        let size = layout.size();
        if size == 0 {
            return null_mut();
        }
        unsafe { RtlAllocateHeap(heap, 0, size) as *mut u8 }

        // 分配时不需要对内存刷零:详见md文件
    }

    /// Deallocates memory using the custom heap.
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }

        unsafe { core::ptr::write_bytes(ptr, 0, layout.size()) };
        unsafe {
            RtlFreeHeap(Self::get(), 0, ptr.cast());
        }
    }
}

// 通过windows_targets::link!实现跨平台的static linking需要的函数
windows_targets::link!("ntdll" "system" fn RtlAllocateHeap(heap: HANDLE, flags: u32, size: usize) -> *mut c_void);
windows_targets::link!("ntdll" "system" fn RtlCreateHeap(
    flags: u32,
    heap_base: *mut c_void,
    reserve_size: usize,
    commit_size: usize,
    lock: *mut c_void,
    parameters: *mut c_void) -> HANDLE
);

windows_targets::link!("ntdll" "system" fn RtlFreeHeap(heap:HANDLE,flags:u32,ptr: *mut c_void)->i8);
