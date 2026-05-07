- [format\_args!](#format_args)
- [\_print(format\_args!($($arg)\*));](#_printformat_argsarg)
- [OutputDebugStringA](#outputdebugstringa)
- [debug\_log!用途](#debug_log用途)
- [debug\_log!调用的逻辑](#debug_log调用的逻辑)



本章不仅仅有项目的输出,也包含core::fmt的相关概念

## format_args!




## _print(format_args!($($arg)*));

**这里调用了format_args!,但为啥没有实现其必须的Display triat和Display需要的fmt方法?**  
1.  ConsoleWriter 作为“打印机”，不需要实现fmt。它只需要负责把接收到的现成字符串发给 WindowsAPI（OutputDebugStringA）
2.  虽然你最终调用了 Windows API，但 println! 宏内部依然在疯狂使用Display 和 fmt
* 当你写 println!("Value: {}", 42) 时，Windows API只认识字符串，它不认识数字 42
* 谁把 42 变成 "42" 的？ 正是 Rust 官方为 i32 类型实现的 Display trait 和fmt 方法
* 你的 println! 宏利用 Rust编译器生成的“配方”，把各种类型转换成字符流，最后汇聚到你的ConsoleWriter 里



## OutputDebugStringA

dinvk中直接调用了Windows 的WriteConsoleA,将字符串写入控制台,这样可以在cmd或powershell窗口里显示结构.但  
1. 容易被挂钩 (Hooking)：EDR 监控程序非常喜欢挂钩 WriteConsole 相关的 API
2. 增加IAT指纹
3. 如果将shellcode注入explorer.exe或svchost.exe中,这些进程本身没有控制台窗口,这会增加可疑性

puerto:  
1. 调用OutputDebugStringA,无论注入到什么进程,它是win给开发者静默调试的通信管道,不依赖窗口,不改变进程属性.EDR看来.这更像是普通的带有调试信息的合法软件
2. 只链接了OutputDebugStringA,且容易通过module模块动态查找其地址,可以实现无IAT的效果
3. 只能通过windbug看到,对用户来说这完全是静默的

如果在最终编译时彻底去掉console.rs呢  
1. 容易在发布release版之前忘了去掉所有和console.rs相关的调用或内容
2. puerto (利用 `#[cfg(debug_assertions)]`),属于 “编译器级” 手段。你不需要删除任何文件.运行 cargo build --release 时，编译器会直接在 语法树 (AST)阶段把 debug_log! 宏里的所有内容（包括那些敏感的调试字符串）全部丢弃.实现“代码在，但痕迹无”，这比手动删除更安全、更不容易出错
3. 结论:坚持使用 puerto 的宏方案


## debug_log!用途

结合puerto中macro.rs中自定义的debug_log!宏,具体的使用场景:  
1. 快速查看复杂结构体  
```rust
// 如果你给结构体加了 derive
#[derive(Debug)]
 struct MyModule {
   base: *mut c_void,
   size: usize,
}

// 这里的_是啥?
let m = MyModule { base: 0x123 as _, size: 1024 };
    
// 使用场景：不需要手动打印每个字段
debug_log!("Module info: {:?}", m);
// 输出内容自动生成为: Module info: MyModule { base: 0x123, size: 1024 }
```
2. 打印原始字节缓冲 (Hex Dump)  
当你从内存里读取了一段数据（如 [u8; 16]），你想看看里面的原始数值：  
```rust
let buffer = [0x4D, 0x5A, 0x90, 0x00];
debug_log!("Buffer content: {:?}", buffer);
// 输出: Buffer content: [77, 90, 144, 0]
```

3. 错误枚举 (Enums)  
当你的函数返回 Error 时，你想知道具体是哪个错误变体  
```rust
// Error 枚举通常都会 derive Debug
   debug_log!("Function failed with: {:?}", Error::ModuleNotFound);
// 输出: Function failed with: ModuleNotFound
```

* 自定义的Debug_log!宏,可以放心大胆的在代码里使用`ebug_log!("PE Header: {:?}", pe)`
  *  在 Debug 模式 下：它会帮你打印出所有字段，调试很爽
  *  在 Release 模式下：由于宏被整个抹除，编译器发现没有任何地方用到那个复杂的 Debug实现，会通过 Dead Code Elimination (DCE)把那些包含字段名的垃圾代码和字符串彻底删掉


## debug_log!调用的逻辑

1. 编译期：检查与配方生成

   * 逻辑：format_args! 扫描模板，检查 Trait，生成 Arguments。
   * 对应源码：

```rust
// 在 debug_log! 宏内部
$crate::println!($($arg)*);

// 进而展开为 println! 宏
$crate::macros::_print(format_args!($($arg)*)); // <--- 关键点！
```
       * format_args!($($arg)*)：这一行代码在编译时负责所有检查，并生成了那个神奇的 Arguments 结构体。

---

2. 运行时 - 调度：创建指挥官

   * 逻辑：_print 拿到配方，调用 write_fmt，构建 Formatter。
   * 对应源码：

```rust
pub fn _print(args: fmt::Arguments) { // <--- args 就是传进来的“配方”
    let mut writer = ConsoleWriter;   // <--- 创建你的底层写入器
    let _ = writer.write_fmt(args);   // <--- 调度开始！write_fmt 是 Write trait 自带的方法
}
```

---

3. 运行时 - 生产：变量自我渲染

   * 逻辑：Formatter 遍历配方，调用变量（如 u32）的 fmt 方法。
   * 对应源码：
       * 这一步发生在 Rust core 库的内部（你看不到源码，但它正在运行）。
       * write_fmt 内部会执行类似 arg.fmt(&mut formatter) 的操作。
       * 如果参数是 100，它会执行 i32::fmt，生成字符串 "100"。

---

4. 运行时 - 消费：喂给写入器

   * 逻辑：Formatter 将生成的字符片段喂给你的 ConsoleWriter::write_str。
   * 对应源码：

```rust
// 这里是你实现的 Trait，也是 Formatter 最终调用的目标
impl Write for ConsoleWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result { // <--- s 就是 "100" 这种片段
        // ... 具体的写入逻辑 ...
    }
}
```

---

5. 终点：栈缓冲与 API 调用

   * 逻辑：把碎片拷贝到栈上，一次性发给 Windows。
   * 对应源码：

```rust
fn write_str(&mut self, s: &str) -> fmt::Result {
    // 定义 API 原型
    unsafe extern "system" { fn OutputDebugStringA(...) }

    // 【关键】栈缓冲区，零堆分配
    let mut buf = [0u8; 1024];

    // 数据搬运：把 s (来自 Formatter) 搬到 buf (准备发给 Windows)
    let bytes = s.as_bytes();
    let len = bytes.len().min(1023);
    buf[..len].copy_from_slice(&bytes[..len]);
    buf[len] = 0; // null-terminate，因为 Windows API 需要 C 风格字符串

    // 最终发射
    unsafe {
        OutputDebugStringA(buf.as_ptr());
    }
    Ok(())
}
```

---

总结图示

把这些源码拼在一起，就是一条完整的数据流水线：

```text
[ 用户代码 debug_log! ]
       |
       v
[ format_args! ]  ----> 生成配方 (Arguments)
       |
       v
[ _print(args) ]
       |
       v
[ writer.write_fmt(args) ] ---> 只有这里才开始干活
       |
       +---> [ core 库内部循环 ]
               |
               +---> [ u32::fmt ] ---> 生成 "100"
                       |
                       v
               +---> [ ConsoleWriter::write_str("100") ]
                       |
                       v
               +---> [ buf 栈缓冲 ] ---> [ OutputDebugStringA ]
```