/// Macro to dynamically invoke a function from a specified module.
/// 
/// # Example
/// 
/// ```
/// let ntdll = get_ntdll_address();
/// let result = dinvoke!(ntdll, "NtQueryInformationProcess", extern "system" fn(...) -> u32, ...);
/// ``` 
#[macro_export]// 标记为导出宏,可以在其他模块(文件)甚至crate外部被使用
macro_rules! dinvoke {
    
    // 宏的工作原理是匹配模式,左边是类型,右边是展开的形式
    // ()表示宏参数列表的开始和结束
    // 宏定义中,变量必须以$开头,module自定义的在宏中定义的变量名
    // :expr,类型指示符,即$module应当是什么样的代码片段
    // $module:expr,代表宏的第一个参数module必须是一个表达式,如变量名/字符串/函数调用等,可以计算出一个值的类型
    // $ty 变量名
    // :ty 类型指示符,表示$ty这个变量,应该是一个rust的类型(Type)
    //  $ty:ty,这里表示传入的必须是一个具体的类型定义,如extern "system" fn(i32) -> i32  这样的函数指针类型 (Function Pointer Type),extern "system"代表符合操作系统api的调用方式,fn代表一个函数指针
    // $( ... ) 表示这括号里面的内容可以重复匹配
    // $arg:expr 每次重复匹配都会将一个expr给$arg
    // , 代表每次重复匹配后后面必须跟一个, 才能继续匹配下一个
    // * 表示前面的模式 "($arg:expr)," 可以出现0次或多次,即不管有多少模式匹配都会存入$arg中
    // 如 dinvoke!(..., 10, 20, 30) 那么$arg 列表为[10, 20, 30]
    ($module:expr, $function:expr, $ty:ty, $($arg:expr),*) => 
    // 双层大括号,外层是宏语法的界定符,内层是生成的代码块,这样宏展开后是一个独立的语句块,有自己的作用域,避免变量污染外部
    {{
        // Get the address of the function in the specified module
        // $crate 总是代表定义这个宏的crate的根路径 .当$crate展开时,替换成“绝对路径的库名” .在dinvk库内展开为crate::module::get_proc_address;库外使用$crate时展开为dinvk::module::get_proc_address
        let address = $crate::module::get_proc_address($module, $function, None);
        if address.is_null() {
            None
        } else {
            // Transmute the function pointer to the desired type and invoke it with the provided arguments
            // 这里address是通过 get_proc_address得到的*mut c_void类型的指针,指向内存中的某个位置,且必须转换后才能使用
            // $ty代表一个具体的类型,这里是一个函数指针
            // core::mem::transmute是rust中极为强大和危险的"位转换",放弃编译器的类型检查,直接把第一个泛型参数当作第二泛型参数的类型来使用
            // 这一段把一个地址变为一个函数,从而实现了动态调用
            let func = unsafe { core::mem::transmute::<*mut core::ffi::c_void, $ty>(address) };
            Some(unsafe { func($($arg),*) })
        }
    }};
}

/// Macro to perform a system call (syscall) by dynamically resolving its function name.
///
/// # Example
///
/// ```
/// let mut addr = null_mut::<c_void>();
/// let mut size = (1 << 12) as usize;
/// let status = syscall!("NtAllocateVirtualMemory", -1isize as HANDLE, &mut addr, 0, &mut size, 0x3000, 0x04)
///    .ok_or("syscall resolution failed")?;
///
/// if !NT_SUCCESS(status) {
///     eprintln!("[-] NtAllocateVirtualMemory Failed With Status: {}", status);
/// }
/// ```
#[macro_export]
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
macro_rules! syscall {
    ($function_name:expr, $($y:expr), +) => {{
        // Retrieve the address of ntdll.dll
        let ntdll = $crate::module::get_ntdll_address();

        // Get the address of the specified function in ntdll.dll
        let addr = $crate::module::get_proc_address(ntdll, $function_name, None);
        if addr.is_null() {
            None
        } else {
            // Retrieve the SSN for the target function
            match $crate::ssn($function_name, ntdll) {
                None => None,
                Some(ssn) => {
                    // Calculate the syscall address
                    match $crate::get_syscall_address(addr) {
                        None => None,
                        Some(syscall_addr) => {
                            // Count number of args
                            let cnt = 0u32 $(+ { let _ = &$y; 1u32 })+;
                            
                            // Execute syscall
                            Some(unsafe { $crate::asm::do_syscall(ssn, syscall_addr, cnt, $($y),+) })
                        }
                    }
                }
            }
        }
    }};
}

/// Prints output to the Windows console.
#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        
        let mut console = $crate::console::ConsoleWriter;
        let _ = writeln!(console, $($arg)*);
    }};
}