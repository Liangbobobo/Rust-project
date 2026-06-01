use core::ffi::c_void;

use crate::error::Result;

use puerto::{helper::PE};
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


/// represent a resolved jump gadget in memory
/// contains the absolute address and the register it jumps through
#[derive(Debug,Clone,Copy)]
pub struct Gadget{
/// absolute virtual address of the gadget
pub addr:u64,

/// the register used in the jump instruction
pub reg:Reg,
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
    
}




}