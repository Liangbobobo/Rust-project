## 关于`#[derive(debug)]`的编译器展开真相

想过保留debug 但是使用 1 2 3这种数字表示错误,而且我自己维护密码本.这样release下的错误,我也能理解了.
```rust
#[derive(Debug)]
#[derive(PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum CryptoError {
    Base64DecodeError=1,
    PayloadTooShort,
    InvalidKeyLength,
    HmacVerificationFailed,
    DecryptionFailed,
    Utf8Error,
}
```

这里有一个关于 Rust 编译器特性的重大误区需要澄清：

`#[derive(Debug)]` 打印的绝对不是 1, 2, 3！它打印的是 `"Base64DecodeError"` 这一长串英文！

──────

一、核心误区揭秘：`#[derive(Debug)]` 到底在干什么？

当你在 Rust 代码中写下：

```rust
#[derive(Debug)]
pub enum CryptoError {
    Base64DecodeError = 1,
    PayloadTooShort = 2,
    ...
}
```

Rust 编译器在后台偷偷为你生成了一大坨格式化代码，相当于：

```rust
// 编译器自动生成的代码：
impl core::fmt::Debug for CryptoError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            // 看到这些字符串了吗？！编译器硬生生把它们塞进了你的 .exe 里！
            CryptoError::Base64DecodeError => f.write_str("Base64DecodeError"),
            CryptoError::PayloadTooShort => f.write_str("PayloadTooShort"),
            CryptoError::InvalidKeyLength => f.write_str("InvalidKeyLength"),
            ...
        }
    }
}
```

- 即使你写了 `= 1`, `= 2`，`#[derive(Debug)]` 也根本不管这些数字！
- 只要你调用了 `println!("{:?}", err)`，控制台输出的是：
  `Base64DecodeError`
- 后果：这些长串英文字符串会被强制编译进 PE 文件的 `.rdata` 节区。EDR 提取静态字符串时一抓一个准。

──────

二、关键真相：拿到数字 1, 2, 3，根本不需要 Debug！

你想要达到的效果是：用你自己的密码本对照 1, 2, 3 来排查生产错误。

这个功能，其实和 Debug 半毛钱关系都没有！
因为我们在枚举上加了 `#[repr(u8)]` 和 `code()`：

```rust
let err = CryptoError::Base64DecodeError;

// 任何时候，无论 Release 还是 Debug，无需任何 trait 支持：
let num: u8 = err as u8;   // 直接拿到数字 1！
let num: u8 = err.code(); // 直接拿到数字 1！
```

- 这里的code方法在wisconsin/crypto.rs中也是多余的
- 底层原理：这是 CPU 寄存器级别的整数读取，汇编只有一条指令 `mov al, 1`。
- 安全性：二进制中 0 个英文字符串残留。

──────

三、如果在 Release 下你真的想用 `{:?}`，而且希望它只打印 1, 2, 3 怎么办？

如果你写习惯了 `println!("{:?}", err)`，想在 Release 下保留 `{:?}`，但坚决不想要英文字符串泄露：

那就不要用 `#[derive(Debug)]`，而是手动为它实现一个“只打印数字”的 Debug！

```rust
#[repr(u8)]
#[derive(PartialEq, Eq, Clone, Copy)] // 注意：去掉了 derive(Debug)
pub enum CryptoError {
    Base64DecodeError = 1,
    PayloadTooShort = 2,
    InvalidKeyLength = 3,
    HmacVerificationFailed = 4,
    DecryptionFailed = 5,
    BufferTooSmall = 6,
}

// 手动实现 Debug：告诉编译器，用 {:?} 打印时，只输出它背后的纯数字！
impl core::fmt::Debug for CryptoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // 永远只打印 1, 2, 3，绝不生成任何英文字符串
        write!(f, "{}", *self as u8)
    }
}
```

这样改造之后的实战效果：

1. 测试与开发阶段：
   你可以随意写 `assert_eq!`、`.unwrap()`、`println!("{:?}", err)`，代码完全不报错。

2. 打印输出效果：

```rust
let err = CryptoError::PayloadTooShort;
println!("Error occurred: {:?}", err);
```

- 控制台显示：`Error occurred: 2`
- 你掏出你的小本本一看：“2 代表包太短了，网络被截断了”。

3. PE 文件反编译（IDA / strings）：
   二进制文件里干干净净，一个 `Base64DecodeError` 单词都搜不到！

──────

四、总结决策

- 如果用 `#[derive(Debug)]`：编译后会泄露明文字符串 `"Base64DecodeError"`。
- 如果手动 `impl Debug` 输出数字（如上代码）：既能用 `{:?}`，又输出了 1, 2, 3，同时彻底封杀了明文字符串泄露！

如果你想用 1, 2, 3 密码本模式且保留 `{:?}` 语法，把 crypto.rs 里的 Debug 改成`#[cfg_attr(any(debug_assertions, test), derive(Debug))]` 是最完美的解法！