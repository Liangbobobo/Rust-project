# process thread

## process

在内核层（Ring 0），进程由 EPROCESS (Executive Process Block)结构体表示。它是系统能够识别该进程的唯一凭证。

### EPROCESS

EPROCESS 是一个巨大的结构体（在 Windows 10/11 上接近 1KB），它包含了或者指向了以下关键组件：

**A. KPROCESS (Kernel Process Block) - 调度核心**  
位于 EPROCESS 的头部。这是内核微内核层（Microkernel）调度器真正关心的部分。  

* **DirectoryTableBase (Cr3/页目录基址):**  
  * 格式: 64位物理地址。  
  * 作用: 这是进程隔离的物理根基。当 CPU 切换到该进程时，OS 会将此值加载到 CR3 寄存器。CPU 的 MMU（内存管理单元）利用它将虚拟地址翻译为物理地址。每个进程都有自己独立的页表树。  
* **ProcessListEntry:** 双向链表节点，将所有活动的进程串联在 ActiveProcessLinks 中（EDR 常用此链表检测隐藏进程）。

**B. 虚拟地址描述符树 (VAD Root - Virtual Address Descriptors)**  

* 格式: 自平衡二叉搜索树 (AVL Tree)。  
* 作用: 虽然页表决定了硬件如何翻译地址，但 VAD 树决定了操作系统如何管理这些地址。  
  * 当你调用 NtAllocateVirtualMemory (dinvk 的核心功能之一) 时，内核并不是立刻填充物理内存，而是在这棵树上插入一个节点（VAD），记录“这段地址范围 (0x1000-0x2000) 被标记为 RWX”。  
  * 用途: 缺页异常（Page Fault）发生时，内核查询 VAD 树，确认该访问是否合法。如果合法，才分配物理页并更新页表；否则报 Access Violation。

**C. 句柄表 (Object Table)**  

* 结构: _HANDLE_TABLE，一个多级指针数组（类似页表结构）。  
* 作用: 存储该进程打开的所有内核对象（文件、互斥体、线程、其他进程）。  
* 格式: 索引是句柄值（如 0x40），值是指向内核对象头（Object Header）的指针。  
* 红队关联: dinvk 使用 NtCurrentProcess() 返回 -1。内核在处理时，会识别这个伪句柄，直接指向当前 EPROCESS，而不需要查表。

**D. 访问令牌 (Token)**  

* 结构: _EX_FAST_REF 指向_TOKEN 对象。  
* 内容:  
  * SID (Security Identifier): 用户的安全 ID（如 S-1-5-21...）。  
  * Privileges: 权限位图（如 SeDebugPrivilege）。  
* 作用: 决定了该进程“能做什么”。任何安全检查（Security Reference Monitor）最终都会比对此结构。

**E. 进程环境块 (PEB - Process Environment Block)**  

* 位置: 这是唯一位于用户空间 (Ring 3) 的核心结构。  
* 作用: 为了减少频繁的 Ring 0 / Ring 3 切换，Windows 将一部分只读或频繁访问的数据映射到用户空间。  
* 关键成员:  
  * ImageBaseAddress: EXE 加载基址。  
  * Ldr (_PEB_LDR_DATA): dinvk 重点利用对象。包含三个双向链表（加载顺序、内存顺序、初始化顺序），记录了所有加载的 DLL。  
  * ProcessParameters: 命令行参数、环境变量。  
  * BeingDebugged: 调试标志位（IsDebuggerPresent 检查的就是这一位）。

### 2. 进程的生命周期存储

* 存储: EPROCESS 结构体存放在 非分页系统内存池 (Non-paged Pool) 中，意味着它永远不会被交换到硬盘上，任何时刻内核都能访问。

---

## 二、 线程 (The Thread) —— 核心结构：ETHREAD

在内核层，线程由 ETHREAD (Executive Thread Block) 表示。它是 CPU 调度的最小单位。

### 1. 核心组成部分与数据结构

ETHREAD 同样包含了一个底层的 KTHREAD (Kernel Thread Block)。

**A. 线程控制块 (KTHREAD) - 调度核心**  

* **State (状态机):** 记录线程当前处于 Running (运行), Ready (就绪), Wait (等待), Terminated (终止) 等状态。  
* **Priority (优先级):** 0-31 的整数。决定了抢占 CPU 的能力。  
* **Quantum (时间片):** 倒计时器。决定了该线程能连续霸占 CPU 多久（通常是几十毫秒）。归零后触发调度。  
* **Kernel Stack (内核栈):**  
  * 作用: 当线程陷入内核（Syscall 或中断）时，RSP 栈指针会切换到这里。  
  * Trap Frame: 保存用户态现场的关键结构。

**B. 线程环境块 (TEB - Thread Environment Block)**  

* 位置: 用户空间 (Ring 3)。  
* 寻址: x64 下通过 GS:[0] 访问，x86 下通过 FS:[0] 访问。  
* 结构:  
  * NtTib (Thread Information Block): 包含栈底 (StackBase) 和栈顶 (StackLimit)。  
  * ProcessEnvironmentBlock: 指向所属 PEB 的指针。  
  * ClientId: 包含 UniqueProcessId 和 UniqueThreadId。  
  * LocalStoragePointer: TLS (线程局部存储) 数组指针。

**C. 上下文 (CONTEXT)**  

* 性质: 这是最重要的数据结构，对应 dinvk 的断点欺骗。  
* 位置: 当线程运行时，它在 CPU 寄存器里；当线程挂起或陷入内核时，它保存在 内核栈 或 ETHREAD 关联的结构 中。  
* 内容:  
  * Rip: 指令指针（下一步执行哪里）。  
  * Rsp: 栈指针。  
  * Rax, Rcx, Rdx...: 通用寄存器。  
  * `Dr0 - Dr7`: 硬件调试寄存器。这就是为什么 set_breakpoint 只能影响当前线程——它修改的是当前线程在内核中保存的 CONTEXT 副本。

**D. APC (Asynchronous Procedure Call) 队列**  

* 结构: KAPC_STATE。  
* 作用: 挂在线程下的一个队列。用于“强行”让线程去执行某个函数。  
* 红队关联: 很多注入技术（APC Injection）通过向目标线程的 APC 队列插入恶意函数，当该线程进入“Alertable Wait”状态时，就会执行恶意代码。

---

## 三、 详细调度机制与关系图谱

### 1. 调度过程 (The Context Switch)

当 CPU 决定从 线程 A 切换到 线程 B 时，内核（函数 SwapContext）会执行以下原子操作：

1. **保存现场 (Save):**  
   * 将 CPU 当前所有寄存器（RIP, RSP, RAX, 标志位等）压入 线程 A 的内核栈，形成 Trap Frame。  
   * 保存特定的系统寄存器到线程 A 的 KTHREAD 结构。

2. **切换判定:**  
   * 检查 线程 B 是否属于同一个进程。

3. **地址空间切换 (如果是不同进程):**  
   * 核心动作: 读取 进程 B 的 EPROCESS.DirectoryTableBase。  
   * 硬件执行: 将其写入 CR3 寄存器。  
   * 后果: 此时，虚拟地址 0x00400000 对应的物理地址瞬间变了。TLB（页表缓存）失效（或部分失效），CPU 必须重新遍历页表。这是进程切换昂贵的根本原因。

4. **恢复现场 (Load):**  
   * 从 线程 B 的内核栈 弹出之前保存的 Trap Frame 到 CPU 寄存器。  
   * 更新 GS 寄存器基址，使其指向 线程 B 的 TEB。  
   * 设置 TSS (Task State Segment)，确保下次中断时能找到线程 B 的内核栈。

5. **执行:**  
   * CPU 执行 IRETQ 或 SYSRET 指令，跳转到 线程 B 之前停止的 RIP 位置继续运行。

### 2. 总结性对比表

| 特性       | 进程 (Process)                      | 线程 (Thread)                            |
|------------|-------------------------------------|------------------------------------------|
| 内核对象   | EPROCESS                            | ETHREAD                                  |
| 用户对象   | PEB (GS:[0x60])                     | TEB (GS:[0x30])                          |
| 唯一标识   | PID (Process ID)                    | TID (Thread ID)                          |
| 内存管理   | 拥有 VAD 树和 CR3 (页表基址)        | 共享进程的 VAD 和 CR3                    |
| 执行状态   | 静态容器，无 RIP                    | 拥有 RIP, RSP, RFLAGS, DRx               |
| 安全凭证   | Token (主要)                        | Impersonation Token (可选，用于模拟)     |
| 调度单位   | 否 (OS 不调度进程)                  | 是 (OS 调度队列里的单位)                 |
| dinvk 视角 | 攻击的目标 (Target)，API 作用的范围 | 攻击的载体 (Vector)，Spoofing 发生的现场 |

通过这个详细模型，你可以清晰地看到：dinvk 的工作流通常是在 线程级（通过 TEB 获取 PEB，修改寄存器欺骗）操作，最终目的是为了在 进程级（通过 VAD 申请内存，通过 Ldr 隐藏模块）留下痕迹或执行载荷。
