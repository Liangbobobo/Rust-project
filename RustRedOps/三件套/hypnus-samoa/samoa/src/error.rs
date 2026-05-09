// 背景知识:详见Rust Grammar and winx64 abi中fmt,有很多尚未理解
// ConsoleWriter->_print->debug_log!

// 单元测试中有测试命令,这里是检查指定宏展开情况(需要cargo expand);--lib指定入口
// cargo expand --lib --tests error::tests::test_console_writer


use core::fmt::{self, Write};

// 自定义栈/调用OutputDebugStringA;s是自定义的传入的字符串
// 要使用core::fmt::Write这个trait接收并处理格式化后的字符串,需要附着在具体的类型上.而实际上并不需要保存接收的数据,而是转交给win的OutputDebugStringA api,所以定义了这个单元结构体
#[cfg(debug_assertions)]
struct ConsoleWriter;

#[cfg(debug_assertions)]
impl Write for ConsoleWriter {
    // trait Write Required Method
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // 见注释1
        unsafe extern "system" {
            fn OutputDebugStringA(lpOutputString: *const u8);
        }
        // 申请栈空间
        let mut buf = [0u8; 1024];
        // 将传入的slice转为bytes.见注释2
        let bytes = s.as_bytes();
        // 截取最大1023个u8类型数据,最后一个在后面被赋0,模拟c风格字符串
        let len = bytes.len().min(1023);
        // 这里并不是转的类型,而是分别截取buf和bytes同样长度的数据.防止后续出现buffer overflow
        // rust中,对一个切片引用&[T]使用[..],切出来的不再是一个引用,而是数据本身.即类型变成了[u8]没有了&
        buf[..len].copy_from_slice(&bytes[..len]);
        // 模拟c字符串以\0结尾,后续传给winapiOutputDebugStringA调用
        buf[len] = 0;

        unsafe {
            OutputDebugStringA(buf.as_ptr());
        }

        Ok(())
    }
}

// 调用自定义的ConsoleWrite
#[cfg(debug_assertions)]
pub fn _print(args: fmt::Arguments) {
    let mut writer = ConsoleWriter;

    let _ = writer.write_fmt(args);
}

// puerto中多写的这段完全没有必要
// #[macro_export]
// macro_rules! println {
//     ($args:tt) => {
//         #[cfg(debug_assertions)]
//         $crate::error::_print(format_args!($($args)*));
//     };
// }

// 输出错误日志
#[macro_export]
macro_rules! debug_log {
    ($($args:tt)*) => {
        #[cfg(debug_assertions)]
        // 即使只有一句也应放在{}中
        {
            $crate::error::_print(format_args!($($args)*));
        }

    };
}

// 实现hypnus中bail!的control flow功能
use crate::types::NTSTATUS;
pub type Result<T> = core::result::Result<T, HypnusError>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(C)]
pub enum HypnusError {

    // 为什么没有初始化?没有初始化的物理实质是什么(见注释3)
    // 框架错误
    InvalidArguments,
    ApiNotFound,
    GadgetNotFound,
    ModuleNotFound,


    // os错误
    OsError(NTSTATUS),
}

// HypnusError中的字段是一个expr类型?
// 解答: 是的。在宏参数 `$err:expr` 中，`expr` 代表 "Expression" (表达式)。
// 当您传入 `HypnusError::InvalidArguments` 或 `HypnusError::OsError(0)` 时，
// 它们在 Rust 语法树中都是合法的表达式（分别对应路径表达式和调用表达式），
// 所以用 `expr` 来捕获它们是完全正确的。

#[macro_export]
macro_rules! stealth_bail {
    // 修复点: 改为 $($args:tt)* 以接收多个参数 (字符串模板 + 多个变量)
    ($err:expr, $($args:tt)*) => {
        {   
            // 不能直接将args给debug_log!:底层的 format_args! 严格要求第一个参数必须是带有 {}的固定模板，直接硬塞会导致用户的模板和变量被拆散，从而找不到对应的坑位而触发编译报错
            $crate::debug_log!("[!] Error: {}", core::format_args!($($args)*));

            return core::result::Result::Err($err);
        }
    };
    
    // 重载模式：如果调用者只想退出，不想打印日志
    ($err:expr) => {
        {
            return core::result::Result::Err($err);
        }
    };
}
















// debug_log!单元测试
// cargo test test_console_writer --release
// 编译成功但在debugview中没有任何显示,说明达到了release下不显示debug信息的目的(可以去掉上述命令中--release作为对比)
#[cfg(test)]
mod tests {
    // 引入当前 crate 根路径下所有宏和 public 模块
    use super::*;

    #[test]
    fn test_console_writer() {
        let target_name = "LSASS.exe";
        let pid = 1024;
        
        // 调用我们刚才定义的宏！
        // 如果宏正常工作，编译不会报错。在 Windows 下借助 DebugView 可以看到输出。
        debug_log!("[+] Targeting {} with PID: {}", target_name, pid);
    }

    // 模拟一个 hypnus 里的业务函数来测试 bail
    fn mock_inject_payload(args_count: usize) -> Result<()> {
        if args_count > 11 {
            stealth_bail!(HypnusError::InvalidArguments, "Too many arguments provided: {}", args_count);
        }
        
        debug_log!("[+] Payload injected successfully!");
        Ok(())
    }

    // 单元测试stealth_bail!
    // cargo expand --lib --tests error::tests::test_stealth_bail
    #[test]
    fn test_stealth_bail() {
        // 测试一：参数数量正确，应该成功
        let res1 = mock_inject_payload(5);
        assert!(res1.is_ok());

        // 测试二：参数超标，应该触发 stealth_bail! 提前报错
        let res2 = mock_inject_payload(12);
        
        match res2 {
            Err(HypnusError::InvalidArguments) => {
                debug_log!("Successfully caught the expected framework error!");
            },
            _ => panic!("Test failed! Did not bail correctly."),
        }
    }
}




// 注释1:这是ffi的声明.rustc通过静态链接(cargo build时)时,遇到这个声明,会通过linker在win基础库kernel32.lib中找到对应名字的函数.(这会在iat留下痕迹),因此外部使用了#[cfg(debug_assertions)]
// 注释2:物理布局上,slice的类型&str(实质是对[T]数组的不可变引用)在这里和&[u8]没有区别.实质都是rust的胖指针,在栈上都占用16字节(win64下),包含一个指向真实数据的8字节指针(*const u8)(win64下指针和寄存器都是8字节64位的)和8字节的长度信息.转换的原因是copy_from_slice的源码签名,其参数是&[T]且T:Copy.如果传&s[..len](类型是&str),在rust中str和[u8]是不同的primitive type,虽然此处语境下在内存中是一样的,但只要类型的名字不同,rustc依然不会隐式转换,依然认为是不同类型
// 注释2:在c中,只要两个结构体的内存布局一致,或者都是指针类型.就可以用cast相互赋值.rust中即使str和[u8]底层完全一致.但rustc其逻辑语义不同,因为str承诺绝对合法的utf-8,[u8]没有这层承诺.所以被认为是两个不同类型
// 注释2:即使物理布局完全一致,rustc仍认为这是两种类型.因为rustc保证str底层每个字节都是合法且纯正的utf-8编码,从而可以实现一系列安全的操作unicode的方法.而[T]不提供任何语义上的保证(可以是图片数据/可以是机器码/乱码等等)

// 注释3:Rust 中，HypnusError 中的InvalidArguments 这种不带任何数据的枚举变体，叫做单元变体（Unit Variant）.它们本身不携带任何附加信息不像 OsError(NTSTATUS)携带一个i32.所以不需要初始化.因为加了#[repr(C)]其物理实质变成一个 C 语言风格的Tagged  Union（标签联合体）
// rust不允许使用未经初始化的变量.但这里只是声明/定义阶段,后续在使用时必须初始化.对于InvalidArguments这种不携带数据(单元变体)的变量在使用时写出全名就是一次完整的初始化,如stealth_bail!(HypnusError::InvalidArguments)
// let my_error = HypnusError::OsError(0xC0000005); return Err(my_error);