# thiserror 和 anyhow统御了错误处理的半壁江山

## ThisError

thiserror 通过过程宏 `#[derive(Error)]`，帮你自动生成.Rust 原生中，定义一个错误类型非常麻烦。你需要手动写 impl Display，手动写 impl Debug，手动写 impl Error，还要写 impl `From<IoError>`... 写一个错误要写几十行代码,实现标准的 std::error::Error

它保留了错误的原始类型结构，不丢失任何信息。

## Anyhow

在 main 函数或者顶层逻辑中，你可能会调用几十个库，它们返回几十种不同的错误（IoError, SerdeError, SqlxError...）。你不想为此定义一个巨大的 Enum 来包容万物

anyhow 提供了一个通用的 `anyhow::Result<T>`（本质上是一个超级智能的 `Box<dyn Error>` 指针）。它能把任何实现了 std::error::Error 的错误“一口吞掉”，并变成统一的 anyhow::Error 类型。

它抹除了具体的错误类型，统一视为“出错”了，但它额外附带了堆栈回溯 (Backtrace) 和 上下文 (Context) 功能。

场景：“应用（Application）”作者（比如 main.rs 的最顶层），或者不需要根据错误类型做分支逻辑的场景。

## 区别

它们不是互斥的，而是互补的
