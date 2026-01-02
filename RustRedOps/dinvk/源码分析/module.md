 let mut data_table_entry = (*ldr_data).InMemoryOrderModuleList.Flink as*const
  LDR_DATA_TABLE_ENTRY; 这里的LDR_DATA_TABLE_ENTRY这个结构是哪来的?详细解释

✦ LDR_DATA_TABLE_ENTRY 结构体来自 src/types.rs 文件。

  在 src/module.rs 的开头，有一行 use crate::{types::*, helper::PE};，这意味着
  src/types.rs 中定义的所有公开类型（pub）都被引入到了当前作用域。

  在 Windows 编程（尤其是涉及到底层 PEB 遍历的场景）中，标准的 Windows API 头文件或者
  Rust 的 winapi/windows-sys
  库中提供的定义往往是不完整、不公开（Undocumented）或者为了兼容性隐藏了部分字段的。

  为了实现像 get_module_address 这样利用 InMemoryOrderModuleList
  遍历模块的功能，dinvk 的作者必须在 types.rs 中手动定义一个与内存布局完全匹配的
  LDR_DATA_TABLE_ENTRY 结构体。

  让我们来验证并查看 src/types.rs 中的定义。这将解开为什么 src/module.rs
  中可以直接强转指针的谜题。
╭───────────────────────────────────────────────────────────────────────────────────╮
│ ✓  ReadFile src\types.rs                                                          │
│                                                                                   │
╰───────────────────────────────────────────────────────────────────────────────────╯
✦ 好的，通过 src/types.rs 文件，我们终于找到了 LDR_DATA_TABLE_ENTRY 的定义。

  现在，我来为您彻底解开 (*ldr_data).InMemoryOrderModuleList.Flink
  强转这个谜团。这涉及到对 Windows 内核数据结构、C 语言内存布局以及 Rust
  互操作性的深刻理解。

  1. 结构体定义分析

  让我们看看 dinvk 在 src/types.rs 中是如何定义 LDR_DATA_TABLE_ENTRY 的：

    1 #[repr(C)]
    2 pub struct LDR_DATA_TABLE_ENTRY {
    3     pub Reserved1: [*mut c_void; 2],     // 偏移 0x00
    4     pub InMemoryOrderLinks: LIST_ENTRY,  // 偏移 0x10 (在 64 位系统上)
    5     pub Reserved2: [*mut c_void; 2],     // 偏移 0x20
    6     pub DllBase: *mut c_void,            // 偏移 0x30
    7     pub Reserved3: [*mut c_void; 2],     // ...
    8     pub FullDllName: UNICODE_STRING,     // ...
    9     // ...
   10 }

  而在 Windows 真实的（或者说微软符号文件中的）定义中，结构体开头通常是这样的：

   1 typedef struct _LDR_DATA_TABLE_ENTRY {
   2     LIST_ENTRY InLoadOrderLinks;        // 偏移 0x00
   3     LIST_ENTRY InMemoryOrderLinks;      // 偏移 0x10
   4     LIST_ENTRY InInitializationOrderLinks; // 偏移 0x20
   5     PVOID DllBase;                      // 偏移 0x30
   6     // ...
   7 } LDR_DATA_TABLE_ENTRY, *PLDR_DATA_TABLE_ENTRY;

  关键点来了：
  dinvk 的定义把前两个 void* (Reserved1) 放在了最前面。在 64
  位系统下，两个指针的大小是 16 字节 (0x10)。
  这恰好对应了真实结构体中第一个字段 InLoadOrderLinks (它也是两个指针，Flink 和
  Blink，共 16 字节) 的大小。

  所以，dinvk 定义的 InMemoryOrderLinks 确实位于偏移 0x10 处，与真实结构体一致。

  1. 那个“奇怪”的类型转换

  回到 src/module.rs 中的这行代码：

   1 let mut data_table_entry = (*ldr_data).InMemoryOrderModuleList.Flink as*const
     LDR_DATA_TABLE_ENTRY;

  这里的逻辑是这样的：

   1. ldr_data 是 PEB_LDR_DATA 结构体。
   2. InMemoryOrderModuleList 是 PEB_LDR_DATA 中的一个 LIST_ENTRY 链表头。
   3. Flink 指向的是下一个模块的 LDR_DATA_TABLE_ENTRY 结构体中 `InMemoryOrderLinks`字段的地址，而不是结构体的起始地址！

  这意味着 data_table_entry 这个指针的值，实际上指向的是目标结构体偏移 0x10 的位置。

  1. “错位”的访问技巧

  现在 data_table_entry 指向的是偏移 0x10
  的位置。如果我们直接用它访问成员，会发生什么？

  Rust (和 C) 编译器计算成员偏移是基于结构体基址的。
  比如，我们要访问 Reserved2 (在 dinvk 定义中偏移是 0x20)。
  编译器会生成代码：读取 (基址 + 0x20)。

  但是，我们的“基址”实际上已经是 真实基址 + 0x10 了。
  所以，实际访问的内存地址是：(真实基址 + 0x10) + 0x20 = 真实基址 + 0x30。

  让我们看看真实结构体偏移 0x30 是什么？
  是 DllBase！

  再看看 dinvk 的代码是怎么写的：

   1 // module.rs
   2 addr = (*data_table_entry).Reserved2[0];

  它访问了 Reserved2。
  根据 dinvk 的定义，Reserved2 在 LDR_DATA_TABLE_ENTRY 中位于 InMemoryOrderLinks
  (0x10) 之后，偏移是 0x20。

  等一下，这里的计算有点微妙。让我们再仔细算一遍：

  假设：

* LDR_DATA_TABLE_ENTRY 的真实基址是 0x1000。
* InMemoryOrderLinks 字段位于 0x1010。
* DllBase 字段位于 0x1030。
* FullDllName 字段位于 0x1048 (0x30 + 8 + 2*8 + 8 = 0x48? 稍微有点不准，取决于
     padding，但大概在这个位置)。

  代码行为：

   1. Flink 指向 0x1010。
   2. data_table_entry 被赋值为 0x1010。
   3. 访问 (*data_table_entry).Reserved2。
       * 在 dinvk 定义中，Reserved2 位于偏移 0x20 处 (Reserved1(0x10) +
         InMemoryOrderLinks(0x10) = 0x20)。
       * 计算出的地址 = 0x1010 + 0x20 = 0x1030。
       * 0x1030 正好是真实的 DllBase 地址！
       * 完美匹配！

  再验证一下 `FullDllName`：

* 在 dinvk 定义中，FullDllName 位于偏移 0x20 (Reserved2) + 0x10 (两个指针) + 0x08
     (DllBase) + 0x10 (Reserved3) = 0x48。
  * Wait, dinvk 的定义顺序是：

   1.         pub Reserved1: [*mut c_void; 2],    // 0x00
   2.         pub InMemoryOrderLinks: LIST_ENTRY, // 0x10
   3.         pub Reserved2: [*mut c_void; 2],    // 0x20
   4.         pub DllBase: *mut c_void,           // 0x30
   5.         pub Reserved3: [*mut c_void; 2],    // 0x38
   6.         pub FullDllName: UNICODE_STRING,    // 0x48

* 代码访问 (*data_table_entry).FullDllName。
  * 偏移是 0x48。
  * 实际地址 = 0x1010 (基址) + 0x48 = 0x1058。
* 让我们看真实的结构体定义：

   1     // Windows 10 x64 典型布局
   2     /*0x00 */ LIST_ENTRY InLoadOrderLinks;
   3     /* 0x10 */ LIST_ENTRY InMemoryOrderLinks;
   4     /* 0x20 */ LIST_ENTRY InInitializationOrderLinks;
   5     /* 0x30 */ PVOID DllBase;
   6     /* 0x38 */ PVOID EntryPoint;
   7     /* 0x40 */ ULONG SizeOfImage;
   8     /* 0x48*/ UNICODE_STRING FullDllName;
       *FullDllName 确实在 0x48！
       * 但是！ 我们的指针是错位的，错位了 0x10。
       *所以我们要访问真实的 0x48，我们的定义里该字段的偏移应该是 0x38 才对 (0x48 -
         0x10)。
       * 但在 dinvk 的定义里，FullDllName 偏移是 0x48。
       *计算结果：0x1010 + 0x48 = 0x1058。
       * 而在真实内存中，0x1058 是 BaseDllName (通常紧跟在 FullDllName 后面)。
  这里似乎有个矛盾？
  除非 dinvk 的 Reserved 字段大小不仅仅是我想的那样，或者 Windows
  的结构在不同版本有变化。

  让我们重新审视 dinvk 的定义：

   1 #[repr(C)]
   2 pub struct LDR_DATA_TABLE_ENTRY {
   3     pub Reserved1: [*mut c_void; 2],      // 16 bytes
   4     pub InMemoryOrderLinks: LIST_ENTRY,   // 16 bytes. Current offset: 32 (0x20)
     -- WAIT!
   5 }

  等一下！
  Reserved1 是 [*mut c_void; 2] -> 16 bytes (x64)。
  InMemoryOrderLinks 是 LIST_ENTRY -> 16 bytes。
  所以 InMemoryOrderLinks 的偏移是 0x10。没问题。

  如果 data_table_entry 指向 InMemoryOrderModuleList.Flink (即真实内存的 0x10 处)。

  我们想访问 DllBase (真实偏移 0x30)。
  我们需要偏移 0x20。
  dinvk 定义中：
  Reserved2 (16 bytes) 紧跟在 InMemoryOrderLinks 后。
  所以 Reserved2 的偏移是 0x10 + 0x10 = 0x20。
  访问 Reserved2 -> base + 0x20 -> (real_base + 0x10) + 0x20 -> real_base + 0x30。
  这对上了！ 所以代码里用 Reserved2[0] 来获取 DllBase 是完全正确的“错位访问”。

  我们想访问 FullDllName (真实偏移 0x48)。
  我们需要偏移 0x38 (0x48 - 0x10)。
  dinvk 定义中：
  DllBase (8 bytes) 在 0x30。
  Reserved3 (16 bytes) 在 0x38。
  FullDllName (16 bytes) 在 0x48。
  如果我们直接访问 FullDllName，偏移是 0x48。
  实际地址 = real_base + 0x10 + 0x48 = real_base + 0x58。
  这不对！ 0x58 是 BaseDllName。

  结论：
  如果这段代码能正常工作，只有一种可能：
  dinvk 的作者并不是在利用“错位指针”，而是利用了 CONTAINING_RECORD
  的逻辑，但是他在代码里并没有减去偏移！

  让我们回头看 module.rs：

   1 let mut data_table_entry = (*ldr_data).InMemoryOrderModuleList.Flink as*const
     LDR_DATA_TABLE_ENTRY;
  这一行把 LIST_ENTRY 指针直接转成了 LDR_DATA_TABLE_ENTRY 指针。这是典型的错位。

  如果 FullDllName 取到的是 BaseDllName (0x58)，代码能跑吗？
  BaseDllName 也是 UNICODE_STRING，存的是文件名 (例如 "kernel32.dll")。
  FullDllName 存的是全路径 (例如 "C:\Windows\System32\kernel32.dll")。

  代码里有这句：
   1 // module.rs
   2 let module = canonicalize_module(&module.to_string());
   3 // ...
   4 if dll_file_name == module { ... }
  canonicalize_module 会去除路径。

  如果取到的是 BaseDllName (只有文件名)，那么 canonicalize_module 后还是文件名。
  如果取到的是 FullDllName (全路径)，canonicalize_module 后是文件名。

  真相大白！
  作者巧妙地（或者碰巧）利用了这个错位：
  他通过错位指针，访问 FullDllName 字段时，实际上读取的是 `BaseDllName` 字段！
  而 BaseDllName
  正好就是文件名，这反而省去了从全路径中解析文件名的麻烦（虽然代码里还是做了一次
  canonicalize 以防万一，或者兼容不同版本的 Windows，因为有时候 BaseDllName
  可能为空？不，BaseDllName 总是存在的）。

  总结这个“黑魔法”：

   1. 来源：LDR_DATA_TABLE_ENTRY
      定义是作者为了适配这种特定访问模式而“捏造”的，并非官方定义。
   2. 错位：代码直接将 InMemoryOrderLinks
      的指针当作结构体基址，导致所有字段访问都向后偏移了 0x10 字节。
   3. DllBase：通过访问 Reserved2 (定义偏移 0x20) 来获取 DllBase (真实偏移
      0x30)，利用了偏移计算 0x10 + 0x20 = 0x30。
   4. DllName：通过访问 FullDllName (定义偏移 0x48) 来获取 BaseDllName (真实偏移
      0x58)，利用了偏移计算 0x10 + 0x48 = 0x58。

  这就是为什么这么写，以及为什么能工作的原因。这是一种非常典型的、利用内存布局巧合的
  黑客编程技巧。

> 这个LDR_DATA_TABLE_ENTRY结构代表什么?为什么在PEB中没有找到相关信息?我知道这么定义是
  为了找当前dll的基址,找基址实现的原理又是什么呢

✦ 1. LDR_DATA_TABLE_ENTRY 代表什么？

  LDR_DATA_TABLE_ENTRY 是 Windows 操作系统内部用于描述每一个已加载模块（DLL 或
  EXE）的元数据结构。

  想象一下，操作系统是一个图书馆管理员，每个加载到内存中的 DLL（比如 kernel32.dll,
  ntdll.dll）都是一本书。管理员需要一个账本记录每本书放在哪个书架（内存地址）、书名是
  什么、有多大等等。

* PEB (Process Environment Block)：是这个管理员的总账本。
* PEB_LDR_DATA：是总账本里专门记录“库存列表”的一页。
* LDR_DATA_TABLE_ENTRY：就是库存列表里的每一行记录。每一个此结构体就对应一个已加载
     的 DLL。

  1. 为什么在 PEB 中“没有找到”？

  你其实在 PEB 中找到了它，只是它是以链表的形式存在的，而不是直接包含在 PEB
  结构体里。

  让我们看 PEB 的结构（简化版）：

   1 struct PEB {
   2     // ...
   3     PEB_LDR_DATA* Ldr; // 指向加载器数据的指针
   4     // ...
   5 }

  再看 PEB_LDR_DATA：

   1 struct PEB_LDR_DATA {
   2     // ...
   3     LIST_ENTRY InLoadOrderModuleList;       // 按加载顺序排列的链表头
   4     LIST_ENTRY InMemoryOrderModuleList;     // 按内存地址排列的链表头
   5     LIST_ENTRY InInitializationOrderModuleList; // 按初始化顺序排列的链表头
   6 }

  这里只有 LIST_ENTRY。LIST_ENTRY 只是两个指针（向前 Flink，向后
  Blink），它不包含任何 DLL 的信息。

  关键在于连接方式：
  Windows 把 LIST_ENTRY 结构体嵌入到了 LDR_DATA_TABLE_ENTRY 结构体内部。

   1 LDR_DATA_TABLE_ENTRY (DLL A)       LDR_DATA_TABLE_ENTRY (DLL B)
   2 +--------------------------+       +--------------------------+
   3 | ...                      |       | ...                      |
   4 | InMemoryOrderLinks (Node)|<----->| InMemoryOrderLinks (Node)|
   5 | ...                      |       | ...                      |
   6 +--------------------------+       +--------------------------+

  所以，你在 PEB 里只能看到链表的头。你需要顺着这个链表摸索（遍历 Flink
  指针），每一个节点实际上都是寄生在一个巨大的 LDR_DATA_TABLE_ENTRY 结构体里面的。

  这就是为什么我们需要通过 CONTAINING_RECORD 宏或者像 dinvk
  那样使用“错位指针”技巧，从链表节点反推回整个结构体的起始位置，从而读取旁边的
  DllBase 或 FullDllName。

  1. 找基址实现的原理是什么？

  所谓的“找基址”，本质上就是遍历这个链表，找到名字匹配的那个节点，然后读取它记录的地
  址。

  原理流程如下：

   1. 起点：获取 PEB 指针（对于 x64，在 gs:[0x60] 寄存器；对于 x86，在
      fs:[0x30]）。这是所有用户模式代码都能访问的、记录当前进程信息的绝对起点。
   2. 跳板：从 PEB 读取 Ldr 指针，进入 PEB_LDR_DATA。
   3. 选路：选择一条链表，通常选 InMemoryOrderModuleList。
   4. 遍历：
       * 读取链表的第一个节点（Flink）。
       * 利用指针运算，访问该节点所属 LDR_DATA_TABLE_ENTRY 中的 FullDllName（或
         BaseDllName）。
       * 比对：这个名字是我要找的吗（比如 "kernel32.dll"）？
           * 是：读取同结构体中的 DllBase 字段。这个值就是该 DLL
             在内存中的起始地址（基址）。任务完成。
           * 否：读取 Flink 字段，跳到下一个节点，重复步骤。

  为什么这个原理有效？
  因为 Windows 加载器（Loader）在启动进程或使用 LoadLibrary 加载 DLL
  时，必须维护这个链表，以便它自己管理模块。攻击者（或底层工具）只是利用了操作系统必
  然存在的这个管理结构，来“手动”查询信息，从而绕过 GetModuleHandle 这样的公开 API。

  总结图示

    1 [ TEB (Thread Environment Block) ]
    2    |
    3    +-> PEB (Process Environment Block)
    4         |
    5         +-> Ldr (PEB_LDR_DATA)
    6              |
    7              +-> InMemoryOrderModuleList (链表头)
    8                   |
    9                   v
   10              [ Node 1 ] -----------------> [ Node 2 ] -----------------> ...
   11              (嵌入在 DLL A 的元数据中)      (嵌入在 DLL B 的元数据中)
   12                   |                             |
   13     +-------------+-------------+  +------------+-------------+
   14     | LDR_DATA_TABLE_ENTRY (A)  |  | LDR_DATA_TABLE_ENTRY (B) |
   15     |                           |  |                          |
   16     | - DllBase: 0x7FF...       |  | - DllBase: 0x7FF...      | <--- 这里的
      DllBase 就是我们要找的基址
   17     | - FullDllName: "ntdll.dll"|  | - FullDllName: "kernel32"| <---
      比对这里的名字
   18     | - InMemoryOrderLinks [Node]|  | - InMemoryOrderLinks [Node]|
   19     +---------------------------+  +--------------------------+

> LDR_DATA_TABLE_ENTRY 维护的是什么信息?由什么产生并维护的?或者说它是peb的一部分还是?

✦ LDR_DATA_TABLE_ENTRY 是 Windows 操作系统中用于描述一个已加载模块（DLL 或
  EXE）核心元数据的结构体。

  为了让你彻底理解，我们可以把它想象成图书馆的图书登记卡，而 PEB 是图书馆的总目录。

  以下是关于它的详细解构：

  1. 它维护了什么信息？
  这个结构体记录了关于一个模块在内存中存活所需的一切关键信息。最核心的字段包括：

* 链表节点 (Links): 把它串联到所有已加载模块列表中的“钩子”。
  * InLoadOrderLinks: 按加载顺序（谁先 LoadLibrary 谁在前）。
  * InMemoryOrderLinks: 按内存地址布局顺序（dinvk 代码中使用的就是这个）。
  * InInitializationOrderLinks: 按 DllMain 初始化完成的顺序。
* 基址 (DllBase): 这个模块被加载到了内存的哪个位置（例如
     0x7FF...）。这是所有黑客工具和系统 API 都在找的东西。
* 入口点 (EntryPoint): 模块的代码开始执行的地方（例如 DllMain 的地址）。
* 大小 (SizeOfImage): 模块在内存中占用了多少字节。
* 名称:
  * FullDllName: 完整路径（例如 C:\Windows\System32\kernel32.dll）。
  * BaseDllName: 文件名（例如 kernel32.dll）。
* 引用计数 (LoadCount): 有多少地方引用了这个 DLL。当计数归零时，DLL 会被卸载。
* 标志位 (Flags): 记录状态，比如是否已经处理了重定位、是否是 .NET 程序集等。

  1. 它是由什么产生并维护的？
  它是完全由 Windows 加载器 (Windows Loader) 产生和维护的。加载器的大部分逻辑位于
  ntdll.dll 中（以 Ldr 开头的函数，如 LdrpLoadDll）。

* 产生 (Creation):
      当你调用 LoadLibrary("abc.dll") 或者系统启动一个进程时，NTDLL 中的加载器会：
       1. 把 DLL 文件从磁盘映射到内存。
       2. 在进程堆 (Process Heap) 上分配一块内存，大小等于 LDR_DATA_TABLE_ENTRY
          结构体。
       3. 填入上述信息（基址、名字等）。
       4. 把这个新生成的结构体挂载到 PEB 的链表中。

* 维护 (Maintenance):
      如果系统需要卸载 DLL
  (FreeLibrary)，加载器会把这个结构体从链表中摘除，并释放这块内存。

  1. 它是 PEB 的一部分吗？
  严格来说，不是 PEB 结构体直接的一部分，而是被 PEB 指向的数据。

* 包含关系: PEB 并不直接包含 LDR_DATA_TABLE_ENTRY 的数据块。
* 引用关系: PEB 包含一个指针指向 PEB_LDR_DATA，而 PEB_LDR_DATA 包含链表的头节点
     (Head)。LDR_DATA_TABLE_ENTRY 是这些链表上的节点 (Node)。

  内存布局图解：

    1 [ PEB (进程环境块) ]
    2    |
    3    +--> Ldr (指针) ---> [ PEB_LDR_DATA 结构体 ]
    4                            |
    5                            +--> InMemoryOrderModuleList (链表头)
    6                                      |
    7            +-------------------------+
    8            |
    9            v
   10     [ LDR_DATA_TABLE_ENTRY (ntdll.dll) ]  <-- 这是堆上的一块独立内存
   11            |
   12            +--> InMemoryOrderLinks (指向下一个)
   13            |         |
   14            |         v
   15            |  [ LDR_DATA_TABLE_ENTRY (kernel32.dll) ] <-- 堆上的另一块内存
   16            |         |
   17            |         v
   18            |  [ ... ]

  1. 为什么 dinvk 要手动解析它？
  在正常的软件开发中，你不需要关心这个结构体，你会使用官方 API：

* GetModuleHandle -> 返回 DllBase
* GetModuleFileName -> 返回 FullDllName

  但是，Windows API 本身也是通过读取这个链表来实现上述功能的。

  dinvk 之所以手动定义并遍历它，是因为：

   1. 隐蔽性 (Stealth): 调用 GetModuleHandle 会在 IAT (导入地址表)
      中留下痕迹，或者会被 EDR (端点检测与响应系统) 的 Hook 监控到。手动遍历 PEB
      是“纯内存操作”，不经过系统 API，很难被拦截。
   2. 断链攻击 (Unlinking): 既然这是一个双向链表，黑客常用的技术是把自己的恶意 DLL
      对应的 LDR_DATA_TABLE_ENTRY 从链表中摘除（修改前后节点的 Flink/Blink
      指针），但保留 DLL 在内存中。这样，该 DLL 依然能运行，但 GetModuleHandle
      或任务管理器等工具遍历链表时就“看”不到它了。这就是所谓的 "Module Hiding"。

  总结：
  LDR_DATA_TABLE_ENTRY 是 Windows 内部用于管理模块的账本记录。它不在 PEB
  结构体内，但通过 PEB 中的链表指针可以访问到。它是所有模块查找、枚举功能的基石。

 你感叹“最难的是用 Rust 表示数据结构”，这确实触及了 Rust
  底层开发的痛点。主要有以下几个原因：

  A. 默认布局不兼容 (The Layout Trap)
  在 Rust
  中，默认情况下编译器可以自由重新排列结构体字段的顺序，以优化内存占用。这在编写普通
  的业务逻辑时很好，但在操作系统层面，如果字段顺序变了，你读取到的就是垃圾数据甚至导
  致程序崩溃。

* 解决方案: 必须给每个结构体加上 #[repr(C)]。这告诉 Rust 编译器：“请严格按照 C
     语言的规则来排列内存，不要乱动”。

  B. 复杂的联合体 (Unions)
  Windows 结构体中到处都是 union（联合体）。在 C 语言中，union 很自然；但在 Rust
  中，union 是非常受限的，且访问任何字段都是 unsafe 的。

* 难点: 你必须手动处理哪个字段在什么情况下有效，且 Rust 对 union 的派生（如 Debug,
     Clone）支持也比 struct 麻烦。

  C. 位域 (Bitfields)
  Windows 喜欢用位域来节省空间，例如：
   1 struct {
   2     ULONG ImageUsesLargePages : 1;
   3     ULONG IsProtectedProcess : 1;
   4     // ...
   5 }
  Rust 官方语法不支持位域。

* 痛苦: 你必须定义一个普通的 u32，然后写一堆位运算（&, |,
     <<）来提取信息，或者使用像 bitflags 这样的库。在 dinvk 的 types.rs
     中，你可以看到作者用了大量的 bitflags!。

  D. 不同架构的差异 (x86 vs x64)
  在 32 位和 64 位下，同一个结构体的成员大小（尤其是指针）和偏移量是完全不同的。

* 麻烦: 你需要大量使用 #[cfg(target_arch = "x86_64")] 和 #[cfg(target_arch =
     "x86")]
     来写两套定义，或者使用条件编译。如果搞错一个偏移量，整个程序就会在运行时报莫名其
     妙的错误。

  E. 未公开与版本变动
  这是最玄学的地方。微软可能在 Win10 21H2 到 Win11
  之间，给某个内部结构体中间偷偷塞了一个字段。

* 代价: 你必须查阅像 Terminus Project
     (<https://www.nirsoft.net/kernel_struct/vista/index.html>)
     这样的非官方文档，或者亲自去 WinDbg 里输入 dt nt!_EPROCESS 来肉眼观察偏移量。

  总结

  dinvk 项目的价值，很大一部分就在于它已经为你预先踩坑，把这些复杂的、容易出错的
  Windows 内部结构体用 Rust 代码“翻译”好了。

  理解了 Reserved1 的 16
  字节原理，你就掌握了内存偏移计算的精髓——在底层开发中，数据是什么类型不重要，重要的
  是它占据了多少空间，以及它的偏移量是多少。

   #[repr(C)]
  //与verg中_LDR_DATA_TABLE_ENTRY相对应
  pub struct LDR_DATA_TABLE_ENTRY {
      pub Reserved1: [*mut c_void; 2],//该字段大小16字节,因为在repr(c)模式下64 bit
  os一个指针占用8字节   这段话对吗?如果对,那么在rust中没有repr c的模式下,一个*mut
  c_void 这种指针多大

✦ 这段话完全正确。

  针对你的疑问，我分两部分为你深入解释：

  1. 为什么那段话是对的？

* 指针大小：在 64 位操作系统（Win64）中，CPU 的寻址空间是 64
     位的，因此一个内存地址（指针）必须占用 8 字节（64 bit）。
* 数组计算：[*mut c_void; 2] 是一个包含两个指针的数组，大小固定为 $8 \times 2 =
     16$ 字节。
* `#[repr(C)]` 的作用：它保证了结构体的内存布局遵循 C 语言标准。在 C
     标准中，数组是连续排列的，且结构体字段不会被编译器乱序。这确保了 Reserved1
     准确地占住了从偏移量 0x0 到 0xF 的位置，让后续字段能对齐到正确的 Windows
     内核偏移上。

  ---

  1. 在没有 #[repr(C)] 的模式下，*mut c_void 指针多大？

  答案是：依然是 8 字节（在 64 位系统上）。

  这里有一个常见的误区需要澄清：

  指针的大小由“硬件架构”决定，而不是由 repr 属性决定

* 裸指针 (`*mut T` / `*const T`)：它们的大小在编译成目标机器码时就已经确定了。在
     64 位平台上永远是 8 字节，在 32 位平台上永远是 4 字节。无论你是否写
     #[repr(C)]，指针本身存储地址所需的空间是不变的。

  那 #[repr(C)] 到底改了什么？
  如果不加 #[repr(C)]，Rust 默认使用 #[repr(Rust)]
  布局。两者的区别不在于“基本类型（如指针、i32）的大小”，而在于“结构体的整容手术”：

   1. 字段重排 (Field Reordering)：
      Rust 编译器为了减少结构体内部的内存碎屑（Padding），可能会重新排列字段顺序。
       * 例如：你定义了 struct { a: u8, b: u32, c: u8 }。
       * repr(C) 模式：内存布局严格按照 a, (padding), b, c 排列，大小为 12 字节。
       * repr(Rust) 模式：编译器可能把它改成 b, a, c，大小缩减为 8 字节。
       * 在 DInvoke 中，这会致命：因为 Windows 内核要求 DllBase 必须在偏移 0x30
         的位置。如果 Rust 把字段换了位置，你读到的地址就是错的。

   2. 对齐 (Alignment)：
      repr(C) 遵循平台相关的 C 对齐准则。Rust 默认布局可能会有更激进的对齐优化。

  特殊情况：胖指针 (Fat Pointers)
  虽然 *mut c_void 大小固定，但 Rust 中有一种指针是两个指针大小（16
  字节），叫做“胖指针”：

* 切片指针：*mut [u32]（包含地址 + 长度）。
* Trait 对象指针：*mut dyn MyTrait（包含地址 + 虚函数表 vtable 地址）。

  但由于 `c_void` 是一个特定的类型，`*mut c_void` 永远只是一个普通的瘦指针（Thin
  Pointer），大小始终等于 CPU 位数。

  总结

* Reserved1 在 64 位下确实是 16 字节。
* 不加 repr(C)，指针还是 8
     字节，但结构体可能会崩掉，因为字段的位置会变得不可预测。
* 做免杀和底层开发，`#[repr(C)]` 是强制性的，因为你要和 C 编写的 Windows
     内核“对暗号”。

## PEB

```rust
pub struct PEB {
    pub InheritedAddressSpace: u8,
    pub ReadImageFileExecOptions: u8,
    pub BeingDebugged: u8,
    pub Anonymous1: PEB_0,
    pub Mutant: HANDLE,
    pub ImageBaseAddress: *mut c_void,
    pub Ldr: *mut PEB_LDR_DATA,
    pub ProcessParameters: *mut RTL_USER_PROCESS_PARAMETERS,
    pub SubSystemData: *mut c_void,
    pub ProcessHeap: *mut c_void,
    pub FastPebLock: *mut RTL_CRITICAL_SECTION,
    pub AtlThunkSListPtr: *mut SLIST_HEADER,
    pub IFEOKey: *mut c_void,
    pub Anonymous2: PEB_1,
    pub Anonymous3: PEB_2,
    pub SystemReserved: u32,
    pub AtlThunkSListPtr32: u32,
    pub ApiSetMap: *mut API_SET_NAMESPACE,
    pub TlsExpansionCounter: u32,
    pub TlsBitmap: *mut RTL_BITMAP,
    pub TlsBitmapBits: [u32; 2],
    pub ReadOnlySharedMemoryBase: *mut c_void,
    pub SharedData: *mut SILO_USER_SHARED_DATA,
    pub ReadOnlyStaticServerData: *mut c_void,
    pub AnsiCodePageData: *mut c_void,
    pub OemCodePageData: *mut c_void,
    pub UnicodeCaseTableData: *mut c_void,
    pub NumberOfProcessors: u32,
    pub NtGlobalFlag: u32,
    pub CriticalSectionTimeout: LARGE_INTEGER,
    pub HeapSegmentReserve: usize,
    pub HeapSegmentCommit: usize,
    pub HeapDeCommitTotalFreeThreshold: usize,
    pub HeapDeCommitFreeBlockThreshold: usize,
    pub NumberOfHeaps: u32,
    pub MaximumNumberOfHeaps: u32,
    pub ProcessHeaps: *mut c_void,
    pub GdiSharedHandleTable: *mut c_void,
    pub ProcessStarterHelper: *mut c_void,
    pub GdiDCAttributeList: u32,
    pub LoaderLock: *mut RTL_CRITICAL_SECTION,
    pub OSMajorVersion: u32,
    pub OSMinorVersion: u32,
    pub OSBuildNumber: u16,
    pub OSCSDVersion: u16,
    pub OSPlatformId: u32,
    pub ImageSubsystem: u32,
    pub ImageSubsystemMajorVersion: u32,
    pub ImageSubsystemMinorVersion: u32,
    pub ActiveProcessAffinityMask: usize,
    pub GdiHandleBuffer: GDI_HANDLE_BUFFER,
    pub PostProcessInitRoutine: PPS_POST_PROCESS_INIT_ROUTINE,
    pub TlsExpansionBitmap: *mut RTL_BITMAP,
    pub TlsExpansionBitmapBits: [u32; 32],
    pub SessionId: u32,
    pub AppCompatFlags: ULARGE_INTEGER,
    pub AppCompatFlagsUser: ULARGE_INTEGER,
    pub pShimData: *mut c_void,
    pub AppCompatInfo: *mut c_void,
    pub CSDVersion: UNICODE_STRING,
    pub ActivationContextData: *mut ACTIVATION_CONTEXT_DATA,
    pub ProcessAssemblyStorageMap: *mut ASSEMBLY_STORAGE_MAP,
    pub SystemDefaultActivationContextData: *mut ACTIVATION_CONTEXT_DATA,
    pub SystemAssemblyStorageMap: *mut ASSEMBLY_STORAGE_MAP,
    pub MinimumStackCommit: usize,
    pub SparePointers: *mut c_void,
    pub PatchLoaderData: *mut c_void,
    pub ChpeV2ProcessInfo: *mut c_void,
    pub Anonymous4: PEB_3,
    pub SpareUlongs: [u32; 2],
    pub ActiveCodePage: u16,
    pub OemCodePage: u16,
    pub UseCaseMapping: u16,
    pub UnusedNlsField: u16,
    pub WerRegistrationData: *mut WER_PEB_HEADER_BLOCK,
    pub WerShipAssertPtr: *mut c_void,
    pub Anonymous5: PEB_4,
    pub pImageHeaderHash: *mut c_void,
    pub Anonymous6: PEB_5,
    pub CsrServerReadOnlySharedMemoryBase: u64,
    pub TppWorkerpListLock: *mut RTL_CRITICAL_SECTION,
    pub TppWorkerpList: LIST_ENTRY,
    pub WaitOnAddressHashTable: [*mut c_void; 128],
    pub TelemetryCoverageHeader: *mut TELEMETRY_COVERAGE_HEADER,
    pub CloudFileFlags: u32,
    pub CloudFileDiagFlags: u32,
    pub PlaceholderCompatibilityMode: i8,
    pub PlaceholderCompatibilityModeReserved: [i8; 7],
    pub LeapSecondData: *mut c_void, // PLEAP_SECOND_DATA
    pub Anonymous7: PEB_6,
    pub NtGlobalFlag2: u32,
    pub ExtendedFeatureDisableMask: u64,
}
```

### ImageBaseAddress

ImageBaseAddress 是 PEB 结构体中的一个重要字段（通常在偏移 0x10 处）

* 它存储了当前进程的主模块（也就是启动这个进程的 .exe文件）被加载到内存中的起始位置。
* 对于大多数 64 位程序，默认情况下这个地址可能是0x140000000，或者是因ASLR（地址空间布局随机化）而随机生成的某个地址。

## PEB_LDR_DATA

```rust
#[repr(C)]
pub struct PEB_LDR_DATA {
    pub Length: u32,
    pub Initialized: u8,
    pub SsHandle: HANDLE,
    pub InLoadOrderModuleList: LIST_ENTRY,
    pub InMemoryOrderModuleList: LIST_ENTRY,
    pub InInitializationOrderModuleList: LIST_ENTRY,
    pub EntryInProgress: *mut c_void,
    pub ShutdownInProgress: u8,
    pub ShutdownThreadId: HANDLE,
}
```

### InMemoryOrderModuleList

`InMemoryOrderModuleList` 并非如其名称暗示的那样"按内存地址高低排序"，这是一个在安全研究和逆向工程社区中广泛流传的误解。  
实际上，这个链表主要反映**模块在内存中的布局顺序和初始化关系**，而非简单的地址高低排序。

InMemoryOrderModuleList的结构体中只有两个指针 Flink (前向) 和 Blink (后向),同样需要注意InMemoryOrderModuleList.flink指向的是LDR_DATA_TABLE_ENTRY这个结构体的**中间位置(不是第一个字段)**,即是一种手拉手的双向链表,而不是手拉头的双向链表:

* Flink 指向的是下一个模块结构体中对应的那个 `InMemoryOrderLinks`字段的地址。
* 它不指向下一个结构体的头部 (Base)。

InMemoryOrderModuleList,在实际的运行中,根据Windows 加载器初始化模块的顺序是雷打不动的：

* Head：链表头（PEB_LDR_DATA 内部）。
* Node 1：主程序 (.exe)（第一个加载）。
* Node 2：ntdll.dll（第二个加载，负责用户层与内核层的交互）。
* Node 3：kernel32.dll（通常情况）。
* 代码逻辑：从 Head 开始，执行两次 Flink 跳转，理应到达ntdll.dll。为了保险，代码还计算了模块名的 Hash 进行校验。

## let mut data_table_entry = (*ldr_data).InMemoryOrderModuleList.Flink as*const LDR_DATA_TABLE_ENTRY

这里的as有点复杂,as转换直接把裸地址变成了可操作的结构体对象。只要地址数值正确，且结构体布局匹配，你就可以直接像操作普通对象一样解引用和访问字段。

### 物理事实 (Memory Reality)

* Flink 指针里存的数值是 `0x1000`（假设这是某个模块 `InMemoryOrderLinks` 的地址）。
* 由于 `InMemoryOrderLinks` 在该模块真正的起始位置（`0x0F90`）往后 16 字节处，所以物理地址就是 `0x1000`。

### 编译器的“错觉” (Compiler's View)

当你执行 `as *const LDR_DATA_TABLE_ENTRY` 时：

* **指令层面**：这行代码在生成的汇编里通常不产生任何指令，它只是改变了编译器账本上的一个标记。
* **认知层面**：你告诉编译器：“从现在开始，把 `0x1000` 这个地址看作是 `LDR_DATA_TABLE_ENTRY` 结构体的第 0 个字节。”

### 结果：偏移量的“平移”

因为编译器认为 `0x1000` 是 `0x00`，那么当你在代码里访问某个字段（比如偏移为 `0x48` 的字段）时：

* **编译器计算**：当前位置 (`0x1000`) + 字段偏移 (`0x48`) = `0x1048`。
* **物理对比**：在真实的内存布局中，`0x1048` 对应的是真实头部 (`0x0F90`) + `0x58`。
* **神奇发现**：`0x58` 处正好就是我们要找的那个数据字段！

### 总结

* “指向...偏移位置 (`0x10`)”：这是物理事实，指针确实指在人家结构体的“肚子”上。
* “编译器看来仍然是指向...第 0 个字节处”：这是类型转换的作用，你强迫编译器从“肚子”开始算作“头”。
