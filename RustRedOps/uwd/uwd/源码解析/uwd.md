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

**栈的物理布局：**  
当一个函数被调用的一瞬间，栈的状态是这样的：

```text
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
```

关键结论：  
1. **位置固定**： 对于被调用函数来说，它的第一个返回地址永远在它进入函数时的 `[RSP]` 位置。  
2. **不可见性**： 在正常的 C/Rust 代码中，你看不见这个地址，它被编译器和 CPU 隐藏了。但对于汇编和 uwd 来说，它就是一个普通的 8 字节内存数值。


**它是如何起作用的？（RET 指令）**

x64下,ret是一个组合动作.其实质等于POP RIP:  
* 硬件层面:当CPU执行到RET时顺序执行以下动作:  
1. 读取并弹出(POP),从当前栈指针RSP指向的内存位置读取8个字节(这8个字节就是返回地址),然后将RSP的值增加8(即释放这8字节的栈空间?这时栈中RSP增加8的位置是否还保存有原返回地址的8字节?)
2. 修改指令指针,将读取到的这8字节直接强行赋给RIP(指令指针寄存器)
3. 下一个CPU周期,CPU就会跳转到这个新地址去执行指令
4. 正常程序中,RET永远与CALL成对出现.call:在跳入函数前,先把下一条指令地址压入栈顶.ret:函数执行完后,从栈顶找回哪个地址,让CPU执行下一条指令.但,CPU并不检查ret弹出来的地址是不是真的由之前的call压进去的.只看RSP指向的地方,这个地方有什么,cpu就跳到哪里.

**EDR视角:**  
返回地址不匹配是恶意软件最明显的特征  
1. 栈回溯检测:当EDR拦截到一个敏感操作时,它会沿着栈向上看.它看到的每一个返回地址(即每一个待执行的ret目标)都必须落在合法的模块内
2. 影子栈(shadow stack/CET):现代cpu(intel 11代之后),引入了硬件级别的影子栈.cpu在硬件内部备份一份call压入地址.当ret时,硬件会自动对比现在弹出来的地址和之前备份的地址是否一致,这会直接发现返回地址不一致的情况.这肯定有解决方案,但应该很复杂


**ret在uwd中的作用:**  

1. uwd能够实现栈欺骗,本质就是利用ret的这个特性:uwd并不使用call这种标准的跳转方式,它会:
   * 手动操作RSP,在栈上手动写入看起来合法的地址(如kernelbase.dll内部的某个位置)
   * 执行自己的逻辑
   * 当目标函数(如VirtualAlloc)执行完毕调用ret时,它会从栈上弹出实现写好的合法地址
   * uwd源码中寻找的add rsp, 0x58; ret这种gadget(指令碎片),就是利用了ret.当跳到一个ret时,可以通过控制栈的内容,让cpu从一个dll的ret跳到另一个dll的ret.每次跳跃都会在栈上留下一个合法的足迹
2. 重构uwd时,必须对ret保持敬畏和谨慎
* 如果函数内部push了数据但没有pop,执行ret时弹出的就是push的数据,而不是返回地址,程序会因为跳到了非法地址而立即崩溃
* 对齐:x64中,ret发生时,栈必须是8字节对齐的(返回地址占8字节).但在调用函数前,栈通常需要16字节对齐.如果算错一个字节,ret弹出的地址就是错位的.这就牵扯到栈对齐



当函数执行到最后一条指令 RET 时，CPU 执行反向操作：  
1. **弹栈（Pop）**： 从当前 RSP 指向的位置读取 8 字节数值。  
2. **恢复（Restore）**： 把这个数值强行赋给 RIP（指令指针）。  
3. **跳转**： 于是 CPU 回到了调用者继续执行。



五、 uwd 为什么要“动”它？（博弈点）

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

一、 核心背景：为什么要“逆向”施工？

当 EDR 进行栈回溯时，它的目标是回答一个问题：“当前函数运行完后，返回地址在哪里？”

要找到返回地址，系统必须知道当前函数的栈帧（Stack Frame）有多大。但函数可能在执行过程中动态修改了栈（例如 sub rsp, 0x40）。  
UNWIND_CODE 的作用就是记录这些修改，以便 RtlVirtualUnwind 能像“倒带”一样，一步步撤销这些操作，把 RSP 恢复到函数被调用前的样子。

---

二、 源码结构深度拆解

在 uwd/src/types.rs 中，它的定义非常精妙（利用了位域和联合体）：

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

1. 字段详解：

- **CodeOffset (8 bits)**:  
  - 含义：记录了这一条栈操作指令相对于函数起始地址（BeginAddress）的偏移。  
  - 作用：回溯时，如果当前的 RIP 还在序言（Prolog）中，系统只会撤销那些 CodeOffset 小于当前位置的操作。
- **UnwindOp (4 bits)**:  
  - 含义：操作码。它是灵魂。它告诉系统这条指令是 push、alloc（减去栈空间）还是 set_fpreg（设置帧指针）。
- **OpInfo (4 bits)**:  
  - 含义：附加参数。例如，如果 UnwindOp 是 PUSH_NONVOL，那么 OpInfo 就代表被压入栈的是哪个寄存器（0=RAX, 1=RCX...）。

---

三、 关键操作码 (Unwind Opcodes) 与 uwd 的关联

在重构 uwd 时，你需要处理以下几种常见的 OpCodes：

1. **UWOP_PUSH_NONVOL (0)**:  
   - 汇编对应：push rbx  
   - 回溯动作：RSP = RSP + 8  
2. **UWOP_ALLOC_LARGE (1)**:  
   - 汇编对应：sub rsp, 0x123456  
   - 注意：这种操作非常特殊，它会占用 2 个或 3 个连续的 UNWIND_CODE 单元来存储那个巨大的 32 位数值。这是重构解析逻辑时最容易写出 Bug 的地方。  
3. **UWOP_ALLOC_SMALL (2)**:  
   - 汇编对应：sub rsp, 0x20  
   - 计算公式：大小 = (OpInfo * 8) + 8。  
4. **UWOP_SET_FPREG (3)**:  
   - 汇编对应：mov rbp, rsp  
   - 重要性：这告诉回溯算法，之后不要再看 RSP 了，去看 RBP。

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
因为 uwd 的目标是伪装。如果你挑选了一个 kernelbase!SomeFunction 作为“肉盾”，这个函数的栈一共占了 0x40 字节。那么你的 uwd 汇编代码就必须在内存中也开辟正好 0x40 字节，并且在正确的位置填入假的返回地址。

如果算错 1 个字节： RtlVirtualUnwind 就会在回溯时偏离轨道，读到一个错误的内存地址作为“上一层返回地址”，EDR 就会立刻报警：“检测到损坏的调用栈！”。

---

五、 深度总结与疑问

你现在已经理解了：  
1. .pdata (目录) 找到 UNWIND_INFO (图纸)。  
2. UNWIND_INFO 包含一系列 UNWIND_CODE (施工指令)。  
3. uwd 通过解析这些指令，像素级复刻了一个合法函数的栈布局。

进阶提问：  
既然我们要“借用”别人的栈空间，那么当我们执行完自己的恶意操作（比如 Syscall）后，该如何全身而退？ 毕竟我们把栈改得面目全非。

你想看 uwd 是如何处理这个“收尾”工作的，还是想看具体的 UNWIND_CODE 解析算法实现？（提示：解析算法在 uwd.rs 的 ignoring_set_fpreg 函数中）。


## 扩展-栈对齐

相关背景:winx64下,指针和寄存器都是64位8字节的,但程序栈上定义的是16字节对齐,这是为啥.  

这牵扯到数据大小size和对齐边界aligment,两个概念:  
一次读取的数据大小”通常被称为 位宽（Bus Width/Word Size）；  
而对齐（Alignment） 是指数据在内存中存放的起始地址必须是位宽的整数倍,这种情况下结束地址也是位宽的整数倍.

**关于位宽:**  
位宽这个词在不同的语境下，含义是不一样的  

1. 寄存器位宽（Architectural Bit Width）——这是我们常说的 “64位系统” 的定义。
* 大小： 64 位（8 字节）。
* 含义： 通用寄存器（如 RAX, RBX, RSP）存储的数据大小。
* 一次读取： 当你执行 mov rax, [ptr] 时，CPU 逻辑上“一次”从内存抓取了 8个字节放进寄存器。

2. 内存总线位宽（Memory Bus Width）——主板和内存条之间的物理连接。
* 大小： 通常也是 64 位（单通道）或 128 位（双通道）。
* 含义： 电子信号在主板上一次能并排跑多少个 bit。

3. 指令执行位宽（Instruction Operand Width）—— uwd 为什么要对齐 16 字节的根本原因。
* 大小： 128 位（16 字节，SSE 指令集）或 256 位（32 字节，AVX 指令集）。
* 含义： 虽然你是 64 位系统，但 CPU 内部有专门的“大胃王”寄存器（如 XMM0）。
* 一次读取： 当执行一条 movaps xmm0, [rsp] 指令时，CPU 要求 “一次性” 抓取 16个字节。


**深度辨析：为什么“一次读取”会导致“对齐”要求？**

请记住这个黄金法则：CPU 的硬件电路为了追求速度，是按“块”设计的    
* 如果对齐： 数据的起始地址正好是 16 的倍数。CPU 的 128位读取电路可以直接对准内存的这个“块”，瞬间吸走 16 字节。
* 如果不对齐： 数据跨越了两个 16 字节的块。
  * 对于普通指令（如 mov rax），CPU 会默默地分两次读，然后拼接。你只会觉得慢。
  * 对于严谨的高速指令（如 movaps），CPU设计者认为：“为了性能，我不打算在电路里加‘跨界拼接’的功能。如果你非要跨界读，我就直接罢工（抛出异常）。”

总结：
> “位宽”是 CPU 处理数据的基本单位；而“对齐”是确保这个单位的数据能被 CPU的硬件电路以“单次、最快、且不报错”的方式读取的物理前提。

**win64架构下,栈对齐stack aligment是由winx64 abi(应用二进制接口)强制规定的**
1. 8字节的数据和16字节的对齐
win64下,指针是8字节.cpu内部有一类特殊的寄存器xmm是16字节,128位的.当cpu要把一个128位的数据从内存搬到寄存器时,如果这个内存地址是16字节的,cpu就可以一次性搬完.如果地址不是16的倍数,cpu必须分多次搬运,甚至某些指令(movaps)时,硬件会报错导致程序崩溃.  
16字节不是为64位的指针准备的,而是为了高性能的128位指令(浮点运算\加密\多媒体处理)准备的

2. 返回地址为啥是8字节

不要说“返回地址是 8 字节对齐的”，这会产生误导.正确的说法是： “在函数序言（Prolog）执行完毕后，RSP 必须回到 16字节对齐；而由于 CALL 压入了 8字节，所以函数序言分配的总空间（包括被压栈的寄存器）必须是 16n + 8字节，从而抵消 CALL 带来的影响。

* 调用前16字节对齐,编译器保证,在执行call之前,rsp必须是16的倍数
* 执行call会破坏对齐:call指令自动把8字节的返回地址压栈.rsp-8,变成了16n+8的对齐方式.此时栈顶是8字节对齐
* 进入函数(修复对齐),由于rsp此时是16n+8对齐.如果不修复函数内部的一些指令(如sse)会崩溃.所以每个非叶子函数(会调用其他函数)在开头第一件事就是,再减去一个奇数倍的8字节空间
  * 例如：sub rsp, 28h (即 40 字节,这里0x20的32字节是影子空间,另外8字节是为了修复call引入的8字节)
  * 数学计算：16n + 8（初始偏移） + 40（函数空间） = 16n + 48。
  * 而 48 刚好是 16 的倍数（16 * 3）。
  * 奇迹发生了： RSP 重新变回了 16 的倍数！ 

以上:  
1. 既然sse指令可能再任何地方出现,abi必须强制要求栈再关键时刻(函数入口)是16对齐的  
2. 牺牲局部，保全整体： 虽然 CALL 压入 8字节破坏了对齐，但只要每个函数都遵循“再补一个 8字节”的约定，整体系统就能高效运行。

## 扩展-Prolog / Prologue(函数序言)

winx64下,prolog是os结构化异常处理SEH和栈回溯stack unwinding机制的基石  
1. 序言是如何产生的？（生成机制）
函数序言是由 编译器后端（Compiler Backend） 自动生成:
*  计算需求： 当你编译 Rust 或 C++ 代码时，编译器会扫描函数，统计：
  1. 局部变量占用的空间。
  2. 函数内调用的最大参数数量（确定影子空间大小）。
  3. 需要保护的寄存器（非易失性寄存器，如 RBP, RBX, RSI, RDI,R12-R15）
* 指令发射：编译器根据这些统计结果，在函数入口处插入特定的汇编指令序列
* 同步元数据： 这是最关键的一步。编译器在生成指令的同时，会在 PE文件的 .pdata 节（Exception Directory）中生成对应的 Runtime Function 结构，并在 .xdata 节中存入详细的 Unwind Codes.如果没有这些元数据，Windows无法处理该函数的异常，也无法进行正确的栈回溯

2. 序言由谁维护
* 静态阶段： 由编译器负责设计，由 链接器（Linker） 负责最终在 PE文件中编排位置。
* 动态阶段： 由 Windows 操作系统（特别是 ntdll.dll）维护。当程序崩溃或调用 RtlCaptureStackBackTrace 时，内核/NTDLL会读取 PE头部的回溯元数据，根据序言的描述“反向执行”操作，从而还原出调用栈

3. 用户是否可以更改
在高级开发和红队对抗中，用户有权且经常更改序言  
* 代码层： 在 Rust 中，可以使用 `#[naked]`属性。这告诉编译器：不要给生成任何序言或尾声，会自己手写汇编?详细说明,手写序言有何作用
* 二进制层： 通过 Hot-patching 或 Inline Hooking。比如 EDR会在敏感函数的序言处强行写入一个 jmp指令，这本质上就是破坏并接管了原始序言

4. 更改的风险
如果你手动更改了序言的代码，但没有同步更新 PE 头的 .pdata回溯表，那么：  
1. 调试器失效： WinDbg 无法显示该函数的调用栈。
2. 异常即崩溃： 如果函数内部发生异常，SEH无法找到回溯路径，程序会立即被系统强制关闭。
3. EDR 报警： 现代 EDR 会扫描内存中的函数开头，如果发现序言不符合.pdata 的描述，会直接标记为恶意篡改

5. 函数序言的核心特性
  1. 确定性顺序： x64 序言必须先 push 寄存器，再 subrsp。顺序反了，回溯引擎会解析失败。
  2. 16 字节对齐： 正如你之前提到的，序言必须通过 sub rsp 的大小来修复 call 造成的 8 字节偏移，确保后续指令在 16 字节边界上运行。
  3. 不可分割性： 在 .pdata定义的“序言范围内”，不允许出现向外的跳转指令。
  4. 影子空间（Shadow Space）： 即使函数没有参数，Windows x64调用约定也要求序言为被调用者预留 32 字节的空间

6. uwd 项目中的高级应用
* 特性利用： uwd 搜索 kernelbase.dll 中现成的、由微软维护的合法序言。
* 动态解析： 它利用 Rust 读取这些合法序言的 .pdata 指令。
* 隐匿性： 由于 uwd 使用的“序言”在 .pdata 中有完美记录，EDR的回溯引擎在扫描时会看到一个完全合法的路径，因为它对比的代码和元数据都是 Windows 原生的。

> 总结:序言是函数的身份契约。编译器签发契约，操作系统执行契约。在普通的开发中，它是透明的；但在红队（如你的 puerto 和uwd）中，掌握了序言的构造，就掌握了欺骗操作系统回溯机制的最高权限

## 扩展-Epilog(函数尾声)

函数尾声（Epilog）:撤销序言所做的一切工作，平稳地将控制权交还给调用者

* Epilog的核心任务-精确地执行序言的逆操作
  1. 修复栈指针（Restore RSP）： 通过 add rsp, X释放序言分配的栈空间（包括局部变量和影子空间）
  2. 还原非易失性寄存器（Restore Registers）： 按照 push 的相反顺序执行pop，将寄存器恢复到调用函数之前的状态（如 pop rbp, pop rbx）
  3. 返回（Return）： 执行 ret指令，从栈顶弹出返回地址，并跳转到该地址
```asm
;典型的prolog Epilog

; prolog
push rbp
sub rsp,20h

; 函数主体逻辑

; Epilog
add rsp,20h ; 释放栈
pop rbp     ;还原基址指针
ret         ;返回调用者
```

* Epilog如何产生的

由 编译器后端 自动生成

1. 一个函数通常只有一个序言（入口），但可能有多个尾声（每一个 return语句处都会生成一个尾声）
2. 优化： 现代编译器会进行“尾调用优化”（Tail Call Optimization）。如果函数的最后一步是调用另一个函数，编译器可能会直接跳转（jmp）到那个函数，从而复用当前的尾声

* Epilog如何维护

1. 静态： 编译器生成指令
2. 特殊性： 与序言不同，尾声通常不在 .pdata 中详细定义操作码。 Windows的 x64回溯引擎使用一种名为“指令流模式扫描”的机制来识别尾声。它会检查即将执行的代码是否符合 add rsp, imm; pop reg; ret 这种特定模式?这里需要展开

* 函数尾声的核心特性
1. 镜像对称性： 尾声必须是序言的完美镜像。任何字节的偏差（多加了8，少弹了一个寄存器）都会导致栈失衡（Stack Imbalance），最终引发Access Violation 崩溃
2. 位置敏感： 尾声必须以 ret 或 jmp 结尾
3. 不可中断性： 在高级调试中，如果在尾声中间（比如 add rsp 之后，ret之前）发生异常，回溯引擎会非常头疼，因为它需要判断当前的栈帧到底是属于当前函数还是上层函数

* 在 uwd 项目中的高级应用（至关重要）

uwd 项目中，尾声的逻辑被“解构”并手动实现了

1. 伪造“尾声 Gadget”-uwd 不使用编译器生成的尾声，它在 kernelbase.dll中寻找现成的尾声片段（Gadgets）.如
```rust
// 寻找 `add rsp, 0x58; ret` 这种尾声片段
let (add_rsp_addr, size) = find_gadget(kernelbase, &[0x48, 0x83,0xC4, 0x58, 0xC3], tables)
```
* 目的： 当你的恶意 API（如WinExec）执行完毕返回时，它需要一个“合法的出口”
* 逻辑： uwd 手动构造了一个 ROP 链，将返回地址指向这个合法的 add rsp;ret 片段。这样，当代码运行到这里时，CPU会觉得它正在执行一个合法函数的正常退场过程

2. 栈恢复（Restore PROC）
在 synthetic.asm 中，最后有一个 RestoreSynthetic 过程  

```asm
RestoreSynthetic PROC

mov rsp, rbp
add rsp, 210h
pop r15
pop rbx
pop rbp
ret
RestoreSynthetic ENDP
```
这段负责在伪造调用结束后，把 puerto的原始现场完全还原。如果没有这几行代码，你的程序执行完 spoof! 宏后就会立即崩溃
















## 扩展-栈和栈帧










## 扩展-汇编指令

### call

### ret

### push

### sub

### add

### 20h

### pop


## 扩展-寄存器



### rsp

### rbp

* 全称: 64-bit Base Pointer-基址指针
* 物理意义:通常指向当前函数栈帧的起始位置
* 逻辑意义:为函数内部访问局部变量和参数提供了一个固定参考点
  
* RBP在栈中的位置
RBP 位置的核心在于函数序言（Prolog）中的那两行定义
```asm
push rbp ;压栈
mov  rbp,rsp
```
1. push rbp (保护旧现场):RSP向下移动8字节(减法?对不对),把上一个函数的RBP存入栈顶.
* 物理位置:此时栈顶RSP存放的是旧RBP

2. mov rbp,rsp :执行后RBP的值和当前的RSP完全一样
* 物理位置:RBP现在指向刚才压入的那个旧RBP的存储地址

假设call指令执行前,rsp处于一个完美的16字节对齐边界上(假设为 0x...00)  
1. 执行call:自动压入8字节返回地址,rsp的地址不再是16字节对齐的
2. 执行push rbp:手动压入8字节旧RBP.此时RSP重新回到了16字节对齐
3. 执行mov rbp, rsp : RBP的值现在等于 0x...F0(为啥是F0,往下看)  
图示prolog的push rbp  mov rbp, rsp之后的stack布局:
```text
  ┌──────────────┬───────┬──────────────────┬────────────────────┐
  │ 物理地址（示 │ 偏移  │ 栈内存内容       │ 物理位置描述与逻辑 │
  │ 例）         │ 量    │ (8字节一格)      │ 含义               │
  ├──────────────┼───────┼──────────────────┼────────────────────┤
  │ 0x...20      │ +0x30 │ Parameter 5      │ 调用者压入的第 5   │
  │              │       │                  │ 个参数             │
  │ 0x...18      │ +0x28 │ Shadow Space     │ 预留给寄存器参数的 │
  │              │       │ (for R9)         │ 32 字节空间        │
  │ 0x...10      │ +0x20 │ Shadow Space     │ (影子空间的高端)   │
  │              │       │ (for R8)         │                    │
  │ 0x...08      │ +0x18 │ Shadow Space     │ (影子空间的低端)   │
  │              │       │ (for RDX)        │                    │
  │ 0x...00      │ +0x10 │ Shadow Space     │ 调用者帧的起始点   │
  │              │       │ (for RCX)        │                    │
  │ 0x...F8      │ +0x08 │ Return Address   │ call               │
  │              │       │                  │ 指令压入的返回地址 │
  │ 0x...F0      │ +0x00 │ Saved Old RBP    │ RBP 和 RSP         │
  │              │       │                  │ 共同指向这里！     │
  │ 0x...E8      │ -0x08 │ (未定义/编译器填 │ 尚未执行 sub rsp,  │
  │              │       │ 充)              │ X，此处是“虚空”    │
  └──────────────┴───────┴──────────────────┴────────────────────┘
```

通过这张图，你可以发现几个极其重要的物理真相：

   1. RBP 的物理指向：
      RBP 此时指向的 不是 局部变量，而是
  “上一个函数留下的坐标原点”。即：[rbp] 里面存的值就是 Old RBP。
   2. 返回地址的绝对位置：
      在 Windows x64 中，返回地址 永远 位于 RBP + 8 的物理位置。这是 EDR
  和调试器进行栈回溯（Unwinding）的硬性物理基准。
   3. 16字节对齐的维持：
      你会发现，RBP 指向的地址（0x...F0）本身就是 16字节对齐
  的。这意味着，如果接下来函数要执行 sub rsp, 20h 分配局部变量，新的 RSP
  依然会保持 16 字节对齐。
   4. 参数 5 的物理距离：
      为什么是 +0x30？
      计算：RBP(0) -> Saved RBP(8) -> ReturnAddr(8) -> ShadowSpace(32) =
  48 字节 ($0x30$)。
      所以，想在函数里通过 RBP 拿到第 5 个参数，物理代码必须写成 mov
  rax, [rbp + 30h]。

  ---


  4. 在 uwd 项目中的终极意义

  uwd 项目的核心任务是 “欺骗堆栈验证（Stack Validation）”。


   * 它的伪造逻辑：
      当 uwd 伪造一个帧时，它必须在内存里手动摆放这些数据。
       * 它会在某个地址写下 Old RBP 的值。
       * 它会紧接着在 +8 的位置写下一个合法的 kernel32
         内部地址（伪造返回地址）。
       * 最关键的： 它必须把 RBP 寄存器的值指向它写 Old RBP 的那个地址。


  如果不严谨会怎样？
  如果 uwd 把 RBP 指向了 Saved RBP 的上方或下方 8 字节，那么当 Windows
  尝试执行异常处理或 EDR 扫描时，RBP + 8
  拿到的就不是返回地址，而是影子空间的乱码。系统会立即判定为 “Stack
  Corruption”，直接触发蓝屏或防御报警。


  结论：
  RBP 就是 “栈帧的脊椎”。push rbp; mov rbp, rsp 之后，RBP
  锁定了当前的对齐状态，并为向上访问参数、向下访问变量建立了一个
  绝对物理原点。

* 核心作用:
x86下,RBP强制使用.x64下,rbp是通用寄存器,但其核心作用依然不可替代:  
1. 访问局部变量:不管RSP因为压栈操作变到了哪里,局部变量相对于RBP的偏移永远不变
  * 如`[rbp-0x10]`永远是第一个局部变量.?
2. 访问函数参数:
  * 对于超过4个参数的函数,需要通过栈传递函数的参数
  * 如`[rbp+0x30]`可能是第5个参数.?
3. RBP向上往高地址走(加法)有返回地址\参数等由调用者准备的内容;向下往低地址走(减法),进入当前函数新开辟的空间.这里的局部变量是当前函数序言继续执行sub rsp, x之后产生的
4. 栈回溯 (Stack Walking)：
      这是 uwd 项目最关心的功能。经典的栈回溯是通过 RBP 链实现的：
       * 当前的 RBP 指向旧的 RBP。
       * 旧的 RBP 又指向更旧的 RBP。
       * 这形成了一个清晰的链条，调试器只需顺着 RBP就能还原出整个调用历史。
       * 这些旧的RBP都是在栈上一直保存着的吗?不然为啥能读取旧的RBP内容?



  1. RBP 的关键特性


  A. 非易失性 (Non-volatile)
  在 Windows x64 调用约定中，RBP 是被调用者保护 (Callee-saved) 的。
   * 这意味着：如果一个函数想用 RBP，它必须在序言里先 push
     rbp，在尾声里再 pop rbp。必须保证函数返回时，RBP
     的值和进来时一模一样。


  B. 可选性 (Frame Pointer Omission - FPO)
  现代编译器（如 Rust 的 rustc 或
  MSVC）非常聪明。它们发现：既然我能精确计算出 RSP
  的每一次变动，那我就不需要 RBP 了，直接用 [rsp + offset]
  访问变量就行。
   * 优点： 多出一个通用寄存器供计算使用，提高性能。
   * 后果： 导致“栈回溯”变难了。这也是为什么 Windows 必须引入 .pdata
     表的原因——因为编译器不保证有 RBP
     链，系统必须通过元数据来找回调用者。


  C. 对齐缓冲
  在序言中，mov rbp, rsp 之后，即使 RSP 之后为了对齐又减了 8 字节，RBP
  依然保持着原始进入函数时的位置参考。

  ---

  4. 在 uwd 项目中的灵魂地位


  你正在研究的 uwd 为什么要死磕 RBP？请看 uwd.rs 里的这行代码：


   1 fn find_push_rbp(module_base: *mut c_void, runtime_table:
     &[IMAGE_RUNTIME_FUNCTION]) -> Option<Self> {
   2     // 它在寻找那些手动 PUSH RBP 的函数
   3 }


  为什么 uwd 喜欢带 RBP 的函数？
   1. 稳定性： 带有 push rbp 指令的函数，其 Unwind
      机制通常更标准、更易于模拟。
   2. 欺骗 EDR： 很多高级 EDR 的扫描引擎如果看到栈回溯中缺少 RBP
      链，可能会触发启发式报警。uwd 通过模拟一个带有 RBP
      的合法帧，让栈看起来极其“自然”。
   3. 精确计算： uwd 需要知道
      rbp_offset。因为它要在伪造的栈空间里，在确切的偏移位置填入一个虚假
      的返回地址或上一级 RBP 的值。

  ---

  5. RBP 与 RSP 的爱恨情仇


   * RSP 是“动态”的： 随着 push, pop, sub 不断跳动。它是 CPU
     执行的物理边界。
   * RBP 是“静态”的：
     在一个函数生命周期内通常保持不动。它是程序员理解代码逻辑的逻辑边界
     。


  总结给您的技术视角：


  作为一个红队开发者：
   1. 当你看到 push rbp; mov rbp,
      rsp，你就知道进入了一个“有规矩”的函数，它是你伪造栈的最佳模版。
   2. 当你用 Rust 写 unsafe 汇编时，记住 RBP 是你的锚点。
   3. 如果你想让你的 puerto 隐身，你就得像 uwd 那样，学会如何在没有物理
      RBP 链的情况下，通过修改内核对象手动构建出一套让系统信以为真的逻辑
      RBP 链。


  RBP
  不仅仅是一个存储地址的容器，它是程序执行流的“史书”。掌握了它，你就掌握
  了重写历史的能力。
### rip






















