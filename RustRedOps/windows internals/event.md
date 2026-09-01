- [Event](#event)
  - [win64 Event](#win64-event)
  - [Event的创建](#event的创建)
    - [Win32 标准创建 (CreateEventW)](#win32-标准创建-createeventw)
    - [Win32 扩展创建 (CreateEventExW)](#win32-扩展创建-createeventexw)
    - [Native API底层系统调用 NtCreateEvent](#native-api底层系统调用-ntcreateevent)
    - [内核驱动级物理初始化 (KeInitializeEvent)](#内核驱动级物理初始化-keinitializeevent)
- [Event的使用](#event的使用)
  - [Event和Timer的配合使用(hypnus.rs/timer())](#event和timer的配合使用hypnusrstimer)
  - [Event和](#event和)
  - [Event Thread区别](#event-thread区别)
  - [常见Event创建方式](#常见event创建方式)
    - [与函数原型的映射解析](#与函数原型的映射解析)
  - [Event使用](#event使用)


# Event





## win64 Event

Event:一种由内核管理的同步原语（Synchronization Object）
1. 它是一个内核对象，拥有“有信号（Signaled）”和“无信号（Non-signaled）”两个物理状态
2. 它允许一个线程在执行到特定位置时挂起等待（RedLight），直到另一个线程（或系统中断）将其状态修改为“有信号”（GreenLight），从而将其唤醒
3. 用户态通过一个 64位的句柄（Handle）来操控它，是实现多线程复杂逻辑同步（如：A干完，B才能开始）的基石.

**Event实质:**
1. 在 Windows 内核中，事件的实质是一个存储在非分页池（Non-paged Pool） 中的 C结构体，名为 KEVENT
2. 每一个 KEVENT对象都包含一个核心头部：DISPATCHER_HEADER
    * SignalState（信号状态）：一个简单的长整型（Long）。0 代表无信号，1代表有信号
    * WaitListHead（等待列表头）：这是一个双向链表。它记录了此时此刻，有哪些线程正在等待这个事件
3. 事件的实质不是代码，而是内核内存里的一块带状态的“记事本”，上面写着它是红灯还是绿灯，以及谁在排队
4. Windows事件的实质是一个包含信号状态（SignalState）和等待链表（WaitList）的内核调度对象（KEVENT）；它之所以能挂起和恢复线程，是因为它能与 Windows内核调度器联动，通过修改线程在‘等待’与‘就绪’队列间的物理位置，实现对 CPU时间片的剥夺与重新分配

## Event的创建

在win64下,event的创建根据所处的执行层级(应用层,底层,Native,内核驱动层)使用的创建方式不同.

### Win32 标准创建 (CreateEventW)

常规的用户态（Ring 3）创建方式，几乎所有标准 Windows 软件都使用它

### Win32 扩展创建 (CreateEventExW)

Windows Vista 之后引入的现代 Win32创建方式。它允许开发者在创建事件的同时，直接控制句柄的访问掩码

### Native API底层系统调用 NtCreateEvent

用户态进入内核态的最后一步系统调用，也是 CreateEventW底层的真实物理实现。红队免杀工具（如 samoa）为了规避 EDR 的Hook，常直接调用它


```c
/**
 * The NtCreateEvent routine creates an event object, sets the initial state of the event to the specified value,
 * and opens a handle to the object with the specified desired access.
 *
 * \param EventHandle A pointer to a variable that receives the event object handle.
 * \param DesiredAccess The access mask that specifies the requested access to the event object.
 * \param ObjectAttributes A pointer to an OBJECT_ATTRIBUTES structure that specifies the object attributes.
 * \param EventType The type of the event, which can be SynchronizationEvent or a NotificationEvent.
 * \param InitialState The initial state of the event object.
 * \return NTSTATUS Successful or errant status.
 * \see https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/nf-ntifs-zwcreateevent
 */
_Kernel_entry_
NTSYSCALLAPI
NTSTATUS
NTAPI
NtCreateEvent(
    _Out_ PHANDLE EventHandle,
    _In_ ACCESS_MASK DesiredAccess,
    _In_opt_ PCOBJECT_ATTRIBUTES ObjectAttributes,
    _In_ EVENT_TYPE EventType,
    _In_ BOOLEAN InitialState
    );
```

在win底层开发和内核研究中,能够熟练阅读和解构这类原生c语言内核级函数声明是基本功.这类声明包含大量微软特有的修饰符(Macros/Annotations)和原生类型

**函数前缀和修饰符**





```rust
/// Wrapper for the `NtCreateEvent` API.
#[inline]
pub fn NtCreateEvent(
    EventHandle: *mut HANDLE,// 输出的事件句柄指针
    DesiredAccess: u32,// 输入参数,期望的访问权限
    ObjectAttributes: *mut c_void,// 输入参数,对象属性,原本指向内核结构体OBJECT_ATTRIBUTES  的指针,此处简化强转为*mut c_void.代表一个匿名事件,该匿名事件只存在于当前进程的私有句柄表中,隐蔽性高,外部工具难以直接检测
    EventType: EVENT_TYPE,//详见下面解释
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

1. 通过winapis()调用dinvk::get_proc_address得到NtCreateEvent的内存地址.对该地址使用transmute强制转为一个函数指针,即本文件定义的NtCreateEvent函数指针
2. EventType: EVENT_TYPE:指定事件对象的复位行为.NotificationEvent代表通知型事件/手动复位,一旦事件被设置为有信号状态,会一直保持有信号,直到显示调用NtResetEvent将其复位.所有正在等待该事件的线程会被同时唤醒.在hypnus中确保多个步骤的上下文可以安全的监听同一个状态信号.
3. SynchronizationEvent,同步型事件/自动复位:一旦事件变为有信号状态,只会唤醒一个正在等待它的线程,在唤醒瞬间,内核会自动将该事件重新复位为无信号状态.
4. InitialState,输入参数,初始状态.事件对象被创建时的初始激活状态,rust中1代表true,0代表false:1代表创建即为有信号active状态,任何线程此时调用NtWaitForSingleObject等待这个事件,都不会阻塞.0代表创建时为无信号状态,后续有线程调用等待函数,会立即陷入阻塞,直到其他线程调用NtSetEvent激活.hypnus中传入0,代表将主线程挂起,后续唤醒

### 内核驱动级物理初始化 (KeInitializeEvent)

在内核驱动程序（Ring 0）下，你不能使用句柄，也不能查进程句柄表。你必须在非分页内存池中直接分配一个KEVENT 结构体，并对其进行就地初始化

# Event的使用

event本身不具备主动控制任何东西的能力，它只是一块死内存。真正控制线程调度的是 Windows 内核的核心组件——微内核分发器（Kernel Dispatcher）. Event 只是调度器用来做决策的数据依据.

1. 在内核(ring0)视角下没有handle只有数据结构.Thread在内核中是用KTHREAD表示的庞大结构体;Event在内核中是用Kevent表示的小结构体,其内部有一个关键头部DISPATCHER_HEADER,该头部包含SignalState(当前信号值,0表示无信号,>0表示有信号)/WaitListHead(双向链表的表头,表示等待名单)
2. 挂起机制:比如调用WaitForSingleObject(Event...)或NtWaitForSingleObject(event, ...) 时,底层最终会调用内核函数KeWaitForSingleObject.该底层内核函数会:
    * 创建桥梁(KWAIT_BLOCK):内核并不会把THREAD直接塞进Event名单.而是在线程栈或内核池中临时创建一个等待块KWAIT_BLOCK的结构体.该结构体有三个指针,一个指回线程KTHREAD,一个指向事件KEVENT,一个用与连接链表
    * 登记:内核把KWAIT_BLOCK挂载到KEVENT的WaitListHead双向链表.此时,Event知道有一个线程通过这个块连接了,等待绿灯
    * 上下文切换(context switch):这是调度核心,Dispatcher将当前KTHREAD状态从running改为waiting.接着触发一次软中断,强制cpu保存当前线程所有寄存器状态,然后去os的Read Queue中找一个其他线程来运行.此时,你的线程在物理层面被踢出cpu
3. 恢复机制(Event唤醒线程,即SetEvent的内核操作):当另一个线程(比如源码中的混淆线程)调用了SetEvent.底层会进入内核函数KeSetEvent,调度器开始接管
    * 置位：内核将KEVENT中的 SignalState 改为 1
    * 扫描等待链表：内核顺着KEVENT中WaitListHead链表查找发现之前挂在在其上的KWAIT_BLOCK.并继续向下找到木马线程KTHREAD
    * 内核把KWAIT_BLOCK从Event中摘除解除绑定(如果这是一个自动重置事件,内核会顺手把SignalState重新改为0)
    * 重新排队:内核将该被唤醒的kTHREAD状态从Waiting改为Ready.然后内核把该线程塞进某个cpu核心的就绪队列
    * 被塞进就绪队列,不意味着线程立即跑代码,它还得等cpu
    * 当KeSetEvent执行完毕返回或发生下一个时钟中断,内核会检查当前cpu的就绪队列.如果调度器发现刚才被唤醒的木马CPU优先级比当前正在CPU上跑的线程高,调度器会立刻preempt当前线程.
    * 内核恢复木马线程之前保存的寄存器状态,把rip指向WaitForSingleObject之后的代码
    * 此时木马线程重新占用cpu,继续向下执行

>在 hypnus 的这段代码中，events句柄代表的是三个独立的内核同步对象（KEVENT），它们充当异步任务链的‘时序锁’；而线程句柄（如h_thread）则代表受操纵的执行上下文（ETHREAD）。这两者的配合实现了：由‘事件’作为逻辑节拍，指挥‘线程’在影子栈中完成复杂的混淆动作
> 事件是“信号”，线程是“载体”

## Event和Timer的配合使用(hypnus.rs/timer())

在win10/11下,定时器负责时间维度的控制(核实触发),事件负责线程维度的同步(通知哪个线程唤醒)

| [主线程] | [线程池工作线程] |
|---|---|
| 1. 创建事件 events[0] (红灯) | |
| 2. 注册定时器 (回调设为NtSetEvent2) | |
| &nbsp;&nbsp;&nbsp;&nbsp;参数绑定 events[0] | |
| 3. 调用 TpSetTimer 启动倒计时 | |
| 4. 调用 NtWaitForSingleObject | |
| &nbsp;&nbsp;&nbsp;&nbsp;主线程在内核中陷入沉睡 | |
| ↓ 100ms | ↑ 5. 定时器过期<br>&nbsp;&nbsp;&nbsp;&nbsp;内核唤醒工作线程 |
| 7. events[0] 变绿灯，主线程复活 ◄── | 6. 执行 NtSetEvent2(events[0])<br>&nbsp;&nbsp;&nbsp;&nbsp;点亮事件 |

以hypnus.rs/timer函数为例:
1. 主线程首先创建一个匿名、初始为无信号的事件 `events[0]`
2. 装填定时器：调用 TpAllocTimer创建一个定时器。将回调函数（Callback）直接设为 NtSetEvent2，将 Context参数设为 `events[0]`
3. 激活定时器并挂起主线程:主线程调用 TpSetTimer 激活定时器，设定 100 毫秒后过期;随后，主线程立即调用 NtWaitForSingleObject(`events[0]`, ...)，使自己挂起进入内核态沉睡
4. 100 毫秒后，硬件时钟中断触发，线程池的工作线程从ntdll!ZwWaitForWorkViaWorkerFactory 状态被内核叫醒;工作线程执行回调，物理上调用了 NtSetEvent2(`events[0]`);`events[0]` 瞬间变为有信号状态（变绿灯）;内核调度器监测到红灯变绿，立刻把沉睡的主线程放回Ready（就绪）队列，主线程复活，继续执行后续代码

**使用event和timer配合的目的**
主线程在等待期间没有任何用户态指令执行，且它的栈是被安全伪造的。而负责叫醒它的动作（NtSetEvent2）是完全由线程池的合法辅助线程在后台默默完成的，切断了主线程执行流的主动唤醒特征

## Event和




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

## 常见Event创建方式




### 与函数原型的映射解析

1. 属于ntdll.dll
2. 
3. EventType: EVENT_TYPE:这里是enum与原型参数是否匹配


## Event使用