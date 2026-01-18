- [winapis](#winapis)
  - [pub fn \_\_readgsqword](#pub-fn-__readgsqword)
    - [3. 操作数约束 (Operands)](#3-操作数约束-operands)
    - [4. 选项 options(...)](#4-选项-options)
    - [总结](#总结)
  - [pub fn GetStdHandle](#pub-fn-getstdhandle)
    - [参数解剖：那些“负数”的秘密](#参数解剖那些负数的秘密)
    - [三、 深入内核与 PEB：句柄藏在哪里？](#三-深入内核与-peb句柄藏在哪里)
      - [1. PEB 里的缓存](#1-peb-里的缓存)
      - [2. 函数运作逻辑](#2-函数运作逻辑)
    - [四、 红队实战场景：为什么要关注它？](#四-红队实战场景为什么要关注它)
      - [1. 调试与隐蔽输出 (Console Logging)](#1-调试与隐蔽输出-console-logging)
      - [2. I/O 重定向 (I/O Redirection / Piping)](#2-io-重定向-io-redirection--piping)
      - [3. 检测沙箱 (Sandbox Detection)](#3-检测沙箱-sandbox-detection)
  - [pub fn NtSetContextThread](#pub-fn-ntsetcontextthread)
    - [1. hthread: 权限的门槛](#1-hthread-权限的门槛)
    - [2. lpcontext: 精确的手术刀](#2-lpcontext-精确的手术刀)
    - [二、 内核黑箱：当你调用 Set 时发生了什么？](#二-内核黑箱当你调用-set-时发生了什么)
      - [1. 合法性检查 (Sanitization)](#1-合法性检查-sanitization)
      - [2. 写入流程](#2-写入流程)
    - [三、 dinvk 中的核心用途：激活上帝模式](#三-dinvk-中的核心用途激活上帝模式)
      - [1. 硬件断点机制](#1-硬件断点机制)
      - [2. 操作步骤](#2-操作步骤)
    - [四、 进阶：其他的红队玩法 (Thread Hijacking)](#四-进阶其他的红队玩法-thread-hijacking)
    - [五、 风险与检测 (OpSec)](#五-风险与检测-opsec)
    - [总结](#总结-1)
  - [pub fn NtGetContextThread](#pub-fn-ntgetcontextthread)
    - [参数hthread: HANDLE](#参数hthread-handle)
    - [参数lpcontext: \*mut CONTEXT](#参数lpcontext-mut-context)
  - [AddVectoredExceptionHandler](#addvectoredexceptionhandler)
    - [参数](#参数)
    - [该函数涉及的数据结构](#该函数涉及的数据结构)
      - [PVECTORED\_EXCEPTION\_HANDLER](#pvectored_exception_handler)
      - [pub ExceptionRecord: \*mut EXCEPTION\_RECORD](#pub-exceptionrecord-mut-exception_record)
      - [pub ContextRecord: \*mut CONTEXT](#pub-contextrecord-mut-context)
  - [NtCreateThreadEx —— dinvk 中的定义详解](#ntcreatethreadex--dinvk-中的定义详解)
    - [参数 1: thread\_handle](#参数-1-thread_handle)
    - [参数 2: desired\_access](#参数-2-desired_access)
    - [参数 3: object\_attributes](#参数-3-object_attributes)
    - [参数 4: process\_handle (🔥🔥🔥 核心参数)](#参数-4-process_handle--核心参数)
    - [参数 5: start\_routine](#参数-5-start_routine)
    - [参数 6: argument](#参数-6-argument)
    - [参数 7: create\_flags](#参数-7-create_flags)
    - [参数 8: zero\_bits](#参数-8-zero_bits)
    - [参数 9: stack\_size](#参数-9-stack_size)
    - [参数 10: maximum\_stack\_size](#参数-10-maximum_stack_size)
    - [参数 11: attribute\_list](#参数-11-attribute_list)
    - [第三部分：总结](#第三部分总结)
    - [关于 `dinvk` 的特别说明](#关于-dinvk-的特别说明)
  - [LoadLibraryA](#loadlibrarya)
  - [NtAllocateVirtualMemory](#ntallocatevirtualmemory)
    - [双指针](#双指针)
    - [in out](#in-out)
  - [win32 api 和 导出函数(Native api)](#win32-api-和-导出函数native-api)

# winapis

## pub fn __readgsqword

```rust
#[inline(always)]
#[cfg(target_arch = "x86_64")]
pub fn __readgsqword(offset: u64) -> u64 {
    let out: u64;
    unsafe {
        core::arch::asm!(
            "mov {}, gs:[{:e}]",// 要生成的汇编指令原型
            lateout(reg) out,   // 对应第一个占位符{},out是u64,编译器会分配一个64为通用寄存器,如果编译器分配rax,这里就变成rax
            in(reg) offset,
            options(nostack, pure, readonly),
        );
    }

    out
}
```

汇编模板字符串："mov {}, gs:[{:e}]"

这是告诉编译器要生成的汇编指令原型。

- `mov`: 数据传送指令。将源操作数的数据复制到目的操作数。
- `{}` (占位符 0):
  - 对应后面定义的第一个操作数（即 out）。
  - 因为 out 是 u64 类型，编译器会分配一个 64 位的通用寄存器（如 rax, rbx, r8 等）。
  - 最终替换：如果编译器分配了 rax，这里就变成了 rax。
- `,`: 操作数分隔符。
- `gs:` (段前缀):
  - 这是 x64 架构的关键。它强制 CPU 使用 GS 段寄存器 的基地址进行寻址。
  - 如果不加这个前缀，默认是使用 DS 段（数据段），那就变成读取普通内存了。
- `[...]`: 内存寻址符号。表示“取这个地址里的内容”，而不是取地址本身。
- `{:e}` (带修饰符的占位符 1):
  - 对应后面定义的第二个操作数（即 offset）。
  - `:` 分隔符：后面跟的是修饰符 (Modifier)。
  - `e` 修饰符: 表示 "以 32 位寄存器名称输出"。
    - 即使输入的 offset 是 u64 类型（64位），编译器分配的是 64 位寄存器（如 rcx），{:e} 会强制在生成的汇编代码中写成其对应的 32 位低位名称（即 ecx）。
    - 为什么要这样做？
      - TEB/PEB 的偏移量通常很小（如 0x30, 0x60），完全装得下 32 位。
      - 指令 `mov rax, gs:[ecx]` 是合法的。
      - 这通常是为了兼容性或微小的编码优化。如果不加 e，生成 `mov rax, gs:[rcx]` 也是完全正确的。

假设编译器分配 `rax` 给 `out`，分配 `rcx` 给 `offset`，最终生成的汇编代码是：

1. `mov rax, gs:[ecx]`

---

### 3. 操作数约束 (Operands)

这里定义了 Rust 变量如何映射到 CPU 寄存器。

**lateout(reg) out (输出)**

- `out`: 这是 Rust 中的变量名，用于接收结果。
- `reg`: 寄存器类 (Register Class)。
  - 告诉编译器：“请从通用寄存器池（General Purpose Registers）里随便挑一个空闲的给我用”。
  - 编译器可能会挑 rax, rdx, r8 等。
- `lateout`: 后期输出 (Late Output)。这是极关键的优化点。
  - 含义：告诉编译器，这个输出寄存器只在所有输入都被读取之后才会被写入。
  - 优化效果：编译器可以复用寄存器。
  - 例子：如果 offset 用了 rax 传入，指令执行时先读取 gs:[eax]，然后才把结果写回寄存器。因为读取在前，写入在后，编译器可以决定让输入 offset 和输出 out 共用同一个寄存器 `rax`！
    - 输入：rax = 0x60
    - 执行：`mov rax, gs:[eax]` (读 gs:60 的值覆盖到 rax)
    - 输出：rax 现在是结果。
  - 如果不写 late 而写 out，编译器会被迫分配两个不同的寄存器，浪费资源。

**in(reg) offset (输入)**

- `in`: 声明这是一个只读的输入变量。
- `reg`: 同样让编译器自动分配一个通用寄存器。
- `offset`: Rust 变量来源。

---

### 4. 选项 options(...)

这是给编译器的“安全承诺书”，允许编译器进行激进的优化。

- `nostack`:
  - 承诺：这段汇编代码不会执行 push 或 pop 指令，也不会修改栈指针 RSP。
  - 作用：编译器不需要在汇编代码前后插入“保存栈帧”和“恢复栈帧”的指令。如果是在频繁调用的函数中，这能显著提高性能。
- `pure`:
  - 承诺：这是一个纯函数。即：对于相同的输入 offset，它永远返回相同的结果，并且没有任何副作用（不修改全局变量，不写文件等）。
  - 作用：死代码消除 (Dead Code Elimination)。如果你调用了 __readgsqword(0x60) 但没有使用返回值，编译器有权直接把这行代码删掉，完全不执行。
- `readonly`:
  - 承诺：这段汇编指令只读取内存，绝不写入内存。
  - 作用：编译器知道这段代码不会改变程序的内存状态，因此它可以在优化周围的 Rust 代码时，不必担心缓存失效或内存重排问题。

---

### 总结

这段代码通过 Rust 的 asm! 宏，生成了一条极其精炼的 x64 机器指令。

- Rust 意图：从 offset 指定的偏移处读取 GS 段内存。
- 机器执行：`MOV` 目标寄存器, `GS:[源寄存器低32位]`。
- 性能：利用 `lateout` 复用寄存器，利用 `nostack/pure` 去除多余开销，效率等同于 C 语言的 intrinsic 函数 `__readgsqword`，是系统编程中性能优化的极致体现。

## pub fn GetStdHandle

动态调用原因：直接链接到 kernel32.lib 会导致程序的导入表（IAT）中出现 GetStdHandle。红队工具通过 dinvoke! 配合字符串混淆（s!），使 IAT 保持干净，让静态分析工具难以察觉该程序具有控制台交互行为。

在 Windows 编程中，每个控制台应用程序（Console Application）启动时，系统都会为其分配三个标准的通信通道（流）：

1. 标准输入 (`STD_INPUT_HANDLE`, -10):通常连接到键盘缓冲区。程序通过它读取用户的按键输入。
2. 标准输出 (`STD_OUTPUT_HANDLE`, -11):通常连接到当前的控制台窗口屏幕。程序通过它显示正常的文本信息。
3. 标准错误 (`STD_ERROR_HANDLE`, -12):同样连接到屏幕，但专门用于输出错误信息。即便标准输出被重定向到文件，错误信息通常依然显示在屏幕上。

`GetStdHandle` 的作用就是拿到这三个通道的“遥控器”（Handle）。拿到 Handle后，你才能使用 WriteConsole（写屏幕）或 ReadFile（读键盘）等函数。

在 dinvk 这个特定项目中，GetStdHandle扮演着非常关键的基础设施角色。由于本项目大量使用了 no_std（不依赖 Rust标准库）特性，为了实现调试信息的输出，它必须手动构建一套打印机制

### 参数解剖：那些“负数”的秘密

`handle`: `u32` 接收的是 Windows 定义的特定常量。虽然参数类型是 `u32`（无符号），但在 Windows 文档中，这些值通常被定义为：

| 常量名称         | 原始定义 (C)     | 实际 u32 值   | 含义                     |
|------------------|------------------|---------------|--------------------------|
| STD_INPUT_HANDLE  | ((DWORD)-10)     | 4294967286    | 标准输入 (Keyboard)      |
| STD_OUTPUT_HANDLE | ((DWORD)-11)     | 4294967285    | 标准输出 (Console Screen)|
| STD_ERROR_HANDLE  | ((DWORD)-12)     | 4294967284    | 标准错误 (Error log)     |

**细节陷阱：**

- 为什么用 `u32` 传负数？因为 Windows 的 `DWORD` 是无符号 32 位。在内存中，-11 的补码表示与 4294967285 完全相同。  
- 返回值：如果进程没有关联控制台（例如一个 GUI 程序或被注入的 Service），该函数可能返回 `INVALID_HANDLE_VALUE` (即 -1 强转的指针) 或 `NULL`。

---

### 三、 深入内核与 PEB：句柄藏在哪里？

`GetStdHandle` 是一个非常特殊的 API，它在大多数情况下甚至不需要进入内核态。

#### 1. PEB 里的缓存

Windows 将标准句柄缓存在每个进程的 PEB (Process Environment Block) 中。  

- 路径：PEB -> ProcessParameters (RTL_USER_PROCESS_PARAMETERS)。  
- 在 src/types.rs 中，你可以看到 RTL_USER_PROCESS_PARAMETERS 结构体：

```rust
1     pub struct RTL_USER_PROCESS_PARAMETERS {
2         // ... 其他字段 ...
3         pub StandardInput: HANDLE,
4         pub StandardOutput: HANDLE,
5         pub StandardError: HANDLE,
6         // ...
7     }
```

#### 2. 函数运作逻辑

当你调用 `GetStdHandle(STD_OUTPUT_HANDLE)` 时：

1. `kernel32.dll` 内部会读取当前线程的 `TEB`。  
2. 通过 `TEB` 找到 `PEB` 地址。  
3. 定位到 `ProcessParameters` 偏移处。  
4. 直接从 `StandardOutput` 字段读取那个 `HANDLE` 并返回给你。

**物理意义：** 这意味着 `GetStdHandle` 的执行速度极快，因为它本质上只是在读取内存中的一个预存值。

---

### 四、 红队实战场景：为什么要关注它？

在 dinvk 这样的红队框架中，这个函数有三个核心用途：

#### 1. 调试与隐蔽输出 (Console Logging)

项目中定义了 `println!` 宏。为了在不依赖标准库 (std) 的情况下实现打印，宏底层会调用 `GetStdHandle` 获取输出句柄，然后调用 `WriteConsoleA`。  

- 红队技巧：如果你正在编写一个 DLL 注入到目标进程（如 lsass.exe），调用这个函数通常会返回 NULL，因为后台进程没有控制台。通过判断返回值是否为 NULL，你的代码可以自动决定是“静默运行”还是“弹出一个控制台”。

#### 2. I/O 重定向 (I/O Redirection / Piping)

如果你正在编写一个 反弹 Shell (Reverse Shell)：  

1. 你建立一个 Socket 连接到攻击者。  
2. 你使用 `NtSetContextThread` 或 `CreateProcess` 的属性列表。  
3. 你可以手动修改 `PEB->ProcessParameters->StandardOutput` 字段，将其指向你的 Socket 句柄。  
4. 从此之后，该进程产生的所有 `printf` 输出都会直接飞向攻击者的屏幕。

#### 3. 检测沙箱 (Sandbox Detection)

一些简单的自动化沙箱在运行样本时并不模拟控制台环境。  

- 如果 `GetStdHandle(STD_OUTPUT_HANDLE)` 返回 NULL，且当前进程不是服务程序，这可能意味着程序正运行在非交互式的自动化分析环境中。

## pub fn NtSetContextThread

```rust
pub fn NtSetContextThread(
    hthread: HANDLE,// 目标线程句柄
    lpcontext: *const CONTEXT,// 新的上下文数据(只读指针)
) -> i32 {
    dinvoke!(
        get_ntdll_address(),
        s!("NtSetContextThread"),
        NtSetThreadContextFn,
        hthread,
        lpcontext
    )
    .unwrap_or(0)
}
```

### 1. hthread: 权限的门槛

- 权限要求：必须拥有 `THREAD_SET_CONTEXT` (0x0010) 权限。  

- 注意：`THREAD_GET_CONTEXT` (0x0008) 和 `THREAD_SET_CONTEXT` (0x0010) 是两个独立的权限位。很多时候你打开线程时只请求了 GET，如果你试图调用 SET，会直接返回 `STATUS_ACCESS_DENIED`。  
- 伪句柄：同样支持 `NtCurrentThread()` (-2)，用于修改自己。

### 2. lpcontext: 精确的手术刀

- 输入指针：注意这里是 `*const`，表示数据是流入内核的。  

- `ContextFlags` 的决定性作用：  
  - 内核只看 `ContextFlags` 里标记的部分。  
  - 如果你只设置了 `CONTEXT_DEBUG_REGISTERS`，内核就只会把 `Dr0-Dr7` 的值拷贝进线程状态，完全忽略 `Rax`、`Rip` 等字段。  
  - 危险：如果你设置了 `CONTEXT_FULL`，但你只初始化了寄存器的一半，剩下的全是垃圾值（0或随机数），一旦调用成功，线程恢复执行时会立即因为 `Rip` 指向 `0x00000000` 或栈指针 `Rsp` 错误而崩溃。

---

### 二、 内核黑箱：当你调用 Set 时发生了什么？

#### 1. 合法性检查 (Sanitization)

内核不会盲目信任你传入的所有数据。为了防止用户态代码破坏内核稳定性或提权，内核（`PspSetContextThreadInternal`）会进行清洗：  

- 段寄存器 (CS, DS, SS...)：你不能随意修改 CS 寄存器来切换代码段（比如从 64位切回 32位，或者指向内核段）。内核会强行修正这些值为合法值。  
- EFlags：一些敏感的标志位（如 IOPL I/O 特权级）是无法通过这种方式修改的。

#### 2. 写入流程

- **本地线程：** 直接修改当前线程内核栈上的 Trap Frame。当 `NtSetContextThread` 系统调用返回（执行 `sysret` / `iret`）时，CPU 会从修改后的 Trap Frame 恢复寄存器。  
  - 现象：函数返回的一瞬间，寄存器就已经变了。  

- **远程线程：**  
   1. 内核发送 APC 暂停目标线程。  
   2. 目标线程进入内核态，保存现场到 Trap Frame。  
   3. 内核将你的 `lpcontext` 数据覆盖到目标线程的 Trap Frame 上。  
   4. 目标线程恢复，从新的状态开始执行。

---

### 三、 dinvk 中的核心用途：激活上帝模式

在本项目中，`NtSetContextThread` 主要用于操作 调试寄存器 (Debug Registers)。

#### 1. 硬件断点机制

x64 架构允许在 CPU 层面设置 4 个硬件断点。这比传统的软件断点（修改内存写入 0xCC）要强大且隐蔽得多：  

- 无痕：不修改内存中的任何代码字节（Hash 值不变，绕过完整性校验）。  
- 灵活：可以设置为“执行时触发”，也可以设置为“读取/写入特定内存地址时触发”。

#### 2. 操作步骤

在 `src/breakpoint.rs` 中，流程通常是：  

1. 构建一个 `CONTEXT` 结构体。  
2. 设置 `ContextFlags = CONTEXT_DEBUG_REGISTERS`。  
3. `Dr0` = Syscall Address: 告诉 CPU 盯着这个地址。  
4. `Dr7` = 0x1: 告诉 CPU 启用 Dr0，并且是“执行断点 (Execute Breakpoint)”。  
5. 调用 `NtSetContextThread`。

一旦调用成功，CPU 硬件逻辑立即生效。 下一次 CPU 执行流经过该地址时，硬件会强制产生 `EXCEPTION_SINGLE_STEP` 异常，随后被我们在 `AddVectoredExceptionHandler` 中注册的 VEH 捕获。

---

### 四、 进阶：其他的红队玩法 (Thread Hijacking)

虽然 dinvk 这里主要用它做断点欺骗，但 `NtSetContextThread` 也是 线程劫持 (Thread Hijacking) 的核心 API：

1. 挂起 目标进程的某个线程。  
2. 获取 上下文 (`NtGetContextThread`)。  
3. 修改 `Rip` 指向你注入的 Shellcode 地址。  
4. 设置 上下文 (`NtSetContextThread`)。  
5. 恢复 线程。  
   - 结果：该线程“跳”到了你的恶意代码执行，执行完后再跳回去（或者直接结束）。

---

### 五、 风险与检测 (OpSec)

1. **ETW 监控：**  
   `Microsoft-Windows-Kernel-Process` 提供者会产生 `ThreadSetContext` 事件。EDR 如果订阅了这个事件，会看到一个线程正在修改另一个线程（或者自己修改自己）的寄存器。  
   - dinvk 的优势：因为是修改自己的调试寄存器，这在某些调试场景下是合法的，相比于修改远程线程的 Rip，特征稍弱，但依然会被高敏感度的 EDR 标记为“可疑调试行为”。

2. **反调试陷阱：**  
   有些程序会启动一个看门狗线程，不断调用 `NtGetContextThread` 检查主线程的 `Dr` 寄存器。如果发现非零值，就知道自己被下断点了（无论是否连接了调试器）。

---

### 总结

`NtSetContextThread` 是 “应用层对 CPU 寄存器的最高控制权”。

- **输入：** `ContextFlags` 标记的“修改补丁包”。  
- **作用：** 将用户态的意图强制写入内核态的 Trap Frame。  
- **结果：** 改变线程的执行流（`Rip`）、栈布局（`Rsp`）或调试状态（`Dr0-7`）。

在 dinvk 中，它是开启 Syscall Spoofing 魔法的开关。没有它，硬件断点就无法生效，VEH 就永远等不到那个异常。

## pub fn NtGetContextThread

```rust
pub fn NtGetContextThread(
    hthread: HANDLE,// 目标线程句柄
    lpcontext: *mut CONTEXT,// 接收数据的缓冲区指针(必须预先分配内存)
) -> i32 {
    dinvoke!(
        get_ntdll_address(),
        s!("NtGetContextThread"),
        NtGetThreadContextFn,
        hthread,
        lpcontext
    )
    .unwrap_or(0)
}
```

### 参数hthread: HANDLE

- 权限要求：这个句柄在打开时必须拥有 `THREAD_GET_CONTEXT` (0x0008)权限。如果没有这个权限（例如你只用 THREAD_QUERY_INFORMATION打开了线程），调用会直接返回 STATUS_ACCESS_DENIED。
- 自身线程：在本项目中，经常传入 NtCurrentThread()（即伪句柄-2），表示获取“我自己”的上下文。

### 参数lpcontext: *mut CONTEXT

- CONTEXT 结构体非常大（x64下超过 1KB）。
- 输入也是输出：你不能传一个全零的结构体进去！
- `ContextFlags` 字段：在调用这个函数之前，你必须设置(*lpcontext).ContextFlags。这相当于你告诉内核：“我只想要这部分数据，其他的别给我。”
- CONTEXT_CONTROL: 只要 Rip, Rsp, EFlags 等控制寄存器。
- CONTEXT_INTEGER: 只要 Rax, Rbx, Rcx 等通用寄存器。
- `CONTEXT_DEBUG_REGISTERS`: 只要 Dr0-Dr7（本项目最关心的）。
- CONTEXT_FULL: 全都要。
- 后果：如果你不设置 ContextFlags（即默认为0），内核看到你需要“0”个寄存器，于是它什么都不做，直接返回Success，你的缓冲区里依然全是 0。

## AddVectoredExceptionHandler

AddVectoredExceptionHandler 是 Windows提供的一种机制，允许程序捕获进程内发生的任何异常。注册一个监听器，监听当前进程内发生的所有异常(不仅仅是EXCEPTION_SINGLE_STEP)  
捕获 `EXCEPTION_SINGLE_STEP`：虽然 VEH本身可以捕获所有类型的异常（如内存访问冲突、除零错误等），但在 dinvk这个项目中，它的主要任务确实是捕获由硬件断点触发的EXCEPTION_SINGLE_STEP。这是实现“参数欺骗”的核心环节。

EXCEPTION_SINGLE_STEP 是由 CPU (硬件) 触发的，而不是由这个 API 注册的

**AddVectoredExceptionHandler 是 kernel32.dll提供的函数，用于在当前进程的用户态空间注册一个全局异常回调。当进程内因为硬件断点或其他原因产生异常时，CPU 会先陷入内核，随后内核将异常信息派发回用户态的 `ntdll.dll`，最后由 `ntdll.dll` 负责调用你自定义的函数。**

### 参数

```rust
pub fn AddVectoredExceptionHandler(
    first: u32,// 处理顺序
    handler: PVECTORED_EXCEPTION_HANDLER,// 自定义的处理异常的函数指针
) -> *mut c_void 
```

first: u32  
0:自定义的处理函数会被添加到 VEH 链表的末尾  
1:自定义的处理函数会被添加到 VEH 链表的头部  
在红队开发中，我们几乎总是传 1。因为EDR（安全软件）有时也会注册 VEH 来监控异常。我们需要在 EDR之前捕获异常，完成参数修改（Spoofing），然后再让系统继续运行，从而骗过EDR。

`handler: PVECTORED_EXCEPTION_HANDLER`:

- 这是一个函数指针，指向你写在 src/breakpoint.rs（或其他地方）中的那个 Rust函数。
- 一旦注册成功，Windows 操作系统承诺：  
当进程内发生任何异常（除零、内存访问违规、断点触发等）时，在操作系统杀死程序之前，先调用这个函数问问你怎么处理。

### 该函数涉及的数据结构

当异常发生时，Windows 会调用你的veh_handler函数，并传给它一个指针。这个指针指向的数据结构包含了“异常发生现场”的所有信息

传给veh_handler的指针的结构定义在src/types.rs中(exceptioninfo: *mut EXCEPTION_POINTERS):  

#### PVECTORED_EXCEPTION_HANDLER

```rust
pub type PVECTORED_EXCEPTION_HANDLER = Option<unsafe extern "system" fn(exceptioninfo: *mut EXCEPTION_POINTERS) -> i32>;
```

输入参数:exceptioninfo: *mut EXCEPTION_POINTERS.指向异常信息的指针

输出：i32。通常返回两个值之一：  

- EXCEPTION_CONTINUE_SEARCH (0): "我不处理，你问问下一个处理函数吧。"
- EXCEPTION_CONTINUE_EXECUTION (-1):"我处理好了，请根据我修改后的上下文（Context）恢复程序执行。"

#### pub ExceptionRecord: *mut EXCEPTION_RECORD

```rust
#[repr(C)]
#[derive(Clone, Copy)]
pub struct EXCEPTION_POINTERS {
    pub ExceptionRecord: *mut EXCEPTION_RECORD,
    pub ContextRecord: *mut CONTEXT,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct EXCEPTION_RECORD {
    pub ExceptionCode: NTSTATUS,//异常代码,如0x80000004 EXCEPTION_SINGLE_STEP
    pub ExceptionFlags: u32,
    pub ExceptionRecord: *mut EXCEPTION_RECORD,// 该字段是一个链表指针，用于在发生嵌套异常（即在处理异常时又触发了新异常）时，指向
  前一个异常记录以保留完整的异常因果链条。
    pub ExceptionAddress: *mut c_void,// 异常发生的内存地址(哪一行汇编地址)
    pub NumberParameters: u32,
    pub ExceptionInformation: [usize; 15],
}
```

自定义的处理函数首先读取 ExceptionCode,如果是EXCEPTION_SINGLE_STEP(0x80000004)，说明是我们的硬件断点触发了，开始干活。如果是其他代码（比如0xC0000005 Access Violation），说明程序真崩溃了，直接返回 CONTINUE_SEARCH不管。

#### pub ContextRecord: *mut CONTEXT

这是实现 Hook、参数欺骗、断点续传的关键。它就是 CPU 寄存器在内存中的映射

能够修改 `CONTEXT` 是 VEH 强大的根本原因。你可以在处理函数中修改这个结构体里的Rcx 值，当处理函数返回 EXCEPTION_CONTINUE_EXECUTION 时，Windows 会把修改后的 Rcx写入真正的 CPU 寄存器，程序就像无事发生一样继续跑，但参数已经被你换掉了。

[CONTEXT 结构深度解析](breakpoint.md#context-结构深度解析)

当你执行 dinvk::winapis::AddVectoredExceptionHandler 时，虽然代码看起来是在 Rust层运行，但它通过 `dinvoke!` 宏 完成了以下闭环，实现了在 ntdll 中的自动注册：

1. 动态穿透：它绕过静态导入表，直接在内存中找到 kernel32.dll（最终指向ntdll.dll）里的 API 入口。
2. 提交地址：它将 src/breakpoint.rs 中定义的 veh_handler函数的内存绝对地址作为参数传递给系统(通过core::mem::transmute直接将得到的地址作为函数指针进行执行)。
3. 系统接管（自动注册）：一旦这个调用进入 ntdll 的内部逻辑（即RtlAddVectoredExceptionHandler），Windows操作系统就会接手，自动完成以下动作：
      - 在进程堆中分配一个节点。
      - 把你的 veh_handler 地址填入该节点。
      - 将该节点挂载到 ntdll 内部维护的全局链表（LdrpVectoredHandlerList）中。

  结论：
  是的。只要这行代码执行成功（返回了一个非空的句柄），注册就完成了。从此以后，该进
  程内发生的任何异常（包括由硬件断点触发的异常），ntdll
  都会“自动”去查那个链表，并跳转到你的 veh_handler 中执行。

## NtCreateThreadEx —— dinvk 中的定义详解

NtCreateThreadEx 是 Windows Vista 之后引入的，它比老的 NtCreateThread 多了一个AttributeList 参数，这使得它成为了 Windows 线程创建功能的集大成者。

1. NtCreateThreadEx 是 Windows内核导出（ntdll.dll）的一个未公开（Undocumented）函数。它是用户态通往内核态创建执行单元的最后一道门。

2. 当你调用 CreateThread（win32 api中Kernel32中的函数）,其他高级语言创建线程如C#时.最终都会汇聚到ntdll中NtCreateThreadEx这个函数。对于红队来说，它之所以比 CreateRemoteThread 更重要，是因为它提供了更强大的控制能力（如AttributeList），且更贴近内核，特征更少。

现在我们看着 dinvk 项目中 src/winapis.rs 的定义。这是您最关心的部分。

```rust
1 pub fn NtCreateThreadEx(
2     mut thread_handle: *mut HANDLE,             // 参数 1
3     mut desired_access: u32,                    // 参数 2
4     mut object_attributes: *mut OBJECT_ATTRIBUTES, // 参数 3
5     mut process_handle: HANDLE,                 // 参数 4
6     start_routine: *mut c_void,                 // 参数 5
7     argument: *mut c_void,                      // 参数 6
8     create_flags: u32,                          // 参数 7
9     zero_bits: usize,                           // 参数 8
10    stack_size: usize,                          // 参数 9
11    maximum_stack_size: usize,                  // 参数 10
12    attribute_list: *mut PS_ATTRIBUTE_LIST      // 参数 11
1)  -> NTSTATUS
```

我们将逐一解析每一个参数。请注意区分 Type (类型)、Substance (实质) 和 Purpose (作用)。

### 参数 1: thread_handle

- **Rust 类型:** `*mut HANDLE`  

- **实质:** 双重指针 (Pointer to a Handle)。  
  - `HANDLE` 本质上是一个整数（isize 或 usize），它是内核对象表的一个索引。  
  - 这个参数是一个指向内存地址的指针，这个地址准备用来接收那个整数。  
- **作用:** 输出 (Output)。  
  - 既然是“Output”，意味着在调用函数前，你不用给它赋值（或者给个空的）。  
  - 当函数执行成功后，操作系统会把新诞生的那个线程的 ID 牌（句柄）写到这个指针指向的内存里。  
  - 拿到这个句柄，你以后才能控制这个线程（比如杀掉它、挂起它）。

### 参数 2: desired_access

- **Rust 类型:** `u32`  

- **实质:** 位掩码 (Bitmask)。  
- **作用:** 权限声明。  
  - 你希望拿到的那个 `thread_handle` 有多大的权力？  
  - `0x1FFFFF` (THREAD_ALL_ACCESS): 我要生杀予夺的所有权力。  
  - `0x0040` (THREAD_SUSPEND_RESUME): 我只想有权挂起或恢复它。  
  - 如果这里权限申请太大，而你当前权限不足，函数会失败。

### 参数 3: object_attributes

- **Rust 类型:** `*mut OBJECT_ATTRIBUTES`  

- **实质:** 结构体指针。  
- **作用:** 对象属性。  
  - 主要用于设置安全描述符（Security Descriptor），或者决定这个句柄是否可以被子进程继承。  
  - 在绝大多数红队开发中，这里直接传 NULL (也就是 Rust 里的 `null_mut()`)，表示使用默认安全属性。

### 参数 4: process_handle (🔥🔥🔥 核心参数)

- **Rust 类型:** `HANDLE`  

- **实质:** 整数索引 (指向内核对象表)。  
- **作用:** 指定“车间”。  
  - 这个参数决定了新线程将寄生在哪个进程的内存空间里。  
  - 自我创建: 传入 `NtCurrentProcess()` (即 -1)，表示在当前程序里创建。  
  - 远程注入: 传入 `OpenProcess` 打开的别的进程（如 explorer.exe）的句柄。  
  - 注意: 这是一个值传递。你传的是那把“钥匙”的副本。

### 参数 5: start_routine

- **Rust 类型:** `*mut c_void`  

- **实质:** 内存地址 (Function Pointer)。  
- **作用:** 入口点。  
  - 告诉“工人”第一步去哪里执行指令。  
  - 极度重要: 这个地址必须是 在 `process_handle` 所指向的那个进程的虚拟内存空间里 有意义的地址。  
  - 如果你是远程注入，你必须先用 `NtWriteVirtualMemory` 把代码写到目标进程，拿到地址 `AddrX`，然后把 `AddrX` 传给这里。如果你传本地函数的地址，远程线程一运行就会因为找不到代码而崩溃（Access Violation）。

### 参数 6: argument

- **Rust 类型:** `*mut c_void`  

- **实质:** 数据指针 或 通用数据。  
  - 虽然类型是指针，但你可以把它当成一个 `u64` 的整数容器。  
- **作用:** 上下文参数。  
  - 这是传递给 `start_routine` 函数的唯一参数。  
  - 在 x64 汇编层面，线程启动时，这个值会被直接这就放进 `RCX` 寄存器。  
  - 如果不需要传参，这里给 NULL (0)。

### 参数 7: create_flags

- **Rust 类型:** `u32`  

- **实质:** 标志位。  
- **作用:** 出生状态。  
  - `0`: 线程创建后立即开始奔跑。  
  - `4` (`CREATE_SUSPENDED`): 重点。线程创建后是“冻结”的。内核创建了它，分配了栈，但 CPU 不会调度它。  
  - 为什么需要挂起？  
     这给了黑客一个时间窗口。在线程跑起来之前，你可以修改它的寄存器（`NtSetContextThread`），或者修改它要运行的代码，然后再手动解冻（`NtResumeThread`）。

### 参数 8: zero_bits

- **Rust 类型:** `usize`  

- **实质:** 整数。  
- **作用:** 内存布局控制。  
  - 告诉内核：分配线程栈的时候，我不希望地址太高。如果这是 n，则栈地址的高 n 位必须是 0。  
  - 通常传 0，让操作系统看着办。

### 参数 9: stack_size

- **Rust 类型:** `usize`  

- **实质:** 字节数。  
- **作用:** 栈的初始提交大小 (Commit)。  
  - 线程栈需要占用物理内存。这里指定初始分配多少物理内存。  
  - 传 0 表示使用 PE 文件头里定义的默认值。

### 参数 10: maximum_stack_size

- **Rust 类型:** `usize`  

- **实质:** 字节数。  
- **作用:** 栈的预留大小 (Reserve)。  
  - 这是虚拟内存的大小（通常默认 1MB）。这块空间预留给栈，但不会立即占用物理 RAM。  
  - 传 0 使用默认值。

### 参数 11: attribute_list

- **Rust 类型:** `*mut PS_ATTRIBUTE_LIST`  

- **实质:** 复杂的结构体指针。  
- **作用:** 高级配置 (The Magic)。  
  - 这是 `NtCreateThreadEx` 比老旧的 `CreateRemoteThread` 强大的地方。  
  - 它是一个属性数组，可以实现非常高级的红队技术：  
    - **PPID Spoofing**: 设置 `PROC_THREAD_ATTRIBUTE_PARENT_PROCESS`，你可以指定任何一个进程作为新线程的“名义父进程”。  
    - **BlockDLLs**: 设置策略，禁止非微软签名的 DLL 加载进这个线程所在的进程。  
  - 如果不使用这些高级功能，传 NULL。

---

### 第三部分：总结

`NtCreateThreadEx` 的执行流动画：

1. 你调用函数，传入 `process_handle`（车间）和 `start_routine`（工位）。  
2. 内核在内存中创建一个 `ETHREAD` 对象（建立档案）。  
3. 内核在目标进程的内存里划出一块地做 Stack（分配工作台），并在其中初始化 `TEB`（发工牌）。  
4. 内核设置 CPU 上下文：`RIP = start_routine`, `RCX = argument`, `RSP = stack_top`。  
5. 如果没挂起，线程被放入调度队列。  
6. 轮到该线程时，CPU 加载这些寄存器值，指令指针跳转到 `start_routine`，新的生命开始了。

### 关于 `dinvk` 的特别说明

在 dinvk 的源码中，前几个参数被定义为 `mut`，这是非常规的。正如我们之前分析的，这完全是为了 Hardware Breakpoint Spoofing（硬件断点欺骗）。  

- 正常的 API 定义不需要 `mut process_handle`。  
- dinvk 需要 `mut` 是因为它要在函数内部、在发起系统调用之前，把这个句柄改成 `-1` 来骗过杀软，等进了异常处理函数再改回来。

希望这个全方位的拆解能帮助您彻底理解这个函数！

## LoadLibraryA

在 Windows 操作系统中，LoadLibraryA 是 Kernel32.dll 导出的一个非常核心的 API 函数。

它的核心定义：  
它的作用是将一个指定的模块（通常是 .dll 文件，也可以是 .exe）加载到调用进程的虚拟地址空间中。

这里的“A”是什么意思？  
Windows API 通常成对出现：

- LoadLibraryA：接受 ANSI 字符串（传统的单字节字符，如 C 语言的 char*）。
- LoadLibraryW：接受 Wide 字符（Unicode 字符串，如 wchar_t*）。
它们的功能完全一样，只是输入参数的字符编码不同。

它的具体工作流程（当你调用它时）：

1. 查找文件：系统会在硬盘上按照特定的搜索顺序（当前目录 -> 系统目录 -> PATH 环境变量等）寻找你指定名字的 DLL 文件（例如 user32.dll）。
2. 映射内存：如果找到了，系统会将这个 DLL 文件从硬盘“搬运”（映射）到你当前进程的内存空间里。
3. 处理依赖：如果这个 DLL 还需要其他的 DLL 才能运行，系统会递归地把那些 DLL 也加载进来。
4. 初始化：系统会执行该 DLL 的入口函数 DllMain，让 DLL 做一些初始化的准备工作。
5. 返回基址：最重要的一步，它会返回一个 HMODULE（句柄）。  
   - 这个句柄本质上就是该 DLL 在内存中的 起始地址（基地址，Base Address）。

为什么要用它？

- 动态扩展能力：程序不需要在编译时就把所有功能都打包进去。可以通过 LoadLibraryA 在运行时按需加载插件。
- 获取其他 API 的前置条件：如果你想调用某个 DLL 里的函数（比如 MessageBox），你首先必须确保这个 DLL 已经被加载到内存里了，并且你需要拿到它的基地址，才能计算出 MessageBox 在内存里的准确位置。

## NtAllocateVirtualMemory

是ntdll 导出的原始接口，参数细节比 Win32 API (VirtualAlloc) 更加底层  

```rust
pub type NtAllocateVirtualMemoryFn = unsafe extern "system" fn(
    ProcessHandle: HANDLE,
    BaseAddress: *mut *mut c_void,
    ZeroBits: usize,
    RegionSize: *mut usize,
    AllocationType: u32,
    Protect: u32,
) -> NTSTATUS;
```

- `NtAllocateVirtualMemory` 是 Windows 内存管理的核心 Native API。在 dinvk  
  项目中，它是实现 Shellcode 加载、间接系统调用（Indirect  
  Syscall）和参数欺骗（Argument Spoofing）的关键载体。

  下面我结合 dinvk 的源码，从函数定义、参数细节到实战调用三个维度详细解释。

- **1. 函数定义 (Rust 视角)**

  在 Rust 中，我们要严格匹配 C 语言的 ABI。参考 dinvk 项目中 `src/types.rs` 或  
  `src/winapis.rs` 的定义：

  ```rust
  // 摘自 dinvk/src/types.rs 或类似的类型定义
  use std::ffi::c_void;

  // pub type 类型别名
  // extern "system" 符合目标平台的调用约定(其他形式如 extern "C")
  // fn()->i32 表示是一个函数指针,也是一种符合这种形式的函数的类型
  pub type NtAllocateVirtualMemoryFn = unsafe extern "system" fn(
      ProcessHandle: HANDLE,          // 目标进程句柄
      BaseAddress: *mut *mut c_void,  // [关键] 指向指针的指针 (IN/OUT)
      ZeroBits: usize,                // 零位掩码 (通常为0)
      RegionSize: *mut usize,         // [关键] 指向大小的指针 (IN/OUT)
      AllocationType: u32,            // 分配类型 (Reserve/Commit)
      Protect: u32                    // 内存权限 (如 RWX)
  ) -> i32; // NTSTATUS
  ```

### 双指针

如果函数需要修改你手里的指针变量让它指向别处，你就必须传这个指针变量的地址（双指针）  
双指针,指向的是具体内容的指针的地址,如果修改双指针,修改的是指向具体内容的指针的地址  
双指针就是为了给函数一个“修改权”，让它能把你手里的那个指针重定向到新的地方。

### in out

分别代表传入的值和返回的值

- 我们来把 `NtAllocateVirtualMemory` 的 6 个参数拆解到“原子级”，结合内核原理、Rust 类型以及攻防对抗（Red Team vs EDR）的视角来详细解释。

  这是 dinvk 中定义的函数签名：

  ```rust
  pub type NtAllocateVirtualMemoryFn = unsafe extern "system" fn(
      ProcessHandle: HANDLE,          // 参数 1
      BaseAddress: *mut *mut c_void,  // 参数 2
      ZeroBits: usize,                // 参数 3
      RegionSize: *mut usize,         // 参数 4
      AllocationType: u32,            // 参数 5
      Protect: u32                    // 参数 6
  ) -> i32;
  ```

  ---

- **1. ProcessHandle (目标进程句柄)**  
  - 类型: `HANDLE` (通常是 `isize` 或 `*mut c_void`)  
  - 含义: 你想在谁的脑子里塞东西？  
  - 关键值:  
    - `-1` (即 `0xFFFFFFFFFFFFFFFF`):  
      - 含义: `NtCurrentProcess`，当前进程伪句柄。  
      - 原理: 内核看到 -1，直接操作当前线程所属的进程结构体 (`EPROCESS`)。  
      - 优势: 不需要 `OpenProcess`，速度最快，没有权限检查（永远拥有所有权）。  
    - 其他值 (如 `0x1234`):  
      - 含义: 其他进程的句柄。  
      - 前提: 你必须先调用 `NtOpenProcess` 拿到这个句柄，并且打开时必须请求 `PROCESS_VM_OPERATION` (允许操作虚拟内存) 权限。  
      - 底层: 内核会进行上下文切换 (`KeAttachProcess`)，把页表（CR3 寄存器）切到目标进程，操作完再切回来。这是远程代码注入的基础。

- **2. BaseAddress (基址 - IN/OUT)**  
  - 类型: `*mut *mut c_void` (指向指针的指针)  
  - 含义: 既是“我希望在哪里分配”，也是“实际在哪里分配了”。  
  - 输入 (IN):  
    - `NULL` (指向的变量值为 0):  
      - 系统开启 ASLR (地址空间布局随机化)，随机找个空闲位置。这是最常用、最安全的方式。  
    - 具体地址 (如 `0x00007FF712340000`):  
      - 你强制要求在这个地址分配。  
      - 场景: 恢复被挂起进程的入口点、或者 Process Hollowing（把原本在那里的合法 EXE 代码掏空，换成你的）。  
  - 输出 (OUT):  
    - 内核写入实际分配的基址。  
    - 注意: 哪怕你输入 `0x1001`，内核也会向下对齐到 `0x1000` (4KB边界)。

- **3. ZeroBits (零位掩码)**  
  - 类型: `usize`  
  - 含义: 这是一个为了兼容性存在的参数。它告诉内核：“这个地址的高位，必须有多少个 bit 是 0”。  
  - 作用: 控制分配地址的“高度”。  
  - 计算公式: 有效地址 < `(1 << (64 - ZeroBits))`  
  - 常见值:  
    - `0`: 默认值。如果是 64 位系统，就在 64 位空间随便找（0~128TB 范围）。  
    - `32` (在 64 位系统上): 强制分配在低 4GB 空间（`0xFFFFFFFF` 以下）。  
      - 场景: 当你的 Shellcode 是 32 位的（WoW64 模式），或者你用了一些只支持 32 位指针的古老汇编指令时，必须设这个值。

- **4. RegionSize (区域大小 - IN/OUT)**  
  - 类型: `*mut usize` (指向大小的指针)  
  - 含义: 既是“我想要多大”，也是“实际给了多大”。  
  - 输入 (IN):  
    - Shellcode 的字节数，比如 512 字节。  
  - 输出 (OUT):  
    - 内核总是按页 (Page) 分配。  
    - 在 x64 Windows 上，一页是 4096 字节 (`0x1000`)。  
    - 如果你输入 1，这里会被改写为 4096。  
    - 如果你输入 4097，这里会被改写为 8192 (2页)。

- **5. AllocationType (分配类型)**  
  - 类型: `u32` (位掩码标志位)  
  - 含义: 你想怎么操作这块地皮？是“先圈地”还是“马上盖房”？  
  - 常用标志:  
    - `MEM_COMMIT` (`0x00001000`): [核心]  
      - 分配物理内存（RAM）或页文件。不加这个，你访问内存会直接崩（Access Violation）。  
    - `MEM_RESERVE` (`0x00002000`): [核心]  
      - 在地址空间里“占坑”，防止被别人申请走，但还没给物理内存。  
    - 通常组合: `MEM_COMMIT | MEM_RESERVE` (`0x3000`)。一步到位，既占坑又给内存。  
  - 特殊/黑客标志:  
    - `MEM_TOP_DOWN` (`0x00100000`):  
      - 告诉内核：尽量从高地址往低地址分配。  
      - 对抗: 有些简陋的 EDR/Sandbox 监控可能只盯着低地址区域，用这个有时能绕过监控。

- **6. Protect (内存权限/页保护)**  
  - 类型: `u32`  
  - 含义: 这块内存允许做什么？（读 R、写 W、执行 X）  
  - 攻防焦点: 这是 EDR 报警的重灾区。  
  - 常见值:  
    - `PAGE_NOACCESS` (`0x01`):  
      - 不可访问。通常用于做“警戒页”或者堆喷射中的占位。  
    - `PAGE_READONLY` (`0x02`):  
      - 只读。放字符串常量。  
    - `PAGE_READWRITE` (`0x04`): [安全]  
      - 可读可写。存放数据。EDR 觉得这很正常。  
    - `PAGE_EXECUTE` (`0x10`):  
      - 只执行。极其罕见。  
    - `PAGE_EXECUTE_READ` (`0x20`): [半危险]  
      - 可读可执行。标准的代码段权限（`.text` 段）。  
    - `PAGE_EXECUTE_READWRITE` (`0x40`): [极度危险 - RWX]  
      - 红队: 最爱。因为 Shellcode 往往需要自己解密（需要写权限）然后运行（需要执行权限）。  
      - 蓝队: 绝杀。除了 JIT 编译器（如浏览器 JS 引擎、C# CLR），正常程序极少申请 RWX 内存。一旦发现，直接标黑。

- **总结一张表**

  | 参数           | 方向   | 核心作用     | 典型红队用法                        |
  |----------------|--------|--------------|-------------------------------------|
  | ProcessHandle  | IN     | 确定目标进程 | -1 (自身) 或 远程进程句柄 (注入)    |
  | BaseAddress    | IN/OUT | 确定内存位置 | NULL (随机) 或 指定地址 (Hollowing) |
  | ZeroBits       | IN     | 限制地址高度 | 0 (默认)                            |
  | RegionSize     | IN/OUT | 确定内存大小 | Shellcode 长度 (会被对齐到 4KB)     |
  | AllocationType | IN     | 分配模式     | 0x3000 (`COMMIT \| RESERVE`)        |
  | Protect        | IN     | 权限控制     | RWX (`0x40`) (最敏感，也是最需要的) |
  
## win32 api 和 导出函数(Native api)

win32 api(如 virtualalloc)和ntdll.dll的导出函数NtAllocateVirtualMemory有啥不同,win32 api在系统中是否真实存在?ntdll.dll在系统中什么地方存放着?

- 区别是什么？  
  Windows 的用户模式（User Mode） API 分为不同的层级：

  - Win32 API (Documented/公开):  
    - 代表: `VirtualAlloc`, `CreateFile`, `CreateThread`。  
    - 位置: 位于 `kernel32.dll`, `user32.dll`, `gdi32.dll` 等“子系统 DLL”中。  
    - 作用:  
         它们是微软提供给开发者的官方接口。它们负责参数检查、错误处理（设置  
         `GetLastError`），以及兼容性处理。  
    - 本质: 它们通常不直接干活，而是作为“中间人”，最终调用底层的 Native  
         API。例如，`VirtualAlloc` 内部做完检查后，会调用  
         `NtAllocateVirtualMemory`。

  - Native API (Undocumented/未完全公开):  
    - 代表: `NtAllocateVirtualMemory`, `NtCreateFile`, `NtCreateThreadEx`。  
    - 位置: 位于 `ntdll.dll` 中。  
    - 作用: 它们是用户模式进入内核模式（Kernel Mode）的最后一道大门。  
    - 本质: 它们负责设置系统调用号（SSN/System Service  
         Number），然后执行汇编指令（`syscall` 或 `sysenter`）跳转到 Ring 0  
         内核层。

- Win32 API 是否真实存在？  
  是的，真实存在。  
  物理上，它们存在于 `C:\Windows\System32\kernel32.dll`  
  等文件中。逻辑上，它们是真实存在的导出函数，你可以通过 `GetProcAddress`  
  获取它们的地址并调用。  

  但在操作系统内核的视角里，Win32 API 并不存在，内核只认识 Native API（如  
  `Nt...` 系列）。Win32 API 只是用户层的封装库。

- `ntdll.dll` 存放在哪里？  
  这取决于你的操作系统位数和程序位数（WoW64机制）：

  - 64位系统上的 64位程序:  
    - 路径: `C:\Windows\System32\ntdll.dll` (这是真正的64位核心库)  
  - 64位系统上的 32位程序:  
    - 路径: `C:\Windows\SysWOW64\ntdll.dll` (这是为了兼容32位程序提供的库)  
  - 32位系统:  
    - 路径: `C:\Windows\System32\ntdll.dll`
