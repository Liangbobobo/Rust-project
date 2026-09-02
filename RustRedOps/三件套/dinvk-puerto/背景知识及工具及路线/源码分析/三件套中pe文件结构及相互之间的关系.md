## PE文件整体结构

| 物理内存位置 | 结构/内容 | 详细字段与说明 | 三件套核心链路应用 |
| --- | --- | --- | --- |
| 物理内存低地址 (Base Address / *mut c_void) | 1. IMAGE_DOS_HEADER (DOS 头, 64 字节) | ├── e_magic: 0x5A4D ("MZ" 魔数, 2 字节)<br>└── e_lfanew: 0x3C 偏移处 (i32) (记录 NT 头相对偏移 RVA) | |
| | 2. DOS Stub (DOS 历史残留汇编, "This program cannot be run...") | | |
| | 3. IMAGE_NT_HEADERS (NT 头结构体, 共 264 字节) (nt_header() 目标位置) | ├── Signature: 0x00004550 ("PE\0\0" 4 字节魔数校验)<br>├── FileHeader (20 字节)<br>│    ├── Machine: 0x8664 (AMD64 / x64 架构)<br>│    └── NumberOfSections: 节区总数量 (u16)<br>└── OptionalHeader (可选头 240 字节, IMAGE_OPTIONAL_HEADER64)<br>     ├── ImageBase: 模块首选虚拟加载基址<br>     ├── AddressOfEntryPoint: 程序入口点 RVA<br>     └── DataDirectory[16] (16 个关键数据目录项数组)<br>          ├── [0] EXPORT Directory<br>          ├── [1] IMPORT Directory (导入表 IAT)<br>          ├── [3] EXCEPTION Directory (.pdata)<br>          ├── [5] BASERELOC Directory (重定位表)<br>          └── [14] CLR Runtime Header (DotNet) | |
| | 4. 节区头目录表 (IMAGE_SECTION_HEADER 数组, 每个 40 字节) (sections() 切片) (紧随 NT 头末尾: (nt as *const u8).add(size_of::<NT>())) | ├── [节表 1: .text] (Name, VirtualAddress, VirtualSize, Characteristics: 0x60000020 RX)<br>├── [节表 2: .rdata] (Name, VirtualAddress, VirtualSize, Characteristics: 0x40000040 R)<br>├── [节表 3: .data] (Name, VirtualAddress, VirtualSize, Characteristics: 0xC0000040 RW)<br>├── [节表 4: .pdata] (Name, VirtualAddress, VirtualSize, Characteristics: 0x40000040 R)<br>└── [节表 5: .reloc] (Name, VirtualAddress, VirtualSize, Characteristics: 0x42000040 R) | |
| | 5. 各节区实际数据内容区 (Actual Section Raw Data in Memory) | .text 节区 (代码段 / 可执行机器码) | [三件套应用]:<br>1. mariana: 搜寻 ROP 跳板 (48 83 C4 58 C3 / FF 23)<br>2. mariana: find_valid_instruction_offset (48 FF 15...)<br>3. puerto: 扫描 syscall; ret (0F 05 C3 间接系统调用跳板) |
| | | .rdata 节区 (只读数据段 / 存放导出表与常量)<br>└── IMAGE_EXPORT_DIRECTORY (导出目录表)<br>     ├── NumberOfNames: 导出函数名称数量<br>     ├── NumberOfFunctions: 导出函数总数量<br>     ├── AddressOfNames (RVA) ──► [Name RVA 0, Name RVA 1, ...] (ASCII字符串)<br>     ├── AddressOfNameOrdinals (RVA) ───► [Ordinal 0, Ordinal 1, ...] (u16序号数组)<br>     └── AddressOfFunctions (RVA) ──────► [Func RVA 0, Func RVA 1, ...] (函数入口) | [puerto]: 逐个对比 fnv1a_utf16_from_u8 哈希<br>[puerto]: 通过名字索引拿到函数的序号索引 (Ordinal)<br>[puerto Hell's Gate]: 读取机器码 4C 8B D1 B8 <SSN> 00 00 提取系统调用号<br>[puerto 转发导出]: 若 RVA 落在导出表区间内，递归解析目标 DLL!API |
| | | .pdata 节区 (异常展开目录段 / Exception Directory)<br>└── IMAGE_RUNTIME_FUNCTION 结构体数组 (每个 12 字节)<br>     ├── BeginAddress (函数起始 RVA)<br>     ├── EndAddress (函数结束 RVA)<br>     └── UnwindData (指向 UNWIND_INFO 的 RVA)<br>          └── UNWIND_INFO (版本, 标志位, 序言大小)<br>          └── UNWIND_CODE 数组 (解析 11 种操作码):<br>               • UWOP_PUSH_NONVOL (非易失寄存器压栈)<br>               • UWOP_ALLOC_SMALL / ALLOC_LARGE<br>               • UWOP_SET_FPREG (RBP 帧指针建立)<br>               • UWOP_SAVE_XMM128 (向量寄存器保存)<br>               • UWOP_PUSH_MACH_FRAME (硬件机器帧)<br>               • UNW_FLAG_CHAININFO (递归链式展开) | [mariana / uwd 核心]: 精确计算出函数的真实物理栈深与 RBP 偏移 |
| | | .data 节区 (可读写数据段 / 全局变量) | |
| | | .reloc 节区 (基址重定位表) | [samoa / 最终加载器应用]: 模块未加载到首选基址时修复指针 |
| 物理内存高地址 | | | |


## PE 导出表（Export Directory）结构和其内部字段之间关系

**导出表Export Directory:**只有提供函数接口的 PE 文件（主要是DLL）才拥有导出表；只要它拥有导出表，就必然由三大数组（名字表、序号桥梁表、函数地址表）共同联动，存储该模块所有的导出函数信息

**PE导出表设计为三个数组的原因:**
1. 支持“仅靠序号导出（Export by Ordinal）”：Windows 允许某些内部函数只有序号、没有名字；因此，函数地址表的数量（NumberOfFunctions）通常 ≥函数名字表的数量（NumberOfNames）
2. 支持“极速二分查找（Binary Search）”：微软规定：名字表（AddressOfNames）必须按字母 A~Z 严格升序排列；但是函数在内存里的地址（AddressOfFunctions）是乱序的；微软引入了 AddressOfNameOrdinals 作为中间路由

**PE导出表三个数组分别代表的含义:**
| 编号 | 数组名称 | 元素类型 | 元素数量 | 物理作用 |
| --- | --- | --- | --- | --- |
| ① | AddressOfNames (names) | u32 (RVA) | NumberOfNames | 名字指针表：每个元素是一个 RVA，指向一个以 \0 结尾的 ASCII 串 |
| ② | AddressOfNameOrdinals (ords) | u16 (序号) | NumberOfNames | 桥梁转换表：每个元素是一个数字，记录该名字在函数表里的真实下标 |
| ③ | AddressOfFunctions (funcs) | u32 (RVA) | NumberOfFuncs | 函数地址表：每个元素是一个 RVA，指向函数真正的机器码入口 |

**三个数组之间配合的寻址流程:**

**当以名字或hash查找一个api时**,代码在三大数组中的流转轨迹如下:
1. 读取第 i 个名字:`names[i]` (RVA) ──────► + Base ──────► "VirtualAlloc" (ASCII 字符串)或哈希对比成功
2. 用相同的下标 i，去桥梁表查真实序号:`ords[i]` ─────────────► 读取出一个 u16 整数 (例如 42)
3. 把 42 当作下标，直接杀入函数地址表:`funcs[42]` (RVA)──────► + Base ──────► 0x7FFF_8957_CC60 (目标函数的真实绝对物理地址 VA)