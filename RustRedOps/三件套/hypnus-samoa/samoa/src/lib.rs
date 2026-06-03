#![no_std]
#![allow(
    clippy::missing_transmute_annotations, 
    clippy::useless_transmute,
    clippy::collapsible_if,
    non_snake_case, // 变量/字段名警告
    non_camel_case_types, // 类型名警告
    non_upper_case_globals // 全局变量/常量名警告
)]

/// std会自动引入alloc;no_std下需要link alloc库
/// 
/// 但需要注册全局分配器(该分配器只需实现一次,已经在allocator.rs中实现),再使用use alloc::vec::Vec引入
extern crate alloc;

pub mod error;
pub mod types;
pub mod allocator;
pub mod winapis;
pub mod hypnus;
pub mod config;
pub mod cfg;
pub mod spoof;
pub mod gadget;


