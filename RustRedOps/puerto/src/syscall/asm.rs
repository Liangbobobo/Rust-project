// Reference: <https://github.com/janoglezcampos/rust_syscalls>
//该项目中用于执行间接系统调用 (Indirect Syscalls) 的核心汇编实现
#[cfg(target_arch = "x86_64")]
// 宏,允许在rust文件的全局作用域内直接编写原始汇编代码
// 将双引号内的字符串直接交给汇编器,与asm!宏不同,global_asm!是一个完整\独立的函数或符号,可以像普通c函数一样被链接和调用
core::arch::global_asm!("

; 汇编指令(Directive),用于声明全局符号,其他rust模块可以使用
.global do_syscall

; 指定接下来的代码应该存放在二进制文件的哪个“分分区”里
; 这是二进制文件中存放可执行机器指令的标准段名称
.section .text

; : 表示这是一个标签,标记了do_syscall函数在内存中的起始地址
do_syscall:

    ; 寄存器备份(rsi rdi r15为非易失性寄存器)
    ; rsi rdi不能换,因为后面使用了rep movsq指令,这个指令固定使用rsi作为源地址,使用rdi作为目标地址.如果更换就不能再使用rep movsq而必须手写loop循环或mov指令
    ; r12 为了免杀性,必须更换(不能用r11)
    mov [rsp - 0x8],  rsi
    mov [rsp - 0x10], rdi
    ; mov [rsp - 0x18], r12
    mov [rsp - 0x18], r15

    ; ssn与跳转地址准备(进行系统调用需要的前三个参数)
    mov eax, ecx
    ; 在ntdll中的函数地址,在更改参数之后执行的函数地址
    mov r15, rdx
    ; 即将进行系统调用的参数总数
    mov rcx, r8 

    ; 参数重映射(FastCall -> Syscall ABI)
    ; 系统调用(winapi)的第一个参数,放入r10(原来是r9)
    mov r10, r9
    ; rdx前后发生了变化,所以需要保存,具体原因在asm.md中
    mov rdx,  [rsp + 0x28]
    mov r8,   [rsp + 0x30]
    mov r9,   [rsp + 0x38]

    ; 动态栈参数拷贝 (处理 5 个及以上参数)
    ; rcx代表总参数,如果小于4,直接跳过
    sub rcx, 0x4
    jle skip


    ; lea(Load Effective Address)：加载地址，不读取内存 
    lea rsi,  [rsp + 0x40] ;rep movsq操作的源地址
    lea rdi,  [rsp + 0x28] ;rep movsq操作的目标地址

    rep movsq
skip:

    ; 还原寄存器现场并准备跳转
    mov rcx, r15; 将之前保存的rdx地址(mov r15,rdx)作为执行地址放入rcx

    mov rsi, [rsp - 0x8]
    mov rdi, [rsp - 0x10]
    mov r15, [rsp - 0x18]

    ; 间接跳转执行 (绕过 EDR 对 ntdll 函数头的 Hook)
    jmp rcx
");

#[cfg(target_arch = "x86")]
core::arch::global_asm!("
.global _do_syscall

.section .text

_do_syscall:
    mov ecx, [esp + 0x0C]
    not ecx
    add ecx, 1
    lea edx, [esp + ecx * 4]

    mov ecx, [esp]
    mov [edx], ecx

    mov [edx - 0x04], esi
    mov [edx - 0x08], edi

    mov eax, [esp + 0x04]
    mov ecx, [esp + 0x0C]

    lea esi, [esp + 0x10]
    lea edi, [edx + 0x04]

    rep movsd

    mov esi, [edx - 0x04]
    mov edi, [edx - 0x08]
    mov ecx, [esp + 0x08]
    
    mov esp, edx

    mov edx, fs:[0xC0]
    test edx, edx
    je native

    mov edx, fs:[0xC0]
    jmp ecx

native:
    mov edx, ecx
    sub edx, 0x05
    push edx
    mov edx, esp
    jmp ecx
    ret

is_wow64:
");

#[doc(hidden)]
#[allow(unused_doc_comments)]
#[cfg(target_arch = "x86_64")]
unsafe extern "C" {
    pub fn do_syscall(
        ssn: u16,
        syscall_addr: u64,
        n_args: u32,
        ...
    ) -> i32;
}

#[doc(hidden)]
#[allow(unused_doc_comments)]
#[cfg(target_arch = "x86")]
unsafe extern "C" {
    pub fn do_syscall(
        ssn: u16,
        syscall_addr: u32,
        n_args: u32,
        ...
    ) -> i32;
}