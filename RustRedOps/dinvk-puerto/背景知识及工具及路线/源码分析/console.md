- [为啥需要手写console](#为啥需要手写console)
- [源码解析](#源码解析)
  - [pub struct ConsoleWriter;](#pub-struct-consolewriter)
  - [core::fmt::Write trait](#corefmtwrite-trait)
  - [fn write\_str(\&mut self, s: \&str) -\> fmt::Result](#fn-write_strmut-self-s-str---fmtresult)
  - [let buffer = Vec::from(s.as\_bytes());](#let-buffer--vecfromsas_bytes)
  - [WriteConsoleAFn](#writeconsoleafn)
  - [GetStdHandle((-11i32) as u32)](#getstdhandle-11i32-as-u32)


# 为啥需要手写console

1. no_std环境下没有自带控制台
    * 平时使用的 println! 和 eprint! 宏，是定义在 `std`（标准库） 里的.当你调用 println!，它底层会经过复杂的 std::io缓冲区，最终通过系统调用向操作系统的“标准输出流”（stdout，文件描述符1）写入数据
    * no_std下,编译器根本不认识 println!,没有默认分配器：标准的 IO通常需要堆内存来做缓冲，而你在初始化分配器之前，连 Vec 都没法用;
    * 没有运行时环境：std 的输出逻辑假设程序拥有一个完整的控制台环境，而Shellcode 或注入的 DLL 往往运行在没有窗口的进程（如 lsass.exe）中

**在 no_std下，如果你想看到任何输出，你必须手写一个驱动程序来直接跟操作系统的显示接口打交道。这就是 console.rs 的技术使命**


2. 接口适配：实现 core::fmt::Write-Rust 的核心库 (core) 提供了一个极其重要的Trait：`core::fmt::Write`.core库其他方法负责格式化逻辑(把数字转为字符串/拼凑模板等),这个trait只实现了字符串发出的功能


3. core是os无关的,那么输出需要自己实现.  
* core内部完全没有“文件”、“屏幕”、“串口”的概念。它只处理内存中的字节和逻辑流
* 实现 Write Trait 时，你是在“用户空间（或驱动空间）” 写代码。这个实现是平台相关的.比如调用 ntdll!NtWriteFile 或kernel32!OutputDebugStringA

# 源码解析

## pub struct ConsoleWriter;

如注释,这个结构体自定义了core::fmt::Write这个trait的实现.  
这个struct没有定义字段,是一个unit struct(单元结构体),不占内存,只作为一个载体,用来挂载core::fmt::Write trait的实现逻辑  
Rust中,想拥有某个功能,但不要真的缓冲区是存储就可以用这种方式.(这个理解对不)

```rust
pub trait Write {
    // Required method
    fn write_str(&mut self, s: &str) -> Result;

    // Provided methods
    fn write_char(&mut self, c: char) -> Result { ... }
    fn write_fmt(&mut self, args: Arguments<'_>) -> Result { ... }
}
```

1. trait for writing or formatting into Unicode-accepting buffers or streams.
2. This trait only accepts UTF-8–encoded data and is not flushable. If you only want to accept Unicode and you don’t need flushing, you should implement this trait; otherwise you should implement std::io::Write.

## core::fmt::Write trait

**core::fmt::Write trait的作用:**  
1.  它是格式化引擎的“标准化接口” (The Standard Interface);core 库内部集成了一个极其复杂的 格式化引擎（负责处理 {} 占位符、十六进制{:X}、填充对齐等）。这个引擎是平台无关的，但它需要一个出口
* 作用：Write Trait 定义了一个统一的出口协议
* 契约内容：任何实现了 Write 的类型，都承诺提供一个 write_str(&mut self, s:&str) 方法
* 意义：这让格式化引擎不需要关心数据最终是去了 String、串口 还是 Windows控制台。它只管把处理好的字符串片段交给 write_str

[扩展-格式化引擎](#扩展-格式化引擎)

## fn write_str(&mut self, s: &str) -> fmt::Result 

参数 `s: &str`：这是 core::fmt 引擎处理好的一个 UTF-8 字符串片段.这里的字符串片段是`slice[T]`的类型吗?T的类型又是怎么确定的?对于字符串来说,T永远是`[u8]` [扩展-rust中的\&str与slice](#扩展-rust中的str与slice)  
这个函数会被引擎多次调用，直到所有碎块都传完

## let buffer = Vec::from(s.as_bytes());

```rust
 pub const fn as_bytes(&self) -> &[u8] {
        // SAFETY: const sound because we transmute two types with the same layout
        unsafe { mem::transmute(self) }
    }
```
Converts a string slice to a byte slice. To convert the byte slice back into a string slice, use the **from_utf8** function

> s.as_bytes() 将 &str 转换为 &[u8]。
> 1. 约束消除：取消了对内容的 UTF-8 编码验证约束。
> 2. 类型降维：将指针指向的类型从“文本”降级为“原始字节（u8）”。
> 3. 数值传递：两者在内存中的 16 字节胖指针保持不变。由于 u8 长度为 1字节，转换后 `&[u8]`的元素个数逻辑上等于原字符串的字节数，实现了物理长度到逻辑计数的无缝对接

**对红队开发的意义：**  
Windows API（如 WriteConsoleA）不需要“文字”逻辑，它只需要“数据”流。通过as_bytes()，我们主动放弃了 Rust 提供的高级文明保护，回到了 C语言那种“一切皆字节”的原始状态。

[扩展-as\_bytes()](#扩展-as_bytes)


## WriteConsoleAFn

在types.rs中的定义:  
```rust
pub type WriteConsoleAFn = unsafe extern "system" fn(
    hConsoleOutput: HANDLE, 
    lpBuffer: *const u8, 
    nNumberOfCharsToWrite: u32, 
    lpNumberOfCharsWritten: *mut u32, 
    lpReserved: *mut c_void
);
```

* 关于返回值
  * 在 Rust 中，如果一个函数定义没有写 -> 类型，它默认返回“单元类型” `()`（相当于 C 语言中的 void）
  * 但在 Windows 官方文档中，WriteConsoleA 确实是有返回值的：它返回一个 BOOL（即i32），用于告知调用是否成功
  * 为什么这里的 WriteConsoleAFn 定义没有写返回值？这涉及到底层编程的实用主义和Rust 语法的默认行为
    * **在 x64 调用约定 中，函数的返回值永远放在 `RAX` 寄存器里**
    * 如果定义了 `-> i32`：Rust 编译器在生成代码时，会去 RAX寄存器里读取这个值，并将其交给你的变量。
    * 如果不定义返回值：Rust 编译器在执行完 call 指令后，会直接忽略 `RAX`寄存器里的内容，就当它不存在
    * 在 console.rs 的上下文中，我们仅仅是想把调试信息印在屏幕上。即使WriteConsoleA 失败了，我们通常也不会（也不想）去处理这个错误，因为在panic_handler 这种极端环境下，如果连打印错误都失败了，我们也没什么好办法了. 因此，开发者在这里省略了返回值，仅仅是为了代码更简洁，因为它是一个“触发后即忘”（Fire and Forget）的调用

* 关于rax
  *  RAX 确实既存 SSN，也存返回值。但它们发生在 不同的时间点.RAX寄存器就像是函数调用的“传达室”。根据执行的时间段不同，它的身份会发生戏剧性的切换
    * 进入内核前 (SSN 身份)当你调用类似 NtAllocateVirtualMemory 这样的底层函数时
        1. 代码首先运行在 ntdll.dll 内部。
        2. 它会执行：mov eax, 0x18 (0x18 是 SSN)。
        3. 此时 RAX 代表 SSN。
        4. 执行 syscall 指令，CPU 带着 RAX 里的这个数字冲进内核
    * 内核处理中,内核看到 RAX 是 0x18，于是知道：“你要分配内存”。内核完成工作后，会产生一个结果（比如 0x0代表成功）
    * 返回用户态后 (返回值身份)
      1. 内核在退出前，把结果（NTSTATUS）覆盖到 RAX 寄存器中。
      2. 执行 sysret 返回。
      3. 此时 RAX 代表返回值

**为什么在 WriteConsoleAFn 里它代表返回值？**  
因为 WriteConsoleA 是一个 “一般函数” (Normal Function)，而不是直接的系统调用  
* 调用方式：你使用的是 call 指令跳转到 kernel32.dll。
* 约定：按照 Microsoft x64 Calling Convention (FastCall) 的规定：
  * 输入：参数放 RCX, RDX, R8, R9。
  * 输出：返回值必须放在 RAX 里

>以上,因为这是 FastCall (x64 默认调用约定) 规定的。在所有 x64 Windows函数中，只要函数执行完了，RAX 寄存器里装的一定是它的战果（返回值）

* pub type:Type Alias 类型别名  
这里通过dinvok!宏调用了一个Win api(BOOL WriteConsoleA),但rustc不知道目的地址处的函数是什么样子的,通过定义这个别名,告诉rustc将给定的内存地址,当作这种格式的函数来对待(宏定义中将这个地址当作函数执行了)

* `extern "system"`：指定 Calling Convention (调用约定)

* WriteConsoleAFn 为什么后面加个 A？ (ANSI vs Unicode)Windows 历史遗留
  *  `WriteConsoleW` (Wide)：接收 UTF-16 编码（每个字 2 字节）。这是 Windows内核的原生语言
  *  `WriteConsoleA` (ANSI)：接收本地编码（在我们的环境下通常兼容 ASCII/UTF-8的子集）
* 红队选择 `A` 的理由：
  1. 兼容性：Rust 的字符串片段（&str）可以直接作为字节流传给 A 版本API，而不需要进行复杂的编码转换（转为 u16 数组）。
  2. 精简：减少了代码量，不需要为了打印一个调试信息而引入大规模的字符串转码逻辑

windows官方原型:  
```c
BOOL WriteConsoleA(
  HANDLE hConsoleOutput, 
  const VOID *lpBuffer, DWORD
  nNumberOfCharsToWrite, LPDWORD lpNumberOfCharsWritten, 
  LPVOID lpReserved);
```

1. `HANDLE` (hConsoleOutput)：
       * 含义：控制台缓冲区的“准入证”。
       * 实质：就是一个指针地址（在 x64 下是 8 字节）。
2. `*const u8` (lpBuffer)：
       * 含义：指向你要印的那些字的内存地址。
       * 背景：这里用 u8 正好对应 ANSI 字符。
3. `u32` (nNumberOfCharsToWrite)：
       * 含义：告诉 API 读多少个字节。
       * 背景：Windows API 的 DWORD 对应 Rust 的 u32。
4. `*mut u32` (lpNumberOfCharsWritten)：
       * 含义：API 用来返回“实际写了多少字节”的坑位。
       * 背景：我们需要提供一个可变的内存地址，让 API 往里填数。
5. `*mut c_void` (lpReserved)：
       * 含义：保留参数。
       * 背景：微软留着备用的，目前必须传 NULL
6.  返回值：i32 (BOOL)
      * 背景知识：在 Win32 API 中，BOOL 其实就是 int。
      * 逻辑：如果成功返回非 0，失败返回 0。

**WriteConsoleAFn 是你连接 Rust 逻辑 和 Windows 硬件绘制 的合同模版**:  
当你之后执行 dinvoke!(..., WriteConsoleAFn, ...) 时：
1. dinvoke 找到了 WriteConsoleA 的原始地址。
2. Rust 按照 WriteConsoleAFn 的规定，把你的 buffer.as_ptr() 塞进 RDX寄存器，把 buffer.len() 塞进 R8 寄存器。
3. 程序跳转到 kernel32 执行

## GetStdHandle((-11i32) as u32)






























