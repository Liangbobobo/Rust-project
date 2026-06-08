# Gadget.md

是对samoa/src/gadget.rs中注释的补充

## const JMP_GADGET

其内部字段用于主线程休眠期间,构建ROP链（ VirtualProtect  ->Encrypt  ->  NtDelayExecution  ->  Decrypt  ->  VirtualProtect）时，在各个 API 调用转折过渡阶段使用,用于进入下一个api

以调用VirtualProtect为例:
1. 在 OS 模块中搜寻 Gadget:在find()中遍历kernelbase.dll的.text节,通过memchr::memmem::find找到jmp r11的机器码(0x41, 0xFF, 0xE3)的地址.将地址和r11写入Gadget结构体
2. 在构建栈帧和跳转时使用:在伪造栈并准备调用VirtualProtect 时,将VirtualProtect  所需的参数放进  rcx ,  rdx ,  r8 ,  r9(win fastcall).然后将VirtualProtect的实际函数入口地址移入r11.
3. 安全跳转:不执行call r11(会在栈顶留下不合法的返回地址).而是把返回地址预置在栈上(设置为`AddRspXGadget` 的地址，即 `add rsp, X; ret`).然后执行jmp r11(从kernelbase.dll找到的gadget,隐匿调用源)
4. JMP_GADGET独独没有用rbx:rbx用于最后控制流收尾时,安全返回恶意程序的唯一通道.
以上,在edr视角,当敏感 API 正在执行并触发栈回溯（Stack Walk）时，检测引擎读取到的栈顶返回地址是 `AddRspXGadget`，这属于系统DLL（如 kernelbase.dll）内部合法的非叶子函数，且其前方的指令确实是合法的 `call`（绕过了Call-preceding 检查），从而认定该 API 调用历史是完全合法的，放行拦截



## 注释1-栈展开

### 栈展开（Unwind）的运作真相：

  当 EDR 或者是系统的  RtlVirtualUnwind  开始回溯栈时，它的数学计算过程如下：

    [步骤 1] EDR 看到栈顶的返回地址指向：ntdll!RtlpSearchExceptionHandlers +
  0x120
               ↓
    [步骤 2] EDR 去查询系统注册的异常表（.
  pdata），找到了这个函数的官方注册项。
               ↓
    [步骤 3] EDR 读取该函数真实的 UNWIND_INFO，上面写着官方数据：
             "该函数在进入时，会执行 `sub rsp, 0x28`，即开辟 0x28
  字节栈空间。"
               ↓
    [步骤 4] 关键的一步！EDR 的退栈计算：
             "为了找到上一个调用者，我需要让当前的 RSP 寄存器加上 0x28
  字节。"
             计算：新的 RSP = 原 RSP + 0x28
               ↓
    [步骤 5] EDR 去读取 [新的 RSP] 地址处的值，作为上一个调用者的返回地址。

  #### 我们的"欺骗"是如何配合的？

  正因为我们在代码里通过  scan_runtime  事先知道了这个函数开辟的栈空间是
  0x28 （40）字节：

  • 我们在内存栈上，手动塞入了 40 字节的垃圾数据（对齐垫片）。
  • 在这 40 字节的末尾，我们精准地填入了下一个伪造的返回地址（比如指向
  kernel32!BaseThreadInitThunk ）

## region.as_ref().as_ptr() as usize 

**在 Windows进程的虚拟内存空间中，一个指向内存数据的“指针”，它的数值本身就是那个数据的“绝对虚拟地址（VA）**

1. 物理现实：什么是指针.当你调用 region.as_ref().as_ptr() 时，Rust 实际上是去内存里读取了一个寄存器或栈上的数值
    * 这个数值是一个 64 位的整数（比如 0x00007FF8B9E11000）
    * 这个地址通过 CPU 的 MMU (内存管理单元) 配合 CR3 寄存器指向的页表，最终被翻译为物理内存。对于用户态程序来说，这个数值就是它能感知的“唯一、绝对”的坐标
2. 这里的“绝对”地址到底指什么:它是在当前进程的 43/47/57位虚拟地址空间中分配的一个逻辑位置.范围通常是0x0000_0000_0000_0000 到 0x0000_7FFF_FFFF_FFFF (47位共 128TB);
    * 虽然它是“虚拟”的，但相对于进程内部的所有操作，它就是该字节唯一的绝对的寻址坐标。一旦 CPU 的 CR3寄存器加载了当前进程的页表基址，这个 VA就具备了定位到物理内存的唯一能力

**为什么 as_ptr() as usize 就能得到 VA**
1. as_ptr() 的本质： 它返回的是一个原始指针。在 x64 Windows下，这个指针在 CPU 寄存器（如 RAX）或内存中，物理上存储的就是一个64 位的无符号整数
2.  这个 64位整数的值，物理上完全等同于目标内存单元的虚拟地址 (VA)
3.  在 Rust 中，指针类型不允许直接进行加减数学运算.as usize 是一次非破坏性的位解释（Bit-wise cast）。它没有对数据进行任何偏移或转换，只是告诉编译器：“请允许我把这个原本只能用来解引用（dereference）的地址值，当作一个普通的 64 位数字来参与算术运算。”

**RVA VA**

* RVA (Relative Virtual Address)：
    * 它是静态的。它是 PE 头部定义的偏移值。
    * 代表了：“如果模块加载到地址 X，那么这个节就在 X + RVA 处”。
* VA (Virtual Address)：
    * 它是动态的（受 ASLR 影响）。
    * 它是：ImageBase (当前模块的真实加载起始点) + RVA。
* as_ptr() 的返回值核实：
    * 结论：它返回的绝对是 VA。
    * 理由： as_ptr() 捕获的是程序运行时的真实状态。既然 .text 节已经被加载到内存，那么它的指针必然指向那个经过 ASLR偏移后的、真实的内存位置。


## pub fn get_text_section

win64下,内存以页为单位.PE文件加载进内存的时候,Windows的加载器ldr会根据节表定义,给每个页加上不同的执行权限(R/W/X)

**硬件级的防御：DEP (数据执行保护)**

如果 CPU 尝试去执行一块标记为 RW 但没有 X属性的内存地址，硬件会立即触发一个 访问违规 (Access Violation)异常，系统直接强行关掉你的程序

因此, Gadget（如 jmp r11）必须去 .text 节找

**如何区分源码并放入不同的节表**
1. 编译器根据语法定义给每行代码加上的不同的逻辑标签(CODE/DATA/CONST)
2. 编译器生成.obj文件,obj文件是多个逻辑零件包.链接器Linker把多个obj文件放入不同的节中(CODE->.text;DATA->.data/.bss;CONST->.rdata)
3. 打包生成pe文件结构:链接器生成pe文件头部,并在其中写下节表
4. os加载器ldr读取pe头部.如.text调用NtMapViewOfSection,分配内存,使用mmu把这块内存设为只允许执行.cpu的dep就有了依据.如果在.text之外执行代码,会抛出异常




**.text节特性**
1. 安全性：防止由于缓冲区溢出（Buffer Overflow）导致的恶意代码执行
2. 效率：CPU 有专门的 “指令缓存 (I-Cache)”。将所有的指令集中放在 .text段，能极大地提高 CPU 读取指令的速度
3. 不可修改性：.text段通常是只读的。这意味着病毒很难在不引起报警的情况下，直接修改已加载DLL 的函数代码