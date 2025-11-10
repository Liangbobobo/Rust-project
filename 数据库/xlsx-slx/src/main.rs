use anyhow::Result;
use calamine::{open_workbook, Reader, Xlsx};
//use sqlx::sqlite::SqlitePoolOptions;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<()> {
  
    let path = Path::new("./sun032-wechat.xlsx");
    // 打开xlsx文件
    let mut workbook:Xlsx<_>=open_workbook(path)?;
    //或取xlsx文件中的表名
    let sheet_name:String = workbook
    .worksheets()
    .into_iter()
    .map(|(name,_)|name)
    .collect();

    println!("{:?}",sheet_name);
    //读取第一行作为表名
   
    Ok(())
}