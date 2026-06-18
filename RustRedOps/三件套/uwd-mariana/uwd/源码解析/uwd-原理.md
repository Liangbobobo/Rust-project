- [背景知识](#背景知识)
  - [执行流](#执行流)
  - [win64 异常处理机制及对应的uwd源码](#win64-异常处理机制及对应的uwd源码)
    - [.pdata](#pdata)
    - [IMAGE\_RUNTIME\_FUNCTION](#image_runtime_function)
    - [UNWIND\_INFO](#unwind_info)
    - [UNWIND\_CODE\\bitfield!](#unwind_codebitfield)
    - [enum UNWIND\_OP\_CODES](#enum-unwind_op_codes)
  - [uwd 为什么要返回地址（博弈点）](#uwd-为什么要返回地址博弈点)


# 背景知识

## 执行流

// 将敏感函数地址放入r11中,通过jmp r11直接跳转执行.当敏感函数执行后调用ret,只会弹出8字节的返回地址,不会主动清理堆栈上的影子空间和参数数据.此外,ret前,rsp指向栈顶的返回地址,即AddRspXGadget(uwd/uwd.rs中在系统dll找到的add rsp, X; ret 机器码)的地址,该地址是准备阶段手动构造后写入当前栈顶的.
// ret时,cpu将栈顶AddRspXGadget地址弹出赋给rip,同时rsp+8后,rsp指向废弃的影子空间.此时控制流进入AddRspXGadget，CPU执行 `add rsp,X`。该指令强行让栈指针向下平移 X个字节，完美跨越并丢弃了那片废弃的影子空间（及堆栈参数数据）
// rsp跨越后指向在堆栈中预置的下一个返回地址JmpRbxGadget（合法系统DLL 中的 `jmp [rbx]` 机器码地址）.然后继续执行该gadget的ret,cpu将栈顶的JmpRbxGadget地址弹出给rip,控制流随之跳入并执行jmp [rbx].由于在伪造堆栈的初始阶段,让brx指向保存有RestoreSynthetic（底层的恢复现场代码）绝对地址的那个原始栈槽。因此,jmp [rbx]直接用过解引用,在不依赖当前堆栈下,隐蔽/安全的将控制流导回真实的恶意代码内部,最终RestoreSynthetic负责清理剩余的假栈帧并恢复真实的寄存器环境.


// 敏感函数执行完毕ret时,cpu跳到`AddRspXGadget`释放当前的伪造栈帧,然后ret跳转到`JmpRbxGadget`.由于jmp不改变rsp,不压入返回地址.rsp不是指向kernelbase.dll!jmp [rbx](这是rip指向的可执行代码段).而是指向JmpRbxGadget宿主函数的堆栈空间(即JmpRbxGadget被pop后,栈指针指向其分配的栈帧区域).而rbx寄存器始终指向存放RestoreSynthetic(uwd/synthetic.asm:用于承接rbx,后续敏感api执行后,通过ROP链AddRspXGadget->JmpRbxGadget,最终执行系统dll中的jmp [rbx])绝对地址的那个原始栈槽.通过jmp [rbx]就能将控制流导回,即在敏感api执行后,安全的将控制流从伪造的系统栈帧导回真实的恶意代码中.原因是:如果敏感api执行后调用ret,如果直接返回到源码中,栈顶(即返回地址)必须是内存中处于不合法PE模块区域的恶意代码地址.那么敏感api处于执行或休眠时,如果edr引擎对该线程stack walk栈回溯,会发现这个栈顶地中没有关联任何合法磁盘pe文件(如系统dll),从而被判定为栈欺骗.因此,必须在执行前,将栈顶的返回地址伪造为合法的系统地址(AddRspXGadget),在敏感api执行完毕后,用AddRspXGadget释放假栈帧,再借助 jmp [rbx]的非压栈跳转特性,通过解引用rbx(其指向保存有  RestoreSynthetic  地址的栈槽),隐秘的跳回RestoreSynthetic.以此再不往栈中留下任何非法内存地址痕迹的前提下,安全的收入控制流.
// 但是edr的回溯不仅检查当前执行指令是不是在合法的IAT中,还会检查函数的栈展开信息(.pdata节区保存).
// 回溯引擎读取栈顶后,退栈一步发现返回地址是 kernelbase!JmpRbxGadget 的宿主函数（例如处于系统函数RtlpSearchExceptionHandlers  内部）.回溯引擎去.pdata中找gadget宿主函数的UNWIND_INFO,假设该函数在prologue中分配了0x30的栈帧.
// 回溯继续,将rsp+0x30后读取对应地址的值.这里可能是对齐/垃圾/参数等数据,导致栈展开失败.因此在scan_runtime中用`ignoring_set_fpreg`过滤使用帧指针寄存器的函数,找到gadget宿主函数的UNWIND_INFO,并计算该宿主函数的栈帧大小.
// 在uwd的synthetic.asm/SpoofSynthetic内伪造栈帧,并手动执行sub rsp,栈帧大小.用以模拟并匹配该宿主函数的UNWINDO_INFO栈展开规则,确保edr回溯时rsp+栈帧大小(加8字节返回地址)后,能落到下一个合法的栈帧,从而欺骗回溯引擎.
// 然后在这个伪造的栈帧底部,填入下一个伪造的合法返回地址BaseThreadInitThunk以及RtlUserThreadStart,知道用0截断回溯链




## win64 异常处理机制及对应的uwd源码

在 x86 时代，回溯栈靠的是 EBP 链(帧指针)。每个函数在序言(prolog)中都会执行push ebp;mov ebp,esp.回溯时,只需要沿着ebp指向的地址像链表一样向上爬即可.x86下这是必须的

但在 x64下，通过rip查表就可以知道当前函数栈帧大小,不需要RBP指针也能精准回溯.现代编译器(MSVC,Rustc)默认开启帧指针省略,在ntdll.dll等系统组件中,绝大多数函数不再使用RBP序言,这种情况下,RBP成为一个通用易失性寄存器,编译器多了一个可使用的寄存器做高速运算,可以减少内存访问,同时省去了push mov pop等指令(但并没有完全取消在1.调试中2.动态分配(函数内部使用alloca()或变长数组,导致栈帧大小在编译时无法确定,编译器必须使用RBP锁定局部变量的访问基准)3.某些复杂的内核函数依然保留)   
即win64下,运行核心逻辑是依靠表(.pdata)回溯,而不是依靠链(RBP chain)回溯  
win64引入的基于表格的异常处理机制,编译器在编译每个函数时,会额外生成一段元数据(.xdata节,对应uwd中UNWIND_INFO结构体),记录这个函数如何操作栈\保存了哪些寄存器.这些元数据的索引被存放在PE文件的.pdata段,即Exception Directory.  
源码中的IMAGE_RUNTIME_FUNCTION就是这个索引中的每一个条目.

**uwd中对异常处理机制的解析:**  
代码地址 ->栈布局说明 -> 物理还原


Unwind (Manager/Struct)->IMAGE_RUNTIME_FUNCTION -> UNWIND_INFO -> UNWIND_CODE -> UNWIND_OP_CODES (枚举) 

1. 作用链(逻辑依赖关系):  
Unwind (Manager/Struct)  // uwd源码封装.本质是PE解析器,负责在整个内存模块中搜索和匹配,作为回溯逻辑的context
->IMAGE_RUNTIME_FUNCTION // 指令区间的索引锚点.只要cpu的rip落在这个结构体包含的区间,系统就会认定,必须且只能使用它绑定的哪个UNWIND_INFO
->UNWIND_INFO           // 描述一个函数回溯信息的全局特征(是否启用帧寄存器/版本等),是后续UNWIND_CODE的容器头
->UNWIND_CODE           // 原始操作码,记录序言中单次汇编动作.是对prolog阶段每条修改rsp或rbp指令的二进制镜像.是逆向的脚本
->UNWIND_OP_CODES (枚举) // 语义解释器,将4
bit的原始数据解释为具有物理意义的操作动作

2. 位置链(布局逻辑):  
Unwind (Manager/Struct)  // rust程序的运行态内存中
->IMAGE_RUNTIME_FUNCTION  //PE -> .pdata节  
->UNWIND_INFO             // .xdata节
->UNWIND_CODE             // UNWIND_INFO一部分
->UNWIND_OP_CODES (枚举)  // 

### .pdata

win64下.pdata节(Procedure Data)被称为异常目录(Exception Directory),是实现基于表格的异常处理及异步栈回溯的核心基础.  
它是由编译器(GCC/Rustc/Clang)在生成PE文件时自定义的一个段名,之所以叫.pdata是由于微软制定的PE/coff标准规范继承下来的.在os内核/加载器,没有这个.pdata的名称,在PE文件的Optional Header结尾有一个数组Data Directory,在Data Directory`[3]`,有一个结构体,这个结构体的一个字段指向的位置,os内核会把这里当作Exception Table来解析

1. 核心作用:与x86的异常处理依赖栈上链表不同.win64下,基于性能和标准化回溯流程的考虑,编译器将每个函数的行为(栈的使用情况\寄存器的使用情况)预先编码为元数据.
* .pdata是一个全局索引.当cpu执行到RIP指向的某地址发生了异常或需要回溯时,os通过.pdata进行检索:该地址属于哪个函数,他的回溯说明(UNWIND_INFO)在什么地方
* uwd中通过扫描.pdata检验一个伪造的返回地址是否落在一个合法函数范围

2. 物理结构及位置: .pdata本质是由IMAGE_RUNTIME_FUNCTION结构体组成的连续数组.每个条目固定占用12字节(3个32位的RVA).  
.pdata节不在固定的物理位置,Optional Header->Data Directory(Optional Header末尾的数组)->Data Directory`[3]`->IMAGE_DIRECTORY_ENTRY_EXCEPTION->VirtualAddress(指向.pdata节在内存中的偏移地址)

3. 强制排序:.pdata的所有条目必须按照BeginAddress的升序排列.这使得os能够使用二分查找算法定位任意RIP对应的函数条目

4. Non-leaf Function,凡是调用其他函数\修改栈指针\保存非易失性寄存器的函数,必须在.pdata中注册;而leaf Function,不操作栈/不调用其他函数,可以不在.pdata中注册,os回溯时默认其只占用一个返回地址(8字节)

5. .pdata仅作为索引,详细的回溯操作码(Unwind Codes)存放在.xdata节中.得以实现索引与数据的解耦,优化内存页的加载效率

6. 对抗意义 (Anti-Forensics perspective): 现代 EDR（如 SentinelOne 或 CrowdStrike）的栈校验逻辑：
* 验证一： 返回地址必须落在某个 .pdata 条目的 [BeginAddress, EndAddress] 范围内
* 验证二： 返回地址指向的指令，其对应的 UNWIND_INFO必须与其物理栈的表现一致


**Exception Directory(.pdata) 的机制：**
* .pdata 段：存储了一个 RUNTIME_FUNCTION数组(对应uwd源码中的IMAGE_RUNTIME_FUNCTION)，记录了每个函数的起始、结束地址。.pdata段是一个连续的IMAGE_RUNTIME_FUNCTION数组,可以通过(End_of_pdata - Start_of_pdata) /size_of(IMAGE_RUNTIME_FUNCTION) 来计算该模块有多少个函数
* 不是所有函数都有.pdata条目,如果一个函数不调用其他函数/不修改堆栈(不分配空间)/不修改非易失性寄存器,它就可能没有.pdata条目.但在uwd中,要伪造的通常是非叶子函数,因为我们要作为调用者存在
   * UNWIND_INFO：每个函数对应一个描述符，记录了该函数如何分配栈空间、保存了哪些寄存器。
   * RtlVirtualUnwind：这是系统用于回溯的核心函数。它根据当前 RIP 在 .pdata找函数，再根据 UNWIND_INFO “撤销”当前栈帧，找到上一层调用者。

uwd 的核心任务： 伪造一套符合上述规则的、指向合法 DLL 的 .pdata 记录和栈帧，让RtlVirtualUnwind 在回溯时“迷路”，最后带它走到BaseThreadInitThunk（合法的线程起点）。  
uwd项目中,在内存中寻找合法的 已有的IMAGE_RUNTIME_FUNCTION条目,然后借用这些条目  
1. 为了让伪造的栈看起来真实,uwd从kernelbase.dll这种合法模块中挑一个合法的IMAGE_RUNTIME_FUNCTION
2. uwd读取BeginAddress,然后计算出该函数内部的一个偏移量.它会把这个返回地址伪装成合法函数内部的代码地址
3. 它必须读取UnwindData对应的UNWIND_INFO,以确保它伪造出来的栈大小(stack size)与该合法函数声明的大小完全一致

### IMAGE_RUNTIME_FUNCTION

```rust
#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct IMAGE_RUNTIME_FUNCTION {
    pub BeginAddress: u32,// RUNTIME_FUNCTION数组,记录了每个函数的起始\结束地址
    pub EndAddress: u32,
    pub UnwindData: u32// RVA,指向一个具体的数据结构(UNWIND_INFO)
}
```

位置: IMAGE_RUNTIME_FUNCTION条目存放在.pdata段(Exception Directory),这是一个连续的数组.该数组的每个元素是IMAGE_RUNTIME_FUNCTION  
PE Header -> Optional Header -> Data Directory 数组-> 索引为 3 的位置（IMAGE_DIRECTORY_ENTRY_EXCEPTION）就是异常目录 -> VirtualAddress指向.pdata开头(Size除以12(IMAGE_RUNTIME_FUNCTION 的大小)就是函数条目总数)

各字段含义:  
1. BeginAddress: u32 
* 含义:该函数相对镜像基址(ImageBase)的偏移量(RVA),实际指向的是什么?即这个RVA加上基址后的指针指向的是什么?实际指向该函数第一条机器码指令
* 作用:当系统或EDR要回溯栈时,它们拥有RIP(当前的指令指针).它会去.pdata表中搜索一个条目,使得BeginAddress <= (RIP - ImageBase) < EndAddress
* 类型为啥是u32:代表一个相对值.为了减小PE文件体积,并让代码在内存中重新加载(ASLR)后依然有效

2. EndAddress (u32) —— 函数的终点（RVA）
* 含义： 函数结束位置相对于镜像基址的偏移量.实际指向的是什么?即这个RVA加上基址后的指针指向的是什么?实际指向该函数的最后一条机器码指令,通常是ret之后的第一个字节.和BeginAddress共同划定了该函数在内存中的范围
* 作用： 标记了该函数的代码边界。如果 RIP超出了这个范围，说明当前指令不属于这个函数，或者该函数是一个“叶子函数”（Leaf Function，不分配栈空间、不调用其它函数，通常没有 .pdata 条目）
* 意义:如果你的代码执行流（RIP）落在一个没有任何IMAGE_RUNTIME_FUNCTION覆盖的内存区域，EDR的回溯算法会立即判定这是一个“异常调用栈”，因为正常的已编译函数必须被 .pdata覆盖。

3. UnwindData (u32) —— 栈操作说明书（RVA）
* 含义： 指向另一个结构体 UNWIND_INFO 的 RVA
* 作用：存放函数调用时保存的具体元数据:
  *  这个函数分配了多少栈空间？
  *  它把 RBX, RSI, RDI 等寄存器备份到了栈的哪个位置？
  *  它是否使用了帧指针（RBP）？
  *  它的序言（Prolog）有多少字节？
*  栈展开流程： 当 RtlVirtualUnwind 被调用时，它读取这个字段找到UNWIND_INFO，然后执行逆向操作：如果 UNWIND_INFO 说“函数序言里减去了 0x40字节栈空间”，那么展开函数就会把 RSP 加上 0x40，从而恢复到调用者的栈状态


### UNWIND_INFO

```rust
/// Structure containing the unwind information of a function.
#[repr(C)]
pub struct UNWIND_INFO {
    /// Separate structure containing `Version` and `Flags`.
    pub VersionFlags: UNWIND_VERSION_FLAGS,// 低三位是版本号,高5位是标志位flags

    /// Size of the function prologue in bytes.
    pub SizeOfProlog: u8,// 序言大小(字节数)

    /// Number of non-array `UnwindCode` entries.
    pub CountOfCodes: u8,// 下面Unwindcode数组的条目数

    /// Separate structure containing `FrameRegister` and `FrameOffset`.
    pub FrameInfo: UNWIND_FRAME_INFO,//帧寄存器信息.如果不使用RBP通常为0

    /// Array of unwind codes describing specific operations.
    pub UnwindCode: UNWIND_CODE,

    /// Union containing `ExceptionHandler` or `FunctionEntry`.
    pub Anonymous: UNWIND_INFO_0,

    /// Optional exception data.
    pub ExceptionData: u32,
}

#[repr(C)]
pub union UNWIND_INFO_0 {
    /// Address of the exception handler (RVA).
    pub ExceptionHandler: u32,

    /// Address of a chained function entry.
    pub FunctionEntry: u32,
}
```

位置: UNWIND_INFO 结构体通常存放在 .xdata 段（或者是 .rdata 段）.UNWIND_INFO和IMAGE_RUNTIME_FUNCTION是独立的,多个 IMAGE_RUNTIME_FUNCTION 条目甚至可以指向同一个UNWIND_INFO（如果这些函数的栈操作完全一样），这样可以节省 PE 文件体积 

**关于union:**  
Rust/C中,union意味着两个字段共享同一块内存空间(这里大小4字节).  
这里使用union因为,一个函数要么有自己的异常处理器,要么是一个链式条目,两个不会同时存在,通过union可以节省空间.  
那这里为什么不用enum?  
* 内存布局的“强制性”（ABI Compatibility）
  * PE结构是硬编码在os 内核中的,对于ntdll.dll里栈回溯算法来说,它预期在UNWIND_INFO变长数组后的那个位置,正好偏移4字节的地方找到一个RVA地址
* enum会额外占用1个以上字节作为标签,用来标记当前是哪个变体.如果使用enum,这个结构体大小会变成4字节数据+1字节标签(甚至会因为对齐变成8字节).这会破坏整个PE结构偏移量,导致RtlVirtualUnwind读到错误的数据,引发系统崩溃
* Rust的union(裸联合体),union不占用任何额外空间,它的两个字段完美重叠在一个4字节空间里,这与win定义的二进制布局完全一致
* union代表对一块内存的不同解释,enum设计模式匹配,在编译时会产生额外的检查逻辑和分支代码,增加二进制文件的指纹

以上, UNWIND_INFO_0 结构体启用哪个字段,已经由UNWIND_INFO中VersionFlags: UNWIND_VERSION_FLAGS字段指定了,再使用enum增加一个标签不仅冗余而且会破坏内存布局

1. ExceptionHandler: u32  
* 含义:指向 __C_specific_handler（针对 C/C++）或自定义异常处理函数的 RVA 
* 当你代码里写了 try-except 或 panic!时，系统回溯到这个函数发现它有ExceptionHandler，就会调用它来决定是“处理这个异常”还是“继续向上抛”
* 在uwd中,如果一个函数有 ExceptionHandler，说明它比较复杂。uwd在挑选“肉盾函数”时，有时会避开或特殊处理这种函数，以防干扰异常链

1. FunctionEntry (链式函数条目)
* 含义： 指向另一个 IMAGE_RUNTIME_FUNCTION 结构的 RVA
* 背景知识（非常重要）：  
  有些函数特别大，或者被编译器拆分成了多个不连续的部分
  * 当 UNWIND_INFO 的 Version 字段中设置了 UNW_FLAG_CHAININFO 标志时，这个union(union UNWIND_INFO_0) 就被视为 FunctionEntry.它告诉系统：“我这个条目只是这个函数的一部分，真正的栈信息请去这个FunctionEntry 指向的地方看。”
  * uwd中,这是重构时最容易出错的地方。如果你没处理链式条目，直接去读栈大小，你会读到一个错误的值。joaoviictorti 的 uwd源码中会有递归或循环逻辑来处理这种“链条”


### UNWIND_CODE\bitfield!


```rust
#[repr(C)]
pub union UNWIND_CODE {
    pub FrameOffset: u16,    // 有时这 2 个字节代表一个偏移量
    pub Anonymous: UNWIND_CODE_0, // 有时这 2 个字节代表一组位域信息
}

bitfield::bitfield! {
    pub struct UNWIND_CODE_0(u16);
    pub u8, CodeOffset, SetCodeOffset: 7, 0;  // 偏移量（记录在哪一条指令发生的栈操作）
    pub u8, UnwindOp, SetUnwindOp: 11, 8;     // 操作码（OpCode，具体做了什么）
    pub u8, OpInfo, SetOpInfo: 15, 12;        // 附加信息（通常是寄存器编号或大小）
}
```

UNWIND_CODE是UNWIND_INFO结构体的一个字段,它逐条记录了函数在启动时Prolog对栈做了什么.

**核心背景:**  
* 当EDR进行栈回溯时,其目的是看当前函数运行完后,返回地址在哪里.即如果程序崩溃,os怎么知道如何将栈恢复unwind到上一层.而函数可能在执行过程中动态修改了栈（例如 sub rsp, 0x40）,因此编译器在生成函数时,会将上述对修改栈的操作记录在pe文件的.xdata节中.UNWIND_CODE就这这张表中的一条撤销指令,作用就是记录这些修改，以便 RtlVirtualUnwind能像“倒带”一样，一步步撤销这些操作，把 RSP 恢复到函数被调用前的样子
* win64 ABI中,一个UNWIND_CODE回溯操作码占2个字节:第一个字节：CodeOffset。该操作在函数序言（Prolog）中的偏移量;第二个字节：这是一个被拆分的字节，包含两个 4-bit 的字段.高4位OpInfo。操作信息（如寄存器索引、堆栈比例）,低 4 位 (Bits 0-3)：UnwindOp。具体的操作码（如 PUSH、ALLOC）
  
| 位索引 | 15 14 13 12 | 11 10 09 08 | 07 06 05 04 03 02 01 00 |
| :--- | :--- | :--- | :--- |
| 字段 | OpInfo | UnwindOp | CodeOffset |
| 长度 | 4 bits | 4 bits | 8 bits |

* Windows的元数据定义通常是非常紧凑的位域(bit-field),rust没有原生的像c的位域支持,这里使用bitfield!作为中间层搭建桥梁.

**bitfield!**  
bitfield库通过宏,将16位原始二进制数据转为具有逻辑意义的字段,比直接操作字节安全高效.  
1. 结构体声明-pub struct UNWIND_CODE_0(u16);元组结构体,即该结构体在内存中本质上是一个2字节的无符号整数
  * 每一个UNWIND_CODE在PE文件的.xdata节中,占用2字节,使用u16保证内存布局的绝对对齐
  * UNWIND_CODE_0:在微软官方UNWIND_CODE union中,第一种解释方式是CodeOffset+UnwindOP+OpInfo 因此后缀带上_0
  
2. 字段一：pub u8, CodeOffset, SetCodeOffset: 7, 0;
  * 通过宏,自动生成对应的mask(掩码)和shift(位移)代码;该字段表示生成名为CodeOffset和SetCodeOffset的函数,使用对应的mask shift进行位操作(取反掩码/逻辑与/shift).从16位的数据中读取或set低8位(7-0)
  * u8:返回类型/设置类型.虽然原始数据是u16,但提取出的偏移量只需要8位,因此定义为u8
  * CodeOffset：Getter 方法名。调用 instance.CodeOffset() 将返回提取的值.
  * SetCodeOffset：Setter 方法名。调用 instance.SetCodeOffset(val)将修改内部二进制位
  * 7, 0：位区间（Bit Range）即从第 7 位到第 0 位（包含两端）.内部物理逻辑（宏生成的函数）:
    * 读取：(self.0 & 0x00FF)。将低 8 位提取出来
    * 转换：由于占据的是最低 8 位，不需要移位，直接转为 u8
* 作用:记录该操作在函数序言（Prolog）中的偏移位置（相对于函数起始地址）
* 这是 EDR还原栈帧的第一步。它告诉系统：在执行到函数第几个字节时，RSP 发生了变化

3. 字段二：pub u8, UnwindOp, SetUnwindOp: 11, 8;
  * 对应11到8位的操作
* 决定了这条指令到底做了什么（Push, Alloc, SetFP 等）.在enum UNWIND_OP_CODES 中对应
* 这是 uwd 逻辑分发的核心。如果解析出是ALLOC_SMALL，则需要计算字节；如果是 SET_FPREG，则可能需要忽略

4. 字段三：pub u8, OpInfo, SetOpInfo: 15, 12;
  * 生成Getter逻辑:((self.0 & 0xF000)>>12) as u8
  * 作用：提供操作码的附加参数;如果是 PUSH：代表寄存器索引（如 3 代表 RBX;如果是 ALLOC：参与计算分配的字节数
  * 在 ignoring_set_fpreg 函数中，OpInfo 的意义完全取决于它左边的 UnwindOp


**字段二通过TryFrom转换**  
**uwd中这些操作(字段二的组合)被映射为 UNWIND_OP_CODES 枚举**

### enum UNWIND_OP_CODES

```rust
#[repr(u8)]
#[allow(dead_code)]
pub enum UNWIND_OP_CODES {
    UWOP_PUSH_NONVOL = 0,
    UWOP_ALLOC_LARGE = 1,
    UWOP_ALLOC_SMALL = 2,
    UWOP_SET_FPREG = 3,
    UWOP_SAVE_NONVOL = 4,
    UWOP_SAVE_NONVOL_BIG = 5,
    UWOP_EPILOG = 6,
    UWOP_SPARE_CODE = 7,
    UWOP_SAVE_XMM128 = 8,
    UWOP_SAVE_XMM128BIG = 9,
    UWOP_PUSH_MACH_FRAME = 10,
}
```

1. UWOP_PUSH_NONVOL = 0 :UWOP_PUSH_NONVOL是一个UNWIND_CODE节点(2字节).UWOP(unwind operation回溯操作),代表UnwindOp(共4位)的含义.是win64 seh结构异常处理的核心,它不是执行指令,用于撤销指令操作;NONVOL(Non-volatile)非易失性,用于对寄存器分类(共有8个:RBX, RBP, RDI, RSI, R12, R13, R14, R15)
  * 当UnwindOp是UWOP_PUSH_NONVOL时,Opinfo代表寄存器编号.opinfo是5代表rbp,3是rbx
  * 对栈的影响.假设要伪装kernelbase.dll某个函数,这个函数的.pdata有3个UWOP_PUSH_NONVOL,那么在虚假栈中必须腾出3*8共24字节空间;当edr调用RtlVirtualUnwind检查这个函数的栈时,回溯读取UWOP_PUSH_NONVOL,并预期在当前的栈指针位置找到一个有效的寄存器数值;后续是寄存器恢复为压栈的值

2. UWOP_ALLOC_SMALL = 2: 全称为unwind operation allocate small stack area.对应汇编指令sub rsp, constant
  * small限制分配的大小在8-128字节空间,再大编译器需要使用UWOP_ALLOC_LARGE.
  * 当UnwindOp是WOP_ALLOC_SMALL时,Opinfo这4位不再代表寄存器的类型,而是一个倍数因子.根据AMD规范,opinfo为0代表倍数因子是1,15代表16
  * x64栈指针rsp必须8字节对齐.即最小操作单位是8字节

3. UWOP_ALLOC_LARGE = 1:全称Unwind Operation Allocate Large Stack Area.对应汇编指令sub rsp, constant
  * 与samll不同,opinfo的4位表示不同的分配策略.0代表最高分配512KB-8字节/1代表最高分配4GB-8字节
  * 当UnwindOp是UWOP_ALLOC_LARGE时.会占用紧邻的一个或2个unwind_code.当opinfo为0的时候,unind_code这个union不再是Anonymous而是FrameOffset(u16).此时FrameOffset中存的是字节数(实际大小/8).对应的此时的i应加2,即除了自身的unwind_code又跳过了一个,到达第三个位置
  * opinfo为1时,源码使用了*(unwind_code.add(1) as *mut i32).代表将紧邻的下一个unwind_code(`*mut unwind_code` 16字节的原始指针)强转为`*mut i32`,意味着一次性横跨两个unwind_code.占用三个unwind_code对应的i应加3
  * 此时根据微软规范,不再将额外占用的两个unwind_code当作结构体看待,而是把它们变为纯粹的raw data原始数据容器.此时,栈的大小就是通过紧随其后的两个槽位合成的一个 32 位原始数据（Raw
  Data）来表示的
  * `*(unwind_code.add(1) as *mut i32)`,此时当前的unwind_code用于区分类型(large分配),add(1)表示移到下一个unwind_code,这里是起点,是转为的*mut i32的数据的起点.而i32是一个4字节数据,读取一个4字节数据相当于跳过了两个unwind_code
  * win的线程栈默认1mb,最大很少超过几十mb
  * 使用i32是win abi的行业标准,对应c中的LONG/DWORD

4. UWOP_SAVE_NONVOL = 4:
  * 背景知识:push和mov在栈上的物理区别.push rsi会让rsp做减法,动态增加栈大小;`mov [rsp+0x40],rsi`时,rsp不动,用到的空间必须由之前的sub rsp ,x提前准备.
  * 编译器有时候为了优化会一次性分配空间,然后用mov将寄存器放入栈空间
  * 这个操作码是否代表mov?还能代表其他操作吗
  * 微软约定此时占用两个unwind_code.第一个表示操作码和寄存器索引(opinfo);另一个以FrameOffset的形式代表存储位置的偏移.对应i+2
  * 这个操作不增栈

5. UWOP_SAVE_NONVOL_BIG = 5:当函数分配巨大的栈(如1mb),又想将寄存器存到非常靠后的位置(偏移超过512kb)时,16位的unwind_code就无法表示偏移了
  * 占用3个unwind_code.第一个表示操作码(unwindop)和寄存器索引(opinfo);








## uwd 为什么要返回地址（博弈点）

这是你理解 uwd 源码的关键：

1. **EDR 的检测逻辑**：  
   当你的恶意程序调用 NtAllocateVirtualMemory 时，EDR 会查看当前的栈。  
   它会读取 [RSP]，发现返回地址指向 0x0000000013371000。  
   它一查：“这个地址属于哪个 DLL？”  
   结果发现：“这个地址不属于任何已加载的合法 DLL，它是你的恶意 Shellcode 所在的内存！”  
   结论： 判定为攻击，拦截。

2. **uwd 的欺骗逻辑**：  
   uwd 不使用 CALL 指令去调用敏感函数。  
   它手动操作 RSP，在栈上手动写入一个看起来非常合法的地址（例如 kernelbase.dll 内部的某个地址）。  
   然后它使用 JMP 跳过去执行。  
   - **当 EDR 检查时**： 它看到返回地址指向 kernelbase.dll。  
   - **EDR 认为**： “哦，是系统自己在分配内存，没问题。”  
   - **实际上**： 当函数 RET 时，它会跳到 uwd 事先布置好的 ROP Gadget（如 add rsp, XX; ret），最后经过多次跳转，安全回到你的代码。

---

总结疑问：

- 返回地址的作用？ 告诉 CPU “函数执行完后回哪儿”。  
- 存放在什么地方？ 存放在内存的 栈（Stack） 中，具体在每个函数栈帧的基底。  
- 如何保存的？ 硬件 CALL 指令自动压栈，或者由 uwd 这种项目手动写入（伪造）。

理解了这一点，你现在看 uwd.rs 里的那些偏移量计算，就会发现它们全是在为了“准确地在栈上找到存放返回地址的那 8 个字节”而努力。

你是否想看看 uwd 源码中是如何定义“伪造返回地址”这个动作的？（提示：搜索 `config.return_address`）。

---


非易失性通用寄存器rbx.  

**在uwd中:**  
1. 汇编开始处,push rbx把使用之前的rbx值存入栈
2. lea rbx,rsp 将rbx指向栈顶,此时栈顶存的是恢复函数地址
3. 跳转gadget:在kernelbase.dll中寻找0xFF 0x23对应指令(即jmp qword ptr `[rbx]`)
4. 如果直接jmp RestoreSynthetic,跳转到恢复函数.edr直接明白了跳转逻辑.使用lea rbx,rsp和jmp qword ptr `[rbx]`,先把rbx指向栈顶,然后跳往栈顶地址.更像函数返回ret的操作.但相比ret底层可控
**此时:**  
1. rbx指向栈上的一个位置
2. 这个位置存放这真正的恢复程序的入口
3. 调用jmp rbx这个合法指令,cpu会认为正在进行一个正常的间接跳转
4. 效果是EDR只看到一条位于kernelbase.dll内部的合法跳转指令





























