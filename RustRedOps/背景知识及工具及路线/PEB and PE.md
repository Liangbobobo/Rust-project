- [基础知识](#基础知识)
  - [ASCII UTF-8 UTF-16 ANSI / Code Pages (GBK, Latin1)](#ascii-utf-8-utf-16-ansi--code-pages-gbk-latin1)
- [前言](#前言)
  - [为啥要定义这么多数据结构(types.rs中)](#为啥要定义这么多数据结构typesrs中)
- [TEB](#teb)
- [PEB相关(使用dinvk中的例子及windbg)](#peb相关使用dinvk中的例子及windbg)
    - [一个可执行文件产生多个进程时PEB是怎么样的?](#一个可执行文件产生多个进程时peb是怎么样的)
  - [ApiSetMap](#apisetmap)
    - [peb.apisetmap作用](#pebapisetmap作用)
    - [结构体定义](#结构体定义)
      - [API\_SET\_NAMESPACE(根结构体,Schema 头部)](#api_set_namespace根结构体schema-头部)
      - [API\_SET\_NAMESPACE\_ENTRY (虚拟模块条目)](#api_set_namespace_entry-虚拟模块条目)
      - [API\_SET\_VALUE\_ENTRY (重定向目标/宿主条目)](#api_set_value_entry-重定向目标宿主条目)
      - [API\_SET\_HASH\_ENTRY (哈希索引条目)](#api_set_hash_entry-哈希索引条目)
    - [puerto中resolve\_api\_set\_map解析apisetmap的逻辑](#puerto中resolve_api_set_map解析apisetmap的逻辑)
    - [免杀视角 —— 这些结构体在对抗中的核心作用](#免杀视角--这些结构体在对抗中的核心作用)
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

# 基础知识

## ASCII UTF-8 UTF-16 ANSI / Code Pages (GBK, Latin1)

 这不仅是编程基础，这是内存取证（Memory Forensics）和反编译（Reverse Engineering）的基石。

  ---

  第一章：字符编码的本质

  在 CPU 眼里，没有“文字”，只有数字。编码就是一本“字典”，规定了哪个数字代表哪个图形。

  1. ASCII (American Standard Code for Information Interchange)
   * 状态：现代计算机的始祖，所有编码的子集。
   * 定义：使用 7个比特 (bit) 表示 128 个字符（0x00 - 0x7F）。
   * 内存形态 (Hex)：
       * A -> 41
       * a -> 61
       * 1 -> 31
       * . -> 2E
   * C语言/Windows 特性：
       * Null-Terminated：在 C 语言中（char*），字符串必须以 0x00 结尾。
       * 例如 "ABC" 在内存中占用 4 字节：41 42 43 00。
   * Rust 类型：&[u8] 或 b"ABC"。

  🔴 红队视角 (Red Team Ops)
   * 导出表陷阱：PE 文件的导出表（Export Table）中，函数名（如 OpenProcess）永远是 ASCII
     编码的。
       * 致命错误：如果你用 UTF-16 的哈希算法去算导出表里的函数名，你的 Shellcode
         永远找不到地址。
   * 特征码检测：LoadLibraryA 这个字符串在内存中就是 4C 6F 61 64...。这是一个极强的静态特征。

  ---

  2. OEM Code Pages (ANSI / DBCS) —— “乱码之源”
   * 状态：在 Unicode 诞生前的权宜之计（Windows 95/98 时代遗留）。
   * 定义：根据操作系统的“区域设置”不同，同一个字节代表不同含义。
       * 0x00-0x7F：兼容 ASCII。
       * 0x80-0xFF：高位字节，不仅自己有意义，还可能和后一个字节组合。
   * 例子：
       * GBK (CP936, 中文)：0xD6 0xD0 = "中"。
       * Latin-1 (CP1252, 西欧)：0xD6 = "Ö" (带分音符的O)。
   * 内存形态：单字节或双字节混排。

  🔴 红队视角
   * 环境敏感性：如果你的木马是用 GBK
     编码写的中文提示信息，扔到一台美国（CP1252）的服务器上运行，弹出的信息就是乱码。
   * API 版本：MessageBoxA、CreateProcessA 中的 A 就是指 ANSI。这些 API 会根据当前系统的 Code
     Page 把字符串转成 Unicode 再交给内核。
   * 路径解析漏洞：某些安全软件在处理 ANSI
     路径时存在溢出漏洞，通过构造特殊的“双字节字符”路径，可能绕过检测。

  ---

  3. UTF-16 LE (Little Endian) —— Windows 的皇冠
   * 状态：Windows 内核的原生编码。NT 内核在 1993 年设计时选择了当时被认为最先进的
     UCS-2（后演化为 UTF-16）。
   * 定义：
       * 基本平面（BMP, 常用字）：使用 2 个字节 (u16)。
       * 增补平面（Emoji, 生僻字）：使用 4 个字节 (双 u16，称为代理对 Surrogate Pairs)。
   * Little Endian (小端序)：这是最关键的。低位字节在低地址。
       * 字符 A (Unicode 0x0041) -> 内存存储为 41 00。
       * 字符 中 (Unicode 0x4E2D) -> 内存存储为 2D 4E。
   * Windows 特性：
       * WCHAR / PWSTR：C 语言中的宽字符指针，要求以 2字节的 Null (`00 00`) 结尾。
       * UNICODE_STRING：内核结构体，不要求 Null 结尾，依靠 Length 字段。ApiSetMap
         里的字符串就属于这种类型！
   * Rust 类型：Vec<u16>。

  🔴 红队视角
   * API 调用：几乎所有现代 Windows API（ntdll.dll, kernel32.dll）底层只接受 UTF-16。如果你传
     ASCII，系统要在内部做一次 RtlAnsiStringToUnicodeString，不仅慢，还可能因为 Code Page
     导致转换错误。
   * 00 字节特征：
       * ASCII: c m d -> 63 6D 64 (紧凑)
       * UTF-16: c m d -> 63 00 6D 00 64 00 (稀疏)
       * 检测：安全人员只要在内存 Hex 视图里看到大量的 00 间隔，就知道这是 UTF-16
         字符串区域。
   * ApiSetMap 解析：你在解析 ApiSetMap 时，NameOffset 指向的数据就是 UTF-16
     LE。如果你直接把它当 ASCII 读，会读到 a (0x61)，然后读到
     0x00，你的字符串读取函数就会以为字符串结束了，导致只能读出一个字母。

  ---

  4. UTF-8 —— Rust 与现代网络的标准
   * 状态：互联网通用的标准，Rust 的 String 默认编码。
   * 定义：变长编码（1-4 字节）。
       * ASCII 字符：1 字节 (和 ASCII 一模一样，0x41)。
       * 中文：通常 3 字节 (中 -> E4 B8 AD)。
       * Emoji：通常 4 字节。
   * Rust 类型：String, &str。

  🔴 红队视角
   * C2 通信：你的木马回传数据给控制台（Cobalt Strike / Sliver）时，通常是 JSON 或 XML
     格式，这些全是 UTF-8。
   * 主要冲突：Rust 的世界是 UTF-8，Windows 的世界是 UTF-16。
       * 你在写代码时：let name = "kernel32.dll"; (这是 UTF-8)。
       * 你调用 API 时：LdrLoadDll 需要 buffer: *mut u16 (这是 UTF-16)。
       * 必须转换：你必须时刻进行 UTF-8 -> UTF-16 的转换（expanding）和 UTF-16 -> UTF-8
         的转换（narrowing）。

  ---

  第二章：内存指纹深度对比（Hex View）

  假设我们要存储字符串 "A中"。


  ┌────────────┬────────────────────┬────────────────────────────────────┬─────────┐
  │ 编码       │ 内存十六进制 (Hex) │ 解释                               │ 长度    │
  ├────────────┼────────────────────┼────────────────────────────────────┼─────────┤
  │ ASCII      │ 41 3F              │ 41('A'), '中'无法表示，变成3F('?') │ 2 bytes │
  │ GBK (ANSI) │ 41 D6 D0           │ 41('A'), D6 D0('中' Code Page)     │ 3 bytes │
  │ UTF-16 LE  │ 41 00 2D 4E        │ 41 00('A'), 2D 4E('中')            │ 4 bytes │
  │ UTF-16 BE  │ 00 41 4E 2D        │ 大端序，Windows 不用 这个          │ 4 bytes │
  │ UTF-8      │ 41 E4 B8 AD        │ 41('A'), E4 B8 AD('中')            │ 4 bytes │
  └────────────┴────────────────────┴────────────────────────────────────┴─────────┘


那说下pe peb结构及其字段有哪些是 ascii utf-16 及其他编码的?

✦ 这是一个极具实战价值的问题。在红队开发（特别是手动映射、各种注入、Shellcode 编写）中，混淆
  PE 文件头（静态）和 PEB（动态）里的编码格式，是导致 Payload
  崩溃或者被杀软静态查杀的根本原因之一。

  简单总结规律：
   * PE 文件头（磁盘/内存中的镜像）：90% 是 ASCII（遗留产物，为了兼容 DOS 时代）。
   * PEB（进程环境块/系统加载器）：99% 是 UTF-16 LE（NT 内核的原生语言）。

  下面是详细的字段级拆解，附带 Rust 开发中的注意事项。

  ---

  第一部分：PE 结构 (Portable Executable)
  特征：静态文件，或者手动映射到内存中的镜像。主要使用 ASCII。


  ┌───────────┬─────────┬───────┬──────────────────────────────────────────────────────┬─────
  ──┐
  │ 结构体... │ 字段名  │ 编... │ 长度特征                                             │
  Ru... │
  ├───────────┼─────────┼───────┼──────────────────────────────────────────────────────┼─────
  ──┤
  │ **IMAG... │ e_magic │ ASCII │ 固定 2 字节 (0x4D 0x5A -> "MZ")                      │
  `u... │
  │ **IMAG... │ `Sig... │ ASCII │ 固定 4 字节 (0x50 0x45 00 00 -> "PE\0\0")            │
  `u... │
  │ **IMAG... │ Name    │ **... │ 固定 8 字节 ([u8; 8])。<br>坑：如果名字刚好 8 字...  │
  `&... │
  │ **IMAG... │ Name    │ ASCII │ RVA 指向一个以 \0 结尾的字符串（DLL 原始文件名，...  │
  读... │
  │ **导出... │ `Add... │ ASCII │ RVA 数组，每个 RVA 指向一个 \0 结尾的函数名（如 `... │
  读... │
  │ **IMAG... │ Name    │ ASCII │ RVA 指向导入的 DLL 名（如 USER32.dll）。             │
  读... │
  │ **IMAG... │ Name    │ ASCII │ 具体导入的函数名。                                   │
  读... │
  │ **资源... │ `Nam... │ **... │ 例外情况！资源段中的字符串通常是 Unicode。           │
  `V... │
  └───────────┴─────────┴───────┴──────────────────────────────────────────────────────┴─────
  ──┘


  🔴 红队实战警告：
  你在写 get_proc_address（获取导出函数地址）时，PE 导出表里的函数名是 ASCII。
   * 如果你想查找 LoadLibraryW。
   * 导出表里存的是 0x4C 0x6F ... (ASCII)。
   * 千万不要把这个字节流直接强转成 u16 去算哈希，除非你的哈希算法专门处理了这种情况。

  ---

  第二部分：PEB 结构 (Process Environment Block)
  特征：操作系统在运行时生成的管理结构，位于内存中。主要使用 UTF-16 LE。


  ┌───────────────┬──────────┬──────┬───────────────────────────────────────────┬────────────
  ─┐
  │ 结构体 / 位置 │ 字段名   │ ...  │ 结构特征                                  │ Rust 处理
  │
  ├───────────────┼──────────┼──────┼───────────────────────────────────────────┼────────────
  ─┤
  │ PEB_LDR_DATA    │ 无直...  │ -    │ 这是一个链表头，本身不存字符串。          │ -
  │
  │ **LDR_DATA_T... │ `Full... │ *... │ UNICODE_STRING 结构。包含完整路径（如 ... │ &[u16]
  (... │
  │ **LDR_DATA_T... │ `Base... │ *... │ UNICODE_STRING 结构。只有文件名（如 `k... │ &[u16]
  (... │
  │ **RTL_USER_P... │ `Imag... │ *... │ 当前进程 EXE 的完整路径。                 │ &[u16]
  │
  │ **RTL_USER_P... │ `Comm... │ *... │ 启动命令行参数。                          │ &[u16]
  │
  │ **RTL_USER_P... │ `Wind... │ *... │ 进程创建时的窗口标题。                    │ &[u16]
  │
  │ **RTL_USER_P... │ `Envi... │ *... │ 环境变量块。这是一个巨大的 `Key=Value\... │ 解析复杂
  │
  │ ApiSetMap     │ `Entr... │ *... │ 无 Null 结尾，纯字符数组。                │ &[u16]
  │
  │ ApiSetMap     │ `Valu... │ *... │ 无 Null 结尾，纯字符数组。                │ &[u16]
  │
  └───────────────┴──────────┴──────┴───────────────────────────────────────────┴────────────
  ─┘

  🔴 红队实战警告：
   * UNICODE_STRING 陷阱：PEB 中的字符串大多被封装在 UNICODE_STRING 结构体中。

   1     struct UNICODE_STRING {
   2         USHORT Length;        // 字节长度（不含结尾空）
   3         USHORT MaximumLength; // 缓冲区总大小
   4         PWSTR  Buffer;        // 指向 UTF-16 数据的指针
   5     };
   * 一定要用 Length：虽然 Windows 通常会在 Buffer 后面好心放一个 00
     00，但不要依赖它。规范的做法是只读取 Length 指定的字节数。
   * 大小写敏感：LDR 中的 DLL
     名称有时是大写，有时是小写（取决于加载方式）。计算哈希时最好统一转成小写（Lower
     Case）处理。

  ---

  第三部分：一个极度混淆的特例 —— "Forwarder String"

  在导出表（PE 结构）中，有一种特殊情况叫 Forwarder（转发器）。

   * 位置：当导出函数的 RVA 指向导出表自身范围内部时。
   * 内容：这代表这个函数不是当前 DLL 实现的，而是转发给别的 DLL。
   * 格式：NTDLL.RtlAllocateHeap（模块名.函数名）。
   * 编码：ASCII。

  坑点：
  你的代码逻辑是这样的：
   1. 解析 PEB（UTF-16）找到了 kernel32.dll。
   2. 解析 kernel32.dll 的导出表（ASCII），找到了 HeapAlloc。
   3. 发现 HeapAlloc 是个转发器，指向 NTDLL.RtlAllocateHeap（ASCII 字符串）。
   4. 你需要去加载 ntdll.dll。
       * 转换时刻：你手里拿着 ASCII 的 "ntdll.dll"，但 LdrLoadDll 需要 UTF-16 的
         "ntdll.dll"。这里必须做一次 ASCII -> UTF-16 的转换。

  ---

  第四部分：Rust 代码中的速查表

  在编写 puerto 这种工具时，建议定义好辅助函数，防止搞混。

    1 // 1. 处理 PE 导出表函数名 (ASCII -> String/Hash)
    2 // 来源：ImageExportDirectory -> AddressOfNames
    3 pub fn parse_pe_ascii_name(ptr: *const u8) -> String {
    4     let mut len = 0;
    5     unsafe {
    6         while *ptr.add(len) != 0 { len += 1; } // 找 \0 结尾
    7         let slice = core::slice::from_raw_parts(ptr, len);
    8         // 如果是纯 ASCII，直接转，不需要 lossy
    9         String::from_utf8_lossy(slice).to_string()
   10     }
   11 }
   12
   13 // 2. 处理 PEB LDR 模块名 (UTF-16 LE -> String/Hash)
   14 // 来源：LdrDataTableEntry -> BaseDllName
   15 pub fn parse_peb_unicode_string(unicode_str: &UNICODE_STRING) -> String {
   16     unsafe {
   17         if unicode_str.Buffer.is_null() || unicode_str.Length == 0 {
   18             return String::new();
   19         }
   20         let len = (unicode_str.Length / 2) as usize; // 字节转 u16 个数
   21         let slice = core::slice::from_raw_parts(unicode_str.Buffer, len);
   22         String::from_utf16_lossy(slice)
   23     }
   24 }
   25
   26 // 3. 比较 Hash (为了免杀，通常不比较字符串，只比较 Hash)
   27 // 统一标准：无论来源是 ASCII 还是 UTF-16，都按 u8 逐字节处理，还是统一转成 u16 处理？
   28 // 推荐：统一把 ASCII 扩展成 u16 (e.g., 'A' -> 0x0041) 再算 Hash。
   29 // 这样你的 Hash 函数就能同时兼容 PE 里的名字和 PEB 里的名字。
   30 pub fn calc_hash_u16(buffer: &[u16]) -> u32 {
   31     let mut hash = 0x811c9dc5; // FNV-1a offset basis
   32     for &byte in buffer {
   33         // 注意：这里要处理大小写不敏感
   34         let c = if byte >= b'A' as u16 && byte <= b'Z' as u16 {
   35             byte + 32
   36         } else {
   37             byte
   38         };
   39
   40         // 简单处理：把 u16 拆成两个 u8 还是只用低位？
   41         // 通常红队算法里，如果确定是 ASCII 字符，只取低位算 Hash 即可。
   42         // 或者 (hash ^ c) * prime
   43         hash ^= c as u32;
   44         hash = hash.wrapping_mul(0x01000193);
   45     }
   46     hash
   47 }

  总结
   * 找文件头、找导出函数 -> 盯着 ASCII (`u8`)，注意 \0 结尾。
   * 找模块基址、解析 ApiSet、伪装命令行 -> 盯着 UTF-16 (`u16`)，注意 UNICODE_STRING 的
     Length。


# 前言

## 为啥要定义这么多数据结构(types.rs中)

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

### 一个可执行文件产生多个进程时PEB是怎么样的?

一个可执行文件产生多个进程,但这不改变“每个进程有一个 PEB”的事实。

一个可执行文件产生了多个进程，情况如下：  

1. 多开（Multiple Instances）:多次打开同一个可执行文件.系统创建了两个完全独立的进程（PID 1001 和 PID 1002）.PEB: 它们各自拥有自己独立的 PEB。虽然它们来自同一个 .exe文件，但它们在内存中是两个互不相干的世界。
2. 父子进程（Spawning Child Processes）:在一个可执行文件中,调用 CreateProcess("myapp.exe") 自我复制.结果: 一个父进程，一个子进程。 PEB: 依然是两个独立的 PEB。
3. 多线程（Multi-threading）:这是最容易混淆的。一个进程可以包含多个线程。结果: 1 个进程，N 个线程。PEB: 所有这 N 个线程共享同一个 PEB（因为它们属于同一个进程） TEB: 每个线程拥有自己独立的 TEB (Thread Environment Block)。

无论一个 .exe 启动了多少次，或者它自己又派生了多少子进程，只要那是 Windows上的一个标准 Win32 进程，它就一定拥有一个属于它自己的、独一无二的 PEB。

## ApiSetMap

请使用windbg(notepad示例)理解ApiSetMap相关结构体的定义,在windbg文件夹中有

在 PEB 结构体中，ApiSetMap 字段被定义为 PVOID（即void*），因为它是一个不透明指针，指向的结构体随着Windows 版本变化（Win7,Win8, Win10 结构体都不一样）。

因为在PEB中,ApiSetMap被定义为一个指针,要找到它真正指向的结构体，你需要去查找 Loader (Ldr) 相关的头文件，而不是 PEB 的头文件。

在phnt的github仓库中,有该结构体的详细定义(ntpebteb.h文件中)

Google 搜索：site:geoffchappell.com "API Set Schema"

目前主流环境（Win10/11）使用的是 Schema Version 6。所有的Offset（偏移量）都是相对于 ApiSetMap 结构体起始地址的字节偏移

### peb.apisetmap作用

ApiSetMap 是 Windows 用户层的“DNS服务器”,有四个数据结构组成,

详解其作用有:

1. 解决“DLL地狱”与解耦

在 Windows 7 之前，程序依赖kernel32.dll。但随着系统升级，微软想重构内核，把功能移动到 kernelbase.dll 或ucrtbase.dll 中如果直接改文件名，成千上万的老程序（写死了依赖 kernel32.dll）就会崩溃

微软发明了 API Sets（即那些 api-ms-win-core-...dll）,如虚拟文件名：api-ms-win-core-processthreads-l1-1-0.dll,物理文件名：kernel32.dll 或 kernelbase.dll  
操作系统加载器在运行时查这张表，把虚拟名“翻译”成物理名

2. 为什么结构体这么复杂？（因为要支持“千人千面”）

为什么有 NAMESPACE、NAMESPACE_ENTRY、VALUE_ENTRY这么多层级？

假设有一个虚拟 DLL 叫 `api-ms-win-core-memory-l1-1-0.dll`
* 如果是普通程序（如 notepad.exe）加载它，它应该指向 `kernelbase.dll`。
* 如果是某些遗留程序（为了兼容性），它可能指向 `kernel32.dll`。

简单的 Key-Value 做不到, ApiSetMap 的结构逻辑（一对多 + 条件判断）：

1. 第一层（Namespace Entry 数组）：
       * 你在数组里找到了 `api-ms-win-core-memory-l1-1-0.dll` 这一项。
       * 这项数据告诉你：“想知道我到底是谁？去看我的 Value Entry 数组，我有 2 个可能的身份。”

   2. 第二层（Value Entry 数组）：
       * Value Entry [0]:
           * 条件 (Importing Name): "OldLegacyApp.exe"
           * 结果 (Host Name): "kernel32.dll"
           * 含义：如果是 OldLegacyApp.exe 问我，我就伪装成 kernel32.dll。
       * Value Entry [1]:
           * 条件 (Importing Name): NULL (无条件/默认)
           * 结果 (Host Name): "kernelbase.dll"
           * 含义：如果是其他任何人问我，我就指向 kernelbase.dll。

之所以要定义这么多结构体，是因为这不是一个静态的“别名表”，而是一个带有条件判断逻辑的动态路由表。
   * NAMESPACE 是数据库入口。
   * NAMESPACE_ENTRY 是所有的虚拟 Key。
   * VALUE_ENTRY 是带有if-else 条件的物理 Value。


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
    pub Count: u32, // 虚拟 DLL 的数量(表示 API_SET_NAMESPACE_ENTRY数组中元素的个数)你需要用它来控制遍历循环的边界
    pub EntryOffset: u32,// 命名空间条目数组的偏移(指向 API_SET_NAMESPACE_ENTRY数组相对头部的起始偏移,也就是RVA)
    pub HashOffset: u32,  // 哈希条目数组的偏移(指向 API_SET_HASH_ENTRY数组的起始偏移)
    pub HashFactor: u32   // 哈希乘数(计算 API Set名称哈希时使用的乘数)计算目标名称哈希时需要乘以这个值
}

```

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
	pub NameOffset: u32,  // 虚拟 DLL名称字符串偏移(指向该虚拟 DLL 名称的 Unicode 字符串),注意: 这里的字符串没有 Null 结尾 (\0)，必须结合 NameLength 读取
	pub NameLength: u32,//  虚拟 DLL名称字节长度(该名称的字节数,不是字符数),因为是 Unicode (UTF-16)，所以字符数 = NameLength / 2
	pub HashedLength: u32,  // 用于哈希计算的长度(API Set 名称通常包含后缀（如 -1-0），但哈希计算可能只取前缀。此字段指明算哈希时取多少字节)
	pub ValueOffset: u32,// 指向 API_SET_VALUE_ENTRY 数组的偏移(指向 API_SET_VALUE_ENTRY 数组的起始位置。这个数组包含该虚拟 DLL 对应的真实宿主 DLL 信息)
	pub ValueCount: u32,// 宿主映射规则的数量(该虚拟 DLL 有多少个可能的宿主（通常是 1 个，但可能有多个用于不同导入者）)
}
```

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

####  API_SET_HASH_ENTRY (哈希索引条目)

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
* L3 (Kernel32): LoadLibrary, GetProcAddress
* L2 (Ntdll): LdrLoadDll, LdrGetProcedureAddress

如果你想获取某个函数地址，传统的做法是调用LoadLibrary("api-ms-win-core-sysinfo-l1-1-0.dll")。
* EDR 看到：你在尝试加载一个虚拟 DLL，它会拦截并检查

`resolve_api_set_map` 的价值：通过直接读取 PEB 内存（src/types.rs中定义的结构体），自己在用户态实现了路由解析逻辑

1. 你拿到了虚拟 DLL 名
2. 你遍历内存中的结构体，算出了它对应的物理 DLL 是 kernelbase.dll
3. 你直接去加载 kernelbase.dll（或者如果在内存里有了，直接用）
4. 你完成了解析，但没有调用任何 Windows API。EDR 的 Hook根本捕捉不到这一过程。你实现了“无声”的模块定位

2. 隐蔽的 IAT 解析

很多安全产品扫描内存中的 IAT  
* 如果你的 Payload 导入表里明晃晃写着 kernel32.dll，容易被分析。
* 利用 ApiSetMap，你可以让你的 Payload 看起来只依赖一些晦涩的 ext-ms-win-...虚拟 DLL。
* 静态分析工具可能无法轻易知道这些虚拟 DLL 到底指向什么功能。
* 而你的 puerto 加载器在运行时通过解析 PEB 动态还原它们，实现了静态混淆，动态还原。

3. 各结构体与免杀之间联系

1. `API_SET_NAMESPACE` (The Database Header)
       * PEB中的含义：整个路由数据库的元数据。
       * 你的代码用途：获取 EntryOffset，这是进入迷宫的入口。
       * 免杀意义：这是内存中一块只读数据，EDR很少监控对它的读取操作，是安全的“信息源”。

2. `API_SET_NAMESPACE_ENTRY` (The Key / 虚拟DLL)
       * PEB中的含义：数据库的“索引键”，代表所有可能存在的虚拟文件名。
       * 你的代码用途：在这里循环，匹配你的目标（如contract_name）。
       * 免杀意义：通过遍历这里，你可以确认当前系统支持哪些 API集，用来做环境指纹识别（比如判断是 Win10 还是 Win11），从而动态下发不同的Payload，反沙箱技术的一种。

   3. `API_SET_VALUE_ENTRY` (The Value / 物理DLL)
       * PEB中的含义：数据库的“值”，代表真实的磁盘文件路径。
       * 你的代码用途：获取最终的物理路径，传给你的 retrieve_module_add或其他加载函数。
       * 免杀意义：这里藏着真理。攻击者甚至可以（理论上，虽然很难因为是只读内存）修改这里，让系统把所有对 kernel32 的调用重定向到你恶意的DLL，实现全局劫持（ApiSet Hijacking）。

   4. `API_SET_HASH_ENTRY` (The Speed Hack)
       * PEB中的含义：为了让系统启动变快做的哈希索引。
       * 你的代码用途：如果你想写得极快，用这个二分查找。如果你不在乎几微秒的性能，可以直接暴力遍历 Namespace，代码更少，特征更小。


这四个结构体都是数组吗?
这四个结构体之间的联系?请聚个简单的例子说明为啥需要这么多结构体表示映射关系?之所以需要这么多结构体的逻辑是什么?
现在ai那么厉害,还有如openclaw的这种流行项目,我学习这些东西真的有用吗?会过时吗?会被ai替代吗?



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