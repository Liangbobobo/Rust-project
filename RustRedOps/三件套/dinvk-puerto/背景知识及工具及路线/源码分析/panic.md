- [Rust中的panic](#rust中的panic)
  - [panic机制](#panic机制)
    - [core::panic::PanicInfo](#corepanicpanicinfo)
- [panic时背后发生了什么](#panic时背后发生了什么)
  - [dinvk中panic的逻辑](#dinvk中panic的逻辑)
- [重构](#重构)


# Rust中的panic

**Rust中,panic指程序遇到了无法恢复的错误(Unrecoverable Error)如:**  
1. 数组越界-Index out of bounds
2. 显示调用-panic!("...")
3. unwrap()在None上的调用

## panic机制

Rust std分层非常严谨,Panic就是其中代表  

**为什么要分这么多模块？（设计哲学）**  
这是 Rust “零成本抽象” 和 “可移植性” 的体现  

1. 最小化内核 (`core`)：Rust 希望成为系统级语言（替代 C/C++）。如果 Panic 机制强绑定了catch_unwind（需要 OS），那 Rust 就不能写操作系统内核了。所以，必须把“panic 的定义”（core）和“panic 的处理”（std）切分开。

2. 按需付费：
* `puerto` (no_std)：只用core::panic。我只需要知道“出错了”和“在哪错的”，然后我自己决定是死循环还是退出进程。我不想要 catch_unwind带来的巨大运行时开销。
* Web 服务器 (std)：使用 std::panic。它需要 catch_unwind来保证一个请求崩溃不会导致整个服务器宕机。

3. 兼容性：所有的库（crate）都依赖 core。这意味着无论是在 Windows上还是在微波炉芯片上，PanicInfo的结构都是一样的。这让生态系统极其统一



**painc的结构**
1. 最底层：core::panic (模块)-定位:基石
*  core 库是完全独立于操作系统的，不依赖堆分配，也不依赖libc。它可以运行在裸机（Bare Metal）或你的 Shellcode 中
*  功能：它定义了 Panic 机制中最抽象的数据结构
   * `PanicInfo`：结构体，描述了“Panic 发生了什么”（位置、消息）
   * `Location`：结构体，描述了源代码的位置（文件名、行号）
*  为什么需要它？ 因为无论你是写操作系统内核，还是写 Web服务，你都需要一个统一的格式来描述错误

2. 最底层：core::panic! (宏)-定位:入口
* 功能：这是你在代码中调用的 panic!("error") 宏的真正定义处（在 2021 edition 及以后）
* 行为：它负责将你的错误消息格式化，并调用编译器内置的panic_impl（Panic 实现入口）
* 注意：在 no_std 环境下，这个宏最终会跳转到你用 `#[panic_handler]`标记的那个函数

```rust
macro_rules! panic {
    ($($arg:tt)*) => { ... };
}
```

3. 上层建筑：std::panic (模块)-定位:高级封装
* std库依赖操作系统（OS）。它知道什么是线程，什么是控制台，什么是堆栈
* 功能：它在 core::panic 的基础上，增加了与 OS 交互的能力
  * `catch_unwind`：捕获 Panic。这需要 OS 的异常处理支持（如 Windows SEH）
  * `set_hook`：允许你自定义 Panic发生时的行为（比如记录日志到文件，而不仅仅是打印到控制台）
  * `resume_unwind`：重新抛出捕获的 Panic
* 为什么分出来？ 因为在嵌入式或 Shellcode 开发中（no_std），没有 OS支持，这些功能根本无法实现。如果混在一起，core 就没法在裸机上跑了

4. 上层建筑：std::panic! (宏)-定位：别名 (Alias)
   * 在现代 Rust 中，std::panic! 实际上通常就是重导向到了 core::panic!。
   * 在早期版本中，它们可能有细微差别，但现在你可以认为它们在语法层面是一致的。



5. panic! 宏：一切的起点(`panic!("Something went wrong: {}", err);`)
* 主动触发Panic
* 底层,创建一个fmt::Arguments对象(格式化后的错误信息),然后调用底层的Panic入口函数
* std 和 no_std
  * 在 std 中：会根据 Cargo.toml 的配置（unwind 或abort）来决定是展开栈还是直接终止
  * 在 no_std 中：它直接跳转到你定义的 `#[panic_handler]` 函数


以上:  
* `core::panic` -> 数据定义（你是谁？你在哪？）。适合所有人
* `std::panic` -> 流程控制（捕获它！记录它！）。只适合有 OS的普通程序
* no_std:关注点应完全集中在 `core::panic`上。你需要手动实现原本由 `std` 帮你做的事情：决定 Panic发生后，世界该如何终结

###  core::panic::PanicInfo

dinvk/src/panic.rs中,这是传递给panic_handler的唯一参数,包含所有panic发生时

```rust

#[lang = "panic_info"]
#[stable(feature = "panic_hooks", since = "1.10.0")]
#[derive(Debug)]
pub struct PanicInfo<'a> {
    message: &'a fmt::Arguments<'a>,
    location: &'a Location<'a>,
    can_unwind: bool,
    force_no_backtrace: bool,
}
```



# panic时背后发生了什么

**std环境下**,Rust提供一个默认的Panic Handler,它会:  
1. stack unwinding,栈回溯,清理栈上资源,释放内存,优雅的退出当前线程
2. 打印信息,在终端打印文件名\行号\错误信息

**no_std环境下**  
* 没有默认处理器,Rust不知道如何打印信息(因为没有OS标准输出),也不知道如何退出程序(因为没有OS的进程管理)
* 编译器强制要求指定panic handler,如果声明`#[no_std]`,那么必须定义一个函数,并用`#[panic_handler]`属性进行标记,否则编译会直接报错

**Panic 处理在免杀（OpSec）中至关重要**  
* 特征泄露：默认的 Panic 逻辑通常会把源代码的文件名（如src/main.rs）和错误信息（如 "index out of bounds"）编译进二进制文件。安全分析师用 strings命令一扫，就能看到你的项目结构和逻辑意图
* 行为暴露：如果你的木马在运行中崩溃并弹出了一个控制台窗口打印错误，那就彻底暴露了

## dinvk中panic的逻辑

1. 触发层：代码崩溃，跳转到 panic_handler。
2. 数据层：编译器准备好 PanicInfo，里面有 info.message()（一个 PanicMessage类型）。
3. 协议层 (writeln!)：
* 你调用 writeln!(console, "{}", info.message())。
* core 的格式化代码开始运行，它把 PanicMessage 里的内容提取出来。
4. 接口层 (write_fmt)：
* core 把解析出的原始字符串片段（比如 "index out of bounds"）传给write_fmt。
5. 驱动层 (你的实现)：
* write_fmt 调用你手写的 ConsoleWriter::write_str。
* 你在这里接入 Windows API。
* 数据最终通过 OutputDebugStringA 被 Windows 操作系统捕捉，并显示在 WinDbg 里。

**这种设计非常强大**  
1. 极高的可测试性：如果你把输出改为写内存缓冲区，你可以在没有任何 OS的情况下测试格式化逻辑
2. 极致的隐蔽性：在 puerto 中，你可以通过修改 write_str的实现，随时把输出从“控制台”切换到“无痕的内存日志”甚至是“网络套接字”，而上层的 `panic_handler` 代码逻辑一行都不用改


# 重构

应该分两种情况来写 panic.rs

1. Debug 模式：保留详细的打印功能（类似 dinvk），方便你开发调试。
   2. Release 模式：
       * 静默处理：什么都不打印，直接退出。
       * 消除特征：确保文件名和行号信息被剥离。