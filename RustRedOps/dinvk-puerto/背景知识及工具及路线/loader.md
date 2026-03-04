- [程序执行全生命周期](#程序执行全生命周期)
  - [标准 Windows 64位控制台程序（如 Rust 编译出的 .exe）从双击到退出的全景流程](#标准-windows-64位控制台程序如-rust-编译出的-exe从双击到退出的全景流程)
    - [第一阶段：进程创建与内核态工作(打造容器)](#第一阶段进程创建与内核态工作打造容器)
    - [第二阶段：苏醒（用户态加载器 Loader）](#第二阶段苏醒用户态加载器-loader)
    - [第三阶段：运行（Runtime 与 Main）](#第三阶段运行runtime-与-main)
    - [第四阶段：终结（进程退出）](#第四阶段终结进程退出)
  - [ntdll.dll](#ntdlldll)
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