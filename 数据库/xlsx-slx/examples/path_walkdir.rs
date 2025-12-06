use std::{
    io::{self, Write},
    path::Path,
};

fn main() {
    println!("输入一个目录,按回车键结束输入");
    io::stdout().flush().expect("输出缓冲区没有刷新");
    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("未能读取输入");

    let path_str = input.trim();
    let dir_path = Path::new(path_str);
   
}
