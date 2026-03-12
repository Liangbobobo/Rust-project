# 背景知识

## win64 异常处理机制及对应的uwd源码

在 x86 时代，回溯栈靠的是 EBP 链(帧指针)。每个函数在序言(prolog)中都会执行push ebp;mov ebp,esp.回溯时,只需要沿着ebp指向的地址像链表一样向上爬即可.  
但在 x64下，为了释放RBP寄存器作为通用寄存器使用,获取性能，取消了这种机制。win64引入了基于表格的异常处理机制,编译器在编译每个函数时,会额外生成一段元数据,记录这个函数如何操作栈\保存了哪些寄存器.这些袁术的索引被存放在PE文件的.pdata段,即Exception Directory.源码中的IMAGE_RUNTIME_FUNCTION就是这个索引iao中的每一个条目.Windows 改用一种名为 Exception Directory(.pdata) 的机制：
   * .pdata 段：存储了一个 RUNTIME_FUNCTION数组(对应uwd源码中的IMAGE_RUNTIME_FUNCTION)，记录了每个函数的起始、结束地址。.pdata段是一个连续的IMAGE_RUNTIME_FUNCTION数组,可以通过(End_of_pdata - Start_of_pdata) /size_of(IMAGE_RUNTIME_FUNCTION) 来计算该模块有多少个函数
   * 不是所有函数都有.pdata条目,如果一个函数不调用其他函数/不修改堆栈(不分配空间)/不修改非易失性寄存器,它就可能没有.pdata条目.但在uwd中,要伪造通常是非叶子函数,因为我们要作为调用者存在
   * UNWIND_INFO：每个函数对应一个描述符，记录了该函数如何分配栈空间、保存了哪些寄存器。
   * RtlVirtualUnwind：这是系统用于回溯的核心函数。它根据当前 RIP 在 .pdata找函数，再根据 UNWIND_INFO “撤销”当前栈帧，找到上一层调用者。


uwd 的核心任务： 伪造一套符合上述规则的、指向合法 DLL 的 .pdata 记录和栈帧，让tlVirtualUnwind 在回溯时“迷路”，最后带它走到BaseThreadInitThunk（合法的线程起点）。  
uwd项目中,在内存中寻找合法的 已有的IMAGE_RUNTIME_FUNCTION条目,然后借用这些条目  
1. 为了让伪造的栈看起来真实,uwd从kernelbase.dll这种合法模块中挑一个合法的IMAGE_RUNTIME_FUNCTION
2. uwd读取BeginAddress,然后计算出该函数内部的一个偏移量.它会把这个返回地址伪装成合法函数内部的代码地址
3. 它必须读取UnwindData对应的UNWIND_INFO,以确保它伪造出来的栈大小(stack size)与该合法函数声明的大小完全一致

UnwindData UNWIND_INFO是什么关系,在PE中是怎么存放的?这两个是独立的结构体吗?

**源码:**

```rust
#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct IMAGE_RUNTIME_FUNCTION {
    pub BeginAddress: u32,// RUNTIME_FUNCTION数组,记录了每个函数的起始\结束地址
    pub EndAddress: u32,
    pub UnwindData: u32// RVA,指向一个具体的数据结构(UNWIND_INFO)
}
```

位置: MAGE_RUNTIME_FUNCTION条目存放在.pdata段(Exception Directory),这是一个连续的数组.该数组的每个元素是IMAGE_RUNTIME_FUNCTION  
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


### UnwindCode: UNWIND_CODE

作用:  
如果把IMAGE_RUNTIME_FUNCTION 比作地图，把 UNWIND_INFO 比作建筑图纸，那么UNWIND_CODE就是图纸上的施工步骤

这是UNWIND_INFO结构体的一个字段,它逐条记录了函数在启动时Prolog对栈做了什么.

**核心背景:**  
当EDR进行栈回溯时,其目的是当前函数运行完后,返回地址在哪里.  
要找到返回地址，系统必须知道当前函数的栈帧（Stack Frame）有多大。但函数可能在执行过程中动态修改了栈（例如 sub rsp, 0x40）。UNWIND_CODE 的作用就是记录这些修改，以便 RtlVirtualUnwind能像“倒带”一样，一步步撤销这些操作，把 RSP 恢复到函数被调用前的样子。?这里不理解,返回地址的作用?存放在什么地方?如果保存的?

**函数返回地址**  

1. 返回地址是 CPU执行完当前函数后，下一条需要执行的指令在内存中的绝对地址
* 作用:路标.指引CPU执行完当前函数后,下一步执行的指令
* 如何保存:返回地址的产生是硬件级别的,由CPU的CALL指令自动完成.假设在汇编中执行CALL 0x123456(调用函数的指令)时,CPU实际上会把CALL指令之后的指令地址(即当前RIP寄存器的值)压入当前栈顶(RSP).然后Jmp跳转把RIP修改为目标函数地址(这里的0x123456).这里RIP RSP的代表什么?作用?
* 存放位置:win64下,返回地址存放在stack中,

在 Win64 环境下，返回地址存放在 栈（Stack） 中。


  栈的物理布局：
  当一个函数被调用的一瞬间，栈的状态是这样的：



  ┌─────────────────────┬─────────────────┬────────────────────────┐
  │ 内存地址 (高地址 -> │ 存放内容        │ 说明                   │
  │ 低地址)             │                 │                        │
  ├─────────────────────┼─────────────────┼────────────────────────┤
  │ 0x000000A0          │ ...             │ 调用者之前的栈内容     │
  │ 0x00000098          │ Return Address  │ 就在这里！ 由 CALL     │
  │                     │                 │ 指令压入               │
  │ 0x00000090          │ Shadow Space    │ Win64 特有的 32        │
  │                     │ (RCX)           │ 字节预留空间           │
  │ 0x00000088          │ Shadow Space    │ ...                    │
  │                     │ (RDX)           │                        │
  │ 0x00000080          │ Shadow Space    │ ...                    │
  │                     │ (R8)            │                        │
  │ 0x00000078          │ Shadow Space    │ ...                    │
  │                     │ (R9)            │                        │
  │ 0x00000070          │ Local Variables │ 函数内部定义的局部变量 │
  │ ...                 │ ...             │ RSP 指向当前位置       │
  └─────────────────────┴─────────────────┴────────────────────────┘


  关键结论：
   1. 位置固定： 对于被调用函数来说，它的第一个返回地址永远在它进入函数时的
      [RSP] 位置。
   2. 不可见性： 在正常的 C/Rust 代码中，你看不见这个地址，它被编译器和 CPU
      隐藏了。但对于汇编和 uwd 来说，它就是一个普通的 8 字节内存数值。

  ---

  四、 它是如何起作用的？（RET 指令）


  当函数执行到最后一条指令 RET 时，CPU 执行反向操作：
   1. 弹栈（Pop）： 从当前 RSP 指向的位置读取 8 字节数值。
   2. 恢复（Restore）： 把这个数值强行赋给 RIP（指令指针）。
   3. 跳转： 于是 CPU 回到了调用者继续执行。

  ---

  五、 uwd 为什么要“动”它？（博弈点）

  这是你理解 uwd 源码的关键：


  1. EDR 的检测逻辑：
  当你的恶意程序调用 NtAllocateVirtualMemory 时，EDR 会查看当前的栈。
  它会读取 [RSP]，发现返回地址指向 0x0000000013371000。
  它一查：“这个地址属于哪个 DLL？”
  结果发现：“这个地址不属于任何已加载的合法 DLL，它是你的恶意 Shellcode
  所在的内存！”
  结论： 判定为攻击，拦截。


  2. uwd 的欺骗逻辑：
  uwd 不使用 CALL 指令去调用敏感函数。
  它手动操作 RSP，在栈上手动写入一个看起来非常合法的地址（例如 kernelbase.dll
  内部的某个地址）。
  然后它使用 JMP 跳过去执行。
   * 当 EDR 检查时： 它看到返回地址指向 kernelbase.dll。
   * EDR 认为： “哦，是系统自己在分配内存，没问题。”
   * 实际上： 当函数 RET 时，它会跳到 uwd 事先布置好的 ROP Gadget（如 add rsp,
     XX; ret），最后经过多次跳转，安全回到你的代码。

  ---

  总结疑问：


   * 返回地址的作用？ 告诉 CPU “函数执行完后回哪儿”。
   * 存放在什么地方？ 存放在内存的 栈（Stack） 中，具体在每个函数栈帧的基底。
   * 如何保存的？ 硬件 CALL 指令自动压栈，或者由 uwd 这种项目手动写入（伪造）。


  理解了这一点，你现在看 uwd.rs
  里的那些偏移量计算，就会发现它们全是在为了“准确地在栈上找到存放返回地址的那 8
  个字节”而努力。


  你是否想看看 uwd 源码中是如何定义“伪造返回地址”这个动作的？（提示：搜索
  config.return_address）。



  


 一、 核心背景：为什么要“逆向”施工？


  当 EDR
  进行栈回溯时，它的目标是回答一个问题：“当前函数运行完后，返回地址在哪里？”


  要找到返回地址，系统必须知道当前函数的栈帧（Stack
  Frame）有多大。但函数可能在执行过程中动态修改了栈（例如 sub rsp, 0x40）。
  UNWIND_CODE 的作用就是记录这些修改，以便 RtlVirtualUnwind
  能像“倒带”一样，一步步撤销这些操作，把 RSP 恢复到函数被调用前的样子。

  ---

  二、 源码结构深度拆解


  在 uwd/src/types.rs 中，它的定义非常精妙（利用了位域和联合体）：


    1 #[repr(C)]
    2 pub union UNWIND_CODE {
    3     pub FrameOffset: u16,    // 有时这 2 个字节代表一个偏移量
    4     pub Anonymous: UNWIND_CODE_0, // 有时这 2 个字节代表一组位域信息
    5 }
    6
    7 bitfield::bitfield! {
    8     pub struct UNWIND_CODE_0(u16);
    9     pub u8, CodeOffset, SetCodeOffset: 7, 0;  //
      偏移量（记录在哪一条指令发生的栈操作）
   10     pub u8, UnwindOp, SetUnwindOp: 11, 8;     //
      操作码（OpCode，具体做了什么）
   11     pub u8, OpInfo, SetOpInfo: 15, 12;        //
      附加信息（通常是寄存器编号或大小）
   12 }

  1. 字段详解：


   * CodeOffset (8 bits):
       * 含义：记录了这一条栈操作指令相对于函数起始地址（BeginAddress）的偏移。
       * 作用：回溯时，如果当前的 RIP 还在序言（Prolog）中，系统只会撤销那些
         CodeOffset 小于当前位置的操作。
   * UnwindOp (4 bits):
       * 含义：操作码。它是灵魂。它告诉系统这条指令是
         push、alloc（减去栈空间）还是 set_fpreg（设置帧指针）。
   * OpInfo (4 bits):
       * 含义：附加参数。例如，如果 UnwindOp 是 PUSH_NONVOL，那么 OpInfo
         就代表被压入栈的是哪个寄存器（0=RAX, 1=RCX...）。

  ---


  三、 关键操作码 (Unwind Opcodes) 与 uwd 的关联

  在重构 uwd 时，你需要处理以下几种常见的 OpCodes：


   1. UWOP_PUSH_NONVOL (0):
       * 汇编对应：push rbx
       * 回溯动作：RSP = RSP + 8
   2. UWOP_ALLOC_LARGE (1):
       * 汇编对应：sub rsp, 0x123456
       * 注意：这种操作非常特殊，它会占用 2 个或 3 个连续的 UNWIND_CODE
         单元来存储那个巨大的 32 位数值。这是重构解析逻辑时最容易写出 Bug
         的地方。
   3. UWOP_ALLOC_SMALL (2):
       * 汇编对应：sub rsp, 0x20
       * 计算公式：大小 = (OpInfo * 8) + 8。
   4. UWOP_SET_FPREG (3):
       * 汇编对应：mov rbp, rsp
       * 重要性：这告诉回溯算法，之后不要再看 RSP 了，去看 RBP。

  ---

  四、 uwd 为什么要深挖这些“施工步骤”？

  这是 joaoviictorti 项目中最核心的计算逻辑。

  在 uwd.rs 中，有一个关键需求：计算一个合法函数的总栈大小（Total Stack Size）。


  重构逻辑如下：
   1. 遍历 UNWIND_INFO 里的所有 UNWIND_CODE。
   2. 根据 UnwindOp 识别出它是 ALLOC_SMALL 还是 PUSH_NONVOL 等。
   3. 累加这些操作对 RSP 造成的所有改变。
   4. 最后加上 8 字节（返回地址本身占用的空间）。


  为什么要算得这么准？
  因为 uwd 的目标是伪装。如果你挑选了一个 kernelbase!SomeFunction
  作为“肉盾”，这个函数的栈一共占了 0x40 字节。那么你的 uwd
  汇编代码就必须在内存中也开辟正好 0x40 字节，并且在正确的位置填入假的返回地址。


  如果算错 1 个字节： RtlVirtualUnwind
  就会在回溯时偏离轨道，读到一个错误的内存地址作为“上一层返回地址”，EDR
  就会立刻报警：“检测到损坏的调用栈！”。

  ---

  五、 深度总结与疑问


  你现在已经理解了：
   1. .pdata (目录) 找到 UNWIND_INFO (图纸)。
   2. UNWIND_INFO 包含一系列 UNWIND_CODE (施工指令)。
   3. uwd 通过解析这些指令，像素级复刻了一个合法函数的栈布局。

  进阶提问：
  既然我们要“借用”别人的栈空间，那么当我们执行完自己的恶意操作（比如
  Syscall）后，该如何全身而退？ 毕竟我们把栈改得面目全非。


  你想看 uwd 是如何处理这个“收尾”工作的，还是想看具体的 UNWIND_CODE
  解析算法实现？（提示：解析算法在 uwd.rs 的 ignoring_set_fpreg 函数中）。