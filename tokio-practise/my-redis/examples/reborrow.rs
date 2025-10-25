#[allow(unused)]
fn main() { 
let mut x  = "hello";
let  a=&mut x;
//通过类型转换，equal to let  b= &mut x;  这里发生了reborrow
let  b= &mut *a;

*b= "rust";
// 改变顺序将*a放在*b之上，会改变这两个变量的lifetime，会报错。
*a= "world";
}

//这里牵扯到reference中type coercions章节引发的reborrow的概念（未经官方确认）
//https://stackoverflow.com/questions/65474162/reborrowing-of-mutable-reference