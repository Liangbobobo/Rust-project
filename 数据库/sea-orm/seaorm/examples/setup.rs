use std::path::Path;

use calamine::{self, Reader, Xlsx, open_workbook};

fn main() {
    let path_str = r"C:\Users\liang\Desktop\1.xlsx";
    let path = Path::new(path_str);
    let mut workbook: Xlsx<_> = match open_workbook(path) {
        Ok(workbook) => workbook,
        Err(e) => {
            eprintln!("错误：无法打开文件 '{:?}'. 原因: {}", path, e);
            std::process::exit(1)
        }
    };

    //这里为什么要使用to_vec？
    let sheet_name = workbook.sheet_names().to_vec();
    //下一步完善将sheet_name放入变量，然后获取range进一步操作
    println!("{:?}", sheet_name);
    //缺少错误控制
    let rang = workbook.worksheet_range("sheet_1").expect("无法打开表");
    for (_row_index, row) in rang.rows().enumerate().take(2) {
        for (_col_index, cell) in row.iter().enumerate() {
            // 不需要match data，println!("{:?}",cell);即可显示所有cell的格式
            println!("{:?}", cell);
            //  println!("数据行的格式");
            //          match cell {

            //     Data::Int(v) => {
            //         println!("这是整数类型，值在 v 中: {}", v);
            //     }
            //     Data::Float(v) => {
            //         println!("这是浮点数类型，值在 v 中: {}", v);
            //     }
            //     Data::String(s) => {
            //         println!("这是字符串类型，值在 s 中: {}", s);
            //     }
            //     Data::DateTime(_) => {
            //         println!("这是日期时间类型");
            //     }
            //     Data::Empty => {
            //         println!("这是空单元格");
            //     }
            //     // ... 匹配其他类型
            //     _ => {
            //         println!("其他情况");
            //     }
            // }
        }
    }
}
