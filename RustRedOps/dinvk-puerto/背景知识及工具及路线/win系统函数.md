- [WinApi](#winapi)
  - [参考资料](#参考资料)
  - [Kernel32.dll](#kernel32dll)
  - [ntdll](#ntdll)
  - [RtlAllocateHeap](#rtlallocateheap)
    - [RtlAllocateHeap分配内存的来源](#rtlallocateheap分配内存的来源)
  - [RtlFreeHeap](#rtlfreeheap)
  - [扩展-源dinvk中为啥将GetProcessHeap()作为默认内存分分配器](#扩展-源dinvk中为啥将getprocessheap作为默认内存分分配器)
  - [扩展-为什么 dinvk 不动态查找 RtlAllocateHeap](#扩展-为什么-dinvk-不动态查找-rtlallocateheap)
  - [扩展- Windows 的内存管理](#扩展--windows-的内存管理)


# WinApi

有必要深入c吗?

## 参考资料

这里主要是win内核的原生api,有些是公开的,有些是未公开逆向出来的.主要参考:  
1. https://github.com/winsiderss/phnt/blob/fc1f96ee976635f51faa89896d1d805eb0586350/ntrtl.h#L5314

## Kernel32.dll


## ntdll
，ntdll.dll 是用户态（User Mode）与内核态（Kernel Mode）之间的最后一道防线


## RtlAllocateHeap

* Rtl=Run-Time Library (运行时库),它们不直接进内核，而是在用户态完成复杂的逻辑（如堆管理、字符串处理、压缩、哈希）
  * Nt...：系统调用的入口,它们直接通往内核;
  * Zw...：在用户态，它们等同于 Nt；在内核态，它们有特殊含义

* 作用:它是 Windows内存管理体系中的“总管”，是所有用户态内存分配的终点


* 位置:ntdll.dll
  * 当你调用Kernel32.dll中的 HeapAlloc 时，它内部百分之百会调用 ntdll.dll 里的RtlAllocateHeap 
  * 在红队开发中，直接调用 ntdll 里的函数被称为 "Direct Native Call"，这比调用
  kernel32 要隐蔽得多，因为很多基础的监控工具只挂钩(Hook)高层的 kernel32API

* 有无RtlAllocateHeap的区别
  *  没有RtlAllocateHeap：你只能手动管理静态数组，或者每次申请内存都直接找内核要
  *  有了 RtlAllocateHeap：你只需要把 GetProcessHeap()拿到的句柄交给它，它就能像正常的 Rust环境一样，为你提供灵活的小块内存分配

[扩展- Windows 的内存管理](#扩展--windows-的内存管理)

* 原型(phnt/ntrtl.h)
```c
NTSYSAPI
_Success_(return != 0)
_Must_inspect_result_
_Ret_maybenull_
_Post_writable_byte_size_(Size)
__drv_allocatesMem(Mem)
DECLSPEC_ALLOCATOR
DECLSPEC_NOALIAS
DECLSPEC_RESTRICT
PVOID
NTAPI
RtlAllocateHeap(
    _In_ PVOID HeapHandle,
    _In_opt_ ULONG Flags,
    _In_ SIZE_T Size
    );
```

在 Windows内核编程中，微软使用大量的宏来告诉编译器和静态分析工具这个函数“表现得像什么”

这段代码有三部分:编译器指令（Macros）、静态分析注解（SAL Annotations） 和 真正的函数定义

* 第一部分：函数修饰符（编译器指令与宏）,大写的单词通常是宏，定义了函数在链接和编译时的行为

| C 语言代码 | 详细解释 | Rust 对应/含义 |
| :--- | :--- | :--- |
| `NTSYSAPI` | 告诉编译器这个函数是从外部 DLL（通常是 ntdll.dll）导入的。 | `extern "system"` 块的作用。 |
| `PVOID` | 函数的返回值类型。即 `void*`，代表一个通用的内存地址。 | `*mut c_void` 或 `*mut u8`。 |
| `NTAPI` | 调用约定 (Calling Convention)。定义了参数如何压栈、谁来清理堆栈。在 x64 Windows 上，它代表 `__stdcall`。 | `extern "system"` 里的 `"system"` 部分。 |
| `DECLSPEC_ALLOCATOR` | 告诉编译器：这是一个内存分配器。编译器会据此优化调试信息。 | Rust 无直接对应，属于编译器底层优化。 |
| `DECLSPEC_NOALIAS` | 告诉编译器：返回的指针不会与现有的任何指针“重叠”。 | 类似于 Rust 的 `noalias` 属性（Rust 默认会对引用做此优化）。 |
| `DECLSPEC_RESTRICT` | 类似于 C 语言的 `restrict` 关键字。表示该指针是访问这块内存的唯一途径。 | 类似于 Rust 的唯一引用（`&mut`）的概念。 |

* 第二部分：SAL 注解（Microsoft 静态分析语言）

这些以下划线开头的代码（如 `_Success_`）是给微软的静态检查工具看的，用来在编译阶段发现逻辑错误.

**SAL注解是好老师，它告诉了你调用这个函数时需要注意的坑（是否可能为空、是否必须检查结果）**

| C 语言代码 | 详细解释 | Rust 逻辑对比 |
| :--- | :--- | :--- |
| `_Success_(return != 0)` | 逻辑：如果返回值不是 0，代表函数执行成功。 | 在 Rust 中，我们通常用 Option 或 Result 来表达这种“成功/失败”的逻辑。 |
| `_Must_inspect_result_` | 强制要求：调用者必须检查返回值，不能忽略。 | 对应 Rust 的 #[must_use] 属性。如果你忽略了返回值，Rust 编译器会报警告。 |
| `_Ret_maybenull_` | 警告：这个函数可能返回 NULL。 | 对应 Rust 的 Option<T>。在 Rust 里，你必须处理 None 才能拿到值，而 C 语言里你可能忘记检查 NULL 导致崩溃。 |
| `_Post_writable_byte_size_(Size)` | 声明：返回的内存块大小为 Size 个字节，且是可写的。 | Rust 中通过 slice::from_raw_parts_mut 手动构建切片来体现这种长度关系。 |


* 第三部分：函数参数列表

这是最核心的部分，决定了你调用时需要传什么。

1. `_In_ PVOID HeapHandle`
* **C 含义**：`_In_` 表示这是一个输入参数。类型是 `PVOID`（即地址）。
* **用途**：指定从哪个堆里分配。通常传 `GetProcessHeap()` 的结果。
* **Rust 对应**：`heap_handle: *mut c_void`。

1. `_In_opt_ ULONG Flags`
* **C 含义**：`_In_opt_` 表示这是一个可选的输入参数（可以传 0）。类型是 `ULONG`（32位无符号整数）。
* **用途**：控制分配行为（如 `HEAP_ZERO_MEMORY`）。
* **Rust 对应**：`flags: u32`。

1. `_In_ SIZE_T Size`
* **C 含义**：输入参数。类型是 `SIZE_T`。
* **注意**：`SIZE_T` 的大小取决于系统架构。在 64 位系统上是 8 字节，在 32 位系统上是 4 字节。
* **Rust 对应**：`size: usize`。Rust 的 `usize` 完美对应了 C 的 `SIZE_T`。

### RtlAllocateHeap分配内存的来源

## RtlFreeHeap

```c
#if (PHNT_VERSION >= PHNT_WINDOWS_8)
_Success_(return != 0) // SAL,非0表示内存释放成功
NTSYSAPI // 该函数从外部系统DLL(ntdll.dll)导出,对应rust extern "system"
LOGICAL  // 返回值类型,win8+是ULONG,4字节无符号整数(rust u32也可用i32)
NTAPI   // 调用约定,rust extern "system"
RtlFreeHeap(
    _In_ PVOID HeapHandle, // _In_,纯输入参数;PVOID=*void;rust HeapHandle:*mut c_void
    _In_opt_ ULONG Flags, // 控制释放行为,是否加锁
    _Frees_ptr_opt_ _Post_invalid_ PVOID BaseAddress  // _Frees_ptr_opt_:告诉分析器,该函数会释放该指针指向内存,且指针本身可选(传入空指针不报错)
    // _Post_invalid_ PVOID:win8+后引入的安全补强,告诉编译器,函数返回后,该指针立即失效
    // rust: ptr:*mut c_void
    );
#else// win7及以下版本
_Success_(return)
NTSYSAPI
BOOLEAN // 重要不同,代表返回值是BOOLEAN类型(unsigned char),1字节.rust:u8或i8
NTAPI
RtlFreeHeap(
    _In_ PVOID HeapHandle,
    _In_opt_ ULONG Flags,
    _Frees_ptr_opt_ PVOID BaseAddress
    );
#endif // PHNT_VERSION >= PHNT_WINDOWS_8
```

* win8前后版本的不同:
  * 除了安全性主要是返回类型不同,win7以下为1字节的BOOLEAN,win8+为LOGICAL(ULONG)4字节无符号整数.
    * rust中为了兼容,建议使用u32,在 Rust 中接收时，由于 x64调用约定的对齐特性，读取 4 字节（u32）通常是安全的，因为 1字节的返回值也会填充在 RAX 的低位
  * 注意c源码中可以传入NULL,rust中在调用之前应检查!ptr.is_null()



* RtlFreeHeap是内存生命周期的重点,这里也是红队销毁痕迹和防止取证的关键环节

* 参数
  *  `_In_ PVOID HeapHandle`,必须与调用的handle完全一致(如果从默认堆申请内存,从私有堆释放,会出现Heap Corruption堆损坏)
  *  `_In_opt_ ULONG Flags`,通常为0;0x00000001单线程专用标志
  *  `_Frees_ptr_opt_ PVOID BaseAddress`,之前申请内存分配的指针,也必须一致.如果之前在alloc时做了Padding随机偏移,在调用RtlFreeHeap之前,必须减去哪个padding,还原原始的基地址,否则对管理器找不到元数据头,直接崩溃
  *  RtlFreeHeap没有size参数,怎么知道要释放的内存大小?而NtFreeVirtualMemory则必须传入 Size
     * 堆管理器在分配内存时，会在返回给你的指针前面（通常是 8 或 16字节的位置）悄悄存了一个 Heap Header(堆头)。这个头部记录了这块内存的大小.当RtlFreeHeap释放内存时,获得了该指针,也获得了该指针中alloc时的大小  


* 底层机制:内存去哪了--调用RtlFreeHeap时,内部不会立即归还给os内核
  * 堆管理器会标记该内存为空闲,并根据其大小挂入不同链表,如果出现alloc同样大小内存,堆管理器会优先从对应链表中取(内存重用)
  * 试图Coalescing合并相邻空闲内存块
  * 只有当整个堆空间Segment有大量空闲内存,且达到一定阈值,堆管理器才会调用NtFreeVirtualMemory将内存还给内核

## 扩展-源dinvk中为啥将GetProcessHeap()作为默认内存分分配器


第一部分：深度解析 GetProcessHeap()

在 Windows 中，“堆 (Heap)”是进程虚拟地址空间中用于动态分配内存的一块区域。

1. 什么是“默认堆”？
每个 Windows 进程在启动时，内核都会自动为它创建一个默认堆 (Default Process Heap)。这个堆的句柄（Handle）存储在一个非常关键的地方——PEB (Process Environment Block)。
* 位置：在 x64 系统中，它位于 PEB 结构体偏移 0x30 的位置。
* 特性：它是系统级别的“公共水桶”。当你调用标准的 C 语言 malloc 或者 Rust 的 Box::new 时，底层通常都是在这个默认堆里抠出内存。

2. GetProcessHeap() 到底做了什么？
当你调用 GetProcessHeap() 时，它其实并不执行复杂的逻辑，它只是：
1. 找到当前线程的 TEB (通过 GS 寄存器)。
2. 从 TEB 找到 PEB 的地址。
3. 从 PEB 的 0x30 偏移处读取那个已经存在那里的 HANDLE 并返回。

3. 为什么 RtlAllocateHeap 需要它？
RtlAllocateHeap 是一个通用的堆管理函数。Windows 允许一个进程拥有多个堆（通过 HeapCreate 手动创建）。所以你必须告诉它：“我想从哪个堆里拿内存？”
* 传 GetProcessHeap() 的结果：使用进程默认的“大水桶”。
* 传手动创建堆的句柄：使用你私人定制的小水桶。



## 扩展-为什么 dinvk 不动态查找 RtlAllocateHeap

之所以在这里使用静态链接（windows_targets::link!）而不是 get_proc_address 动态查找，不是因为偷懒，而是因为一个无法绕过的技术死锁：引导悖论 (Bootstrapping Paradox)  
Puerto之所以能打破这个悖论，是因为它实现了“零依赖 (Zero-Dependency)的哈希查找”，即查找过程不产生任何 String 或 Vec

让我们审视 dinvk 的源码逻辑：

1. get_proc_address 的依赖
请看 dinvk/src/module.rs 中 get_proc_address 的实现：
* 它接收的 function 参数是 T: ToString。
* 内部使用了 function.to_string()。
* 处理转发（Forwarding）时使用了 alloc::format! 和 Vec。
* 结论：dinvk 的 get_proc_address 函数依赖于内存分配器 (alloc) 才能正常工作。

1. 内存分配器的依赖
* 内存分配器（WinHeap）依赖于 RtlAllocateHeap 才能工作。

1. 致命的逻辑死锁
如果你想通过 get_proc_address 动态获取 RtlAllocateHeap：
1. 调用 get_proc_address("RtlAllocateHeap")。
2. 该函数内部调用 to_string()。
3. to_string() 申请内存，调用 GlobalAlloc::alloc。
4. GlobalAlloc::alloc 发现自己还没有 RtlAllocateHeap 的地址，于是试图去获取它...
5. 砰！无限递归导致崩溃。

这就是为什么 dinvk 必须静态链接 RtlAllocateHeap 的原因：它是整个程序的“第一块地基”。在这块地基打好之前，你无法使用任何涉及 String、Vec 或 format! 的高级 Rust 功能。

---

* puerto 的优化

你在重构中做了一个极其关键的改变：你的 get_proc_address 参数改成了 ModuleType (包含 Hash/u32)，且算法改成了直接操作 &[u16]。

这意味着：
* 你的 get_proc_address 不再依赖内存分配 (Zero Alloc)。
* 它可以在 WinHeap 还没初始化的时候就跑通。

你的终极战术：
1. 静态阶段：定义一个不依赖 alloc 的 get_proc_address
2. 引导阶段：在程序刚开始时，用这个“零分配”函数找到 RtlAllocateHeap。
3. 初始化阶段：把找到的地址塞进 WinHeap 的静态变量里。
4. 全面运行阶段：从此以后，所有的 String、Vec 都可以使用了。

总结：
dinvk 的作者为了库的通用性和易用性，牺牲了一部分隐蔽性，选择了静态链接。而你在重构 puerto 时，通过“零分配哈希查找”技术，打破了引导悖论。这正是你作为后来者，在“免杀/隐蔽性”这个维度上能够超越前辈的地方。

## 扩展- Windows 的内存管理

**一、 Windows 内存分配的系统层级**

| 层级 | 负责人 (模块) | 分配单位 | 核心函数 | 类比 |
| :--- | :--- | :--- | :--- | :--- |
| 应用层 | 编程语言运行时 (Rust alloc) | 字节 (Bytes) | Box::new, Vec | 租客 |
| 堆管理层 | 堆管理器 (ntdll.dll) | 小块内存 (Chunks) | RtlAllocateHeap | 二房东 (转租) |
| 虚拟内存层 | 内核 VMM (ntoskrnl.exe) | 页面 (Pages, 4KB) | NtAllocateVirtualMemory | 大地主 (土地局) |

---

**二、 NtAllocateVirtualMemory,页面的分配**

它是系统调用（Syscall），直接与内核对话。它的特点是：
1. 粒度粗：它只能以 4KB (Page) 为最小单位进行操作。你找它要 1 字节，它也会给你 4096 字节。
2. 两步走策略：
    * Reserve (保留)：在虚拟内存里占个坑（划线），但不分配物理内存。
    * Commit (提交)：真正把物理内存（或者交换文件）挂载上去，这时候才真正消耗内存条。

---

**三、 RtlAllocateHeap逻辑**

堆管理器存在的意义，就是为了解决内核“起步价太高”的问题。它的工作流程如下：

1. 初始化（建池）：进程启动时，堆管理器先通过 NtAllocateVirtualMemory 向内核“批发”一大块连续的虚拟地址空间（比如 1MB）。
2. 切片（零售）：当你的程序调用 RtlAllocateHeap 请求 16 字节时，它在自己手里的那 1MB 内存池里找一个空位，标记为“已用”，然后把地址给你。
3. 回收：当你释放内存时，它只是在内部把这块地标为“空闲”，并不会归还给内核。


**四、 深度解析：两者如何交互？**


场景 1：堆内存充足
* 调用 RtlAllocateHeap(100 字节)。
* 堆管理器检查自己的 FreeList（空闲链表）。
* 交互：发现池子里还有地，不需要 找内核。直接从应用层内存池切一块给你。
* 结果：速度极快，不进内核。

场景 2：堆内存耗尽（关键交互）
* 调用 RtlAllocateHeap(100 字节)。
* 堆管理器发现池子全满了，没地了。
* 交互步骤：
    1. 堆管理器调用 NtAllocateVirtualMemory，请求增加一个新的 Segment (段)（通常是 64KB 或更大）。
    2. 内核大地主批了这块地。
    3. 堆管理器将这块新地加入自己的管理范围。
    4. 最后从新地里切出 100 字节给用户。
* 结果：触发了一次昂贵的系统调用。

场景 3：请求超大内存
* 调用 RtlAllocateHeap(2MB 字节)。
* 堆管理器发现这个请求太大了，超过了它管理的常规块限制（Large Allocation）。
* 交互步骤：
    1. 堆管理器放弃“切片”逻辑。
    2. 直接代表用户调用 NtAllocateVirtualMemory。
    3. 这块内存被标记为 "VirtualAllocated"，它不属于任何常规堆池，而是独立的。
* 结果：直接透传给内核。

**五、 给 puerto 重写的技术启示**

1. 为什么 allocator.rs 用 RtlAllocateHeap？
因为你的 Rust 代码会频繁申请小内存（比如存一个 API Hash）。如果你每次都调用 NtAllocateVirtualMemory，你的程序性能会非常差，且内存碎片会瞬间填满进程空间。

2. 免杀上的差异：
    * RtlAllocateHeap：它的调用非常频繁，通常不会被作为敏感行为拦截。
    * NtAllocateVirtualMemory：它是 EDR 监控的重灾区。尤其是参数里带有 PAGE_EXECUTE_READWRITE (0x40) 时，EDR 会立刻警报。

3. 你的策略：
    * 你的 Global Allocator 应该老老实实地用 RtlAllocateHeap 来维持程序的日常运转。
    * 但当你准备注入 Shellcode 或者分配存放恶意代码的内存时，你应该跳过堆管理器，直接调用 NtAllocateVirtualMemory。因为这样你可以更精确地控制内存保护属性（比如先 R/W，运行前再改为 R/X），绕过堆管理器的默认安全限制。