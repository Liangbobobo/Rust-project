// 为了兼容不同的交易明细,选择将核心字段存入数据库的列中,每种交易明细中非必要或者独有的列,放入json中,作为一个details列

use anyhow::{Ok, Result, anyhow};
use calamine::{Reader, Xlsx, open_workbook};
//use sqlx::sqlite::SqlitePoolOptions;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[tokio::main]
async fn main() -> Result<()> {
    //提示
    println!("--- XLSX 数据导入 SQLite 工具 ---");
    println!("本工具将扫描指定文件夹中的所有 .xlsx 文件，并将其内容导入到 SQLite 数据库中。");
    println!("数据库文件名为 'database.db'，将创建在程序运行目录下。");
    println!("将使用第一个找到的 .xlsx 文件的第一个工作表的表头作为所有数据表的统一列名。");
    println!("请确保提供的xlsx文件为从系统下载的原始文件,不要更改里面的内容");

    //获取用户输入的文件夹路径
    println!("输入xlsx文件所在的路径");
    let mut directory_path = String::new();

    //刷新标准输出缓冲区
    io::stdout().flush()?;
    io::stdin().read_line(&mut directory_path)?;

    //去除输入的路径的空白
    let directory_path = directory_path.trim();

    //对路径中的文件进行处理,主要是当路径不对,文件夹中无文件等错误控制
    async fn find_all_xlsx_files(root_path: &str) -> Result<Vec<PathBuf>> {
        let path = Path::new(root_path);

        //.is_dir() 判断该路径是否存在并且是否是一个目录。
        if !path.is_dir() {
            return Err(anyhow!(
                "提供的路径 '{}' 不是一个有效的目录。",
                path.display()
            ));
        }

        //遍历目录
        //目录路径的错误控制
        //目录中文件格式的错误控制
        let all_xlsx_files:Vec<PathBuf> = WalkDir::new(root_path)
        //产生迭代器,因为Walk::new方法impl了IntoIterator trait
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|entry| {
                entry.file_type().is_file()
                    && entry.path().extension().and_then(|s| s.to_str()) == Some("xlsx")
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();

            //目录中无文件
            if all_xlsx_files.is_empty() {
                return Err(anyhow!("目录{}中无xlsx文件",path.display()));
            }
        Ok(all_xlsx_files)
    }

    let path = Path::new("./sun032-wechat.xlsx");
    // 打开xlsx文件
    let mut workbook: Xlsx<_> = open_workbook(path)?;
    //或取xlsx文件中的表名
    let sheet_name: String = workbook
        .worksheets()
        .into_iter()
        .map(|(name, _)| name)
        .collect();

    println!("{:?}", sheet_name);

    //打开工作表
    let range = workbook
        .worksheet_range_at(0)
        .ok_or_else(|| anyhow!("无法获取第一个工作表"))??; //错误处理，尚未完全理解


    //为了兼容其他流水,应该选取部分列插入数据库
    //读取第一行作为列名
    // let header: Vec<String> = range
    //     .rows()
    //     .next()
    //     .ok_or_else(|| anyhow!("工作表 '{}' 为空，找不到表头", sheet_name))?
    //     .iter()
    //     .map(|cell| cell.to_string())
    //     .collect();


    
    Ok(())
}
