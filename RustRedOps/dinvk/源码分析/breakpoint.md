# Breakpoint

## CONTEXT 结构深度解析

CONTENT这个结构本身是谁生成并维护的?存储在什么地方?我的项目中自定义了这个结构后,有什么用?

### 一、这个结构是谁生成并维护的？

**生成者**：Windows 内核 (Kernel) 和 CPU 硬件。

* **源头**：线程的“状态”本质上是 CPU 寄存器里的实时电流/数值（RIP 指令指针、RSP 栈指针、RAX 通用寄存器等）。
* **生成时刻**：当发生“上下文切换”(Context Switch)、“异常”(Exception) 或“系统调用”(Syscall) 时，线程会被暂停。此时，CPU 和 Windows 内核会将 CPU 寄存器里的当前值“转储”(Dump) 到内存中保存起来。这个被保存下来的数据块，其格式就是标准的 CONTEXT 结构。

**维护者**：Windows 内核调度器 (Scheduler)。

* 当你的线程被挂起（Sleeping 或等待时间片）时，内核负责维护这个数据。当线程再次被调度执行时，内核会把 CONTEXT 里的数据重新加载回 CPU 寄存器，让线程以为自己从未停止过。

### 二、存储在什么地方？

这取决于你是在谈论“内核里的源数据”还是“你代码里的变量”。

#### 1. 内核态 (Kernel Mode) - 源数据

* 当线程不运行时，其 CONTEXT 数据通常存储在**内核栈 (Kernel Stack)** 或与线程相关的内核对象（如 ETHREAD / KTRAP_FRAME）中。这是受保护的内存，用户态程序无法直接读取。

#### 2. 用户态 (User Mode) - 你代码中的副本

* **关键点**：在你代码 `src/breakpoint.rs` 中写的 `let mut ctx = CONTEXT { ... };`，这仅仅是在**你当前线程的栈 (Stack)** 上分配的一块内存空间。
* **数据的流动**：
  * 当你调用 `NtGetContextThread` 时，内核将它维护的“源数据”**复制一份**到你的 `ctx` 变量（栈内存）中。
  * 当你调用 `NtSetContextThread` 时，内核读取你的 `ctx` 变量，并将其覆盖回内核的“源数据”中。

### 三、在项目中自定义这个结构后，有什么用？

在你的项目中，自定义 CONTEXT 结构体充当了“通信协议”或“数据模板”的角色。

如果没有这个结构体，你无法告诉内核你要修改哪个寄存器。它的具体作用流程如下：

#### A. 提供内存布局 (Layout Map)

Windows 内核（C/C++ 编写）对内存布局有严格要求。例如，在 x64 下，`ContextFlags` 必须在偏移 0x30 处（假设），`Dr0` 必须在偏移 0x48 处。
你自定义的 `struct CONTEXT` 必须使用 `#[repr(C)]` 严格模仿这种布局。

* **如果不用它**：你就只能传递一个没有任何类型信息的 `&mut [u8; 1232]` 字节数组给内核，操作起来极其痛苦且容易出错（比如你需要手动计算 `Dr7` 是第几个字节）。

#### B. 实现“无感 Hook” (Hardware Breakpoint Hook)

这是你这个项目的核心目的。请看你的代码逻辑：

1. **申请内存**：

   ```rust
   // 在你的栈上挖了一个坑，准备装数据
   let mut ctx = CONTEXT { ... };
   ```

2. **获取快照 (Get)**：

   ```rust
   // 告诉内核：“把当前寄存器的值填进我挖的坑里”
   NtGetContextThread(NtCurrentThread(), &mut ctx);
   ```

3. **修改副本 (Modify)**：

   ```rust
   // 在你的副本上修改。注意：此时 CPU 真实的寄存器还没变！
   ctx.Dr0 = address.into(); // 设置断点地址
   ctx.Dr7 = ...;            // 开启断点
   ```

4. **应用修改 (Set)**：

   ```rust
   // 告诉内核：“把这个修改后的结构体，写回 CPU 寄存器去”
   NtSetContextThread(NtCurrentThread(), &ctx);
   ```

### 总结

自定义这个结构体，是为了让 Rust 代码能够以一种类型安全、结构化的方式，去读写原本由内核管理的 CPU 寄存器状态。通过修改这个结构体里的 `Dr` 字段并写回内核，你欺骗了 CPU，强迫它在执行到特定地址时触发异常，从而让你的 `veh_handler` 捕获并控制程序流程（即实现 Hook）。

## 操作CONTEXT的权限要求与EDR检测分析

这种操作内核和寄存器的方式,不需要特殊的权限吗?会被edr av感知吗?

这是一个非常敏锐的问题。在安全领域，“能不能做”（权限）和“会不会被发现”（EDR对抗）是两个维度的核心问题。

### 一、权限要求 (Permissions)

**结论**：针对自身进程/线程，不需要特殊权限（不需要管理员权限）。

* **自身线程 (`NtCurrentThread`)**：
  你的代码中使用的是 `NtCurrentThread()`。在 Windows 权限模型中，一个进程默认对自己创建的线程拥有完全控制权 (`THREAD_ALL_ACCESS`)。任何普通用户权限运行的程序，都可以随意修改自己线程的寄存器（包括调试寄存器 `Dr0-Dr7`）。
  
* **跨进程 (Remote Process)**：
  如果你试图修改其他进程的线程上下文（例如注入到一个系统服务中），那么你确实需要更高的权限（如 `SeDebugPrivilege` 或管理员权限）来获取目标线程的句柄。

但在你的 `dinvk` 项目场景下（通常是 Shellcode Loader 或植入物），它运行在当前进程空间内，所以普通用户权限足矣。

---

### 二、会被 EDR / AV (杀毒软件) 感知吗？

**结论**：会，但比传统的 Hook 技术隐蔽得多。

这是一个“猫鼠游戏”。让我们分解一下 EDR 是如何感知，以及这种技术试图规避什么。

#### A. 为什么这种技术被称为“无感 Hook” (Stealthy)?

传统的 Hook (Inline Hook) 需要修改内存中的机器码（例如在 `NtAllocateVirtualMemory` 开头写入一个 `JMP` 指令）。

* **传统痛点**：EDR 只需要扫描内存，对比 `ntdll.dll` 的磁盘文件和内存映像。如果发现代码段 (`.text section`) 被篡改了，直接报警。
* **本技术的优势**：硬件断点不修改任何内存字节。它修改的是 CPU 的寄存器。如果 EDR 只扫描内存完整性，它完全发现不了你 Hook 了 API。

#### B. EDR 如何感知/检测这种操作？

虽然它不改内存，但现代 EDR (如 CrowdStrike, SentinelOne, Carbon Black) 有多种方法检测它：

1. **API 监控 (API Hooking)**：
   * **原理**：EDR 通常会在用户态 Hook `NtSetContextThread` 这个 API。
   * **检测**：当你调用这个 API 试图修改 `Dr` 寄存器时，EDR 的 Hook 会拦截到，检查参数。如果发现你设置了 `Dr7` 开启断点，并且 `Dr0-Dr3` 指向了敏感的 API 地址（如 `VirtualAlloc`），EDR 会立即查杀。
   * **对抗**：我看你的文件结构里有 `syscall` 文件夹。通常红队工具会使用 Direct Syscalls (直接系统调用)。通过汇编直接执行 `syscall` 指令进入内核，绕过 `ntdll.dll` 中 EDR 设置的监控钩子。

2. **异常拦截 (Exception Filtering)**：
   * **原理**：硬件断点触发时，CPU 会抛出 `EXCEPTION_SINGLE_STEP` 异常。这个异常会经过系统的分发流程。
   * **检测**：EDR 也可以注册 VEH (向量化异常处理程序) 或者 Hook `KiUserExceptionDispatcher`。如果 EDR 发现有一个 `SINGLE_STEP` 异常发生在 `NtAllocateVirtualMemory` 的入口处，它就知道这里肯定被人下了硬件断点。

3. **线程扫描 (Thread Scanning)**：
   * **原理**：EDR 的内核驱动或扫描线程会定期挂起进程中的线程，读取它们的 `CONTEXT`。
   * **检测**：如果扫描结果显示 `Dr7` 寄存器是非零值，说明启用了硬件断点，这是一个极其可疑的行为（正常软件极少在生产环境使用硬件断点，除非是调试器）。

### 总结

这种“硬件断点 + VEH”的技术：

* **权限**：普通用户即可，门槛低。
* **隐蔽性**：中等偏高。
  * 它完美绕过了“内存完整性校验”（它不改代码）。
  * 它容易被“行为监测”发现（设置上下文的动作、异常触发的瞬间）。

在 `dinvk` 这个项目中，为了不被发现，它必须配合 Syscalls (绕过 API 监控) 使用，并且赌 EDR 没有进行高频的线程寄存器扫描。这是一种高级的红队对抗技术
=======

Hardware Breakpoint Spoofing(硬件断点参数欺骗) 或 "Tampering Syscalls"的原理:  

1. EDR 的监控点：现代 EDR (Endpoint Detection and Response) 通常会 Hook用户态的敏感 API（如 NtAllocateVirtualMemory）。当你调用这些 API 时，EDR会先检查你的参数（例如：你是不是在申请 RWX可读可写可执行的内存？）。如果参数看起来是恶意的，EDR 就会拦截。
2. 欺骗策略 (The Spoof)：  
    * 第一步：我们调用 API 时，传入完全无害的假参数（例如：申请只读内存PAGE_READONLY）。EDR 检查通过，放行。  
    * 第二步：我们在 API 执行“系统调用(Syscall)”指令之前的瞬间，利用硬件断点暂停 CPU  
    *第三步：在异常处理函数（VEH）中，我们将寄存器或栈里的假参数替换为真实的恶意参数（例如：改为 PAGE_EXECUTE_READWRITE）  
    * 第四步：恢复执行。此时 EDR 的检查已经结束了，Syscall将带着恶意参数进入内核。
3. 不同于软件断点（修改内存写入 0xCC，容易被 EDR.硬件断点是修改 CPU 的寄存器，不修改任何内存代码，因此极其隐蔽

>>>>>>> Stashed changes

## 背景知识

一、 硬件断点 (Hardware Breakpoints) 与 Dr寄存器详解

在 x86/x64 架构中，调试不仅仅是软件层面的功能，CPU 硬件本身就内置了一套复杂的调试机制。

1. 调试寄存器概览  
CPU 只有 Dr0, Dr1, Dr2, Dr3, Dr6, Dr7 这几个寄存器是暴露给程序员使用的（Dr4 和 Dr5 是保留的，映射到 Dr6/Dr7）。

2. Dr7：控制中心 (The Control Register)  
这是最复杂的寄存器，它决定了“怎么断”、“断哪里”。这是一个 32 位或 64 位的寄存器，按位（Bit）划分功能：

## Dr7 寄存器总览

Dr7 是一个 32 位（在 x64 下通常也只用低 32 位）的寄存器。它被划分为三个主要功能区：

1. **启用控制区 (0-7位)**：决定是否开启 Dr0-Dr3。  
2. **辅助控制区 (8-15位)**：控制精确断点和通用检测。  
3. **条件控制区 (16-31位)**：决定断点是“读”、“写”还是“执行”，以及监控的字节长度。

### 1. 寄存器位图结构 (Bit Layout)

请复制下面的代码块，这是最准确的位图表示：

```text
31  30  29  28  27  26  25  24  23  22  21  20  19  18  17  16
+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
| LEN3  |  RW3  | LEN2  |  RW2  | LEN1  |  RW1  | LEN0  |  RW0  |
+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
|   (DR3 属性)   |   (DR2 属性)   |   (DR1 属性)   |   (DR0 属性)   |

15  14  13  12  11  10   9   8   7   6   5   4   3   2   1   0
+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
| 0 | 0 | GD| 0 | 0 | 0 | GE| LE| G3| L3| G2| L2| G1| L1| G0| L0|
+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
          |               |   |   |   |   |   |   |   |   |   |
      保护检测          全局 局部 (DR3) (DR2) (DR1) (DR0) 启用位
                        精确 精确 启用  启用  启用  启用
```


### 第一部分：启用控制位 (L0-L3, G0-G3)

位于寄存器的最低 8 位（0-7）。每两个位控制一个地址寄存器（Dr0-Dr3）。

* **L0 (Bit 0) - Local Enable 0**:  
  * **含义**: 局部启用 Dr0 断点。  
  * **作用**: 置 1 时，CPU 会监控 Dr0 中的地址。  
  * **特性**: 在任务切换（Task Switch）时，CPU 硬件会自动将其清零。但在现代操作系统（如 Windows）中，OS 会在上下文切换时保存并恢复此位，因此它实际上在当前线程中一直有效。这是我们在 `breakpoint.rs` 中使用的位。

* **G0 (Bit 1) - Global Enable 0**:  
  * **含义**: 全局启用 Dr0 断点。  
  * **特性**: 任务切换时不会被清零。通常用于系统级调试。

* **L1/G1, L2/G2, L3/G3**:  
  * 分别对应 Dr1, Dr2, Dr3 寄存器，逻辑同上。

### 第二部分：辅助控制位 (8-15 位)

这些位通常用于高级调试场景，在常规红队开发中较少使用。

* **LE (Bit 8) & GE (Bit 9) - Local/Global Exact**:  
  * **含义**: 精确断点检测。在早期的 386/486 处理器上，用于指示 CPU 在指令执行后立即报告断点，而不是延迟报告。  
  * **现状**: 在现代 P6 架构及之后的 CPU 中，这两位通常被忽略，因为现代 CPU 默认支持精确断点。为了兼容性建议置 1，但置 0 通常也没问题。

* **GD (Bit 13) - General Detect Enable**:  
  * **含义**: 调试寄存器保护。  
  * **作用**: 如果置 1，当任何指令试图访问调试寄存器（`MOV DRx, ...`）时，CPU 会抛出异常（Debug Exception #DB）。  
  * **红队用途**: 这是一种反调试技术。如果你开启它，当安全软件试图扫描或修改你的断点时，你可以捕获到它的行为。

### 第三部分：条件与长度控制位 (16-31 位) - 最关键部分

这部分决定了断点“什么时候触发”以及“监控多宽的内存”。每个 Dr 寄存器占用 4 位（2位 R/W，2位 LEN）。

我们以 Dr0 对应的 RW0 和 LEN0 为例：

#### 1. RW0 (Bit 16-17) - Read/Write Control (触发条件)

决定了什么样的操作会触发断点。

* **00: Execution (执行)**  
  * **含义**: 仅当指令指针 (RIP/EIP) 指向 Dr0 地址并试图执行该指令时触发。  
  * **要求**: 对应的长度 (LEN) 必须为 00。  
  * **场景**: `breakpoint.rs` 使用的就是这个模式（Hook syscall 指令）。

* **01: Write Only (仅写入)**  
  * **含义**: 仅当程序向 Dr0 地址写入数据时触发。  
  * **场景**: 监控内存破坏、监视全局变量修改。

* **10: I/O Read/Write (I/O 读写)**  
  * **含义**: 仅当 CPU 执行 I/O 指令（如 `IN`, `OUT`）访问 Dr0 指定的端口时触发。  
  * **现状**: 需要 CR4 寄存器的 DE 位支持，现代用户态代码几乎不用。

* **11: Read/Write (读写)**  
  * **含义**: 读取或写入该地址时都会触发。  
  * **注意**: 仅执行指令不会触发此模式（执行有专用的 00 模式）。

#### 2. LEN0 (Bit 18-19) - Length Control (监控长度)

决定了监控的内存范围大小。

* **00: 1 Byte (1 字节)**  
  * **强制**: 如果 RW 为 00 (执行)，LEN 必须是 00。

* **01: 2 Bytes (2 字节)**  
  * **对齐要求**: 地址必须是 2 字节对齐（地址末位为 0, 2, 4...）。

* **10: 8 Bytes (8 字节 - 仅限 x64) / Undefined on x86**  
  * **对齐要求**: 地址必须是 8 字节对齐。  
  * **场景**: 监控 64 位指针或变量的读写。

* **11: 4 Bytes (4 字节)**  
  * **对齐要求**: 地址必须是 4 字节对齐。

---

### 三、 结合 breakpoint.rs 的实战解读

现在回过头看代码中的这行操作：

```rust
// 代码：set_dr7_bits(ctx.Dr7, 0, 1, 1);
// 意思是：从第 0 位开始，修改 1 个位，将其设为 1。
```

这实际上只做了 **一件事**：

* **Set Bit 0 (L0) = 1**: 启用 Dr0。

那么 RW0 和 LEN0 呢？  
因为 `ctx` 是通过 `NtGetContextThread` 获取的，或者被 `Default::default()` 初始化过。在大多数情况下，未使用的 Dr7 高位默认是 0。

* **Bit 16-17 (RW0) = 00**: 默认为 执行断点。  
* **Bit 18-19 (LEN0) = 00**: 默认为 1 字节长度。

**最终配置效果**：

> "CPU 兄弟，请帮我盯着 Dr0 里的那个地址。只要你正准备执行那个地址上的指令（RW=00），别废话，立刻暂停（Exception），告诉我一声。"

这就是为什么这段代码能精确拦截 `syscall` 指令执行的原因。

为什么“执行断点”的 LEN 必须是 0？  
因为指令的执行是基于“指令指针（RIP）”的，RIP 是一个点，不是一个范围。CPU 只需要判断 `Current_RIP == Dr0` 即可。  
因为代码执行必须从指令的首字节开始，所以 CPU 设计者规定：只要捕捉到 RIP撞上了这个首字节地址（1字节范围），就算命中。除此之外的任何范围监控，对于“执行”这个动作来说都是多余且非法的。

Dr6：状态中心 (The Status Register)
当异常发生时，异常处理程序（VEH）需要看 Dr6 才知道是谁触发了异常。

* B0 - B3 (位 0-3)：如果 Dr0 触发，B0 置 1；如果 Dr1 触发，B1 置 1。  
* BS (位 14)：Single Step。如果是单步调试（EFLAGS 的 TF 位）触发的，这一位会置 1。

---

二、 Windows 异常处理流程 (Exception Dispatching)

当硬件断点触发时，CPU 会抛出 INT 1 中断。Windows 内核捕获后，会经历以下漫长的旅程才能到达你的代码：

1. **内核陷阱处理 (Kernel Trap Handler)**：CPU 转入 Ring 0，内核接管。  
2. **KiDispatchException**：内核判断异常来源。如果是用户态异常，准备发回用户态。  
3. **NtContinue / User APC**：内核将线程状态恢复到用户态，并跳转到 `ntdll!KiUserExceptionDispatcher`。  
4. **RtlDispatchException**：这是用户态异常分发的总指挥。它会按顺序尝试：  
   * 第一轮：**VEH (Vectored Exception Handler)** —— 我们在利用的就是这个！  
   * 第二轮：**SEH (Structured Exception Handler)** —— 即基于栈的 `__try / __except`。  
   * 第三轮：**UEF (Unhandled Exception Filter)** —— 也就是“程序已停止工作”弹窗。

**为什么选择 VEH？**  

1. **优先级最高**：它比 `try/catch` 先运行。  
2. **无需修改栈**：SEH 需要修改栈帧（容易被检测），而 VEH 是通过 API 注册到堆上的一个链表中，更加隐蔽。  
3. **全进程生效**：注册一次，所有线程触发异常都会被它捕获，适合全局 Hook。

---

三、 线程上下文 (CONTEXT) 的本质

`CONTEXT` 结构体不仅仅是一堆数据，它是“时间冻结”的快照。

当 CPU 暂停一个线程时（无论是因为中断、异常还是线程调度），它必须把 CPU 内部所有寄存器的值保存到内存里，这个内存结构就是 `CONTEXT`。

* **修改的原理**：  
  你在 VEH 里修改了 `CONTEXT` 结构体内存里的 `R10` 为 `0xFFFF`。  
  当 VEH 返回 `EXCEPTION_CONTINUE_EXECUTION` 时，`ntdll` 会调用 `NtContinue` 系统调用。  
  `NtContinue` 告诉内核：“嘿，把这个线程恢复运行吧，但恢复的时候，请把 CPU 的 `R10` 寄存器设为 `0xFFFF`”。

这就是为什么修改结构体能改变 CPU 真实状态的原因。

---

四、 x64 调用约定与 Syscall 的特殊性 (非常关键)

这是理解 `breakpoint.rs` 中 `Rsp + 0x30` 这种魔术数字的关键。

1. **Windows x64 ABI (应用程序二进制接口)**  
标准的函数调用（如调用 DLL 函数）遵循以下规则：  

* 参数 1-4：存放在 `RCX`, `RDX`, `R8`, `R9`。  
* 参数 5+：存放在栈 (Stack) 上。  
* **Shadow Space (预留栈空间)**：调用者（Caller）必须在栈上预留 32 字节（0x20）的空间，就在返回地址上面。这主要是为了让被调用函数（Callee）能把寄存器参数（RCX-R9）保存回栈里方便调试。

1. **Syscall 的特殊规则 (The R10 Twist)**  
当你直接执行 `syscall` 指令进入内核时，规则发生了微小的变化：

* **CPU 的行为**：  
  执行 `syscall` 指令时，CPU 会把当前的 `RIP`（下一条指令地址）自动保存到 `RCX` 中，把 `RFLAGS` 保存到 `R11` 中。  
* **冲突**：  
  普通的调用约定里，`RCX` 存放的是第 1 个参数。但 `syscall` 指令会无情地覆盖掉 `RCX`（存入 `RIP`）。  
* **解决方案**：  
  Windows 内核约定：对于系统调用，第 1 个参数必须在 `syscall` 前从 `RCX` 移动到 `R10`。

**结论**：  

* 普通函数 Hook：看 `RCX`。  
* Syscall Hook (我们的场景)：看 `R10`。

1. **栈偏移计算 (Magic Numbers)**  
让我们算一下为什么代码里写 `Rsp + 0x30` 来找第 6 个参数。

当异常发生进入 VEH 时，`CONTEXT.Rsp` 指向的是系统调用发生那一瞬间的栈顶。

栈的布局（从高地址到低地址）：

```
 1 | ...             |
 2 | 参数 6          | <--- RSP + 0x30  (8+32+8 = 48 bytes)
 3 | 参数 5          | <--- RSP + 0x28  (8+32 = 40 bytes)
 4 | Shadow Space    | <--- 32 字节 (0x20) 用于保存 R10, RDX, R8, R9
 5 | ...             |
 6 | Return Address  | <--- RSP + 0x00 (调用 syscall 时的栈顶)
```

Wait, logic check:  
实际上，对于 `syscall` 指令，它不压栈返回地址（因为它存到 `RCX` 了）。  
但是，Wrapper 函数（在 `winapis.rs` 里）在调用 `syscall` 之前，是作为一个普通的 Rust 函数在运行的。Rust 编译器生成的代码会遵循 ABI，压入参数。

让我们重新看 `NtAllocateVirtualMemory` 的调用栈帧：  

1. 调用者压入参数 6, 5。  
2. 调用者预留 32 字节 Shadow Space。  
3. 调用者把参数 1-4 放入寄存器。  
4. 执行 `syscall`。

此时 `Rsp` 指向 Shadow Space 的底部。  

* `Rsp + 0x00` 到 `Rsp + 0x20`：Shadow Space。  
* `Rsp + 0x20` 到 `Rsp + 0x28`：参数 5。  
* `Rsp + 0x28` 到 `Rsp + 0x30`：参数 6。

(注意：Rust 代码中的 `0x30` 可能是针对特定的栈帧调整，或者是包含了返回地址的偏移，具体取决于编译器如何生成 `dinvoke!` 宏的栈帧。在 `no_std` 和内联汇编环境下，栈指针的位置需要极其精确的计算)。

在 `breakpoint.rs` 中：

```rust
*(((*context).Rsp + 0x30) as *mut u32) = Protect;
```

这暗示了当前的 `Rsp` 是标准的函数调用栈顶（包含返回地址）。  

* `0x00-0x08`: Return Address  
* `0x08-0x28`: Shadow Space (32 bytes)  
* `0x28-0x30`: Param 5  
* `0x30-0x38`: Param 6

---

下一步  
有了这些坚实的背景知识，我们就可以开始逐行注解代码了。这些知识点解释了：  

1. 为什么要设 Dr7 的第 0 位（L0）。  
2. 为什么要判断 Dr7 & 1。  
3. 为什么要改 R10 而不是 RCX。  
4. 为什么要去 Rsp + 0x30 找参数。

## 源码

### use core::ffi::c_void

dinvoke 项目的核心是与 Windows 操作系统 API 进行交互（FFI - 外部函数接口）。Windows API（以及 C 语言 API）大量使用 void* 指针来表示“任意类型的内存块”或“不透明句柄”

它是 C 语言中 void 类型的 Rust 等价物。在 Rust 中，c_void 通常是一个没有任何大小（size）和字段的“空”类型。

* C 语言: void* ptr
* Rust 对应: *mut c_void 或*const c_void

```rust
#[doc = include_str!("c_void.md")]
#[lang = "c_void"] //
#[cfg_attr(not(doc), repr(u8))] // An implementation detail we don't want to show up in rustdoc
#[stable(feature = "core_c_void", since = "1.30.0")]
pub enum c_void {
    #[unstable(
        feature = "c_void_variant",
        reason = "temporary implementation detail",
        issue = "none"
    )]
    #[doc(hidden)]
    __variant1,
    #[unstable(
        feature = "c_void_variant",
        reason = "temporary implementation detail",
        issue = "none"
    )]
    #[doc(hidden)]
    __variant2,
}
```

利用枚举来禁止实例化，利用 `repr(u8)` 来保证内存布局非零，利用 `lang item`来获得编译器的原生支持，最终造就了一个完全服务于 FFI 指针操作的特殊类型

## 关于NtGetContextThread的工作原理

### 系统调用 (Syscall)：CPU 从用户模式（Ring 3）切换到内核模式（Ring 0）

当调用 `NtGetContextThread` 时，程序通过 `ntdll.dll` 中的对应函数触发一个软件中断或使用特定的 CPU 指令（如 `syscall`），这使得 CPU 从较低权限的用户模式（Ring 3）切换到具有完全访问权限的内核模式（Ring 0）。这是操作系统对硬件资源进行受保护访问的基础机制。

### 查找线程对象：内核根据 ThreadHandle 找到对应的内核线程对象（KTHREAD/ETHREAD）

在内核模式下，系统调用处理程序接收到的 `ThreadHandle` 参数是一个句柄，它指向一个用户态线程对象。内核首先必须验证这个句柄的有效性，然后将其解析（或“引用”）为对应的内核数据结构——通常是 `KTHREAD` 或 `ETHREAD` 结构体。这个过程确保了内核操作的是正确的线程对象。

### 定位 Trap Frame

#### 如果目标线程是当前线程：由于已经进入内核，原本用户态的寄存器值已经被保存在内核栈的一个叫 Trap Frame 的结构中。内核直接从这里读取数据

当系统调用是由当前正在运行的线程发起时，CPU 在进入内核模式时已经自动将用户态的寄存器状态保存到了该线程内核栈上的一个称为“陷阱帧”（Trap Frame）的结构中。因此，内核可以直接从当前线程的内核栈中访问这个 `TRAP_FRAME` 结构来获取寄存器值，无需额外挂起或切换线程。

#### 如果目标线程是其他线程：内核可能会挂起（Suspend）该线程，确保其状态稳定，然后从其内核栈中读取保存的寄存器状态

如果 `ThreadHandle` 指向的是另一个线程（非调用线程），内核通常需要先挂起该目标线程。挂起操作是为了防止在读取其寄存器状态时，该线程的状态发生变化（例如正在执行指令）。一旦线程被挂起，内核就可以安全地访问该线程内核栈上保存的最新 `TRAP_FRAME`，从中读取寄存器值。

### 过滤与拷贝：内核检查你传入的 ContextFlags，只将你请求的部分从内核空间拷贝到你提供的用户空间 pContext 缓冲区中

用户程序在调用时需要提供一个 `CONTEXT` 结构体的指针和一个 `ContextFlags` 掩码。`ContextFlags` 指明了需要获取或设置哪些寄存器组（例如，整数寄存器、控制寄存器、调试寄存器等）。内核会根据这个掩码，仅将请求的寄存器数据从内核空间的 `TRAP_FRAME` 拷贝到用户空间提供的缓冲区 (`pContext`) 中。这既提高了效率，也增加了灵活性。

---

### 为什么 dinvk (红队/黑客工具) 偏爱它？

通常 Win32 API 提供了 `GetThreadContext`（在 `kernel32.dll` 中），它是 `NtGetContextThread` 的封装。但在安全领域，直接使用 Nt 版本有特殊意义：

#### 绕过用户态 Hook (User-mode EDR evasion)

安全软件（EDR/AV）为了监控可疑行为，经常在用户层钩住（Hook）像 `kernel32!GetThreadContext` 这样的高级 API，因为读取或设置线程上下文通常是进程注入、代码注入等技术的前奏。通过直接调用底层的 `ntdll!NtGetContextThread`，或者更进一步，使用像 dinvk 项目中的间接系统调用（Indirect Syscall）技术，可以绕过 `kernel32.dll` 这一层的监控钩子，使得操作对 EDR 更加隐蔽。

#### 设置硬件断点 (Hardware Breakpoints)

这是 dinvk 中 `src/breakpoint.rs` 文件实现的核心功能。硬件断点利用 CPU 的调试寄存器（Dr0-Dr7）来监控内存访问或指令执行，而无需修改目标内存的代码页。`NtGetContextThread` 和 `NtSetContextThread` 是实现这一功能的关键：

1. 首先，使用 `NtGetContextThread` 获取目标线程当前的 `CONTEXT`。
2. 然后，在 `CONTEXT` 结构中设置调试寄存器（例如，将要监视的内存地址放入 Dr0，并在 Dr7 中配置断点类型和条件）。
3. 最后，使用 `NtSetContextThread` 将修改后的上下文应用回目标线程。
这种方法比修改代码（软件断点）或内存权限更隐蔽，不易被基于内存完整性或代码页监控的安全机制检测到。

---

### 使用流程总结

一个标准的使用 `NtGetContextThread` 的流程如下：

1. **分配内存**：定义一个 `CONTEXT` 类型的变量。通常是在栈上分配（`CONTEXT ctx;`），但也可以动态分配。
2. **设置掩码**：在调用前，必须设置 `ctx.ContextFlags` 字段。例如，要操作调试寄存器，就需要设置 `ctx.ContextFlags = CONTEXT_DEBUG_REGISTERS;`。如果不设置正确的标志位，内核不会填充或更新相应的寄存器数据。
3. **调用读取**：调用 `NtGetContextThread(NtCurrentThread(), &ctx);` 来获取当前线程的上下文。也可以传入其他线程的句柄。
4. **执行逻辑**：读取或修改 `ctx` 结构中的字段。例如，检查 `ctx.Dr0` 是否已被其他调试器使用，或者修改 `ctx.Rip`（指令指针）来重定向程序的执行流。
5. **调用写入（可选）**：如果修改了 `ctx` 中的数据，并希望这些更改生效（例如，应用新的硬件断点设置或更改 `Rip`），必须随后调用 `NtSetContextThread`，将修改后的 `ctx` 结构设置回目标线程。

---

### 总结

`NtGetContextThread` 是一个赋予程序强大“内省”（Introspection）能力的底层系统函数。它允许代码捕获并保存 CPU 在某一时刻的完整状态快照。在合法开发中，它是编写调试器（Debugger）和性能分析工具的基础。而在安全开发（无论是攻击还是防御）中，它被用于实现高级的代码钩子（Hook）、进程注入、反调试和反分析技术，是理解现代用户态恶意软件规避技术的关键组件之一。
