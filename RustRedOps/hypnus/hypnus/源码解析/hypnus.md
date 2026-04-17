- [fn timer::NtDuplicateObject](#fn-timerntduplicateobject)
- [fn timer::rax](#fn-timerrax)
- [TPAllocTimer](#tpalloctimer)
- [TP\_CALLBACK\_ENVIRON\_V3](#tp_callback_environ_v3)
- [struct TP\_POOL\_STACK\_INFORMATION](#struct-tp_pool_stack_information)
- [TpAllocPool(\&mut pool, null\_mut())](#tpallocpoolmut-pool-null_mut)
- [win64 Event](#win64-event)
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
- [扩展-handle句柄](#扩展-handle句柄)


[win64 threadpool](#win64-threadpool)



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



## TPAllocTimer

```rust
TpAllocTimer(
                // 输出参数,内核把新创建的的定时器对象TP_TIMER的物理内存地址填入
                &mut timer_ctx, 
                // 垫片
                self.cfg.trampoline as *mut c_void, 
                // 堆栈上定义的CONTEXT
                &mut ctx_init as *mut _ as *mut c_void, 
                // 执行环境TP_CALLBACK_ENVIRON_V3
                &mut env
            );
```























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