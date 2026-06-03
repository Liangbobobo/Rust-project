#![allow(unused)]
use alloc::vec::Vec;
use core::ffi::c_void;
use crate::config::Config;
use crate::{cfg, debug_log, stealth_bail};
use crate::{error::{Result,HypnusError}, spoof::Unwind};
use puerto::{helper::PE};
use spin::mutex;
// 本模块在项目中作用:主线程进入休眠时,要构建一条ROP执行链(修改内存属性->加密->延时->解密).为了让执行流能在os dll中合法的反复横跳,不能直接使用call敏感api.
// 而是在合法的os dll(如 ntdll.dll/kernerlbase.dll)的.text/.pdata节中找到如jmp r10/jmp r11这些间接跳转的指令碎片(gadget)
// 本文件的作用就是去os 中搜寻/匹配然后提供这些碎片的地址



/// represent win64 general-purpose register suitable for indirect jumps 间接跳转的通用寄存器
///
/// 排除了fastcall的rcx rdx r8 r9及存放函数返回值的rax.rax的用途在64位os中是固定的(win和linux都适用):任何函数执行完毕,它的整数/指针返回值,必须且只能放在rax中交给调用者
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Reg {
    Rdi,
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
}

/// list of short jump opcode patterns mapped to their corresponding register
/// 
/// &[(&[u8], Reg)]对数组的引用,该数组的元素类型是(&[u8], Reg).其中第一个元素类型是&[u8](字节 slice),第二个是enum Reg
const JMP_GADGETS:&[(&[u8],Reg)]=&[
// jmp rdi:跳转到rip存储的地址
    (&[0xFF, 0xE7], Reg::Rdi),
    // jmp r10
    (&[0x41, 0xFF, 0xE2], Reg::R10),
    // jmp r11
    (&[0x41, 0xFF, 0xE3], Reg::R11),
    // 以下均为jmp Reg
    (&[0x41, 0xFF, 0xE4], Reg::R12),
    (&[0x41, 0xFF, 0xE5], Reg::R13),
    (&[0x41, 0xFF, 0xE6], Reg::R14),
    (&[0x41, 0xFF, 0xE7], Reg::R15),
];




/// represent a resolved jump gadget in memory
/// contains the absolute address and the register it jumps through
#[derive(Debug,Clone,Copy)]
pub struct Gadget{
/// absolute virtual address of the gadget
pub addr:u64,

/// the register used in the jump instruction
pub reg:Reg,
}

impl Gadget {
    
    pub fn new(cfg:&Config)->Self {
        /// 可以手动分配栈,代替源码中使用的Vec,达到极致隐蔽.win的默认栈大小一般是1Mb,这里只有 大小,几乎不会出现栈溢出
        let mut gadgets:Vec<Gadget>=Vec::new();

        /// 通过Config.rs/Config获取要查找gadget的dll的基址
        /// as *const u8以1字节为单位读取该指针指向的数据.源地址的指针仍然是u64大小的(win64下指针和地址永远64位)
let modules = [
cfg.modules.ntdll.as_ptr() as *const u8,
cfg.modules.kernel32.as_ptr() as *const u8,
            cfg.modules.kernelbase.as_ptr() as *const u8,
];

/// 遍历三个dll;modules是数组,modules.iter()->引用的迭代器,其每次循环产生的元素类型是& *const u8.如果将&base(base= *const u8)改为base,则base= & *const u8.后续使用base时需要解引用(*base).这称为模式匹配的对消
/// 但在rust 2021之后,数组实现了IntoIterator ,这里可由源码&base in modules.iter().改为base in modules:base的类型就是*const u8
for base in modules{
    
}

        /// 函数返回前,主动将栈上数据擦除
        todo!()
    }


}
/// represent the type of gadget used to spoof control flow transitions
/// 关于enum的初始化问题见rust grammer/enum
#[derive(Debug,Clone,Copy,Default)]
pub enum GadgetKind {
/// call [rbx] gadget
/// 使用#[default]指定Call当作默认值(必须先使用#[derive(Default)])
#[default]
Call,

/// jmp [rbx] gadget
jmp,

}

impl GadgetKind {
    /// scans the specified image base for a supported control-flow gadget
pub fn detect(base:*mut c_void)->Result<Self> {
    
    // 抽象一个PE文件,用一个结构体代表PE文件,该结构体只有一个raw pointer.
    let pe = Unwind::new(PE::parse(base));

    /// 解构exception table
    let Some(tables)=pe.entries()else {
        stealth_bail!(
HypnusError::ExceptionTableNotFound,"failed to parse .pdata unwind info"
        )
    };



    todo!()
}




}