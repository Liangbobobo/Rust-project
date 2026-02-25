
// lib是整个crate的入口,以下声明会递归到所有子模块中(mod hash等)
#![no_std]
#![allow(non_snake_case, non_camel_case_types)]// 保留win的原始命名
/// 引入alloc库
/// 
/// 告诉rust编译器你的项目需要链接（Link）并使用一个外部的库（crate）
/// 
/// 该alloc随着rustc一起安装在本地,所以不能在toml文件中引入使用,必须使用这种方式
extern crate alloc;

pub mod error;
pub mod hash;
pub mod module;
pub mod types;
pub mod winapis;
pub mod helper;
pub mod macros;
pub mod syscall;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub mod breakpoint;