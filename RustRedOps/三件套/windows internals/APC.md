# APC

**注意:**

1. WinDbg,还是WinDbg
2. APC非常常用,研究到最底层绝对有巨大收货

APC (Asynchronous Procedure Call) 是 Windows内核提供的一种机制，允许程序将一个函数“强行塞进”某个线程的任务队列中。当这个线程闲下来（进入Alertable 警觉状态，比如正在调用 SleepEx或等信号）时，系统会打断它，强制它先去执行队列里的 APC函数，执行完了再回来

## 线程队列

1. win内核视角下,每个win的线程在内核都由一个KTHREAD结构体表示.所谓的线程任务队列实质是一个双向循环链表,存在于KTHREAD中.其中一个成员pcState（类型为_KAPC_STATE）
2. 在_KAPC_STATE结构体中,维护了一个包含两个元素的双向链表头数组.
    * `ApcListHead[0]` -> KernelApcListHead（内核模式 APC队列）：供内核驱动和文件系统使用
    * `ApcListHead[1]` -> UserApcListHead（用户模式 APC队列）：用户态任务队列
3. 当调用NtQueueApcThread时,内核会在非分页池中分配一个KAPC结构体(代表一个任务包),该结构体包含:
    * NormalRoutine：待执行的函数地址（如 hypnus 中的 ntdll!NtContinue）
    * NormalContext：传入函数的参数（在 hypnus 中是伪造的 CONTEXT 指针）
    * ApcListEntry：双向链表节点
4. 物理装填:内核通过InsertTailList 宏，把这个 KAPC 结构体挂进目标线程的 KTHREAD.ApcState.`ApcListHead[1]`（用户态队列）链表的尾部

## Alertable state

普通线程在休眠或等待事件时，CPU 会把它们移出调度队列，此时即使你往它的UserApcListHead(即KTHREAD.ApcState.`ApcListHead[1]`)里塞了任务，它也无法执行。只有当线程进入Alertable状态时，内核调度器才会强制唤醒它去清空队列

1.  物理层面的alertable标记:在 KTHREAD 结构体中，有一个位域（Bitfield）成员叫 Alertable（通常占用1个Bit，在 Windows 10/11 的 SameTebFlags 或内核偏移中）
    * 当 Alertable 标志为 0：线程处于普通等待状态，拒绝处理用户态 APC
    * 当 Alertable 标志为 1：线程进入警觉状态，一旦 UserApcListHead有任务，立刻被内核唤醒
2. 如何进入Alertable:线程自己必须调用带有alertable开关的阻塞等待函数.
   * 在底层,这些函数最终会调用内核的KeWaitForSingleObject 或 KeDelayExecutionThread，并将参数 Alertable设为 TRUE
   * 在应用层:
     * A：调用带有 Ex 后缀的等待函数（最常见）:SleepEx,WaitForSingleObjectEx,WaitForMulitpleObjectsEx
     * B:调用特定的同步/通知 API:SingnalObjectAndWait,MsgWaitForMultipleObjectsEx
     * C:底层系统调用直接强开(hypnus.rs做法):直接通过间接系统调用运行NtWaitForSingleObject,将其第二个参数Alertable传入True(1)










// 根据调用约定和运行级别,win下的APC分为:1. 内核模式APC:由内核或驱动程序使用,通常用于系统底层任务(如 异步IO操作的完成).其优先级高于用户态代码,一旦触发,内核会强制中断当前线程的用户态执行流,切入内核态执行该APC;
// 2. 用户模式apc:由用户态应用程序使用(通过 Win32 API QueueUserAPC 或内核 Native API NtQueueApcThread),用户态apc的执行是被动的.当向某线程发送apc任务后,该任务会排在线程apc队列中.只有当目标线程主动调用特定的同步函数并进入alertable state警惕状态时,内核才会派发并执行队列中的apc.常见使线程进入警惕态的api:SleepEx(..., TRUE),WaitForSingleObjectEx(..., TRUE),SignalObjectAndWait(..., TRUE),MsgWaitForMultipleObjectsEx(..., QS_ALLINPUT, MWMO_ALERTABLE)
// 3. 特殊用户模式apc:win10 19041及后续版本适用.特殊用户模式apc不需要线程处于alertable state状态即可强制执行.主要为了支持底层的环形缓冲区ring buffer和高并发异步IO调度,减少不必要的线程上下文切换开销.但win10/11,也推出了ETW-Ti（Threat Intelligence)内核遥测接口,当任何进程尝试调用NtQueueApcThread向另一进程的线程注入apc时,内核会通过ETW-Ti产生遥测事件,获取该动作的完整上下文,这使得传统的跨进程apc注入极易被查杀.

本项目中:
1. 常规APC参数:NtQueueApcThread(目标线程, 要执行的函数地址,传给函数的参数)
2. hypnus中:调用是：NtQueueApcThread(目标线程, NtContinue的地址,我们伪造的CONTEXT地址)
    * 此时执行APC时,实际上执行了NtContinue(&CONTECXT).NtContinue瞬间重置cpu的所有硬件寄存器.这意味着,每个APC并没有真正执行一个函数,而是进行一次暴力的硬件状态传递