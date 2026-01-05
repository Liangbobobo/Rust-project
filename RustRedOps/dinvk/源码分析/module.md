- [注意](#注意)
  - [什么是加载器（Loader）？](#什么是加载器loader)
    - [1. 操作系统视角：标准加载器（OS Loader）](#1-操作系统视角标准加载器os-loader)
      - [主要职责流程](#主要职责流程)
    - [2. 安全与红队视角：自定义加载器（Custom Loader）](#2-安全与红队视角自定义加载器custom-loader)
      - [为什么要自己写加载器？](#为什么要自己写加载器)
      - [常见类型](#常见类型)
    - [3. 结合你的项目（`dinvk`）：加载器的深度实现](#3-结合你的项目dinvk加载器的深度实现)
      - [第一步：解析 PE 头（PE Parsing）](#第一步解析-pe-头pe-parsing)
      - [第二步：内存分配（Allocation）](#第二步内存分配allocation)
      - [第三步：映射节（Mapping Sections）——关键](#第三步映射节mapping-sections关键)
      - [第四步：基址重定位（Base Relocation）](#第四步基址重定位base-relocation)
      - [第五步：解析导入表（Resolve Imports / IAT Fixing）](#第五步解析导入表resolve-imports--iat-fixing)
      - [第六步：权限最终设置（Finalize Protections）](#第六步权限最终设置finalize-protections)
      - [第七步：执行（Execution）](#第七步执行execution)
    - [4. 项目结构与加载器功能的映射](#4-项目结构与加载器功能的映射)
    - [5. 为什么这被称为 “RedOps”？](#5-为什么这被称为-redops)
  - [此项目找DLL 基址的原理](#此项目找dll-基址的原理)
  - [TEB](#teb)
  - [PE和PEB的三大核心联系](#pe和peb的三大核心联系)
    - [联系一:定位主模块基址 (PEB.ImageBaseAddress)](#联系一定位主模块基址-pebimagebaseaddress)
    - [联系二:管理所有加载的 PE 模块 (PEB.Ldr)](#联系二管理所有加载的-pe-模块-pebldr)
    - [联系三: 数据目录的运行时访问（Data Directories）](#联系三-数据目录的运行时访问data-directories)
  - [PE](#pe)
    - [本项目没有定义一个把所有字段打包在一起的单一 `PE`结构体？](#本项目没有定义一个把所有字段打包在一起的单一-pe结构体)
      - [dinvk 的做法：通过指针和偏移量访问](#dinvk-的做法通过指针和偏移量访问)
    - [PE (Memory Layout)](#pe-memory-layout)
    - [PE文件结构](#pe文件结构)
      - [PE 文件结构详解：基于 `dinvk` 项目中的数据结构](#pe-文件结构详解基于-dinvk-项目中的数据结构)
      - [第一部分：DOS Header（兼容性头部）](#第一部分dos-header兼容性头部)
      - [第二部分：NT Headers（PE 核心头）](#第二部分nt-headerspe-核心头)
        - [2.1 File Header（文件物理概况）](#21-file-header文件物理概况)
        - [2.2 Optional Header（逻辑加载信息）](#22-optional-header逻辑加载信息)
      - [2.3 Data Directory（功能索引表）](#23-data-directory功能索引表)
      - [`IMAGE_EXPORT_DIRECTORY->AddressOfNames` 的内存归属解析](#image_export_directory-addressofnames-的内存归属解析)
        - [1. **物理归属：它是目标 DLL 的一部分**](#1-物理归属它是目标-dll-的一部分)
        - [2. **内存位置：位于当前进程的虚拟地址空间内**](#2-内存位置位于当前进程的虚拟地址空间内)
        - [3. **技术细节修正：它是一个“RVA 数组”，而非“字符串数组”**](#3-技术细节修正它是一个rva-数组而非字符串数组)
        - [总结](#总结)
      - [第三部分：Section Headers（节表）](#第三部分section-headers节表)
      - [总结：Windows PE 加载流程](#总结windows-pe-加载流程)
  - [PEB](#peb)
    - [一个可执行文件产生多个进程时PEB是怎么样的?](#一个可执行文件产生多个进程时peb是怎么样的)
    - [LDR](#ldr)
    - [PEB\_LDR\_DATA](#peb_ldr_data)
    - [InMemoryOrderModuleList](#inmemoryordermodulelist)
    - [LDR\_DATA\_TABLE\_ENTRY](#ldr_data_table_entry)
      - [LDR\_DATA\_TABLE\_ENTRY如何产生并维护?](#ldr_data_table_entry如何产生并维护)
        - [产生 (Creation)](#产生-creation)
        - [维护 (Maintenance)](#维护-maintenance)
        - [销毁 (Destruction)](#销毁-destruction)
    - [ImageBaseAddress](#imagebaseaddress)
  - [let mut data\_table\_entry = (*ldr\_data).InMemoryOrderModuleList.Flink as*const LDR\_DATA\_TABLE\_ENTRY](#let-mut-data_table_entry--ldr_datainmemoryordermodulelistflink-asconst-ldr_data_table_entry)
    - [物理事实 (Memory Reality)](#物理事实-memory-reality)
    - [编译器的“错觉” (Compiler's View)](#编译器的错觉-compilers-view)
    - [结果：偏移量的“平移”](#结果偏移量的平移)
    - [总结](#总结-1)
  - [指令集架构](#指令集架构)
    - [1. ARM 架构（AArch64 / ARM64）](#1-arm-架构aarch64--arm64)
    - [2. RISC-V](#2-risc-v)
    - [3. WebAssembly（Wasm）](#3-webassemblywasm)
    - [4. x86（32-bit）/ i686](#4-x8632-bit-i686)
    - [5. 其他嵌入式与专用架构](#5-其他嵌入式与专用架构)
    - [总结：CISC vs RISC](#总结cisc-vs-risc)
  - [EXE 文件和 PE 文件的关系？除了 EXE 还有哪些 PE 文件？](#exe-文件和-pe-文件的关系除了-exe-还有哪些-pe-文件)
    - [2. 除了 EXE，还有哪些常见的 PE 文件？](#2-除了-exe还有哪些常见的-pe-文件)
      - [A. 动态链接库（DLL）](#a-动态链接库dll)
      - [B. 驱动程序（Drivers）](#b-驱动程序drivers)
      - [C. 其他系统/功能文件](#c-其他系统功能文件)
    - [3. 如何区分不同类型的 PE 文件？](#3-如何区分不同类型的-pe-文件)
    - [1. EXE（Executable）—— 独立的执行主体](#1-exeexecutable-独立的执行主体)
    - [2. DLL（Dynamic Link Library）—— 共享的代码仓库](#2-dlldynamic-link-library-共享的代码仓库)
    - [3. SYS（System Driver）—— 内核的延伸](#3-syssystem-driver-内核的延伸)
    - [4. EFI（Extensible Firmware Interface）—— 系统的启动者](#4-efiextensible-firmware-interface-系统的启动者)
    - [5. SCR（Screen Saver）—— 特殊用途的 EXE](#5-scrscreen-saver-特殊用途的-exe)
    - [6. OCX / CPL —— 插件式架构的实现者](#6-ocx--cpl--插件式架构的实现者)
  - [总结：为何需要如此多样的 PE 格式？](#总结为何需要如此多样的-pe-格式)
  - [源码](#源码)
    - [CStr::from\_ptr(ptr).to\_string\_lossy().into\_owned()](#cstrfrom_ptrptrto_string_lossyinto_owned)
      - [1. `CStr::from_ptr(ptr)`：封装原始指针，创建安全视图](#1-cstrfrom_ptrptr封装原始指针创建安全视图)
      - [2. `.to_string_lossy()`：容错式 UTF-8 转换](#2-to_string_lossy容错式-utf-8-转换)
      - [3. `.into_owned()`：夺取所有权，实现深拷贝](#3-into_owned夺取所有权实现深拷贝)
      - [在 `dinvk` 项目中的具体意义](#在-dinvk-项目中的具体意义)
      - [一句话总结](#一句话总结)

# 注意

请记得所有关于内存及windows可执行文件的操作,都可以用windbg查看在内存中的直观显示.  

请记住,一定要使用windbg,这甚至比写代码更加重要

本项目中:  
TEB->PEB->LDR(裸指针)->PEB_LDR_DATA->InMemoryOrderModuleList(双向链表)->LDR_DATA_TABLE_ENTRY

## 什么是加载器（Loader）？

在计算机科学和操作系统领域，**加载器（Loader）** 是一个极其核心的组件。结合你当前的 `RustRedOps/dinvk` 项目（一个涉及 Windows 系统底层、直接系统调用和红队技术的项目），我们需要从 **操作系统原理** 和 **安全攻防（Red Teaming）** 两个角度来深入理解它。

> **简单来说，加载器的作用是：将可执行程序（静态文件）从磁盘“搬运”到内存中，并将其变成可运行的进程（动态实体）。**

---

### 1. 操作系统视角：标准加载器（OS Loader）

当你双击一个 `.exe` 文件（Windows）或运行一个 ELF 程序（Linux）时，操作系统的加载器开始工作。它是操作系统内核或用户空间运行时的一部分。

#### 主要职责流程

1. **验证与解析（Validation & Parsing）**  
   - 读取文件头（如 Windows 的 PE 头），检查文件格式是否合法。
   - 解析节（Section）表、导入表（IAT）、导出表（EAT）、重定位表（`.reloc`）等结构。

2. **内存映射（Mapping）**  
   - 在虚拟内存中为程序申请地址空间。
   - 将磁盘上的代码段（`.text`）、数据段（`.data`）、只读数据段（`.rdata`）等按页映射到内存中的相应虚拟地址。

3. **重定位（Relocation）**  
   - 由于 ASLR（地址空间布局随机化）的存在，程序每次加载的基地址都不同。
   - 加载器会遍历重定位表，修正所有硬编码的绝对地址，使其指向当前实际加载位置。

4. **符号解析与导入（Import Resolution）**  
   - 程序通常依赖外部 DLL（如 `kernel32.dll`, `ntdll.dll`）。
   - 加载器遍历导入地址表（IAT），加载所需 DLL，并将函数的真实地址填入 IAT，使程序能正常调用 API（如 `WriteFile`, `CreateThread`）。

5. **权限设置（Permission Setting）**  
   - 设置各内存页的访问权限：
     - 代码段 → RX（可读、可执行）
     - 数据段 → RW（可读、可写）
     - 避免 RWX（可读、可写、可执行），因其是恶意软件的典型特征。

6. **移交控制权（Execution）**  
   - 初始化栈（Stack）和堆（Heap）。
   - 跳转到程序入口点（Entry Point），通常是 CRT 启动代码，最终进入 `main()` 或 `WinMain()`。

---

### 2. 安全与红队视角：自定义加载器（Custom Loader）

在你的 `RustRedOps/dinvk` 上下文中，“加载器”通常指 **Malware Loader** 或 **Shellcode Loader**。

红队开发人员编写自定义加载器的核心目的：**绕过杀毒软件（AV）和终端检测响应系统（EDR）的监控**。

#### 为什么要自己写加载器？

- 标准的 Windows 加载路径（如 `CreateProcess`, `LoadLibrary`）会被 EDR 深度 Hook。
- 自定义加载器试图在不触发警报的情况下，将恶意 Payload 注入内存并执行。

#### 常见类型

1. **Shellcode Loader**  
   - 最简单形式：调用 `VirtualAlloc` 分配 RWX 内存，复制 Shellcode，然后通过 `CreateThread` 执行。
   - 缺点：RWX 内存极易被检测。

2. **反射式 DLL 注入（Reflective DLL Injection）**  
   - 高级技术：完全在内存中实现 PE 加载全过程（分配、映射、重定位、解析导入表）。
   - **文件不落地（Fileless）**：DLL 从未写入磁盘，仅存在于内存，规避文件扫描。
   - 不调用 `LoadLibrary`，避免触发 API Hook。

3. **Direct Syscall Loader（与 `dinvk` 高度相关）**  
   - 标准 API（如 `NtAllocateVirtualMemory`）位于 `ntdll.dll`，而 EDR 会在其入口处安装用户态 Hook。
   - `dinvk` 这类项目通过 **直接执行 `syscall` 指令** 调用内核函数，绕过 `ntdll.dll`，从而避开 Hook。
   - 实现真正的“静默”内存操作。

---

### 3. 结合你的项目（`dinvk`）：加载器的深度实现

在红队开发语境下，“写一个加载器”本质上是在 **手动模拟 Windows 内核加载 PE 文件的过程**，即 **反射式加载（Reflective Loading）** 或 **手动映射（Manual Mapping）**。

以下是加载器工作的核心步骤——这正是你的 `module.rs` 和 `syscall/` 目录下代码正在实现的内容：

#### 第一步：解析 PE 头（PE Parsing）

- 读取 DOS 头（`MZ`）验证文件合法性。
- 解析 NT 头（`PE\0\0`），获取关键字段：
  - `ImageBase`：期望加载基址
  - `SizeOfImage`：所需虚拟内存大小
  - `AddressOfEntryPoint`：程序入口 RVA
- 遍历节表（Section Headers），获取 `.text`, `.data`, `.rdata` 等节的：
  - `VirtualAddress`（内存偏移）
  - `SizeOfRawData` / `Misc.VirtualSize`
  - `PointerToRawData`（文件偏移）

> ⚠️ 注意：磁盘对齐（通常 512B）≠ 内存对齐（通常 4KB）。

#### 第二步：内存分配（Allocation）

- **标准方式**：调用 `VirtualAlloc`
- **你的做法（`dinvk`）**：通过 `syscall` 直接调用 `NtAllocateVirtualMemory`
  - 申请大小 = `SizeOfImage`
  - 初始权限 = `PAGE_READWRITE`（RW），便于写入数据

#### 第三步：映射节（Mapping Sections）——关键

- 不能直接 `memcpy` 整个文件！必须按节逐个映射。
- 对每个节：
  - 从文件偏移 `PointerToRawData` 读取原始数据
  - 写入内存基址 + `VirtualAddress` 处
- 未初始化数据（如 `.bss`）需显式清零

#### 第四步：基址重定位（Base Relocation）

- **问题**：代码中可能有硬编码地址（如 `CALL 0x180001050`），但实际加载地址 ≠ `ImageBase`
- **解决**：
  1. 计算 Delta = 实际基址 - `ImageBase`
  2. 遍历 `.reloc` 节中的重定位块
  3. 对每个需要修正的地址（通常是 64 位指针），读取原值，加上 Delta，写回
- 注意：64 位程序多用 RIP 相对寻址，重定位项较少，但全局变量仍需处理

#### 第五步：解析导入表（Resolve Imports / IAT Fixing）

- Payload 依赖 Windows API（如 `CreateFileW`）
- **标准方式**：`LoadLibrary` + `GetProcAddress`
- **你的隐蔽方式（`dinvk`）**：
  - 遍历 PEB → `Ldr.InMemoryOrderModuleList`，找到已加载的 `ntdll.dll`, `kernel32.dll` 等
  - 手动解析其导出表（EAT），查找函数名哈希或字符串匹配
  - 将真实函数地址填入 Payload 的 IAT
- **优势**：完全不调用被监控的 API，实现“无痕”解析

#### 第六步：权限最终设置（Finalize Protections）

- 使用 `NtProtectVirtualMemory`（通过 syscall）修改内存权限：
  - `.text` → `PAGE_EXECUTE_READ`（RX）
  - `.data` / `.rdata` → `PAGE_READWRITE`（RW）或 `PAGE_READONLY`
- **关键反检测技巧**：绝不使用 `PAGE_EXECUTE_READWRITE`（RWX）

#### 第七步：执行（Execution）

- 计算实际入口点：`ActualEntry = BaseAddress + AddressOfEntryPoint`
- 创建线程：通过 `NtCreateThreadEx`（syscall）启动新线程
- 或直接跳转：`jmp ActualEntry`（需小心栈和上下文）

---

### 4. 项目结构与加载器功能的映射

回到你的 `dinvk` 项目目录，各模块的角色如下：

| 文件/模块             | 对应加载器功能                          | 说明 |
|----------------------|----------------------------------------|------|
| `src/module.rs`       | PE 解析、重定位、导入表解析            | 实现反射式加载的核心逻辑，包括 EAT 遍历以替代 `GetProcAddress` |
| `src/syscall/`        | 直接系统调用封装                       | 绕过 `ntdll.dll` Hook，安全调用 `NtAllocateVirtualMemory`、`NtProtectVirtualMemory` 等 |
| `src/winapis.rs`      | Windows 内部结构体定义                 | 定义 `IMAGE_DOS_HEADER`、`LDR_DATA_TABLE_ENTRY`、`PEB` 等非公开结构 |
| `src/allocator.rs`    | 自定义内存分配策略                     | 可能用于隔离 Payload 堆内存，避免混入默认进程堆 |

---

### 5. 为什么这被称为 “RedOps”？

普通软件开发无需如此复杂——直接 `std::process::Command::new("app.exe")` 即可。

但红队场景下，你必须这么做，因为：

1. **文件不落地（Fileless）**  
   Payload 可加密嵌入资源、从网络下载、或硬编码在二进制中，全程不写磁盘，规避传统 AV。

2. **规避钩子（Unhooking / Bypassing）**  
   通过 Direct Syscall 绕过 EDR 在用户态 DLL 中的 Hook，实现“干净”的内核调用。

3. **内存伪装（Obfuscation）**  
   控制内存布局、权限、内容，使 Payload 在内存中看起来像普通数据，仅在执行瞬间激活。

---

> **总结**：  
> 对于你的 `dinvk` 项目而言，**加载器就是一个由你编写的 Rust 程序，它手动解析 PE 文件结构，使用直接系统调用（Direct Syscalls）申请内存并写入代码，最后在不被 EDR 察觉的情况下执行目标 Payload。**  
> 你现在不是在写应用，而是在打造一辆“隐形战车”的引擎——静默、精准、穿透防御。

## 此项目找DLL 基址的原理

遍历模块链表以获取基址的核心流程如下：

1. **获取 PEB**  
   - x64: `gs:[0x60]`
   - x86: `fs:[0x30]`

2. **进入加载器数据**  
   从 PEB 读取 `Ldr` 字段，得到 `PEB_LDR_DATA*`。

3. **选择链表**  
   通常使用 `InMemoryOrderModuleList`。

4. **遍历链表**  
   - 从 `Flink` 获取下一个节点地址（该地址 = 目标 `LDR_DATA_TABLE_ENTRY` 的 `InMemoryOrderLinks` 地址）。
   - 利用“错位指针”技巧，将该地址**强制视为** `LDR_DATA_TABLE_ENTRY*`。
   - 访问结构体字段（如 `Reserved2[0]` 实际读取的是 `DllBase`）。
   - 比对 `FullDllName` 或 `BaseDllName` 是否为目标 DLL。
   - 若匹配，返回 `DllBase` —— 即该 DLL 的内存基址。

> ✅ 这种方法绕过了 `GetModuleHandle` 等 API，具有隐蔽性，常用于免杀或底层工具开发。

## TEB

## PE和PEB的三大核心联系

### 联系一:定位主模块基址 (PEB.ImageBaseAddress)

这是最直接的联系。

- PE 层面: PE 文件的 OptionalHeader.ImageBase 只是一个建议地址（例如0x140000000）。
- 运行时: 由于 ASLR（地址空间布局随机化），PE 文件实际上可能被加载到任意地址（例如0x7FF712340000）。
- PEB 层面: PEB.ImageBaseAddress 存储了最终实际加载的基址。

操作逻辑:  
程序通过读取 PEB.ImageBaseAddress，得到了内存中 PE 文件的头部（DOS
Header）地址，从而开始解析整个 PE 结构。

### 联系二:管理所有加载的 PE 模块 (PEB.Ldr)

一个进程通常由多个 PE 文件组成（1 个 EXE + N 个 DLL）。PEB 通过 Ldr 字段维护这些 PE 文件的链表。

系统负责“加载”和“维护”数据，而这个项目负责“手动读取”和“解析”数据  
真正的 PE 结构由系统维护：  
当程序启动或 DLL 被加载时，Windows 操作系统（加载器 Loader）负责将 PE 文件从磁盘映射到内存中。系统会维护关键的内核与用户态结构，例如 **PEB**（Process Environment Block）和 **LDR 链表**（`InMemoryOrderModuleList`），这些结构完整记录了当前进程中所有已加载模块的基地址、文件路径、大小等元信息。你的项目并没有“凭空捏造”一个 PE 结构，而是直接读取操作系统已经加载到内存中的那个**真实的 PE 映像**——它就存在于进程的虚拟地址空间里，只是通常被高级 API（如 `GetModuleHandle`）封装起来而已。

代码通过“模拟”加载器行为来寻找函数：  
通常情况下，程序员只需调用 `GetProcAddress`，让系统帮我们完成函数地址的查找。但在 `src/module.rs` 中，代码完全绕过了标准 Windows API，自己重新实现了一整套底层查找逻辑：

- **获取基址**：通过内联汇编读取 x64 架构下的 `GS` 寄存器（或 x86 下的 `FS` 寄存器），定位到当前线程的 TEB，进而找到 PEB；再遍历 PEB 中的 `Ldr->InMemoryOrderModuleList` 链表，匹配目标模块名（如 `"kernel32.dll"`），从而获得其真实加载基址。
- **手动解析 PE 结构**：拿到基址后，代码将该地址视为字节数组的起点，严格按照 PE 文件格式规范，依次解析：
  - `IMAGE_DOS_HEADER`
  - `IMAGE_NT_HEADERS`（含可选头）
  - `IMAGE_DATA_DIRECTORY[IMAGE_DIRECTORY_ENTRY_EXPORT]`
  - 最终定位到 `IMAGE_EXPORT_DIRECTORY`
- **计算函数地址**：根据导出目录中的 `AddressOfNames`、`AddressOfNameOrdinals` 和 `AddressOfFunctions` 三个 RVA 数组，手动执行二分查找或线性遍历，将函数名映射到实际的导出 RVA，并加上模块基址得到最终虚拟地址。
- **处理转发机制**：甚至对 Windows 的 **API Set**（如 `api-ms-win-core-file-l1-2-0.dll` 这类虚拟 DLL）也做了深度支持——通过解析 `PEB.ApiSetMap` 结构，还原出虚拟名称到真实 DLL（如 `kernelbase.dll`）的映射关系，从而正确解析转发条目。

为什么要这么做？  
这通常是为了**规避 EDR**（终端检测与响应系统）。现代安全软件普遍会对 `GetProcAddress`、`LoadLibrary` 等关键 API 进行 **User-land Hook** 监控，一旦发现程序试图获取敏感函数（如 `NtCreateThreadEx`、`VirtualAlloc`），就会触发告警或阻断。而通过这种“手动解析内存中 PE 结构”的方式（属于 **Reflective Loading / Manual Mapping** 技术的核心环节），程序可以悄无声息地获取任意函数地址，全程不调用任何被监控的系统 API，从而有效绕过用户态钩子，实现隐蔽执行——这正是红队工具和高级加载器（如你的 `dinvk` 项目）追求的核心能力。

```rust
// src/types.rs
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

#[repr(C)]
//与verg中_LDR_DATA_TABLE_ENTRY相对应
pub struct LDR_DATA_TABLE_ENTRY {
    pub Reserved1: [*mut c_void; 2],//该字段大小16字节,因为在repr(c)模式下64 bit os一个指针占用8字节
    pub InMemoryOrderLinks: LIST_ENTRY,
    pub Reserved2: [*mut c_void; 2],
    pub DllBase: *mut c_void,//目标模块（通常是ntdll.dll 或 kernel32.dll）在内存中的起始地址（DllBase）
    pub Reserved3: [*mut c_void; 2],
    pub FullDllName: UNICODE_STRING,
    pub Reserved4: [u8; 8],//此处位置0x58,这里是占位写法,实际上这里对应的是 struct _UNICODE_STRING BaseDllName;
    pub Reserved5: [*mut c_void; 3],
    pub Anonymous: LDR_DATA_TABLE_ENTRY_0,
    pub TimeDateStamp: u32,
}
```

1. **访问 `PEB.Ldr`**  
   获取当前进程的 `PEB_LDR_DATA` 结构。
2. **遍历 `InMemoryOrderModuleList`**  
   该双向链表按模块在内存中的布局顺序组织。
3. **每个节点是一个 `LDR_DATA_TABLE_ENTRY`**  
   描述一个已加载的 PE 模块（EXE 或 DLL）。
4. **读取 `DllBase` 字段**  
   得到该模块在进程地址空间中的基地址。
5. **`DllBase` 即为 `IMAGE_DOS_HEADER` 的地址**  
   可从此处开始解析完整的 PE 结构。
6. **解析导出表以获取函数地址**  
   例如，可手动定位 `kernel32!LoadLibraryA` 的运行时地址。

### 联系三: 数据目录的运行时访问（Data Directories）

PE 文件中的 **数据目录**（如导出表、导入表、资源表等）使用的是 **RVA**（Relative Virtual Address，相对虚拟地址）。

> ⚠️ RVA 仅是一个偏移量，不能直接用于内存访问。必须结合模块基址转换为 **VA**（Virtual Address）。

地址转换公式：  
> **VA = 模块基址（来自 PEB/Ldr） + RVA（来自 PE 头）**

示例场景：手动查找导出函数地址

1. **通过 PEB 定位模块**  
   遍历 `PEB.Ldr` 找到 `kernel32.dll` 的基址 `base_addr`。
2. **定位 NT 头**  
   从 `base_addr` 读取 `IMAGE_DOS_HEADER`，通过 `e_lfanew` 字段跳转到 `IMAGE_NT_HEADERS`。
3. **获取导出表 RVA**  
   读取 `OptionalHeader.DataDirectory[0]`（索引 0 对应导出表），得到 `export_rva`。
4. **计算导出表真实地址**  
   `export_table_va = base_addr + export_rva`
5. **解析导出表**  
   遍历函数名数组，匹配目标函数（如 `"LoadLibraryA"`），获取其 `function_rva`。
6. **计算函数最终地址**  
   `function_address = base_addr + function_rva`

> ✅ **结论**：  
> **PEB 是动态的地图**，指引你在进程内存中定位每一个静态的 PE 文件块。  
> 没有 PEB，内存中的 PE 模块就如同没有索引的图书馆书籍——存在却无法高效使用。

## PE

PE (Portable Executable) 和 PEB (Process Environment Block) 是 Windows操作系统中两个核心概念，它们分别代表了程序的静态存储形态和动态运行形态。  
PEB 记录了 PE 文件被加载到内存后的关键信息，是访问和解析内存中PE 结构的入口。

PE 文件本质上是 **“线性存储的数据结构集合”**。在磁盘上和在内存中，它们的逻辑顺序一致，但物理间距（对齐）不同。  
PE 是 Windows 可执行文件（.exe, .dll,.sys）的标准格式。它描述了代码、数据、资源在文件中如何组织，以及加载到内存时应该如何映射。

在本项目中，调用系统调用（syscall）或关键 API 的过程是纯手动的，不依赖操作系统提供的标准加载器（Loader）功能。  

1. 定义数据结构（Mapping）：
      Rust 代码在 src/types.rs 中使用 #[repr(C)] 精确复刻了 Windows PE
  文件的内存布局。这意味着 Rust 结构体的内存分布与 Windows
  内核和硬件看到的二进制数据完全一致。

   1. 获取基址（Base Address）：
      代码首先通过读取 TEB (Thread Environment Block) -> PEB (Process Environment Block) -> Ldr (Loader Data) 链表，找到目标模块（通常是ntdll.dll 或 kernel32.dll）在内存中的起始地址（DllBase）。

   2. 解析 PE 结构（Parsing）：
      利用第 1 步定义的结构体，代码将这个基址强转为 *const
  IMAGE_DOS_HEADER 指针，通过 e_lfanew 找到 IMAGE_NT_HEADERS，再访问
  OptionalHeader.DataDirectory 找到 导出表 (Export Directory)。

   3. 查找函数（Resolution）：
      遍历导出表中的函数名称数组（AddressOfNames），找到目标函数（例如NtAllocateVirtualMemory 或 LoadLibraryA）。

   4. 获取地址或 SSN（Extraction）：
       - 对于 API
         调用：从导出表中获取该函数的内存地址，将其强转为函数指针（如
         LoadLibraryAFn）并直接调用。
       - 对于 Syscall：解析 ntdll.dll 中对应函数的汇编代码（通常是 mov eax, SSN; syscall），提取出 SSN (System Service Number)。

   5. 执行调用（Execution）：
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

### PE文件结构

为了避免依赖庞大的 windows-sys 或 winapi crate，选择手动定义了这些底层结构。这种做法在恶意软件开发或红队工具（如你正在开发的RustRedOps）中非常常见，目的是为了减少特征指纹?为什么、减小二进制体积?为什么以及拥有更精细的控制权。

#### PE 文件结构详解：基于 `dinvk` 项目中的数据结构

PE（Portable Executable）是 Windows 系统下可执行文件（EXE）、动态链接库（DLL）、驱动（SYS）等的标准格式。本文以开源项目 **`dinvk`** 中 `src/types.rs` 定义的 Rust 结构体为蓝本，**从磁盘文件偏移 0 开始**，逐字段、逐结构地详细解释 PE 文件中每一个字段的含义与作用。

---

#### 第一部分：DOS Header（兼容性头部）

位于文件起始位置（**Offset 0**），共 64 字节（0x40）。其存在是为了向后兼容 MS-DOS 系统。

```rust
#[repr(C, packed(2))]
pub struct IMAGE_DOS_HEADER {
    pub e_magic: u16,    // [0x00] 魔数 "MZ" (0x5A4D)
                         // 标识这是一个 DOS 可执行文件。
                         // Windows 加载器首先检查此值，非 MZ 则拒绝加载。

    pub e_cblp: u16,     // [0x02] 文件最后一页中的字节数（< 512）
                         // DOS 时代用于计算实际文件大小，现代忽略。

    pub e_cp: u16,       // [0x04] 文件总页数（每页 512 字节）
                         // 实际大小 ≈ (e_cp - 1) * 512 + e_cblp

    pub e_crlc: u16,     // [0x06] 重定位项数量
                         // DOS 程序加载时需根据此修正地址，PE 文件中通常为 0。

    pub e_cparhdr: u16,  // [0x08] 头部大小（以 16 字节“段”为单位）
                         // 即 DOS 头 + DOS Stub 的总段数。例如值为 4 表示 64 字节。

    pub e_minalloc: u16, // [0x0A] 程序运行所需的最小额外内存（段）
    pub e_maxalloc: u16, // [0x0C] 程序可申请的最大额外内存（段）

    pub e_ss: u16,       // [0x0E] 初始 SS（堆栈段寄存器值）
    pub e_sp: u16,       // [0x10] 初始 SP（堆栈指针）
                         // DOS 启动时设置堆栈位置。

    pub e_csum: u16,     // [0x12] 校验和（通常为 0，未使用）

    pub e_ip: u16,       // [0x14] 初始 IP（指令指针）
    pub e_cs: u16,       // [0x16] 初始 CS（代码段寄存器值）
                         // DOS 入口点 = CS:IP

    pub e_lfarlc: u16,   // [0x18] 重定位表在文件中的偏移（DOS 用）

    pub e_ovno: u16,     // [0x1A] 覆盖号（Overlay Number）
                         // 用于 DOS 的覆盖管理机制（类似分段加载）

    pub e_res: [u16; 4], // [0x1C] 保留字段（共 8 字节），必须为 0

    pub e_oemid: u16,    // [0x24] OEM 标识符（特定厂商扩展）
    pub e_oeminfo: u16,  // [0x26] OEM 信息（配合 e_oemid 使用）

    pub e_res2: [u16; 10], // [0x28] 保留字段（共 20 字节），必须为 0

    pub e_lfanew: i32,   // [0x3C] ★关键字段★
                         // 指向 NT Headers 的**文件偏移量**（File Offset）。
                         // Windows 加载器读取此值跳转到真正的 PE 头部。
}
```

> 💡 **DOS Stub**  
> 在 `IMAGE_DOS_HEADER` 和 `NT Headers` 之间通常嵌入一段小 DOS 程序（如打印 “This program cannot be run in DOS mode”）。  
> 此部分对现代 PE 解析无实质意义，`dinvk` 未定义其结构。

---

#### 第二部分：NT Headers（PE 核心头）

通过 `e_lfanew` 跳转到达。包含签名、文件头和可选头三部分，是解析 PE 的核心。

```rust
#[repr(C)]
pub struct IMAGE_NT_HEADERS {
    pub Signature: u32,  // [0x00] 固定为 "PE\0\0" (0x00004550)
                         // 第二道合法性校验。若非此值，则不是有效 PE 文件。

    pub FileHeader: IMAGE_FILE_HEADER,         // 描述物理属性
    pub OptionalHeader: IMAGE_OPTIONAL_HEADER64, // 描述加载与运行时行为
}
```

##### 2.1 File Header（文件物理概况）

描述目标平台、节区数量等基本信息，共 20 字节。

```rust
#[repr(C)]
pub struct IMAGE_FILE_HEADER {
    pub Machine: IMAGE_FILE_MACHINE, // [0x04] 目标 CPU 架构
                                     // 常见值：
                                     // - 0x014C = i386 (x86)
                                     // - 0x8664 = AMD64 (x86_64)
                                     // - 0xAA64 = ARM64
                                     // - 0x0200 = IA64

    pub NumberOfSections: u16,       // [0x06] 节（Section）数量
                                     // 决定后续 Section Headers 数组长度。
                                     // 通常 2~10 个（如 .text, .data, .rdata, .pdata）

    pub TimeDateStamp: u32,          // [0x08] 编译时间戳（Unix 时间，秒）
                                     // 自 1970-01-01 00:00:00 UTC 起的秒数。
                                     // 可重现构建（Reproducible Build）常设为 0。

    pub PointerToSymbolTable: u32,   // [0x0C] COFF 符号表文件偏移
                                     // 调试信息，发布版通常为 0。

    pub NumberOfSymbols: u32,        // [0x10] 符号数量（配合上一字段）

    pub SizeOfOptionalHeader: u16,   // [0x14] OptionalHeader 的大小（字节）
                                     // - x86 PE32: 0xE0 (224)
                                     // - x64 PE32+: 0xF0 (240)

    pub Characteristics: IMAGE_FILE_CHARACTERISTICS, // [0x16] 文件属性标志（位掩码）
                                                     // 常见标志：
                                                     // - 0x0001 = 重定位信息已移除
                                                     // - 0x0002 = 可执行文件（EXE）
                                                     // - 0x2000 = DLL
                                                     // - 0x0100 = 32 位机器
                                                     // - 0x0020 = 行号信息已移除
}
```

##### 2.2 Optional Header（逻辑加载信息）

虽名为 “Optional”，但对 EXE/DLL 是**必须存在**的。此处以 64 位版本（`IMAGE_OPTIONAL_HEADER64`）为例。

```rust
#[repr(C, packed(4))]
pub struct IMAGE_OPTIONAL_HEADER64 {
    // --- 标准字段 (Standard Fields) ---
    pub Magic: IMAGE_OPTIONAL_HEADER_MAGIC, // [0x18]
                                            // - 0x10B = PE32 (32-bit)
                                            // - 0x20B = PE32+ (64-bit)

    pub MajorLinkerVersion: u8,  // [0x1A] 链接器主版本号
    pub MinorLinkerVersion: u8,  // [0x1B] 链接器次版本号
                                 // 仅作参考，不影响加载

    pub SizeOfCode: u32,         // [0x1C] 所有含代码节的总大小（如 .text）
                                 // 按 SectionAlignment 对齐后的总和

    pub SizeOfInitializedData: u32,   // [0x20] 已初始化数据节总大小（如 .data, .rdata）

    pub SizeOfUninitializedData: u32, // [0x24] 未初始化数据大小（.bss）
                                      // 该节在磁盘上无数据，仅占内存空间

    pub AddressOfEntryPoint: u32, // [0x28] ★程序入口点 RVA★
                                  // 进程启动后，RIP = ImageBase + 此值
                                  // DLL 的 DllMain 地址也在此指定

    pub BaseOfCode: u32,         // [0x2C] 代码节起始 RVA（仅参考）

    // 注意：64 位结构中无 BaseOfData 字段（32 位有）

    // --- NT 特定字段 (NT Specific Fields) ---
    pub ImageBase: u64,          // [0x30] 建议加载基址（Preferred Load Address）
                                 // - EXE 默认：0x140000000 (x64)
                                 // - DLL 默认：0x180000000 或高位地址
                                 // 若被占用，则触发重定位（ASLR）

    pub SectionAlignment: u32,   // [0x38] 内存中节对齐粒度（通常 0x1000 = 4KB）
                                 // 必须 ≥ FileAlignment

    pub FileAlignment: u32,      // [0x3C] 文件中节对齐粒度（通常 0x200 = 512B）
                                 // 必须是 2 的幂，且 ≥ 512

    pub MajorOperatingSystemVersion: u16, // [0x40] 所需最低 OS 主版本
    pub MinorOperatingSystemVersion: u16, // [0x42] 次版本
                                          // 如 Win10 = 10.0

    pub MajorImageVersion: u16,  // [0x44] 程序自定义主版本（由开发者设置）
    pub MinorImageVersion: u16,  // [0x46] 自定义次版本

    pub MajorSubsystemVersion: u16, // [0x48] 子系统主版本（如 Console, GUI）
    pub MinorSubsystemVersion: u16, // [0x4A] 子系统次版本

    pub Win32VersionValue: u32,  // [0x4C] 保留字段，必须为 0

    pub SizeOfImage: u32,        // [0x50] 内存映像总大小
                                 // 从 ImageBase 到最后一个节结束，
                                 // 按 SectionAlignment 向上对齐

    pub SizeOfHeaders: u32,      // [0x54] 所有头部总大小（DOS+NT+SectionHeaders）
                                 // 按 FileAlignment 向上对齐

    pub CheckSum: u32,           // [0x58] 校验和
                                 // 驱动（.sys）必须有效，普通 EXE 可为 0

    pub Subsystem: IMAGE_SUBSYSTEM, // [0x5C] 子系统类型
                                    // - 2 = GUI (图形界面)
                                    // - 3 = CUI (控制台/命令行)
                                    // - 10 = EFI Application

    pub DllCharacteristics: IMAGE_DLL_CHARACTERISTICS, // [0x5E] 安全特性标志（位掩码）
                                                       // - 0x0040 = ASLR 支持（动态基址）
                                                       // - 0x0100 = DEP/NX（数据不可执行）
                                                       // - 0x0400 = 强制完整性检查
                                                       // - 0x4000 = 无 SEH（/SAFESEH:NO）

    pub SizeOfStackReserve: u64, // [0x60] 栈保留虚拟内存大小（默认 1MB）
    pub SizeOfStackCommit: u64,  // [0x68] 栈初始提交物理内存（默认 4KB）

    pub SizeOfHeapReserve: u64,  // [0x70] 堆保留大小（默认 1MB）
    pub SizeOfHeapCommit: u64,   // [0x78] 堆初始提交大小（默认 4KB）

    pub LoaderFlags: u32,        // [0x80] 已废弃，应为 0

    pub NumberOfRvaAndSizes: u32,// [0x84] DataDirectory 数组元素个数
                                 // 通常为 16，表示支持 16 种数据目录

    // --- 数据目录（功能索引表）---
    pub DataDirectory: [IMAGE_DATA_DIRECTORY; 16], // [0x88]
}
```

#### 2.3 Data Directory（功能索引表）

每个条目是一个指向关键功能表的 RVA 和大小。

```rust
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IMAGE_EXPORT_DIRECTORY {
    pub Characteristics: u32,
    pub TimeDateStamp: u32,
    pub MajorVersion: u16,
    pub MinorVersion: u16,
    pub Name: u32,
    pub Base: u32,
    pub NumberOfFunctions: u32,
    pub NumberOfNames: u32,
    pub AddressOfFunctions: u32,
    pub AddressOfNames: u32,
    pub AddressOfNameOrdinals: u32,
}
```

#### `IMAGE_EXPORT_DIRECTORY->AddressOfNames` 的内存归属解析

你的理解**完全正确**：  
`IMAGE_EXPORT_DIRECTORY` 结构体中的 `AddressOfNames` 字段所指向的函数名数组，**并不在你当前程序（.exe）的二进制文件中，而是位于目标 DLL（如 `ntdll.dll`）被加载到内存后的映射区域中**。

为了更严谨地巩固这一关键概念，我们可以从以下三个维度深入拆解：

---

##### 1. **物理归属：它是目标 DLL 的一部分**

- 该函数名数组是目标 DLL（例如 `kernel32.dll` 或 `ntdll.dll`）在**编译和链接阶段**就生成的导出表（Export Table）数据。
- 当 Windows 加载器将该 DLL 映射进你的进程地址空间时，整个 PE 结构（包括 DOS 头、NT 头、节表、导出表等）都会被载入内存。
- 你的程序（`.exe`）**本身不包含这些字符串**，它只是通过 `src/module.rs` 中的解析逻辑（如 `get_proc_address`）去“读取”另一个模块的内部结构。
  > ✅ 简言之：你的代码是“读者”，DLL 是“书”。书的内容不在读者身体里，但读者可以翻开它。

---

##### 2. **内存位置：位于当前进程的虚拟地址空间内**

- 虽然该数组不属于你的 `.exe` 模块，但由于目标 DLL 已被加载到**同一个进程**中，其内存映像就存在于**当前进程的虚拟地址空间**里。
- 因此，你可以安全地执行如下操作：

  ```rust
  let names_array_rva = export_dir.AddressOfNames;
  let names_array_va = h_module as usize + names_array_rva as usize;
  let names_ptr = names_array_va as *const u32; // 实际是指向 RVA 数组
  ```

- 这种“基址 + RVA”的加法之所以有效，正是因为 `h_module`（即 DLL 的加载基址）和导出表数据**同属一个地址空间**。
  > ⚠️ 反例：若尝试用同样方式读取**其他进程**的 DLL 内存，则会因地址空间隔离而失败（需使用 `ReadProcessMemory` 等 API）。

---

##### 3. **技术细节修正：它是一个“RVA 数组”，而非“字符串数组”**

这里有一个**关键但易错的细节**需要澄清：

- ❌ **错误理解**：  
  `AddressOfNames` → `["NtAllocateVirtualMemory", "NtCreateThreadEx", ...]`

- ✅ **正确结构**：  
  `AddressOfNames` → `[RVA₁, RVA₂, RVA₃, ...]`  
  其中每个 `RVAᵢ` 是一个 **相对虚拟地址（Relative Virtual Address）**，指向真正的函数名字符串。

  也就是说，完整的解析路径是：

  ```
  h_module
    └─> IMAGE_EXPORT_DIRECTORY
          └─> AddressOfNames (RVA) 
                └─> [RVA_to_Name1, RVA_to_Name2, ...]  ← 这是一个 u32 数组
                      └─> h_module + RVA_to_Name1 → "NtAllocateVirtualMemory"
                      └─> h_module + RVA_to_Name2 → "NtCreateThreadEx"
  ```

- 因此，在代码中你通常会看到两层间接寻址：

  ```rust
  // 第一层：获取第 i 个函数名的 RVA
  let name_rva = unsafe { *(names_ptr.add(i)) };
  // 第二层：计算真实字符串地址
  let name_str_ptr = (h_module as usize + name_rva as usize) as *const u8;
  ```

---

##### 总结

你的直觉非常准确：**你的程序只是一个“观察者”或“解析器”**，它通过指针跨越了模块边界，去窥探目标 DLL 在内存中的内部数据结构。

在 `dinvk` 这类红队项目中：

- `h_module`（即 DLL 的加载基址）就是你进行“窥探”的**基准起始点**。
- 所有对导出表（EAT）、导入表（IAT）、重定位表的操作，本质上都是基于这个基址 + RVA 的偏移计算。
- 正是这种对 PE 结构的深度手动解析能力，使得加载器能够绕过 `GetProcAddress` 等被监控的 API，实现隐蔽的函数地址获取。

> 掌握这一点，你就真正理解了“反射式加载”和“无痕 API 解析”的底层基石。

**16 个索引的含义**：

| 索引 | 名称 | 用途 |
|------|------|------|
| `[0]` | Export Table | DLL 导出的函数列表（如 `GetProcAddress` 查询目标）|
| `[1]` | Import Table | 依赖的 DLL 及导入函数（IAT）|
| `[2]` | Resource Table | 图标、对话框、字符串、版本信息等资源 |
| `[3]` | Exception Table | 异常处理信息（如 x64 的 `.pdata`）|
| `[4]` | Certificate Table | 数字签名（文件偏移 + 大小，非 RVA）|
| `[5]` | Base Relocation Table | 重定位信息（用于 ASLR）|
| `[6]` | Debug | 调试信息（PDB 路径等）|
| `[7]` | Architecture | 保留（应为 0）|
| `[8]` | Global Ptr | 保留（应为 0）|
| `[9]` | TLS Table | 线程局部存储（Thread Local Storage）回调 |
| `[10]`| Load Config Table | 加载配置（如 CFG 控制流防护）|
| `[11]`| Bound Import | 绑定导入（预解析地址，加速加载）|
| `[12]`| IAT | 导入地址表（Import Address Table）|
| `[13]`| Delay Import | 延迟加载导入表 |
| `[14]`| CLR Runtime Header | .NET 程序的元数据（CLI 头）|
| `[15]`| Reserved | 保留 |

> ⚠️ 注意：`[4] Certificate Table` 的 `VirtualAddress` 实际是**文件偏移**，不是 RVA！

---

#### 第三部分：Section Headers（节表）

紧跟在 Optional Header 之后，是一个长度为 `NumberOfSections` 的数组。每个节描述一段代码或数据如何从磁盘映射到内存。

```rust
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IMAGE_SECTION_HEADER {
    pub Name: [u8; 8],     // [0x00] 节名（ASCII，如 ".text", ".rdata", ".pdata"）
                           // 仅为标识，加载器不强制依赖。若超长，可能以 `/数字` 形式引用字符串表。

    pub Misc: IMAGE_SECTION_HEADER_0, // [0x08] Union：
                                      // - 在 OBJ 文件中 = PhysicalAddress
                                      // - 在 EXE/DLL 中 = VirtualSize（内存中实际大小）

    pub VirtualAddress: u32,   // [0x0C] 节在内存中的 RVA
                               // 实际地址 = ImageBase + VirtualAddress

    pub SizeOfRawData: u32,    // [0x10] 节在磁盘文件中的大小（按 FileAlignment 对齐）
                               // 可能小于 VirtualSize（如 .bss）

    pub PointerToRawData: u32, // [0x14] 节在文件中的偏移（从文件头开始）
                               // 若为 0，表示该节无原始数据（如 .bss）

    pub PointerToRelocations: u32, // [0x18] 重定位表偏移（OBJ 文件用，EXE=0）
    pub PointerToLinenumbers: u32, // [0x1C] 行号表偏移（调试用，通常=0）
    pub NumberOfRelocations: u16,  // [0x20] 重定位项数（OBJ 用）
    pub NumberOfLinenumbers: u16,  // [0x22] 行号项数

    pub Characteristics: u32,      // [0x24] 节属性（位掩码）
                                   // 权限标志（影响内存页保护）：
                                   // - 0x80000000 = 可写（Write）
                                   // - 0x40000000 = 可读（Read）
                                   // - 0x20000000 = 可执行（Execute）
                                   // 内容类型标志：
                                   // - 0x00000020 = 包含代码
                                   // - 0x00000040 = 包含已初始化数据
                                   // - 0x00000080 = 包含未初始化数据
                                   // - 0x04000000 = 包含导出信息
                                   // - 0x02000000 = 包含共享数据
}
```

> 📌 **关键概念**：  
>
> - **RVA（Relative Virtual Address）**：相对于 `ImageBase` 的偏移。  
> - **File Offset vs RVA**：磁盘布局（File Alignment）≠ 内存布局（Section Alignment）。  
> - **VirtualSize vs SizeOfRawData**：内存中可能比磁盘大（零填充）。

---

#### 总结：Windows PE 加载流程

当 Windows 加载一个 PE 文件时，按以下步骤执行：

1. **验证 DOS Header**  
   检查 `e_magic == "MZ"`。

2. **定位并验证 NT Headers**  
   通过 `e_lfanew` 跳转，检查 `Signature == "PE\0\0"`。

3. **分配虚拟内存**  
   尝试在 `OptionalHeader.ImageBase` 处保留 `SizeOfImage` 大小的内存区域。

4. **映射节区（Sections）**  
   遍历每个 `IMAGE_SECTION_HEADER`：
   - 从文件偏移 `PointerToRawData` 读取 `SizeOfRawData` 字节。
   - 写入内存地址 `ImageBase + VirtualAddress`。
   - 若 `VirtualSize > SizeOfRawData`，剩余部分清零。
   - 根据 `Characteristics` 设置页权限（R/W/X）。

5. **处理导入表（Import Table）**  
   加载所有依赖的 DLL，并解析导入函数地址（填入 IAT）。

6. **应用重定位（Relocations）**  
   若实际加载地址 ≠ `ImageBase`，则遍历重定位表修正硬编码地址。

7. **执行 TLS 初始化（如有）**  
   调用 TLS 回调函数（如 C++ 全局构造函数）。

8. **跳转到入口点**  
   设置主线程的 RIP 为 `ImageBase + AddressOfEntryPoint`，开始执行程序。

> ✅ **核心思想**：  
> PE 格式通过 **RVA + 基址** 实现地址无关性，通过 **节表** 解耦磁盘与内存布局，通过 **数据目录** 提供模块化功能索引。  
> `dinvk` 等底层工具正是基于这些结构，实现无 API 依赖的模块遍历、函数解析与代码注入。

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

- 获取 PEB,x64: gs:[0x60],x86: fs:[0x30]

关于PEB有以下需要注意:  

1. **结构体不透明性**：`EPROCESS`, `ETHREAD`, `KPROCESS` 等内核结构体是 **非公开 (Opaque)** 的。微软从未保证其成员偏移量（Offsets）的稳定性。文中标记的偏移量仅为示例或特定历史版本，实战中**必须**通过符号文件 (`.pdb`) 或运行时特征码搜索动态获取。
2. **ASLR (地址空间布局随机化)**：现代 Windows (Vista+) 强制开启 ASLR。文中出现的内存地址仅为**逻辑示意**，实际运行时基址、堆栈地址每次启动均不同。
3. **架构限定**：本文核心描述 **x64 (AMD64)** 架构下的 Windows 运行机制。(Intel 64、AMD64 和 x86_64指的是同一种指令集架构,Windows 操作系统在底层通常统一使用 AMD64 来标识 64位架构，无论你的 CPU 是 Intel 还是 AMD 生产的。)
4. 在 Windows 中，每个进程都有一个 PEB (Process Environment Block).进程（Process）在 Windows内核对象（EPROCESS）的定义中，就是资源和地址空间的容器。而 PEB是这个容器在用户模式（User Mode）下的管理结构。当内核创建一个新进程时（NtCreateUserProcess），它必须在分配的虚拟地址空间中映射并初始化一个 PEB。没有 PEB，ntdll.dll 无法初始化，用户模式的代码（包括 main函数）根本无法开始执行。

### 一个可执行文件产生多个进程时PEB是怎么样的?

一个可执行文件产生多个进程,但这不改变“每个进程有一个 PEB”的事实。

一个可执行文件产生了多个进程，情况如下：  

1. 多开（Multiple Instances）:多次打开同一个可执行文件.系统创建了两个完全独立的进程（PID 1001 和 PID 1002）.PEB: 它们各自拥有自己独立的 PEB。虽然它们来自同一个 .exe文件，但它们在内存中是两个互不相干的世界。
2. 父子进程（Spawning Child Processes）:在一个可执行文件中,调用 CreateProcess("myapp.exe") 自我复制.结果: 一个父进程，一个子进程。 PEB: 依然是两个独立的 PEB。
3. 多线程（Multi-threading）:这是最容易混淆的。一个进程可以包含多个线程。结果: 1 个进程，N 个线程。PEB: 所有这 N 个线程共享同一个 PEB（因为它们属于同一个进程） TEB: 每个线程拥有自己独立的 TEB (Thread Environment Block)。

无论一个 .exe 启动了多少次，或者它自己又派生了多少子进程，只要那是 Windows上的一个标准 Win32 进程，它就一定拥有一个属于它自己的、独一无二的 PEB。

### LDR

Ldr 是一个指向 PEB_LDR_DATA 结构体的指针 (*mut PEB_LDR_DATA)。

它指向了关于进程已加载模块（如 DLLs）的详细信息。操作系统加载器（Loader）使用这个结构来维护所有加载到该进程地址空间的模块链表。

- Rust 类型: 在 dinvk 中，它被定义为裸指针，意味着访问它需要使用unsafe 代码块。

### PEB_LDR_DATA

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

是 Windows 操作系统内部用于描述每一个已加载模块（DLL 或 EXE）的元数据结构  
通过PEB中的一个`pub Ldr: *mut PEB_LDR_DATA,`字段指向PEB_LDR_DATA这个结构

### InMemoryOrderModuleList

`InMemoryOrderModuleList` 并非如其名称暗示的那样"按内存地址高低排序"，这是一个在安全研究和逆向工程社区中广泛流传的误解。  
实际上，这个链表主要反映**模块在内存中的布局顺序和初始化关系**，而非简单的地址高低排序。

InMemoryOrderModuleList的结构体中只有两个指针 Flink (前向) 和 Blink (后向)，不包含 DLL 信息  
同样需要注意InMemoryOrderModuleList.flink指向的是LDR_DATA_TABLE_ENTRY这个结构体的**中间位置(不是第一个字段)**,即是一种手拉手的双向链表,而不是手拉头的双向链表:

Windows 将 `LIST_ENTRY` **嵌入**到 `LDR_DATA_TABLE_ENTRY` 结构体中（通常在偏移 `0x10` 处为 `InMemoryOrderLinks`）。  
因此，当你拿到 `Flink` 指针时，它指向的是目标结构体内部的 `InMemoryOrderLinks` 字段（偏移 `0x10`），而非结构体起始地址。

- Flink 指向的是下一个模块结构体中对应的那个 `InMemoryOrderLinks`字段的地址。
- 它不指向下一个结构体的头部 (Base)。

InMemoryOrderModuleList,在实际的运行中,根据Windows 加载器初始化模块的顺序是雷打不动的：

- Head：链表头（PEB_LDR_DATA 内部）。
- Node 1：主程序 (.exe)（第一个加载）。
- Node 2：ntdll.dll（第二个加载，负责用户层与内核层的交互）。
- Node 3：kernel32.dll（通常情况）。
- 代码逻辑：从 Head 开始，执行两次 Flink 跳转，理应到达ntdll.dll。为了保险，代码还计算了模块名的 Hash 进行校验。

### LDR_DATA_TABLE_ENTRY

```rust
#[repr(C)]
//与verg中_LDR_DATA_TABLE_ENTRY相对应
pub struct LDR_DATA_TABLE_ENTRY {
    pub Reserved1: [*mut c_void; 2],//该字段大小16字节,因为在repr(c)模式下64 bit os一个指针占用8字节
    pub InMemoryOrderLinks: LIST_ENTRY,
    pub Reserved2: [*mut c_void; 2],
    pub DllBase: *mut c_void,
    pub Reserved3: [*mut c_void; 2],
    pub FullDllName: UNICODE_STRING,
    pub Reserved4: [u8; 8],
    pub Reserved5: [*mut c_void; 3],
    pub Anonymous: LDR_DATA_TABLE_ENTRY_0,
    pub TimeDateStamp: u32,
}
```

是 Windows操作系统加载器（Loader）用来管理每一个已加载模块（DLL 或
EXE）的核心数据结构。  

- 它不在 PEB 中直接定义，而是通过链表从 PEB 间接可达。
- dinvk 手动定义该结构是为了**精确匹配内存布局**，实现无 API 的模块遍历。
- 必须使用#[repr(c)],匹配windows的内存布局

位于堆上, 因为 LDR_DATA_TABLE_ENTRY 只是堆上的普通数据，红队技术中的 "断链隐藏"(Module Unlinking) 就是手动操作 Flink 和 Blink指针，把自己从链表中移除，但不释放内存。这样 FreeLibrary 无法卸载它，且遍历 PEB的工具（如任务管理器）也看不到它。

并发安全: Windows 加载器在操作这些链表时会持有 PEB->LoaderLock (也就是LdrLockLoaderLock)。如果在你的 Rust代码中手动遍历这些链表且不获取锁，理论上存在竞争条件（虽然在单线程 Shellcode中通常忽略不计）。

#### LDR_DATA_TABLE_ENTRY如何产生并维护?

LDR_DATA_TABLE_ENTRY 结构体完全由 Windows 用户模式加载器 (User Mode Loader) 产生和维护，具体实现位于 `ntdll.dll` 中。

它不是内核对象，而是存在于进程私有堆（Process Heap）中的普通数据结构。这意味着拥有该进程权限的代码（包括你的 dinvk项目）可以随意读取甚至修改它。  
以下是其生命周期的详细过程：

##### 产生 (Creation)

当一个模块（.exe 或 .dll）被映射到进程内存时，ntdll.dll中的加载器代码会创建这个结构。

- 静态导入 (进程启动时):
当进程刚启动，内核完成基本的内存映射后，执行权交给
ntdll!LdrInitializeThunk。它会调用 LdrpInitializeProcess，为以下模块分配并初始化LDR_DATA_TABLE_ENTRY：

1. 自身 (ntdll.dll)
2. 主执行文件 (.exe)
3. 静态导入表中的所有 DLL (如 kernel32.dll)

- 动态加载 (运行时):
当你调用 LoadLibrary (Win32 API) 时，调用链如下：  
LoadLibraryW -> KernelBase!LoadLibraryExW -> ntdll!LdrLoadDll ->`ntdll!LdrpLoadDll`  

在 LdrpLoadDll 内部（具体函数名随 Windows 版本可能变化，如LdrpAllocateDataTableEntry），加载器会执行以下操作：

1. 调用 RtlAllocateHeap 从进程默认堆中分配一块内存，大小为sizeof(LDR_DATA_TABLE_ENTRY)。
2. 填充结构体字段：

- DllBase: 填入模块映射的基址。
- FullDllName / BaseDllName: 分配并填入路径字符串。
- LoadCount: 初始化引用计数。

1. 调用 LdrpInsertDataTableEntry 将这个新节点插入到 PEB->Ldr 的三个链表中(InLoadOrder, InMemoryOrder, InInitializationOrder)。

##### 维护 (Maintenance)

在模块的生命周期内，ntdll.dll 负责维护其状态：

- 引用计数 (`LoadCount`):  
如果同一个 DLL 被 LoadLibrary 加载多次，加载器不会重新映射文件，而是找到现有的LDR_DATA_TABLE_ENTRY，将其 LoadCount 加 1。
- 重定位与修复:  
加载器会更新结构中的标记（Flags），表明该模块是否已完成重定位、是否已执行 TLS回调等。

##### 销毁 (Destruction)

当你调用 `FreeLibrary` 时，Windows 加载器会执行以下卸载流程：

1. **调用 `ntdll!LdrUnloadDll`**  
   用户态的 `FreeLibrary` 最终会进入内核模式加载器函数 `LdrUnloadDll`。

2. **递减引用计数**  
   加载器将目标模块对应的 `LDR_DATA_TABLE_ENTRY` 中的 `LoadCount` 字段减 1。

3. **检查是否真正卸载**  
   如果 `LoadCount` 降为 0（且该 DLL 未被标记为“永久加载”的系统模块，如 `ntdll.dll`），则启动完整的卸载流程：
   - **执行 `DllMain` 回调**  
     调用该 DLL 的入口点函数，并传入 `DLL_PROCESS_DETACH` 通知其即将被卸载。
   - **从模块链表中移除节点（Unlink）**  
     将该 `LDR_DATA_TABLE_ENTRY` 从 PEB 中的三条链表中摘除：
     - `InLoadOrderModuleList`
     - `InMemoryOrderModuleList`
     - `InInitializationOrderModuleList`
   - **释放字符串资源**  
     释放 `FullDllName` 和 `BaseDllName` 等 `UNICODE_STRING` 所指向的动态分配内存。
   - **释放元数据结构**  
     调用 `RtlFreeHeap` 释放 `LDR_DATA_TABLE_ENTRY` 结构体本身所占用的堆内存。
   - **解除内存映射**  
     调用内存管理器解除对该 DLL 映像的内存映射（即从进程地址空间中移除其代码和数据段）。

> 💡 注意：如果 `LoadCount > 0`，仅递减计数，**不会执行实际卸载**。这也是为什么多次调用 `LoadLibrary` 需要对应多次 `FreeLibrary` 才能真正卸载模块。

- 为什么在 PEB 中“没有直接看到”它？(不是PEB的字段之一)

你其实已经在 PEB 中找到了它的入口，只是它以**链表节点**的形式存在，而不是直接内嵌在 PEB 中。  
内存结构示意：

```text
[ TEB ]
  |
  +-> [ PEB ]
        |
        +-> Ldr (指向 PEB_LDR_DATA)
              |
              +-> InMemoryOrderModuleList (LIST_ENTRY 链表头)
                    |
                    v
              [ LDR_DATA_TABLE_ENTRY A ] <-> [ LDR_DATA_TABLE_ENTRY B ] <-> ...
                    ^                          ^
                    |                          |
              LIST_ENTRY 节点           LIST_ENTRY 节点
```

标准 Windows SDK（如 `windows-sys`）通常**不公开完整字段**，原因包括：

- 微软将其视为“内部实现细节”（Undocumented）
- 为兼容性隐藏部分字段
- 不同 Windows 版本结构可能变化

因此，dinvk 在 `src/types.rs` 中**手动定义**了一个与真实内存布局一致的版本

### ImageBaseAddress

ImageBaseAddress 是 PEB 结构体中的一个重要字段（通常在偏移 0x10 处）

- 它存储了当前进程的主模块（也就是启动这个进程的 .exe文件）被加载到内存中的起始位置。
- 对于大多数 64 位程序，默认情况下这个地址可能是0x140000000，或者是因ASLR（地址空间布局随机化）而随机生成的某个地址。

## let mut data_table_entry = (*ldr_data).InMemoryOrderModuleList.Flink as*const LDR_DATA_TABLE_ENTRY

这里的as有点复杂,as转换直接把裸地址变成了可操作的结构体对象。只要地址数值正确，且结构体布局匹配，你就可以直接像操作普通对象一样解引用和访问字段。

### 物理事实 (Memory Reality)

- Flink 指针里存的数值是 `0x1000`（假设这是某个模块 `InMemoryOrderLinks` 的地址）。
- 由于 `InMemoryOrderLinks` 在该模块真正的起始位置（`0x0F90`）往后 16 字节处，所以物理地址就是 `0x1000`。

### 编译器的“错觉” (Compiler's View)

当你执行 `as *const LDR_DATA_TABLE_ENTRY` 时：

- **指令层面**：这行代码在生成的汇编里通常不产生任何指令，它只是改变了编译器账本上的一个标记。
- **认知层面**：你告诉编译器：“从现在开始，把 `0x1000` 这个地址看作是 `LDR_DATA_TABLE_ENTRY` 结构体的第 0 个字节。”

### 结果：偏移量的“平移”

因为编译器认为 `0x1000` 是 `0x00`，那么当你在代码里访问某个字段（比如偏移为 `0x48` 的字段）时：

- **编译器计算**：当前位置 (`0x1000`) + 字段偏移 (`0x48`) = `0x1048`。
- **物理对比**：在真实的内存布局中，`0x1048` 对应的是真实头部 (`0x0F90`) + `0x58`。
- **神奇发现**：`0x58` 处正好就是我们要找的那个数据字段！

### 总结

- “指向...偏移位置 (`0x10`)”：这是物理事实，指针确实指在人家结构体的“肚子”上。
- “编译器看来仍然是指向...第 0 个字节处”：这是类型转换的作用，你强迫编译器从“肚子”开始算作“头”。

## 指令集架构

除了 x86_64（AMD64）之外，还存在多种主流和专用的指令集架构（ISA）。作为一名 Rust 开发者，了解以下几种架构尤为重要，因为它们是 Rust 交叉编译的常见目标。

---

### 1. ARM 架构（AArch64 / ARM64）

- **特点**：基于 RISC（精简指令集计算），功耗低，能效比高。
- **应用场景**：
  - **移动设备**：几乎所有的 Android 手机和 iPhone。
  - **桌面端**：Apple Silicon（M1、M2、M3、M4）系列芯片。
  - **服务器**：AWS Graviton 等高性能云服务器。
  - **嵌入式**：Raspberry Pi（树莓派）。
- **Rust 目标三元组示例**：
  - `aarch64-unknown-linux-gnu`
  - `aarch64-apple-darwin`

---

### 2. RISC-V

- **特点**：完全开源、免费、模块化的 RISC 架构。允许任何人设计、制造和销售 RISC-V 芯片而无需支付专利费，被誉为“芯片界的 Linux”。
- **应用场景**：
  - 当前主要用于物联网（IoT）、嵌入式控制器。
  - 正在向服务器和高性能计算（HPC）领域快速扩展。
- **Rust 支持**：
  - 官方支持良好，例如：`riscv64gc-unknown-linux-gnu`

---

### 3. WebAssembly（Wasm）

- **说明**：虽然不是物理 CPU 架构，但是一种极其重要的**虚拟指令集架构**。
- **特点**：基于堆栈的二进制指令格式，设计为可移植的编译目标。
- **应用场景**：
  - 在浏览器中以接近原生速度运行 Rust 代码。
  - 边缘计算（Edge Computing）。
  - 插件系统（如 Envoy Proxy、Fermyon Spin）。
- **Rust 目标三元组**：
  - `wasm32-unknown-unknown`
  - `wasm32-wasi`

---

### 4. x86（32-bit）/ i686

- **特点**：x86_64 的前身，32 位架构，内存寻址上限为 4GB。
- **应用场景**：
  - 老旧 PC、工业控制系统、遗留嵌入式设备。
  - 虽然现代开发已逐渐淘汰，但在兼容性维护中仍需关注。
- **Rust 目标示例**：`i686-unknown-linux-gnu`

---

### 5. 其他嵌入式与专用架构

| 架构       | 简介                                                                 |
|------------|----------------------------------------------------------------------|
| **MIPS**   | 曾广泛用于路由器和机顶盒，现逐渐被 ARM 和 RISC-V 取代。               |
| **PowerPC (PPC)** | 曾用于老款 Mac 和游戏主机（如 PS3、Xbox 360），现主要用于汽车电子、航天等高可靠领域。 |
| **AVR**    | 8 位微控制器架构，典型代表是 Arduino Uno，适用于极低功耗嵌入式场景。     |

---

### 总结：CISC vs RISC

计算机体系结构主要分为两大流派：

1. **CISC（Complex Instruction Set Computing，复杂指令集）**
   - **代表**：x86 / x86_64
   - **特点**：
     - 单条指令可完成复杂操作（如直接内存读写）。
     - 代码密度高（程序体积小）。
     - 硬件实现极其复杂，依赖微码（microcode）。

2. **RISC（Reduced Instruction Set Computing，精简指令集）**
   - **代表**：ARM、RISC-V
   - **特点**：
     - 指令功能简单、固定长度、执行速度快。
     - 依赖编译器优化来组合多条简单指令。
     - 硬件设计简洁，功耗低，适合并行和能效敏感场景。

---

> 💡 **对 RustRedOps 开发者的提示**：  
> 如果你正在开发底层红队/蓝队工具（如 shellcode、PE 解析器、系统调用封装等），未来很可能需要处理 **x86_64 与 AArch64** 在以下方面的差异：
>
> - 系统调用编号与约定（syscall ABI）
> - 寄存器命名与用途（如 `rax` vs `x0`）
> - 内存顺序模型（Memory Ordering）
> - 指令编码与 Shellcode 编写
>
> 掌握多架构知识，将极大提升工具的可移植性与隐蔽性。

## EXE 文件和 PE 文件的关系？除了 EXE 还有哪些 PE 文件？

- **PE**（Portable Executable）  
  是微软定义的**二进制文件格式标准**，规定了可执行文件在磁盘上的结构以及加载到内存后的布局方式。
- **EXE**（`.exe`）  
  是符合 PE 格式的一种**具体文件类型**，专用于标识“可直接启动并作为独立进程运行”的程序。

> 🏗️ **类比**：  
>
> - PE 标准 ≈ **建筑规范**（规定地基、承重墙、门窗位置）  
> - EXE 文件 ≈ **按规范建成的住宅**（可直接入住）

---

### 2. 除了 EXE，还有哪些常见的 PE 文件？

只要是符合 PE 结构的文件，无论扩展名如何，都属于 PE 家族。常见类型包括：

#### A. 动态链接库（DLL）

- **`.dll`**：最常见形式，供 EXE 或其他 DLL 调用，不能独立启动。
- **`.ocx`**：ActiveX 控件（旧版 IE 使用），本质是带 COM 接口的 DLL。
- **`.cpl`**：控制面板项（如“鼠标设置”），本质是导出 `CPlApplet` 函数的 DLL。

#### B. 驱动程序（Drivers）

- **`.sys`**：Windows 内核模式驱动，由内核加载器加载，运行在 Ring 0。

#### C. 其他系统/功能文件

- **`.efi`**：UEFI 固件可执行文件，用于系统引导阶段。
- **`.scr`**：屏幕保护程序，本质是改名的 EXE。
- **`.mui`**：多语言用户界面资源文件。
- **`.tsp`**：电话服务提供程序。

---

### 3. 如何区分不同类型的 PE 文件？

尽管结构相同，Windows 通过 **PE 文件头中的标志位**进行识别：

```rust
// src/types.rs
pub struct IMAGE_FILE_HEADER {
    pub Machine: IMAGE_FILE_MACHINE,
    pub NumberOfSections: u16,
    pub TimeDateStamp: u32,
    pub PointerToSymbolTable: u32,
    pub NumberOfSymbols: u32,
    pub SizeOfOptionalHeader: u16,
    pub Characteristics: IMAGE_FILE_CHARACTERISTICS, // ← 关键字段
}
```

关键标志位（`Characteristics`）：

- `IMAGE_FILE_EXECUTABLE_IMAGE` (`0x0002`) → 可执行文件（EXE/SCR）
- `IMAGE_FILE_DLL` (`0x2000`) → 动态链接库（DLL/OCX/CPL）
- `IMAGE_FILE_SYSTEM` (`0x1000`) → 系统文件（如 `.sys` 驱动）

此外，**子系统字段**（`Subsystem` in `IMAGE_OPTIONAL_HEADER`）也可用于区分：

- `IMAGE_SUBSYSTEM_WINDOWS_GUI` / `CONSOLE` → 普通 EXE
- `IMAGE_SUBSYSTEM_NATIVE` → 驱动或内核组件
- `IMAGE_SUBSYSTEM_EFI_APPLICATION` → EFI 可执行文件

> ✅ **总结**：  
>
> - **PE 是“类名”**（抽象格式标准）  
> - **EXE/DLL/SYS/EFI 是“实例”**（具体用途的实现）  
> 它们共享相同的“心脏”（PE 头结构），但扮演不同的系统角色。

---
虽然都遵循 PE 标准，但不同文件类型承担着**截然不同的操作系统职责**。这种多样性源于对**模块化、安全性、资源效率和启动流程**的需求。

---

### 1. EXE（Executable）—— 独立的执行主体

- **特点**：
  - 拥有明确入口点（如 `main`、`WinMain`）。
  - 操作系统为其创建**独立进程**、PEB 和主线程。
  - 是“主动”执行单元，控制程序主流程。
- **为何需要**？
  - 作为用户交互和业务逻辑的主要载体。
  - 是应用程序的“指挥中心”，负责调度其他组件。

---

### 2. DLL（Dynamic Link Library）—— 共享的代码仓库

- **特点**：
  - **被动加载**：需由 EXE 或其他 DLL 显式/隐式加载。
  - **内存共享**：同一系统 DLL（如 `kernel32.dll`）在物理内存中仅存一份代码（Copy-on-Write）。
  - 入口点为 `DllMain`，仅用于初始化/清理，不持续运行。
- **为何有了 EXE 还要 DLL**？
  - ✅ **模块化开发**：功能解耦，便于独立更新（无需重发整个 EXE）。
  - ✅ **内存效率**：避免重复静态链接通用代码（如 API 函数）。
  - ✅ **插件架构**：支持运行时动态扩展功能。

---

### 3. SYS（System Driver）—— 内核的延伸

- **特点**：
  - 运行在 **Ring 0**（内核模式），拥有最高权限。
  - 可直接访问硬件、I/O 端口、任意内存。
  - **无独立进程**：依附于系统线程或中断上下文。
  - 崩溃导致 **蓝屏**（BSOD），而非普通程序崩溃。
- **为何需要**？
  - 🔧 **硬件抽象**：EXE（Ring 3）无法直接操作硬件，需驱动“翻译”。
  - 🛡️ **系统级安全**：杀毒软件、EDR 需在内核层监控行为。
  - ⚙️ **核心服务实现**：文件系统、网络协议栈等均由驱动提供。

---

### 4. EFI（Extensible Firmware Interface）—— 系统的启动者

- **特点**：
  - 在 **操作系统加载前运行**，由 UEFI 固件直接执行。
  - **无 OS 依赖**：此时无 Windows/Linux，仅能调用 UEFI 服务。
  - PE 头中 `Subsystem = IMAGE_SUBSYSTEM_EFI_APPLICATION`。
- **为何需要**？
  - 🚪 **解决“鸡生蛋”问题**：EXE 需要 OS 才能运行，但 OS 本身需被加载。
  - 💾 **引导加载器**：如 `bootmgfw.efi` 负责加载 Windows 内核。
  - 🌐 **标准化固件接口**：取代 BIOS，提供统一硬件初始化能力。

---

### 5. SCR（Screen Saver）—— 特殊用途的 EXE

- **特点**：
  - **本质是 EXE**，重命名为 `.exe` 即可运行。
  - 必须响应特定命令行参数：
    - `/s`：全屏运行
    - `/c`：打开配置窗口
    - `/p <hwnd>`：预览模式
- **为何需要专用格式**？
  - 主要是 **历史规范与系统识别便利性**，便于集成到 Windows 设置中。

---

### 6. OCX / CPL —— 插件式架构的实现者

| 类型 | 全称 | 本质 | 关键要求 |
|------|------|------|--------|
| **OCX** | OLE Control Extension | 特定规范的 DLL | 实现 COM 接口（如 `DllRegisterServer`） |
| **CPL** | Control Panel Item | 特定规范的 DLL | 导出 `CPlApplet` 函数 |

- **为何需要**？
  - 🧩 **标准化插件接口**：控制面板只需识别 `.cpl` 并调用 `CPlApplet`，无需了解内部逻辑。
  - 🔌 **即插即用**：第三方开发者可按规范编写模块，无缝集成到系统中。

---

## 总结：为何需要如此多样的 PE 格式？

现代操作系统采用 **分层、职责分离** 的设计哲学：

| 层级 | 组件 | 职责 |
|------|------|------|
| **固件层** | EFI | 启动硬件，加载内核 |
| **内核层** | SYS | 管理硬件、内存、安全 |
| **系统服务层** | DLL | 提供通用 API（文件、网络、图形等） |
| **应用层** | EXE | 实现用户功能与交互 |

> ❌ **如果只有 EXE**：  
>
> - 每个程序都要自己写显卡驱动、内存管理器。  
> - 无法实现多任务协作或资源共享。  
> - 系统臃肿、低效、极不稳定。

✅ **多样化的 PE 格式** 是现代多任务操作系统**高效、安全、可扩展**运行的基石。它们共同构成了从硬件到用户应用的完整执行链条。

## 源码

### CStr::from_ptr(ptr).to_string_lossy().into_owned()

这段代码的核心作用是：**将目标 DLL 内存映射区中的“C 风格字符串（以 null 结尾的原始字节）”安全地转换成 Rust 环境中可用的、所有权独立的 `String` 对象。**

这种转换在红队加载器（如 `dinvk`）中至关重要——既要正确读取 Windows PE 结构中的原始数据，又要避免内存安全问题。我们可以将其拆解为以下三个关键步骤：

---

#### 1. `CStr::from_ptr(ptr)`：封装原始指针，创建安全视图

- **背景**：  
  `ptr` 是一个裸指针（`*const i8`），指向 DLL 内存映像中某个名称字符串（例如 `"kernel32.dll\0"` 或 `"NtAllocateVirtualMemory\0"`）。

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

- **作用**：  
  - 尝试将 `CStr` 中的字节序列解释为 UTF-8。
  - **“Lossy”（有损）策略**：若遇到非法 UTF-8 序列（例如某些扩展 ASCII 字符），不会 panic，而是用 Unicode 替代字符 ``（U+FFFD）代替无效字节。
  - 返回类型为 `Cow<str>`（Clone-on-Write）：  
    - 如果输入已是合法 UTF-8，可能直接返回 `&str`（零分配）；  
    - 否则，会分配新内存并返回拥有所有权的 `String`。

- **为何需要？**  
  在红队场景中，你无法控制目标 DLL 的编码细节（尤其是第三方或系统 DLL）。`to_string_lossy()` 提供了**健壮性保障**，避免因个别非标准字符导致整个加载器崩溃。

---

#### 3. `.into_owned()`：夺取所有权，实现深拷贝

- **背景**：  
  `to_string_lossy()` 返回的 `Cow<str>` 可能仍是对 DLL 内存的引用（尤其在纯 ASCII 情况下）。

- **作用**：  
  强制将字符串内容**深拷贝到 Rust 堆内存中**，返回一个完全独立的 `String` 对象。

- **安全意义**：  
  - 即便后续 DLL 被 `FreeLibrary` 卸载，或其内存被覆盖/释放，该 `String` 依然有效。
  - 符合 Rust 的**所有权模型**：`dll_name` 变量现在拥有自己的数据，生命周期不再依赖外部模块。

> 🔒 这是实现“内存隔离”的关键一步——让敏感操作（如日志、转发解析、哈希比对）基于**安全副本**进行。

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
