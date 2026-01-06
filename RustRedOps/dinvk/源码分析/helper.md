# helper.rs

本文件封装了对 Windows PE（Portable Executable）文件格式的解析逻辑

module.rs 是“如何利用 PE 结构找到函数”  
helper.rs 是“如何从内存中读懂 PE 结构本身”。能让你彻底理解 DOS 头、NT头、节表（Section Table）和导出表在 Rust 中是如何被抽象和操作的。

这段代码的核心逻辑就是：Base + Offset (RVA),它像拼图一样：  

   1. 拿到 Base。
   2. Base -> DOS Header -> 拿到 NT Header Offset
   3. Base + NT Header Offset -> NT Header -> 拿到 Export RVA
   4. Base + Export RVA -> Export Directory -> 拿到三个核心数组的 RVA

## BTreeMap：Rust 中有序映射的底层利器

`BTreeMap` 是 Rust 标准库（以及 `alloc` 库）中提供的一种基于 **B-Tree** 数据结构实现的有序键值映射（Key-Value Map）。在你的项目 `src/helper.rs` 中，它被用于存储函数地址到名称的映射：

### BTreeMap 结构体定义

```rust
pub struct BTreeMap<K, V, A = Global>
where
    A: Allocator + Clone,
{
    root: Option<NodeRef<Owned, K, V, LeafOrInternal>>,//树的根节点,树可能为空。如果为空，root 为 None
    length: usize,//当前 Map 中元素的总数量,缓存了元素的个数，使得 len() 方法的时间复杂度为$O(1)$，而不需要每次都遍历整棵树去数
    pub(super) alloc: ManuallyDrop<A>,//内存分配器工具，用于申请和释放节点内存.被 ManuallyDrop 包裹，防止提前释放
    _marker: PhantomData<Box<(K, V), A>>,
}
```

* K :键的类型  
在 BTreeMap 中，K 必须实现 Ord trait（全序比较）,因为 B-Tree是有序的数据结构，需要比较键的大小来决定存储位置

* `V` (Value): 值的类型  
存储在 Map 中的数据

* `A` (Allocator): 分配器类型  

默认值: Global。如果不指定，默认使用全局内存分配器（通常是系统malloc）  
where A: Allocator + Clone。分配器必须实现 Allocatortrait（定义了分配和释放内存的方法）并且是可克隆的（Clone）  
允许用户自定义内存分配策略（例如使用 arena分配器、特定于线程的分配器等）。这在系统编程和嵌入式开发中非常重要

```rust
pub type Functions<'a> = BTreeMap<usize, &'a str>;
```

这行代码定义了一个从内存地址（`usize`）到函数名（`&str`）的有序映射。以下从特性、与 `HashMap` 的对比、API 用法及适用场景等方面详细解析 `BTreeMap`。

---

### 1. 核心特性：有序性（Ordered）

这是 `BTreeMap` 与 `HashMap` 最本质的区别。

* **`HashMap`**：  
  内部无序。遍历时元素顺序由哈希函数、桶布局和插入历史决定，**不可预测且每次运行可能不同**。

* **`BTreeMap`**：  
  始终按键（Key）的**升序**存储和遍历。例如：

  ```rust
  map.insert(0x1000, "FuncA");
  map.insert(0x0500, "FuncB");
  map.insert(0x2000, "FuncC");
  // 遍历时顺序一定是：0x0500 → 0x1000 → 0x2000
  ```

#### 在本项目中的意义

在 PE 文件分析中，导出函数的内存地址（RVA/VA）天然具有空间局部性。按地址排序后：

* 便于调试和人工阅读（如打印符号表）
* 支持高效判断“某个地址落在哪个函数范围内”
* 为后续实现 **地址范围查询**（如反汇编或堆栈回溯）奠定基础

若使用 `HashMap`，输出将杂乱无章，丧失结构化信息的价值。

---

### 2. 数据结构：B-Tree 的优势

* **结构**：  
  `BTreeMap` 基于 **自平衡的 B-Tree**（非二叉树），每个节点可存储多个键值对，高度较低。

* **时间复杂度**：
  * 查找、插入、删除：**O(log N)**
  * 虽略慢于 `HashMap` 的理论平均 **O(1)**，但在实际场景（如系统 DLL 的几千个导出函数）中，性能差距微乎其微。

* **缓存友好性**：  
  B-Tree 的节点通常填满一个 CPU 缓存行（Cache Line），相比红黑树等二叉结构，**减少内存跳转次数**，提升实际访问速度。

---

### 3. API 与典型用法

`BTreeMap` 的接口与 `HashMap` 高度相似，但额外提供**范围查询**能力：

```rust
use alloc::collections::BTreeMap;

let mut map = BTreeMap::new();

// 插入（Key 必须实现 Ord）
map.insert(0x1000, "LoadLibraryA");
map.insert(0x2000, "GetProcAddress");

// 查找
if let Some(name) = map.get(&0x1000) {
    println!("Found: {}", name);
}

// 遍历（保证按键升序）
for (addr, name) in &map {
    println!("0x{:x} -> {}", addr, name);
}

// 范围查询（BTreeMap 独有！）
// 查找地址在 [0x1000, 0x1500) 之间的所有函数
for (addr, name) in map.range(0x1000..0x1500) {
    // ...
}
```

> 💡 **范围查询**（`.range()`）是 `BTreeMap` 的“杀手级功能”，在内存分析、符号解析、区间覆盖检测等场景中极为强大。

---

### 4. 为何在 `dinvk` 项目中选用 `BTreeMap`？

在 `src/helper.rs` 中选择 `BTreeMap` 而非 `HashMap`，主要基于以下关键考量：

#### ✅ **no_std 环境友好**

* `HashMap` 在标准库中依赖 **随机化哈希种子**（Hash Randomization）以防御 HashDoS 攻击。
* 在 `no_std`（无标准库）环境下，获取安全随机源困难，且 `alloc` 中的 `HashMap` 通常需要手动指定 `Hasher` 或引入第三方库（如 `hashbrown`）。
* `BTreeMap` **完全不依赖随机数**，仅要求 Key 实现 `Ord` trait（`usize` 天然满足），在 `alloc` 中开箱即用。

#### ✅ **行为确定性**（Determinism）

* 安全工具、加载器或分析器必须保证**可重复的行为**。
* `BTreeMap` 的遍历和查询结果始终一致；而 `HashMap` 因哈希种子随机化，每次运行顺序可能不同，不利于调试和日志比对。

#### ✅ **符合地址空间语义**

* 内存地址是天然有序的标量。按地址排序不仅符合直觉，也为后续实现**地址区间分析**（如判断某 RVA 属于哪个节或函数）提供结构化基础。

---

### 总结

`BTreeMap` 是一个**有序、确定、缓存友好**的键值容器，在 Rust 的底层开发（尤其是 `no_std` 场景）中具有不可替代的优势。  
在 `dinvk` 这类红队或系统工具项目中，它不仅是 `HashMap` 的可行替代品，更是**契合内存分析语义的更优选择**——既能规避 `no_std` 下的哈希依赖问题，又能提供稳定的排序与强大的范围查询能力，完美支撑 PE 解析、符号映射和地址追踪等核心功能。

## use core::{ffi::{c_void, CStr}

### c_void

c_void是一个非常特殊的类型,用于FFI（Foreign Function Interface，外部函数接口），即 Rust 与 C交互的场景  
可以把它理解为 C 语言中 `void` 类型的 Rust 等价物

1. 它的核心用途：表示“未知类型”的指针  
在 C 语言中，void*表示一个“通用指针”，它可以指向内存中的任何数据，但编译器并不知道该数据的具体类型或大小。  
在 Rust 中：
   * *mut c_void 对应 C 的 void*（可变通用指针）。
   * *const c_void 对应 C 的 const void*（只读通用指针）。

 在你的项目（dinvk）中：  
你经常看到 get_module_address 或 get_proc_address 返回 *mut c_void。这是因为：

* 一个 DLL的基地址（HMODULE）本质上只是内存中的一个起始点，它可能包含各种结构，所以用“通用指针”表示最合适。
* 一个函数的地址，在运行前我们不知道它的具体签名（参数和返回值），所以先用c_void 指针接住，等真正要调用时再强转为具体的函数指针。

1. 为什么不用 Rust 的单元类型 ()？

初学者可能会想，Rust 的 () 也是空，能不能用 *mut ()？

* 含义不同：() 在 Rust 中表示“什么都没有”，它的尺寸（size）是 0。
* FFI 兼容性：c_void 是专门为兼容 C语言定义的。它在底层被定义为一个特殊的枚举或结构，确保在跨语言传递指针时，行为与 C 的 void* 完全一致。

 1. c_void 的特性

* 不可实例化：你不能在 Rust 里创建一个 c_void 类型的变量（比如 let x: c_void = ... 是不行的）。它只能存在于指针后面。即无法在内存中创建一个类型为c_void的值,你不能拥有一个 c_void对象，但你可以拥有一个指向它的地址（即指针）。因为指针本身（地址数值）在 64位系统上固定占用 8 字节，它不需要知道目标对象的大小
* 不可直接解引用：你不能对 *mut c_void 进行解引用（*ptr），因为编译器不知道解引用后该读几个字节。
* 必须强转：你必须先把它转换成具体类型的指针（如 *const IMAGE_DOS_HEADER），然后才能读取数据。

```rust
use core::ffi::c_void;

fn main() {
    // ❌ 报错：无法创建 c_void 变量
    // 因为 c_void 内部没有定义任何可以让你初始化的成员
    // 它在 Rust 中通常被实现为一个“空枚举”或者“不透明结构体”
    let x: c_void = c_void; // 错误！c_void 不是一个值

    let my_data: u32 = 42;

    // ✅ 合法：将 u32 指针转为 c_void 指针
    // 这里我们只是在操作地址，而不是在创建 c_void 实例
    //&my_data对变量my_data的安全引用(rust中引用的生命周期受编译器严格监控)
    let ptr: *const c_void = &my_data as *const u32 as *const c_void;

    println!("数据的内存地址是: {:?}", ptr);
}
```

### let ptr: *const c_void = &my_data as*const u32 as *const c_void

涉及“引用”、“类型”和“裸指针”这三个概念

在 Rust 与系统底层（如 Windows API 或 PE 内存操作）交互时，经常需要将一个普通变量的地址转换为 `*const c_void` 类型——这是 C 风格“通用指针”的 Rust 表示。这种看似简单的类型转换背后，蕴含着 Rust 安全模型与不安全世界之间的明确边界。我们可以从三个层面理解这一过程：

**在 Rust 中，*const u32(`*mut u32`) 是一个整体，表示一种类型，就像 i32 或者 String一样**

---

#### 1. `&my_data` 是什么？——安全引用的本质

`&my_data` 是一个 **Rust 安全引用**（safe reference），虽然**其底层确实对应一个内存地址**，但在编译器眼中，它远不止是一个数字：

* **带有生命周期**（Lifetime）  
  编译器静态追踪该引用的有效范围，确保它不会悬空（dangling）。例如，引用不能比它所指向的数据活得更久。

* **受借用规则约束**（Borrowing Rules）  
  如果存在一个 `&my_data`（不可变借用），则在同一作用域内不允许创建 `&mut my_data`（可变借用），从而防止数据竞争。

* **保证非空**（Non-null Guarantee）  
  Rust 的引用永远不能为 `null`。这是与 C 指针的根本区别之一，也是内存安全的重要基石。

> ✅ 因此，`&my_data` 属于 **Safe Rust** 的范畴，受编译器全程保护。

---

#### 2. `*const u32` 是什么？——裸指针的特性

`*const u32` 是一种 **裸指针**（raw pointer），属于 **Unsafe Rust**：

* **`*`**：表示这是一个裸指针（类似 C 中的 `int*`）。
* **`const`**：表示只读（不可通过此指针修改数据）；对应的可写版本是 `*mut u32`。
* **`u32`**：保留类型信息，表明该指针预期指向一个 `u32` 大小的、可解释为无符号整数的内存区域。

> ⚠️ 裸指针：
>
> * 可以为 `null`
> * 不受生命周期或借用检查约束
> * 不能直接解引用（除非在 `unsafe` 块中）
> * 本质上只是一个地址（`usize`），但附带了“如何解释该地址内容”的类型提示

---

#### 3. 为什么要写成 `&my_data as *const u32 as *const c_void`？

这种两步转换并非冗余，而是 Rust 类型系统对“安全降级”过程的**显式要求**。

##### A. 从“安全世界”跨越到“不安全世界”

* C 函数（如 Windows API）接收的是 `void*`，即 `*const c_void`。
* 但 Rust 不允许直接将受保护的引用“悄悄”变成原始指针，因为这会绕过安全机制。
* 使用 `as` 进行强制转换，是程序员**主动声明放弃安全保证**的行为，必须显式写出。

##### B. 语法规则：分两步“脱壳”

Rust 的类型转换规则要求类型转换路径清晰、可验证：

* ❌ **不允许直接转换**：  

  ```rust
  &my_data as *const c_void  // 通常不被允许或不符合惯用法
  ```

  因为这跳过了中间的“有类型裸指针”阶段，模糊了类型信息的丢失过程。

* ✅ **标准两步法**：

  ```rust
  &my_data as *const u32 as *const c_void
  ```

  **第一步**：`&my_data as *const u32`  
  → 将安全引用降级为**同类型的裸指针**。  
  → 此时仍保留“这是 `u32` 数据”的语义，仅移除了生命周期和借用检查。

  **第二步**：`*const u32 as *const c_void`  
  → 将有类型裸指针转为**无类型通用指针**。  
  → 这一步模拟了 C 语言中“任何指针都能隐式转为 `void*`”的行为，但在 Rust 中必须显式写出。

> 🔍 这种设计体现了 Rust 的哲学：**安全降级必须显式、渐进、可审计**。

---

#### 总结

`&my_data as *const u32 as *const c_void` 的写法，本质上是一次**受控的“安全剥离”**：

1. 先从受保护的引用进入裸指针世界（保留类型）；
2. 再从具体类型泛化为通用指针（丢弃类型）。

在 `dinvk` 这类项目中，由于需要频繁与 Windows 加载器、PE 结构、系统调用等**不受 Rust 控制的内存区域**交互，这种转换成为常态。理解其背后的类型系统逻辑，不仅能写出正确代码，更能避免在 `unsafe` 代码中引入隐蔽的内存错误。
