# syscall

这个文件夹是实现间接系统调用 (Indirect Syscall) 的灵魂

在rust中,/src/syscall这种结构是典型的 “文件模块化” 布局(一个文件夹如果在其父模块显示声明,那么这个文件夹就是一个mode)：  

* `syscall` 文件夹：它在代码里表现为一个名为dinvk::syscall 的模块。
* `x86_64` 文件夹：它在代码里表现为一个名为dinvk::syscall::x86_64的子模块。

1. dinvk/puerto是唯一的crate
根目录下有一个 Cargo.toml，这定义了 `dinvk` 是一个Crate（单元包）。整个 src文件夹及其子文件夹里的所有代码，最终都会被编译成这一个 Crate(这也是判断是否是crate的唯一标准)

2. src/syscall 文件夹的组织形式
它采用的是 Rust 的 “目录模块结构”。是dinvk/puerto的一个mode,dinvk/puerto中src/lib.rs将整个项目的mode连接在一起.   
lib.rs里面有一行 pub mod syscall;把src/lib.rs 与 src/syscall/mod.rs 连接了起来

3. src/syscall/x86_64 也是一种文件模块  
这是 Rust 组织代码的固定套路，你可以把它理解为 “层级链接器”：  
1. `src/lib.rs` 声明了 pub mod syscall;。
2. 于是编译器去读 `src/syscall/mod.rs`。
3. `src/syscall/mod.rs` 里声明了 pub mod x86_64;。
4. 于是编译器去读 `src/syscall/x86_64/mod.rs`。

层层嵌套的mod.rs。它们的作用是接力，把分散在不同文件夹里的代码，最终全部“挂”到lib.rs 这个总根上


组织结构(dinvk的模块组织形式):  
``` rust
src/syscall/
├── mod.rs          <-- 模块入口，定义通用接口、宏及架构路由
├── asm.rs          <-- 跨架构的汇编底层实现（do_syscall）
├── aarch64/
│   └── mod.rs      <-- ARM64 架构特定的系统调用逻辑
├── x86/
│   └── mod.rs      <-- 32位 (x86/WoW64) 架构特定的实现
└── x86_64/
    └── mod.rs      <-- 64位 (x64) 核心逻辑：SSN 解析与 Gate 技术
```

与rust 2018+的组织形式对比:

```rust

   1 src/
   2 ├── lib.rs          (写有 pub mod syscall;)
   3 └── syscall/
   4     ├── mod.rs      (写有 pub mod x86_64;)
   5     └── x86_64/
   6         └── mod.rs  (写有具体代码)
   * 缺点：如果你在 IDE 里打开多个模块，标签页上全是
     mod.rs，你根本分不清哪个是哪个。


1 src/
   2 ├── lib.rs          (写有 pub mod syscall;)
   3 ├── syscall.rs      (入口文件，代替 syscall/mod.rs)
   4 └── syscall/        (文件夹，存放子模块)
   5     ├── x86_64.rs   (入口文件，代替 syscall/x86_64/mod.rs)
   6     └── x86_64/     (文件夹，存放 x86_64 的子模块)


  关键区别：
   * 入口位置：模块的声明代码不再写在文件夹内部的 mod.rs里，而是写在与文件夹同级的 .rs 文件里。
   * 映射关系：编译器看到 syscall.rs，就知道这是一个模块；看到同名的syscall/ 文件夹，就知道这个文件夹里的所有内容都是 syscall 的子模块。
```






### 为什么要专门建syscall文件夹存放相关代码?

1. 架构隔离（Multi-Architecture Support）
```rust
syscall/
├── mod.rs      <-- 统一接口
├── x86_64/     <-- 只放 64 位逻辑
└── x86/        <-- 只放 32 位逻辑
```

在 mod.rs 中通过 #[cfg(target_arch = "x86_64")] pub mod x86_64;  
就能优雅地根据用户的 CPU 架构加载对应的代码

2. 隐藏底层黑魔法（Encapsulation）

系统调用的实现涉及大量的内联汇编 (ASM) 和不安全代码 (Unsafe)
* syscall文件夹作为一个独立的模块，可以将这些“危险”且“肮脏”的底层实现（比如asm.rs 里的原始汇编）隐藏起来。
* 对于外部（即 src/lib.rs 或用户代码）来说，他们只需要看到 syscall模块导出的简单接口或宏（如syscall!()），而不需要关心内部是怎么搬运寄存器的。