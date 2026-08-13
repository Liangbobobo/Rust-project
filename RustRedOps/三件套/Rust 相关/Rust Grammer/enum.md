

## enum定义与初始化

```rust
#[derive(Debug,Clone,Copy,Default)]
pub enum GadgetKind {
/// call [rbx] gadget
#[default]
Call,

/// jmp [rbx] gadget
jmp,

}
```

上述是定义,此时的Call/Jmp在物理内存中是不存在的.它们是不带数据的单元变体,可以直接使用如`let my_gadget = GadgetKind::Call;`,在后续代码中写出全名,编译器就认为已经初始化完毕.

但对于
```rust
pub enum HypnusError {
        ApiNotFound,     // 不带数据的“单元变体 (Unit Variant)”
        OsError(i32),    // 带数据的“元组变体 (Tuple Variant)”
    }
```
中OsError这种带数据的变体,直接用HypnusError::OsError,编译器会报错