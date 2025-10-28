use calamine::{self, Reader,Xlsx, open_workbook};
use std::path::Path;
fn main(){

//使用use std::path::Path;更加合适
let path_str = r"C:\Users\liang\Desktop\1.xlsx";
let path = Path::new(path_str);
// ?,成功会取回OK()中的值，并向左赋值。
//失败，会使用From trait，将calamine::ERROR转为当前函数需要的错误类型，并向上传播直到被一个 match、if let Err
//或另一个 ? 处理，从而避免了层层嵌套的 match 语句，使代码更加线性和易读
let mut workbook:Xlsx<_> =match  open_workbook(path){
Ok(workbook)=>workbook,
Err(e)=>{
    eprintln!("Failed to open workbook: {}", e);
    std::process::exit(1);
} 
};

let mut _range = match workbook.worksheet_range("sheet_1")  {
    Ok(range) => {
        
       
        
            for(col_num,data)in range.rows().enumerate().take(2){
            println!(" Col: {}, Value: {:?}\n",col_num,data);
        }
         
        
    },
    Err(e) => {
        eprintln!("Failed to get worksheet range: {}", e);
        std::process::exit(2);
    }
};


()
}