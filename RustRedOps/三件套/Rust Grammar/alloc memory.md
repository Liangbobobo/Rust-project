# 从底层到高层的内存

从最原始的地基——物理内存开始，一层层向上构建到Rust 的 GlobalAlloc 抽象。

第一层：地基 —— 虚拟内存与页（Virtual Memory & Pages）在系统底层，程序看到的内存全是“假”的。

1. 物理内存 (RAM)：真实的硬件电信号存储单元。
2. 虚拟内存 (VA)：操作系统给每个进程画的 128TB（64位系统）的大饼。
3. 内存管理单元 (MMU)：硬件层面的转换器。它通过页表 (Page Tables)将你的虚拟地址映射到物理地址。
4. 页 (Page)：操作系统管理内存的最小单位，通常是 4KB。
  * 所有的内存操作最终都要归结为：申请页 -> 设置页权限 (RWX) ->读写页。
  * RedOps视角：所有的检测（EDR）和规避（Bypass）都在页权限上做文章。比如PAGE_EXECUTE_READ（代码段）和 PAGE_READWRITE（数据段）。

第二层：建筑师 —— 堆管理器 (The Heap Manager)如果你每次需要 16 字节都要找内核（NtAllocateVirtualMemory）要一个 4KB的页，那是极大的浪费。于是有了堆（Heap）。
1. 堆是什么？堆是一个内存池管理器。它向内核批发一大块内存（Pages），然后零售给程序。
   1. Windows NT Heap (ntdll.dll)：
       * Windows 核心的堆管理引擎是 Rtl 系列函数（RtlCreateHeap,RtlAllocateHeap）。
       * 它负责处理内存碎片、合并空闲块、多线程竞争等极其复杂的逻辑。
   2. 私有堆 vs 默认堆：
       * 每个进程都有一个默认堆（GetProcessHeap()）。
       * hypnus 选择用 RtlCreateHeap 创建私有堆。
       * 使用场景：隔离。如果你的 Shellcode崩溃了，或者你想一次性加密所有恶意数据，私有堆让你能“一网打尽”而不干扰进程的其他部分。

第三层：桥梁 —— Rust 的 GlobalAlloc 抽象:Rust 是一门追求零开销抽象的语言，它对内存分配的需求非常克制。

1. alloc Crate：Rust 内置的 core 库没有内存分配能力。当你使用 Vec,Box, String 时，你需要 alloc 库，而它要求你必须指定一个全局分配器(`#[global_allocator]`)。
   1. Layout 结构体：Rust 分配内存时不仅要 size，还要 align（对齐）。
       * Layout { size: 16, align: 8 } 告诉分配器：我要 16字节，且起始地址必须能被 8 整除。
       * 底层内幕：不正确的对齐会导致 CPU执行效率下降，甚至在某些架构（如 ARM）上直接崩溃。
   2. GlobalAlloc 特质 (Trait)：这是 Rust 给开发者的“插槽”
       * 只要你实现了 alloc 和 dealloc 这两个方法，你就可以接管整个 Rust程序的内存生命周期。



第四层：实战 —— 审视 hypnus/src/allocator.rs

  现在，我们将这些理论代入你正在看的源码：

    1 pub struct HypnusHeap;
    2
    3 unsafe impl GlobalAlloc for HypnusHeap {
    4     unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    5         let heap = Self::get(); // 1. 获取（或创建）私有堆
    6         let size = layout.size();
    7         // ...
    8         unsafe { RtlAllocateHeap(heap, 0, size) as *mut u8 } // 2.
      向 Windows 零售商买内存
    9     }
   10
   11     unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
   12         // ...
   13         unsafe { core::ptr::write_bytes(ptr, 0, layout.size()) };
      // 3. 安全擦除（原子级反取证）
   14         unsafe { RtlFreeHeap(Self::get(), 0, ptr.cast()); } // 4.
      还给零售商
   15     }
   16 }

  关键用例与使用场景分析：

   * 用例：Shellcode 动态加载
       * 当你加载一个反射式 DLL 时，你可以将它的所有内存请求重定向到
         HypnusHeap。
       * 优势：当扫描器（如
         Defender）扫描进程默认堆时，它找不到你的恶意负载。
   * 用例：内存特征混淆
       * 在 hypnus.rs 中，它遍历这个私有堆并 XOR 加密。
       * 背景知识：EDR 经常在线程挂起时扫描堆。如果你在睡眠期间把整个
         HypnusHeap 给加密了，扫描器看到的只是乱码。

  ---

  总结：为什么我们要“费劲”写这个文件？

  如果不写这个文件，Rust 默认会链接到系统分配器（通常是调用 HeapAlloc
  在默认堆上操作）。

  深入底层的意义在于：
   1. 所有权：你拥有了对内存的绝对控制权。
   2. 反取证：你在 dealloc 里的那行 write_bytes(ptr, 0,
      ...)，让内存取证工具（如 Volatility）无法提取到你释放后的密钥。
   3. 绕过扫描：私有堆 + RtlWalkHeap 加密，构成了 hypnus
      最核心的防御壁垒。



      