## pub fn ignoring_set_fpreg

作用:手动解析PE文件的UNWIND_INFO数据解构,一步步还原该函数在prologue阶段开辟的物理空间


**该函数核心任务:**  
1. 通过win64的二进制元数据,精确计算一个函数在执行时占用多少字节的栈空间
2. 该函数手动模拟windows内核(RtlVirtualUnwind)的行为

```rust
pub fn ignoring_set_fpreg(module: *mut c_void, runtime: &IMAGE_RUNTIME_FUNCTION) -> Option<u32> {...}
```
1. module:dll的基址(如kernelbase.dll的内存起始位置)
2. runtime:指向.pdata节中的一个条目.该条目记录了某个函数的起始地址\结束地址\UnwindData(即uwd中的IMAGE_RUNTIME_FUNCTION)
3. 返回该函数在栈上共分配多少字节,以u32表示

### Ok(UWOP_SET_FPREG) => i += 1

UWOP_SET_FPREG  代表 “设置帧指针寄存器（Set Frame Pointer Register）”  
win64下,编译器在函数prologue建立栈帧时分两种:
1. rsp-based:函数内部所有局部变量和参数的定位都是基于rsp+偏移量完成
2. rbp-based:函数开头将rsp的值备份到基址指针rbp中(即设置帧指针).此后函数内部访问局部变量将全部通过rbp计算.此时,必须在UNWIND_INFO中注册UWOP_SET_FPREG操作码,来明确该函数使用rbp作为帧指针管理堆栈
3. 这一步动作没有进行任何物理内存的分配，也没有改变栈的深度.所以代码中不增加栈帧大小



### Windows x64 SEH 内存寻路与包含关系全景图（含 Windows 官方归属定义）


```markdown

        pe.base (模块基址指针 *mut c_void)
           │
           ▼ [地址指针指向] (.nt_header() 计算 DOS 头的 e_lfanew 并与 base 相加进行寻址)
        (*nt) (NT 头结构体 IMAGE_NT_HEADERS)
           │
           ▼ [直接包含] (.OptionalHeader 字段作为成员直接嵌套在 NT 标头内存中)
        (*nt).OptionalHeader (可选头部分)
           │
           ▼ [直接包含] (.DataDirectory 数组直接定义在可选头结构体的尾部)
        (*nt).OptionalHeader.DataDirectory[3] (数据目录项 IMAGE_DATA_DIRECTORY)
           │
           ▼ [地址指针指向] (通过 RVA 寻址到外部独立的内存段)
           │   ├─► 物理位置：PE 文件的 .pdata 节区 (段)
           │   └─► 官方定义：winnt.h 中的 RUNTIME_FUNCTION (单独的、独立的结构体)
        (base + RVA) as *const IMAGE_RUNTIME_FUNCTION
        (指向异常表/运行时函数表数组首地址)
           │
           ▼ [数组取值] (通过 entries().iter().find() 或 scan_runtime 遍历获取数组项的借用)
        runtime (当前函数的表项 &IMAGE_RUNTIME_FUNCTION)
           │
           ▼ [地址指针指向] (通过表项中的 UnwindData RVA 寻址到外部独立的元数据内存段)
           │   ├─► 物理位置：PE 文件的 .rdata 节区 (只读数据段)
           │   └─► 官方定义：winnt.h 中的 UNWIND_INFO (单独的、独立的结构体)
        (base + UnwindData RVA) as *mut UNWIND_INFO (指向回退元数据头结构体)
           │
           ▼ [直接包含] (通过 (unwind_info as *mut u8).add(4) 跨越 4 字节头信息，直接对齐到内部数组起点)
        unwind_code (指向操作码数组首地址，每个元素占 2 字节，类型为 *mut UNWIND_CODE)
           │
           ▼ [直接包含] (unwind_code.add(i) 指向数组中的特定项，同属于整个操作码的连续内存块)
           │   └─► 官方定义：winnt.h 中的 UNWIND_CODE (定义在 UNWIND_INFO 内部的 union 数组)
        unwind_code[i] (当前遍历到的操作码联合体 &UNWIND_CODE)
           │
           ├─► [直接包含] (*unwind_code).Anonymous.UnwindOp() ──► 匹配 UWOP_ALLOC_SMALL 等具体栈操作码
           │
           ├─► [直接包含] (*unwind_code).Anonymous.OpInfo()   ──► 提取相关的寄存器编号或栈分配倍率
           │
           └─► [直接包含] (*unwind_code.add(1)).FrameOffset   ──► 当操作为 ALLOC_LARGE 等大空间分配时，读取延伸槽数据
           │
           ▼ [直接包含] (若包含 UNW_FLAG_CHAININFO [0x4]，父级表项以 inline 方式物理嵌入在当前回退块的末尾)
        Chained Function Entry (内嵌的父级运行时函数表项 IMAGE_RUNTIME_FUNCTION)
           │
           ▼ [地址指针指向] (递归调用 ignoring_set_fpreg，通过父级表项的 UnwindData RVA 重新跳转寻址)
        Parent UNWIND_INFO (父级函数的 UNWIND_INFO 结构体)
```



```markdown
+--------------------------------------------------------------------------+
| 1. PE 模块内存映像 (加载基址 base / module)                              |
+--------------------------------------------------------------------------+
          │
          │ ──► [uwd 源码解析起点]
          │     调用 Unwind::new(PE::parse(module)) 实例化 Unwind 对象(内部只是一个指针)。
          │     随后调用 pe.nt_header() 获取 NT 头
          │
          │ ──► [物理偏移与跳转]
          │     1. 模块基址首地址为 DOS 头起点，在偏移 0x3C 字节处读取 4 字节的 NT 头偏移。
          │     2. NT 头地址 = base + NT头偏移。
          │     3. 可选头地址 = NT 头地址 + 24 字节（跳过 4 字节签名 + 20 字节文件头）。
          │     4. 数据目录表起始地址 = 可选头地址 + 112 字节（x64 架构下系统配置字段大小）。
          │     5. 异常目录项地址 = 数据目录表起始地址 + 24 字节（第 4 个元素，索引 3），从中读取异常表 RVA。
          │        在 uwd 中体现为：(*nt).OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXCEPTION]
          │
          └─► 异常表地址 = base + dir.VirtualAddress
                │   在 uwd 中通过 from_raw_parts(addr, len) 包装成 &[IMAGE_RUNTIME_FUNCTION] 切片。
                │   该切片由 pe_kernelbase.entries() 返回。
                v
+--------------------------------------------------------------------------+
| 2. 异常目录表 / .pdata 节区 (IMAGE_RUNTIME_FUNCTION 数组)                |
|    这里为每一个“非叶子函数”注册了一个描述项                               |
+--------------------------------------------------------------------------+
| [0] IMAGE_RUNTIME_FUNCTION (函数 A)                                      |
| [i] IMAGE_RUNTIME_FUNCTION (当前函数)                                    |
|     ├─ BeginAddress : u32  (函数的起始 RVA) -----------------> 指向 .text 实际代码 |
|     ├─ EndAddress   : u32  (函数的结束 RVA) -----------------> 指向 .text 代码结束 |
|     └─ UnwindData   : u32  (UnwindInfo 相对偏移 RVA) --------------------\
+--------------------------------------------------------------------------+
                 │                                                         │
                 │ ──► [uwd 源码解析节点]                                   │
                 │     在 uwd 中通过 Unwind::function_by_offset(offset)     │
                 │     进行二分/线性查找，锁定当前函数对应的表项。          │
                 │     然后调用 ignoring_set_fpreg(module, runtime)。       │
                 │                                                         │
                 │ ──► [物理偏移与跳转]                                     │
                 │     每一个异常表项大小为 12 字节，偏移 8 字节处为        │
                 │     UnwindData RVA                                       │
                 │     UnwindInfo 实际地址 = base + UnwindData RVA          │
                 │     <────────────────────────────────────────────────────/
                 v
+--------------------------------------------------------------------------+
| 3. 回退元数据头 (UNWIND_INFO 结构体，大小为 4 字节)                       |
+--------------------------------------------------------------------------+
| ├─ VersionFlags            : u8  (高 5 位为 Flags 标志位，低 3 位为 Version 版本)  |
| ├─ SizeOfProlog            : u8  (函数序言的字节长度)                     |
| ├─ CountOfCodes            : u8  (紧随其后的 UNWIND_CODE 元素数量)        |
| └─ FrameRegisterAndOffset  : u8  (若使用了帧指针寄存器如 RBP，记录其寄存器编号与偏移) |
+--------------------------------------------------------------------------+
          │
          │ ──► [uwd 源码解析节点]
          │     ignoring_set_fpreg 中定义：
          │     let unwind_code = (unwind_info as *mut u8).add(4) as *mut UNWIND_CODE;
          │
          │ ──► [物理偏移与跳转]
          │     跳过这 4 字节的 UnwindInfo 头部信息（地址加 4 字节），即为操作码数组起始点。
          v
+--------------------------------------------------------------------------+
| 4. 序言操作码数组 (UNWIND_CODE 数组，每个元素大小为 2 字节)                |
+--------------------------------------------------------------------------+
| [0] UNWIND_CODE (联合体)                                                  |
|     ├─ CodeOffset          : u8  (该操作在 Prologue 内部的偏移)           |
|     ├─ UnwindOp            : u4  (具体的栈操作：UWOP_ALLOC_SMALL / PUSH_NONVOL)   |
|     └─ OpInfo              : u4  (操作数信息，如寄存器编号或分配的大小)    |
| [1] UNWIND_CODE (额外的偏移量信息，常作为前一个操作码的延伸数据，如 ALLOC_LARGE 的大小)  |
| ...                                                                       |
| [CountOfCodes - 1] UNWIND_CODE                                            |
+--------------------------------------------------------------------------+
          │
          │ ──► [uwd 源码解析节点]
          │     在 ignoring_set_fpreg 的 while 循环中，利用 unwind_code.add(i) 遍历操作码。
          │     并解析 (*unwind_code).Anonymous.OpInfo() 和 UnwindOp()。
          │     若发现 UNW_FLAG_CHAININFO 标志位：
          │     let runtime = unwind_code.add(index) as *const IMAGE_RUNTIME_FUNCTION;
          │
          │ ──► [物理偏移与跳转]
          │     如果 VersionFlags 中的 Flags 包含 UNW_FLAG_CHAININFO [0x4]
          │     跳过整个 UNWIND_CODE 数组。为了满足 4 字节对齐要求：
          │     - 奇数个操作码：向后偏移 (CountOfCodes + 1) * 2 字节
          │     - 偶数个操作码：向后偏移 CountOfCodes * 2 字节
          v
+--------------------------------------------------------------------------+
| 5. 链式父级结构 (嵌套的 IMAGE_RUNTIME_FUNCTION)                           |
|                                                                           |
|  当函数存在非连续代码段（如热/冷路径分离）时，该结构体指向它的父级运行时函数 |
+--------------------------------------------------------------------------+
|    ├─ BeginAddress : u32  -------------> 指向父级运行时函数               |
|    ├─ EndAddress   : u32                                                 |
|    └─ UnwindData   : u32  -------------> 递归调用本函数，继续解析父级 Unwind 空间    |
+--------------------------------------------------------------------------+
```