- [项目roadmap](#项目roadmap)
  - [项目配置与入口](#项目配置与入口)
    - [`Cargo.toml`](#cargotoml)
    - [`src/lib.rs`](#srclibrs)
  - [核心功能模块](#核心功能模块)
    - [`src/module.rs`](#srcmodulers)
    - [`src/winapis.rs`](#srcwinapisrs)
    - [`src/syscall/mod.rs` 与 `src/syscall/x86_64/mod.rs`](#srcsyscallmodrs-与-srcsyscallx86_64modrs)
    - [`src/hash.rs`](#srchashrs)
    - [`src/breakpoint.rs`](#srcbreakpointrs)
    - [`src/macros.rs`](#srcmacrosrs)
    - [`src/syscall/asm.rs`](#srcsyscallasmrs)
  - [辅助模块](#辅助模块)
    - [`src/allocator.rs`](#srcallocatorrs)
    - [`src/panic.rs`](#srcpanicrs)
    - [`src/console.rs`](#srcconsolers)
    - [`src/helper.rs`](#srchelperrs)
  - [总结](#总结)
  - [TEB-\>PEB-\>Ldr](#teb-peb-ldr)
    - [澄清一个关键概念：PEB 与 TEB 的关系](#澄清一个关键概念peb-与-teb-的关系)
      - [逻辑关系：先有 Process 还是先有 Thread？](#逻辑关系先有-process-还是先有-thread)
      - [关系图解](#关系图解)
    - [访问路径：为什么我们必须先找 TEB？](#访问路径为什么我们必须先找-teb)
      - [类比理解](#类比理解)
    - [3. 结论](#3-结论)
    - [为什么要经过 TEB？](#为什么要经过-teb)

# 项目roadmap

## 项目配置与入口

### `Cargo.toml`

- **功能**: 依赖管理。
- **关键依赖**:
  - `obfstr`：用于编译期字符串混淆，隐藏敏感 API 名称。
  - `spin`：提供自旋锁，在 `no_std` 环境下实现基本同步原语。
  - `windows-targets`：提供 Windows FFI 所需的类型定义。
- **Features**:
  - `alloc`：启用内存分配器，支持 `Vec`、`String` 等堆结构。
  - `panic`：启用自定义 Panic 处理逻辑。
  - 整体设计面向 `no_std` 裸金属环境，适用于 Shellcode、驱动或纯内存载荷。

### `src/lib.rs`

- **功能**: 库的统一入口与模块组织。
- **实现细节**:
  - 声明 `#![no_std]`，完全禁用 Rust 标准库。
  - 通过 `pub mod` 导出所有子模块（如 `module`, `syscall`, `winapis` 等）。
  - 作为外部调用此库的唯一接口，保持高内聚、低耦合。

---

## 核心功能模块

### `src/module.rs`

- **核心功能**: 动态模块与函数解析（即“Loader”）。
- **实现细节**:
  - `get_module_address`：
    - 实现 **PEB Walking**：从 TEB → PEB → Ldr → `InMemoryOrderModuleList` 遍历双向链表。
    - 支持两种查找模式：明文 DLL 名称比较 或 **API Hashing**（传入哈希值匹配）。
  - `get_proc_address`：
    - 手动解析 PE 导出表（Export Directory）。
    - 遍历 `AddressOfNames`、`AddressOfFunctions` 和 `AddressOfNameOrdinals` 三张表，定位函数 RVA 并转换为 VA。
  - `get_forwarded_address` & `resolve_api_set_map`：
    - 处理 **Forwarded Exports**（如 `kernel32!CreateFileW` 实际指向 `kernelbase!CreateFileW`）。
    - 解析 **ApiSetSchema**（如 `api-ms-win-core-file-l1-2-0.dll` 是虚拟 DLL，需映射到真实实现如 `kernelbase.dll`），这对 Windows 8+ 兼容性至关重要。

### `src/winapis.rs`

- **核心功能**: Windows API 与内核结构定义。
- **实现细节**:
  - 定义关键系统结构体：`TEB`、`PEB`、`LDR_DATA_TABLE_ENTRY`、`CONTEXT`、`IMAGE_NT_HEADERS` 等。
  - 封装高危 API：如 `NtAllocateVirtualMemory`、`NtCreateThreadEx`。
  - **集成 Hardware Breakpoint Spoofing**：
    - 若启用断点欺骗模式，先传入**无害假参数**调用 syscall。
    - 在目标地址设置硬件执行断点。
    - 由 `veh_handler` 在异常触发时，将寄存器/栈中的参数**动态替换为真实恶意参数**，绕过 EDR 的参数检查。

### `src/syscall/mod.rs` 与 `src/syscall/x86_64/mod.rs`

- **核心功能**: 间接系统调用（Indirect Syscalls），规避 EDR Hook。
- **实现细节**:
  - `ssn()` 函数：动态解析目标 API 的系统服务号（SSN）。
  - 实现三种 **Gate 技术**：
    1. **Hell’s Gate**：直接反汇编 `ntdll` 中目标函数，提取 `MOV EAX, <SSN>` 指令中的 SSN。
    2. **Halos Gate**：若目标函数被 EDR Hook（以 `E9 JMP` 开头），则扫描其上下 ±32 字节的**邻近函数**，利用 SSN 连续性推算当前 SSN。
    3. **Tartarus Gate**：处理更复杂的 Hook 模式（如 `E9` 位于函数偏移 +3 处）。
  - `get_syscall_address()`：
    - 在 `ntdll` 中搜索 `syscall; ret`（字节码 `0x0F 0x05 0xC3`）指令序列。
    - 获取其地址后跳转执行，避免在自身代码段直接使用 `syscall` 指令（减少静态特征）。

### `src/hash.rs`

- **核心功能**: API Hashing 算法库。
- **实现细节**:
  - 支持多种哈希算法：Jenkins3、Murmur3、CRC32、DJB2、FNV-1a。
  - 用于将字符串（如 `"NtCreateProcess"`）在**编译期**转换为整数常量。
  - 彻底消除二进制中明文 API 字符串，对抗静态分析和 YARA 规则。

### `src/breakpoint.rs`

- **核心功能**: 硬件断点设置与异常处理（即“The Spoofer”）。
- **实现细节**:
  - `set_breakpoint()`：
    - 调用 `NtGetContextThread` / `NtSetContextThread` 直接操作 CPU 调试寄存器（`Dr0–Dr7`）。
    - 在指定地址（如 syscall stub）设置 **硬件执行断点**。
  - `veh_handler()`：
    - 注册为向量化异常处理器（VEH）。
    - 捕获 `EXCEPTION_SINGLE_STEP` 异常。
    - 根据全局状态 `CURRENT_API`，修改 `CONTEXT` 中的寄存器（`RCX`, `RDX`, `R8`, `R9`）或栈上参数，完成**参数欺骗**。

### `src/macros.rs`

- **核心功能**: 提升开发者体验的语法糖。
- **实现细节**:
  - `dinvoke!(module, "FunctionName", args...)`：
    - 自动调用 `get_module_address` + `get_proc_address` + `transmute`。
    - 使动态调用写法接近原生函数调用。
  - `syscall!(NtFunction, args...)`：
    - 自动解析 SSN、获取 syscall stub 地址、组装参数、执行间接 syscall。
    - 统一且安全的系统调用接口。

### `src/syscall/asm.rs`

---

## 辅助模块

### `src/allocator.rs`

- **功能**: 实现 `GlobalAlloc` trait。
- **实现方式**: 通常通过 `HeapAlloc` / `HeapFree` 调用 Windows 堆管理器。
- **作用**: 使 `no_std` 环境可安全使用 `Vec<T>`、`String` 等需要堆分配的数据结构。

### `src/panic.rs`

- **功能**: 自定义 Panic 行为。
- **实现方式**: 定义 `#[panic_handler]` 函数。
- **典型行为**: 死循环或调用 `NtTerminateProcess`，避免链接标准库的复杂 unwind 逻辑，减小体积并防止信息泄露。

### `src/console.rs`

- **功能**: 实现控制台输出（如 `println!` 的后端）。
- **实现方式**: 可能直接调用 `WriteConsoleA` 或 `NtWriteFile` 到 `CONOUT$`。
- **用途**: 调试输出，可在发布版中条件编译移除。

### `src/helper.rs`

- **功能**: PE 文件解析辅助工具。
- **实现方式**: 提供 `PE` 结构体，封装对 DOS Header、NT Headers、Section Table 的便捷访问。
- **作用**: 简化 `module.rs` 中的 PE 解析逻辑，提升代码可读性。

---

## 总结

这是一个**结构严谨、技术深度极高**的红队开发库。它不依赖任何“黑魔法”，而是通过以下手段实现高级规避：

- **手动重写 Windows 加载器逻辑**（PEB 遍历、导出表解析、ApiSet 映射）；
- **直接操作 CPU 调试寄存器**实现硬件断点欺骗；
- **动态汇编生成与字节码分析**（Hell’s/Halos Gate）绕过 EDR Hook；
- **编译期字符串混淆与 API Hashing**消除静态特征；
- **完整的 `no_std` 支持**，适用于 Shellcode、反射式加载等场景。

每一行代码——无论是 `module.rs` 中的链表遍历，还是 `syscall/x86_64/mod.rs` 中的字节码匹配——都具有明确的**对抗目的**，体现了现代红队工具在**隐蔽性、兼容性与健壮性**上的极致追求。

## TEB->PEB->Ldr

源码中是从下面的代码开始的:

```rust
pub fn NtCurrentPeb() -> *const PEB {
    #[cfg(target_arch = "x86_64")]
    return __readgsqword(0x60) as *const PEB;
    ....
```

表层调用的 NtCurrentPeb()，但实际上 TEB是隐藏在 `NtCurrentPeb()` 这个函数内部的必经之路。  
在 Windows 操作系统原理中，你无法直接“凭空”拿到 PEB的地址。必须先找到当前线程的 TEB，然后从 TEB 里读取 PEB 的指针。

### 澄清一个关键概念：PEB 与 TEB 的关系

这是一个非常容易混淆的操作系统概念。**PEB 的信息本身并不存储在 TEB 里**，TEB 中存储的只是一张指向 PEB 的“名片”——即一个指针。

为了彻底理清这个逻辑，我们必须区分 **“拥有关系”**（Ownership） 和 **“访问路径”**（Access Path）。

---

#### 逻辑关系：先有 Process 还是先有 Thread？

从操作系统对象模型的角度看，**先有 Process（进程），后有 Thread（线程）**。

- **Process（进程）**  
  是资源的容器。它拥有：
  - 虚拟内存空间
  - 文件句柄、安全令牌等内核对象
  - 一个 **PEB**（Process Environment Block）  
    → PEB 就像进程的“身份证”和“全局配置表”，**每个进程只有一个**。

- **Thread（线程）**  
  是执行的单元。每个线程都在某个进程的地址空间中运行，并拥有：
  - 自己的 CPU 上下文（寄存器、栈等）
  - 一个 **TEB**（Thread Environment Block）  
    → TEB 就像工人的“工牌”和“私人物品箱”，**每个线程各有一个**。

#### 关系图解

```html
[ 进程 (Process) ]
│
├── [ 虚拟内存空间 ]
│   │
│   └── [ PEB ] ←───────────────┐
│        (全局唯一，位于内存某处) │
│                                │
├── [ 线程 A (Thread A) ]        │
│   │                            │
│   └── [ TEB A ] ───────────────┘  (TEB A 中包含一个指针，指向 PEB)
│
└── [ 线程 B (Thread B) ]
    │
    └── [ TEB B ] ───────────────→ (TEB B 也指向同一个 PEB)
```

> ✅ **关键点**：所有属于同一进程的线程，其 TEB 都指向**同一个 PEB**。

---

### 访问路径：为什么我们必须先找 TEB？

既然 PEB 属于进程，为什么我们写代码时不能直接获取 PEB，而必须通过 TEB？

**原因：CPU 只认识“当前正在执行的线程”**。

- CPU 在任意时刻只执行一个线程的指令。
- 操作系统利用 x86/x64 架构的 **段寄存器**（FS/GS）为每个线程提供快速自省能力：
  - **x86**：`FS` 寄存器始终指向当前线程的 TEB
  - **x64**：`GS` 寄存器始终指向当前线程的 TEB
- 这是硬件 + OS 协同设计的结果，目的是让线程能**零开销**地访问自身上下文。

#### 类比理解

想象你是一个访客，想找到公司的“总务处”（PEB）：

- **总务处**（PEB）：存放公司名录、已加载模块列表（Ldr）、环境变量等全局信息。
- **员工胸牌**（TEB）：每个员工佩戴，上面印有个人信息和一张小纸条：“总务处在 3 楼 301 室”。
- **你**（CPU/当前代码）：不知道总务处位置，但你可以**立刻看到当前正在工作的员工**（当前线程）。

你只能：

1. 先看员工的胸牌（通过 `FS:[0x30]` 或 `GS:[0x60]` 读取 TEB）
2. 从胸牌上找到“总务处地址”（TEB 中的 PEB 指针字段）
3. 再根据该地址访问 PEB

> 💡 **没有 TEB，你就失去了“当前上下文”的锚点**。在用户态，你无法直接“感应”到 PEB 的位置。

---

### 3. 结论

| 维度 | 说明 |
|------|------|
| **拥有关系** | 进程拥有 PEB；线程属于进程；TEB 属于线程 |
| **存储位置** | PEB 和 TEB 是两块**独立的内存区域**，互不嵌套 |
| **指针关系** | 每个 TEB 中包含一个 **8 字节（x64）或 4 字节（x86）的指针**，指向所属进程的 PEB |
| **访问路径** | 用户态代码必须通过 **FS/GS → TEB → PEB 指针 → PEB** 的链路才能定位 PEB |

因此：

> 虽然逻辑上“先有进程”，但在**代码执行的视角**（寻址路径）上，我们必须“通过 TEB 找到 PEB”。  
> 如果不先访问 TEB，我们的代码（在用户模式下）就无法知道 PEB 到底位于内存的哪个地址。

这也是为什么所有红队技术、Shellcode、无导入表加载器的第一步几乎都是：

```c
// x64
PPEB peb = (PPEB)__readgsqword(0x60);

// x86
PPEB peb = (PPEB)__readfsdword(0x30);
```

——因为这是 Windows 提供的**唯一标准、高效且无需系统调用的 PEB 获取方式**。

### 为什么要经过 TEB？

1. x64 架构下的 GS 寄存器与 TEB

在 64 位 Windows 系统中，CPU 的 GS 段寄存器（Segment Register）被专门用于指向当前线程的 TEB（Thread Environment Block）的起始地址。

代码中的 __readgsqword(0x60) 含义如下：
动作：从 GS 寄存器所指向的地址开始，向后偏移 0x60 字节，读取一个 64 位（8 字节）的值。
内存布局：
GS:[0x00] → TEB 的起始位置
GS:[0x30] → 指向 TEB 自身的指针（Self）
GS:[0x60] → 指向 PEB（ProcessEnvironmentBlock）

因此，当你调用 NtCurrentPeb() 时，其底层硬件执行路径为：
CPU (GS 寄存器) → TEB（偏移 0x60） → 获取 PEB 地址

1. x86 架构下的 FS 寄存器与 TEB

在 32 位 Windows 系统中，使用的是 FS 段寄存器，其作用与 x64 下的 GS 类似：
内存布局：
FS:[0x00] → TEB 的起始位置
FS:[0x30] → 指向 PEB 的指针

对应地，代码中会使用 __readfsdword(0x30) 来读取 PEB 地址。

总结图解

你看到的代码写法可能非常简洁：

rust
let peb = NtCurrentPeb(); // 表面上一步到位

但 CPU 实际执行的步骤（微观视角）如下：

mermaid
graph LR
A[CPU 寄存器 GS/FS] --> 指向 B(当前线程的 TEB)
B --> 偏移 0x60 (x64) 或 0x30 (x86) C(PEB 指针字段)
C --> 存储的值就是 D[PEB 结构体基址]

结论

虽然代码中直接调用 NtCurrentPeb() 看似“一步获取 PEB”，但该函数的底层实现本质上就是读取 TEB 中偏移固定的 PEB 指针字段。

TEB 是访问 PEB 的唯一标准入口。没有 TEB，就无法在不调用系统 API 的前提下定位 PEB。因此，在红队技术、Shellcode 或无导入表（IAT-less）加载器中，“从 TEB 开始”是描述这一底层寻址链条的准确说法。

这也是为什么所有手动解析 DLL 列表、绕过 EDR 或实现动态调用的技术，第一步几乎都是：
通过 GS/FS 寄存器 → 读取 TEB → 获取 PEB → 遍历 Ldr 链表
