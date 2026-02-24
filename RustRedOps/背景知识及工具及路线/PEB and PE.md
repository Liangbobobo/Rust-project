- [工具](#工具)
- [基础知识](#基础知识)
  - [ASCII UTF-8 UTF-16 ANSI / Code Pages (GBK, Latin1)](#ascii-utf-8-utf-16-ansi--code-pages-gbk-latin1)
    - [什么是编码?](#什么是编码)
    - [ASCII (American Standard Code for Information Interchange)](#ascii-american-standard-code-for-information-interchange)
    - [OEM Code Pages (ANSI / DBCS) —— “乱码之源”](#oem-code-pages-ansi--dbcs--乱码之源)
    - [UTF-16 LE (Little Endian) —— Windows 的皇冠](#utf-16-le-little-endian--windows-的皇冠)
    - [UTF-8 —— Rust 与现代网络的标准](#utf-8--rust-与现代网络的标准)
    - [pe peb结构体及其字段编码格式](#pe-peb结构体及其字段编码格式)
  - [Red Team中常用的u8 u16 raw pointer之间的转化及安全性操作(垂悬指针等问题) `&[u8]` (ASCII) 与 `&[u16]` (UTF-16)之间转换](#red-team中常用的u8-u16-raw-pointer之间的转化及安全性操作垂悬指针等问题-u8-ascii-与-u16-utf-16之间转换)
    - [`&[u8]` (ASCII) 与 `&[u16]` (UTF-16)之间转换(Rust中不能直接转换)](#u8-ascii-与-u16-utf-16之间转换rust中不能直接转换)
      - [不同解决方案](#不同解决方案)
  - [ffi::c\_void \*mut c\_void \*const c\_void](#ffic_void-mut-c_void-const-c_void)
  - [rust中引用和指针的区别](#rust中引用和指针的区别)
- [前言](#前言)
  - [dinvk等项目操作pe peb的原理](#dinvk等项目操作pe-peb的原理)
- [TEB](#teb)
- [PEB相关(使用dinvk中的例子及windbg)](#peb相关使用dinvk中的例子及windbg)
    - [一个可执行文件产生多个进程的PEB情况?](#一个可执行文件产生多个进程的peb情况)
  - [ApiSetMap](#apisetmap)
    - [结构体定义](#结构体定义)
      - [API\_SET\_NAMESPACE(根结构体,Schema 头部)](#api_set_namespace根结构体schema-头部)
      - [API\_SET\_NAMESPACE\_ENTRY (虚拟模块条目)](#api_set_namespace_entry-虚拟模块条目)
      - [API\_SET\_VALUE\_ENTRY (重定向目标/宿主条目)](#api_set_value_entry-重定向目标宿主条目)
      - [API\_SET\_HASH\_ENTRY (哈希索引条目)](#api_set_hash_entry-哈希索引条目)
    - [puerto中resolve\_api\_set\_map解析apisetmap的逻辑](#puerto中resolve_api_set_map解析apisetmap的逻辑)
    - [免杀视角 —— 这些结构体在对抗中的核心作用](#免杀视角--这些结构体在对抗中的核心作用)
    - [如果虚拟dll在ApiSetMap中找不到呢](#如果虚拟dll在apisetmap中找不到呢)
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
    - [总结](#总结)
- [PE(Portable Executable)](#peportable-executable)
  - [PE Memory Layout](#pe-memory-layout)
  - [PE核心数据结构](#pe核心数据结构)
    - [第一部分：DOS Header（兼容性头部）](#第一部分dos-header兼容性头部)
      - [第二部分：NT Headers（PE 核心头）](#第二部分nt-headerspe-核心头)
        - [2.1 File Header（文件物理概况）](#21-file-header文件物理概况)
        - [2.2 Optional Header（逻辑加载信息）](#22-optional-header逻辑加载信息)
      - [2.3 Data Directory（功能索引表）](#23-data-directory功能索引表)
      - [`IMAGE_EXPORT_DIRECTORY->AddressOfNames` 的内存归属解析](#image_export_directory-addressofnames-的内存归属解析)
        - [1. **物理归属：它是目标 DLL 的一部分**](#1-物理归属它是目标-dll-的一部分)
        - [2. **内存位置：位于当前进程的虚拟地址空间内**](#2-内存位置位于当前进程的虚拟地址空间内)
        - [3. **技术细节修正：它是一个“RVA 数组”，而非“字符串数组”**](#3-技术细节修正它是一个rva-数组而非字符串数组)
        - [总结](#总结-1)
      - [第三部分：Section Headers（节表）](#第三部分section-headers节表)
      - [总结：Windows PE 加载流程](#总结windows-pe-加载流程)
  - [RVA FOA](#rva-foa)
    - [为什么如果一个导出函数的 RVA 指向了导出目录 (Export Directory) 所在的内存范围内那么它一定不是代码，而是一个转发字符串 (Forwarder String)](#为什么如果一个导出函数的-rva-指向了导出目录-export-directory-所在的内存范围内那么它一定不是代码而是一个转发字符串-forwarder-string)
- [PE和PEB的三大核心联系](#pe和peb的三大核心联系)
    - [联系一:定位主模块基址 (PEB.ImageBaseAddress)](#联系一定位主模块基址-pebimagebaseaddress)
    - [联系二:管理所有加载的 PE 模块 (PEB.Ldr)](#联系二管理所有加载的-pe-模块-pebldr)
    - [联系三: 数据目录的运行时访问（Data Directories）](#联系三-数据目录的运行时访问data-directories)
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


# 工具

逆向经典：Windows Internals Part 1 (7th Ed), Chapter 3 "System Mechanisms"

# 基础知识

## ASCII UTF-8 UTF-16 ANSI / Code Pages (GBK, Latin1)

这不仅是编程基础，这是内存取证（Memory Forensics）和反编译（Reverse Engineering）的基石。

### 什么是编码?

字符编码的本质:在 CPU 眼里，没有“文字”，只有数字。编码就是一本“字典”，规定了哪个数字代表哪个图形。而在程序的源码中,有些内容需要编码,有些比如源码中的结构体不需要编码,只是二进制数据.

之所以说结构体本身没有“编码格式”，是因为在计算机科学中，“编码”是专门为“文本（字符）”设计的，而结构体是由“二进制数值”组成的。

1. 文本 vs 数值（概念的区别）
* 编码(Encoding)：是将“抽象的字符”（如：'A'，'中'）映射为“二进制数字”的字典。只有当你处理文本时，才需要讨论它是 UTF-8、ASCII 还是 UTF-16。
* 二进制数值 (Binary Value)：结构体成员（如 Length）是直接的数字。
* 比如 Length 是 20，在内存里就是 1400（十六进制）。这不代表任何字符，它就是一个长度数值。
* 数值不需要“编码字典”，CPU 的寄存器直接就能读懂它。


2. 结构体就像一个“标签”或“信封”
想象一个信封：
* 信封上的属性：
* 信件长度：100 字节（这是一个数值）
* 信件地址：内存 0x123456（这是一个指针）
* 信封里的内容：
* 一封用“中文”写的信（这就是编码，比如 UTF-16）。


比如c中常见的`UNICODE_STRING`结构体本身就是那个“信封”。它只负责告诉你：字符串有多长、在哪个地址。它自己不包含任何文本字符，所以它不需要编码。

3. 内存中的真实样子 (x64 环境)

```rust
#[repr(C)]
#[derive(Copy, Clone)]
pub struct UNICODE_STRING {
    pub Length: u16,
    pub MaximumLength: u16,
    pub Buffer: *const u16,
}
```

| 偏移  | 字段名        | 内存数据 (示例)       | 说明 |
|-------|---------------|------------------------|------|
| +...  | Length        | 0C 00                  | 二进制数 12，表示 12 字节 |
| +...  | `MaximumLength`  | 0E 00                  | 二进制数 14，表示缓冲区 ... |
| +...  | (填充)        | 00 00 00 00            | 为了 8 字节对齐而存在的空白 |
| +...  | Buffer        | `A0 55 44 33 22 ...    | 内存地址，指向真正存字符... |

结论：  
你看，这里面全是十六进制的数字（二进制值）。只有当你顺着 Buffer提供的地址 0x00001122334455A0 找过去，读到的那一串数据，才需要用 UTF-16LE编码去解释。


一句话总结：  
结构体是“元数据（管理数据的数据）”，它是纯二进制数值；只有它指向的缓冲区才是“文本数据”，才需要编码。

### ASCII (American Standard Code for Information Interchange)

- 状态：现代计算机的始祖，所有编码的子集。
- 定义：使用 7个比特 (bit) 表示 128 个字符（0x00 - 0x7F）。
- 内存形态 (Hex)：
  - A -> 41
  - a -> 61
  - 1 -> 31
  - . -> 2E
- C语言/Windows 特性：
  - Null-Terminated：在 C 语言中（char*），字符串必须以 0x00 结尾。
  - 例如 "ABC" 在内存中占用 4 字节：41 42 43 00。
- Rust 类型：&[u8] 或 b"ABC"。

  🔴 红队视角 (Red Team Ops)

- 导出表陷阱：PE 文件的导出表（Export Table）中，函数名（如 OpenProcess）永远是 ASCII编码的。
- 致命错误：如果你用 UTF-16 的哈希算法去算导出表里的函数名，你的 Shellcode永远找不到地址。
- 特征码检测：LoadLibraryA 这个字符串在内存中就是 4C 6F 61 64...。这是一个极强的静态特征。

---

### OEM Code Pages (ANSI / DBCS) —— “乱码之源”

- 状态：在 Unicode 诞生前的权宜之计（Windows 95/98 时代遗留）。
- 定义：根据操作系统的“区域设置”不同，同一个字节代表不同含义。
- 0x00-0x7F：兼容 ASCII。
- 0x80-0xFF：高位字节，不仅自己有意义，还可能和后一个字节组合。
- 例子：
- GBK (CP936, 中文)：0xD6 0xD0 = "中"。
- Latin-1 (CP1252, 西欧)：0xD6 = "Ö" (带分音符的O)。
- 内存形态：单字节或双字节混排。

🔴 红队视角

- 环境敏感性：如果你的木马是用 GBK编码写的中文提示信息，扔到一台美国（CP1252）的服务器上运行，弹出的信息就是乱码。
- API 版本：MessageBoxA、CreateProcessA 中的 A 就是指 ANSI。这些 API 会根据当前系统的 CodePage 把字符串转成 Unicode 再交给内核。
- 路径解析漏洞：某些安全软件在处理 ANSI路径时存在溢出漏洞，通过构造特殊的“双字节字符”路径，可能绕过检测。

---

### UTF-16 LE (Little Endian) —— Windows 的皇冠

- 状态：Windows 内核的原生编码。NT 内核在 1993 年设计时选择了当时被认为最先进的UCS-2（后演化为 UTF-16）。
- 定义：
  - 基本平面（BMP, 常用字）：使用 2 个字节 (u16)。
  - 增补平面（Emoji, 生僻字）：使用 4 个字节 (双 u16，称为代理对 Surrogate Pairs)。
- Little Endian (小端序)：这是最关键的。低位字节在低地址。
  - 字符 A (Unicode 0x0041) -> 内存存储为 41 00。
  - 字符 中 (Unicode 0x4E2D) -> 内存存储为 2D 4E。
- Windows 特性：
  - WCHAR / PWSTR：C 语言中的宽字符指针，要求以 2字节的 Null (`00 00`) 结尾。
  - UNICODE_STRING：内核结构体，不要求 Null 结尾，依靠 Length 字段。ApiSetMap里的字符串就属于这种类型！
- Rust 类型：`Vec<u16>`

  🔴 红队视角

- API 调用：几乎所有现代 Windows API（ntdll.dll, kernel32.dll）底层只接受 UTF-16。如果你传ASCII，系统要在内部做一次 RtlAnsiStringToUnicodeString，不仅慢，还可能因为 Code Page导致转换错误。
- 00 字节特征：
- ASCII: c m d -> 63 6D 64 (紧凑)
- UTF-16: c m d -> 63 00 6D 00 64 00 (稀疏)
- 检测：安全人员只要在内存 Hex 视图里看到大量的 00 间隔，就知道这是 UTF-16字符串区域。
- ApiSetMap 解析：你在解析 ApiSetMap 时，NameOffset 指向的数据就是 UTF-16LE。如果你直接把它当 ASCII 读，会读到 a (0x61)，然后读到0x00，你的字符串读取函数就会以为字符串结束了，导致只能读出一个字母

---

### UTF-8 —— Rust 与现代网络的标准

- 状态：互联网通用的标准，Rust 的 String 默认编码。
- 定义：变长编码（1-4 字节）。
- ASCII 字符：1 字节 (和 ASCII 一模一样，0x41)。
- 中文：通常 3 字节 (中 -> E4 B8 AD)。
- Emoji：通常 4 字节。
- Rust 类型：String, &str。

🔴 红队视角

- C2 通信：你的木马回传数据给控制台（Cobalt Strike / Sliver）时，通常是 JSON 或 XML格式，这些全是 UTF-8。
- 主要冲突：Rust 的世界是 UTF-8，Windows 的世界是 UTF-16。
- 你在写代码时：let name = "kernel32.dll"; (这是 UTF-8)。
- 你调用 API 时：LdrLoadDll 需要 buffer: *mut u16 (这是 UTF-16)。
- 必须转换：你必须时刻进行 UTF-8 -> UTF-16 的转换（expanding）和 UTF-16 -> UTF-8的转换（narrowing）。

**第二章：内存指纹深度对比（Hex View）**

| 编码       | 内存十六进制 (Hex) | 解释                                 | 长度    |
|------------|--------------------|--------------------------------------|---------|
| ASCII      | 41 3F              | 41('A'), '中'无法表示，变成3F('?')   | 2 bytes |
| GBK (ANSI) | 41 D6 D0           | 41('A'), D6 D0('中' Code Page)     | 3 bytes |
| UTF-16 LE  | 41 00 2D 4E        | 41 00('A'), 2D 4E('中')            | 4 bytes |
| UTF-16 BE  | 00 41 4E 2D        | 大端序，Windows 不用 这个           | 4 bytes |
| UTF-8      | 41 E4 B8 AD        | 41('A'), E4 B8 AD('中')            | 4 bytes |

### pe peb结构体及其字段编码格式

✦ 这是一个极具实战价值的问题。在红队开发（特别是手动映射、各种注入、Shellcode 编写）中，混淆PE 文件头（静态）和 PEB（动态）里的编码格式，是导致 Payload崩溃或者被杀软静态查杀的根本原因之一。

简单总结规律：

- PE 文件头（磁盘/内存中的镜像）：90% 是 ASCII（遗留产物，为了兼容 DOS 时代）。
- PEB（进程环境块/系统加载器）：99% 是 UTF-16 LE（NT 内核的原生语言）。

下面是详细的字段级拆解，附带 Rust 开发中的注意事项。

  ---

第一部分：PE 结构 (Portable Executable)  
特征：静态文件，或者手动映射到内存中的镜像。主要使用 ASCII。

| 结构体/上下文                     | 字段名             | 编码类型       | 长度特征                                                                 | Rust 类型 / 读取方式                     |
|----------------------------------|--------------------|----------------|--------------------------------------------------------------------------|------------------------------------------|
| **IMAGE_DOS_HEADER**             | `e_magic`          | ASCII          | 固定 2 字节 (`0x4D 0x5A` → `"MZ"`)                                       | `u16`                                    |
| **IMAGE_NT_HEADERS**             | `Signature`        | ASCII          | 固定 4 字节 (`0x50 0x45 0x00 0x00` → `"PE\0\0"`)                        | `u32`                                    |
| **IMAGE_SECTION_HEADER**         | `Name`             | **ASCII**      | 固定 8 字节 (`[u8; 8]`)。<br>坑：如果名字刚好 8 字符，则**不以 `\0` 结尾**。 | `&[u8; 8]` 或转为 `String`（需手动处理） |
| **IMAGE_EXPORT_DIRECTORY**       | `Name`             | ASCII          | RVA 指向一个以 `\0` 结尾的字符串（DLL 原始文件名，如 `"KERNEL32.dll"`）    | 通过 RVA 读取 `CStr` → `.to_bytes()`     |
| **IMAGE_EXPORT_DIRECTORY**       | `AddressOfNames`   | ASCII          | RVA 数组，每个 RVA 指向一个 `\0` 结尾的函数名（如 `"CreateFileW"`）        | 通过 RVA 读取 `CStr` → `.to_string_lossy()` |
| **IMAGE_IMPORT_DESCRIPTOR**      | `Name`             | ASCII          | RVA 指向导入的 DLL 名（如 `"USER32.dll"`），以 `\0` 结尾                  | 通过 RVA 读取 `CStr`                     |
| **IMAGE_THUNK_DATA / IAT**       | (间接指向)         | ASCII          | 具体导入的函数名（若按名称导入），以 `\0` 结尾                            | 通过 `IMAGE_IMPORT_BY_NAME.Name` 读取    |
| **资源段 (Resource Directory)**  | `Name`             | **Unicode**    | 例外情况！资源段中的字符串通常是 **UTF-16**（宽字符）                      | `Vec<u16>` → `OsString::from_wide()`     |

  🔴 红队实战警告：
  你在写 get_proc_address（获取导出函数地址）时，PE 导出表里的函数名是 ASCII。

- 如果你想查找 LoadLibraryW。
- 导出表里存的是 0x4C 0x6F ... (ASCII)。
- 千万不要把这个字节流直接强转成 u16 去算哈希，除非你的哈希算法专门处理了这种情况。



第二部分：PEB 结构 (Process Environment Block)
特征：操作系统在运行时生成的管理结构，位于内存中。主要使用 UTF-16 LE。


| 结构体 / 位置                   | 字段名               | 类型                | 结构特征                                                                 | Rust 处理方式                                      |
|--------------------------------|----------------------|---------------------|--------------------------------------------------------------------------|----------------------------------------------------|
| `PEB_LDR_DATA`                 | （无直接字符串字段）  | —                   | 这是一个链表头结构（如 `InLoadOrderModuleList`），本身不存储任何字符串。     | —                                                  |
| **`LDR_DATA_TABLE_ENTRY`**     | `FullDllName`        | `UNICODE_STRING*`   | 包含已加载模块的完整路径（例如：`C:\Windows\System32\kernel32.dll`）。       | `&[u16]`（通过 `Buffer` + `Length / 2` 安全切片）   |
| **`LDR_DATA_TABLE_ENTRY`**     | `BaseDllName`        | `UNICODE_STRING*`   | 仅包含模块文件名（例如：`kernel32.dll`），不含路径。                         | `&[u16]`（注意：可能无 null 终止符，需用 `Length`）|
| **`RTL_USER_PROCESS_PARAMETERS`** | `ImagePathName`    | `UNICODE_STRING*`   | 当前进程可执行文件的完整路径（启动时由系统传入）。                           | `&[u16]`                                           |
| **`RTL_USER_PROCESS_PARAMETERS`** | `CommandLine`      | `UNICODE_STRING*`   | 启动命令行参数（例如：`"myapp.exe" --debug`）。                             | `&[u16]`                                           |
| **`RTL_USER_PROCESS_PARAMETERS`** | `WindowTitle`      | `UNICODE_STRING*`   | 进程创建时指定的窗口标题（主要用于 GUI 应用）。                              | `&[u16]`                                           |
| **`RTL_USER_PROCESS_PARAMETERS`** | `Environment`      | `*mut u16`          | 环境变量块。格式为连续的宽字符串：`Key=Value\0Key2=Value2\0...\0\0`。        | 需手动遍历解析（按 `\0` 分割条目，再按 `=` 拆分键值）|
| `ApiSetMap` (v6)               | `NameOffset`         | `u32`（相对偏移）   | 指向虚拟 DLL 名称（如 `api-ms-win-core-heap-l1-2-0`），**无 null 结尾**。    | `&[u16]`（长度由对应 `NameLength` 字段指定）       |
| `ApiSetMap` (v6)               | `ValueOffset`        | `u32`（相对偏移）   | 指向宿主 DLL 名称（如 `kernelbase.dll`），**无 null 结尾**。                | `&[u16]`（长度由对应 `ValueLength` 字段指定）       |

🔴 红队实战警告：

- UNICODE_STRING 陷阱：PEB 中的字符串大多被封装在 UNICODE_STRING 结构体中。

```c
        struct UNICODE_STRING {
            USHORT Length;        // 字节长度（不含结尾空）
            USHORT MaximumLength; // 缓冲区总大小
            PWSTR  Buffer;        // 指向 UTF-16 数据的指针
        };
```

- 一定要用 Length：虽然 Windows 通常会在 Buffer 后面好心放一个 00
     00，但不要依赖它。规范的做法是只读取 Length 指定的字节数。
- 大小写敏感：LDR 中的 DLL
     名称有时是大写，有时是小写（取决于加载方式）。计算哈希时最好统一转成小写（Lower
     Case）或大写处理。

  ---

第三部分：一个极度混淆的特例 —— "Forwarder String"

在导出表（PE 结构）中，有一种特殊情况叫 Forwarder（转发器）。

- 位置：当导出函数的 RVA 指向导出表自身范围内部时。
- 内容：这代表这个函数不是当前 DLL 实现的，而是转发给别的 DLL。
- 格式：NTDLL.RtlAllocateHeap（模块名.函数名）。
- 编码：ASCII。

  坑点：
  你的代码逻辑是这样的：

   1. 解析 PEB（UTF-16）找到了 kernel32.dll。
   2. 解析 kernel32.dll 的导出表（ASCII），找到了 HeapAlloc。
   3. 发现 HeapAlloc 是个转发器，指向 NTDLL.RtlAllocateHeap（ASCII 字符串）。
   4. 你需要去加载 ntdll.dll。
   - 转换时刻：你手里拿着 ASCII 的 "ntdll.dll"，但 LdrLoadDll 需要 UTF-16 的"ntdll.dll"。这里必须做一次 ASCII -> UTF-16 的转换。

  ---


总结

- 找文件头、找导出函数 -> 盯着 ASCII (`u8`)，注意 \0 结尾。
- 找模块基址、解析 ApiSet、伪装命令行 -> 盯着 UTF-16 (`u16`)，注意 UNICODE_STRING 的Length。

具体pe peb字段及其关联的结构是二进制数据还是带有编码格式的数据,在pe peb内容中会详细分析

## Red Team中常用的u8 u16 raw pointer之间的转化及安全性操作(垂悬指针等问题) `&[u8]` (ASCII) 与 `&[u16]` (UTF-16)之间转换

###  `&[u8]` (ASCII) 与 `&[u16]` (UTF-16)之间转换(Rust中不能直接转换)

Rust 中，切片 `&[T]` 要求内存必须是连续且对齐的

因为 u16 占 2 字节，而u8 占 1 字节，你无法在不移动数据或分配新空间的情况下，直接把 `&[u8] 转成&[u16]`

为什么不能直接强转指针？  
```rust
// 错误示范
let ptr = name_slice.as_ptr() as *const u16;
let slice = from_raw_parts(ptr, len);
```

如果你强转，CPU 会每 2 个字节读一次内存。如果你原始数据是 [0x41, 0x42,0x43, 0x44] ("ABCD")，强转后你会读到 [0x4241, 0x4443]。这和你想要模拟的[0x0041, 0x0042, 0x0043, 0x0044]完全不同。所以物理上的内存转换是必须的，除非你改变哈希计算的步长

#### 不同解决方案

方案一：使用“双重哈希”策略（兼容性最好）

由于 PEB (LDR) 里是 UTF-16，而 EAT (导出表) 里是ASCII。为了避免类型转换的麻烦，最简单的办法是针对同一算法实现两个版本的哈希函数

1. hash_u16(data: &[u16]) -> 用于模块名
2. hash_u8(data: &[u8]) -> 用于函数名

实现原理：  
由于 ASCII 本质上是高位为 0 的 UTF-16。只要你的 hash_u8在计算时，逻辑上把每个字节当成 u16 来处理（即结果与 u16版本一致），你就可以直接传入 `&[u8]`


方案二:重构哈希函数为“流式”计算（最地道，推荐）  

在红队编程中，为了灵活性，通常不会传递 `fn(&[u16]) -> u32`这种死板的函数指针。更好的做法是让哈希函数支持增量更新

```rust
pub fn fnv1a_generic<I>(iter: I) -> u32
// IntoIterator是一个trait,这里代表I是可以转换为迭代器的类型,
// Item,关联类型,代表迭代时每个元素的类型必须是u16
 where I: IntoIterator<Item = u16>
 {
    let mut hash = 0x811c9dc5;
    for code in iter {
      hash ^= code as u32;
      hash = hash.wrapping_mul(0x01000193);}
      hash
 }
```

在 `get_proc_address` 中调用:  

// 无需创建数组，直接传递迭代器  
let func_hash = fnv1a_generic(name_slice.iter().map(|&b| b as u16));

原理：迭代器是延迟计算的（Lazy）。它会逐个取出 u8，强转为u16，然后立即参与哈希运算。全程不需要存储中间的 `u16` 数组  
优点：完美解决签名问题，且内存占用为 0


方案三(最直接,无堆分配):  

能够保证`&[u8]`类型的变量不超过固定长度情况下,可以在栈上开辟一个固定大小的缓冲区,存放转换之后的数据

```rust
let mut buffer = [0u16; 256];

let len = if name_slice.len() > 256 { 256 } else { name_slice.len() };

// 手动拷贝并提升类型
for i in 0..len {
buffer[i] = name_slice[i] as u16;
}

// 传递切片的一段引用
let func_hash = (hash_func.unwrap())(&buffer[..len]);
```

完全没有 alloc，速度极快;  
在当前函数的栈帧中分配了一块空间。这种方式符合 `&[u16]`  
如果函数名超过 256（极少见），哈希会不匹配

## ffi::c_void *mut c_void *const c_void

在 Rust FFI（外部函数接口）中:

| 表达式 | 类型层级 | C 语言等价 | 用途 | 安全性/使用场景 |
|--------|----------|------------|------|-----------------|
| `ffi::c_void` | 类型本身 | `void`（抽象类型） | 作为指针的目标类型占位符 | ❌ 不能实例化（零大小、不透明） |
| `*const c_void` | 不可变原始指针 | `const void*` | 传递只读内存地址（如 `memcmp` 参数） | ✅ 可安全创建；解引用需 `unsafe` |
| `*mut c_void` | 可变原始指针 | `void*` | 传递可写内存地址（如 `memcpy` 目标） | ✅ 可安全创建；解引用/写入需 `unsafe` |

相互之间的转换:

| 项目 | 说明 |
|------|------|
| 指针转换 | `*mut T` → `*mut c_void`：安全（`as` 转换）<br>`*const c_void` → `*mut c_void`：需 `unsafe`（破坏不可变性） |
| 解引用 | 所有原始指针解引用必须在 `unsafe` 块中进行 |
| 空指针 | `std::ptr::null()` → `*const c_void`<br>`std::ptr::null_mut()` → `*mut c_void` |
| Windows HANDLE | 在 `windows-rs` 等 crate 中，`HANDLE` 常定义为 `*mut c_void`（如 `NtAllocateVirtualMemory` 的 `BaseAddress`） |
| 为什么不用 `()` | `()` 是 Rust 元组类型，有明确内存布局（0 字节但可实例化）；`c_void` 是 FFI 专用不透明类型，语义更准确 |


c_void：FFI 中的“类型占位符”，永远不直接使用值。

`*const c_void：安全传递只读内存地址（C 的 const void*）。
`
`*mut c_void：安全传递可写内存地址（C 的 void*）`，是 Windows/Linux 系统 API 中“通用指针”的标准表示。

## rust中引用和指针的区别

引用是指针吗?不是

在 Rust 中，引用（reference）和原始指针（raw pointer）有本质区别；从语言设计层面看：引用 ≠ 指针，尽管底层实现上引用通常编译为指针（内存地址）。

| 维度 | 引用 (`&T`, `&mut T`) | 原始指针 (`*const T`, `*mut T`) |
|------|------------------------|----------------------------------|
| 语言层级 | 高级安全抽象（所有权系统核心） | 低级内存操作工具 |
| 空值 | ❌ 永不为 null（编译器保证） | ✅ 可为 null（需手动检查） |
| 悬垂 | ❌ 编译器通过生命周期检查杜绝 | ✅ 可能悬垂（需开发者负责） |
| 别名规则 | ✅ 严格：`&mut T` 独占，`&T` 可共享 | ❌ 无限制（可任意别名） |
| 解引用 | ✅ 安全（无需 `unsafe`） | ❌ 必须 `unsafe` 块 |
| 指针运算 | ❌ 禁止 | ✅ 允许（如 `ptr.offset(1)`） |
| 生命周期 | ✅ 有显式/隐式生命周期参数 | ❌ 无（但使用时需注意） |
| 创建 | ✅ 安全（`&x`） | ✅ 安全（`&x as *const _`） |
| 典型场景 | 日常安全代码、函数参数传递 | FFI、OS 内核、自定义分配器、绕过借用检查 |

引用是指针吗？

| 层面 | 回答 |
|------|------|
| 机器码层面 | ✅ 是。引用编译后通常是一个内存地址（与指针二进制表示相同） |
| Rust 语言设计层面 | ❌ 不是。引用是带有编译时安全契约（生命周期、借用规则）的独立类型，刻意与“裸指针”语义解耦 |
| 开发者心智模型 | 🚫 不应视为指针。将引用理解为“安全借用的句柄”更符合 Rust 哲学 |

Rust 创始人 Graydon Hoare 明确表示：  
“References are not pointers. They are a safe abstraction over pointers.”

误区：Option<&T> 内部用 null 优化，所以引用可以是 null  
正解：Option<&T> 是安全抽象，引用本身永不为 null，null 仅作为 None 的内部表示（编译器保证使用者无法接触）

误区：&mut T 和 *mut T 都是“可变”，可以互换  
正解：&mut T 有独占性保证（无数据竞争），*mut T 无任何保证，混用极易导致 UB

误区：原始指针 = 不安全 = 应避免  
正解：原始指针是必要工具（如实现 Vec、与 C 交互），关键在于：创建安全，使用需 unsafe 且开发者自证安全

引用是 Rust 内存安全的基石，原始指针是 突破安全边界的工具。
理解二者的界限，是写出既安全又高效 Rust 代码的关键。

# 前言

## dinvk等项目操作pe peb的原理

在对pe peb等属于windows用户态或内核的各种数据结构进行操作时(比如获取peb结构\pe结构,操作结构中的各个字段),利用了自定义的各种数据结构(types.rs).此时你并没有“创建”这些结构，你只是在画一张“地图”来解释已经存在的地形

程序运行起来的那一瞬间，操作系统加载器（Windows Loader）已经在内存里铺好了一大块数据。这是一串连续的 010101二进制流。不管你定义不定义结构体，数据就在那里.在 types.rs 里写的 struct API_SET_NAMESPACE实际上不是在让计算机去“分配”或“初始化”内存。你是在告诉 Rust 编译器当我拿到一个指向 0x12345678 的指针时，请把它后面的 前4个字节 当作Version，再后面4个字节 当作 Size

所以:  

1. 数据来源：是 Windows 内核在进程启动时填进去的（系统自动生成且初始化好的）
2. 你的定义：是一个模板（Template）。你把这个模板“扣”在那个内存地址上，以便你能用 .Version这种人类可读的方式去访问那段二进制数据
3. 初始化是谁做的?Windows 内核做的.当你调用 let map = (*peb).ApiSetMap 时，你拿到的这个指针，指向的是一块只读的系统内存.这块内存里的每一个比特，早在你的 main 函数执行之前，就已经被 Windows 填好了.在 types.rs 里定义的结构体，那些字段（比如 Reserved 或Flags）即使你不用，你也必须把它们写出来，或者用 pad: u32 占位。如果你跳过了中间的一个 u32 没定义，那么后面所有的字段偏移量都会错乱。比如，本来 Count在偏移 12 字节处，如果你少写了一个前面的字段，你的代码就会去偏移 8 字节的地方读Count，读出来的就是乱码(所以有很多pading,为了对齐Alignment)

# TEB

# PEB相关(使用dinvk中的例子及windbg)

请记得使用windbg对结构体进行分析,所有的结构体可以用这种方式reveal

1. **结构体不透明性**：`EPROCESS`, `ETHREAD`, `KPROCESS` 等内核结构体是 **非公开 (Opaque)** 的。微软从未保证其成员偏移量（Offsets）的稳定性。文中标记的偏移量仅为示例或特定历史版本，实战中**必须**通过符号文件 (`.pdb`) 或运行时特征码搜索动态获取。
2. **ASLR (地址空间布局随机化)**：现代 Windows (Vista+) 强制开启 ASLR。文中出现的内存地址仅为**逻辑示意**，实际运行时基址、堆栈地址每次启动均不同。
3. **架构限定**：本文核心描述 **x64 (AMD64)** 架构下的 Windows 运行机制。(Intel 64、AMD64 和 x86_64指的是同一种指令集架构,Windows 操作系统在底层通常统一使用 AMD64 来标识 64位架构，无论你的 CPU 是 Intel 还是 AMD 生产的。)
4. 在 Windows 中，每个进程都有一个 PEB (Process Environment Block).进程（Process）在 Windows内核对象（EPROCESS）的定义中，就是资源和地址空间的容器。而 PEB是这个容器在用户模式（User Mode）下的管理结构。当内核创建一个新进程时（NtCreateUserProcess），它必须在分配的虚拟地址空间中映射并初始化一个 PEB。没有 PEB，ntdll.dll 无法初始化，用户模式的代码（包括 main函数）根本无法开始执行。

```rust
pub struct PEB {
    pub InheritedAddressSpace: u8, // 类型: u8 (布尔标志) - 是否从父进程继承地址空间 (0=否, 非0=是)
    pub ReadImageFileExecOptions: u8, // 类型: u8 (布尔标志) - 是否从映像文件读取执行选项
    pub BeingDebugged: u8, // 类型: u8 (布尔标志) - 进程是否被调试器附加 (1=是)
    pub Anonymous1: PEB_0, // 类型: PEB_0 (联合体) - 包含BitField/Reserved等字段 (具体结构见PEB_0定义)
    pub Mutant: HANDLE, // 类型: HANDLE (*mut c_void) - 进程互斥体句柄 (通常为NULL表示主进程)
    pub ImageBaseAddress: *mut c_void, // 类型: *mut c_void - 指向当前进程EXE映像基地址 (IMAGE_DOS_HEADER起始)
    pub Ldr: *mut PEB_LDR_DATA, // 类型: *mut PEB_LDR_DATA - 指向PEB_LDR_DATA结构 (模块加载链表头)
    pub ProcessParameters: *mut RTL_USER_PROCESS_PARAMETERS, // 类型: *mut RTL_USER_PROCESS_PARAMETERS - 指向进程启动参数结构 (含ImagePathName/CommandLine等)
    pub SubSystemData: *mut c_void, // 类型: *mut c_void - 子系统专用数据 (通常为NULL)
    pub ProcessHeap: *mut c_void, // 类型: *mut c_void - 指向进程默认堆 (HEAP结构)
    pub FastPebLock: *mut RTL_CRITICAL_SECTION, // 类型: *mut RTL_CRITICAL_SECTION - 指向PEB临界区锁
    pub AtlThunkSListPtr: *mut SLIST_HEADER, // 类型: *mut SLIST_HEADER - ATL单向链表头指针
    pub IFEOKey: *mut c_void, // 类型: *mut c_void - 指向Image File Execution Options注册表键句柄
    pub Anonymous2: PEB_1, // 类型: PEB_1 (联合体) - 包含CrossProcessFlags/Reserved等字段
    pub Anonymous3: PEB_2, // 类型: PEB_2 (联合体) - 包含KernelCallbackTable/ReservedForWin32等字段
    pub SystemReserved: u32, // 类型: u32 - 系统保留字段
    pub AtlThunkSListPtr32: u32, // 类型: u32 - 32位ATL thunk链表指针 (WoW64环境使用)
    pub ApiSetMap: *mut API_SET_NAMESPACE, // 类型: *mut API_SET_NAMESPACE - 指向ApiSet映射表 (解析api-ms-*.dll到真实DLL)
    pub TlsExpansionCounter: u32, // 类型: u32 - TLS扩展索引计数器
    pub TlsBitmap: *mut RTL_BITMAP, // 类型: *mut RTL_BITMAP - 指向TLS位图结构 (管理TLS槽位分配)
    pub TlsBitmapBits: [u32; 2], // 类型: [u32; 2] - TLS位图缓存 (前64个槽位状态)
    pub ReadOnlySharedMemoryBase: *mut c_void, // 类型: *mut c_void - 指向只读共享内存基址 (KUSER_SHARED_DATA映射)
    pub SharedData: *mut SILO_USER_SHARED_DATA, // 类型: *mut SILO_USER_SHARED_DATA - 指向共享用户数据 (KUSER_SHARED_DATA别名)
    pub ReadOnlyStaticServerData: *mut c_void, // 类型: *mut c_void - 指向只读静态服务器数据数组
    pub AnsiCodePageData: *mut c_void, // 类型: *mut c_void - 指向ANSI代码页数据 (NLS)
    pub OemCodePageData: *mut c_void, // 类型: *mut c_void - 指向OEM代码页数据 (NLS)
    pub UnicodeCaseTableData: *mut c_void, // 类型: *mut c_void - 指向Unicode大小写转换表
    pub NumberOfProcessors: u32, // 类型: u32 - 系统CPU核心数
    pub NtGlobalFlag: u32, // 类型: u32 - 全局标志 (影响堆行为/调试等，如FLG_HEAP_ENABLE_TAIL_CHECK)
    pub CriticalSectionTimeout: LARGE_INTEGER, // 类型: LARGE_INTEGER - 临界区超时时间 (100ns单位)
    pub HeapSegmentReserve: usize, // 类型: usize - 堆段预留大小 (字节)
    pub HeapSegmentCommit: usize, // 类型: usize - 堆段提交大小 (字节)
    pub HeapDeCommitTotalFreeThreshold: usize, // 类型: usize - 堆反提交总空闲阈值
    pub HeapDeCommitFreeBlockThreshold: usize, // 类型: usize - 堆反提交空闲块阈值
    pub NumberOfHeaps: u32, // 类型: u32 - 进程堆数量
    pub MaximumNumberOfHeaps: u32, // 类型: u32 - 最大堆数量
    pub ProcessHeaps: *mut c_void, // 类型: *mut c_void - 指向堆句柄数组 (HEAP*)
    pub GdiSharedHandleTable: *mut c_void, // 类型: *mut c_void - 指向GDI共享句柄表
    pub ProcessStarterHelper: *mut c_void, // 类型: *mut c_void - 进程启动辅助函数指针
    pub GdiDCAttributeList: u32, // 类型: u32 - GDI DC属性列表大小
    pub LoaderLock: *mut RTL_CRITICAL_SECTION, // 类型: *mut RTL_CRITICAL_SECTION - 指向加载器锁
    pub OSMajorVersion: u32, // 类型: u32 - 操作系统主版本号 (如10)
    pub OSMinorVersion: u32, // 类型: u32 - 操作系统次版本号
    pub OSBuildNumber: u16, // 类型: u16 - 系统构建号 (如19041)
    pub OSCSDVersion: u16, // 类型: u16 - CSD版本 (如"Service Pack 1"的内部编号)
    pub OSPlatformId: u32, // 类型: u32 - 平台ID (VER_PLATFORM_WIN32_NT=2)
    pub ImageSubsystem: u32, // 类型: u32 - 映像子系统 (如IMAGE_SUBSYSTEM_WINDOWS_GUI)
    pub ImageSubsystemMajorVersion: u32, // 类型: u32 - 子系统主版本
    pub ImageSubsystemMinorVersion: u32, // 类型: u32 - 子系统次版本
    pub ActiveProcessAffinityMask: usize, // 类型: usize - 进程CPU亲和性掩码
    pub GdiHandleBuffer: GDI_HANDLE_BUFFER, // 类型: GDI_HANDLE_BUFFER ([u32; 60]) - GDI句柄缓存数组
    pub PostProcessInitRoutine: PPS_POST_PROCESS_INIT_ROUTINE, // 类型: 函数指针 - 进程初始化后回调函数
    pub TlsExpansionBitmap: *mut RTL_BITMAP, // 类型: *mut RTL_BITMAP - 指向TLS扩展位图
    pub TlsExpansionBitmapBits: [u32; 32], // 类型: [u32; 32] - TLS扩展位图缓存 (1024个槽位状态)
    pub SessionId: u32, // 类型: u32 - 会话ID (如控制台会话=1)
    pub AppCompatFlags: ULARGE_INTEGER, // 类型: ULARGE_INTEGER - 应用程序兼容性标志 (全局)
    pub AppCompatFlagsUser: ULARGE_INTEGER, // 类型: ULARGE_INTEGER - 应用程序兼容性标志 (用户)
    pub pShimData: *mut c_void, // 类型: *mut c_void - Shim数据库数据指针
    pub AppCompatInfo: *mut c_void, // 类型: *mut c_void - 应用程序兼容性信息
    pub CSDVersion: UNICODE_STRING, // 类型: UNICODE_STRING - 指向CSD版本字符串 (如"Service Pack 1")
    pub ActivationContextData: *mut ACTIVATION_CONTEXT_DATA, // 类型: *mut ACTIVATION_CONTEXT_DATA - 指向SxS激活上下文数据
    pub ProcessAssemblyStorageMap: *mut ASSEMBLY_STORAGE_MAP, // 类型: *mut ASSEMBLY_STORAGE_MAP - 指向程序集存储映射
    pub SystemDefaultActivationContextData: *mut ACTIVATION_CONTEXT_DATA, // 类型: *mut ACTIVATION_CONTEXT_DATA - 系统默认激活上下文
    pub SystemAssemblyStorageMap: *mut ASSEMBLY_STORAGE_MAP, // 类型: *mut ASSEMBLY_STORAGE_MAP - 系统程序集存储映射
    pub MinimumStackCommit: usize, // 类型: usize - 最小栈提交大小
    pub SparePointers: *mut c_void, // 类型: *mut c_void - 保留指针 (Windows 8+ 用于FlsBitmap)
    pub PatchLoaderData: *mut c_void, // 类型: *mut c_void - 热补丁加载数据
    pub ChpeV2ProcessInfo: *mut c_void, // 类型: *mut c_void - CHPEv2进程信息 (ARM64EC相关)
    pub Anonymous4: PEB_3, // 类型: PEB_3 (联合体) - 包含AppCompatInfo/Reserved等字段
    pub SpareUlongs: [u32; 2], // 类型: [u32; 2] - 保留ULONG数组
    pub ActiveCodePage: u16, // 类型: u16 - 活动ANSI代码页 (如936=GBK)
    pub OemCodePage: u16, // 类型: u16 - OEM代码页 (如437=US)
    pub UseCaseMapping: u16, // 类型: u16 - 是否启用大小写映射 (NLS)
    pub UnusedNlsField: u16, // 类型: u16 - 未使用的NLS字段
    pub WerRegistrationData: *mut WER_PEB_HEADER_BLOCK, // 类型: *mut WER_PEB_HEADER_BLOCK - 指向Windows错误报告注册数据
    pub WerShipAssertPtr: *mut c_void, // 类型: *mut c_void - WER断言处理函数指针
    pub Anonymous5: PEB_4, // 类型: PEB_4 (联合体) - 包含pContextData/Reserved等字段
    pub pImageHeaderHash: *mut c_void, // 类型: *mut c_void - 映像头哈希指针 (用于完整性验证)
    pub Anonymous6: PEB_5, // 类型: PEB_5 (联合体) - 包含TracingFlags/Reserved等字段
    pub CsrServerReadOnlySharedMemoryBase: u64, // 类型: u64 - CSR服务器只读共享内存基址 (跨会话)
    pub TppWorkerpListLock: *mut RTL_CRITICAL_SECTION, // 类型: *mut RTL_CRITICAL_SECTION - 线程池工作者列表锁
    pub TppWorkerpList: LIST_ENTRY, // 类型: LIST_ENTRY - 线程池工作者链表头
    pub WaitOnAddressHashTable: [*mut c_void; 128], // 类型: [*mut c_void; 128] - WaitOnAddress哈希表 (128桶)
    pub TelemetryCoverageHeader: *mut TELEMETRY_COVERAGE_HEADER, // 类型: *mut TELEMETRY_COVERAGE_HEADER - 遥测覆盖率头指针
    pub CloudFileFlags: u32, // 类型: u32 - 云文件标志 (OneDrive等)
    pub CloudFileDiagFlags: u32, // 类型: u32 - 云文件诊断标志
    pub PlaceholderCompatibilityMode: i8, // 类型: i8 - 占位符兼容模式
    pub PlaceholderCompatibilityModeReserved: [i8; 7], // 类型: [i8; 7] - 保留字节
    pub LeapSecondData: *mut c_void, // 类型: *mut c_void - 指向闰秒数据 (PLEAP_SECOND_DATA)
    pub Anonymous7: PEB_6, // 类型: PEB_6 (联合体) - 包含LeapSecondFlags/Reserved等字段
    pub NtGlobalFlag2: u32, // 类型: u32 - 扩展全局标志 (Windows 10 19H1+)
    pub ExtendedFeatureDisableMask: u64, // 类型: u64 - 扩展特性禁用掩码 (如禁用AVX512)
}

```

1. 安全提示：所有指针字段解引用均需 unsafe，且需验证有效性（避免悬垂指针导致 UB）
2. 版本差异：部分字段在 Windows 不同版本中含义可能变化（如 SparePointers 在 Win8+ 用于 FLS），注释已标注典型用途。
3. HANDLE 本质是 *mut c_void，但语义上为句柄（如 Mutant）


- 获取 PE,x64: gs:[0x60],x86: fs:[0x30]

### 一个可执行文件产生多个进程的PEB情况?

一个可执行文件产生多个进程,但这不改变“每个进程有一个 PEB”的事实。

一个可执行文件产生了多个进程，情况如下：  

1. 多开（Multiple Instances）:多次打开同一个可执行文件.系统创建了两个完全独立的进程（PID 1001 和 PID 1002）.PEB: 它们各自拥有自己独立的 PEB。虽然它们来自同一个 .exe文件，但它们在内存中是两个互不相干的世界。
2. 父子进程（Spawning Child Processes）:在一个可执行文件中,调用 CreateProcess("myapp.exe") 自我复制.结果: 一个父进程，一个子进程。 PEB: 依然是两个独立的 PEB。
3. 多线程（Multi-threading）:这是最容易混淆的。一个进程可以包含多个线程。结果: 1 个进程，N 个线程。PEB: 所有这 N 个线程共享同一个 PEB（因为它们属于同一个进程） TEB: 每个线程拥有自己独立的 TEB (Thread Environment Block)。

无论一个 .exe 启动了多少次，或者它自己又派生了多少子进程，只要那是 Windows上的一个标准 Win32 进程，它就一定拥有一个属于它自己的、独一无二的 PEB。

## ApiSetMap

ApiSetMap是peb中的一个字段:peb->ApiSetMap

`pub ApiSetMap: *mut API_SET_NAMESPACE`

请使用windbg(notepad示例)理解ApiSetMap相关结构体的定义,在windbg文件夹中有

在 PEB 结构体中，ApiSetMap 字段被定义为 PVOID（即void*），因为它是一个不透明指针，指向的结构体随着Windows 版本变化（Win7,Win8, Win10 结构体都不一样）。

因为在PEB中,ApiSetMap被定义为一个指针,要找到它真正指向的结构体，你需要去查找 Loader (Ldr) 相关的头文件，而不是 PEB 的头文件。

1. 在phnt的github仓库中,有该结构体的详细定义(ntpebteb.h文件中)
2. Google 搜索：site:geoffchappell.com "API Set Schema"

目前主流环境（Win10/11）使用的是 Schema Version 6。所有的Offset（偏移量）都是相对于 ApiSetMap 结构体起始地址的字节偏移

该字段有多个扩展结构,主要是API_SET_NAMESPACE;API_SET_NAMESPACE_ENTRY;API_SET_VALUE_ENTRY  


**常规查询流程**
1. 通过 Count 遍历所有 API_SET_NAMESPACE_ENTRY
2. 比对 NameOffset 指向的虚拟 DLL 名称
3. 通过 ValueCount 确定需遍历几个 API_SET_VALUE_ENTRY
4. 从 API_SET_VALUE_ENTRY.ValueOffset 读取宿主 DLL 真实名称
5. (可选)NameOffset 用于处理别名重定向

总结图谱

  为了让你在写代码时脑子像 CPU 一样清晰，请看这张数据流向图：

   1. PEB.ApiSetMap (&API_SET_NAMESPACE)
       * EntryOffset (u32)
           * ⬇️ (Base + Offset)
   2. Entry Array (&[API_SET_NAMESPACE_ENTRY])
       * [0] -> ValueOffset (u32)
           * ⬇️ (Base + Offset)
   3. Value Array (&[API_SET_VALUE_ENTRY])
       * [0] -> ValueOffset (u32)
           * ⬇️ (Base + Offset)
   4. Raw Memory (&[u16])
       * 6B 00 65 00 72 00 ... ("kernelbase.dll")

ApiSetMap 是 Windows 用户层的“DNS服务器”,有四个数据结构组成,

详解其作用有:

1. 解决“DLL地狱”与解耦

在 Windows 7 之前，程序依赖kernel32.dll。但随着系统升级，微软想重构内核，把功能移动到 kernelbase.dll 或ucrtbase.dll 中如果直接改文件名，成千上万的老程序（写死了依赖 kernel32.dll）就会崩溃

微软发明了 API Sets（即那些 api-ms-win-core-...dll）,如虚拟文件名：api-ms-win-core-processthreads-l1-1-0.dll,物理文件名：kernel32.dll 或 kernelbase.dll  
操作系统加载器在运行时查这张表，把虚拟名“翻译”成物理名

1. 为什么结构体这么复杂？（因为要支持“千人千面”）

为什么有 NAMESPACE、NAMESPACE_ENTRY、VALUE_ENTRY这么多层级？

假设有一个虚拟 DLL 叫 `api-ms-win-core-memory-l1-1-0.dll`

- 如果是普通程序（如 notepad.exe）加载它，它应该指向 `kernelbase.dll`。
- 如果是某些遗留程序（为了兼容性），它可能指向 `kernel32.dll`。

简单的 Key-Value 做不到, ApiSetMap 的结构逻辑（一对多 + 条件判断）：

1. 第一层（Namespace Entry 数组）：
*你在数组里找到了 `api-ms-win-core-memory-l1-1-0.dll` 这一项。
* 这项数据告诉你：“想知道我到底是谁？去看我的 Value Entry 数组，我有 2 个可能的身份。”

2. 第二层（Value Entry 数组）：
- Value Entry [0]:
- 条件 (Importing Name): "OldLegacyApp.exe"
- 结果 (Host Name): "kernel32.dll"
- 含义：如果是 OldLegacyApp.exe 问我，我就伪装成 kernel32.dll。
- Value Entry [1]:
- 条件 (Importing Name): NULL (无条件/默认)
- 结果 (Host Name): "kernelbase.dll"
- 含义：如果是其他任何人问我，我就指向 kernelbase.dll。

之所以要定义这么多结构体，是因为这不是一个静态的“别名表”，而是一个带有条件判断逻辑的动态路由表。

- NAMESPACE 是数据库入口。
- NAMESPACE_ENTRY 是所有的虚拟 Key。
- VALUE_ENTRY 是带有if-else 条件的物理 Value。


**核心记忆点**

| 概念 | 指向内容 | 决定因素 |
|------|----------|----------|
| 虚拟 DLL | `API_SET_NAMESPACE_ENTRY.NameOffset` | 开发者代码中写的 DLL 名 |
| 宿主 DLL | `API_SET_VALUE_ENTRY.ValueOffset` | 系统实际加载的 DLL |
| 数量 | `API_SET_NAMESPACE_ENTRY.ValueCount` | 一个虚拟 DLL 可映射多个宿主 DLL（fallback 机制） |
| 字符串长度 | `*.Length` 字段 | 字节长度（UTF-16LE → 长度=字符数×2） |



### 结构体定义

请注意:

所有 Offset (偏移量) 字段，其基准地址都是 `API_SET_NAMESPACE`结构体的起始地址（即 `PEB.ApiSetMap` 指针的值）

在以下结构体中,各字段的值均为RVA(用于组成内存中的地址),不代表真实的数据结构

peb结构体中`pub ApiSetMap: *mut API_SET_NAMESPACE,`即为apisetmap的定义开始

#### API_SET_NAMESPACE(根结构体,Schema 头部)

不是数组。它是一个单例 (Singleton)整个进程内存中只有一个这东西。它是数据库的“封面”和“目录”，告诉你数据在哪里开始，有多少条记录

整个路由数据库/映射表的元数据,其中EntryOffset字段是一个RVA,该RVA加上基址,就是一个真实的内存地址指针,该指针指向的是一个数组,数组中的元素为API_SET_NAMESPACE_ENTRY

解析入口。你需要从中获取 Count 和 EntryOffset 来遍历所有 API Set，或者获取 HashOffset 和 HashFactor 来进行二分查找

位于 PEB.ApiSetMap 指向的内存起始位置

```rust

#[repr(C)]
pub struct API_SET_NAMESPACE {
    pub Version: u32,// 协议版本号(Windows 10/11 使用的是 6),如果不是6其结构体和本文件中的定义是不同的
    pub Size: u32,  // 总大小(整个 ApiSetSchema 数据块占用的字节数。很少用到)
    pub Flags: u32, // 标志位(通常为 0)
    pub Count: u32, // 虚拟 DLL 的数量(表示 API_SET_NAMESPACE_ENTRY数组中元素的个数)你需要用它来控制遍历循环的边界.
    pub EntryOffset: u32,// 命名空间条目数组的偏移(指向 API_SET_NAMESPACE_ENTRY数组相对头部的起始偏移,也就是RVA)
    pub HashOffset: u32,  // 哈希条目数组的偏移(指向 API_SET_HASH_ENTRY数组的起始偏移)
    pub HashFactor: u32   // 哈希乘数(计算 API Set名称哈希时使用的乘数)计算目标名称哈希时需要乘以这个值
}

```

```rust
#[repr(C)]
pub struct API_SET_NAMESPACE {
    /// 【标量字段】类型: u32  
    /// 含义: ApiSetSchema 版本号（关键解析依据）  
    /// 指向数据: 无（纯数值）  
    /// 常见值:  

    ///   - 6 = Windows 8.1 / Windows 10 (1507–1809),及以后的版本   
    /// ⚠️ 解析前必须校验！不同版本内存布局差异极大
    pub Version: u32,

    /// 【标量字段】类型: u32  
    /// 含义: 整个 ApiSetMap 内存区域的总字节数（含头+条目+字符串+哈希表）  
    /// 指向数据: 无（纯数值）  
    /// 用途: 边界检查（验证所有偏移量 < Size）
    pub Size: u32,

    /// 【标量字段】类型: u32  
    /// 含义: 标志位集合  
    /// 指向数据: 无（纯数值）  
    /// 位定义:  
    ///   - Bit 0: 1 = 启用哈希表加速（HashOffset 有效）  
    ///   - 其他位: 保留（Win10 通常为 0x00000001）
    pub Flags: u32,

    /// 【标量字段】类型: u32  
    /// 含义: 虚拟 DLL 条目总数（即 API_SET_NAMESPACE_ENTRY 数组长度）  
    /// 指向数据: 无（纯数值）  
    /// 典型值: Win10 约 200–300（含 api-ms-* / ext-ms-*）
    pub Count: u32,

    /// 【偏移量字段】类型: u32  
    /// 含义: 从本结构体起始地址到 API_SET_NAMESPACE_ENTRY 数组的**字节偏移**  
    /// 指向数据类型: `[API_SET_NAMESPACE_ENTRY; Count]`（连续内存数组）  
    /// 计算示例:  
    ///   `let entries = unsafe { (base_ptr as usize + header.EntryOffset) as *const API_SET_NAMESPACE_ENTRY };`  
    /// ⚠️ 所有偏移均**相对于 ApiSetMap 基址**（非条目自身地址）
    pub EntryOffset: u32,

    /// 【偏移量字段】类型: u32  
    /// 含义: 从本结构体起始地址到哈希桶数组的**字节偏移**（Flags & 1 有效时使用）  
    /// 指向数据类型: `[u32]`（动态长度数组）  
    /// 数据语义:  
    ///   - 每个 u32 元素 = API_SET_NAMESPACE_ENTRY 数组的**索引**（0 到 Count-1）  
    ///   - 无效桶值 = 0xFFFFFFFF  
    ///   - 桶数量 = 系统预分配（通常为质数，> Count）  
    /// 哈希查找流程:  
    ///   1. 计算虚拟 DLL 名哈希（去掉 ".dll" 后缀）  
    ///   2. `bucket_idx = ((hash * HashFactor) >> 32) % bucket_count`  
    ///   3. 从哈希表取索引 → 定位 Entry
    pub HashOffset: u32,

    /// 【标量字段】类型: u32  
    /// 含义: 哈希计算乘法因子（使哈希值均匀分布到桶）  
    /// 指向数据: 无（纯数值）  
    /// 典型值: 0x9E3779B9（黄金比例变体）  
    /// 用途: 与字符串哈希值相乘后取高32位，再模桶数得桶索引
    pub HashFactor: u32,
}
```

API_SET_NAMESPACE 是 Windows ApiSetSchema（API 集命名空间）的根描述符，位于进程 PEB 的 ApiSetMap 字段（x64 偏移 0x68）。

核心作用：将虚拟 DLL（如 api-ms-win-core-heap-l1-1-0.dll）动态映射到真实宿主 DLL（如 kernelbase.dll），实现系统 API 的模块化解耦与安全更新。

**设计价值：**
1. 允许微软拆分 kernel32.dll 等巨型系统 DLL 为逻辑虚拟 DLL
2. 更新宿主 DLL 时无需修改调用方二进制（虚拟 DLL 名称保持稳定）
3. 支持沙箱/容器环境重定向 API（如 ext-ms-* 用于 OneCore）

**字段深度解析（基于 Win10+ Version 6）**

| 字段 | 详细说明 | 实战注意事项 |
|------|----------|--------------|
| `Version` | Schema 版本号：<br>• `6` = Win8.1 / Win10 及以后 | 必须先校验！  |
| `Size` | 整个 ApiSetMap 内存块大小（单位：字节），包含：<br>• 头部 (28B)<br>• Entry 数组 (Count × 24B)<br>• ValueEntry 数组<br>• 所有字符串数据区<br>• 哈希表 | 用于边界检查：`if offset >= header.Size { panic!("越界!") }` |
| `Flags` | 位标志：<br>• `bit 0` = `1`：启用哈希表（`HashOffset` 有效）<br>• 其他位保留 | 多数 Win10 系统此值为 `1`（启用哈希加速） |
| `Count` | 虚拟 DLL 条目总数（即 `API_SET_NAMESPACE_ENTRY` 数组长度） | 典型值：Win10 约 200~300 个（含 `api-ms-*`, `ext-ms-*`） |
| `EntryOffset` | 相对偏移：从 `API_SET_NAMESPACE` 起始地址 → `API_SET_NAMESPACE_ENTRY` 数组首地址 | 关键！ 所有偏移均相对于 `ApiSetMap` 基址（非条目自身）。计算：`let entries = (base as usize + header.EntryOffset) as *const API_SET_NAMESPACE_ENTRY;` |
| `HashOffset` | 相对偏移：到哈希桶数组（`u32` 数组，每个元素是 Entry 索引） | 哈希查找流程：<br>1. 计算虚拟 DLL 名哈希（去掉 `.dll` 后缀）<br>2. `bucket = ((hash * HashFactor) >> 32) % bucket_count`<br>3. 从哈希表取索引 → 定位 Entry |
| `HashFactor` | 预计算的乘法哈希因子（使哈希值均匀分布） | 通常为质数（如 `0x9E3779B9` 变体），无需修改，直接用于计算 |

重要字段:  
Count字段,是一个u32数字,代表虚拟dll的数量,每个虚拟dll都有一个API_SET_NAMESPACE_ENTRY结构  
EntryOffset字段,是一个RVA,和基址共同组成指向第一个API_SET_NAMESPACE_ENTRY数组的指针,即Base + EntryOffset = API_SET_NAMESPACE_ENTRY 数组的起始地址.  
指向数据的类型：`[API_SET_NAMESPACE_ENTRY; Count]` (结构体数组)

**内存布局全景（以 Version 6 为例）**

``` text
[API_SET_NAMESPACE] (28 bytes)
│
├─ EntryOffset → [API_SET_NAMESPACE_ENTRY × Count] 
│                 │
│                 ├─ NameOffset → "api-ms-win-core-heap-l1-1-0" (无\0, Unicode)
│                 └─ ValueOffset → [API_SET_VALUE_ENTRY × ValueCount]
│                                    │
│                                    └─ ValueOffset → "kernelbase.dll" (无\0, Unicode)
│
├─ HashOffset → [u32 哈希桶数组] (用于 O(1) 查找)
│
└─ (字符串数据区连续存储所有 Name/Value)
```


**用途**
1. 解析 IAT 中的 api-ms-win-*.dll → 真实 DLL（绕过 IAT Hook）
2. Reflective Loader 中手动解析导入表
3. 检测 EDR 注入的虚拟 DLL（如异常 ext-ms-* 条目）

#### API_SET_NAMESPACE_ENTRY (虚拟模块条目)

是数组,数组长度由头部(API_SET_NAMESPACE)的 Count 字段决定,数组的每一个元素代表一个虚拟 DLL（例如 api-ms-win-core-file-l1-1-0.dll）

描述一个虚拟 DLL（如 api-ms-win-core-file-l1-1-0.dll）

通过 NameOffset 和 NameLength 读取虚拟文件名，与你想要解析的contract_name 进行比较;找到匹配的 Entry 后，通过 ValueOffset 访问具体的重定向规则

内存布局:  API_SET_NAMESPACE.EntryOffset 指向的是一个数组,该数组中的元素的类型是API_SET_NAMESPACE_ENTRY

```rust
#[repr(C)]
#[derive(Clone, Copy)]
pub struct API_SET_NAMESPACE_ENTRY {
 pub Flags: u32,   //属性标志(0 表示正常，1 表示 Sealed（密封）。通常解析时忽略)
 pub NameOffset: u32,  // 虚拟 DLL名称字符串偏移(指向该虚拟 DLL 名称的 Unicode 字符串,（如 "api-ms-win-core-heap-l1-1-0"）),注意: 这里的字符串没有 Null 结尾 (\0)，必须结合 NameLength 读取
 pub NameLength: u32,//  虚拟 DLL名称字节长度(该名称的字节数,不是字符数),因为是 Unicode (UTF-16)，所以字符数 = NameLength / 2
 pub HashedLength: u32,  // 用于哈希计算的长度(API Set 名称通常包含后缀（如 -1-0），但哈希计算可能只取前缀。此字段指明算哈希时取多少字节)
 pub ValueOffset: u32,// 指向 API_SET_VALUE_ENTRY 数组的偏移(指向 API_SET_VALUE_ENTRY 数组的起始位置。这个数组包含该虚拟 DLL 对应的真实宿主 DLL 信息)
 pub ValueCount: u32,// 宿主映射规则的数量(该虚拟 DLL 有多少个可能的宿主（通常是 1 个，但可能有多个用于不同导入者）)
}
```

API_SET_NAMESPACE_ENTRY(是数组的形式):  
NameOffset字段,RVA,代表虚拟 DLL 名称偏移.  
指向数据的类型：`[u16; NameLength/2]` (UTF-16 字符数组)

ValueCount字段,表示这个虚拟 DLL 有多少条重定向规则（通常是1）  

ValueOffset字段,是一个RVA,和基址共同组成一个指针,指针指向的是 `API_SET_VALUE_ENTRY` 数组的起始地址.  
指向数据的类型：`[API_SET_VALUE_ENTRY; ValueCount]` (结构体数组)


在该项目中module.rs文件中,
```rust
// Base pointer for the namespace entry array
let ns_entry = ((*map).EntryOffset as usize + map as usize) as *const
  API_SET_NAMESPACE_ENTRY;

let ns_entries = from_raw_parts(ns_entry, (*map).Count as usize);
```

这里将所有API_SET_NAMESPACE_ENTRY结构体组成了一个slice,那后续中  for entry in ns_entries是如何区分出每个entry呢?

✦ 这是一个非常好的问题，触及了 Rust（以及 C/C++）底层内存操作的核心机制。

简单直接的回答是：`ns_entries` 这个 slice携带了类型信息（`API_SET_NAMESPACE_ENTRY`），编译器知道这个结构体的大小（Size），因此可以通过简单的“指针运算”计算出下一个元素的内存地址。

具体原理如下：

1. 连续的内存布局 (Contiguous Memory)   
  slice::from_raw_parts 函数的作用是告诉 Rust 编译器：“从 ns_entry
  这个内存地址开始，往后的一段连续内存中，存放了 Count 个
  API_SET_NAMESPACE_ENTRY 类型的结构体。”

内存中的实际样子大概是这样的：

```text
   1 内存地址 (假设结构体大小为 24 字节)
   2 -------------------------------------------------------
   3 0x1000  |  Entry 0 的内容 (24 bytes)  <-- ns_entry 指针指向这里
   4 0x1018  |  Entry 1 的内容 (24 bytes)
   5 0x1030  |  Entry 2 的内容 (24 bytes)
   6 ...
```

2. 类型的大小 (Size of Type)

Rust 在编译时就已经确切知道了 API_SET_NAMESPACE_ENTRY
  这个结构体占用了多少字节（假设使用了 #[repr(C)]，其大小是固定的）。

  当你写 for entry in ns_entries 时，实际上发生的是：

   1. Rust 生成了一个迭代器（Iterator）。
   2. 第一次循环，它读取切片起始位置（索引 0）。
   3. 第二次循环，它将指针向后移动 1 个单位。

  在底层指针运算中，pointer + 1 并不代表地址加 1，而是代表 地址 +
  `size_of::<API_SET_NAMESPACE_ENTRY>()`。

3. 代码层面的等价逻辑

```rust
  for entry in ns_entries 的底层逻辑在概念上等同于下面的伪代码：

    1 let size = std::mem::size_of::<API_SET_NAMESPACE_ENTRY>(); // 获取步长
    2 let start_addr = ns_entry as usize;
    3
    4 for i in 0..(*map).Count {
    5     // 区分每个 entry 的关键：利用索引 i 和 结构体大小 size 计算偏移量
    6     let current_entry_addr = start_addr + (i as usize * size);
    7
    8     // 将计算出的地址转换回引用
    9     let entry = &*(current_entry_addr as *const API_SET_NAMESPACE_ENTRY);
   10
   11     // ... 你的循环体逻辑
   12 }
```

  总结  
  之所以能区分出每个 entry，是因为：
  1. 数据是连续的（由 Windows API 保证）。
  2. 结构体大小是固定的（由 Rust 类型系统定义）。
  3. Slice 知道步长：Slice不仅仅是一个指针，它包含了类型定义。迭代器利用这个类型大小（步长）来准确地跳到下一个结构体的起始位置。

#### API_SET_VALUE_ENTRY (重定向目标/宿主条目)

是数组,但是：它不是一个巨大的全局数组。每一个 API_SET_NAMESPACE_ENTRY 都有自己独立的一小段API_SET_VALUE_ENTRY

数组长度由对应的 NAMESPACE_ENTRY 中的 ValueCount 决定。通常长度是1，但为了支持特殊情况，设计成了数组

表示一个 API Set 可能映射到的目标 DLL。一个 API Set 可以有多个 ValueEntry，用于处理不同宿主程序（Host）加载同一个 API Set 时重定向到不同 DLL的情况（这种情况较少见，但存在）

这是解析的最终目标

逻辑: 遍历该数组。如果 NameLength 不为0，检查它是否匹配当前的宿主进程/模块名（host_name）。如果匹配，则使用ValueOffset 指向的 DLL 为结果。如果 NameLength 为 0，则作为默认的回退结果（Default Fallback）

描述虚拟 DLL 最终映射到的物理 DLL（如 kernelbase.dll）。它是查找过程中的“值 (Value)”。

内存布局: 这是一个数组，由API_SET_NAMESPACE_ENTRY.ValueOffset 指向

```rust
#[repr(C)]
#[derive(Clone, Copy)]
pub struct API_SET_VALUE_ENTRY {
 pub Flags: u32,   // 标志位,通常忽略
 pub NameOffset: u32,  // 导入者名称偏移 (约束条件)(指向一个进程或模块名称。如果不为空，表示“只有当这个模块加载该 API Set 时，本条规则才生效”)
 pub NameLength: u32,  // 导入者名称字节长度,如果 NameLength != 0: 这是一个特定规则。你需要检查它指向的名字是否等于你当前的 host_name
 pub ValueOffset: u32, // 目标 DLL 名称偏移,指向最终的物理 DLL 路径/名称（Unicode，无 Null）.使用: 这就是你函数最终要返回的字符串的来源
 pub ValueLength: u32 // 目标 DLL 名称字节长度,用于读取最终的 DLL 名称
}
```

```rust
pub struct API_SET_VALUE_ENTRY {
    pub Flags: u32,        // 位0: 1=使用NameOffset（别名），0=仅用ValueOffset
    pub NameOffset: u32,   // → 宿主 DLL **别名**字符串偏移（如 "api-ms-win-core-heap-l2-1-0"）
    pub NameLength: u32,   // 别名字节长度
    pub ValueOffset: u32,  // → **宿主 DLL 真实名称**字符串偏移（如 "kernelbase.dll"）❗
    pub ValueLength: u32,  // **宿主 DLL 名称字符串的字节长度**（非数量！）❗
}
```

API_SET_VALUE_ENTRY(是数组的形式):  
NameOffset字段,是一个RVA,和基址共同组成一个指针,该指针指向的是一个utf-16类型的字符串  
指向数据的类型：`[u16; NameLength/2]` (UTF-16 字符数组)

NameLength字段,代表名称NameOffset指向的字符串的长度(字节数)

ValueOffset字段,是一个RVA,和基址共同组成一个指针,指针指向的是目标 DLL 名称字符串
指向数据的类型：`[u16; ValueLength/2]` (UTF-16 字符数组)

ValueLength字段,目标 DLL 名称的字节长度

#### API_SET_HASH_ENTRY (哈希索引条目)

是数组,它是为了加速查找 NAMESPACE_ENTRY 而存在的排序数组

为了加快查找速度，Windows 预计算了 API Set名称的哈希并排序，允许通过二分查找定位

性能优化你可以选择暴力遍历 API_SET_NAMESPACE_ENTRY 数组来查找名字（实现简单）;或者使用 HashFactor 对目标名称进行哈希，然后在 API_SET_HASH_ENTRY数组中进行二分查找（性能更高，实现稍繁琐）

辅助结构，用于通过二分查找快速定位 API_SET_NAMESPACE_ENTRY，避免遍历整个数组。内存布局: 这是一个数组，由 API_SET_NAMESPACE.HashOffset 指向。该数组按 Hash 值排序

```rust
#[repr(C)]
#[derive(Clone, Copy)]
pub struct API_SET_HASH_ENTRY {
 Hash: u32,  //预计算的哈希值。这是虚拟 DLL 名称经过特定算法（利用 HashFactor）计算出的哈希
 Index: u32, // 索引值。<br>指向 API_SET_NAMESPACE_ENTRY 数组的下标。<br>使用: 当你在二分查找中找到匹配的 Hash 后，用这个 Index 去访问 Namespace Entry数组，获取真正的索引
}
```

### puerto中resolve_api_set_map解析apisetmap的逻辑

1. PEB -> 拿到 ApiSetMap 指针 (即 API_SET_NAMESPACE 基址 Base)
2. Base + EntryOffset -> 拿到 NamespaceEntry 数组
3. (遍历或哈希查找) NamespaceEntry:比较 Base + NamespaceEntry[i].NameOffset 指向的字符串是否等于 contract_name (如 api-ms-win-core-sysinfo-l1-1-0)
4. 找到匹配的 NamespaceEntry 后:Base + NamespaceEntry[i].ValueOffset -> 拿到 ValueEntry 数组
5. (遍历) ValueEntry:检查约束: 查看 ValueEntry[j].NameLength;如果有值，检查 Base + ValueEntry[j].NameOffset 是否等于 host_name;获取结果: 如果匹配（或者是默认规则），读取 Base + ValueEntry[j].ValueOffset 指向的字符串;这就是最终的物理 DLL 名称 (如 kernelbase.dll)

### 免杀视角 —— 这些结构体在对抗中的核心作用

1. 规避 API Hook（核心价值）

EDR（端点检测与响应系统）通常会 Hook 两个层面的 API：

- L3 (Kernel32): LoadLibrary, GetProcAddress
- L2 (Ntdll): LdrLoadDll, LdrGetProcedureAddress

如果你想获取某个函数地址，传统的做法是调用LoadLibrary("api-ms-win-core-sysinfo-l1-1-0.dll")。

- EDR 看到：你在尝试加载一个虚拟 DLL，它会拦截并检查

`resolve_api_set_map` 的价值：通过直接读取 PEB 内存（src/types.rs中定义的结构体），自己在用户态实现了路由解析逻辑

1. 你拿到了虚拟 DLL 名
2. 你遍历内存中的结构体，算出了它对应的物理 DLL 是 kernelbase.dll
3. 你直接去加载 kernelbase.dll（或者如果在内存里有了，直接用）
4. 你完成了解析，但没有调用任何 Windows API。EDR 的 Hook根本捕捉不到这一过程。你实现了“无声”的模块定位

5. 隐蔽的 IAT 解析

很多安全产品扫描内存中的 IAT  

- 如果你的 Payload 导入表里明晃晃写着 kernel32.dll，容易被分析。
- 利用 ApiSetMap，你可以让你的 Payload 看起来只依赖一些晦涩的 ext-ms-win-...虚拟 DLL。
- 静态分析工具可能无法轻易知道这些虚拟 DLL 到底指向什么功能。
- 而你的 puerto 加载器在运行时通过解析 PEB 动态还原它们，实现了静态混淆，动态还原。

1. 各结构体与免杀之间联系

1. `API_SET_NAMESPACE` (The Database Header)
       *PEB中的含义：整个路由数据库的元数据。
       * 你的代码用途：获取 EntryOffset，这是进入迷宫的入口。
       * 免杀意义：这是内存中一块只读数据，EDR很少监控对它的读取操作，是安全的“信息源”。

1. `API_SET_NAMESPACE_ENTRY` (The Key / 虚拟DLL)
       *PEB中的含义：数据库的“索引键”，代表所有可能存在的虚拟文件名。
       * 你的代码用途：在这里循环，匹配你的目标（如contract_name）。
       * 免杀意义：通过遍历这里，你可以确认当前系统支持哪些 API集，用来做环境指纹识别（比如判断是 Win10 还是 Win11），从而动态下发不同的Payload，反沙箱技术的一种。

   1. `API_SET_VALUE_ENTRY` (The Value / 物理DLL)
       - PEB中的含义：数据库的“值”，代表真实的磁盘文件路径。
       - 你的代码用途：获取最终的物理路径，传给你的 retrieve_module_add或其他加载函数。
       - 免杀意义：这里藏着真理。攻击者甚至可以（理论上，虽然很难因为是只读内存）修改这里，让系统把所有对 kernel32 的调用重定向到你恶意的DLL，实现全局劫持（ApiSet Hijacking）。

   2. `API_SET_HASH_ENTRY` (The Speed Hack)
       - PEB中的含义：为了让系统启动变快做的哈希索引。
       - 你的代码用途：如果你想写得极快，用这个二分查找。如果你不在乎几微秒的性能，可以直接暴力遍历 Namespace，代码更少，特征更小。

这四个结构体都是数组吗?
这四个结构体之间的联系?请聚个简单的例子说明为啥需要这么多结构体表示映射关系?之所以需要这么多结构体的逻辑是什么?
现在ai那么厉害,还有如openclaw的这种流行项目,我学习这些东西真的有用吗?会过时吗?会被ai替代吗?

### 如果虚拟dll在ApiSetMap中找不到呢

| 场景 | 系统行为 | 原因 |
|------|----------|------|
| 虚拟 DLL 在 ApiSetMap 中无映射条目 | ❌ 加载失败（`STATUS_DLL_NOT_FOUND`） | Windows 加载器严格依赖 ApiSetMap 解析虚拟 DLL |
| 宿主 DLL 文件本身缺失/损坏 | ❌ 加载失败（`STATUS_DLL_INIT_FAILED`） | ApiSetMap 仅做名称映射，不验证文件存在性 |
| 程序直接使用真实 DLL 名（如 `kernel32.dll`） | ✅ 正常加载 | 不经过 ApiSetMap，走常规加载路径（KnownDLLs/系统目录） |


flowchart TD
    A[LoadLibraryW<br/>“api-ms-win-core-heap-l1-1-0.dll”] --> B{是否为虚拟 DLL？<br/>（前缀 api-ms-/ext-ms-）}  
    B -->|是| C[查询 ApiSetMap]  
    B -->|否| D[直接按原名加载<br/>（KnownDLLs/系统目录）]  
    C --> E{ApiSetMap 中存在映射？}  
    E -->|是| F[替换为宿主 DLL 名<br/>“kernelbase.dll”]  
    E -->|否| G[返回 STATUS_DLL_NOT_FOUND<br/>❌ 进程崩溃/报错]  
    F --> H[按“kernelbase.dll”加载]  
    H --> I{kernelbase.dll 文件存在？}  
    I -->|是| J[✅ 加载成功]  
    I -->|否| K[返回 STATUS_DLL_NOT_FOUND<br/>❌ 文件系统层失败]  

**场景 1：ApiSetMap 被篡改/损坏（EDR 拦截、内存破坏）**  
现象：api-ms-win-core-heap-l1-1-0 映射条目被删除或指向恶意 DLL
后果：  
条目缺失 → 进程启动即崩溃（常见于恶意软件破坏系统）  
条目被篡改 → 加载到 EDR 注入的 Hook DLL  

**场景 2：跨 Windows 版本兼容性问题**
现象：Win7 程序在 Win11 运行，ApiSetMap 中无旧版虚拟 DLL 条目
真相：  
Windows 向后兼容设计：新版 ApiSetMap 保留旧版虚拟 DLL 映射（如 Win11 仍含 Win8 的 api-ms-win-core-heap-l1-1-0）
→ 极少因版本缺失导致失败（微软维护映射表完整性）  
例外：  
某些 ext-ms-*（扩展 API 集）在特定 SKU（如 Server Core）中可能缺失 → 需检查 ValueCount == 0

**场景 3：程序直接调用真实 DLL（最常见！）**  
关键认知：  
ApiSetMap 仅作用于“虚拟 DLL 名”（api-ms-* / ext-ms-*）
→ kernel32.dll、user32.dll、ntdll.dll 等真实 DLL 名永不经过 ApiSetMap  
验证方法：  
用 Dependency Walker 或 dumpbin /imports 检查 EXE 导入表：  
若显示 api-ms-win-core-heap-l1-1-0.dll → 依赖 ApiSetMap  
若显示 kernelbase.dll → 直接加载，与 ApiSetMap 无关  

| 问题类型 | 根本原因 | 解决方案 |
|----------|----------|----------|
| “虚拟 DLL 解析失败” | ApiSetMap 条目缺失/损坏 | 1. 检查系统完整性（sfc /scannow）<br>2. Red Team：使用硬编码回退表 |
| “宿主 DLL 加载失败” | 文件被删除/权限问题/路径劫持 | 1. 检查 `%SystemRoot%\System32\kernelbase.dll`<br>2. 检查 KnownDLLs 注册表项 |
| “为何我的程序不查 ApiSetMap？” | 导入表直接使用真实 DLL 名 | 正常行为！ApiSetMap 仅处理虚拟 DLL |

🌟 终极心法：  
ApiSetMap 是“名称翻译器”，不是“DLL 存储库”。  
它只回答：“api-ms-win-core-heap-l1-1-0 应该去加载哪个真实 DLL？”  
它不保证：“kernelbase.dll 文件一定存在且可加载”  
理解这一边界，是避免在内存加载、反射式 DLL 注入等场景中踩坑的核心。  

## LDR

Ldr 是一个指向 PEB_LDR_DATA 结构体的指针 (*mut PEB_LDR_DATA)。

它指向了关于进程已加载模块（如 DLLs）的详细信息。操作系统加载器（Loader）使用这个结构来维护所有加载到该进程地址空间的模块链表。

- Rust 类型: 在 dinvk 中，它被定义为裸指针，意味着访问它需要使用unsafe 代码块。



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

是 Windows 操作系统内部用于描述每一个已加载模块（DLL 或 EXE）的元数据结构  
通过PEB中的一个`pub Ldr: *mut PEB_LDR_DATA,`字段指向PEB_LDR_DATA这个结构

### InMemoryOrderModuleList

`InMemoryOrderModuleList` ,是一个双向循环链表的表头（Sentinel Node/Head）,并非如其名称暗示的那样"按内存地址高低排序"，这是一个在安全研究和逆向工程社区中广泛流传的误解。  
实际上，这个链表主要反映**模块在内存中的布局顺序和初始化关系**，而非简单的地址高低排序。

(*ldr_data).InMemoryOrderModuleList 本身不包含任何模块信息（它只是PEB_LDR_DATA 结构体中的一个字段）
如果要指向“表头”，也是用 (*ldr_data).InMemoryOrderModuleList

如果你想表达整个链表的“锚点”或“哨兵节点”，那么就是(*ldr_data).InMemoryOrderModuleList。在汇编或底层 C开发中，我们通常用它的地址来判断是否已经遍历完一圈

(*ldr_data).InMemoryOrderModuleList.Flink 指向链表的第一个真正节点

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
- Node 3：kernel32.dll（通常情况）,也有可能是kernelbase.dll。
- 代码逻辑：从 Head 开始，执行两次 Flink 跳转，理应到达ntdll.dll。为了保险，代码还计算了模块名的 Hash 进行校验(重要)。

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

# PE(Portable Executable)

PE 文件本质上是 **“线性存储的数据结构集合”**。在磁盘上和在内存中，它们的逻辑顺序一致，但物理间距（对齐）不同。  
PE 是 Windows 可执行文件（.exe, .dll,.sys）的标准格式。它描述了代码、数据、资源在文件中如何组织，以及加载到内存时应该如何映射。

## PE Memory Layout

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

## PE核心数据结构

结构体均定义在 Windows SDK 的 `winnt.h` 中

为了避免依赖庞大的 windows-sys 或 winapi crate，选择手动定义了这些底层结构。这种做法在恶意软件开发或红队工具（如你正在开发的RustRedOps）中非常常见，目的是为了减少特征指纹?为什么、减小二进制体积?为什么以及拥有更精细的控制权。

PE（Portable Executable）是 Windows 系统下可执行文件（EXE）、动态链接库（DLL）、驱动（SYS）等的标准格式。本文以开源项目 **`dinvk`** 中 `src/types.rs` 定义的 Rust 结构体为蓝本，**从磁盘文件偏移 0 开始**，逐字段、逐结构地详细解释 PE 文件中每一个字段的含义与作用。基于 `dinvk` 项目中的数据结构详解：

---

### 第一部分：DOS Header（兼容性头部）

位于文件起始位置（**Offset 0**），共 64 字节（0x40）。其存在是为了向后兼容 MS-DOS 系统。

```rust
#[repr(C, packed(2))]
pub struct IMAGE_DOS_HEADER {
    pub e_magic: u16,    // [0x00] 魔数 "MZ" (0x5A4D),2字节
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

    pub e_lfanew: i32,   // [0x3C] ★关键字段★,4字节
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

**DataDirectory 的关键索引**：

- **[0] Export Table** (`dinvk` 解析的目标)
- **[1] Import Table** (正常 IAT)
- **[3] Exception Table** (`uwd` 需要解析的 `.pdata`)
- **[5] Base Relocation Table** (手动映射必须处理的)

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

物理上（内存里）存储的 3 个数组：
AddressOfNames (u32数组)：只存名字
元素[0]: 指向 "CreateFile"
元素[1]: 指向 "ExitProcess"
元素[2]: 指向 "VirtualAlloc"
(注：这个数组必须按字母排序，为了方便二分查找)
AddressOfNameOrdinals (u16数组)：只存映射关系
元素[0]: 5 (对应 CreateFile)
元素[1]: 2 (对应 ExitProcess)
元素[2]: 0 (对应 VirtualAlloc)
(注：它的顺序严格跟随上面的 Name 数组，是一一对应的)
AddressOfFunctions (u32数组)：只存代码地址 (不管顺序，这是仓库)
元素[0]: 0x3000 (这里放着 VirtualAlloc 的代码)
... (其他无名函数)
元素[2]: 0x2000 (这里放着 ExitProcess 的代码)
...
元素[5]: 0x1000 (这里放着 CreateFile 的代码)

#### `IMAGE_EXPORT_DIRECTORY->AddressOfNames` 的内存归属解析

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

## RVA FOA

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

### 为什么如果一个导出函数的 RVA 指向了导出目录 (Export Directory) 所在的内存范围内那么它一定不是代码，而是一个转发字符串 (Forwarder String)

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

# PE和PEB的三大核心联系

PE (Portable Executable) 和 PEB (Process Environment Block) 是 Windows操作系统中两个核心概念，它们分别代表了程序的静态存储形态和动态运行形态。  
PEB 记录了 PE 文件被加载到内存后的关键信息，是访问和解析内存中PE 结构的入口。

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

# 指令集架构

除了 x86_64（AMD64）之外，还存在多种主流和专用的指令集架构（ISA）。作为一名 Rust 开发者，了解以下几种架构尤为重要，因为它们是 Rust 交叉编译的常见目标。

---

## 1. ARM 架构（AArch64 / ARM64）

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

## 2. RISC-V

- **特点**：完全开源、免费、模块化的 RISC 架构。允许任何人设计、制造和销售 RISC-V 芯片而无需支付专利费，被誉为“芯片界的 Linux”。
- **应用场景**：
  - 当前主要用于物联网（IoT）、嵌入式控制器。
  - 正在向服务器和高性能计算（HPC）领域快速扩展。
- **Rust 支持**：
  - 官方支持良好，例如：`riscv64gc-unknown-linux-gnu`

---

## 3. WebAssembly（Wasm）

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

## 4. x86（32-bit）/ i686

- **特点**：x86_64 的前身，32 位架构，内存寻址上限为 4GB。
- **应用场景**：
  - 老旧 PC、工业控制系统、遗留嵌入式设备。
  - 虽然现代开发已逐渐淘汰，但在兼容性维护中仍需关注。
- **Rust 目标示例**：`i686-unknown-linux-gnu`

---

## 5. 其他嵌入式与专用架构

| 架构       | 简介                                                                 |
|------------|----------------------------------------------------------------------|
| **MIPS**   | 曾广泛用于路由器和机顶盒，现逐渐被 ARM 和 RISC-V 取代。               |
| **PowerPC (PPC)** | 曾用于老款 Mac 和游戏主机（如 PS3、Xbox 360），现主要用于汽车电子、航天等高可靠领域。 |
| **AVR**    | 8 位微控制器架构，典型代表是 Arduino Uno，适用于极低功耗嵌入式场景。     |

---

## 总结：CISC vs RISC

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

# EXE 文件和 PE 文件的关系？除了 EXE 还有哪些 PE 文件？

- **PE**（Portable Executable）  
  是微软定义的**二进制文件格式标准**，规定了可执行文件在磁盘上的结构以及加载到内存后的布局方式。
- **EXE**（`.exe`）  
  是符合 PE 格式的一种**具体文件类型**，专用于标识“可直接启动并作为独立进程运行”的程序。

> 🏗️ **类比**：  
>
> - PE 标准 ≈ **建筑规范**（规定地基、承重墙、门窗位置）  
> - EXE 文件 ≈ **按规范建成的住宅**（可直接入住）

---

## 2. 除了 EXE，还有哪些常见的 PE 文件？

只要是符合 PE 结构的文件，无论扩展名如何，都属于 PE 家族。常见类型包括：

### A. 动态链接库（DLL）

- **`.dll`**：最常见形式，供 EXE 或其他 DLL 调用，不能独立启动。
- **`.ocx`**：ActiveX 控件（旧版 IE 使用），本质是带 COM 接口的 DLL。
- **`.cpl`**：控制面板项（如“鼠标设置”），本质是导出 `CPlApplet` 函数的 DLL。

### B. 驱动程序（Drivers）

- **`.sys`**：Windows 内核模式驱动，由内核加载器加载，运行在 Ring 0。

### C. 其他系统/功能文件

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

### 总结：为何需要如此多样的 PE 格式？

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
