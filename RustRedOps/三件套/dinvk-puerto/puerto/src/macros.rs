    /// #[macro_export]会将宏提升到crate根
    /// 写rust宏时,经常会出现ide提示失效
    /// 使用cargo expand检测
    /// 安装：cargo install cargo-expand(cargo expand --lib winapis)  
    /// 或者新建一个tests/macro_test.rs 进行单独测试
    
    
    
    
    use core::fmt::{self, Write};
    
    /// 由于p版更改了部分函数的参数和返回值,这里与d版的宏不同.
    #[macro_export]
    macro_rules! dinvok {
        // expr:表达式指示符;ty:类型指示符(可以是函数的类型,如unsafe extern "system" fn(...) -> ...)
        // $() 是重复匹配模式  , 是分隔符 * 表示零个或多个
        ($module:expr,$function:expr,$ty:ty,$($arg:expr),*) => {
            {
                let address=$crate::module::get_proc_address(Some($module),Some($function),Some($crate::hash::fnv1a_utf16));

                // 这里用unwrap()解构是否合适?
                // 这里的宏内部是否可用debug_log!
                if address.unwrap().is_null(){
                    None
                }else
                {   
                    // pub const unsafe fn transmute<Src, Dst>(src: Src) -> Dst
                    let func=unsafe{
                        // 忽略类型检查,将src按位强转为dst提供的函数指针类型
                        core::mem::transmute::<*mut core::ffi::c_void,$ty>(address.unwrap())
                    };

                    Some(unsafe{func($($arg),*)})
                }
            }
        };
    }

    
    // 源d项目中的console.rs完全移入了macro中
    #[macro_export]
    macro_rules! println {
        // tt=token tree 表示捕获所有输入,原封不动的传递
        ($($arg:tt)*) => {
            // _print中的_表示?
            // format_args! 编译器内置宏,在编译期解析格式化字符串(如 {}),将其与参数绑定,生成一个不可变的core::fmt::Arguments结构体.注意,此过程不涉及任何堆分配
            $crate::macros::_print(format_args!($($arg)*));
        };
    }

    #[macro_export]
    macro_rules! eprintln {
        ($($arg:tt)*) => {
            $crate::macros::_print(format_args!($($arg)*));
        };
    }

    pub fn _print(args: fmt::Arguments) {
        let mut writer = ConsoleWriter;
        // 模式匹配中的通配符绑定.明确忽略write_fmt返回的result,如果底层输出失败,程序不崩溃,而是静默失败.let不是不可辩驳的模式吗?怎么会失败?
        let _ = writer.write_fmt(args);
    }

    // 单元结构体(ZST类型),不占用内存
    struct ConsoleWriter;
    
        impl Write for ConsoleWriter {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                             unsafe extern "system" {
                                fn OutputDebugStringA(lpOutputString: *const u8);
                            }    
                // 使用栈上的固定大小缓冲区，避免依赖 global allocator
                let mut buf = [0u8; 1024];
                let bytes = s.as_bytes();
                let len = bytes.len().min(1023);
                buf[..len].copy_from_slice(&bytes[..len]);
                buf[len] = 0; // null-terminate
    
                unsafe {
                    OutputDebugStringA(buf.as_ptr());
                }
                Ok(())
            }
        }
     /// 仅在 Debug 模式下向 Windows 控制台打印调试信息(通过#[cfg(debug_assertions)]);在 Release 模式下，该宏的内容会被编译器忽略，不占用空间。
    /// 
     #[macro_export]
     macro_rules! debug_log {
         ($($arg:tt)*) => {
             #[cfg(debug_assertions)]
            {   
                $crate::println!($($arg)*);
             }
        };
    }