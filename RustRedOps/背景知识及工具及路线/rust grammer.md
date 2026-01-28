# Rust Grammer

## 内联宏

`#[inline]`是一个编译器指令，建议编译器将该函数的代码直接“嵌入”到调用它的地方，而不是通过函数调用（Jump/Call）指令跳转执行

对于函数的操作极其简单（比如,只是把一个指针包进另一个结构体），函数调用的开销（压栈、跳转、出栈）甚至比函数体本身的逻辑还要重  

这样实现了零成本抽象（Zero-cost Abstraction）。在生成的二进制机器码中，这个函数调用通常会彻底消失，效率等同于你手动初始化结构体。

## let-else

称为“可反驳模式绑定”（Refutable Pattern Binding）  
不可反驳模式 (Irrefutable)：一定会匹配成功的模式（如普通的 let x =5;）

```rust
let <模式> = <表达式> else {
        // 当模式匹配失败时执行的代码块
       // 必须包含发散逻辑（return, break, continue, panic! 等）
    };
    // 如果匹配成功，模式中的变量在这里开始生效
```

只要 `=` 左边的“模板”没法套在右边的“实际数据”上，程序就会跳进`else` 块

一旦进入 else，你必须用 return、break 或 continue离开当前逻辑，因为 Rust 编译器需要保证：如果代码跑到了 `let-else`的下一行，左边的变量（如 `nt_header`）必须是已经成功拿到值的

它结合了 if let 的简洁模式匹配和 match的非嵌套流程控制。它专注于“快速失败”（Early Return）逻辑

### 重要特性

(1) 变量作用域（最重要）
  与 if let 不同，let-else绑定成功的变量，其作用域是在当前代码块的剩余部分，而不是在一个局部的大括号
  内。

* `if let`：变量只在 { ... } 块内有效。
* `let-else`：变量在 let-else 语句之后一直有效

(2) 必须“发散”（Diverging）

  else块中的代码绝对不能让程序流程正常“流出”到下一行。它必须通过以下方式之一结束：

* return（返回函数）
* break / continue（在循环中）
* panic!（终止程序）
* 调用返回值为 !（Never 类型）的函数

  (3) 模式匹配的强大
  它不仅支持 Some/None，还支持任何模式：

    ```rust
    // 解构元组并带条件检查
    let (Some(nt), Some(exports)) = (get_nt(), get_exports()) else {
        return;
    };
    ```

## Result<>

## derive[debug]
