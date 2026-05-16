#![no_std]
#![allow(
    clippy::missing_transmute_annotations, 
    clippy::useless_transmute,
    clippy::collapsible_if,
    non_snake_case, // 变量/字段名警告
    non_camel_case_types, // 类型名警告
    non_upper_case_globals // 全局变量/常量名警告
)]

pub mod error;
pub mod types;
pub mod allocator;
pub mod winapis;


