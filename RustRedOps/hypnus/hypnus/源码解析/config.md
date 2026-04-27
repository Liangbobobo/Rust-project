- [Config Struct](#config-struct)
  - [StackSpoof](#stackspoof)
  - [Config::new()](#confignew)
    - [LoadLibraryA](#loadlibrarya)
  - [Config::alloc\_callback()](#configalloc_callback)
  - [Thread Pool](#thread-pool)
  - [NtContinue](#ntcontinue)
    - [TpAllocTimer TpAllocWait NtContinue](#tpalloctimer-tpallocwait-ntcontinue)



## alloc_callback函数

在 hypnus 项目中，alloc_callback函数展示了低功耗免杀（RedOps）中极其核心的一个技术：动态代码生成与内存跳板（Trampoline）。

其实质是将字节数组写入内存并赋予执行权限的操作，本质上是在手动构建一个微型函数。


一、 变量与流程深度解析

1. 机器码数组 (callback)
```rust
let callback = &[
  0x48, 0x89, 0xD1,       // mov rcx, rdx
  0x48, 0x8B, 0x41, 0x78, // mov rax, [rcx + 0x78]
  0xFF, 0xE0,             // jmp rax
];
```

* 这些字节是 x86_64 的机器码
* mov rcx, rdx: 在 Windows x64 调用约定中，rcx 是第 1 个参数，rdx是第 2 个。线程池回调（如 TpAllocTimer）会将你传入的 Context指针放在 rdx 中。这行指令将其移动到 rcx。
* mov rax, [rcx + 0x78]: 在 Windo的存储位置。这行代码从结构体中取出预设的目标地址。
* jmp rax: 直接跳转。注意：它没有 ret指令，因为它是一个跳板，目的是移交执行权
  * ret一般与call配合使用,ret执行 rip=rsp;rsp=rsp+8.这两个原子动作,且会同步读取硬件影子栈并比对.即ret从rsp原子级弹出地址至rip并受cet一致性校验的栈依赖型返回指令
  * jmp直接或间接重定向rip且不会触发rsp/cet的栈无关的转移指令.但其间接跳转受CFG约束

2. 内存分配 (NtAllocateVirtualMemory)

```rust
 if !NT_SUCCESS(NtAllocateVirtualMemory(
            NtCurrentProcess(), 
            &mut addr, 
            0, 
            &mut size, 
            MEM_COMMIT | MEM_RESERVE, 
            PAGE_READWRITE
        )) {
            bail!(s!("failed to allocate stack memory"));
        }
```
* NtCurrentProcess(): 一个伪句柄（值为 -1），代表当前进程。表示这块内存分配给当前进程
* addr: 初始化为 null_mut(),表示不在乎具体内存地址,只需要一个空闲位置。内核执行后，它会存储分配到的内存的起始地址。在本文件中,该地址随后被存放9字节的trampoline机器码
* 0:该字段用于指定分配地址的对齐要求.通常设为0,表示遵循os默认页面对齐(一般64k)不是4k吗?
  * page size总是4kb,即页面大小是内存的物理单位和保护单位.虽然只申请9字节,size参数会被内核向上取整为4kb的倍数.
  * 当给NtAllocateVirtualMemory 的addr参数传0,让os自选地址时,os返回的地址一定是64kb的倍数.这是为了减少vad树碎片化.这个函数本质上在vad上注册,之后mmu才会允许后续的写入和执行
  * vad记录虚拟地址开始/解释/权限/私有/映射等信息.win规定,申请新内存时,其起始地址必须在64k的格子中.即vad中内存地址必须是64k的,但其代表的物理页面可以是4k的.且vad只代表有这么一个虚拟内存,当尝试(代码行为)访问这个位置时,cpu会触发page fault,内核从物理内存池中找到一个4k的页面.内核修改PTE将虚拟地址指向找到的这个物理页面.即PTE才是真正记录虚拟4k对应物理4k的地方.vad负责证明合法拥有该地址.edr一定会监测vad树. 
  * 64k是vad树分配地址的起始对齐单位,vad记录该区域的属性和边界;4K 物理管理 是由页表（PTE）在 CPU 访问内存时动态完成的
* size: 传入时是 9 字节，但内核会按页面（Page，通常是4KB）对齐，实际分配 4096 字节。
* MEM_COMMIT | MEM_RESERVE: 既预留虚拟空间又分配物理内存。
* PAGE_READWRITE (RW): 关键点。在写入代码时，内存必须是可写的。这里绝不能直接申请PAGE_EXECUTE_READWRITE(RWX).现在EDR会严格监控带有执行权限的内存分配.文件中先申请RW,再写入机器码,再通过NtProjectVirtualMemory改为RX(READ/Write)
  * 相对可分配内存的VirtualAlloc(位于kernel.dll),NtAllocateVirtualMemory 位于 ntdll.dll，更接近内核,不易被EDR设置监控.且本项目使用hash值(NtAllocateVirtualMemory)动态解析其地址,IAT中没有该函数
  * 这里的分配的每一字节虚拟地址,物理都必须经过cpu的mmu(内存管理单元)进行地址翻译.在NtAllocateVirtualMemory 中设置的PAGE_READWRITE,最终会变成第四级页表(PTE)中的一个R/W位.mmu如何索引到该页面?
  * 软件层面,win内核使用VAD管理进程内存.当调用NtAllocateVirtualMemory 时,内核在vad树中增加一个节点(EDR会定期遍历该树,寻找孤儿内存(无对应磁盘文件/具有执行权限)).hypnus最初申请rw权限,此时在vad中就像是普通数据,不触发警报.再写入机器码后,才反转为rx

  1. 写入与权限转换 (NtProtectVirtualMemory)
   * copy_nonoverlapping: 将那 9 个字节的“灵魂”拷贝进刚申请的空白页。
   * PAGE_EXECUTE_READ (RX):
     写入完成后，通过此函数将权限从“可读写”改为“可读可执行”。
       * 安全意义: 这是为了绕过 DEP
         (数据执行保护)。如果你直接在普通堆内存里运行代码，CPU
         会报异常。这种“先写后改权限”的模式遵循了 W^X (不可同时写和执行)
         的安全原则。

  2. 锁定内存 (NtLockVirtualMemory)
   * VM_LOCK_1: 将这块页锁定在物理内存中。
   * 意义: 防止系统的内存管理器为了节省空间把这块代码置换（Page
     out）到硬盘的 swap
     文件中。在免杀中，这能防止因硬盘取证被抓到特征，也能保证高频跳板的
     性能。

  ---

  二、 这种形式与“一般函数调用”有何不同？

  这是理解 hypnus 原理的关键：


  ┌──────┬────────────────┬───────────────────────────────────────┐
  │ 特性 │ 一般函数调用   │ 动态跳板 (Dynamic Trampoline)         │
  │      │ (Static        │                                       │
  │      │ Function)      │                                       │
  ├──────┼────────────────┼───────────────────────────────────────┤
  │ 存在 │ 编译在可执行文 │ 运行时动态申请，存在于私有堆/匿名页中 │
  │ 位置 │ 件（PE）的     │ 。                                    │
  │      │ .text 段中。   │                                       │
  │ 可见 │ 杀软和 EDR     │ 静态扫描不可见，只存在于内存中，且可  │
  │ 性   │ 能轻易扫描文件 │ 以随时销毁。                          │
  │      │ 的静态特征。   │                                       │
  │ 调用 │ 通过 call      │ 它是作为回调函数被内核线程池调用的，  │
  │ 方式 │ 指令，会在栈上 │ 没有传统的 call 链。                  │
  │      │ 压入返回地址。 │                                       │
  │ 控制 │ 逻辑是固定的。 │ 这里它通过 jmp rax                    │
  │ 流   │                │ 实现了动态跳转，目标地址在运行时根据  │
  │      │                │ CONTEXT 改变。                        │
  │ 栈特 │ 拥有完整的符号 │ 栈欺骗的核心。当 EDR                  │
  │ 征   │ 和回溯路径。   │ 检查栈时，它看到的是系统线程（如      │
  │      │                │ tpworker.dll）在执行代码，起点是合法  │
  │      │                │ 的系统地址，而不是你的恶意模块。      │
  └──────┴────────────────┴───────────────────────────────────────┘


  三、 变量深度总结

  在这个代码路径下，callback 就像是一个“临时演员”。
   1. 你向系统申请了一个名为 addr 的舞台。
   2. 把剧本 callback 抄在舞台地板上。
   3. 把剧场权限从“装修（写）”改成“演出（执行）”。
   4. 为了防止剧场被强拆，你用了 NtLock 锁门。

  为什么要这么大费周章？
  因为在
  hypnus（希普诺斯，睡眠之神）中，它的目标是在木马休眠时，将真正的恶意代
  码隐藏起来。这个跳板是唯一暴露在外面的“呼吸孔”，通过它，线程池可以在休
  眠结束时，精准地跳转回被加密/混淆后的主逻辑，而 EDR
  在扫描栈时只会看到一个极其简单的、看起来像系统组件调用的跳转逻辑。

 > 我是说这种在内存中直接将字节流当作指令的形式我不熟悉,请你深入帮我讲解一下
▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
✦ 这种“将字节流（Byte
  Stream）视为指令并执行”的技术，是底层开发、编译器设计以及
  RedOps（红队攻击技术） 的核心基石。

  要理解这个过程，我们需要打破高级语言（如
  Rust、C++）给我们的“函数”抽象，深入到 CPU 和操作系统的视角。

  ---

  1. 核心本质：CPU 眼中没有“数据”和“代码”的区分
  在 CPU 看来，内存里只有 0 和 1。
   * 如果 CPU 的 RIP（指令指针寄存器）
     指向一个地址，它就会尝试把那里的字节解释为操作码（Opcode）并执行。
   * 示例：字节 0x90。
       * 如果它在一个字符串里，它就是一个普通的数字 144。
       * 如果 RIP 指向它，CPU 就会执行 NOP（无操作）指令。

  结论：所谓的“代码”，其实就是排好序的数据字节。

  ---

  2. 现代操作系统的屏障：DEP（数据执行保护）
  虽然 CPU 不分数据和代码，但操作系统（Windows）为了安全，必须分。
   * 内存页属性：操作系统通过 CPU 的 MMU（内存管理单元）
     给每一块内存打上标签：
       * RW (Read/Write)：普通变量、堆栈所在地。禁止执行。
       * RX (Read/Execute)：程序代码所在地。禁止写入。
   * NX 位（No-Execute）：如果 RIP
     尝试跳转到一个标记为“不可执行”的内存页，CPU 会立刻触发
     访问冲突异常（Access Violation） 并强制关闭程序。

  这就是为什么你在代码中看到了三步走：申请内存 (RW) -> 写入字节 ->
  修改属性 (RX)。

  ---

  3. “灵魂转世”：从 u8 数组到可执行函数
  让我们通过 hypnus 的这个例子看看这个“转世”过程：

  第一步：定义灵魂（机器码）

   1 let callback = &[0x48, 0x89, 0xD1, ...]; // 这些是硬编码的机器指令
  这时，这些指令只是 Rust 程序数据段里的一些普通字节。它们还在“凡间”。

  第二步：寻找肉身（分配内存）

   1 NtAllocateVirtualMemory(..., PAGE_READWRITE);
  你向内核要了一个新的、干净的内存页。此时它是 数据页 (RW)。

  第三步：刻入记忆（拷贝）
   1 core::ptr::copy_nonoverlapping(callback.as_ptr(), addr, ...);
  你把那些字节搬运到了新内存里。

  第四步：点火启动（权限翻转）
   1 NtProtectVirtualMemory(..., PAGE_EXECUTE_READ);
  这是最神奇的一步。你通过系统调用告诉内核：“请允许 CPU
  执行这块内存里的内容”。此时，原本是“数据”的字节，在系统层面正式变成了“
  代码”。

  ---

  4. 它是如何被调用的？
  在 Rust 层面，你拿到的 addr 只是一个 u64。但你可以通过
  类型转换（Casting） 强行把它变成一个函数指针：

   1 // 伪代码：将内存地址转换为 Rust 的函数签名
   2 let func: fn(u64, u64, u64) = unsafe { core::mem::transmute(addr)
     };
   3 func(1, 2, 3); // 现在你可以像调用普通函数一样调用它了！
  当你调用 func 时，CPU 会执行一条 CALL 指令，将 RIP 跳转到 addr
  指向的位置。

  ---

  5. 为什么要手动写字节，而不是直接写 Rust 函数？
  这是 hypnus 最精妙的地方。

   1. 位置无关性 (Position Independent Code, PIC):
      普通的 Rust
  函数在编译后，其内部跳转通常是相对地址。如果你把一个普通的 Rust
  函数直接拷贝到新内存，它引用的全局变量或外部函数地址会全部失效（因为基
  址变了）。而这几行手写的汇编字节是完全自包含的，搬到哪都能跑。

   2. 逃避监控 (Evasion):
      普通的 Rust 函数会在程序的 IAT（导入表） 或 符号表
  中留下痕迹。而这种动态生成的代码在文件静态扫描时完全不存在，只在内存中
  昙花一现。

   3. 精准控制寄存器:
      hypnus 需要在线程池回调中精确接管 RCX 和 RDX
  寄存器。高级语言（Rust）的编译器为了保证安全和堆栈对齐，会在函数开头自
  动插入大量的 push, sub rsp 等指令（称为
  Prologue）。手动写字节可以绕过编译器的自作聪明，直接实现最纯粹的寄存器
  操作。

  总结
  你看到的这种方式，实际上是手动模拟了编译器的最后一步工作。
   * 编译器：源代码 -> LLVM -> 汇编 -> 机器码字节 -> 写入 .exe 文件。
   * 你：机器码字节 -> 写入内存页 -> 运行。

  这种技术在 RedOps 中被称为 “内存加载（In-Memory Loading）” 或
  “动态跳板生成”。它是实现高级免杀（如堆栈欺骗、睡眠混淆）的必经之路。
▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀
 > 这种方式还有栈展开的吗?
▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
✦ 这是一个非常深入且极具技术含量的后续问题。

  简单直接的回答是：默认情况下，这种方式完全没有“栈展开（Stack
  Unwinding）”支持。

  为了让你理解这意味着什么，我们需要从 Windows x64 的异常处理机制和
  hypnus 的设计初衷两个角度来解析。

  ---

  1. 为什么默认没有栈展开？
  在现代 Windows (x64) 中，当发生异常或调用
  RtlCaptureStackBackTrace（获取调用栈）时，系统需要知道如何从当前函数回
  溯到上一个函数。

   * 编译后的函数：编译器在生成代码时，会在 PE 文件的 .pdata
     段中生成一个异常表（Runtime Function
     Table）。这个表记录了每个函数的起始地址、结束地址，以及最重要的——Un
     wind
     Info（展开信息）。它告诉操作系统：如果在这个函数里出事了，你应该如
     何恢复 RSP 和 RBP 寄存器以找到调用者。
   * 动态生成的字节流：你手动写入内存的这 9
     个字节，在操作系统的“花名册”（.pdata 段）里完全没有记录。
       * 后果：如果你在这段代码里发生了访问违规（Crash），或者你在这里尝
         试抛出一个 Rust panic!，Windows 的异常处理器会发现 RIP
         指向一个没有对应 Unwind Info 的地址。
       * 结果：系统会认为调用栈已损坏，直接触发 深度防御（Fail-fast）
         或蓝屏，你连调试的机会都没有，因为调用栈回溯到这里就“断”了。

  ---

  2. 为什么 hypnus 不在乎（甚至故意为之）？
  在 hypnus 的这段代码中，这种“断裂”的栈特征反而是它的核心武器。

  A. 它是一个“叶子函数（Leaf Function）”
  观察这段代码：

   1 mov rcx, rdx
   2 mov rax, [rcx+0x78]
   3 jmp rax
  它没有执行 push rbp，没有 sub rsp, XX。这意味着它没有改变栈指针。在
  x64 规范中，这种不修改栈指针且不调用其他函数的代码被称为“叶子函数”。
   * 对于叶子函数，即使没有 Unwind Info，回溯器有时也能通过 RSP
     指向的地址强行找到上一层。

  B. 栈欺骗（Stack Spoofing）的需要
  hypnus 的目的是免杀。EDR（如
  CrowdStrike）最强大的武器之一就是栈扫描（Stack Scanning）。
   * 如果你的木马在 Sleep 时，EDR
     扫描你的线程栈，发现调用链是：ntdll.dll -> 你的木马.exe (匿名内存)
     -> kernel32.dll，它会立刻判定你是非法注入的 Shellcode。
   * 这种动态跳板的作用：它通过一个不留痕迹的跳转，让 EDR
     的自动回溯工具在这里产生混淆。因为这个地址不在任何已知的模块（DLL/E
     XE）范围内，回溯算法往往会在这里停止或报错。

  ---

  3. 如果我真的想要栈展开，该怎么办？
  在底层开发中，如果你动态生成了复杂的、需要支持异常处理的代码，你必须手
  动向操作系统“注册”这个函数。

  在 Windows 上，你需要使用以下系统 API：

   1 // 伪代码：向系统注册动态生成的代码块
   2 RtlAddFunctionTable(
   3     FunctionTable: *mut RUNTIME_FUNCTION,
   4     EntryCount: u32,
   5     BaseAddress: u64
   6 );
  你需要手动构造一个 RUNTIME_FUNCTION
  结构体，详细描述你那段代码的栈行为。这样，当 panic! 发生时，Rust 的
  std 库才能顺着这个表找到上一层。

  ---

  4. 总结：这种方式与 Rust 开发的冲突
   * 在普通的 Rust 开发中：这种方式极度危险，因为它破坏了 Rust
     的安全保障（Panic
     Safety）。一旦出问题，程序会死得非常难看，没有任何堆栈信息。
   * 在 RedOps（红队）开发中：这种方式是艺术。你通过手动控制 jmp 而非
     call，刻意避开了编译器生成的栈帧信息。

  一句话总结：
  这种动态字节流没有自动的栈展开能力。它是一个孤立的、没有身份证明的代码
  孤岛。在 hypnus 中，这种“孤立性”正是用来躲避 EDR 栈回溯追踪的防护盾。
▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀
 > 深入全面的分析这种函数和普通函数的区别.
   深入全面的分析这种函数和普通函数的区别
▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
✦ 这是一次关于计算机底层运行机制的深度博弈。我们将从生命周期、内存布局、
  执行上下文、元数据特征、以及安全防御五个核心维度，对这两种“代码存在形
  式”进行全方位的对比分析。

  ---

  一、 根本属性：身份与生命周期 (Identity & Lifecycle)


  ┌────┬───────────────────────────────┬───────────────────────────┐
  │ 维 │ 普通函数 (Ordinary Function)  │ 动态字节流函数 (Dynamic   │
  │ 度 │                               │ Trampoline)               │
  ├────┼───────────────────────────────┼───────────────────────────┤
  │ 定 │ 编译期（Compile-time）。在代  │ 运行时（Runtime）。程序运 │
  │ 义 │ 码还没运行前，逻辑就已经固定  │ 行到一半，根据需要临时在  │
  │ 时 │ 并写死在硬盘的文件里。        │ 内存里“捏”出来的逻辑。    │
  │ 机 │                               │                           │
  │ 身 │ 有名（Named）。在符号表中有名 │ 匿名（Anonymous）。它只是 │
  │ 份 │ 字，有明确的作用域（Scope）， │ 内存中的一个起始地址，像  │
  │ 标 │ 属于某个模块（DLL/EXE）。     │ 一个没有户口的“黑户”。    │
  │ 识 │                               │                           │
  │ 加 │ OS 加载器                     │ 你（开发者）。通过        │
  │ 载 │ (ntdll.dll)。系统在启动程序时 │ NtAllocateVirtualMemory   │
  │ 者 │ ，自动将它从硬盘映射到内存。  │ 手动申请内存并充当搬运工  │
  │    │                               │ 。                        │
  └────┴───────────────────────────────┴───────────────────────────┘

  ---

  二、 内存居所：物理与逻辑布局 (Memory Residency)

   * 普通函数：驻留在 PE 映像段中
       * 它位于文件的 .text（或 .code）段。
       * 这个段被标记为 MEM_IMAGE，表示它与磁盘上的某个文件有对应关系。
       * 特征：内存地址相对固定（考虑 ASLR 后也是基于基址偏移）。
   * 动态字节流：驻留在私有堆/匿名页中
       * 它位于 MEM_PRIVATE 类型的内存中。
       * 特征：它在内存中是“孤立”的。如果你扫描进程内存，普通函数是在一
         排整齐的宿舍楼里，而动态字节流就像是在荒郊野外临时搭的一个帐篷
         。

  ---

  三、 执行机制：ABI 与“仪式感” (Execution Mechanics)

  这是两者的技术分水岭：

  1. 序言与收尾 (Prologue & Epilogue)
   * 普通函数：由 Rust/C++ 编译器自动生成大量“仪式代码”。
       * push rbp; mov rbp, rsp (保存栈帧)
       * sub rsp, 0x40 (开辟局部变量空间)
       * ret (平栈并返回)
   * 动态字节流：Naked（裸奔）。
       * 正如你在 hypnus 中看到的，它没有 push/pop，甚至没有
         ret。它直接用
         jmp。它不需要维护自己的栈帧，它只是在借用别人的栈。

  2. 调用约定 (Calling Convention)
   * 普通函数：严格遵守 Windows x64 ABI。参数必须进 RCX, RDX, R8, R9。
   * 动态字节流：它可以肆意操纵调用约定。
       * 在 hypnus 的例子中，它就是为了解决调用约定不匹配：线程池给的是
         RDX，但后续逻辑想要
         RCX。它像一个中间适配器，强制修改寄存器后再跳转。

  ---

  四、 元数据与可观测性 (Metadata & Observability)

  这是调试和防御时最关键的区别：

  1. 异常处理与栈展开 (Unwind Info)
   * 普通函数：在 PE 文件中有 .pdata 记录。
       * 作用：当发生异常或获取调用栈时，系统查表就知道怎么回溯。它是“可
         溯源”的。
   * 动态字节流：断头路。
       * 系统中没有它的任何记录。如果在这里崩溃，所有的调试器都会显示
         [Unknown] 或 No symbols。它是一个回溯陷阱。

  2. 调试符号 (Debug Symbols)
   * 普通函数：有 PDB 文件支持，能看到文件名、行号、变量名。
   * 动态字节流：只有原始的十六进制字节。

  ---

  五、 安全防御：红队视角 (Security & Stealth)

  在 RedOps（红队）开发中，动态字节流之所以优越，是因为它能完美对抗现代
  EDR：

   1. 静态扫描 (Static Scan)
       * 普通函数：杀软扫描硬盘上的 .exe 就能发现恶意逻辑。
       * 动态字节流：完全隐身。硬盘上只有一串看起来像乱码的数据，只有运
         行起来后，它才在内存中变成指令。

   2. IAT/EAT 监控
       * 普通函数：调用外部 API
         必须通过导入地址表（IAT）。杀软可以挂钩（Hook）IAT。
       * 动态字节流：可以直接通过硬编码的 syscall
         或者动态计算的函数指针来调用，不留下任何导入表痕迹。

   3. 栈校验 (Stack Walking)
       * 普通函数调用链：ModuleA -> ModuleB ->
         ModuleC。每一层都在文件系统中找得到主。
       * 动态字节流带来的欺骗：它创造了一个“断裂点”。当 EDR
         扫描栈时，它会发现一个执行点不在任何已加载模块中。
           * 注意：现代顶级 EDR（如 CrowdStrike）会专门标记这种
             “Module-less Code Execution” 行为。这也是为什么 hypnus
             还需要 NtLockVirtualMemory 和其他的混淆手段。

  ---

  总结比较表


  ┌────────────┬──────────────────┬─────────────────────────────────┐
  │ 特性       │ 普通函数         │ 动态字节流函数                  │
  ├────────────┼──────────────────┼─────────────────────────────────┤
  │ 存储介质   │ 磁盘文件 ->      │ 内存堆 -> 动态写入              │
  │            │ 内存映射         │                                 │
  │ 内存属性   │ MEM_IMAGE        │ MEM_PRIVATE                     │
  │ 代码生成   │ 编译器 (Offline) │ 开发者/JIT (Online)             │
  │ 符号支持   │ 完整支持 (PDB)   │ 无                              │
  │ 栈展开信息 │ 记录在 .pdata    │ 无（除非手动注册）              │
  │ ABI 遵循   │ 严格             │ 可选/破坏                       │
  │ 主要应用   │ 正常业务逻辑     │ Shellcode, 跳板, JIT 引擎, 免杀 │
  │            │                  │ Payload                         │
  └────────────┴──────────────────┴─────────────────────────────────┘


  深度见解：
  在 hypnus 这个项目中，你看到的这个 u8 数组不是普通的逻辑，它是对 CPU
  指令集的直接“点穴”。普通函数是“按规矩办事”的平民，而这种动态字节流是“
  不留痕迹”的特种兵。它牺牲了可维护性、兼容性和稳定性（一旦出错就蓝屏）
  ，换取了极致的隐蔽性和对寄存器的绝对控制权。


# Config Struct

> Config 充当了项目的“环境确定性引擎”，它通过预先解析 API地址、测量系统栈帧元数据并分配执行跳板，将不可控的操作系统环境固化为一套精确的物理坐标字典，为后续异步混淆链提供绝对可靠的跳转依据

Config是静态系统环境快照  
CONTEXT是动态执行时的系统环境

在 hypnus 中，Config 和 CONTEXT 字段的选择，本质上是“攻击链路对系统原语的需求” 与 “CPU 执行现场的寄存器约束”之间的精确映射

构造 Config 时，字段选择遵循 “执行流链式劫持”的最小路径原则。它只保留那些能让 Payload “合法存在并自主切换”的字段。  
这 22 个字段并不是随手抓来的，而是一个红队开发者在Windows 内核中反复试错后，筛选出的“执行流操纵最小集合


类型是:`Once<Config>`:
1. 在异步环境下(线程池或纤程),spin::Once确保Config只会被初始化以此,且没有互斥锁的开销
2. 通过init_config(),在真正需要混淆时才去扫描内存,解析hash.降低消耗和敏感性

```rust
#[derive(Default, Debug, Clone, Copy)]
pub struct Config {
  // 存储伪造系统函数（如RtlUserThreadStart）所需的全部栈帧尺寸.在后续的 CONTEXT.Rsp 中用于制作栈大小,并在栈上填入各个假的返回地址.让edr扫描时得到伪造好的调用链.
    pub stack: StackSpoof,
    // 和trampoline解决 x64 下 rdx 与 rcx参数不匹配的“物理硬伤”，确保回调能成功进入 RtlCaptureContext
    pub callback: u64,
    pub trampoline: u64,
    // 三个dll的基址
    pub modules: Modules,
    pub wait_for_single: WinApi,
    pub base_thread: WinApi,
    pub enum_date: WinApi,
    pub system_function040: WinApi,
    pub system_function041: WinApi,
    // 核心引擎:每一个 CONTEXT结束时都会通过这个函数加载下一个 CONTEXT
    pub nt_continue: WinApi,
    pub nt_set_event: WinApi,
    // 和base_thread一起提供合法的系统启动点地址，用于填充栈欺骗中的返回地址
    pub rtl_user_thread: WinApi,
    // 隐匿开关。负责在执行前将内存改为 RWX，执行后改回 R
    pub nt_protect_virtual_memory: WinApi,
    pub rtl_exit_user_thread: WinApi,
    pub nt_get_context_thread: WinApi,
    pub nt_set_context_thread: WinApi,
    pub nt_test_alert: WinApi,
    pub nt_wait_for_single: WinApi,
    pub rtl_acquire_lock: WinApi,
    pub tp_release_cleanup: WinApi,
    // 原始模板。获取线程初始“干净”状态的唯一手段
    pub rtl_capture_context: WinApi,
    pub zw_wait_for_worker: WinApi,
}
```

Default:     
允许通过Config::default()创建一个全零/默认值的实例

Debug:  
允许在调试模式下打印该结构其中字段的内容(Api地址等)

Clone/Copy:  
使Config像i32一样通过内存拷贝传递,不需要引用计数和所有权转移.在no_std环境下非常高效/安全

## StackSpoof

> Windows x64 架构下，EDR 会通过栈回溯（Stack Walk）来检查当前代码的合法性。StackSpoof的每一个字段，都是为了在内存中完美模拟出一个“出身正统”的系统线程


```rust
/// Represents a reserved stack region for custom thread execution.
#[derive(Default, Debug, Clone, Copy)]
pub struct StackSpoof {
    /// Address of a `gadget_rbp`, which realigns重新对齐 the stack (`mov rsp, rbp; ret`).
    gadget_rbp: u64,

    /// Stack frame size for `BaseThreadInitThunk`.
    base_thread_size: u32,

    /// Stack frame size for `RtlUserThreadStart`.
    rtl_user_thread_size: u32,

    /// Stack frame size for `EnumResourcesW`.
    enum_date_size: u32,

    /// Stack frame size for `RtlAcquireSRWLockExclusive`.
    rlt_acquire_srw_size: u32,

    /// Type of gadget (`call [rbx]` or `jmp [rbx]`).
    gadget: GadgetKind,
}
```

收集并存储伪造一个“合法 Windows调用栈”所需的所有关键尺寸和跳转点  
现代编译器,特别是MSVC优化编译,大多开启了FPO(Frame Pointer Omission),即RBP不再作为栈帧指针,而被当作普通通用寄存器使用,栈帧寻址完全依赖RSP.

**那么hypnus中gadget_rbp为什么仍然用了RBP** 

虽然微软优化了RBP的使用,但EDR和Windows异常处理(如RtlVirtualUnwind)在回溯栈时,依然会兼容两种模式:
1. Frame-based Unwinding(经典模式):依赖RBP链,很对遗留模块/非优化代码还在用
2. Table-based Unwinding(现代模式):即pdata(procedure data)/xdata表.这是现代win64主流.编译器在PE文件中生产一张表,记录函数在每段偏移量下,RSP和RBP如何变化

gadget_rbp:栈重新定向的物理桥梁.其指向一段机器码地址,该机器码执行mov rsp, rbp; ret  
在hypnus中,需要频繁的在伪造的返回地址和真实的执行逻辑之间跳转.
1. 当混淆链执行到一个任务时(如NtProtect...)结束后,cpu会执行ret
2. 项目中手动指定了rsp,标准的ret会让cpu跑飞
3. 在alloc_memory中手动分配了一段内存写入mov rsp ,rbp; ret  .通过这个指令,强制将当前rsp重置到基址指针rbp的位置,实现栈帧的平滑回退
4. 在spoof.rs中,这个地址被填入ctx.rbp
5. gadget_rbp 的真实角色：不是为了链，而是为了跳转.
  * 用途:为了骗过EDR的栈回溯,在栈上构建了伪造的看起来合法的函数调用序列。当 Payload 执行完毕，当前rsp指向的是构造的虚假栈帧,而非原本的系统栈.你不能直接ret，因为此时的 RSP 已经指向了你伪造的栈区域.这里将rbp寄存器征用为合法的栈基址寄存器,进入payload前,RBP提前保存了构造的合法栈基址
  * mov rsp, rbp: 栈平移.瞬间将栈指针 RSP恢复到你之前构造好的“伪造合法栈帧”的基址.
  * ret: 直接跳转到你栈顶存放的那个“合法返回地址”（比如指向RtlUserThreadStart 内部）
  * 所以，这个 RBP此时已经不是“帧指针”了，它被当成了一个“栈指针基址寄存器”来使用。这是一种对寄存器用途的“重定向”

**再次展开这里用到的rbp作用**:

gadget_rbp 根本不是在构造 RBP 链，它是在向 Windows 的 pdata异常展开引擎（Unwind Engine）进行一次“格式化演出”。

1. 明确一个事实：现代 Windows 根本不在乎 RBP 链.在 x64 环境下，微软强推基于 pdata 表的表驱动展开 (Table-based Unwinding)。这意味着：
  * 对于 pdata： EDR 根本不去理会 RBP 的值是多少。它只看当前的 RIP指针，然后去 PE 文件的 .pdata 段中搜索这个 RIP属于哪个函数范围，并根据对应的 xdata结构读取“如何恢复上一层栈帧”的指令。
  * 所以： 那个 gadget_rbp 代码片段 mov rsp, rbp; ret 在现代调试器或 EDR看来，它仅仅是一个“将 RBP 的值赋给 RSP 的指令序列”。

2. 那为什么要叫它 gadget_rbp.这其实是一个“反向误导”，或者是对该 Gadget 作用的简写：
  * 它并不是要构造一个 RBP 链条。
  * 它的真实意图是：“确保在执行 ret 之前，RSP处于一个我（攻击者）定义好的、完全合法的状态。”
  * 之所以叫 rbp，是因为在这个特定的 Gadget 中，程序逻辑把 RBP当作了一个暂存区，用来存放“我想要恢复到的 RSP 基址”。

3. 如何处理 pdata 表（真正的欺骗逻辑）既然不依赖 RBP 链，hypnus 是如何骗过 pdata 回溯器的.真正的欺骗发生在 src/spoof.rs 中的 spoof 函数里，它的逻辑是这样的：
  * 静态伪造 pdata 契约：hypnus 在构造栈帧时，会模拟 RtlUserThreadStart或其他系统函数的“栈帧布局”。
  * 堆栈模拟：它在栈上预留的空间大小（base_thread_size 等字段），严格匹配这些函数在 pdata 中定义的 Prologue 对栈的操作
  * 诱导回溯：
    * 当 EDR 回溯时，它读到当前的 RIP 指向 NtProtectVirtualMemory的内部地址。
    * 它去查 pdata，发现这个地址需要“弹出 0x28 字节的栈空间”
    * 关键点：你在构造栈帧时，已经预先填充了那 0x28字节的数据（虽然是假的）。
    * EDR回溯器成功地“弹出”了这些假数据，并将回溯器认为的“上一层调用者地址”指向了你伪造的下一个函数地址（比如 RtlUserThreadStart）。


**总结：Gadget 在此处的角色**
* 针对 pdata 的处理：通过精确控制栈帧大小（_size 字段）来匹配 pdata记录的展开信息。
* 针对 gadget_rbp的处理：它是在你的“伪造栈帧”全部执行完，要回归真实的系统调用链末尾时，负责执行最后一次“栈底校准”的指令。它完全跳过了 RBP 链的逻辑，直接通过修改 RSP来欺骗那个基于栈帧大小回溯的引擎。



>hypnus 不走 RBP 链，它走的是 “精确栈帧覆盖”。它通过 gadget_rbp 执行 mov rsp, rbp，并不是为了构造 RBP链，而是为了在切换回系统调用栈时，通过这一行汇编指令，把因为之前伪造栈帧而变得混乱的 RSP 指针，强行归位到你精心构建的、符合 pdata 预期的“合法栈顶”。这是一种利用汇编指令在物理内存层面“重置坐标系”的技巧，而非维护某种链表结构

**源码示例:**
gadget_rbp机制通过以下四个物理步骤实现，将“伪造栈”产生的混乱状态完美还原至“真实执行流”：

1. 机器码预置 (Code Injection)  
  对应函数： StackSpoof::alloc_memory (位于 spoof.rs)  
  实现逻辑： 在初始化阶段，申请一块PAGE_EXECUTE_READ 内存，将指令 mov rsp, rbp;ret (机器码 0x48 0x89 0xEC 0xC3) 写入其中。其物理地址被封装为二级指针，存储在Config.stack.gadget_rbp 字段中。

2. 物理坐标备份 (Context Snapshot)
  对应函数： StackSpoof::spoof (位于 spoof.rs)  
  实现逻辑： 在构建异步混淆链循环中，将当前线程真实的栈顶地址ctx.Rsp（即程序正常的返回坐标）备份到 ctx.Rbp寄存器中，将其作为物理层面的“归位锚点”。

3. 指令链条关联 (Register Linking)
  对应函数： StackSpoof::spoof (位于 spoof.rs)  
  实现逻辑： 将步骤 1 中准备好的“二级指针地址”填入 ctx.Rbx 寄存器。此时，Rbx 充当了传送门的钥匙，它指向那段能修改 Rsp 的机器码。

4. 栈帧平滑回退 (Stack Realignment)
  执行位置： 混淆链任务结束时的 ret 跳转 (由 Hypnus::timer/wait/foliage 编排)  
  实现逻辑： 当混淆任务（如 NtProtect...）执行完毕，触发 ROP 跳转至系统 DLL 中的 `call [rbx]`指令。
   * CPU 读取 Rbx 进入注入的机器码，执行 mov rsp, rbp
   * 物理结果： Rsp 瞬间抛弃了下移 10 个页面的“伪造深渊”，重新指向了 Rbp中备份的“家”的地址。随后的 ret指令便能从真实栈顶弹出下一条合法指令，实现从隐匿态到执行态的无缝切换。


## Config::new()

```rust
impl Config {
    /// Create a new `Config`.
    pub fn new() -> Result<Self> {
        // Resolve hashed function addresses for all required APIs
        let mut cfg = Self::winapis(Self::modules());
        cfg.stack = StackSpoof::new(&cfg)?;
        cfg.callback = Self::alloc_callback()?;
        cfg.trampoline = Self::alloc_trampoline()?;

        // Register Control Flow Guard function targets if enabled
        if let Ok(true) = is_cfg_enforced() {
            register_cfg_targets(&cfg);
        }

        Ok(cfg)
    }
```


### LoadLibraryA

模块进入地址空间有三种途径:  
1. 系统预加载:如ntdll.dll
2. 静态链接加载
3. 动态手动加载:LoadLibraryA

hypnus中使用伪造栈加载LoadLibraryA


## Config::alloc_callback()

```rust
pub fn alloc_callback() -> Result<u64> {
        // Trampoline shellcode
        let callback = &[
            0x48, 0x89, 0xD1,       // mov rcx,rdx
            0x48, 0x8B, 0x41, 0x78, // mov rax,QWORD PTR [rcx+0x78] (CONTEXT.RAX)
            0xFF, 0xE0,             // jmp rax
        ];

        // Allocate RW memory for trampoline
        let mut size = callback.len();
        let mut addr = null_mut();
        if !NT_SUCCESS(NtAllocateVirtualMemory(
            NtCurrentProcess(), 
            &mut addr, 
            0, 
            &mut size, 
            MEM_COMMIT | MEM_RESERVE, 
            PAGE_READWRITE
        )) {
            bail!(s!("failed to allocate stack memory"));
        }

        // Write trampoline bytes to allocated memory
        unsafe { core::ptr::copy_nonoverlapping(callback.as_ptr(), addr as *mut u8, callback.len()) };

        // Change protection to RX for execution
        let mut old_protect = 0;
        if !NT_SUCCESS(NtProtectVirtualMemory(
            NtCurrentProcess(), 
            &mut addr, 
            &mut size, 
            PAGE_EXECUTE_READ as u32, 
            &mut old_protect
        )) {
            bail!(s!("failed to change memory protection for RX"));
        }
```

在win64下,通过动态生成并注入Gadget,解决线程池回调(Threadpoll Callback)和系统上下文切换(NtContinue)之间的不匹配问题


## Thread Pool

win中,线程池由ntdll.dll内部的TppWorkerThread工作者线程管理.  
1. 当调用TpAllocTimer或TpAllocWait,指定的代码没有执行,只是在内核中


1. 分配 (Allocation)：调用 TpAlloc*
      创建任务对象。此时会绑定回调函数地址和用户自定义参数 (Context)。
   2. 设置 (Setting)：调用 TpSet* 激活任务（如设置 5 秒后触发）。
   3. 内核监控：任务进入内核队列。此时你的主代码不占用
      CPU，没有任何行为特征。
   4. 派发 (Dispatching)：条件满足，内核唤醒一个 ntdll!TppWorkerThread。
   5. 回调执行：
       * 寄存器分配：Instance -> RCX, Context -> RDX, Timer/Wait -> R8。
       * 栈环境：代码运行在系统原生的、合法的线程栈上。
   6. 清理 (Cleanup)：任务完成后，Worker 线程回到池中等待下一个任务

## NtContinue

``` NtContinue(PCONTEXT ContextRecord, BOOLEAN TestAlert)```

Ntcontinue是ntdll.dll中的系统调用,是处理seh的核心.其作用不是调用函数,而是状态替换.在内核中把当前cpu所有的寄存器扔掉,换成ContextRecord中的值
1. 无视堆栈,因为直接改写rsp.可以让cpu跳到一个伪造的堆栈空间
2. 无视返回路径.没有ret
3. 权限高:是内核级指令的用户态包装,只要参数合法,内核无条件执行跳转
4. edr是否会监测该函数

### TpAllocTimer TpAllocWait NtContinue

位置:ntdll.dll中的Tp系列和Nt系列函数
作用:是Windows用户态最底层,直接和内核通信

**TpAllocTimer(线程池计时器分配)**  
功能：在 Windows 线程池引擎中创建一个计时器对象  
1. 当你调用 TpAllocTimer时，你并不是在执行代码，而是在向系统注册一个“未来任务”。
2. 这个任务会被交给 Windows 的 TppWorkerThread（工作者线程）
3. 规避点：如果你的代码直接运行，EDR 的监测点就在你的线程里。但通过TpAllocTimer，真正的代码执行发生在系统的合法线程中，你的恶意线程可以完全处于睡眠或挂起状态。这实现了“身首异处”，让扫描器找不到代码的真正发起者
4. 在 hypnus 中的角色：作为“引信”。它在 timer策略中负责在指定延迟后，由系统线程触发那个 9 字节的 Trampoline

**TpAllocWait (线程池等待对象分配)**  
功能：创建一个等待对象，当某个内核句柄（如 Event,Mutex）被触发时，执行回调  
意图：事件驱动的异步触发  
1. 与计时器不同，它是被动触发的.在 hypnus中，它被用来实现更复杂的混淆逻辑。例如，你可以让混淆链停在某个位置，直到另一个合法的系统动作触发了某个 Event，混淆链才继续往下走
2. 这种基于事件的执行流极其难以追踪。因为它不符合常规程序的“顺序执行”逻辑。对于自动化分析沙箱来说，这种不确定的触发机制往往会导致超时而无法检测到恶意行为

**NtContinue (上下文恢复与跳转)**
1. 实现 Context Chaining（上下文串联）,上帝视角的执行流操纵
2. 底层行为：它会强行覆盖当前 CPU 所有的寄存器状态（RIP, RSP, RAX...），让CPU 按照 ContextRecord 指向的状态重新开始执行
3. 无视调用约定：常规的 call 或 jmp 只能修改 RIP。但 NtContinue可以一次性修改所有寄存器。这允许我们在不回到自己代码的前提下，在NtProtectVirtualMemory、SystemFunction040 等系统函数之间反复横跳
4. 规避返回地址扫描：如果你用 call 调用API，栈上会留下你的返回地址。如果你用 NtContinue “降临”到 API内部，栈上可以完全没有你的痕迹（因为你可以自己伪造 RSP）

**三者联动配合**  
1. 准备阶段：Config::new 准备好一个 CONTEXT 数组。每个 CONTEXT都是一个动作（比如：第 1 个是修改权限，第 2 个是加密）。
2. 注册阶段：调用 TpAllocTimer，告诉 Windows：“5 秒后，去执行我的Trampoline 地址，并把第一个 CONTEXT 指针传给它。”
3. 引爆阶段：5 秒到，系统线程调用 Trampoline。Trampoline 把参数挪到RCX，然后 jmp NtContinue。
4. 链式爆发：
    * NtContinue 加载 CONTEXT_1，CPU 跳去执行 NtProtectVirtualMemory。
    * 最精妙的点：在 CONTEXT_1 的伪造栈顶，预先存好了 NtContinue的入口地址。
    * 当 NtProtectVirtualMemory 执行完 ret 时，它会再次跳进 NtContinue。
    * NtContinue 接着加载 CONTEXT_2（加密内存）。
    * 结果：整个加解密过程在系统线程中像多米诺骨牌一样自动倒下，而你的主代码从头到尾都没参与。

