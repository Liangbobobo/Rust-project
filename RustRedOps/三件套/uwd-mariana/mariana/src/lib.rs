#![no_std]
#![allow(
    clippy::missing_transmute_annotations, 
    clippy::useless_transmute,
    clippy::collapsible_if,
    non_snake_case, // 变量/字段名警告
    non_camel_case_types, // 类型名警告
    non_upper_case_globals // 全局变量/常量名警告
)]

// 挂载并声明子模块:让编译器从项目里找uwd.rs或uwd.rs/mod.rs文件,将其编译进来,并分配一个叫uwd的命名空间
pub mod uwd;


// 编译器把uwd模块中所有pub函数/类型,直接复制/映射到当前层级.后续不需要uwd::的前缀就可以直接使用其函数和类型
pub use uwd::*;
pub mod types;
pub mod util;
pub mod error;

extern crate alloc;