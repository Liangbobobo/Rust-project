    /// #[macro_export]会将宏提升到crate根
    /// 写rust宏时,经常会出现ide提示失效
    /// 使用cargo expand检测
    /// 安装：cargo install cargo-expand(cargo expand --lib winapis)  
    /// 或者新建一个tests/macro_test.rs 进行单独测试
    
    
    
    
    use core::fmt::{self, Write};
    
    /// 由于p版更改了部分函数的参数和返回值,这里与d版的宏不同.
    #[macro_export]
    macro_rules! dinvok {
        ($module:expr,$function:expr,$ty:ty,$($arg:expr),*) => {
            {
                let address=$crate::module::get_proc_address(Some($module),Some($function),Some($crate::hash::fnv1a_utf16))

                if address.unwrap().is_null(){
                    None
                }else
                {   
                    // pub const unsafe fn transmute<Src, Dst>(src: Src) -> Dst
                    let func=unsafe{
                        core::mem::transmute::<*mut core::ffi::c_void,$ty>(address.unwrap())
                    };

                    Some(unsafe{func($($arg),*)})
                }
            }
        };
    }


    #[macro_export]
    macro_rules! println {
        ($($arg:tt)*) => {
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
        let _ = writer.write_fmt(args);
    }

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
     /// 仅在 Debug 模式下向 Windows 控制台打印调试信息。
    /// 在 Release 模式下，该宏的内容会被编译器忽略，不占用空间。
     #[macro_export]
     macro_rules! debug_log {
         ($($arg:tt)*) => {
             #[cfg(debug_assertions)]
            {   
                 $crate::println!($($arg)*);
             }
        };
    }