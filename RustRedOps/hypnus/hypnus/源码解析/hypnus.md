- [十个ctxs功能](#十个ctxs功能)
- [NtSetEvent2](#ntsetevent2)
- [TPAllocTimer](#tpalloctimer)
  - [第一个参数是指针的指针](#第一个参数是指针的指针)
- [TpAllocTimer中的trampoline](#tpalloctimer中的trampoline)
- [threadpool workerfactory worker](#threadpool-workerfactory-worker)
- [SystemFunction041](#systemfunction041)
- [SystemFunction040](#systemfunction040)
- [NtWaitForSingleObject](#ntwaitforsingleobject)
- [`ctxs[0]`](#ctxs0)
- [关于PE文件的节区Section](#关于pe文件的节区section)
- [IMAGE\_SECTION\_HEADER](#image_section_header)
- [section\_by\_name](#section_by_name)
- [dinvk::helper::section()](#dinvkhelpersection)
- [fn timer::NtDuplicateObject](#fn-timerntduplicateobject)
- [fn timer::rax](#fn-timerrax)
- [hypnus.rs的执行流](#hypnusrs的执行流)
- [TP\_CALLBACK\_ENVIRON\_V3](#tp_callback_environ_v3)
- [struct TP\_POOL\_STACK\_INFORMATION](#struct-tp_pool_stack_information)
- [TpAllocPool(\&mut pool, null\_mut())](#tpallocpoolmut-pool-null_mut)
- [三个event\[\]](#三个event)
- [win64 Event](#win64-event)
- [Event Thread区别](#event-thread区别)
- [Struct Hypnus::time::NtCreateEvent](#struct-hypnustimentcreateevent)
  - [与函数原型的映射解析](#与函数原型的映射解析)
- [struct ObfMode](#struct-obfmode)
- [Fiber 纤程(Windows)](#fiber-纤程windows)
  - [适用场景](#适用场景)
  - [ConvertThreadToFiber](#convertthreadtofiber)
- [mod \_\_private](#mod-__private)
- [扩展-关于Pin代替Box](#扩展-关于pin代替box)
- [扩展-Box::into\_raw/内部数据拷贝时发生的内部布局移动](#扩展-boxinto_raw内部数据拷贝时发生的内部布局移动)
- [CONTEXT-暂存知识点,后续需要移动到其他文件中](#context-暂存知识点后续需要移动到其他文件中)
  - [进程 线程 纤程切换概览](#进程-线程-纤程切换概览)
- [扩展-关于handle的概念](#扩展-关于handle的概念)
- [win64 threadpool](#win64-threadpool)
  - [系统默认线程池](#系统默认线程池)
- [线程池和事件](#线程池和事件)
- [IOCP (I/O Completion Port)和worker factory](#iocp-io-completion-port和worker-factory)
- [扩展-handle句柄](#扩展-handle句柄)
- [扩展- `AsRef<[u8]>`](#扩展--asrefu8)







## 十个ctxs功能

event`[0]`绑定的是RtCaptureContext(通过ctx_init\TpAllocTimer(中NtSetEvent2)).主线程在 NtWaitForSingleObject(`event[0]`) 这里停一下，是为了确保ctx_init 这个“容器”已经被填满了真实的 CPU 数据。没有这一步，后面 10 个 ctxs都是空壳.当RtCaptureContext执行完之后,才将event`[0]`(初始设为阻塞)设为点亮

event`[1]`绑定的是`ctx[0]`的寄存器


`ctx[0]`:等待`events[1]`,目的是在混淆链条第一步堵塞,等执行完old_protect写入内存后,在放后面的9个ctx执行.后续线程池（辅助线程）(在后面的for ctx in &mut ctxs中)会按照代码顺序执行下面的各个ctx





[win64 threadpool](#win64-threadpool)

##  NtSetEvent2

```rust
 status = TpAllocTimer(
                // 第二个定时器handle
                &mut timer_event,

                // win api:将事件对象从无信号转为有信号 
                // 如何从外部链接到本项目的:在winapis.rs中有对NtSetEvent2的定义(作为一个封装函数),其内部调用NtSetEvent接收一个event.因为标准的NtSetevent签名与线程池回调签名不匹配.
                // 在混淆逻辑运行时,调用NtSetEvent2的是win线程池的工作线程(worker thread).当线程池触发定时器跳到NtSetEvent2时,工作线程内部会执行给寄存器赋值操作
                NtSetEvent2 as *mut c_void,

                //  函数开头创建的第一个事件handle
                // 1. events[0]->TpAllocTimer;2. 定时器触发-> events[0] 被塞进 CPU 的 RDX 寄存器;3. NtSetEvent2 被调用 -> 它用 RDX中的handle,去内核
                events[0], 
                &mut env
            );
```

核心目的是：当线程池完成“自拍（快照捕获）”这一动作后，通过触发一个事件，把陷入休眠的主线程“叫醒

这里的 NtSetEvent2是对NtSetEvent(event, null_mut())的wrapper.但是 NtSetEvent2有四个参数.  
这是一个经典的abi隐式传参.这个传参动作不是rust的,由Windows内核(线程池引擎)直接在cpu寄存器层面完成的.

在混淆逻辑运行时,调用NtSetEvent2 的不是你的主程序，而是 Windows线程池的工作线程（Worker Thread）



## TPAllocTimer

```c
// winbase:CreateThreadpoolTimer
/**
 * Allocates a timer object.
 *
 * \param[out] Timer A pointer to a variable that receives the new timer object.
 * \param[in] Callback The callback function to execute when the timer expires.到期/期满
 * \param[in,out] Context Optional application-defined程序定义好的 data to pass to the callback function.
 * \param[in] CallbackEnviron Optional callback environment for the callback.
 * \return NTSTATUS Successful or errant status.
 * \sa https://learn.microsoft.com/en-us/windows/win32/api/threadpoolapiset/nf-threadpoolapiset-createthreadpooltimer
 */
NTSYSAPI
NTSTATUS
NTAPI
TpAllocTimer(
    _Out_ PTP_TIMER *Timer,
    _In_ PTP_TIMER_CALLBACK Callback,
    _Inout_opt_ PVOID Context,
    _In_opt_ PTP_CALLBACK_ENVIRON CallbackEnviron
    );


```


```rust

TpAllocTimer(
                // 输出参数,内核把新创建的的定时器对象TP_TIMER的物理内存地址填入
                &mut timer_ctx, 
                // 垫片
                self.cfg.trampoline as *mut c_void, 
                // 堆栈上定义的CONTEXT.执行时，RDX 寄存器里要装什么（例如伪造的 CONTEXT 结构体地址）
                &mut ctx_init as *mut _ as *mut c_void, 
                // 执行环境TP_CALLBACK_ENVIRON_V3
                &mut env
            );
```

**作用:**Allocates a timer object.在进程的堆内存heap中开辟一块空间,存放一个TP_TIMER结构体.
1. TpAllocTimer 所分配的 Timer Object（定时器对象）并非一个孤立的数据结构，它是 Windows 现代线程池（Thread Pool API） 架构中连接用户态逻辑与内核态硬件中断的精密桥梁
2. 在 Windows 中，“Timer Object”在不同的层级有不同的形态，TpAllocTimer操纵的是用户态的包装对象，但它背后由内核态对象支撑
3. 你在 Rust 中调用 TpAllocTimer 时，它实际上在进程的用户态堆（NT Heap）中分配了一个未文档化的内部结构体，通常被逆向工程师称为 _TP_TIMER 或 _TTP_TIMER（在 Windows 10/11 ntdll 中）.它的**核心作用是：状态追踪与信息绑定**.这个结构体包含:
    *用户态实体_TP_TIMER:
       * Callback 指针：定时器触发时要执行的代码地址（在 hypnus 中是那个 trampoline 汇编）
       * Context 指针：用户自定义的数据（hypnus 中的伪造 CONTEXT）
       * Environment 绑定：指向 _TP_POOL 和清理组（Cleanup  Group），决定这个定时器属于哪个线程池
       * 状态机标志：记录这个定时器是处于“未激活”、“倒计时中”还是“正在执行回调”的状态
     * 内核态实体KTIMER:
       * 单靠用户态的堆内存是无法计时的。Windows真正的计时能力在内核（Ntoskrnl.exe）。用户态的 _TP_TIMER最终要依赖内核层的调度器（Dispatcher）对象 KTIMER。KTIMER 是一种可以处于“有信号（Signaled）”或“无信号（Nonsignaled）”状态的内核同步对象.它直接与操作系统的硬件时钟系统挂钩
4. TpAllocTimer 本身只是准备了用户态的数据结构，但当它配合 TpSetTimer 激活时，它撬动了 Windows 操作系统最核心的四个底层机制：
 1. 硬件定时器与时钟中断 (Hardware Interrupts)
  OS 的计时依赖于主板上的硬件：RTC（实时时钟）、HPET（高精度事件定时器）或 CPU
  的 TSC（时间戳计数器）。
   * 这些硬件会以固定的频率（默认约 15.6 毫秒，可通过 timeBeginPeriod 提升至 1
     毫秒）向 CPU 发送硬件中断（IRQL: CLOCK_LEVEL）。
   * Windows
     内核的时钟中断处理程序（KeUpdateSystemTime）会捕获这个节拍，更新系统时间，
     并检查有没有哪个 KTIMER 到期了。

  2. DPC (延迟过程调用 - Deferred Procedure Call)
  由于硬件中断的处理必须极快，内核不能在中断上下文里直接去执行复杂的线程池逻辑。
   * 当内核发现一个 KTIMER 到期时，它会生成一个 DPC（IRQL: DISPATCH_LEVEL）
     并挂入队列。
   * 这个 DPC 的作用是：在 CPU
     稍闲时，把“定时器到期”这个事件，打包成一个数据包。

  3. IOCP (I/O 完成端口 - KQUEUE) 与 Worker Factory
  这是 Windows 10/11 现代线程池的灵魂。
   * 每一个 Windows 线程池（TP_POOL）在内核中都对应一个 Worker Factory
     (工作者工厂) 对象。
   * Worker Factory 的底层核心是一个 I/O 完成端口 (IOCP，在内核中叫 KQUEUE)。
   * 当 DPC 执行时，它会将“定时器到期”的数据包（I/O Completion
     Packet）直接插入到这个 IOCP 的队列中。

  4. ZwWaitForWorkViaWorkerFactory (线程唤醒机制)
  还记得 hypnus 代码里的 ctx_spoof.Rip = cfg.zw_wait_for_worker.as_u64(); 吗？
   * 线程池里平时有一些闲置的 Worker 线程，它们都在调用
     ntdll!ZwWaitForWorkViaWorkerFactory（底层是
     NtWaitForWorkViaWorkerFactory）处于休眠状态，死死盯着那个 IOCP。
   * 一旦定时器的包进入 IOCP，内核调度器就会唤醒其中一个休眠的 Worker
     线程，把包交给它。

  ---

  三、 完整生命周期：从 TpAllocTimer 到代码执行

  现在，我们把所有的碎片拼接起来，看一条完整的执行流。这正是 hypnus 想要利用的
  OS 逻辑：

   1. 阶段 1：装填弹药 (TpAllocTimer)
       * 在用户态的堆里创建 _TP_TIMER 对象，把 hypnus 伪造的 CONTEXT 和
         Trampoline 地址封进这个结构体里。内核此时对此一无所知。
   2. 阶段 2：拉开引信 (TpSetTimer)
       * ntdll 内部的线程池管理器调用底层 Native API（如 NtSetTimerEx
         或向线程池的统一定时器管理线程发送 ALPC 消息）。
       * 内核在队列中激活一个 KTIMER 对象，设置其到期时间（例如 100 毫秒后）。
   3. 阶段 3：静默倒计时 (Hardware & Kernel)
       * CPU 产生硬件中断 -> 内核时钟节拍增加。
       * 100 毫秒到了，KeUpdateSystemTime 发现 KTIMER 过期。
   4. 阶段 4：信号投递 (DPC & IOCP)
       * 内核触发 DPC，DPC 将一个表示“任务就绪”的完成包压入当前线程池绑定的
         Worker Factory（IOCP 队列）。
   5. 阶段 5：唤醒与执行 (Worker Thread)
       * 系统线程池中的某个 Worker 线程被内核从 ZwWaitForWorkViaWorkerFactory
         状态唤醒。
       * Worker 线程从 IOCP 中取出包，根据指针找到了我们在阶段 1 创建的
         _TP_TIMER 结构体。
       * Worker 线程提取出里面的 Callback 地址（我们的蹦床）和
         Context（伪造的寄存器快照），将 Context 放入 RDX，执行 call Callback。
       * 蹦床执行 mov rcx, rdx; jmp rax，彻底劫持执行流进入
         NtContinue，免杀逻辑正式启动！

  四、 为什么在免杀 (RedOps) 中它如此致命？

  了解了以上 OS 底层细节，你就能明白为什么 hypnus 要用它：

   1. 真正的“异步断层”：我们不是自己写了个循环或者
      Sleep()，我们是把执行权交给了内核的 DPC 和系统的 Worker Factory。这导致
      EDR 在进行线程调用栈回溯（Stack Walking）时，看到的栈底永远是干净的
      ntdll!RtlUserThreadStart ->
      ntdll!TppWorkerThread，完全追溯不到是哪段恶意代码触发了这个回调。
   2. 滥用系统信任设施：Worker Factory 和 IOCP 是 Windows
      IIS、RPC、系统内部组件高并发通信的基础。EDR 无法粗暴地 Hook 或阻止基于
      Worker Factory 的唤醒行为，否则整个 Windows 都会崩溃。

  综上所述，TpAllocTimer 创建的不是一个简单的倒计时器，而是一个接入 Windows
  最核心的中断驱动与 IOCP
  调度生态的入场券。这正是高级免杀技术对操作系统底层设施的一种“合法滥用”。


**特性:**
1. 一个纯粹的内存准备动作。它只在用户态堆内存中创建并初始化了数据结构，不会启动任何计时器，内核甚至不知道它的存在
2. 它绝对不会阻塞当前线程
3. 所属模块：ntdll.dll（Windows Native API 层）,Undocumented
4. CreateThreadpoolTimer（位于kernel32.dll / kernelbase.dll），其底层实际上就是对 ntdll!TpAllocTimer 的封装


### 第一个参数是指针的指针

1. 其原型中的第一个参数Out_ PTP_TIMER *Timer，在 Rust 中是 *mut *mut c_void(即指针的指针);
2. 其原型的第一个参数类型*Timer在c中含义:因为c函数参数都是值传递,如果是Timer,那么在c中是指针的拷贝,函数内部把新分配的堆地址赋给这个拷贝,函数结束后,拷贝销毁,调用者外部的变量依然是null
3. 一个变量物理实质是内存中的值,那么就分为内存地址和内存地址中的值,指针就是地址,解引用指针就值.根据源码TpAllocTimer函数外部定义了一个变量timer(假设其地址是0x1000,值是0).
4. 假设TpAllocTimer第一个参数不是*Timer而是Timer(对应rust中的`*mut c_void`和`*mut *mut c_void`):首先需要理解在c的视角下,c函数是值传递,那么接收到参数timer(timer=0)后,在自己的栈上创建一个参数副本,值就是外部传入实参0,内部会为这个实参重新分配地址(假设是栈地址0x2000).TpAllocTimer函数在堆上分配了真实的定时器对象,假设是栈地址0x9000;由于第一个参数是被赋值的.根据TpAllocTimer的定义那么该参数副本会被赋值0x9000,当函数退出清理自身栈参数副本0x2000被销毁,其被赋值的0x9000也不存在了,这就出现了内存泄露.
5. 回到*Timer的情况,传入的是&timer(物理地址0x1000,值0),那么传入的就不再是0,而是0x1000.同样TpAllocTimer会在栈上拷贝一份该传入的地址0x1000的副本(假设该副本地址是0x2000,其值就是0x1000).然后TpAllocTimer创建定时器对象(假设地址是堆0x9000),并将0x9000这个地址存入地址0x1000指向的内存.函数销毁,0x2000被清理,但是在函数外部的变量中的值已经被修改了.
6. 以上,在传参后TpAllocTimer赋值给第一个参数阶段,一级指针情况下,外部变量的值在传入TpAllocTimer后被拷贝了一份,之后的操作都是对这份拷贝的操作,之后这个拷贝被赋值为TpAllocTimer产生的计时器对象地址,当函数结束被拷贝的值被清理,外部变量没有改变;在二级指针情况下,外部变量的指针被拷贝进来,之后TpAllocTimer产生的计时器对象地址被赋值给这个拷贝进来的指针指向的内存中的值.函数结束二级指针被销毁,但是一级指针和二级指针共同指向的内存中的值已经被改变了.
7. 在Timer的情况下在函数内部是赋值给值,在*Timer情况下赋值给一级指针导致最后赋值给一级指针指向的内存中;即传值只改复印件，传址能改原文件。要想让内核帮你分配对象并带回来，就必须交出你变量的物理坐标（二级指针），让内核顺着坐标强行写入
8. 函数原型的`*Timer`代表一个指针内部的值,在rust中应该用*mut *mut c_void来表示.该原型前的_Out_,是微软sal源代码注释语言标注.其代表告诉调用者:在自己的栈上准备一个空指针(`*mut c_void`),然后将该空指针的物理地址传过来(`*mut *mut c_void`)
9. 这里就引申出c是值传递,rust也是值传递.c/rust,函数传参永远只有一种方式:值传递/拷贝.无论其参数是指针\&引用或者其他类型,其实质都是值传递,当参数是指针或引用,物理实质就是把内存地址当作普通数值拷贝给函数.
10. Rust 能够通过 FFI（外部函数接口）与 C无缝对接的原因——因为它们在内存物理层面的价值观是完全一致的：一切皆为“值拷贝”，只不过有时拷贝的是数据本身，有时拷贝的是数据的物理坐标.
11. C 的指针(*)：编译器不管你传进来的地址对不对，也不管你什么时候解引用，完全信任程序员
12.  Rust 的引用 (& / &mut)：物理上依然是指针，但 Rust 编译器（Borrow Checker）在编译时给这个地址加了严格的“生命周期”和“读写权限”审查。它确保传进去的这个地址一定是有效的，且符合“借用规则”。
13.  Rust 的原生指针 (*mut / *const)：当你在 Rust 里写 FFI 调用 Windows API时（就像 hypnus 里这样），Rust 放弃了审查，退化成了和 C一模一样的裸指针行为，此时生死自负（所以必须包在 unsafe 块里）
14. Rust 独创的“引用（& / &mut） + 生命周期（Lifetime） +所有权（Ownership）”机制，其根本目的，就是为了在保留 C语言这种“直接传递内存地址（传值）”的极高运行效率的同时，在编译阶段彻底堵死它带来的安全黑洞 
15. 一级指针*mut c_void,代表内存中实际创建的TP_TIMER对象的地址(假设 0x000001FF4500)
16. 二级指针*mut *mut c_void,接收上个堆地址的变量的物理地址,即栈地址(即接收0x000001FF4500的变量的物理地址,假设0x000000AABBCC)
17. TpAllocTimer在堆上分配对象(假设地址0x1FF4500),然后用调用者传进来的栈地址0xAABBCC,将 0x1FF4500写入到0xAABBCC











## TpAllocTimer中的trampoline

当通过TpAllocTimer提交任务,os唤醒工作线程执行时,它遵循TpTimerCallback签名.在执行时对应的寄存器状态:
1. RCX：存储的是 PTP_CALLBACK_INSTANCE (实例指针)
2. RDX：存储的是 Context 指针（也就是在 TpAllocTimer 第三个参数传入的 &mutctx_init）
3. R8：存储的是 PTP_TIMER 句柄

而真正想要执行的是RtlCaptureContext,它只需要一个参数:  
RCX:ContextRecord指针,要求把要写入的内存地址放在这里



## threadpool workerfactory worker




## SystemFunction041

在 Windows 内核中，SystemFunction041 是 RtlDecryptMemory 的导出名
1. 核心功能：它是 SystemFunction040 (加密) 的对称函数
2. 对称性：它使用相同的内，将乱码还原回原始字节
3. 原子性：由于该操作是原地（In-place）进行的，解密后的数据会直接覆盖掉原来的加密区域


## SystemFunction040

位于cryptbase.dll 中  
本质：它是微软内部使用的 RtlEncryptMemory 函数的公开导出别名




## NtWaitForSingleObject

```c
/**
 * The NtWaitForSingleObject routine waits until the specified object is in the signaled state or the time-out interval elapses.
 *
 * \param Handle The handle to the wait object.
 * \param Alertable The function returns when either the time-out period has elapsed or when the APC function is called.
 * \param Timeout A pointer to an absolute or relative time over which the wait is to occur. Can be null. If a timeout is specified,
 * and the object has not attained a state of signaled when the timeout expires, then the wait is automatically satisfied.
 * If an explicit timeout value of zero is specified, then no wait occurs if the wait cannot be satisfied immediately.
 * \return NTSTATUS Successful or errant status.
 * \sa https://learn.microsoft.com/en-us/windows/win32/api/winternl/nf-winternl-ntwaitforsingleobject
 */
NTSYSCALLAPI
NTSTATUS
NTAPI
NtWaitForSingleObject(
    _In_ HANDLE Handle,
    _In_ BOOLEAN Alertable,
    _In_opt_ PLARGE_INTEGER Timeout
    );
```


作用:
1. 挂起执行流：它通过系统调用告知内核调度器，当前线程不再具备“执行资格”
2. 放弃 CPU 资源：该线程会被移出物理 CPU 的核心，寄存器状态（RIP, RSP, RAX等）被封存在内核栈中
3. 原子等待：它在内核中进入一个高效的睡眠循环。只有当目标对象（如`events[1]`）的 SignalState 变为 1 时，内核才会重新激活它
4. hypnus 利用它的核心原因：创造一个绝对静止的内存取证真空:
5. 消除执行特征：当线程停在 NtWaitForSingleObject时，它不执行任何用户态指令。EDR 的行为监控引擎（Behaviora lEngine）无法通过“指令序列分析”来判定它是恶意代码
6. 合法的“避风港”：在 Windows 系统中，成千上万个合法的系统线程（如svchost.exe）都在调用这个函数。将恶意执行流“停”在这里，相当于把一滴水藏进了大海
7. 内存翻转的安全期：由于线程在内核里“冻结”了，它的栈和堆此时是静态的。这给了主线程一个完美的物理时机去执行 XOR 加密（Heap Obfuscation），而不用担心发生内存访问冲突
8. 利用uwd作为堆栈欺骗的“归位锚点”,是整个控制流劫持的“中转站”
9. 利用 ret 指令的物理特性：
10. 当 NtWaitForSingleObject 完成使命准备“回家”时，它会执行汇编指令 ret;常规逻辑：ret 应该跳回调用它的那行代码
11. 由于我们是用 jmp 杀进来的，并在栈上预先填入了一个“受硬件认可的假地址”（如 BaseThreadInitThunk 内的某个位置）
12. 欺骗硬件检查 (CET Bypass)：
13. 此时 CPU 的硬件影子栈正在盯着这个 ret;由于 uwd 已经通过 .pdata解析，精准地把物理数据栈（RSP）对齐到了影子栈预期的那个合法位置
14. NtWaitForSingleObject执行完后，顺着我们铺好的路，合法地跳进了我们的 Gadget 陷阱中

> NtWaitForSingleObject 的实质是一个‘执行流的逻辑断点’。在 hypnus 与 uwd的配合下，它扮演了三个角色：第一，它是‘身份洗白器’，将恶意线程转化为合法的系统等待线程；第二，它是‘同步锁’，确保内存加密与执行流切换的时序一致性；第三，也是最核心的，它是‘劫持跳板’，它利用系统原生函数的合规返回动作，在硬件影子栈不感知的状态下，将执行权移交给后续的混淆链条


## `ctxs[0]`

`ctxs[0]`代表指定的conext

进入这段之前已经:
1. 找到了ntdll!NtWaitForSingleObject地址
2. 通过Gadget::new从ntdll的二进制流中找到合法的`jmp <reg>`片段
3. 通过NtDuplicateObject为主线程签发实态句柄
4. 已经通过``` ctxs=[ctx_init; 10]```复印了十份相同的环境
5. 通过创建`event[1]`,调用NtCreatEvent创建事件,调度对应的线程
6. ctxs1`[0]`是影子线程Worker Thread唤醒后执行的第一个动作


**分别调用:**
gadget::CONTEXT::jmp->Gadget::new(cfg)->get_text_section(extract .text节)->section_by_name(find .text节)->sections(PE中所有节)

1


1. 调用gadget::CONTEXT::jmp
2. 在jmp内部通过Gadget::new(cfg):
3. get_text_section
4. jmp:修改`ctxs[0].rip`,将rip指向ntdll中 `jmp <reg>`片段

## 关于PE文件的节区Section

静态的磁盘文件与动态的内存布局

为什么要有节区:
1. PE文件不是混在一起的二进制数据,而是被组织成功能隔离的区域.这种划分的本质原因是内存保护策略
2. 代码区 (.text)：必须是只读 + 可执行，防止被恶意篡改
3. 数据区 (.data)：必须是可读写，但通常不可执行
4. 资源区 (.rsrc)：图片、图标等，只需只读
5. 通过IMAGE_SECTION_HEADER，操作系统在加载程序时，能为不同的区域分配不同的页面保护属性 (Memory Protection)

## IMAGE_SECTION_HEADER

1. 位置:IMAGE_DOS_HEADER(e_lfanew)->IMAGE_NT_HEADERS(FileHeader字段存储节区数组的长度等信息)->IMAGE_SECTION_HEADER(其地址=NT Headers 起始地址 +sizeof(IMAGE_NT_HEADERS))
2. 是一个定长（Fixed-size）的结构,每个40字节.存放独赢节区的元数据描述.包含Name(节区名字)\Raw Data(节区物理布局:节区数据在磁盘上起点,占用空间大小)\Virtual Mapping(内存映像:节区加载到内存后RVA,在内存中实际占用的内存空间)\Characteristics(权限与行为)
3. 用于让加载器（Loader）以最快速度读取文件布局
4. 在pe文件中IMAGE_SECTION_HEADER数组是IMAGE_NT_HEADERS之后的一块连续的内存区域.
5. IMAGE_NT_HEADERS数组构成整个程序的内部布局蓝图.Windows加载器通过循环遍历这个数组,逐个解析每个节区信息,并根据节区信息向os申请内存\拷贝文件数据\赋予对应页面保护属性.
6. 没有IMAGE_NT_HEADERS数组,pe文件就是一堆乱序
7. IMAGE_NT_HEADERS是索引,section是实体



## section_by_name

**core::str::from_utf8_unchecked**

## dinvk::helper::section()

IMAGE_DOS_HEADER->IMAGE_NT_HEADERS->IMAGE_SECTION_HEADER

作者在这里其实做了一个假设：
`size_of::<IMAGE_NT_HEADERS>()` 等于 OptionalHeader 的末尾地址。但在实际的 PE 规范中，IMAGE_NT_HEADERS 包含的 OptionalHeader的大小是可变的（由 FileHeader.SizeOfOptionalHeader 决定）。

如果这个 PE 文件是一个非标准生成的，或者经过了某些“特殊处理”，简单的`size_of::<IMAGE_NT_HEADERS>()`可能无法定位到节表。成熟的解析逻辑通常会加上
`FileHeader.SizeOfOptionalHeader` 进行计算。

**(*nt).FileHeader.NumberOfSections**
1. 代表pe文件的IMAGE_SECTION_HEADER有多少
2. 加载器在加载 PE 时，如果不读取NumberOfSections，它甚至无法完成内存分配，程序根本启动不起来
3. 只要程序正在运行（或者你正在分析一个 PE文件），这个数字必须是正确的。如果它被改坏了，程序在加载阶段就会因为“无效的 PE 格式”被系统拒绝运行

## fn timer::NtDuplicateObject


```c
/**
 * The NtDuplicateObject routine creates a handle that is a duplicate of the specified source handle.
 *
 * \param SourceProcessHandle A handle to the source process for the handle being duplicated.
 * \param SourceHandle The handle to duplicate.
 * \param TargetProcessHandle A handle to the target process that is to receive the new handle. This parameter is optional and can be specified as NULL if the DUPLICATE_CLOSE_SOURCE flag is set in Options.
 * \param TargetHandle A pointer to a HANDLE variable into which the routine writes the new duplicated handle. The duplicated handle is valid in the specified target process. This parameter is optional and can be specified as NULL if no duplicate handle is to be created.
 * \param DesiredAccess An ACCESS_MASK value that specifies the desired access for the new handle.
 * \param HandleAttributes A ULONG that specifies the desired attributes for the new handle.
 * \param Options A set of flags to control the behavior of the duplication operation.
 * \return NTSTATUS Successful or errant status.
 * \sa https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/nf-ntifs-zwduplicateobject
 */
NTSYSCALLAPI
NTSTATUS
NTAPI
NtDuplicateObject(
    _In_ HANDLE SourceProcessHandle,
    _In_ HANDLE SourceHandle,
    _In_opt_ HANDLE TargetProcessHandle,
    _Out_opt_ PHANDLE TargetHandle,
    _In_ ACCESS_MASK DesiredAccess,
    _In_ ULONG HandleAttributes,
    _In_ ULONG Options
    );
```
hypnus:

```rust
/// Wrapper for the `NtDuplicateObject` API.
#[inline]
pub fn NtDuplicateObject(
    SourceProcessHandle: HANDLE,
    SourceHandle: HANDLE,
    TargetProcessHandle: HANDLE,
    TargetHandle: *mut HANDLE,
    DesiredAccess: u32,
    HandleAttributes: u32,
    Options: u32,
) -> NTSTATUS {
    unsafe {
        (winapis().NtDuplicateObject)(
            SourceProcessHandle,
            SourceHandle,
            TargetProcessHandle,
            TargetHandle,
            DesiredAccess,
            HandleAttributes,
            Options,
        )
    }
}
```
1. 典型的NTAPI,所有参数通过寄存器/栈传递,直接进入内核态
2. 将一个进程内的句柄（Handle）复制到另一个进程中
    * 句柄是内核句柄表中的索引。一个句柄值（例如0x4）只在特定进程的句柄表中才有意义
    * 复制的本质：它在目标进程的句柄表中，创建一个指向源句柄所对应内核对象的新条目



## fn timer::rax

hypnus的混淆链,利用的是没有call的跳转和劫持ret的技术





## hypnus.rs的执行流

1. 预设陷阱：主线程调用 TpAllocTimer，把 ctx_init 和trampoline（跳板快照）交给线程池
2. 设置闹钟：主线程调用 TpSetTimer。此时，Worker线程还没动，它还在等内核信号
3. 主线程“休眠” (NtWaitForSingleObject(events`[0]`, ...))：
    * 为什么现在睡？ 因为 RtlCaptureContext 必须在 Worker 线程里运行才能抓到 Worker 线程的特征。
    * 如果主线程不睡，而是直接往下走去伪造 Context，它拿到的 ctx_init 还是全零的（或者只有它自己的特征），这会导致后续所有的伪造全盘皆错
4. Worker 线程“拍照”：
    * 100ms 后，Worker 线程醒了，跳入 trampoline，执行 RtlCaptureContext。
    * 关键点：拍完照后，Worker线程执行完了。它并不会自动告诉主线程“我拍好了”
5. 点亮信号灯：
    * 作者设置了第二个定时器（timer_event），它执行的是NtSetEvent2(events`[0]`)。
    * 当这个定时器触发时，Worker 线程会点亮 events`[0]`
6. 主线程“复活”：
    * events`[0]` 变绿，主线程从 NtWaitForSingleObject 返回
    * 此时的状态：主线程确信 ctx_init 已经被 Worker 线程填满了最真实的“系统指纹”

三个event
1.  events`[0]`：任务： 确保主线程在“开始伪造”之前，Worker线程已经完成了自拍（RtlCaptureContext）
2.  绑定：作者创建了一个专门点亮 events`[0]` 的定时器任务（timer_event），执行的是 NtSetEvent2
3.  触发：它被设定在快照任务（timer_ctx）之后执行
4.  结果：主线程调用 NtWaitForSingleObject(events`[0]`)。只有当 Worker 线程拍完照并点亮了这个灯，主线程才会醒来执行后续的 ctxs 伪造逻辑
5.  目的：消除主线程与线程池之间的竞态，防止主线程读取到空的快照数据
















## TP_CALLBACK_ENVIRON_V3 

```rust
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TP_CALLBACK_ENVIRON_V3 {
    pub Version: u32,
    pub Pool: *mut c_void,
    pub CleanupGroup: *mut c_void,
    pub CleanupGroupCancelCallback: *mut c_void,
    pub RaceDll: *mut c_void,
    pub ActivationContext: isize,
    pub FinalizationCallback: *mut c_void,
    pub u: TP_CALLBACK_ENVIRON_V3_0,
    pub CallbackPriority: i32,
    pub Size: u32,
}

impl Default for TP_CALLBACK_ENVIRON_V3 {
    fn default() -> Self {
        Self {
            Version: 3,
            Pool: null_mut(),
            CleanupGroup: null_mut(),
            CleanupGroupCancelCallback: null_mut(),
            RaceDll: null_mut(),
            ActivationContext: 0,
            FinalizationCallback: null_mut(),
            u: TP_CALLBACK_ENVIRON_V3_0 { Flags: 0 },
            CallbackPriority: 1,
            Size: size_of::<TP_CALLBACK_ENVIRON_V3>() as u32,
        }
    }
}
```

1. C语言二进制接口（ABI）的内存块

## struct TP_POOL_STACK_INFORMATION

```rust
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct TP_POOL_STACK_INFORMATION {
    pub StackReserve: usize,
    pub StackCommit: usize,
}
```
用于描述线程池栈属性的低级结构体

* StackReserve (0x80000 - 512KB)：
    * 术语：虚拟地址空间预留 (VAS Reservation)。
    * 物理意义：它告诉操作系统：“请为池里的每个线程，在进程的虚拟地址空间中预留出连续的 512KB区域”。此时，系统并没有分配真实的物理内存，只是在页表里占了个坑。
* StackCommit (0x80000 - 512KB)：
    * 术语：物理内存提交 (Memory Commitment)。
    * 物理意义：它强制要求操作系统立即为这 512KB预留空间分配真实的物理内存（或分页文件空间）




























## TpAllocPool(&mut pool, null_mut())

1. 是 Windows 线程池 API (Thread Pool API) 的核心构造函数
2. 第一个参数 &mut pool: 传入 pool指针的地址，执行成功后，内核会将新创建的线程池对象地址填入这里。
3. 第二个参数 null_mut():这是一个保留参数（Reserved），根据微软文档，必须传入 NULL


**创建dedicated线程池意义**
1. 物理隔离 (Isolation):使用系统默认线程池，你的恶意混淆代码会和正常的系统任务（如浏览器载、系统更新）跑在同一个线程池里
2. 创建一个专用的 pool，意味着你拥有了完全属于自己的、干净的 worker线程。这些线程的生命周期和状态完全由 hypnus 控制，不会被其他程序的干扰
3. 当 EDR 扫描线程池线程时，它会回溯栈帧。私有线程池的线程起始于ntdll!TppWorkerThread，它的调用栈非常深且包含大量系统 DLL 指令
4. 通过在专用池里执行，hypnus让自己的代码看起来像是由系统内核发起的后台维护任务，而不是用户主动运行的恶意代码
5. 隐匿操作（如内存加密）必须是串行的。如果多个线程同时跑，会导致内存状态混乱（A 在加密，B却在读取）。私有池允许我们强制限制只有一个线程，确保不会出错

> TpAllocPool 的实质是在进程地址空间内申请一个独立的线程调度容器；在 hypnus中，它的作用是为异步混淆链提供一个物理隔离的‘执行沙盒’，通过脱离系统默认线程池，实现在规避 EDR 行为监控的同时，确保内存混淆逻辑在单线程环境下的线性确定性.
> 这是在为接下来的“影子操作”搭建一个专属的舞台






## 三个event[]

1. `events[0]`：与 NtSetEvent2 绑定
   * 绑定位置：在调用 TpAllocTimer创建第二个定时器任务时
   * 作用过程：当定时器触发，线程池的工作线程跳入 NtSetEvent2，此时 CPU 的 RDX 寄存器里存的就是这个 `events[0]`。NtSetEvent2 内部通过 RDX 拿到句柄并点亮信号，主线程的 NtWaitForSingleObject(`events[0]`...) 随之解锁

2. `events[1]：与 ctxs[0]` 的 RCX 寄存器绑定
   * 绑定位置：在主线程配置第一个伪造上下文 `ctxs[0]` 时
   * 作用过程：当混淆链启动，第一个任务 ctxs[0] 运行。因为它的 RCX 是`events[1]`，它会立即陷入等待。直到主线程完成所有配置，调用 NtSignalAndWaitForSingleObject(`events[1]`, ...)，手动点亮这个“发车信号”

3. `events[2]：与 ctxs[9]` 的 RCX 寄存器绑定
   * 绑定位置：在配置链条最后一环 `ctxs[9]` 时
   * 作用过程：这是整个链条的“终点线”。当之前的加解密、休眠任务全部顺序跑完，执行到 `ctxs[9]` 时，它会按照 RDX 里的指令点亮 `events[2]`。此时，主线程的`NtSignalAndWaitForSingleObject(..., events[2], ...)`接收到信号，宣告整个混淆周期结束

## win64 Event

Event:一种由内核管理的同步原语（Synchronization Object）
1. 它是一个内核对象，拥有“有信号（Signaled）”和“无信号（Non-signaled）”两个物理状态
2. 它允许一个线程在执行到特定位置时挂起等待（RedLight），直到另一个线程（或系统中断）将其状态修改为“有信号”（GreenLight），从而将其唤醒
3. 用户态通过一个 64位的句柄（Handle）来操控它，是实现多线程复杂逻辑同步（如：A干完，B才能开始）的基石.[扩展-关于handle的概念](#扩展-关于handle的概念)

**Event实质:**
1. 在 Windows 内核中，事件的实质是一个存储在 非分页池（Non-paged Pool） 中的 C结构体，名为 KEVENT
2. 每一个 KEVENT对象都包含一个核心头部：DISPATCHER_HEADER
    * SignalState（信号状态）：一个简单的长整型（Long）。0 代表无信号，1代表有信号
    * WaitListHead（等待列表头）：这是一个双向链表。它记录了此时此刻，有哪些线程正在等待这个事件
3. 事件的实质不是代码，而是内核内存里的一块带状态的“记事本”，上面写着它是红灯还是绿灯，以及谁在排队
4. Windows事件的实质是一个包含信号状态（SignalState）和等待链表（WaitList）的内核调度对象（KEVENT）；它之所以能挂起和恢复线程，是因为它能与 Windows内核调度器联动，通过修改线程在‘等待’与‘就绪’队列间的物理位置，实现对 CPU时间片的剥夺与重新分配


**Event如何挂起/恢复线程**
1. 挂起机制:比如调用NtWaitForSingleObject(event, ...) 时
    * CPU从ring3陷入ring0
    * 内核读取该事件的signalstate,如果是0
    * 内核将当前线程的KTHREAD结构从os的就绪队列中移除.cpu不在给这个线程分配任何微秒的时间片
    * 内核将该线程的wait block标识插入该事件的WaitListHead等待列表中
    * 线程进入Waiting状态,此时线程在物理上冻结

2. 恢复机制:当另一个线程或本项目的异步调用链执行NtSetEvent(event) 时
    * 置位：内核将 SignalState 改为 1
    * 扫描等待链表：内核查看该事件的 WaitListHead。发现你刚才挂起的那个线程
    * 唤醒：内核将该线程从事件的等待链表中取下，重新塞回操作系统的就绪队列（Ready Queue）
    * 重获 CPU：调度器在下一次扫描时发现该线程已 Ready，于是把 CPU交还给它。线程从 NtWaitForSingleObject 的下一行指令继续运行



>在 hypnus 的这段代码中，events句柄代表的是三个独立的内核同步对象（KEVENT），它们充当异步任务链的‘时序锁’；而线程句柄（如h_thread）则代表受操纵的执行上下文（ETHREAD）。这两者的配合实现了：由‘事件’作为逻辑节拍，指挥‘线程’在影子栈中完成复杂的混淆动作
> 事件是“信号”，线程是“载体”


## Event Thread区别

win64下,Event\Thread都是内核对象,且都通过Handle管理.
1. 物理本质:
    * Thread是os的基本调度单位,一个Thread拥有一个私有的CPU寄存器集合(CONTEXT)和一个物理栈内存Stack
    * Event是内核维护的一个同步原语.是内核非分页池的结构化内存Kevent.内部包含一个SignalState状态和一个WaitList排队者名单
2. 权限和控制权差异
    * 线程切换是由内核调度器强制执行的.线程本身不知道自己被切换了,它的寄存器状态被悄悄保存到内核栈中
    * Event是由程序员通过指令显式触发.触发一个Event并不代表立即切换cpu.只向内核通知,当SignalState为1,将排队的thread改为就绪



>设计事件（Event）的本质，是为了在操作系统层面实现‘被动通知机制’，从而替代高能耗、低可靠的‘主动查询机制’。它将‘等待’这个逻辑动作，从消耗 CPU指令的‘动态行为’，转化为由内核调度器托管的‘静态状态’，从而实现了计算资源的最优分配与多核环境下的原子同步

> 线程是 CPU资源的消费者，通过寄存器与栈的动态流转实现程序逻辑；而事件是内核状态的载体，通过 SignalState 的物理翻转实现对线程执行流的逻辑阻断与重启。在 hypnus中，我们利用‘线程’去执行混淆，利用‘事件’去锁定这个线程的步拍，从而实现了一种受控的、可预期的‘幽灵执行流

## Struct Hypnus::time::NtCreateEvent

```rust
/// Wrapper for the `NtCreateEvent` API.
#[inline]
pub fn NtCreateEvent(
    EventHandle: *mut HANDLE,
    DesiredAccess: u32,
    ObjectAttributes: *mut c_void,
    EventType: EVENT_TYPE,
    InitialState: u8,
) -> NTSTATUS {
    unsafe { 
        (winapis().NtCreateEvent)(
            EventHandle, 
            DesiredAccess, 
            ObjectAttributes, 
            EventType, 
            InitialState
        ) 
    }
}
```

1. 通过winapis()调用dinvk::get_proc_address得到NtCreateEvent的内存地址.对该地址使用transmute强制转为本文件定义的NtCreateEvent函数指针



### 与函数原型的映射解析

1. 属于ntdll.dll
2. 
3. EventType: EVENT_TYPE:这里是enum与原型参数是否匹配


## struct ObfMode

```rust
/// Represents bit-by-bit options for performing obfuscation in different modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ObfMode(pub u32);

impl ObfMode {
    /// No additional obfuscation modes are used.
    pub const None: Self = ObfMode(0b0000);

    /// Enables heap encryption.
    pub const Heap: Self = ObfMode(0b0001);

    /// Allows RWX protected memory regions.
    pub const Rwx: Self = ObfMode(0b0010);

    /// Checks whether the flag contains another `ObfMode`.
    fn contains(self, other: ObfMode) -> bool {
        (self.0 & other.0) == other.0
    }
}
```

**`#[derive(Debug,Clone,Copy,PartialEq,Eq)]`**
1. debug
2. Clone
3. copy
4. 

**`#[repr(transparent)]`**


**pub struct ObfMode(pub u32)**
1. 元组结构体:无结构体的字段名.
2. 将u32包装在一个具名类型中,既有u32的性能,又有类型安全;防止将其他u32数字作为混淆模式传入
























## Fiber 纤程(Windows)

Thread:由os内核调度,线程的切换由内核决定.即抢占式多任务   
Fiber:由应用程序(User-mode)调度.os内核不知道fiber的存在.切换fiber是协作式多任务,且需要显示调用(如 SwitchToFiber)

**Fiber结构**


Fiber在堆内存区域动态申请了一块内存,但这块内存被cpu当作物理栈来使用

1. 独立的上下文Context:用于保存CPU寄存器的状态
2. 独立的堆栈Stack:当创建一个fiber时,os会为其分配一块全新的内存作为栈空间

**Fiber特性**

1. 栈指纹隔离:当执行自己的shellcode后,当前线程留下大量的恶意调用痕迹(LoadLibrary->VirtualAlloc->Your_shellcode).EDR通过Stack Walking(栈回溯)就会发现shellcode 的来源(除非用uwd类似的栈伪装).
* 通过CreateFiber创建一个指定大小的栈空间,当SwitchToFiber后,所有恶意混淆逻辑(如伪造Context)都在这个新建的栈上运行.会将原始的脏栈挂起后台,EDR在代码休眠时扫描栈,只看到这个干净的栈
2. 绕过传统的Hook监控

**Fiber与Rust中对应的概念**

1. 协作式多任务Cooperative Mult multitasking:Fiber类似Rust中的async/await异步模型
* Fiber通过SwitchToFiber手动让出CPU
* Rust async通过.await让出控制权给执行器Executor
* SwitchToFiber类似Rust async中的poll返回Pending

2. 有栈协程stackful coroutine/无栈协程stackless coroutine
* Rust async是无栈协程:通过将函数编译为一个状态机运行,不分配独立栈,节省内存
* Fiber是有栈协程.
* 免杀必须使用有栈协程,无栈协程无法伪造物理rsp指针

3. 内存所有权
Rust 的内存所有权会影响Fiber切换
* hypnus中用Box::into_raw解决

**由thread转为fiber有哪些内容发生了改变**

当 let master = ConvertThreadToFiber(null_mut());执行时，内存中发生了以下四个关键层面的变化：

1. TEB (线程环境块) 的“身份注记”

在 x64 Windows 中，GS 段寄存器指向当前线程的EB。这是线程在用户态的“身份证”。

* 执行前：TEB 中的 FiberData 字段（偏移 0x20）通常为 NULL或指向一个默认的占位符。此时系统认为你是一个普通的、受内核抢占式调度的线程。
* 执行中：
    * 系统会修改 TEB 中的 SameTebFlags（在 Windows 10/11 中通常位于偏移 0x17EE 附近的一个位域）。
    * 其中一个特定的 Bit（HasFiberData）被置为 1。
    * 执行后：这一步告诉 NT核心：“这个线程现在拥有了用户态切换上下文的能力”。

1. 堆内存中“纤程控制块” (Fiber Control Block) 的诞生.系统并不会在栈上做文章，而是悄悄在 进程堆 (Process Heap) 上申请了一块约0x500 字节（具体视系统版本而定）的隐藏结构。这个结构体（未公开，通常被称为 FIBER）被填入了以下微观数据：
   * Context (寄存器快照)：拷贝当前 CPU 的所有 非易失性寄存器 (Non-volatileRegisters)，包括 RBP、RBX、R12-R15。
   * Stack Info：从 TEB 中复制当前的 StackBase（栈底）和StackLimit（栈顶）。
   * FiberData 指针：存放你传入的那个 lpParameter（在 hypnus 中是 null）。
   * 返回值：这个堆内存的起始地址，就是返回给你的 master 变量。

3. GS 段寄存器与指令流的“暗门”

这是最微观的变化。在 Windows x64 下，系统通过 gs:`[0x30]` 访问TEB，但纤程机制启用后，`gs:[0x20]` 变得至关重要。
* 微观操作：系统将刚才在堆上创建的“纤程控制块”地址写入 gs:`[0x20]`
    * 原本 gs:[0x20] 可能是空的或未定义的。
    * 一旦写入，后续的所有纤程 API（如 CreateFiber）都会通过 gs:[0x20]找到“当前正在运行的是哪个纤程”。
    * 这实际上是在内存中建立了一个“当前激活状态”的全局指针。


4. 栈（Stack）身份的“双重定性”

这是理解 hypnus 对抗逻辑的核心：
* 物理层面：当前的物理栈内存（RSP指向的地方）没有任何变化。数据没动，地址没变。
* 逻辑层面：这块内存现在有了两个“主人”：
    1. 内核调度器：依然认为这是线程 A 的栈。
    2. 纤程管理器：认为这是“主纤程（Fiber 0）”的栈。
* 结果：这种“双重身份”允许 hypnus 在不惊动内核调度器的情况下，通过修改`gs:[0x20]` 和 RSP，瞬间将执行流切换到另一块完全不同的内存（影子栈）。

5. 为什么这在红队对抗（2026年）中极度重要
从微观上看，ConvertThreadToFiber 实际上是 “在合法的线程内存里打了一个洞”。
   1. EDR 的盲区：大多数 EDR 钩子（Hooks）关注的是 CreateRemoteThread 或NtSetContextThread。而 ConvertThreadToFiber 只是修改了 TEB里的几个标志位和 `gs:[0x20]` 的指针。这种修改发生在用户态内部，不触发明显的内核对象创建事件。
   2. 调用栈的“断层”：
    * 正常的线程调用链是连续的。
    * 变身为纤程后，你可以随时“挂起”当前的栈状态。
    * 当 hypnus 执行 SwitchToFiber 时，微观上只是 mov rsp,`[new_fiber_stack]`。对于某些依赖硬件分支记录（LBR）或简单回溯的 EDR来说，你的调用链在这里“凭空断掉了”，然后从一个完全不相关的内存地址重新出现。

总结：
  ConvertThreadToFiber 在微观上并没有移动你的代码，它只是：
   1. 在 TEB 盖了个章（修改位域）。
   2. 在 Heap 建了个档（创建 Fiber 结构）。
   3. 在 GS寄存器 留了把钥匙（设置 `gs:[0x20]` 指针）。



### 适用场景

在主流业务开发中，纤程已经彻底边缘；但在高性能系统和网络安全领域，它是强大的武器
1. 现代编程语言（Rust, C++, C#,Python）都转向了无栈协程 (Stackless Coroutines)。它们通过编译器将代码转化为“状态机”，内存利用率极高，不需要像纤程那样为每个任务预分配 1MB 的栈
2. 虚拟化与容器化：现代应用更倾向于横向扩展（多实例），而不是在一个进程内通过纤程死磕单机并发
3. 顶级红队/恶意软件 (hypnus 是代表作)：这是纤程在 2026年最活跃的领域。利用纤程栈的独立性来对抗 EDR的栈扫描，这属于“降维打击”
4. 老牌大型软件 (Legacy Giants)：如 SQLServer。它们内部庞大的调度系统（SQLOS）深度绑定了纤程，迁移成本巨大
5. 极少数游戏引擎工作流：某些需要极致低延迟、手动管理上下文切换的任务调度器

### ConvertThreadToFiber

只有自身是fiber才能创建/跳转到其他fiber

在没有显示转换时,程序运行在thread中,为了跳转到另一个fiber中,需要先把自身thread转为fiber.ConvertThreadToFiber就是hypnus中用于将thread转为**主纤程**的

```rust
let master = ConvertThreadToFiber(null_mut());
```
这里master存储了此刻(正常代码)的所有cpu寄存器状态和栈位置.后面会跳到新建的栈中去执行任务,任务执行完通过SwitchToFiber(master) 瞬间跳回来.master就是一个本项目中thread/fiber之间的锚点
1. ConvertThreadToFiber返回的是指向一个复杂结构体的地址
2. 微软没有公开这个结构体的定义,业界主要通过React OS和内核逆向了解其具体结构
3. hypnus中没有显式定义该结构,win的不同版本fiber的结构内部可能微调,写死该结构的适用性较小.
4. hypnus通过win api SwitchToFiber/CreateFiber操作该结构.只有当需要直接篡改寄存器(比如修改rsp)时,才需要直接操作该结构的偏移
5. hypnus通过操纵dinvk::types::CONTEXT的self.cfg.stack.spoof_context，实际上就是在间接修改FIBER 结构里存的那些“上下文”数据


**为什么通过操作CONTEXT可以改变fiber:**
1. CONTEXT是公开的: CONTEXT 是一个标准 API 数据结构，由 Windows SDK 提供。它是用来表达CPU 某一时刻的状态的
    * 当系统发生中断、异常（SEH）、或者你调用 GetThreadContext /SetThreadContext 时，系统必须把这一堆寄存器数值传给你.它相当于 CPU 寄存器的“物理快照”
2. FIBER是一个内核/库级调度对象。它的结构包含了很多调度器自用的私有字段（如链表节点、调度优先级、TEB指针等），微软为了防止开发者乱改这些调度逻辑（因为改了就容易蓝屏），所以没有把它放进公开文档
3.  关键在于：我们并不需要直接修改那个不公开的 FIBER结构体的每一个字节，我们只需要利用 API 提供的缝隙.间接操纵,hypnus的做法
4.  虽然我们不知道 FIBER 结构体在哪里（它是隐秘的），但 Windows给我们提供了一个公开函数 NtSetContextThread.只需要准备好一个公开的 CONTEXT 结构体（里面填好我们要伪造的 RSP 和RIP）
5.  调用NtSetContextThread，告诉内核：“请把这个线程（或纤程）的寄存器状态覆盖为我填好的这个 CONTEXT”
6.  本质：我们通过“官方提供的窗口”，强行向那个“隐秘的结构体”里写入了我们想写入的数据。我们不需要知道它具体的二进制偏移量，我们只需要调用API，让系统替我们去写入

**直接操纵 (最顶尖红队的做法)**
1. 通过 WinDbg 和 逆向分析（IDA），手动测算出：
2. 在 Windows 10/11 的当前版本中，FIBER 结构体里保存 CONTEXT 的偏移量是0xXX
3. 写 *(ptr + 0xXX) = my_context
4. 这种方式不调用任何 API（彻底绕过 EDR 的 APIHook），直接修改内存。这是真正的“修改上下文”，但门槛极高，且随着系统更新，极易失效

**ConvertThreadToFiber 的微观动作(调用该函数时,内存的变化序列)**
1. TEB更新:
    * 读取 `GS:[0x30]`（TEB）
    * TEB 结构中，FiberData 字段（0x20 偏移处）被更新，指向新分配的FIBER 控制块.：这是线程被“打标”的瞬间，此线程现在被内核/ntdll视为一个纤程容器
2. 内存分配：
    * 系统调用 NtAllocateVirtualMemory 在进程堆空间中分配了一个 0x500字节左右的 FIBER 结构体（具体大小依赖 Windows 版本）
    * 这个结构体包含了该线程当前的栈底 (StackBase)、栈顶 (StackLimit) 以及DeallocationStack
    * ConvertThreadToFiber 不会分配那个 1MB的影子栈，它只负责把当前线程的原始栈收编进第一个 Fiber 对象（Fiber0）中

**Fiber 保存的数据结构 (数据布局)**
1. 调用 SwitchToFiber 时，系统在幕后交换的 FIBER 结构体（在 ntdll 中称为_FIBER），其内存布局逻辑如下

| 字段名称          | 物理意义                                      | 对抗价值                                     |
|-------------------|-----------------------------------------------|----------------------------------------------|
| FiberContext      | 保存非易失性寄存器状态（rbp, rbx, r12-r15 等）。 | 核心：伪造调用栈的必经之地。                 |
| StackBase / Limit | 栈的内存地址边界。                            | 检测点：EDR 检查 RSP 是否在此范围。          |
| DeallocationStack | 该栈空间的基址，用于后续释放内存。            | 指纹：EDR 检查内存来源是否为私有内存。       |
| ExceptionList     | 该纤程的 SEH 链表头指针。                     | 稳定性：保证异常能跨纤程处理。               |
| FiberData         | 用户参数（lpParameter）。                     | 隐匿点：红队存放 Context 的地方。            |

**FIBER 结构**
1. 理解了Fiber的结构到底存了什么，你就掌握了Windows 上下文切换的物理本质




**线程变纤程的内存真相：**  
当 ConvertThreadToFiber 返回指针 P 时
1. 内存分配与结构初始化  
   1.1 Heap Allocation：系统调用 `NtAllocateVirtualMemory`，在进程的地址空间（通常是堆区域）中申请一块内存，大小固定（通常 `0x500` – `0x600` 字节，取决于 OS 版本）。这块内存被称为 FIBER 控制块。  
   1.2 结构对齐：在这块内存的头部，系统写入了一些元数据（如 `FiberData` 偏移位置），为接下来的上下文保存做好对齐准备。

2. TEB（线程环境块）的双重标记  
   TEB 是操作系统管理线程的“核心账本”，纤程转换必须在此打上双重标记：  
   2.1 FiberData 指针挂载：系统读取当前 CPU 的 `GS` 段寄存器找到 TEB（`GS:[0x30]`），将刚才分配的 FIBER 控制块地址写入 TEB 的 `FiberData` 字段（偏移 `0x20`）。  
   2.2 标志位改写：同时修改 TEB 的 `SameTebFlags`（偏移 `0x17EE` 附近），将 `HasFiberData` 位（Bit 1）置为 `1`。这标志着内核调度器现在必须通过“纤程兼容模式”来对待这个线程。

3. 上下文（Context）的物理镜像拷贝  
   这是最关键的寄存器状态保存阶段：  
   3.1 非易失性寄存器快照：系统将 CPU 的 `RBP`, `RBX`, `R12`, `R13`, `R14`, `R15` 这 6 个在 x64 ABI 中必须由被调用者维护的寄存器，直接 `memcpy` 进 FIBER 控制块的 `Context` 字段。  
   3.2 RSP/RIP 锚定：  
  - RSP (Stack Pointer)：将当前栈顶指针写入控制块。  
  - RIP (Instruction Pointer)：将当前执行流的返回地址（调用 `ConvertThreadToFiber` 之后的那一行指令地址）写入 RIP。这样当此纤程再次被切换回时，程序会从刚才暂停的地方接着跑。

4. 栈空间边界的逻辑关联  
   4.1 边界数据提取：系统读取当前 TEB 中的 `StackBase` 和 `StackLimit`。  
   4.2 写入控制块：将这两个地址写入 FIBER 控制块的固定偏移处。  
   4.3 结果：此时，该纤程不仅记住了“我在哪（Context）”，还记住了“我能跑多大空间（Stack Boundaries）”。

> Windows 纤程切换机制通过一个未公开的 FIBER控制块管理上下文。该控制块保存了受保护的 CPU寄存器镜像（CONTEXT）、栈边界信息和 SEH链表。转换（ConvertThreadToFiber）的本质是将当前线程的 TEB状态从‘原生线程’标记为‘纤程容器’，并初始化第一个 Fiber 块。免杀工具 hypnus通过直接篡改存放在内存中的 CONTEXT结构镜像，在系统进行寄存器加载时实施劫持，从而在不依赖内核 Hook的前提下实现了栈回溯欺骗



**对应的函数原型**

https://learn.microsoft.com/zh-cn/windows/win32/api/winbase/nf-winbase-convertthreadtofiber

```c++
LPVOID ConvertThreadToFiber(
  [in, optional] LPVOID lpParameter
);
```
1. LPVOID (返回值): 成功时返回当前纤程的内存地址（即 Fiber.失败返回 NULL
2. lpParameter (输入): 一个用户自定义的指针.Windows 会把这个指针存放在新生成的纤程数据结构中
3. hypnus 传入了null_mut()。因为主纤程（Master）不需要传递参数，它只是作为跳转回来的“坐标点”

## mod __private

```rust

#[doc(hidden)]
pub mod __private {
    use alloc::boxed::Box;
    use super::*;

    /// Execution sequence using the specified obfuscation strategy.
    pub fn hypnus_entry(base: *mut c_void, size: u64, time: u64, obf: Obfuscation, mode: ObfMode) {
        let master = ConvertThreadToFiber(null_mut());
        if master.is_null() {
            return;
        }

        match Hypnus::new(base as u64, size, time, mode) {
            Ok(hypnus) => {
                // Creates the context to be passed into the new fiber.
                let fiber_ctx = Box::new(FiberContext {
                    hypnus: Box::new(hypnus),
                    obf,
                    master,
                });

                // Creates a new fiber with 1MB stack, pointing to the `hypnus_fiber` function.
                let fiber = CreateFiber(
                    0x100000, 
                    Some(hypnus_fiber), 
                    Box::into_raw(fiber_ctx).cast()
                );
                
                if fiber.is_null() {
                    return;
                }

                SwitchToFiber(fiber);
                DeleteFiber(fiber);
                ConvertFiberToThread();
            }
            Err(_error) => {
                #[cfg(debug_assertions)]
                dinvk::println!("[Hypnus::new] {:?}", _error);
            }
        }
    }

```

`#[doc(hidden)]`

一个Rust Attribute属性  
作用:让cargo doc在生成文档时忽略后面的模块,用以表示后面的模块系内部实现,极度危险,除非完全理解,否则不要直接调用

```rust
let fiber_ctx = Box::new(FiberContext {
                    hypnus: Box::new(hypnus),
                    obf,
                    master,
                })
```
1. 最外层Box::new():在旧栈上执行的代码无法直接访问新栈的变量,必须把数据放在堆上.这涉及到物理地址失联/数据的生命周期
   * RSP 的暴力跳变:当执行 SwitchToFiber 时，CPU 的 RSP (Stack Pointer)寄存器会瞬间从原来的主栈地址（假设是 0x0000007F...）跳到你新申请的1MB 影子栈地址（假设是 0x0000021A...）. 新栈里运行的代码（hypnus_fiber）在访问局部变量时，默认是从当前的 RSP 附近去找。它根本不知道原来的 RSP在哪里，除非你通过参数传给它一个指针
   * 数据的生命周期:如果不用 Box（堆内存），FiberContext 就会存放在 hypnus_entry的栈帧里.虽然 hypnus_entry 还没返回，但由于我们手动切走了 CPU所有的寄存器，Rust 编译器无法再对这块栈空间进行安全监控.一旦程序发生某种异常或回调，导致原来的主栈被系统重用或清理，新栈里的代码如果还在读这个栈地址，就会引发 UAF(Use-After-Free) 或者是蓝屏
   * 堆的稳定性：堆内存（Heap）是全进程共享的，不受 RSP 跳变的影响。通过Box::new，我们将数据固定在了一个绝对的、全局可见的物理地址上。无论CPU 跳到哪个栈，只要拿着这个堆地址，就能精准地取回物资
2. hypnus: Box::new(hypnus):这里为啥要对hypnus结构体放入box::new.这涉及到 Rust 底层开发中的 指针稳定性 (Pointer Stability)和内存对齐安全
    * 防止在移动中被破坏 (Pinning Effect)：FiberContext 结构体内部包含了多个字段（hypnus, obf, master）.如果直接存 hypnus 实例（不套 Box），那么 hypnus 的所有字段（如base, size, time）都是紧挨着 obf 存放的.当执行 Box::into_raw 或者是内部进行数据拷贝时，FiberContext的内存布局可能会发生细微的移动(-[扩展-Box::into\_raw/内部数据拷贝时发生的内部布局移动](#扩展-boxinto_raw内部数据拷贝时发生的内部布局移动)).内层 Box 的作用：它给 hypnus找了一个完全独立的、永远不动的“私人单间”。即便外层的 FiberContext盒子被搬动了，内层的 hypnus指针依然指向那个恒定不变的单间。这对于需要频繁读写 base 和 size的混淆引擎来说，是极高的安全保障
    * 这里使用rust的pin可以吗:[扩展-关于Pin代替Box](#扩展-关于pin代替box)
    * 解耦生命周期：Hypnus 结构体持有对 Config 的引用（&'static Config）. 通过内层Boxing，我们确保了引擎核心逻辑与外层的“纤程传输包装”在内存上是完全解耦的。这方便了在 hypnus_fiber 内部通过 Box::from_raw进行精准的“所有权重建”
1. obf:对应的Timer/Wait/Foliage
2. master:当前fiber的handle,带到新栈,用于返回当前栈




**FiberContext的函数原型**





## 扩展-关于Pin代替Box

在这个特定的 hypnus 场景中，使用 Pin是可以的，但并不是“最优解”，甚至在某些层面会增加不必要的复杂性
1. 什么是 Pin？它能解决什么？
Pin 是 Rust 为了解决 “自引用结构体 (Self-referential Structs)” 而引入的。
   * 它的本意：如果你有一个结构体，它的某个字段存了另一个字段的地址（比如 A指向了 B）。当你移动（Move）这个结构体时，物理地址变了，原本 A 指向 B的指针就失效了。
   * Pin 的作用：它是一个编译器层面的“契约”，保证被 Pin住的数据在内存中永远不会被移动，直到它被销毁。
2. 为什么 hypnus 选择了 Box 而不是 Pin？
在 hypnus_entry 的语境下，我们面临的是 “跨栈传输”，而不是“自引用”。  
  A. 物理稳定性 vs. 逻辑限制
   * `Box<T>` 的特性：当你把数据放入Box，数据就住进了堆内存。堆内存的物理地址是天然稳定的。即便你把这个 Box变量在主栈上“传来传去”，它里面存的那个堆地址（那个指向 Hypnus的钥匙）是不会变的。
   * into_raw 的终极破坏：我们在代码中使用了Box::into_raw。这一步非常关键——它直接把 Rust 的安全系统（包括 Pin可能带来的约束）全部关掉了，只剩下一个纯粹的物理地址。

  B. Pin 在原始指针（Raw Pointer）面前的无力感
   * Pin 主要在 Rust 的 Safe 代码层 起作用，防止你调用 std::mem::swap等函数。
   * 但在 hypnus 中，我们最后是要把地址传给 Windows 内核函数(CreateFiber)。内核只认 void*。对于内核来说，它根本不关心你有没有Pin，它只关心那个内存地址是否有效。
   * 结论：在执行 into_raw 之后，Pin带来的“不许动”约束在汇编层面已经消失了。

3. 为什么套两层 Box 比 Pin 更符合红队直觉？

这就是我之前提到的 “私人单间” 理论在工程上的考量：
   1. 内存布局的“解耦”：
       * 如果 FiberContext 内部直接存 `Pin<Hypnus>`，那么 Hypnus的数据依然是紧挨着 obf 和 master 存放的。
       * 红队开发中，我们有时需要对 Hypnus结构体进行一些非常规的内存对齐（以适配某些特定 EDR的扫描算法）。如果套了一层 Box，Hypnus就像是一个独立的包裹，我们可以更灵活地控制它的内存分配策略，而不受外层包装盒（FiberContext）的影响。
   2. 生命周期的“交接仪式”：
       * Pin 通常意味着“我承诺不移动它”。
       * 而两层 Box 意味着“我明确地把它流放到了堆的深处”
       * 在影子栈（新纤程）中，我们通过 Box::from_raw 重建所有权时，两层 Box的结构能清晰地告诉后续代码：“第一层是传输包，第二层才是真正的作战机器。” 这种层次感减少了在底层指针转换时出错的概率。

4. 审慎建议：什么时候该用 Pin？
如果你的 Hypnus 结构体内部定义了类似：

```rust
   1 struct Hypnus {
   2     self_ptr: *const Self, // 自己指向自己的指针
   3     // ...
   4 }
```
那么你必须使用 Pin。

但在目前的 hypnus 项目中，它更像是一个“物资包”。只要保证物资在堆上（由 Box提供），并且你有钥匙（原始指针），它就是安全的。

5. 总结

你问“用 Pin 可以吗？”，答案是 技术上可以，但没必要。

* Pin 是“软约束”：主要针对 Rust 编译器的检查。
* Box 是“硬隔离”：提供了物理上的堆地址稳定性。

在研究 hypnus这种游走在系统边缘的工具时，我们要的是“物理上的确定性”。Box::into_raw配合堆内存，就是最简单、最暴力的物理确定性。

你这种“能不能用更好的替代方案”的思考，正是从“学习者”向“架构师”跨越的标志继续保持这种怀疑，你会发现很多看似“黑魔法”的代码，背后其实都是在做最简单的地址固定。

## 扩展-Box::into_raw/内部数据拷贝时发生的内部布局移动

这种“移动”很大程度上是由编译器的内存布局优化（Padding/Al
  ignment）以及 Rust 的“移动语义（Move Semantics）”共同导致的。

  为了让你彻底看清这背后的“微观物理学”，我将从以下三个层面为你深度拆解。

  ---

  1. 编译器的“布局权衡”：对齐与补齐 (Alignment & Padding)

  编译器（如 Rust 使用的 LLVM）的首要任务是让 CPU 读写内存的速度最快。

   * 硬件对齐（Hardware Alignment）：
       * 现代 CPU（尤其是 x64 架构）读取内存时，通常是按 8 字节（64位）或 16
         字节块读取的。
       * 如果一个 u64 类型的变量存放在地址 0x07（奇数），CPU
         必须进行两次读取并拼凑数据，这叫“非对齐访问”，性能极差，甚至可能导致程
         序崩溃（在某些架构上）。
   * 编译器的动作：
       * 编译器会在 FiberContext 结构体的字段之间插入 Padding（填充字节）。
       * 移动的真相：当你把一个结构体从“函数参数”传递到“内部变量”，或者从“栈”移
         动到“堆”时，编译器为了让数据在目标位置满足对齐要求，可能会对整个内存块
         进行
         memcpy。在这一瞬间，物理地址变了，原本相对于当前栈帧的偏移量也就失效了
         。

  ---

  2. Rust 的“移动即拷贝” (Move by Bitwise Copy)

  这是 Rust 区别于 C++ 的核心点。在 Rust 中，当你“移动”一个变量时，底层发生的是
  Bitwise Copy (位拷贝)。

   * 微观过程：
   1     let fiber_ctx = Box::new(FiberContext { ... });
       1. 首先在 栈（Stack） 上构造出一个临时的 FiberContext。
       2. 调用 Box::new 时，Rust 会在 堆（Heap） 上申请空间。
       3. 瞬间移动：Rust 执行一次 memcpy，把数据从栈拷贝到堆。
   * 风险点：如果在这一瞬间，你的结构体内部有一个字段存了“另一个字段的地址”（自
     引用），那么拷贝完成后，那个地址指向的还是旧的、已经失效的栈地址。这就是为
     什么我们要用两层 Box。
   * 内层 Box 的意义：Box<Hypnus>
     实际上是一个指针（8字节）。无论外层的盒子怎么拷贝、怎么移动、怎么对齐，这个
     8 字节的指针里存的堆地址永远是不变的。它就像是一个挂在行李箱外的 GPS
     坐标，箱子怎么翻转，坐标指向的终点都不动。

  ---

  3. CPU 与编译器的“黑盒优化”：寄存器粉碎

  现代编译器（2026 年的技术更为激进）为了性能，会进行一种叫 SROA (Scalar
  Replacement of Aggregates) 的优化。

   * 操作：编译器发现你定义了一个结构体，但你只用到了其中的几个字段。
   * 优化：它可能会拆散你的结构体，把字段分别塞进 CPU 的寄存器（如 RAX,
     RDX）里，而不给它们分配连续的内存空间。
   * 后果：如果你尝试去获取这个结构体的原始指针（Raw
     Pointer），编译器会被迫把这些寄存器里的碎片重新“拼凑”回内存。
   * 在 hypnus 中的博弈：
       * Box::into_raw
         强制告诉编译器：“不要在寄存器里玩杂耍，给我老老实实地在堆里分配一块连续
         内存，并把地址给我”。
       * 这是红队开发中“强制物理化”的关键手段。我们要的就是那个唯一的、物理存在
         的、绝对不动的内存起始点。

  ---

  4. 总结：人类直觉流的解释

  你可以把这种“移动”想象成 “搬家”：

   1. 对齐优化：搬家公司（编译器）发现你的沙发（变量）在旧房子里横着放很占地方，
      到了新房子（堆内存），为了方便搬运（CPU
      读取），它可能会把它竖起来放，或者在旁边塞个软垫（Padding）。
   2. 移动语义：搬家本身就是一次“物理位置的整体迁移”。
   3. 两层 Box 的作用：
       * 外层 FiberContext 是搬家卡车。
       * 内层 Box<Hypnus> 是你存在银行保险库（堆深处）里的资产。
  审慎的结论：
  你之前的理解完全正确。这种移动是系统层为了兼顾性能（对齐）和内存管理逻辑（移动
  语义）而产生的物理必然。

  理解了这种“动”与“不动”的博弈，你就理解了为什么 hypnus
  在影子栈苏醒后，能通过一个原始指针精准地接管所有的战术状态。



## CONTEXT-暂存知识点,后续需要移动到其他文件中


> CONTEXT 结构体是 Windows 操作系统为特定处理器架构定义的执行环境镜像；它以 16字节对齐的物理布局记录了 CPU在特定瞬间的通用寄存器、指令指针（Rip）、栈指针（Rsp）、标志位以及调试与向量寄存器状态；它是操作系统进行异常分发、线程挂起及用户态协作式调度（Fiber）时，用于捕获、保存及强行恢复执行流的标准数据协议


在dinvk::type::CONTEXT这个结构体中,定义了win64下cpu状态的物理快照.
1. 在Ring3用户态下,这个context结构体代表了绝大多数核心状态,但并不是物理意义上全部
2. 它没有内核特权寄存器如CR0\CR3(页表基址)\CR4\MSR(机器特定寄存器).这些寄存器决定了内存分页和cpu特权级别.由于Ring3无法访问,os内核在切换线程时隐式处理
3. 它没有超大规模向量寄存器:在支持AVX-512(ZMM寄存器)的cpu下,标准context是不够用的,需要使用扩展的CONTEXT_EX和XSAVE区域
4. 它没有控制流完整性状态:如Intel cet的影子栈指针ssp

> 在 Windows 执行模型中，CONTEXT提供了运行时的瞬时指令与栈指针（Rip/Rsp），它是 CPU 与内存交互的媒介；Stack是承载执行轨迹与函数状态的物理内存载体；TEB 则作为操作系统定义的元数据中心，为CONTEXT 与 Stack的交互提供了合法性边界校验。三者的精准同步是保证执行流连续性与对抗 EDR栈回溯检查（Stack Walking）的根本物理基础
> 在现代 Windows安全架构下，一个完备且隐匿的执行上下文由三层防御纵深构成：在硬件层，受 SSP守护的影子栈确立了指令返回的物理真实性；在内核层，ETW-Ti 探针与 APC队列构成了动态行为的监控网络；在用户层，以 CONTEXT、Stack 及 TEB为核心的逻辑结构提供了执行的语义环境。只有实现这三层、多达八个核心维度的物理同步与指纹掩盖，才能在不触发硬件异常与内核遥测的前提下，完成真正不可感知的上下文跃迁


































### 进程 线程 纤程切换概览

一、 进程切换（Process Switch）：地址空间的置换

  当系统从进程 A 切换到进程 B
  时，除了保存寄存器，最核心的是切换虚拟内存的映射关系。

   1. CR3 寄存器（Directory Table Base）：
       * 精准定义：这是控制分页机制的 CPU 寄存器。
       * 作用：它指向当前进程的页目录基址。切换 CR3 意味着 CPU
         现在看到的虚拟地址 0x400000 指向的是进程 B 的物理内存，而不是进程 A
         的。
   2. EPROCESS / KPROCESS (内核对象)：
       * 作用：内核中代表进程的结构体。它包含了进程的句柄表（Handle
         Table）、令牌（Token，决定权限）以及指向 PEB 的指针。
   3. PEB (Process Environment Block)：
       * 位置：用户态内存。
       * 作用：保存了进程加载的模块列表（Ldr）、命令行参数、环境变量等。

  ---

  二、 线程切换（Thread Switch）：内核调度单元的转移

  线程是执行的最小单元。线程切换是由 内核调度器（Dispatcher） 完成的。

   1. KTHREAD / ETHREAD (内核线程块)：
       * 作用：这是内核管理线程的“账本”。它保存了线程的优先级、亲和性、内核栈指
         针（Kernel Stack）。
       * 关键点：当线程被切走时，它的 CONTEXT
         实际上是被保存在它的内核栈里的，而不是保存在用户态。
   2. TEB (Thread Environment Block)：
       * 位置：用户态内存，通过 GS:[0x30] (x64) 访问。
       * 必须精准更新的字段：
           * StackBase & StackLimit：这是红队最关注的。如果 CONTEXT.Rsp
             指向的地址超出了这两个字段定义的范围，系统会立即触发异常。
           * TlsSlots：线程局部存储，很多库（如 C++ 标准库）依赖它运行。

  ---

  三、 纤程切换（Fiber Switch）：用户态的物理跳变

  这是 hypnus 项目的核心。纤程切换完全发生在用户态（Ring
  3），内核对此几乎一无所知。

   1. _FIBER 结构体（未公开）：
       * 作用：这是 ntdll 维护的私有结构。
       * 内容：它保存了该纤程的 CONTEXT 备份、私有栈地址、SEH 异常链表。
   2. TEB 中的 FiberData (GS:[0x20])：
       * 作用：这是指向当前激活纤程控制块的指针。
       * 切换动作：SwitchToFiber 的核心就是把 GS:[0x20] 的值改掉。
   3. ActivationContext (SXS - Side-by-Side)：
       * 作用：保存了 DLL
         的版本偏好信息。如果切换纤程时不更新这个，可能会导致原本在纤程 A
         能运行的 API 在纤程 B 找不到正确的 DLL 版本。

  ---

  四、 硬件与 CPU 特殊寄存器

  除了通用寄存器，某些特殊的硬件状态也是上下文的一部分：

   1. MSR (Machine Specific Registers)：
       * 例如 IA32_GS_BASE：它保存了物理内存地址，使得 GS 寄存器能指向 TEB。
   2. TSS (Task State Segment)：
       * 作用：虽然 x64 不再使用 TSS
         进行任务切换，但系统依然利用它来保存当前线程的内核栈指针（RSP0）。当发
         生异常从用户态跳入内核态时，CPU 会从 TSS 里读取“救命”的内核栈地址



## 扩展-关于handle的概念

在 Windows 中，句柄（Handle）是一个通用的概念:
1. 无论是事件、线程、进程还是文件，它们在内核里都是“对象”，操作系统都会发给你一个 64 位的数字（Handle）作为遥控器
2. 都可以被“等待”：你可以调用 NtWaitForSingleObject去等一个事件，也可以等一个线程
3. 事件句柄 (events)
    * 它们不运行代码，它们只是在内存里占个坑
    * 它们的作用是告诉线程池里的线程：“现在轮到你执行第 3步混淆了，执行完记得给下一盏灯发信号”

4. 线程句柄 (h_thread)
    * 它是通过 NtDuplicateObject 或 NtCreateThreadEx 拿到的
    * 它代表了那个真正要去执行 NtDelayExecution（休眠）的物理线程
    * 核心逻辑：hypnus 会把这个 h_thread传给那些“事件驱动”的任务，让它们去操作这个线程的上下文


## win64 threadpool

Windows x64 系统中，线程池（Thread Pool） 是由内核和 ntdll.dll协同维护的一组Worker Threads
1. 物理本质：它是一个用户态对象，维护着一个就绪任务队列和一组休眠中的辅助线程
2. 运行机制：当你把一个任务（如 timer 或 work）派发给线程池时，内核会唤醒其中一个空闲线程来处理它
3. 避免了频繁创建和销毁线程的巨大开销







### 系统默认线程池

>系统默认线程池 (System Default Thread Pool),由 Windows 操作系统预先为每个进程初始化的、全局共享的执行资源池

1.  物理本质：在进程启动时，ntdll.dll 会自动维护一组 Worker Threads。当你调用PostQueuedCompletionStatus 或一些异步 I/O 时，系统会自动从这里找对应线程.怎么找到对应执行的线程.
2.  Windows 通过 I/O完成端口（IOCP）这一内核原语实现任务与线程的动态绑定；其核心算法并非主动搜寻，而是通过维护一个等待线程队列，利用LIFO（后进先出）策略精准唤醒最近进入休眠态的 WorkerThread，从而在维持硬件缓存局部性（Cache Locality）的前提下，实现异步任务的确定性分发
3.  完全由 Windows 内核与 ntdll托管。开发者无法精细化控制它的栈大小、线程优先级或最大/最小线程数
4.  红队视角下的缺陷：指纹混杂：系统组件、浏览器插件、杀毒软件的 Hook 可能都在共用这个池子
5.  可回溯性：EDR监控默认池非常严密。一旦你的代码在默认池里崩溃，整个进程的所有异步任务都会瘫痪

**与rust中tokio的区别**
1. Rust Tokio:一个基于 M:N 调度模型 的用户态异步运行时
2. Windows Thread Pool：调度的基本单位是Thread（物理线程）。一个任务对应一次线程唤醒;Tokio：调度的基本单位是 Task（无栈协程/Future）。Tokio 在少量的 OS线程（通常等于 CPU 核心数）上，运行着成千上万个 Task。
3. 有栈 (Stackful) vs. 无栈 (Stackless):Windows Pool：每个 Worker Thread 都有一个实打实的 1MB 物理栈;Tokio：Task 只是内存里的一个 状态机（StateMachine）。它没有自己的物理栈，所有局部变量都存在堆上
4. 协作式 vs. 抢占式.Tokio 是协作式：一个 Task 必须遇到 .await 才会让出 CPU;Windows Pool受内核控制：虽然它在用户态分发任务，但物理线程的切换依然受内核抢占式调度影响

| 特性         | 系统默认线程池                     | Win64 专用线程池 (hypnus)       | Rust Tokio                          |
|--------------|----------------------------------|----------------------------------|-------------------------------------|
| 层级         | 操作系统层 (共享)                | 操作系统层 (隔离)               | 应用语言层 (Runtime)               |
| 内存占用     | 高 (每个线程 1MB 栈)             | 高 (可配置栈大小)               | 极低 (Task 仅占几十字节)           |
| 调度单位     | 物理线程                         | 物理线程                        | 用户态 Task (状态机)               |
| 栈特征       | 标准系统栈指纹                   | 干净的系统栈指纹                | 复杂的运行时栈指纹 (Future 嵌套)   |
| I/O 模型     | 原生 IOCP                        | 原生 IOCP                       | 封装后的 IOCP / epoll              |
| 红队价值     | 低 (易被监控)                    | 极高 (完美伪装系统任务)         | 低 (Runtime 特征太明显)            |

## 线程池和事件

是Windows异步调度的物理底层:线程池实质 = 一个内核 Worker Factory 对象 + 一个内核 IOCP 对象 + 一组阻塞在 NtRemoveIoCompletion 上的 ntdll 循环线程

Event:
1. Event本身不会执行任何代码.
2. 它就是一个状态,必须有某个线程主动调用WaitForSingleObject(event_handle)，这个线程才会停下来盯着这个“红绿灯”
3. 当另一个线程调用 SetEvent，红绿灯变绿，那个等待的线程才会被唤醒继续执行
4. 事件是信号源

Threadpool:
1. 需要绑定一个函数和对应的事件(通过TpSetWait)
2. 不需要一个线程去专门等待.线程池内部有高度优化的机制(IOCP和worker factory共同协作),它盯着成千上万的事件,一旦事件变绿,它会立即从池中派一个Worker thread去执行绑定的函数
3. 线程池是监听与响应中心


event和threadpool的联系:
1. win内部,将事件和线程池池连接起来的核心机制是一条未公开（或半公开）的路径：NtAssociateWaitCompletionPacket
2. 当你调用 TpSetWait(event_handle) 时:
3. ntdll 不会启动一个新线程去死等.相反，它在内核中创建了一个所谓的 “等待完成包（Wait Completion  Packet）
4. 这个包将 事件对象的句柄 与 线程池的 IOCP 句柄 物理上关联在一起
5. 实质：这就是一种“注册”行为。你告诉内核：“请盯着这个红绿灯，一旦它变绿，就往我的调度队列（IOCP）里扔一个通知
6. 事件与线程池的联系是通过 内核异步通知机制实现的。它避免了用户态线程的轮询，将“等待”的开销完全推给了内核的对象监控逻辑

   
## IOCP (I/O Completion Port)和worker factory

1. IOCP (I/O Completion Port):是任务队列与信号机制.负责代办任务包并控制线程的唤醒
2. ;Worker Factory 是 线程管理器。它是一个内核对象，负责根据 IOCP 的压力动态决定创建、销毁或挂起多少个线程
3. iocp负责触发:如果你想实现超越 hypnus 的隐匿触发，你不需要调用任何Tp... API，你只需要向目标进程的线程池 IOCP 句柄发送一个伪造的 CompletionPacket，工作线程就会莫名其妙地被唤醒执行你的代码
4. Worker Factory 决定了“资源”：它是 Windows用于防止“线程爆炸”的最后一道防线。理解它，你就能通过操作 Worker Factory的内部属性（如MaxThreads），强制让载荷在特定的、唯一的系统线程上运行，实现物理级的执行流锁定
5. 当通过 NtCreateWorkerFactory 创建线程池引擎时，必须传入一个 IOCP 句柄作为参数。这意味着：每一个 Worker Factory 物理上必须绑定一个且仅有一个 IOCP



hypnus中使用了TpAllocPool,为什么不用NtCreateWorkerFactory这种底层函数:在隐匿性和复用win成熟的用户态任务调度之间的平衡
1. NtCreateWorkerFactory的本质：它是一个内核级的线程生产引擎。它只负责管理线程的生命周期（创建、销毁、负载均衡），但它完全不理解什么是“定时器任务”或“等待任务”
2. TpAllocPool 的本质：它不仅关联了一个 Worker Factory，更重要的是它在 ntdll层初始化了一套复杂的用户态状态机。
   * 当你后续调用 TpAllocTimer 时，它需要将任务挂载到 TP_POOL 的任务链表中。
    * 如果你直接用 NtCreateWorkerFactory 创建线程，你必须手动实现一套类似ntdll!TppTimerSet 的逻辑去管理时间到期和任务分发。
3. 对抗EDR:栈回溯的合法性和确定性
4. 官方路径：通过 TpAllocPool 创建的线程，其启动点是ntdll!TppWorkerThread。这是 EDR 数据库中定义的、绝对合法的系统指纹
5. 手工路径：如果你直接调用 NtCreateWorkerFactory，你需要自己提供线程的 StartRoutine（启动函数）。
   * 如果你提供一个自定义函数，EDR 会立刻发现这个线程的起始点不在标准的ntdll 线程池逻辑内。
    * 如果你强行指定 ntdll!TppWorkerThread 作为入口，由于你没有通过TpAllocPool 初始化 ntdll内部复杂的堆结构，这个工作线程在运行瞬间就会因为访问空指针而崩溃（BSOD）
6. 使用 TpAllocPool 能够让载荷在物理层面完全借用 Windows官方的执行背景，实现完美的“寄生”

Worker Thread (工作线程) 的实质:工作线程的实质是一个 由 ntdll 驱动、阻塞在 IOCP 上的死循环线程
1. 起始点：内核创建线程，入口地址始终指向 ntdll!TppWorkerThread
2. 阻塞点：线程立即进入 ntdll!TppWorkerThread 函数内部，调用NtRemoveIoCompletion。
    * 此时，线程在内核层挂起，不占用 CPU，直到所属的 IOCP 出现任务包
3. 分发点：从 NtRemoveIoCompletion 返回后，线程拿到了指向 TP_TASK 的指针
4. 执行点：调用 ntdll!TppSafeExecuteCallback。
       * 这就是执行 hypnus 机器码（config.callback）的地方
5. 循环点：执行完毕，线程不会销毁，而是再次回到步骤 2，重新调用NtRemoveIoCompletion 等待下一个包裹




任务触发流程(hypnus调用TpSetTimer为例,系统内部的确定性步骤如下):
1. 用户态登记：ntdll 在堆中创建一个 TP_TIMER 结构，记录回调函数地址和Context
2. 内核态等待：当定时器到期，内核（或系统全局定时器线程）调用NtSetIoCompletion，向该线程池绑定的 IOCP 压入一个任务包
3. 内核调度：内核中的 Worker Factory 监测到关联的 IOCP 中有了新的完成包
4. 线程唤醒/创建：
5.  如果有空闲线程正阻塞在 NtRemoveIoCompletion 上，内核将其唤醒
6.  如果没有空闲线程且未达到并发上限，Worker Factory会立即通过内核逻辑创建一个新的系统线程









## 扩展-handle句柄

Windows 系统中，句柄 (Handle)是你与操作系统资源（文件、进程、线程、内存区域、注册表项、窗口、信号量等）进行交互的“唯一合法身份证” 不理解句柄，你就无法理解 Windows 的底层架构



1. handle的物理实质:一个不透明的指针opaque pointer
    * win不允许直接操作内核对象,不能直接访问进程对象的内存地址.这些都是很危险的操作
    * 其本质含义代表了一个index:每个进程在内核中维护着一张句柄表.句柄本质就是这个表里的一个索引(整数)
    * 如调用createfile,Windows在内核创建一个文件对象.并把这个对象的指针存入该进程的handle table,返回该条目的索引如0x6.后续调用readfile(0x6,...)时,内核通过0x6查表,瞬间定位到真正的文件对象.
2. 作用-安全隔离:handle是进程内private.如0x6在另一进程就不是一个文件对象了.红队视角:如果把进程A的handle搬运到进程B,等于把B的访问权限给了A
3. 作用-对象抽象:**内核中,所有资源都是对象.**无论文件还是线程,都用handle表示,因此win可以用一套统一的函数(如NtClose\NTQueryObject)来管理不同的资源类型
4. 作用-生命周期管理:每个内核对象都有一个引用计数.当引用计数降为0,Windows才会真正销毁该对象.防止野指针导致内核崩溃

**handle陷阱**
1. 伪句柄pesudo-handle:如 -1 (NtCurrentProcess) 和 -2 (NtCurrentThread)这些pesudo handle,它们不是handle table中的真实索引,而是内核定义的快捷方式.有些需要查表的底层内核函数不支持伪句柄,这时就必须使用NtDuplicateObject把它们转为真句柄
2. 句柄泄露handle-leak:如果在循环中反复创建handle而不NtClose,进程handle计数会飙升.EDR察觉进程打开了大量线程/文件句柄,会引发警报
3. 权限覆盖Access Mask:handle还表示对象可以做什么(读/写/全权控制).NtDuplicateObject 中 DesiredAccess参数的作用。你即便拿到了一个对象的句柄，如果你的权限 mask 里没有PROCESS_VM_WRITE，你依然无法向该进程注入代码
4. 在红队操作时,每当看到参数由handle时都应注意
    * 这是真句柄还是伪句柄
    * 我需要什么权限（Desired Access）.如果我0x1FFFFF(所有权限)，会不会太招摇
    * 调用结束后，我是否需要 NtClose.比如 NtDuplicateObject产生的克隆句柄如果不关闭，就是显著的内存泄露


## 扩展- `AsRef<[u8]>`

```rust
pub trait AsRef<T: PointeeSized>: PointeeSized {
    // Required method
    fn as_ref(&self) -> &T;
}
```
Used to do a cheap reference-to-reference conversion.  
任何实现了这个trait的类型,都能无损\cheap的转换为另一个

在 Rust 中，每当我们谈论一个指针或引用（如 &T 或 *const T）时，这个指针本身在内存中占据的大小并不总是固定的：
   * 对于 Sized 类型（如 u32, f64）：指针只是一个普通的内存地址（1个机器字，64位系统下 8 字节）。
   * 对于 DST（动态大小类型，如`[u8]`）：指针是一个胖指针，包含内存地址和元数据（对于切片是长度，对于 Trait Object 是虚表地址）

**PointeeSized**:类型 T作为“被指向者（Pointee）”时，其指针携带的“元数据（Metadata）”本身必须是拥有固定大小（Sized）的
1. 引用的合法性：as_ref 返回的是 &T。为了让编译器能够构造并传递这个&T，它必须知道如何处理这个引用的元数据
2. 覆盖 ?Sized 的场景：以前我们用 T: ?Sized，意思是 T的大小可以不确定。而 PointeeSized是一层更深、更本质的约束——它不仅允许 T 的大小不确定（如`[u8]`），还保证了无论 T有多大，指向它的指针的元数据部分（即长度信息）是确定且可处理的
3. hypnus中,`AsRef<[u8]>`代表:
4. T 是 `[u8]`：它是一个动态大小类型，不满足Sized（因为它的大小取决于运行时）
5. 它满足 PointeeSized：因为指向 `[u8]`的指针，其元数据是一个 usize 类型的“长度”。而 usize 是有固定大小的（64位系统下是 8 字节）
6. 结果：编译器可以安全地为 .text 节构造出一个胖指针（地址 +长度），并将其通过 as_ref() 传递
7. 如果 T 不满足 PointeeSized.那样的类型甚至无法通过引用 &T 来访问，在 Rust中几乎不存在这种类型，除非是某些极端的试验性编译器特性