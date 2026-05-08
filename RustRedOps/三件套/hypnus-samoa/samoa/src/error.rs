

// 为啥_print可以加#[cfg(debug_assertions)],而macro_rules! println不需要呢
// 要使用core::fmt::Write这个trait接收并处理格式化后的字符串,需要附着在具体的类型上.而实际上并不需要保存接收的数据,而是转交给win的OutputDebugStringA api,所以定义了这个单元结构体

use core::fmt::{self,Write,Result};
struct ConsoleWriter;

#[cfg(debug_assertions)]
impl Write for ConsoleWriter {
    // trait Write Required Method
   fn write_str(&mut self,s:&str)->
    Result{
        // 见注释1
        unsafe extern "system" {
            fn OutputDebugStringA(lpOutputString: *const u8);
    }
   // 申请栈空间
   let mut buf=[0u8;1024];
   // 将传入的slice转为bytes.见注释2
   let bytes=s.as_bytes();
    // 截取最大1023个u8类型数据,最后一个在后面被赋0,模拟c风格字符串
    let len=bytes.len().min(1023);
    // 这里并不是转的类型,而是分别截取buf和bytes同样长度的数据.防止后续出现buffer overflow
    // rust中,对一个切片引用&[T]使用[..],切出来的不再是一个引用,而是数据本身.即类型变成了[u8]没有了&
    buf[..len].copy_from_slice(&bytes[..len]);
    // 模拟c字符串以\0结尾,后续传给winapi调用
    buf[len]=0;

    unsafe { OutputDebugStringA(buf.as_ptr());}

    Ok(())
  
}
}




// #[macro_export]
// macro_rules! println {
//     ($args:tt) => {
//         $crate::error::_print(format_args!($($args)*));
//     };
// }





// 注释1:这是ffi的声明.rustc通过静态链接(cargo build时)时,遇到这个声明,会通过linker在win基础库kernel32.lib中找到对应名字的函数.(这会在iat留下痕迹),因此外部使用了#[cfg(debug_assertions)]
// 注释2:物理布局上,slice的类型&str(实质是对[T]数组的不可变引用)在这里和&[u8]没有区别.实质都是rust的胖指针,在栈上都占用16字节(win64下),包含一个指向真实数据的8字节指针(*const u8)(win64下指针和寄存器都是8字节64位的)和8字节的长度信息.转换的原因是copy_from_slice的源码签名,其参数是&[T]且T:Copy.如果传&s[..len](类型是&str),在rust中str和[u8]是不同的primitive type,虽然此处语境下在内存中是一样的,但只要类型的名字不同,rustc依然不会隐式转换,依然认为是不同类型
// 注释2:在c中,只要两个结构体的内存布局一致,或者都是指针类型.就可以用cast相互赋值.rust中即使str和[u8]底层完全一致.但rustc其逻辑语义不同,因为str承诺绝对合法的utf-8,[u8]没有这层承诺.所以被认为是两个不同类型
// 注释2:即使物理布局完全一致,rustc仍认为这是两种类型.因为rustc保证str底层每个字节都是合法且纯正的utf-8编码,从而可以实现一系列安全的操作unicode的方法.而[T]不提供任何语义上的保证(可以是图片数据/可以是机器码/乱码等等)