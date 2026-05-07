- [背景-Rust内存分配](#背景-rust内存分配)
  - [内存分配的一般流程](#内存分配的一般流程)
  - [简单的例子](#简单的例子)
  - [其他需要注意的](#其他需要注意的)
    - [堆隔离-Heap Isolation](#堆隔离-heap-isolation)
    - [内存惩罚 (Zero-fill on Dealloc)](#内存惩罚-zero-fill-on-dealloc)
    - [递归分配风险](#递归分配风险)
- [源码](#源码)
  - [winapis::GetProcessHeap](#winapisgetprocessheap)
      - [PEB中的ProcessHeap字段](#peb中的processheap字段)
  - [RtlAllocateHeap](#rtlallocateheap)
  - [RtlFreeHeap](#rtlfreeheap)
  - [扩展-为什么返回\*mut u8](#扩展-为什么返回mut-u8)
  - [扩展-内存分配的核心协议：GlobalAlloc Trait](#扩展-内存分配的核心协议globalalloc-trait)
  - [扩展-：堆内存加固与抗扫描（Heap Hardening \& Anti-Scanning）](#扩展-堆内存加固与抗扫描heap-hardening--anti-scanning)
    - [分配时更加隐蔽的方案(未实现)](#分配时更加隐蔽的方案未实现)



# 背景-Rust内存分配

## 内存分配的一般流程

在 Rust 中，内存申请并不是一步到位的，它经过了三个层次的传递   
1. 用户层 (Collection 层)：你调用 Vec::new(), Box::new(), String::push()。
   * 这一层关心的是： “我需要存 10 个整数”
2.  抽象层 (`alloc` crate)：这是 Rust 的官方库.它接收到“存 10个整数”的请求，计算出需要 $10 \times 4 = 40$ 字节，并附带对齐要求
    * 这一层关心的是： `Layout`（布局）。它会调用GlobalAlloc::alloc(layout)
3. 硬件/系统层 (Allocator 层)：这就是我们要写的 allocator.rs。它接收到Layout，调用 Windows API（如 RtlAllocateHeap）从内核手里抠出 40字节的真实物理内存
   * 这一层关心的是： `*mut u8`（原始指针）

`Vec` (想要内存) -> `Layout` (计算尺寸和对齐) -> `WinHeap` (你的代码) ->`RtlAllocateHeap` (Windows 内核) -> 返回指针
 
## 简单的例子

以`let my_vec: Vec<u32> = Vec::with_capacity(2);`为例:  
1. Vec 内部逻辑:需要两个u32,每个u324字节,共8字节(size),对齐:u32要求4字节对齐
2. 中间件 — alloc 库与 Layout,Vec 自己不去要内存，它找 alloc 库帮忙:alloc 库构造了一个 `Layout` 对象
    * Layout { size: 8, align: 4 }
    * alloc库调用rust编译器在链接阶段定好的全局分配器入口(指令:ALLOCATOR.alloc(layout))
3. 执行层 — 你的 WinHeap (重构的核心)
```rust
unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    // 1. 拆解蓝图
    let size = layout.size(); // 拿到 8

    // 2. 准备工具：获取堆句柄
    // 为什么要句柄？因为 Windows 有很多堆，你得告诉内核你想在哪个堆里挖地。
    let h_heap = GetProcessHeap();

    // 3. 发起请求
    // 我们在这里真正执行底层调用。
    let ptr = RtlAllocateHeap(h_heap, 0, size);

    // 4. 交货
    ptr as *mut u8
}
```
4. 底层控制 — RtlAllocateHeap (Windows 内核),在第3步执行到RtAllocateHeap时,控制权转到ntdll.dll.此时堆管理器检查空闲列表,找到一块8字节空闲地址,并把这个8字节的地址标记为已占用.虽然u32是4字节对齐,但win默认给8或16字节对齐的地址.最后返回一个十六进制的地址,如 0x000001A2B3C4D5E0

5. 反馈回路,WinHeap把这个地址传回alloc库,alloc库把这个地址包进RawVec,Vec拿到地址并返回

* 为什么用RtlAllicateHeap?
    * 相比malloc(c运行库)或HeapAlloc(Kernel32)
    * Rtl是Runtime Librart缩写.**ntdll.dll是用户态最底层DLL**,据大多数AV的Hook是kernel32!HeapAlloc,这样能过绕过一层监控

* layout.size()是0的情况
    * Rust规定,分配器可以返回一个特殊的非空指针,或者null
    * dinvk中,如果size是0,直接返回null_mut(),繁殖后续代码误操作


## 其他需要注意的

### 堆隔离-Heap Isolation

Windows允许创建一个私有的堆（HeapCreate）。如果你把 Shellcode放在一个全新的私有堆里，而不是系统默认堆，可以避开某些针对默认堆的内存扫描。

### 内存惩罚 (Zero-fill on Dealloc)

dealloc时手动写0。这是为了反取证。在高级取证中，内存释放后数据并不会立即消失，黑客工具必须在还给系统前把痕迹抹干净。

### 递归分配风险

我们在 GlobalAlloc内部绝对不能调用任何会触发内存分配的东西（比如打印日志到String）。否则：alloc -> log -> alloc -> log…… 瞬间爆栈。


# 源码

**在no_std环境下,为了使用依赖堆内存的Vec\String\HashMap等,必须提供自定义内存分配.其流程如下:(以dinvk的allocator.rs为例)**  
1. 契约绑定：实现 GlobalAlloc Trait,这是最关键的一步。Rust 编译器不关心你用什么方法要内存，它只认 GlobalAlloc协议
* 你定义的 pub struct WinHeap; 就像是一个内存分配器的身份声明
* 通过 unsafe impl GlobalAlloc forWinHeap，你正式告诉编译器：“以后所有关于堆内存的操作，都请转交给这个结构体的方法来处理”

2. 资源寻址：获取堆句柄 (HANDLE),通过`fn get(&self) -> HANDLE { GetProcessHeap() }`
* 背景：Windows进程启动时，内核会分配一个默认堆。所有的内存操作都需要这把“钥匙”（句柄）
* 实现：通过 self.get() 获取这个句柄。在你的项目中，这个句柄是通过 winapis模块获取的（通常是读取 PEB 里的 ProcessHeap 字段）

3. 底层链接：与 ntdll.dll 握手
* dinvk使用了windows-targets这个crate.puerto中使用的是unsafe  extern "system".这两种方式都是声明外部函数的
* 仍然不安全,可能在IAT中留下记录,应使用module.rs中的功能找到系统中分配内存的函数地址在执行内存分配

**当在项目中发生内存分配时会:**  
当在项目中（或者 Rust 的 alloc库）调用这个分配器时  

* 内存申请阶段 (alloc 被触发),当代码执行 let v = Vec::with_capacity(10); 时：
   1. 计算尺寸：alloc 库根据 u8 类型和长度 10 算出 Layout（大小为 10 字节）。
   2. 句柄获取：调用 self.get()，拿到进程默认堆的地址。
   3. 零大小检查：代码中有一个 if size == 0 { return null_mut();}。这是为了防止向 Windows 堆管理器申请 0字节导致的未定义行为，保护了程序的健壮性。
   4. 内核调用：执行 RtlAllocateHeap。
   5. 返回地址：Windows 从堆中划出一块地，返回起始地址给 Rust，此时 Vec就拥有了真实的物理内存。


* 内存释放阶段 (dealloc 被触发),当 v 离开作用域（Drop）时：
   1. 空指针判定：if ptr.is_null() { return; } 确保不会释放一个空地址。
   2. 【关键】红队反取证动作：  
    1. unsafe { core::ptr::write_bytes(ptr, 0, layout.size()) };
       * 功能：在把内存还给系统之前，用 0 彻底抹除这块区域的内容。
       * 意义：即使 EDR或取证工具在随后扫描内存，也无法通过残留数据分析出你刚才在这里存过什么敏感信息（如 API Hash 或 Shellcode）。

* 正式释放：调用 RtlFreeHeap，内存块回归系统池。

## winapis::GetProcessHeap

```rust
#[inline(always)]
pub fn GetProcessHeap() -> HANDLE {
    let peb = NtCurrentPeb();
    (unsafe { *peb }).ProcessHeap
}
```

* #[inline]  和  #[inline(always)]
    * 这两个属性都是告知编译器,把后面的函数内容直接嵌入到调用的地方,不要产生真正的函数调用(Call指令).对于只有两三行或功能很简单的函数,其调用的开销(压栈\跳转\返回)比函数体本身都大
    * #[inline]知识一个建议,如果编译器根据函数大小\复杂度\调用频率评判,觉得内联后反而会让代码变慢\体积太大,会拒绝内联
    * 它的主要作用是允许函数在不同的crate之间进行内联(默认不允许)
    * #[inline(always)],强制内联指令,除非遇到物理极限(比如递归函数无法展开)
    * OpSec(免杀),内联可以打碎函数指纹,如果GetProcessHeap是一个独立函数,EDR能很容易的在这个函数入口打补丁/监控,但使用内联可以揉碎进入其他代码,变成几条散乱指令,增加逆向分析的难度

#### PEB中的ProcessHeap字段

是 Windows 进程管理的基石之一

1. 如何产生?当一个进程启动(ntdll!LdrpInitializeProcess 阶段),win内核会自动为该进程创建一个默认内存池(Default Process Heap)
    * 内核创建好堆后,会将这个堆的地址(Handle)记录在PEB结构体的ProcessHeap字段中
2. ProcessHeap字段背后的原理
    * 句柄的本质:在ProcessHeap中,HANDLE就是一个内存指针,指向堆管理器的核心控制块(Heap Header)
    * 唯一性:一个进程只有一个默认堆,虽然可通过HeapCreate再建新的堆,但PEB.ProcessHeap永远指向哪个最初的系统分配的堆
3. 免杀性
    * 标准API调用链: 你的程序->kernel32!GetProcessHeap ->ntdll!RtlGetCurrentPeb -> 读取 PEB
    * 本项目调用链:你的程序 -> gs:[0x60] -> 直接读 PEB
      * 优势:零依赖,不需要导入Win api就能拿到句柄
      * 优势:完全绕过HooK,如果AV/EDR在kernel32!GetProcessHeap入口打了个补丁监控谁在分配内存，你的代码直接从 CPU 寄存器（GS）里拿数据，杀软根本察觉不到你
4. 风险
    * 共享风险：因为这是“默认堆”，进程中的其他模块（比如你引用的某些库，甚至是系统组件）也会在这个堆里分配内存
    * 锁定机制：当你调用 RtlAllocateHeap 操作这个句柄时，Windows内部会加一个临界区锁。这意味着如果多线程竞争激烈，性能会稍微受影响。但这正是为了保证内存分配的安全性。

以上,(unsafe { *peb }).ProcessHeap,跳过了（Kernel32），直接在（PEB）找到了（堆内存）地址

## RtlAllocateHeap 

```rust
unsafe  extern "system"{
    fn RtlFreeHeap(heap: HANDLE, flags: u32, ptr: *mut c_void) -> i8;
    fn RtlAllocateHeap(heap: HANDLE, flags: u32, size: usize) -> *mut c_void;
}
```

参见winapi.md中的讲解

## RtlFreeHeap



## 扩展-为什么返回*mut u8

理解了为什么要用 u8，你就能理解Rust 编译器是如何看待“原始内存”的.u8是rust中表示高级数据结构的基本单位

1. u8 代表“字节”，它是内存的最小度量单位
在计算机底层：  
   * 内存本身不区分类型。它只是一串连续的、由 0 和 1 组成的比特流。
   * 1 字节 (8位) 是 CPU 寻址和处理数据的最小单位。

所以： 当 alloc 返回一个指针时，它返回的是一个“不知道具体存什么，只知道占用了多少空间” 的起始地址。在 Rust
中，表达“原始的、无类型的字节序列”，最合适的类型就是 u8

2. 指针算术 (Pointer Arithmetic)：
    * 在 Rust 中，你不能对 *mut c_void进行加减运算（因为它的大小是未知的）。
    * 但你可以对 *mut u8 进行偏移。例如 ptr.add(1) 代表向后移动 1字节。这非常符合内存管理的直觉。

3. Rust 类型系统的约定：
    * Rust 的标准库 (core::alloc) 明确规定了 GlobalAlloc 的接口必须返回*mut u8。这是为了让上层（比如 Vec 或Box）拿到指针后，能方便地将其强制转换（cast）为它们需要的类型（比如u32, String 等）

## 扩展-内存分配的核心协议：GlobalAlloc Trait

GlobalAlloc 是 Rust core 库定义的一个
接口协议（Trait）。要定义一个合格的分配器，你必须保证能实现以下两个核心方法：
1. unsafe fn alloc(&self, layout: Layout) -> *mut u8
    * 参数 `Layout`：它包含两个核心信息：
    1. size：要多少字节
    2. align：对齐要求（比如必须是 8 的倍数地址）。这是 Rust内存安全的重要保障，某些 CPU 指令如果操作了不对齐的地址会直接崩溃
    * 返回值 `*mut u8`：返回分配好的内存起始地址。如果失败，返回空指针（null）

* 实际使用中,这里的layout并不能显示的传入,只要实现了GlobalAlloc trait,并标记`#[glob_allocator]`就向rustc声明,只要程序需要堆内存,就由我来分配
  * 关于layout.size()的大小,由rustc根据需要堆内存的消费者(vec/String等)在底层自动构造并传递给自定义的分配器
    * 如`let v = Vec::<u32>::with_capacity(10);` ,rustc知道这里需要10个u32,一个u32是8字节,同时u32是4字节对齐,那么这里就是40个字节大小的堆内存
  * 通过alloc库构造一个core::alloc::Layout实例,这个实例内部字段size=40,align=4
  * rustc自动插入ALLOCATOR.alloc(layout),实现隐式调用

* OpSec的意义(这个过程是隐式的，所以它非常隐蔽)
  *  代码里只是正常地用 Vec 或 String
  *  在底层，每一次内存分配都在悄悄地执行你写的 RtlAllocateHeap 逻辑
  *  如果在 fn alloc中对分配的内存进行简单的异或混淆,程序里所有的String 在内存里都会是处于“加密”状态的
  *  [扩展-：堆内存加固与抗扫描（Heap Hardening \& Anti-Scanning）](#扩展-堆内存加固与抗扫描heap-hardening--anti-scanning)



2. unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout)
    * 职责：释放内存
    * Rust 极其严谨，它在释放内存时不仅给你指针，还把当初申请时的Layout原封不动地还给你。这让分配器可以做一些校验（比如确认释放的大小是否正确

定义好 GlobalAlloc还不够，你还得告诉编译器：“以后全家人的内存申请都找我！”这就是 #[global_allocator] 静态变量的作用。
* 它是全局唯一的
* 编译器在编译整个程序时，会把所有的 alloc调用链接到这个静态变量指向的方法上


## 扩展-：堆内存加固与抗扫描（Heap Hardening & Anti-Scanning）

虽然 Rust 的 alloc 函数本身只负责“划地盘”（返回指针），并不直接接触数据，但因为你控制了整个内存分配的“出入口”，你确实可以在这里通过一些“小动作”来极大地迷惑 EDR 和内存取证工具。

以下是三种实战方案：

**方案 1：内存“自毁”与毒化（Anti-Forensics / Poisoning）**

这是最基础也最实用的免杀技巧。EDR 的扫描器经常会扫描那些已经“释放”但尚未被覆盖的内存残留。

在 allocator.rs 中：
* 分配时 (Alloc)：强制将内存填充为 0x00 或随机数，确保不会有旧数据的残留.不建议随机数或0x00000008,这会留下明显的指纹,建议保留d中的源码0。
* 释放时 (Dealloc)：这才是最关键的。在把内存还给系统之前，先用垃圾数据覆盖它。

```rust
unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    if ptr.is_null() { return; }

    // 【加点料】：在释放前，先用 0xAA 覆盖整块内存
    // 这样内存取证工具即便抓到了这块内存，也只能看到一堆 0xAA，看不到曾经存过的敏感字符串
    core::ptr::write_bytes(ptr, 0xAA, layout.size());

    // 真正归还给 Windows
    RtlFreeHeap(self.get(), 0, ptr.cast());
}
```

---

方案 2：堆布局随机化（Heap Layout Randomization）

EDR 扫描内存时通常寻找特定的“结构特征”。比如 Vec 的数据通常是连续的。如果你能打乱这个连续性，扫描器就会失效。

思路：在分配内存时，故意多申请一点点空间（Padding），然后返回一个带有随机偏移的指针。

```rust
unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    let original_size = layout.size();
    // 【加点料】：多申请 8 个字节
    let padded_size = original_size + 8;

    let ptr = RtlAllocateHeap(self.get(), 0, padded_size) as *mut u8;

    // 返回偏移后的指针（例如偏移 4 字节）
    // 这会让内存里的数据分布看起来非常奇怪，破坏了固定的特征码偏移
    if !ptr.is_null() {
        ptr.add(4)
    } else {
        ptr
    }
}
```
注：这种做法需要你在 dealloc 时同样处理偏移，否则会崩溃。

---

方案 3：透明内存混淆（真正的“加密”思路）

如果你想让 String 在内存里始终是“加密”的，光靠 alloc 是不够的，因为 alloc 不知道什么时候数据会被写进去。

但你可以配合 Rust 的 Wrapper 类型实现“透明混淆”：

你可以定义一个特殊的类型 ObfuscatedString，它的底层数据依然存在你手写的 WinHeap 中，但在读写时自动异或。

进阶技巧：休眠时加密整个堆（Heap Encryption on Sleep）
这是一个顶级红队技巧（如 Cobalt Strike 的 SleepMask 插件）：
1. 挂钩分配器：记录所有活跃的内存块地址。
2. 进入休眠前：遍历所有记录的内存块，全部异或（XOR）一遍。此时，你的整个堆内存里全是乱码，EDR 扫描不到任何明文。
3. 唤醒后：再异或回来，程序继续正常运行。

---

为什么这对 puerto 极其重要？

在 puerto 这种重构项目中，如果你只是简单地复现 dinvk，那它的内存特征依然是明显的。

但如果你在 allocator.rs 里加入了上述逻辑：
* EDR 在扫描你的进程内存时：发现里面全是毫无意义的随机数或 0xAA。
* 内存审计工具：无法提取出你的 C2 地址、解密后的 Shellcode 或 API 名字，因为它们在不用的时候都被“毒化”或“混淆”了。

总结
你对“加点料”的直觉非常正确。分配器是整个程序的“咽喉”。
通过在 allocator.rs 中加入释放前擦除（Wipe on Free）和随机填充（Random Padding），你不需要修改业务逻辑，就能让整个程序的隐蔽性提升一个量级。

这也是为什么我在之前的建议中说，手写分配器不仅仅是为了 no_std，更是为了获得对内存的绝对控制权。
### 分配时更加隐蔽的方案(未实现)

该方案被称为“幽灵分配器（Ghost Allocator）”：核心在于弃用 Windows官方堆管理器，转而通过间接系统调用（Indirect Syscall）执行NtAllocateVirtualMemory 申请原始匿名内存页，从而彻底绕过 PEB 中ProcessHeaps 列表的枚举审查；在此隔离空间内，利用 Rust手写一套支持对齐安全的内存管理算法（如 Bump 分配器）来承载 no_std环境下的动态分配需求，并配合随机偏移填充（Padding）与“释放即销毁”的内存毒化机制，旨在消除 API 调用指纹并打乱数据的物理形状，实现针对 EDR内存扫描与取证分析的极高强度隐蔽性，但其成功的关键在于对 CPU 硬件对齐限制和内存生命周期的极精确控制