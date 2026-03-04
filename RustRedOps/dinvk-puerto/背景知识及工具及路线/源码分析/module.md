- [注意](#注意)
  - [此项目找DLL 基址的原理](#此项目找dll-基址的原理)
    - [本项目没有定义一个把所有字段打包在一起的单一 `PE`结构体？](#本项目没有定义一个把所有字段打包在一起的单一-pe结构体)
      - [dinvk 的做法：通过指针和偏移量访问](#dinvk-的做法通过指针和偏移量访问)
    - [PE (Memory Layout)](#pe-memory-layout)
  - [源码](#源码)
    - [CStr::from\_ptr(ptr).to\_string\_lossy().into\_owned()](#cstrfrom_ptrptrto_string_lossyinto_owned)
      - [1. `CStr::from_ptr(ptr)`：封装原始指针，创建安全视图](#1-cstrfrom_ptrptr封装原始指针创建安全视图)
      - [2. `.to_string_lossy()`：容错式 UTF-8 转换](#2-to_string_lossy容错式-utf-8-转换)
      - [3. `.into_owned()`：夺取所有权，实现深拷贝](#3-into_owned夺取所有权实现深拷贝)
  - [to\_string\_lossy() 是一种“懒惰”的转换，只有在必要时（遇到无效UTF-8）才分配内存。加上.into\_owned() 是为了确保你最终拿到的一定是一个独立的、拥有所有权的 String 对象](#to_string_lossy-是一种懒惰的转换只有在必要时遇到无效utf-8才分配内存加上into_owned-是为了确保你最终拿到的一定是一个独立的拥有所有权的-string-对象)
      - [在 `dinvk` 项目中的具体意义](#在-dinvk-项目中的具体意义)
      - [一句话总结](#一句话总结)
    - [CStr::from\_ptr((h\_module + names\[i\] as usize) as \*const i8).to\_str().unwrap\_or("")](#cstrfrom_ptrh_module--namesi-as-usize-as-const-i8to_strunwrap_or)
      - [性能与生命周期驱动的设计：为何在循环中使用 `to_str()` 而非 `to_string_lossy().into_owned()`](#性能与生命周期驱动的设计为何在循环中使用-to_str-而非-to_string_lossyinto_owned)
      - [1. **性能差异：避免在循环中产生大量临时内存分配**](#1-性能差异避免在循环中产生大量临时内存分配)
      - [2. **生命周期与使用场景的匹配**](#2-生命周期与使用场景的匹配)
      - [3. **容错策略的差异化选择**](#3-容错策略的差异化选择)
      - [总结](#总结)
    - [get\_function\_address](#get_function_address)

# 注意

请记得所有关于内存及windows可执行文件的操作,都可以用windbg查看在内存中的直观显示.  

请记住,一定要使用windbg,这甚至比写代码更加重要

这里只突出dinvk项目中moudle.rs文件的逻辑\rust语法相关内容\源码解析,关于windows基础知识在PEB and PE中

## 此项目找DLL 基址的原理

本项目中:  
TEB->PEB->LDR(裸指针)->PEB_LDR_DATA(存储多个PE文件)->InMemoryOrderModuleList(双向链表,指向不同的PE文件)->LDR_DATA_TABLE_ENTRY(具体的一个PE文件)->BaseDllName(模块名称,hash对比要找的模块,若对)->DllBase(模块基址,也是该PE文件的dos header,(*data_table_entry).Reserved2[0])

```html

    A[TEB (Thread Environment Block)] -->|Gs:[0x60]| B(PEB (Process Environment Block))
    B -->|.Ldr| C(PEB_LDR_DATA)
    C -->|.InMemoryOrderModuleList| D{LIST_ENTRY (Double Linked List)}
    D -->|.Flink| E[LDR_DATA_TABLE_ENTRY (Shifted View)]

    subgraph "Shifted LDR_DATA_TABLE_ENTRY Access"
        E -- "Offset +0x48 (Code Logic)" --> F("BaseDllName (Offset 0x58 in Real Struct)")
        E -- "Offset +0x20 (Code Logic)" --> G("DllBase (Offset 0x30 in Real Struct)")
    end

    F -->|Compare Name| H{Match?}
    H -- Yes --> G
    H -- No --> D
    G --> I[Return Module Base Address]
```

在本项目中，调用系统调用（syscall）或关键 API 的过程是纯手动的，不依赖操作系统提供的标准加载器（Loader）功能。  

1. 定义数据结构（Mapping）：
      Rust 代码在 src/types.rs 中使用 #[repr(C)] 精确复刻了 Windows PE
  文件的内存布局。这意味着 Rust 结构体的内存分布与 Windows
  内核和硬件看到的二进制数据完全一致。

   1. 获取基址（Base Address）：
       DllBase 的数值在内存中指向的位置正是该 DLL 的PE文件的 DOS 头 (`IMAGE_DOS_HEADER`)，它是整个 PE 结构的开头

   2. 解析 PE 结构（Parsing）：
      利用第 1 步定义的结构体，代码将这个基址强转为 *const
  IMAGE_DOS_HEADER 指针，通过 e_lfanew 找到 IMAGE_NT_HEADERS，再访问
  OptionalHeader.DataDirectory 找到 导出表 (Export Directory)。

   1. 查找函数（Resolution）：
      遍历导出表中的函数名称数组（AddressOfNames），找到目标函数（例如NtAllocateVirtualMemory 或 LoadLibraryA）。

   2. 获取地址或 SSN（Extraction）：
       - 对于 API
         调用：从导出表中获取该函数的内存地址，将其强转为函数指针（如
         LoadLibraryAFn）并直接调用。
       - 对于 Syscall：解析 ntdll.dll 中对应函数的汇编代码（通常是 mov eax, SSN; syscall），提取出 SSN (System Service Number)。

   3. 执行调用（Execution）：
      使用内联汇编（asm!）直接执行 syscall 指令（传入提取出的
  SSN），或者跳转到获取到的 API 函数地址执行。

  核心意义：
  这个过程完全绕过了 Windows 的 GetProcAddress 和 GetModuleHandle 等标准API。这样做使得安全软件（EDR/AV）难以通过通过挂钩（Hook）标准 API来监控你的行为，从而实现隐蔽调用。  
  本项目中使用rust表示了pe文件各个字段的结构,需要调用syscall时,会通过这些定义的数据结构,来获取相关信息  

### 本项目没有定义一个把所有字段打包在一起的单一 `PE`结构体？

主要原因在于：PE 文件在磁盘上的形态（File Alignment）与在内存中的形态（Section Alignment）是不一致的。  
因为数据在加载后位置变了，你无法用一个连续的 Rust/C结构体来“套”住整个内存或磁盘上的 PE 文件

#### dinvk 的做法：通过指针和偏移量访问

正因为不能用一个大结构体表示，dinvk采用的是基于指针的解析方式。这也是操作系统和调试器的标准做法。

没有 struct PE 是因为 PE不是一个静态的连续数据块，而是一个蓝图。根据这个蓝图，文件被“拆散”并“重组”到了内存中。因此，使用多个小的 Header 结构体 + 指针算术是处理 PE 文件的唯一正确方式。

### PE (Memory Layout)

假设内存地址从低到高向下增长：

```text
基地址 (ImageBase) ----> +-----------------------------+
                        |      MS-DOS Header          |  <- "MZ" 头
                        +-----------------------------+
                        |       MS-DOS Stub           |  <- 历史遗留废话
                        +-----------------------------+
ntHeaders 指针 -------->|      PE Signature           |  <- "PE\0\0"
(DOS.e_lfanew)          +-----------------------------+
                        |    IMAGE_FILE_HEADER        |  <- 物理概况 (CPU架构, 节数量)
                        +-----------------------------+
                        | IMAGE_OPTIONAL_HEADER (64)  |  <- 逻辑核心 (OEP, ImageBase)
                        |                             |
                        |   [ Data Directories ]      |  <- *关键数组* (导出表/导入表索引)
                        +-----------------------------+
SectionHeaders 指针 --->|   IMAGE_SECTION_HEADER [0]  |  <- .text 的描述信息
                        |   IMAGE_SECTION_HEADER [1]  |  <- .rdata 的描述信息
                        |   ...                       |
                        +-----------------------------+
                        |         (填充/Padding)      |  <- 对齐间隙
                        +=============================+
                        |        Section .text        |  <- 真正的代码
                        +-----------------------------+
                        |        Section .rdata       |  <- 常量/导入表数据
                        +-----------------------------+
                        |        Section .data        |  <- 全局变量
                        +-----------------------------+
                        |        Section .reloc       |  <- 重定位数据
                        +-----------------------------+
```

## 源码

### CStr::from_ptr(ptr).to_string_lossy().into_owned()

这段代码的核心作用是：**将目标 DLL 内存映射区中的“C 风格字符串（以 null 结尾的原始字节）”安全地转换成 Rust 环境中可用的、所有权独立的 `String` 对象。**

这种转换在红队加载器（如 `dinvk`）中至关重要——既要正确读取 Windows PE 结构中的原始数据，又要避免内存安全问题。我们可以将其拆解为以下三个关键步骤：

---

#### 1. `CStr::from_ptr(ptr)`：封装原始指针，创建安全视图

- **背景**：  
  `ptr` 是一个裸指针（`*const i8`），指向 DLL 内存映像中某个名称字符串（例如 `"kernel32.dll\0"` 或 `"NtAllocateVirtualMemory\0"`）。从裸指针创建了一个临时引用 &CStr，它没有分配内存，也没有产生所有权。

- **作用**：  
  `CStr::from_ptr(ptr)` 告诉 Rust：“从这个地址开始，逐字节读取，直到遇到 `\0` 字节为止”，并将该内存片段包装为一个 `CStr` 类型。

- **关键特性**：  
  - **零拷贝**：`CStr` 仅是对原始内存的**借用（borrow）**，不进行数据复制。
  - **生命周期绑定**：它隐含地依赖于底层内存的有效性——如果 DLL 被卸载或内存被释放，该 `CStr` 将变为悬空引用（dangling pointer）。
  - **安全抽象**：Rust 通过 `CStr` 提供了对 C 字符串的安全访问接口，防止越界读取。

> ✅ 此步建立了对 DLL 内部字符串的**受控视图**，但尚未脱离原始内存的束缚。

---

#### 2. `.to_string_lossy()`：容错式 UTF-8 转换

- **背景**：  
  PE 文件中的函数名和模块名通常使用 **ASCII** 或 **ANSI（如 Windows-1252）** 编码，而 Rust 的 `String` **严格要求合法 UTF-8**。

```rust
pub fn to_string_lossy(&self) -> Cow<'_, str>

pub enum Cow<'a, B>where
    B: ToOwned + ?Sized + 'a,{
    Borrowed(&'a B),
    Owned(<B as ToOwned>::Owned),
}
```

- **作用**：  
  - 尝试将 `CStr` 中的字节序列解释为 UTF-8,返回一个枚举。If the contents of the CStr are valid UTF-8 data, this function will return a Cow::Borrowed with the corresponding &str slice.
  - **“Lossy”（有损）策略**：若遇到非法 UTF-8 序列（例如某些扩展 ASCII 字符），不会 panic，而是用 Unicode 替代字符 ``（U+FFFD）代替无效字节。
  - 返回类型为 `Cow<str>`（Clone-on-Write）：  
    - 如果输入已是合法 UTF-8，可能直接返回 `&str`（零分配）；  
    - 否则，会分配新内存并返回拥有所有权的 `String`。

- 它的返回类型是 Cow<'a, str>。这是一个智能枚举，用来优化内存分配。
       *情况 A (Borrowed): 如果原始的 C 字符串已经是合法的 UTF-8，它直接返回
         Cow::Borrowed(&str)。这是一个借用，没有发生内存分配/拷贝。
       * 情况 B (Owned): 如果原始字符串包含无效的 UTF-8 字符，它会将无效字符替换为
         `，并分配一个新的 String，返回 Cow::Owned(String)`。

- **为何需要？**  
  在红队场景中，你无法控制目标 DLL 的编码细节（尤其是第三方或系统 DLL）。`to_string_lossy()` 提供了**健壮性保障**，避免因个别非标准字符导致整个加载器崩溃。

---

#### 3. `.into_owned()`：夺取所有权，实现深拷贝

- **背景**：  
  `to_string_lossy()` 返回的 `Cow<str>` 可能仍是对 DLL 内存的引用（尤其在纯 ASCII 情况下）.to_string_lossy(),可能返回借用（Borrowed），而在这个代码块结束后，原始的指针引用（或者是借用的生命周,期）可能不再适用，或者你需要一个确定的 String 类型来传值/存储。

- **作用**：  
  强制将字符串内容**深拷贝到 Rust 堆内存中**，返回一个完全独立的 `String` 对象。
  如果是 Borrowed，它会调用 .to_string() 进行拷贝，生成一个新的String（此时才真正获取所有权）。如果是 Owned，它直接取出里面的 String，不进行额外拷贝。

- **安全意义**：  
  - 即便后续 DLL 被 `FreeLibrary` 卸载，或其内存被覆盖/释放，该 `String` 依然有效。
  - 符合 Rust 的**所有权模型**：`dll_name` 变量现在拥有自己的数据，生命周期不再依赖外部模块。

> 🔒 这是实现“内存隔离”的关键一步——让敏感操作（如日志、转发解析、哈希比对）基于**安全副本**进行。
to_string_lossy() 是一种“懒惰”的转换，只有在必要时（遇到无效UTF-8）才分配内存。加上.into_owned() 是为了确保你最终拿到的一定是一个独立的、拥有所有权的 String 对象
---

#### 在 `dinvk` 项目中的具体意义

在 `get_proc_address` 函数中，获取当前 DLL 的名称（`dll_name`）主要用于处理 **函数转发（Export Forwarding）** 场景：

- 某些 DLL（如 `kernel32.dll`）并不直接实现所有导出函数，而是通过导出表中的**转发条目**指示：“请去 `kernelbase.dll!SomeFunction` 找真正的实现”。
- 为了递归解析这类转发，程序必须知道：
  1. 当前正在解析的是哪个 DLL（即 `dll_name`）；
  2. 转发目标的格式（如 `"KERNELBASE.CreateFileW"`）。

因此，`dll_name` 会被传入 `get_forwarded_address` 函数，用于：

- 分割转发字符串（提取目标 DLL 名和函数名）
- 递归加载或查找目标模块
- 最终定位真实函数地址

---

#### 一句话总结

> 这行代码完成了从 **“危险的原始内存字节”** 到 **“安全的、符合 Rust 所有权模型的标准字符串”** 的跨越，既保证了与 Windows PE 结构的兼容性，又杜绝了悬空指针和编码崩溃风险——这是构建可靠、隐蔽加载器的基石之一。

### CStr::from_ptr((h_module + names[i] as usize) as *const i8).to_str().unwrap_or("")

#### 性能与生命周期驱动的设计：为何在循环中使用 `to_str()` 而非 `to_string_lossy().into_owned()`

这段代码在处理 DLL 导出函数名时，对 `dll_name` 和循环中的 `name` 采用了不同的字符串转换策略，主要出于 **性能优化** 和 **生命周期管理** 两方面的深思熟虑：

---

#### 1. **性能差异：避免在循环中产生大量临时内存分配**

- **`dll_name`（循环外）**：  
  整个函数只需解析一次模块名称（如 `"kernel32.dll"`）。即使使用 `to_string_lossy().into_owned()` 进行一次堆内存分配，开销微乎其微，完全可以接受。

- **`name`（循环内）**：  
  此处的 `for` 循环会遍历目标 DLL 的**全部导出函数名**。像 `ntdll.dll` 或 `kernel32.dll` 这类系统 DLL，通常包含 **数千个导出项**。
  - 若对每个 `name` 都调用 `to_string_lossy().into_owned()`，将导致：
    - 每次迭代都进行一次 **堆内存分配**（`malloc`/`HeapAlloc`）
    - 紧接着在作用域结束时 **立即释放**（`free`/`HeapFree`）
    - 数千次无意义的分配/释放不仅浪费 CPU 周期，还可能触发内存碎片或 EDR 对异常堆行为的监控。
  - 而使用 `CStr::from_ptr(...).to_str()` 得到的是一个 `&str`（字符串切片），它**直接指向 DLL 内存映像中的原始字节**，**零拷贝、零分配**，性能极高。

> ✅ 在高频循环中，避免 `String` 是 Rust 高性能编程的基本准则。

---

#### 2. **生命周期与使用场景的匹配**

- **`name` 是临时的**：  
  在循环体内，`name` 仅用于：
  - 与用户传入的目标函数名（`api_name`）进行字符串比较
  - 或计算哈希值用于快速匹配  
  一旦比较完成，该名称就不再需要。因此，一个**临时的、无所有权的 `&str`** 完全满足需求，且生命周期天然受限于当前迭代。

- **`dll_name` 具有延续性**：  
  该变量在主逻辑块结束后，还需作为参数传递给 `get_forwarded_address` 函数（用于处理函数转发）。虽然在当前上下文中 `&str` 也能工作（因为 DLL 内存不会被卸载），但使用 `String` 能：
  - 明确表达“此数据需跨作用域使用”的意图
  - 避免在更复杂的控制流中因引用悬空（dangling reference）引发安全问题
  - 提升代码的可维护性和鲁棒性

---

#### 3. **容错策略的差异化选择**

- **`to_str()`（严格模式）**：  
  - 若函数名包含非法 UTF-8 字节（如某些非标准 ANSI 扩展字符），`to_str()` 会返回 `Err`。
  - 通过 `unwrap_or("")`，异常项会被转为空字符串，从而在后续比较中自然被跳过。
  - **合理性**：在系统 DLL 中，合法的导出函数名几乎总是 ASCII（UTF-8 兼容）。若出现编码错误，极可能是损坏条目或干扰项，直接忽略是安全且高效的选择。

- **`to_string_lossy()`（宽容模式）**：  
  - 对非法字节进行替换（如 `` U+FFFD），保证转换永不失败。
  - 适用于**关键元数据**（如模块名），因为丢失整个 DLL 名可能导致转发解析失败。
  - 作者可能认为：“即使名字带占位符，也比完全无法处理更可取”。

---

#### 总结

在 Rust 底层系统编程中，有一条黄金法则：**“能在循环中用引用（`&str`）就绝不用拥有权对象（`String`）”**。  
此处对 `name` 使用 `to_str()`，正是为了在遍历成百上千个导出函数时，**将 CPU 时间集中在核心逻辑上，而非浪费在堆内存的频繁申请与释放中**。这种设计既体现了对性能的极致追求，也展示了对 Rust 所有权模型和生命周期语义的精准把握——这正是高质量红队工具或系统级加载器的关键特质。

### get_function_address

当你调用 `dinvk::get_function_address` 时，代码实际上是在做如下跳跃：

1. **输入**：模块内存基址 `BaseAddress`。
2. **跳跃**：
    - `DOS` -> `NT` (`Base + DOS.e_lfanew`)
    - `NT` -> `Optional` -> `DataDirectory[0]` (导出表 RVA)。
3. **定位**：`ExportDir = BaseAddress + DataDirectory[0].VirtualAddress`。
4. **遍历**：读取 `ExportDir->AddressOfNames` (指向一堆字符串指针)。
5. **计算**：`PointerToFunctionName = BaseAddress + NameRVA`。
6. **对比**：拿到字符串，算 Hash，和目标 Hash 对比。
7. **结果**：如果匹配，去 `AddressOfFunctions` 数组取对应下标的函数地址 RVA。
8. **输出**：`BaseAddress + FunctionRVA`。
