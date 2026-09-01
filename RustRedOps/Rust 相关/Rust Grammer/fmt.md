- [概览](#概览)
- [core::fmt](#corefmt)
  - [实现流程](#实现流程)
  - [format\_args!](#format_args)
  - [core::fmt::Display](#corefmtdisplay)
- [扩展](#扩展)
  - [扩展-格式化引擎](#扩展-格式化引擎)



# 概览

以writeln!(console, "Addr: 0x{:X}", 0x1234);为例子,概述其在core内部如何从原始苏剧变为字符碎块,最后到达自定义的驱动代码的.

**第一阶段：编译期解析 (Compiler Macro Expansion)**  
Rust 编译器（rustc）首先介入  
1. 模板静态拆分：编译器解析 "Addr: 0x{:X}\n"。它发现这串字符可以拆分为：
* 固定部分 1："Addr: 0x"
* 动态占位符：{:X} (十六进制格式)
* 固定部分 2："\n" (由 writeln 补全的换行符)
2. 类型检查：编译器检查 0x1234 是否能满足 {:X}（十六进制）的要求
3. 生成 `format_args!`(rustc在编译阶段完成的)：宏展开后会生成一个特殊的 `fmt::Arguments` 对象
* 关键点：这完全发生在编译期，生成的是一段静态的数据结构描述

**第二阶段：描述符构造 (Arguments Preparation)**

程序运行到这一行时，会在栈上准备数据

1. 构造 `fmt::Arguments` 实例：这个结构体包含两个数组指针
2. 零分配承诺：注意，这个过程不涉及任何堆内存申请。它只是把现有的数据地址临时打包。

```rust
#[lang = "format_arguments"]
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Copy, Clone)]
pub struct Arguments<'a> {
    template: NonNull<u8>,
    args: NonNull<rt::Argument<'a>>,// 这里是Argument的数据结构,不是定义这个字段的Arguments结构体
}
```

**第三阶段：核心格式化引擎 (The fmt Engine)**

此时，代码进入了 core::fmt 模块的内部算法区。这是最“烧脑”的地方

1. 递归下降转换：引擎开始遍历 Arguments。
2. 数值转字符算法：
* 引擎发现第一个参数 0x1234 需要按十六进制处理。
* 它在栈上分配一个小缓冲区（通常是几十个字节）。
* 它通过数学运算（除以 16，取余数）算出每一位：4, 3, 2, 1。
* 将余数映射到十六进制字符表：'1', '2', '3', '4'。
1. 对齐与填充处理：如果你设置了宽度（如{:08X}），引擎会在这里计算需要补多少个 '0'

**第四阶段：协议握手 (Trait Method Dispatch)**

现在，引擎手里已经握着拼好的字符碎片了，它需要把它们递出去

1. 调用 `write_fmt`：这是 core::fmt::Write trait 的默认实现。
2. 分批次回调 `write_str`：这是最精彩的“泵送”过程。
* 第一波：引擎发现有一段静态文本，它调用 console.write_str("Addr: 0x")。
* 第二波：引擎完成了数字转换，它调用 console.write_str("1234")。
* 第三波：引擎处理最后的换行符，它调用 console.write_str("\n")。


**第五阶段：物理分发 (The Implementation)**

这是你写的 ConsoleWriter 代码发挥作用的时刻:  
1. 进入你的实现：  
```rust
fn write_str(&mut self, s: &str) -> fmt::Result {
  // s 此时依次收到了 "Addr: 0x", "1234", "\n"
}
```
2. 跨越边界：你在这个函数里调用 Windows API（如 WriteConsoleA）
3. 字符落地：Windows 操作系统接管数据，操作 GPU 在显示器上绘制出对应的像素点


**分类角度:**  

1. 宏 (Macros) —— 系统的入口
   * format_args!：上帝宏。所有其他宏（write!, println!, panic!, format!）的底层全是它。它负责在编译期通过 AST（语法树）检查类型，生成 Arguments 结构体。
   * write!：搬运工。它只是把 format_args! 的结果，搬运给某个实现了 Write trait 的对象。

2. Traits (特征) —— 系统的契约
这个系统里有两种 Trait，分工极其明确：

   * A 类：Source (数据源) —— “我是什么？”
       * Display ({})
       * Debug ({:?})
       * Binary ({:b})
       * LowerHex ({:x})
       * 依赖关系：format_args! 会根据占位符，去调用数据源的 fmt 方法。

   * B 类：Sink (目的地) —— “去哪里？”
       * core::fmt::Write：极其重要。这是 no_std 下唯一的出口。你的 ConsoleWriter 就属于这里。
       * std::io::Write：这是标准库层面的（写文件、写网络）。在 puerto 中你用不到。

3. 核心方法 (Methods) —— 系统的齿轮

   * write_fmt(args)：
       * 谁调用它？：你（或 write! 宏）。
       * 它是干嘛的？：它是 “调度器”。它拿着 Arguments 这个配方，遍历里面的每一项。
       * 它依赖谁？：它依赖 Trait A (Source) 的 fmt 方法，让数据把自己画出来；然后依赖 Trait B (Sink) 的 write_str 方法，把画出来的字发出去。




# core::fmt

Utilitie公共设施 for formatting and printing strings.

无 OS 依赖，无内存分配

## 实现流程

**一、 core::fmt 的三大支柱（核心结构）**

以puerto中macro.rs的debug_log!为例

1. 消费者：Write Trait（目的地）
它定义了 “数据去哪里”。
   * 核心方法：write_str(&mut self, s: &str)。
   * 你的实现：ConsoleWriter。它不关心数据是什么，只关心怎么把字符串发给OutputDebugStringA(或其他调用/自定义的方法)

2. 生产者：格式化 Trait（数据源）
它们定义了 “数据长什么样”。不同的占位符对应不同的 Trait：
   * Display ({})：普通外观。
   * Debug ({:?})：程序员调试外观。
   * LowerHex ({:x})：十六进制外观。
   * Pointer ({:p})：地址外观。

3. 胶水/配方：Arguments 结构体
这是 core::fmt 最天才的设计。它是 “如何拼装这顿饭的配方”。
   * 来源：由 format_args! 宏在编译期生成。
   * 特性：栈分配、零拷贝、延迟执行。它只存引用，不存数据。
   * 

**二、 宏与 Trait 的依赖关系图**

让我们看这个“指挥链”：

1. 核心工厂：format_args!
   * 地位：所有格式化宏的“祖宗”。
   * 作用：将模板字符串和变量打包成 Arguments 结构体.下一步可以根据显示的方式调用println!或debug!(当然需要实现对应的trait)
   * 依赖：它会根据你写的占位符（如 {:?}），要求变量必须实现对应的 Trait（如Debug）。

2. 执行桥梁：write! / writeln!
   * 用法：write!(output_target, "fmt", args)。
   * 依赖：
       1. output_target 必须实现 Write Trait。
       2. "fmt" 和 args 会被传给 format_args!。
   * 逻辑：它负责把“消费者”和“配方”连在一起。

3. 便利工具：println! / eprintln!
   * 地位：只是 write! 的一个包装。
   * 你的实现：在 puerto 中，你定义它们是为了模拟标准库体验。


**三、 系统性架构图 (ASCII Map)**

```text
[数据源: 变量] --依赖--> [格式化 Trait: Debug/Display]
      |                          |
      +----------+---------------+
                 |
        [ 宏: format_args! ]  <-- 核心调度员 (编译期)
                 |
        生成 [ 结构体: Arguments ] (内存中的配方)
                 |
        传递给 [ 方法: write_fmt ] (core::fmt 定义的通用逻辑)
                 |
        最终调用 [ Trait: Write ] (你实现的 ConsoleWriter)
                 |
        落地到 [ 系统 API ] (OutputDebugStringA)
```

---

**四、 为什么需要定义这么多不同的宏？**

这是为了在 编译期安全 和 运行时效率 之间做平衡：

1. format_args!：为了零分配。如果你直接用 format!，它会强制申请堆内存。而format_args! 只在栈上生成一个临时的配方。
2. write! vs println!：
    * write! 是为了通用性（可以写到文件、控制台、网络）。
    * println! 是为了便捷性（自动定位到标准输出）。
3. debug_log! (你的自定义宏)：
    * 这是为了 条件编译 (Conditional Compilation)。
    * 它的存在是为了在 Release模式下，利用宏的特性物理抹除掉整个调用链，包括那些敏感的字符串模板。

---

五、 总结你的 debug_log! 宏调用链

当你写下 debug_log!("Base: {:p}", ptr) 时，发生的递归依赖如下：

1. debug_log! 检查当前是否是 Debug 模式。
2. 如果是，调用 println!。
3. println! 隐式调用 format_args!。
4. format_args! 检查 ptr 是否实现了 Pointer ({:p})。
5. format_args! 生成一个 Arguments 对象。
6. 调用你的 _print 函数，将 Arguments 传给你的 ConsoleWriter。
7. ConsoleWriter 调用 write_fmt。
8. write_fmt 最终触发你在 ConsoleWriter 里写的 write_str。
9. 你的代码通过汇编/FFI 执行 OutputDebugStringA。

核心结论：
* Write Trait 是出口（Consumer）。
* Arguments 是中间载体（The Glue）。
* format_args! 是编译期质检员（The Factory）。

这套系统的美妙之处在于：只要你实现了一个简单的 Write Trait，你就可以在 no_std环境下享受 Rust 完整的、类型安全的、高性能的字符串格式化能力。

## format_args!

core:  
Macro format_args 

```rust
 #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "format_args_macro"]
    #[allow_internal_unsafe]
    #[allow_internal_unstable(fmt_internals, fmt_arguments_from_str)]
    #[rustc_builtin_macro]
    #[macro_export]
    macro_rules! format_args {
        ($fmt:expr) => {{ /* compiler built-in */ }};
        ($fmt:expr, $($args:tt)*) => {{ /* compiler built-in */ }};
    }
```

**作用:**Constructs parameters for the other string-formatting macros.  
This macro functions by taking a formatting string literal containing {} for each additional argument passed.   
format_args! prepares准备 the
additional parameters to ensure the output can be interpreted as a string and canonicalizes标准化 the arguments into a single type. 

**使用条件:**Any value that implements the Display trait can be passed to format_args!, as can any Debug implementation be passed to a {:?} within the formatting string包含{:?}的字面量模板字符串(这里的the formatting string代指包含占位符（如 {} 或 {:?}）的字面量模板字符串).  
关于第二句,只要一个类型实现了 Debug trait（不管是你手写的，还是用`#[derive(Debug)]` 自动生成的），它就可以被填进格式化字符串中 {:?} 

**调用该宏结果:**This macro produces a value of type fmt::Arguments.  
This value can be passed to the macros within std::fmt for performing useful redirection.  
All other formatting macros (format!, write!, println!, etc) are **proxied** through this one. format_args!, unlike its derived macros, avoids heap allocations.

**feature**  
1. no heap allocation
2. You can use the fmt::Arguments value that format_args! return
```rust
let args = format_args!("{} foo {:?}", 1, 2);
let debug = format!("{args:?}");
let display = format!("{args}");
assert_eq!("1 foo 2", display);
assert_eq!(display, debug);
```

**Argument lifetimes**  
Except when no formatting arguments are used, the produced fmt::Arguments value borrows temporary values. To allow it to be stored for later use, the arguments’ lifetimes, as well as those of temporaries they borrow, may be extended when format_args! appears in the initializer expression of a **let statement**.   
format_args!产生的fmt::Arguments类型的值,该值是一个借用的临时值,除非使用let赋值语句延长其lifetime,该临时值会在离开其scope后消失不能在后续中使用
```rust

let x = 42;
let args = format_args!("Value: {}", x);
// 发生了什么？：format_args! 并没有复制 x。它在 args 结构体里存了一个指向 x 的引用 (&x)。
// 风险：如果 x 很快就销毁了，而 args 还在，那 args 里的指针就变成了指向无效内存的“野指针”。

// 情况 A：没有寿命扩展 (危险！)

// 假设有一个函数返回 Arguments
fn get_args() -> fmt::Arguments<'static> {
    format_args!("Temp: {}", 42) // ❌ 报错：42 是临时值，不能存活到函数返回
}

// 情况 B：利用 let 扩展寿命 (安全)

{
    // 编译器看到 args 是在 let 语句里定义的
    let args = format_args!("Temp: {}", 10 + 20);

    // 魔法发生：编译器会自动让那个临时值 (30) 的寿命，
    // 延长到和 args 一样长！

    println!("{}", args); // ✅ 安全，30 还没消失
}
// 到这里，args 和它借用的 30 一起销毁。

let my_saved_args;
{
    let x = 42;
    my_saved_args = format_args!("{}", x); // 借用了 x
}
// 到这里，x 销毁了！
println!("{}", my_saved_args); // ❌ 砰！非法内存访问 (Crash)
```
总结:  
1. 即用即毁：在 puerto 中，永远保持 “宏里生成format_args!，立即传给打印函数，打印完立即销毁” 的模式。千万不要尝试把Arguments 存进 static 变量或结构体里。
2. 理解“借用”：记住 format_args!只是一个“影子”，它本身不持有数据，它只是一组指向数据的指针。


## core::fmt::Display

**format_args!唯一需要的条件就是需要Dispay trait**   
在 no_std 的红队工具开发中，Display triat不仅仅是为了“好看”，更是我们精确控制二进制指纹的关键 

```rust
pub trait Display: PointeeSized {
    // Required method
    //  这里Result=core::fmt::Result，成功返回 Ok(())，失败返回Err(core::fmt::Error)
    fn fmt(&self, f: &mut Formatter<'_>) -> Result;
}
```

Format trait for an empty format, {}(对{}的展开和解释)

**feature**  
1. Implementing this trait for a type will automatically implement the ToString trait for the type, allowing the usage of the .to_string() method.
2. Prefer implementing the Display trait for a type, rather than ToString
3. Display is similar to Debug, but Display is for user-facing output, and so cannot be derived.

**Completeness and parseability**完整性和可解析性  

Display for a type might not necessarily be a lossless无损的 or complete representation of the type.  

It may omit internal state, precision精度, or other information the type does not consider important for user-facing output, as determined by the type.  
As such, the output of Display might not be possible to parse, and even if it is, the result of parsing might not exactly match the original value. 

However, if a type has a lossless Display implementation whose output((这个类型的输出)) is meant to be conveniently machine-parseable and not just meant for human consumption(人类消耗,被人阅读), then the type may wish to accept the same format in FromStr, and document文档化 that usage.  
Having both Display and FromStr implementations where the result of Display cannot be parsed with FromStr may surprise users.如果一个type实现了Display和FromStr两个trait,那么Display的输出结果应该被FromStr还原  
主要表达的是对称性原则：Display ↔ FromStr:  
1. Display：将类型转为字符串 (Type -> String)
2. FromStr：将字符串转回类型 (String -> Type)
如果这两个 Trait 在同一个类型上并存，用户会产生一种心理预期  
```rust
let s = x.to_string(); 
let y: Type = s.parse().unwrap()
```
以上,对rust中实现了Display trait的type来说,也应实现FromStr.这样,对于一个结构体ApiHash:  
1. Display 让它打印出 0x1234ABCD;
2. FromStr让它能读入 0x1234ABCD 并转回 ApiHash类型
但在OpSec中,有时候会故意违反上述原则,可以让 Display 输出一种格式（迷惑分析人员），但内部 FromStr接收另一种格式（真实的解密逻辑）.或者只实现Display不实现FromStr(这种不实现FromStr的方案怎么样?)

**既然format_args!必须Display trait,而Display trait又必须fmt method,那么在puerto的macro.rs中为啥没有实现fmt method?**  
在“格式化系统中的角色分工”。你的 ConsoleWriter在这套系统中扮演的是 “消费者（目的地）”，而不是 “数据生产者”  
1. fmt 方法是给“数据”实现的，而不是给“打印机”实现的:在 core::fmt 系统中：
* 生产者 (Data)：如果你定义了一个结构体 MyStruct，你想让它能被 println!打印，你需要为 MyStruct 实现 fmt 方法（即实现 Display 或 Debug trait）
* 消费者 (Writer)：你的 ConsoleWriter是一个“打印机”。它的任务是接收别人已经处理好的字符串，并把它发出去
* 结论：format_args! 宏内部会去调用你传进去的那些参数变量的 fmt 方法，而不会去调 ConsoleWriter 的 fmt 方法

2. ConsoleWriter 实现的是 Write Trait
```rust
impl Write for ConsoleWriter{
    fn write_str(&mut self, s: &str) -> fmt::Result {
    }
}
```
*  Write Trait 只需要你实现一个方法：write_str
*  逻辑链条：
  * format_args! 把你的数据（比如数字 42）变成了字符串 "42"
  * write_fmt 方法（Write trait 自带的默认实现）会拿到这个 "42"
  * write_fmt 会自动调用你写的 write_str("42")

**这就是为什么你不需要写 fmt 方法的原因：你是在接收最终产物，而不是在生产它**  
一个自定义Display及其fmt方法的例子:  
```rust
use core::fmt;

// 1. 定义你的数据结构
struct ModuleInfo {
    name_hash: u32,
    base_address: usize,
}

// 2. 为它手动实现 Display trait
impl fmt::Display for ModuleInfo {
    // 这就是你问的 fmt 方法！
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 使用 f.write_str 或 write! 宏来定义“怎么画出这个结构体”
        // 我们想打印成：[HASH: 0x1234] AT 0x7FF...
        f.write_str("[HASH: 0x")?;
        fmt::LowerHex::fmt(&self.name_hash, f)?; // 调用数字的十六进制格式化
        f.write_str("] AT 0x")?;
        fmt::LowerHex::fmt(&self.base_address, f)?;
        Ok(())
    }
}

// 3. 如何使用？
fn test_print() {
    let info = ModuleInfo { name_hash: 0xABCD, base_address: 0x7FFE0000 };

    // 当你调用这个宏时：
    // 编译器发现 {} 占位符 -> 查找 ModuleInfo 的 Display 实现 -> 运行上面的 fmt 方法
    debug_log!("Module found: {}", info);
}
```
* 完全掌控：在 fmt 方法里，你可以决定不打印敏感的字符串，只打印代号。
* 零分配：注意上面的代码中，我们没有创建任何 String对象。我们是直接把一小段一小段的硬编码字符串（如 "`] AT 0x`"）通过 f发送出去的。
* 链式反应：你的 debug_log! 宏会调用这个 fmt，fmt 又会把数据喂给你的ConsoleWriter，最终通过 OutputDebugStringA 输出


# 扩展




## 扩展-格式化引擎

**更加深入一点--格式化引擎**  

其内部会调用format_args! 宏

以`writeln!(console, "Addr: 0x{:X}", 0x1234)`为例:  
1. 逻辑解析 (Compilation):编译看到"Addr: 0x{:X}"后,知道这里有静态部分和动态占位符{:X},且知道参数是0x1234.编译器会生成fmt::Arguments结构体.这个结构体不占用堆内存,只是在栈上记录,相关静态片段和有个数字需要按照十六进制转义
2. 格式化计算 (The Engine):接着core内部把0x1234 这个数字，通过算法转成字符 '1', '2', '3', '4';接着把这些零散的字符和之前的静态片段组合在一起.注意此时不会拼成一个巨大的String(因为没有堆内存),它是一边算一边往外传的
3. 数据泵送 (The Bridge - Write Trait): core::fmt::Write 开始出现,引擎算出了第一段 "Addr: 0x" -> 它立刻调用你的 console.write_str("Addr: 0x"),引擎算出了第二段 "1234" -> 它立刻调用你的 console.write_str("1234"),引擎最后补个换行 "\n" -> 它调用你的 console.write_str("\n")


**更加深入一点：为什么引擎是“平台无关”的**  

这是 Rust 设计最精妙的地方：数学逻辑 vs 物理 IO  

* 平台无关部分 (The Core Engine)：要把数字 10 变成字符 '1' 和 '0'，这是一个纯数学计算。无论是在Windows、Linux 还是在一个没有操作系统的计算器芯片上，数学规则是一样的。core库实现了这套算法。
* 平台相关部分 (Your Implementation)：要把字符 '1' 和 '0' 真正印在屏幕上，这是一个物理动作。


`core::fmt::Write`的真正作用是：它充当了数学逻辑和物理动作之间的“缓冲区”或“协议层”

* 输入：各种奇形怪状的类型（u32, &str, struct, [u8; 4] 等）。
* 处理：引擎根据你的占位符（如 {:p}, {:b},{:?}），把这些异构数据统统“降维打击”成最基础的、人类可读的 UTF-8 字符流。
* 输出：通过 Write 协议，分批次地将这些 &str 片段喂给你的底层驱动

Gemini对Rust的这套实现及其崇拜,认为这种实现兼顾了高性能\零内存安全风险\零os依赖


**core::fmt::Write trait实现 required Methods(唯一需要的方法)**  
1. `fn write_str(&mut self, s: &str) -> Result`  
Writes a string slice into this writer, returning whether the write succeeded.   
1. 官方source中这个trait只是一个定义  
```rust 
#[stable(feature = "rust1", since = "1.0.0")]
fn write_str(&mut self, s: &str) -> Result;
```
根本没有具体的实现代码,这么做:  
1. core 的设计目标是：不假设任何操作系统环境;
* core 只负责制定标准（定义 Trait），它把具体的工作（实现Trait）全部留给了下游

那么谁来具体实现?  
1. std,官方会实现对应的trait
2. 第三方库,会实现它，让你在没有堆内存的情况下也能拼凑字符串.比如正在写的puerto这个项目

**dinvk是怎么实现的core::fmt::Write trait**  
这里为啥不直接堆ConsoleWrite实现core::fmt::Write trait,而是定义了一个新的Write trait?  
这里并没有定义新的 Write trait,从`use core::{fmt::{self, Write}, ptr}`和 `impl Write for ConsoleWriter { ... }` 这里可以看出实现的就是std中的`core::fmt::Write`  

1. 语法解析：这是“导入”而不是“定义”
* use core::fmt::Write; 这一行已经从核心库里把官方的 Write trait导入（Import）到了当前作用域
* 接下来的 impl Write for ConsoleWriter 就是在为 ConsoleWriter实现官方的那个协议
* 如果其形式是 ` trait MyWrite { ... } impl MyWrite for MyStruct { ... }; ` 这才是定义了新的trait

