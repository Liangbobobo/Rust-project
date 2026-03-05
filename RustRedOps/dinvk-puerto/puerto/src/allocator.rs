// 为啥需要手写allocator
// 在有std的rust项目中,不需要关心内存分配.但是在这种#![no_std]环境中不手写allocator:
// 会失去默认分配器,std中有一个基于os api(如win下的HeapAlloc)的全局分配器,在#![no_std]中自然没有了这个默认分配器
// 项目中使用了Vec String等,这些定义在rust alloc库中.但alloc库本身只负责逻辑,它需要一个底层搬运工帮它向os申请内存
// 如果只写了 extern crate alloc,不提供分配器,rustx在链接时会出现,error: no global memory allocator found

// allocator的作用:
// 将rust的高层代码(如 String::from("..."))的内存申请,转为底层windows原生系统调用
// std的分配器包含很多复杂的安全检查和性能缓存逻辑,手写可以减小生成的shellcode或dll的体积
// 增加隐蔽性

// 待优化
// 1.链接外部win的函数时,会在IAT中留下记录


// 该文件实现内存分配的逻辑是什么?一步一步的说明,比如1.新建一个空WinHeap结构,作为.. 2. 在该结构体实现get获取..


use core::{alloc::GlobalAlloc, ptr::null_mut};
use core::ffi::{c_void};
use core::ptr::write_bytes;
use spin::lock_api::Mutex;

// 获取当前进程的默认heap handle
use crate::{types::HANDLE, winapis::GetProcessHeap};

/// unit-like struct(单元结构体),不占用任何内存空间
/// 
/// 作用: rust中,要实现分配器必须先定义一个类型,再为这个类型实现GlobalAlloc trait.这里WinHeap就是向rust编译器说明将使用win的Default Process Heap(默认进程堆)管理内存.每个win进程都有一个默认堆,且默认堆有一个堆句柄(Heap Handle)
/// 
/// 
pub struct WinHeap;

impl WinHeap {

    #[inline]
    fn get(&self)->HANDLE {
        GetProcessHeap()
    }
}

unsafe impl GlobalAlloc for WinHeap {
    
    // allocates memory using the custom heap
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {

        // self代表一个WinHeap的实例,等同WinHeap::get(&self)
        let heap =self.get();

        // 获取调用者需要的内存大小,详见winapi
        let size =layout.size() ;

        // size为0的情况
        if size==0 {
            return null_mut();
        }
        unsafe {
            let ptr =RtlAllocateHeap(
                heap,
                0,// 不要使用0x00000008这个有明显特征的magic num
                size
            );

            
            // 需要判断RtlAllocateHeap返回指针是否为空
            // write_bytes()返回(),而()不能直接转为*mut u8,所以这里不能在RtlAllocateHeap中链式调用write_bytes()
            if !ptr.is_null() {
                write_bytes(ptr as *mut u8, 0, size);
            }  

            // 这里与ptr as *mut u8在生成的二进制文件中没有任何区别,不会产生额外的指令
            ptr.cast()    
        }
        
    }

      /// Deallocates memory using the custom heap.
    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        if ptr.is_null() {
            return;
        }
    
        unsafe { core::ptr::write_bytes(ptr, 0, layout.size()) };// 释放前将内存数据归0,避免被扫描
        unsafe { RtlFreeHeap(self.get(), 0, ptr.cast()); }
    }

}


// 源dinvk中使用了windows_targets::link!(通过在toml中引入windows-target这个crate),这里直接使用extern "system"达到同样目的,且为rust原生
// 告知Rust编译器,查找给定签名的函数,链接到本程序中,可以像使用本地函数一样使用找到的函数.在编译时,会查找对应的lib文件,在运行时由windows 加载器把里面的地址填写进来.但这么做会在IAT(导入表)表留下记录
// extern "system"这种方式,编译器(rustc)不关心函数在哪,只是记录本程序需要一个对应的外部函数,调用约定是system(x64的fastcall),编译器会在生成的.obj文件中留下一个未解析的外部符号.
// 当运行cargo build或cargo test时,链接器(如MSVC的link.exe)会扫描你提供的所有库文件(lib文件)及rustc自动链接的一些win的基础库(如ntdll kernel32)等,发现对应的函数定义,链接器就会将这里的代码和ntdll里面的函数关联起来
// windows-targets 宏只是指定了在哪个模块里面找,而extern会扫描所有模块.当出现重名函数时windows-targets 宏就很有优势了
unsafe  extern "system"{
    fn RtlFreeHeap(heap: HANDLE, flags: u32, ptr: *mut c_void) -> u32;// 为了兼容改为u32,详见winapi的分析
    fn RtlAllocateHeap(heap: HANDLE, flags: u32, size: usize) -> *mut c_void;
}

// 告诉编译器WinHeap是全局分配器,下面两句可以放在文件开始或者末尾
#[global_allocator]
static ALLOCATOR:WinHeap=WinHeap;


// 优化方案

//  正如你所观察到的，你的函数签名如下：


//    1 pub fn get_proc_address(
//    2     h_module: Option<HMODULE>,
//    3     function: hash_type, // 即 Option<u32>
//    4     hash_func: Option<fn(&[u16]) -> u32>,
//    5 ) -> Option<*mut c_void>


//   关键点：它完全避开了内存分配。
//    1. 输入是 Hash：调用者直接传 0x803BA0E0，不需要传字符串
//       "NtAllocateVirtualMemory"。
//    2. 原地转换：在遍历导出表时，你使用了 from_raw_parts 和 u8_to_u16
//       缓冲区（虽然目前是 256 字节的栈空间），这避免了创建 String 对象。
//    3. 零 `alloc`
//       依赖：这意味着这个函数在你的内存分配器（WinHeap）初始化之前就能跑
//       通！

//   ---

//   基于此现状，重构 allocator.rs 的“终极方案”建议


//   既然你的 get_proc_address
//   已经具备了“无分配解析”的能力，那么我们之前讨论的 “冷启动单次解析”
//   方案就有了完美的落地前提。

//   重构 `allocator.rs` 的具体步骤建议：


//    1. 第一步：定义全局指针
//       在 allocator.rs 顶部定义两个静态变量，用来存储 RtlAllocateHeap 和
//   RtlFreeHeap 的地址。


//    2. 第二步：实现 `init` 逻辑
//       利用你的 get_ntdll_address() 和 get_proc_address()（传入预先算好的
//   Hash），获取这两个 API 的真实地址。

//    3. 第三步：实现 `GlobalAlloc`
//       在 alloc 函数中，直接使用 mem::transmute
//   将存储的地址转为函数指针并调用。


//   这样做的一个巨大好处：
//   你的 allocator.rs 将不再有任何 extern "system" 块。
//    * 静态分析结果：你的二进制文件 IAT 表里完全没有 RtlAllocateHeap。
//    * 动态分析结果：你只在启动时做了一次极其隐蔽的 Hash
//      查找，后续全是纯指针调用。