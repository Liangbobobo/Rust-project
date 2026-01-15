- [程序执行全生命周期](#程序执行全生命周期)
  - [标准 Windows 64位控制台程序（如 Rust 编译出的 .exe）从双击到退出的全景流程](#标准-windows-64位控制台程序如-rust-编译出的-exe从双击到退出的全景流程)
    - [第一阶段：进程创建与内核态工作(打造容器)](#第一阶段进程创建与内核态工作打造容器)
    - [第二阶段：苏醒（用户态加载器 Loader）](#第二阶段苏醒用户态加载器-loader)
    - [第三阶段：运行（Runtime 与 Main）](#第三阶段运行runtime-与-main)
    - [第四阶段：终结（进程退出）](#第四阶段终结进程退出)
  - [ntdll.dll](#ntdlldll)
  - [PEB](#peb)
    - [PEB结构](#peb结构)
  - [⚠️ 关键技术声明 (必读)](#️-关键技术声明-必读)
  - [PE](#pe)
    - [🗺️ PE 文件全景地图 (Memory Layout)](#️-pe-文件全景地图-memory-layout)
    - [🧬 核心数据结构详解](#-核心数据结构详解)
      - [1. IMAGE\_DOS\_HEADER (DOS 头)](#1-image_dos_header-dos-头)
      - [2. IMAGE\_NT\_HEADERS64 (NT 头)](#2-image_nt_headers64-nt-头)
      - [3. IMAGE\_SECTION\_HEADER (节表)](#3-image_section_header-节表)
      - [4. The Sections (具体的节内容)](#4-the-sections-具体的节内容)
    - [🧱 实战案例：从 dinvk 视角看结构链](#-实战案例从-dinvk-视角看结构链)
    - [🧪 学习小技巧：偏移计算 (Address Conversion)](#-学习小技巧偏移计算-address-conversion)
  - [为什么如果一个导出函数的 RVA 指向了导出目录 (Export Directory) 所在的内存范围内那么它一定不是代码，而是一个转发字符串 (Forwarder String)](#为什么如果一个导出函数的-rva-指向了导出目录-export-directory-所在的内存范围内那么它一定不是代码而是一个转发字符串-forwarder-string)

# 程序执行全生命周期

## 标准 Windows 64位控制台程序（如 Rust 编译出的 .exe）从双击到退出的全景流程

---

### 第一阶段：进程创建与内核态工作(打造容器)

当你双击 dinvk.exe 时，父进程（通常是 Explorer.exe）发起请求。此时代码甚至还没开始加载。

1. **API 调用**
   - Layer 1 (Win32): Explorer.exe 调用 CreateProcessW。
   - Layer 2 (KernelBase): CreateProcessInternalW 处理参数、环境变量、继承句柄。
   - Layer 3 (Native): ntdll.dll 中的 NtCreateUserProcess 发起 syscall (系统调用指令 0F 05)。
   - Layer 4 (Kernel - Ring 0): CPU 切换到内核模式，执行 ntoskrnl.exe 中的 NtCreateUserProcess。

2. **内核对象的建立 (The EPROCESS)**
   操作系统内核开始分配核心数据结构：
   - EPROCESS: 创建一个执行体进程块。这是进程在内核中的“肉身”，包含 PID、创建时间、配额等。
   - VAD (Virtual Address Descriptors): 初始化虚拟地址描述符树。这是管理该进程 128TB 虚拟内存空间的账本。此时内存是空的，VAD 树只是个根节点。
   - 句柄表 (Handle Table): 创建句柄表，如果是从控制台启动，会继承父进程的标准输入/输出（Stdin/Stdout）句柄。

3. **映射可执行映像 (Section Object)**
   内核不会把整个 EXE 文件读取到物理内存（RAM）中（那太慢了）。
   - 创建 Section: 内核打开磁盘上的 .exe 文件，创建一个 Section Object（节对象）。
   - 内存映射 (Map): 内核将这个 Section 映射到进程的虚拟地址空间。
       - PE 头的 ImageBase（如 0x140000000）决定了首选位置。
       - ASLR（地址随机化）会在此刻介入，给它选一个新的随机基址。
   - 注意: 此时物理内存里几乎什么都没有，只是建立了一个“地址 -> 磁盘偏移”的映射关系。只有当代码执行访问到某页时，才会触发缺页中断 (Page Fault)，真正把数据从磁盘读入 RAM。

4. **映射 NTDLL**
   内核强制将 ntdll.dll 映射到进程空间。这是所有用户态进程的根基，包含堆管理器、加载器和系统调用存根。

5. **创建初始线程 (ETHREAD)**
   进程必须至少有一个线程。
   - 内核分配 ETHREAD 结构。
   - 分配 内核栈 (Kernel Stack) 和 用户栈 (User Stack)。
   - 初始化 Context (寄存器上下文)：
       - RIP (指令指针) 被设置为 ntdll!RtlUserThreadStart (不是你的 main 函数！)。
       - RCX (第一个参数) 被设置为 EXE 的入口点地址。

---

### 第二阶段：苏醒（用户态加载器 Loader）

内核工作完成，执行 sysret 或 iret 指令，CPU 权限级从 Ring 0 降回 Ring 3。线程开始在 ntdll.dll 中运行。

1. **初始化的入口 (LdrInitializeThunk)**
   - PEB 初始化: 线程首先访问 GS:[0x60] 获取 PEB (Process Environment Block)。
   - 堆初始化: 调用 RtlCreateHeap 创建进程的默认堆（Default Heap）。这是后续 malloc 或 Rust Box 的底层来源。

2. **递归加载依赖 (Dependency Walking)**
   ntdll 中的 LdrpInitializeProcess 开始工作，它是一个图遍历算法：
   - 解析导入表: 读取 .exe 的 PE 头 -> OptionalHeader -> DataDirectory[1] (Import Table)。
   - 检查依赖: 发现程序依赖 KERNEL32.DLL。
   - KnownDlls 检查: 为了加速，加载器先查看 \KnownDlls 对象目录（内存中预加载的系统 DLL 缓存）。如果找到了，直接映射 Section，不需要读磁盘。
   - 搜索路径: 如果没找到，按 当前目录 -> System32 -> Windows ... 的顺序搜索磁盘。
   - 递归: 加载 KERNEL32.DLL 后，发现它依赖 ntdll (已加载) 和 KERNELBASE.DLL。加载器会递归加载所有深层依赖。
   - 构建链表: 每加载一个 DLL，就在堆上分配一个 LDR_DATA_TABLE_ENTRY，并挂入 PEB.Ldr.InLoadOrderModuleList 等三个链表中。

3. **地址重定位 (Base Relocations)**
   由于 ASLR，DLL 加载地址与编译时的首选地址不同。
   - 加载器读取 DLL 的 .reloc 节。
   - 遍历所有需要修正的地址，计算 Delta = 实际基址 - 首选基址。
   - 将 Delta 加到代码段的硬编码地址上。

4. **导入地址绑定 (IAT Snapping)**
   这是最关键的一步，也是 dinvk 手动模仿的步骤：
   - 加载器遍历 .exe 的导入表 (IAT)。
   - 对于每一个导入函数（如 WriteFile）：
       - 在 KERNEL32.DLL 的导出表 (EAT) 中查找该名字。
       - 获取其确切内存地址。
       - 写入: 将地址填入 .exe 的 IAT 表槽位中。
   - 至此，你的代码中的 call [WriteFile] 才能正确跳转。

5. **安全机制初始化**

- Stack Cookie: 生成随机数种子 __security_cookie，防止栈溢出攻击。
- CFG (控制流卫士): 验证间接调用目标的位图。

1. **执行 DLL 初始化**
   加载器按照依赖顺序的反序（先底层库，后上层库），依次调用所有 DLL 的入口点：

- DllMain(hInst, DLL_PROCESS_ATTACH, ...)。
- 此时 DLL 可以创建线程或初始化全局锁。

---

### 第三阶段：运行（Runtime 与 Main）

此时所有 DLL 准备就绪，控制权终于要交给你的 EXE 了。

1. **语言运行时 (CRT Startup)**
   加载器跳转到 PE 头中定义的 AddressOfEntryPoint。对于 Rust/C++ 程序，这通常不是 main，而是编译器插入的桩代码（如 mainCRTStartup 或 _start）：

- 命令行解析: 调用 GetCommandLineW 并解析成 argv 数组。
- C++ 全局构造: 如果有全局对象（如 static 类实例），在此刻执行构造函数。
- Rust Runtime: 初始化 Rust 的 panic 钩子、栈溢出保护等。

1. **你的代码 (Main Execution)**

- 调用 main()。
- 此时程序逻辑正式执行。

---

### 第四阶段：终结（进程退出）

当 main 函数返回，或者调用 exit() 时：

1. **用户态清理**

- CRT 清理: 调用 C++ 全局析构函数，刷新 stdio 缓冲区（把没写完的日志写进文件）。
- DllMain Detach: 加载器再次遍历 DLL 链表，调用 DllMain(..., DLL_PROCESS_DETACH, ...)，让 DLL 有机会清理内存。

1. **进入内核自杀**

- 调用 NtTerminateProcess。
- 内核态:
  - 关闭所有打开的句柄（文件、Socket）。引用计数减一。
  - 解除内存映射（VAD 清空）。
  - 将进程对象的 Signaled 状态置位（通知父进程“我退出了”）。
  - 退出代码（Exit Code）写入进程对象。
- 最后的清理: 最后一个线程终结。如果没有任何其他进程持有该进程的句柄，内核销毁 EPROCESS 结构，PID 被回收。

---

这是一个程序在 Windows 上“生老病死”的完整物理过程。dinvk 的所谓“黑客技术”，本质上就是在用户态手动模拟了第 7、8、9 步（加载、重定位、IAT绑定），从而欺骗操作系统认为该模块从未存在过。

## ntdll.dll

## PEB

在 Windows 中，每个进程都有一个 PEB (Process Environment Block)

### PEB结构

## ⚠️ 关键技术声明 (必读)

1. **结构体不透明性**：`EPROCESS`, `ETHREAD`, `KPROCESS` 等内核结构体是 **非公开 (Opaque)** 的。微软从未保证其成员偏移量（Offsets）的稳定性。文中标记的偏移量仅为示例或特定历史版本，实战中**必须**通过符号文件 (`.pdb`) 或运行时特征码搜索动态获取。
2. **ASLR (地址空间布局随机化)**：现代 Windows (Vista+) 强制开启 ASLR。文中出现的内存地址仅为**逻辑示意**，实际运行时基址、堆栈地址每次启动均不同。
3. **架构限定**：本文核心描述 **x64 (AMD64)** 架构下的 Windows 运行机制。

## PE

这是一个关于 **PE (Portable Executable) 文件格式** 的全景式深度解构。

为了让你能够独立编写解析器（例如为了理解 `dinvk` 如何手动加载 DLL，或者 `uwd` 如何解析异常表），我们不能只看概览，必须看**内存布局**(使用windbg)和**C 语言结构体定义**(msdn)。

PE 文件本质上是**“线性存储的数据结构集合”**。在磁盘上和在内存中，它们的逻辑顺序一致，但物理间距（对齐）不同。

---

### 🗺️ PE 文件全景地图 (Memory Layout)

首先，在脑海中建立这个物理模型。假设内存地址从低到高向下增长：

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

---

### 🧬 核心数据结构详解

以下结构体均定义在 Windows SDK 的 `winnt.h` 中。

#### 1. IMAGE_DOS_HEADER (DOS 头)

- **位置**：文件偏移 `0x00`。
- **作用**：为了兼容 1980 年代的 DOS 系统，现在主要是为了找到 NT 头。
- **关键成员**：
  - `e_magic`: **WORD (2字节)**。必须是 `0x5A4D` (**"MZ"**)。
  - `e_lfanew`: **LONG (4字节)**。位于偏移 `0x3C` 处。**这是指向 PE 头（NT Headers）的文件偏移量**。这是解析的第一跳。

#### 2. IMAGE_NT_HEADERS64 (NT 头)

- **位置**：`基地址 + e_lfanew`。
- **结构**：

    ```c
    struct IMAGE_NT_HEADERS64 {
        DWORD Signature;                // 0x00004550 ("PE\0\0")
        IMAGE_FILE_HEADER FileHeader;
        IMAGE_OPTIONAL_HEADER64 OptionalHeader;
    };
    ```

**2.1 IMAGE_FILE_HEADER (文件头)**

- **位置**：紧跟 Signature 之后。
- **作用**：描述文件的物理属性。
- **关键成员**：
  - `Machine`: `0x8664` (AMD64) 或 `0x14C` (i386)。
  - `NumberOfSections`: **WORD**。决定了后面要读取多少个节表项。**循环解析节表时的计数器**。
  - `SizeOfOptionalHeader`: 后面那个结构体的大小。

**2.2 IMAGE_OPTIONAL_HEADER64 (可选头)**

- **位置**：紧跟 File Header 之后。
- **作用**：PE 的灵魂，告诉 OS 加载器如何运行它。
- **关键成员**：
  - `AddressOfEntryPoint` (OEP): **RVA**。程序启动后 IP 指针指向的第一行代码（通常是 `mainCRTStartup`）。
  - `ImageBase`: **QWORD**。程序的首选加载内存地址。
  - `SectionAlignment`: 内存对齐粒度（通常 4KB / `0x1000`）。
  - `FileAlignment`: 磁盘对齐粒度（通常 512B / `0x200`）。
  - **`DataDirectory[16]`**: **这是红队最关注的数组**。它包含 16 个关键数据结构的 **地址 (RVA)** 和 **大小**。

**DataDirectory 的关键索引**：

- **[0] Export Table** (`dinvk` 解析的目标)
- **[1] Import Table** (正常 IAT)
- **[3] Exception Table** (`uwd` 需要解析的 `.pdata`)
- **[5] Base Relocation Table** (手动映射必须处理的)

#### 3. IMAGE_SECTION_HEADER (节表)

- **位置**：紧跟在 NT Headers 后面。
- **作用**：目录表。描述了真实数据（代码、变量）在哪里，以及有多大。
- **数量**：由 `FileHeader.NumberOfSections` 决定。
- **结构成员 (关键)**：
  - `Name`: 8 字节 ASCII (e.g., ".text")。注意不一定以 `\0` 结尾。
  - `VirtualSize`: 数据在内存中未对齐前的真实大小。
  - **`VirtualAddress` (RVA)**: 该节被映射到内存后的起始偏移。**ImageBase + VirtualAddress = 内存真实地址**。
  - `SizeOfRawData`: 该节在磁盘文件中对齐后的大小。
  - **`PointerToRawData`**: 该节在磁盘文件中的起始偏移。
  - **`Characteristics`**: 权限位掩码。
    - `0x20000000` (Executable)
    - `0x40000000` (Readable)
    - `0x80000000` (Writable) - **警报：**如果 .text 段有此标志，必杀。

#### 4. The Sections (具体的节内容)

这些不是结构体，而是大块的二进制数据，由上面的节表指向。

- **`.text` 段**:
  - 存放机器码 (OpCode)。
  - `Indirect Syscall` 的跳板 (`syscall; ret`) 就藏在这里面。
- **`.rdata` 段** (Read-only Data):
  - **导出表 (Export Directory)** 通常在这里。
    - `IMAGE_EXPORT_DIRECTORY` 结构体：包含 `AddressOfFunctions`, `AddressOfNames`, `AddressOfNameOrdinals` 三个并列数组。**这是 dinvk 实现 GetProcAddress 的核心数据源**。
  - **导入表 (Import Directory)** 及其 Lookup Table。
  - **异常目录 (Exception Directory)**：`RUNTIME_FUNCTION` 数组，用于栈回溯。**这是 uwd 的核心数据源**。
- **`.reloc` 段**:
  - 包含一堆数据块，告诉加载器：“如果我的加载基址变了（ASLR），请帮我把代码里 offset `0x100` 和 offset `0x500` 处的硬编码地址修改一下。”

---

### 🧱 实战案例：从 dinvk 视角看结构链

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

---

### 🧪 学习小技巧：偏移计算 (Address Conversion)

在红队开发中（特别是做 Manual Mapping 时），你必须精通 **RVA 转 FOA** 的计算：

- **RVA (Relative Virtual Address)**: 内存中的偏移。
- **FOA (File Offset Address)**: 磁盘文件中的偏移。

**转换逻辑**：

1. 遍历所有 **节表 (Section Headers)**。
2. 判断：目标 RVA 是否在 `[Section.VirtualAddress, Section.VirtualAddress + Section.VirtualSize]` 区间内？
3. 如果中：
    - `偏移量 = RVA - Section.VirtualAddress`
    - `FOA = Section.PointerToRawData + 偏移量`

掌握了这个，你就打通了“文件”与“内存”的壁垒，这是写加载器的终极内功。

## 为什么如果一个导出函数的 RVA 指向了导出目录 (Export Directory) 所在的内存范围内那么它一定不是代码，而是一个转发字符串 (Forwarder String)

逻辑：导出目录开始 <= 地址 < 导出目录结束

这是 Windows PE (Portable Executable)文件格式设计中的一个“空间优化技巧”，或者说是一个“硬性约定”。  
微软为了节省空间，没有为“是否是转发函数”单独设计一个标志位（Flag），而是复用了函数地址字段，通过判断地址范围来区分它是“真正的代码”还是“转发字符串”。

1. 正常的代码绝不可能在这个范围内导出目录 (Export Directory) 本身是一个纯数据结构（包含IMAGE_EXPORT_DIRECTORY 结构体、函数名数组、序号数组、地址数组等）  
这块区域存的全是表格数据  
真正的函数代码（汇编指令）通常位于 .text代码段，距离导出目录（通常在 .edata 或 .rdata 段）非常远。  
逻辑互斥：有效的机器码不可能“寄生”在导出表的数据结构内部。如果代码真的写在这里，它会覆盖掉导出表的数据，导致 DLL 格式损坏
2. 设计者的思路：复用 AddressOfFunctions  
当 Windows 加载器 (Loader) 解析导出表时，它会读取 AddressOfFunctions数组里的 RVA (Relative Virtual Address)。此时面临两种情况：

- 情况 A：普通导出函数我们需要指向函数的代码入口。RVA 指向代码段。
- 情况 B：转发函数 (Forwarder)我们需要一个字符串（例如"NTDLL.RtlAllocateHeap"）来告诉加载器去哪里找这个函数。

设计者不想增加额外的字段（比如加个 boolisForwarder），那样会浪费空间并破坏对齐。于是他们制定了这条规则：
> “如果在取出的 RVA 地址处，发现居然刚好落在了导出目录在这个 DLL里的地盘内，那它一定不是代码（因为代码不可能写在目录表里），而是指向了一个字符串。”
