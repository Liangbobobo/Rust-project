//检查xlsx文件单元格中数据的格式

use calamine::{Data, Reader, Xlsx, open_workbook};

// 辅助函数：根据 Data 枚举变体返回类型名称字符串
fn get_cell_type_name(cell_data: &Data) -> &'static str {
    match cell_data {
        Data::Empty => "Empty",
        Data::String(_) => "String",
        Data::Int(_) => "Int",
        Data::Float(_) => "Float",
        Data::Bool(_) => "Bool",
        Data::DateTime(_) => "DateTime",
        Data::DurationIso(_) => "Duration",
        Data::Error(_) => "Error",
        // 如果您使用的 calamine 版本有其他变体，可以根据需要添加
        _ => "Unknown",
    }
}

fn main() {
    let path = "./sun032-wechat.xlsx";

    let mut workbook: Xlsx<_> = match open_workbook(path) {
        Ok(book) => book,
        Err(e) => {
            eprintln!("Error opening file '{}': {}", path, e);
            return;
        }
    };

    let sheet = match workbook.worksheet_range_at(0) {
        Some(Ok(range)) => range,
        Some(Err(e)) => {
            eprintln!("Error reading first sheet: {}", e);
            return;
        }
        None => {
            eprintln!("No sheets found in file.");
            return;
        }
    };

    let mut rows = sheet.rows();

    println!("--- Analyzing row 1 (Header) ---");
    if let Some(header_row) = rows.next() {
        for (col_idx, cell_data) in header_row.iter().enumerate() {
            // 使用我们定义的辅助函数来获取类型名称
            println!(
                "  Col {}: Value = '{}', Type = {}",
                col_idx + 1,
                cell_data.to_string(),
                get_cell_type_name(cell_data)
            );
        }
    } else {
        println!("Sheet is empty.");
        return;
    }

    println!("\n--- Analyzing row 2 (First Data Row) ---");
    if let Some(first_data_row) = rows.next() {
        for (col_idx, cell_data) in first_data_row.iter().enumerate() {
            // 使用我们定义的辅助函数来获取类型名称
            println!(
                "  Col {}: Value = '{}', Type = {}",
                col_idx + 1,
                cell_data.to_string(),
                get_cell_type_name(cell_data)
            );
        }
    } else {
        println!("Sheet has only one row.");
    }
}
