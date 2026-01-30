     use core::fmt::{self, Write};

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