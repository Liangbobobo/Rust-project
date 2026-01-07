# Breakpoint

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

```html
31  30  29  28  27  26  25  24  23  22  21  20  19  18  17  16
+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
| LEN3  |  RW3  | LEN2  |  RW2  | LEN1  |  RW1  | LEN0  |  RW0  |  <-- 条件与长度控制
+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+

15  14  13  12  11  10   9   8   7   6   5   4   3   2   1   0
+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
| 0 | 0 | GD| 0 | 0 | 0 | GE| LE| G3| L3| G2| L2| G1| L1| G0| L0|  <-- 启用与辅助控制
+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
```

---

### 第一部分：启用控制位 (L0-L3, G0-G3)

位于寄存器的最低 8 位（0-7）。每两个位控制一个地址寄存器（Dr0-Dr3）。

- **L0 (Bit 0) - Local Enable 0**:  
  - **含义**: 局部启用 Dr0 断点。  
  - **作用**: 置 1 时，CPU 会监控 Dr0 中的地址。  
  - **特性**: 在任务切换（Task Switch）时，CPU 硬件会自动将其清零。但在现代操作系统（如 Windows）中，OS 会在上下文切换时保存并恢复此位，因此它实际上在当前线程中一直有效。这是我们在 `breakpoint.rs` 中使用的位。

- **G0 (Bit 1) - Global Enable 0**:  
  - **含义**: 全局启用 Dr0 断点。  
  - **特性**: 任务切换时不会被清零。通常用于系统级调试。

- **L1/G1, L2/G2, L3/G3**:  
  - 分别对应 Dr1, Dr2, Dr3 寄存器，逻辑同上。

### 第二部分：辅助控制位 (8-15 位)

这些位通常用于高级调试场景，在常规红队开发中较少使用。

- **LE (Bit 8) & GE (Bit 9) - Local/Global Exact**:  
  - **含义**: 精确断点检测。在早期的 386/486 处理器上，用于指示 CPU 在指令执行后立即报告断点，而不是延迟报告。  
  - **现状**: 在现代 P6 架构及之后的 CPU 中，这两位通常被忽略，因为现代 CPU 默认支持精确断点。为了兼容性建议置 1，但置 0 通常也没问题。

- **GD (Bit 13) - General Detect Enable**:  
  - **含义**: 调试寄存器保护。  
  - **作用**: 如果置 1，当任何指令试图访问调试寄存器（`MOV DRx, ...`）时，CPU 会抛出异常（Debug Exception #DB）。  
  - **红队用途**: 这是一种反调试技术。如果你开启它，当安全软件试图扫描或修改你的断点时，你可以捕获到它的行为。

### 第三部分：条件与长度控制位 (16-31 位) - 最关键部分

这部分决定了断点“什么时候触发”以及“监控多宽的内存”。每个 Dr 寄存器占用 4 位（2位 R/W，2位 LEN）。

我们以 Dr0 对应的 RW0 和 LEN0 为例：

#### 1. RW0 (Bit 16-17) - Read/Write Control (触发条件)

决定了什么样的操作会触发断点。

- **00: Execution (执行)**  
  - **含义**: 仅当指令指针 (RIP/EIP) 指向 Dr0 地址并试图执行该指令时触发。  
  - **要求**: 对应的长度 (LEN) 必须为 00。  
  - **场景**: `breakpoint.rs` 使用的就是这个模式（Hook syscall 指令）。

- **01: Write Only (仅写入)**  
  - **含义**: 仅当程序向 Dr0 地址写入数据时触发。  
  - **场景**: 监控内存破坏、监视全局变量修改。

- **10: I/O Read/Write (I/O 读写)**  
  - **含义**: 仅当 CPU 执行 I/O 指令（如 `IN`, `OUT`）访问 Dr0 指定的端口时触发。  
  - **现状**: 需要 CR4 寄存器的 DE 位支持，现代用户态代码几乎不用。

- **11: Read/Write (读写)**  
  - **含义**: 读取或写入该地址时都会触发。  
  - **注意**: 仅执行指令不会触发此模式（执行有专用的 00 模式）。

#### 2. LEN0 (Bit 18-19) - Length Control (监控长度)

决定了监控的内存范围大小。

- **00: 1 Byte (1 字节)**  
  - **强制**: 如果 RW 为 00 (执行)，LEN 必须是 00。

- **01: 2 Bytes (2 字节)**  
  - **对齐要求**: 地址必须是 2 字节对齐（地址末位为 0, 2, 4...）。

- **10: 8 Bytes (8 字节 - 仅限 x64) / Undefined on x86**  
  - **对齐要求**: 地址必须是 8 字节对齐。  
  - **场景**: 监控 64 位指针或变量的读写。

- **11: 4 Bytes (4 字节)**  
  - **对齐要求**: 地址必须是 4 字节对齐。

---

### 三、 结合 breakpoint.rs 的实战解读

现在回过头看代码中的这行操作：

```rust
// 代码：set_dr7_bits(ctx.Dr7, 0, 1, 1);
// 意思是：从第 0 位开始，修改 1 个位，将其设为 1。
```

这实际上只做了 **一件事**：

- **Set Bit 0 (L0) = 1**: 启用 Dr0。

那么 RW0 和 LEN0 呢？  
因为 `ctx` 是通过 `NtGetContextThread` 获取的，或者被 `Default::default()` 初始化过。在大多数情况下，未使用的 Dr7 高位默认是 0。

- **Bit 16-17 (RW0) = 00**: 默认为 执行断点。  
- **Bit 18-19 (LEN0) = 00**: 默认为 1 字节长度。

**最终配置效果**：

> "CPU 兄弟，请帮我盯着 Dr0 里的那个地址。只要你正准备执行那个地址上的指令（RW=00），别废话，立刻暂停（Exception），告诉我一声。"

这就是为什么这段代码能精确拦截 `syscall` 指令执行的原因。

为什么“执行断点”的 LEN 必须是 0？  
因为指令的执行是基于“指令指针（RIP）”的，RIP 是一个点，不是一个范围。CPU 只需要判断 `Current_RIP == Dr0` 即可。  
因为代码执行必须从指令的首字节开始，所以 CPU 设计者规定：只要捕捉到 RIP撞上了这个首字节地址（1字节范围），就算命中。除此之外的任何范围监控，对于“执行”这个动作来说都是多余且非法的。

Dr6：状态中心 (The Status Register)
当异常发生时，异常处理程序（VEH）需要看 Dr6 才知道是谁触发了异常。

- B0 - B3 (位 0-3)：如果 Dr0 触发，B0 置 1；如果 Dr1 触发，B1 置 1。  
- BS (位 14)：Single Step。如果是单步调试（EFLAGS 的 TF 位）触发的，这一位会置 1。

---

二、 Windows 异常处理流程 (Exception Dispatching)

当硬件断点触发时，CPU 会抛出 INT 1 中断。Windows 内核捕获后，会经历以下漫长的旅程才能到达你的代码：

1. **内核陷阱处理 (Kernel Trap Handler)**：CPU 转入 Ring 0，内核接管。  
2. **KiDispatchException**：内核判断异常来源。如果是用户态异常，准备发回用户态。  
3. **NtContinue / User APC**：内核将线程状态恢复到用户态，并跳转到 `ntdll!KiUserExceptionDispatcher`。  
4. **RtlDispatchException**：这是用户态异常分发的总指挥。它会按顺序尝试：  
   - 第一轮：**VEH (Vectored Exception Handler)** —— 我们在利用的就是这个！  
   - 第二轮：**SEH (Structured Exception Handler)** —— 即基于栈的 `__try / __except`。  
   - 第三轮：**UEF (Unhandled Exception Filter)** —— 也就是“程序已停止工作”弹窗。

**为什么选择 VEH？**  

1. **优先级最高**：它比 `try/catch` 先运行。  
2. **无需修改栈**：SEH 需要修改栈帧（容易被检测），而 VEH 是通过 API 注册到堆上的一个链表中，更加隐蔽。  
3. **全进程生效**：注册一次，所有线程触发异常都会被它捕获，适合全局 Hook。

---

三、 线程上下文 (CONTEXT) 的本质

`CONTEXT` 结构体不仅仅是一堆数据，它是“时间冻结”的快照。

当 CPU 暂停一个线程时（无论是因为中断、异常还是线程调度），它必须把 CPU 内部所有寄存器的值保存到内存里，这个内存结构就是 `CONTEXT`。

- **修改的原理**：  
  你在 VEH 里修改了 `CONTEXT` 结构体内存里的 `R10` 为 `0xFFFF`。  
  当 VEH 返回 `EXCEPTION_CONTINUE_EXECUTION` 时，`ntdll` 会调用 `NtContinue` 系统调用。  
  `NtContinue` 告诉内核：“嘿，把这个线程恢复运行吧，但恢复的时候，请把 CPU 的 `R10` 寄存器设为 `0xFFFF`”。

这就是为什么修改结构体能改变 CPU 真实状态的原因。

---

四、 x64 调用约定与 Syscall 的特殊性 (非常关键)

这是理解 `breakpoint.rs` 中 `Rsp + 0x30` 这种魔术数字的关键。

1. **Windows x64 ABI (应用程序二进制接口)**  
标准的函数调用（如调用 DLL 函数）遵循以下规则：  

- 参数 1-4：存放在 `RCX`, `RDX`, `R8`, `R9`。  
- 参数 5+：存放在栈 (Stack) 上。  
- **Shadow Space (预留栈空间)**：调用者（Caller）必须在栈上预留 32 字节（0x20）的空间，就在返回地址上面。这主要是为了让被调用函数（Callee）能把寄存器参数（RCX-R9）保存回栈里方便调试。

1. **Syscall 的特殊规则 (The R10 Twist)**  
当你直接执行 `syscall` 指令进入内核时，规则发生了微小的变化：

- **CPU 的行为**：  
  执行 `syscall` 指令时，CPU 会把当前的 `RIP`（下一条指令地址）自动保存到 `RCX` 中，把 `RFLAGS` 保存到 `R11` 中。  
- **冲突**：  
  普通的调用约定里，`RCX` 存放的是第 1 个参数。但 `syscall` 指令会无情地覆盖掉 `RCX`（存入 `RIP`）。  
- **解决方案**：  
  Windows 内核约定：对于系统调用，第 1 个参数必须在 `syscall` 前从 `RCX` 移动到 `R10`。

**结论**：  

- 普通函数 Hook：看 `RCX`。  
- Syscall Hook (我们的场景)：看 `R10`。

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

- `Rsp + 0x00` 到 `Rsp + 0x20`：Shadow Space。  
- `Rsp + 0x20` 到 `Rsp + 0x28`：参数 5。  
- `Rsp + 0x28` 到 `Rsp + 0x30`：参数 6。

(注意：Rust 代码中的 `0x30` 可能是针对特定的栈帧调整，或者是包含了返回地址的偏移，具体取决于编译器如何生成 `dinvoke!` 宏的栈帧。在 `no_std` 和内联汇编环境下，栈指针的位置需要极其精确的计算)。

在 `breakpoint.rs` 中：

```rust
*(((*context).Rsp + 0x30) as *mut u32) = Protect;
```

这暗示了当前的 `Rsp` 是标准的函数调用栈顶（包含返回地址）。  

- `0x00-0x08`: Return Address  
- `0x08-0x28`: Shadow Space (32 bytes)  
- `0x28-0x30`: Param 5  
- `0x30-0x38`: Param 6

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

- C 语言: void* ptr
- Rust 对应: *mut c_void 或*const c_void

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