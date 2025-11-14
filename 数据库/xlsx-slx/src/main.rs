// --- 依赖项导入 ---
// `calamine` 用于读取 Excel 文件 (.xlsx, .xls, .ods 等)。
use calamine::{open_workbook, DataType, Reader, Xlsx};
// `serde_json` 用于处理 JSON 数据，特别是将多余的列序列化为 JSON 字符串。
use serde_json::Value;
// `sqlx` 是一个现代的、异步的 Rust SQL 工具包。这里我们使用它的 `sqlite` 功能。
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
// `std::collections::HashMap` 用于创建键值对集合，例如存储表头和其列索引的映射。
use std::collections::HashMap;
// `std::io` 模块提供了 Rust 的核心 I/O 功能，如此处的 `Write` trait 用于刷新输出缓冲区。
use std::io::{self, Write};
// `std::path` 模块提供了处理文件系统路径的工具。
use std::path::{Path, PathBuf};
// `thiserror` 是一个方便的库，用于创建自定义、结构化的错误类型。
use thiserror::Error;
// `walkdir` 库提供了一个高效遍历目录树的功能。
use walkdir::WalkDir;

// --- 使用 thiserror 定义的精细化错误类型 ---
// `#[derive(Error, Debug)]` 宏会自动为我们的 `ImportError` 枚举实现 `std::error::Error` trait 和 `Debug` trait。
#[derive(Error, Debug)]
pub enum ImportError {
    // `#[error("...")]` 定义了当这个错误变体被打印时应显示的格式化字符串。
    // `#[from]` 属性会自动将源错误类型 (如此处的 `io::Error`) 转换为我们的自定义错误变体。
    #[error("IO错误: {0}")]
    Io(#[from] io::Error),

    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Excel处理错误: {0}")]
    Excel(#[from] calamine::Error),

    // XlsxError
    #[error("Excel打开错误: {0}")]
    OpenError(#[from] calamine::XlsxError),

    #[error("JSON序列化错误: {0}")]
    Json(#[from] serde_json::Error),

    #[error("提供的路径 '{0}' 不是一个有效的目录")]
    InvalidPath(String),

    #[error("在目录 '{0}' 中没有找到任何 .xlsx 文件")]
    NoXlsxFilesFound(String),

    #[error("文件 '{0}' 中找不到任何工作表")]
    SheetNotFound(String),

    #[error("文件 '{file}' 的工作表 '{sheet}' 中缺少表头")]
    HeaderNotFound { file: String, sheet: String },

    #[error("文件 '{file}' 中缺少必需的列: '{column}'")]
    MissingRequiredColumn { file: String, column: String },

    // 这个变体用于记录可以恢复的警告，例如单行数据解析问题。
    #[error("文件 '{file}' 的第 {row} 行（数据行）解析失败: {details}")]
    RowParseWarning {
        file: String,
        row: usize,
        details: String,
    },
}

/// 用于暂存从 Excel 行解析出的单条交易记录的数据结构。
// `#[derive(Debug, Default)]` 自动实现 `Debug` trait (允许我们打印这个结构体) 和 `Default` trait (允许我们创建一个所有字段都为默认值的实例)。
#[derive(Debug, Default)]
struct TransactionRecord {
    account: Option<String>,
    transaction_type: Option<String>,
    transaction_time: Option<String>,
    amount: Option<f64>,
    balance: Option<f64>,
    counterparty_account: Option<String>,
    counterparty_name: Option<String>,
    details: Option<String>, // 存储序列化后的 JSON 字符串
}

/// 主程序入口。`#[tokio::main]` 宏将一个普通的 `async fn main` 转换成一个同步的 `main` 函数，并为其设置 Tokio 运行时。
#[tokio::main]
async fn main() {
    // 调用核心逻辑函数 `run`，并对可能返回的错误进行统一处理。
    if let Err(e) = run().await {
        // 如果 `run` 函数返回任何 `ImportError`，则将其打印到标准错误流。
        eprintln!("\n[致命错误]: 操作失败。\n详细信息: {}", e);
        // 退出程序并返回一个非零状态码，表示程序异常终止。
        std::process::exit(1);
    }
}

/// 运行主逻辑，将业务流程与顶层的错误处理分离。返回一个 `Result`，表示操作可能成功也可能失败。
async fn run() -> Result<(), ImportError> {
    // --- 用户交互部分 ---
    println!("--- XLSX 数据导入 SQLite 工具 (生产环境版) ---");
    // 打印提示信息，要求用户输入路径。
    print!("请输入xlsx文件所在的路径: ");
    // `flush()` 确保提示信息能立即显示在控制台，而不是等待缓冲区填满。
    
    // ?操作符作为语法糖解包result<T,E>,当表达式返回Err(error)时,会立即从当前函数返回.但它不会直接返回Err(error),而是先尝试将这个error转换为当前函数声明返回的错误类型E,然后再包装成Err返回.  
    // flush() 的返回类型是 io::Result<()>，它是 Result<(), std::io::Error>
    //的类型别名。所以，它失败时产生的错误类型是 std::io::Error
    //?操作符(底层是编译器)依赖std::convert::From trait实现了自动的类型转换
    //同时还能获得into()函数
    io::stdout().flush()?; // `?` 操作符用于错误传播，如果 `flush` 失败，会立即返回 `ImportError::Io`。

    // 创建一个可变的空字符串来存储用户输入。
    let mut directory_path = String::new();
    // 从标准输入读取一行，并存入 `directory_path`。
    io::stdin().read_line(&mut directory_path)?;
    // `trim()` 去除用户输入字符串两端的空白字符（如换行符）。
    let directory_path = directory_path.trim();

    // --- 核心流程 ---
    // 1. 查找所有 .xlsx 文件。
    let files = find_all_xlsx_files(directory_path)?;
    println!("\n找到 {} 个 .xlsx 文件，准备处理...", files.len());

    // 2. 设置并连接数据库。
    let pool = setup_database().await?;
    println!("数据库 'database.db' 已准备就绪。");

    // 3. 遍历并处理每个文件。
    for file_path in files {
        println!("\n-> 开始处理文件: {}...", file_path.display());
        // 对每个文件的处理结果进行匹配。
        match process_xlsx_file(&file_path, &pool).await {
            // 如果成功，打印导入的记录数和所有警告信息。
            Ok((inserted, warnings)) => {
                println!("   成功从 {} 导入 {} 条记录。", file_path.display(), inserted);
                if !warnings.is_empty() {
                    println!("   处理过程中产生 {} 个警告:", warnings.len());
                    // 为了避免刷屏，只显示前5个警告。
                    for (i, warn) in warnings.iter().take(5).enumerate() {
                        eprintln!("     {}. {}", i + 1, warn);
                    }
                    if warnings.len() > 5 {
                        eprintln!("     ... (更多警告未显示)");
                    }
                }
            }
            // 如果失败，打印错误信息，然后继续处理下一个文件。
            Err(e) => {
                eprintln!("   [错误] 处理文件 {} 失败: {}", file_path.display(), e);
            }
        }
    }

    println!("\n--- 所有文件处理完毕 ---");
    // 如果所有操作都顺利完成，返回 `Ok(())` 表示成功。
    Ok(())
}

/// 查找指定目录下的所有 .xlsx 文件。
fn find_all_xlsx_files(root_path: &str) -> Result<Vec<PathBuf>, ImportError> {
    // 将字符串路径转换为 `Path` 对象。
    let path = Path::new(root_path);
    // 检查路径是否存在且是否为目录。
    if !path.is_dir() {
        return Err(ImportError::InvalidPath(root_path.to_string()));
    }

    // 使用 `walkdir` 创建一个目录迭代器。
    let files: Vec<PathBuf> = WalkDir::new(path)
        .into_iter()
        // `filter_map` 会过滤掉迭代中产生的错误（如权限问题），只保留 `Ok` 的结果。
        .filter_map(|e| e.ok())
        // 过滤条目，只保留是文件且扩展名为 "xlsx" 的条目。
        //filter是筛选,可以看看源码的定义就明白了,其参数就是一个返回bool的闭包
        .filter(|e| 
            e.path().extension()
            //and_then从上一步产生的Option中unwrap值,对每个值执行f
            .and_then(|s| s.to_str()) == Some("xlsx"))
        // 将 `DirEntry` 对象转换为 `PathBuf`（一个拥有所有权的路径）。
        .map(|e| e.into_path())
        // 将所有符合条件的路径收集到一个 `Vec` 中。
        .collect();

    // 如果没有找到任何文件，返回错误。
    if files.is_empty() {
        return Err(ImportError::NoXlsxFilesFound(root_path.to_string()));
    }
    // 返回包含所有 .xlsx 文件路径的 `Vec`。
    Ok(files)
}

/// 设置并初始化数据库连接池和表结构。
async fn setup_database() -> Result<SqlitePool, ImportError> {
    // 创建一个 SQLite 连接池配置，最大连接数为 5。
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        // 连接到指定的数据库文件（如果不存在则会创建）。
        .connect("sqlite:database.db")
        .await?;
    // 执行 SQL 语句来创建 `transactions` 表，`IF NOT EXISTS` 确保不会重复创建。
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS transactions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            account TEXT,
            transaction_type TEXT,
            transaction_time TEXT,
            amount REAL,
            balance REAL,
            counterparty_account TEXT,
            counterparty_name TEXT,
            details TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(&pool) // 在连接池上执行查询。
    .await?;
    // 返回创建好的连接池。
    Ok(pool)
}

/// 处理单个Excel文件，返回成功插入的记录数和遇到的警告列表。
async fn process_xlsx_file(
    file_path: &Path,
    pool: &SqlitePool,
) -> Result<(i64, Vec<String>), ImportError> {
    // 使用 `calamine` 打开指定路径的 Excel 工作簿。
    let mut workbook: Xlsx<_> = open_workbook(file_path)?;
    // 将文件路径转换为字符串，用于后续的错误信息展示。
    let file_name = file_path.to_string_lossy().to_string();

    // 获取工作簿中的第一个工作表的名称。
    let sheet_name = workbook
        .sheet_names()
        .get(0)
        .cloned()
        .ok_or_else(|| ImportError::SheetNotFound(file_name.clone()))?;

    // 根据工作表名称获取其数据范围。`??` 是一个双重 `?`，第一个处理 `Option`，第二个处理 `Result`。
    let sheet = workbook
        .worksheet_range(&sheet_name)
        .map_err(|e| ImportError::SheetNotFound(file_name.clone()))?;

    // 获取工作表的所有行，并创建一个迭代器。
    let mut rows = sheet.rows();
    // 读取第一行作为表头。
    let headers: Vec<String> = rows
        .next()
        .ok_or_else(|| ImportError::HeaderNotFound {
            file: file_name.clone(),
            sheet: sheet_name.clone(),
        })?
        .iter()
        .map(|cell| cell.to_string().trim().to_string())
        .collect();

    // 验证表头是否包含所有必需列，并获取它们的索引。
    let col_indices = map_required_columns(&headers, &file_name)?;

    // 创建一个 `Vec` 来存储从 Excel 解析出的所有记录。
    let mut records = Vec::new();
    // 创建一个 `Vec` 来存储解析过程中遇到的所有警告。
    let mut warnings = Vec::new();

    // 遍历所有数据行（表头行已被消耗）。`enumerate` 提供从0开始的索引。
    for (row_idx, row) in rows.enumerate() {
        // 调用 `parse_row` 函数解析每一行。行号 `row_idx + 2` 是因为 Excel 行号从1开始，且我们跳过了表头。
        match parse_row(row, &headers, &col_indices, &file_name, row_idx + 2) {
            // 如果行解析成功，将记录添加到 `records` Vec。
            Ok(record) => records.push(record),
            // 如果是可恢复的警告，将其详情添加到 `warnings` Vec。
            Err(ImportError::RowParseWarning { details, .. }) => warnings.push(details),
            // 如果是其他不可恢复的错误，则直接向上抛出。
            Err(e) => return Err(e),
        }
    }

    // 将解析出的所有记录分批次插入数据库。
    let inserted_count = batch_insert_records(pool, records).await?;
    // 返回成功插入的记录数和警告列表。
    Ok((inserted_count, warnings))
}

/// 验证并映射Excel表头到列索引。如果任何必需列缺失，则返回错误。
fn map_required_columns<'a>(
    headers: &'a [String],
    file_name: &str,
) -> Result<HashMap<&'static str, usize>, ImportError> {
    // 定义所有必需的列名。
    const REQUIRED_COLS: &[&str] = &[
        "支付帐号",
        "交易主体的出入账标识",
        "交易时间",
        "交易金额",
        "交易余额",
        "收款方的支付帐号",
        "收款方的商户名称",
    ];
    // 创建一个 HashMap 来存储列名到其索引的映射。
    let mut indices = HashMap::new();
    // 遍历所有必需的列名。
    for &col_name in REQUIRED_COLS {
        // 在 `headers` Vec 中查找列名的位置（索引）。
        let index = headers
            .iter()
            .position(|h| h == col_name)
            .ok_or_else(|| {
                // 如果找不到，`position` 返回 `None`，`ok_or_else` 会构造并返回一个错误。
                ImportError::MissingRequiredColumn {
                    file: file_name.to_string(),
                    column: col_name.to_string(),
                }
            })?;
        // 如果找到，将列名和索引存入 HashMap。
        indices.insert(col_name, index);
    }
    // 返回包含所有必需列索引的 HashMap。
    Ok(indices)
}

/// 解析单行数据，将其转换为 `TransactionRecord`。
fn parse_row(
    row: &[DataType],
    headers: &[String],
    col_indices: &HashMap<&str, usize>,
    file: &str,
    row_num: usize,
) -> Result<TransactionRecord, ImportError> {
    // 创建一个默认的 `TransactionRecord` 实例。
    let mut record = TransactionRecord::default();
    // 创建一个 `serde_json::Map` 来存储所有不属于必需列的额外数据。
    let mut detail_map = serde_json::Map::new();

    // 使用辅助函数从行中安全地获取字符串数据。
    record.account = get_string_from_row(row, col_indices["支付帐号"]);
    record.transaction_type = get_string_from_row(row, col_indices["交易主体的出入账标识"]);
    record.transaction_time = get_string_from_row(row, col_indices["交易时间"]);
    record.counterparty_account = get_string_from_row(row, col_indices["收款方的支付帐号"]);
    record.counterparty_name = get_string_from_row(row, col_indices["收款方的商户名称"]);

    // 使用辅助函数获取浮点数，`map_err` 用于在解析失败时将错误转换为 `RowParseWarning`。
    record.amount = get_float_from_row(row, col_indices["交易金额"]).map_err(|e| {
        ImportError::RowParseWarning {
            file: file.to_string(),
            row: row_num,
            details: format!("'交易金额'列解析失败: {}", e),
        }
    })?;
    record.balance = get_float_from_row(row, col_indices["交易余额"]).map_err(|e| {
        ImportError::RowParseWarning {
            file: file.to_string(),
            row: row_num,
            details: format!("'交易余额'列解析失败: {}", e),
        }
    })?;

    // 遍历行中的每一个单元格，以收集额外数据。
    for (i, cell) in row.iter().enumerate() {
        // 检查当前列的索引是否不属于任何一个必需列。
        if !col_indices.values().any(|&idx| idx == i) {
            // 如果是额外列，获取其表头。
            if let Some(header) = headers.get(i) {
                // 确保表头不为空。
                if !header.is_empty() {
                    // 将单元格数据 (`DataType`) 转换为 `serde_json::Value`。
                    let value = match cell {
                        DataType::String(s) => Value::String(s.clone()),
                        DataType::Float(f) => Value::from(*f),
                        DataType::Int(i) => Value::from(*i),
                        DataType::Bool(b) => Value::from(*b),
                        DataType::Empty => Value::Null,
                        _ => Value::String(cell.to_string()),
                    };
                    // 将表头和转换后的值插入 `detail_map`。
                    detail_map.insert(header.clone(), value);
                }
            }
        }
    }
    // 如果 `detail_map` 中有数据，则将其序列化为 JSON 字符串。
    if !detail_map.is_empty() {
        record.details = Some(Value::Object(detail_map).to_string());
    }

    // 返回解析完成的记录。
    Ok(record)
}

// --- 数据行解析辅助函数 ---

/// 从给定的行和索引中安全地获取一个字符串。返回 `Option<String>`。
fn get_string_from_row(row: &[DataType], index: usize) -> Option<String> {
    // `get` 返回一个 `Option`，防止索引越界。
    row.get(index)
        // 将单元格内容转换为字符串。
        .map(|c| c.to_string())
        // 过滤掉空字符串，将其视作 `None`。
        .filter(|s| !s.is_empty())
}

/// 从给定的行和索引中安全地获取一个浮点数。能处理数字、字符串和空单元格。
fn get_float_from_row(row: &[DataType], index: usize) -> Result<Option<f64>, String> {
    match row.get(index) {
        // 直接是浮点数。
        Some(DataType::Float(f)) => Ok(Some(*f)),
        // 是整数，可以安全地转换为浮点数。
        Some(DataType::Int(i)) => Ok(Some(*i as f64)),
        // 是字符串但为空，视作 `None`。
        Some(DataType::String(s)) if s.trim().is_empty() => Ok(None),
        // 是非空字符串，尝试解析它。
        Some(DataType::String(s)) => s
            .parse::<f64>()
            .map(Some)
            .map_err(|_| format!("无法将字符串 '{}' 转换为数值", s)),
        // 单元格为空或索引越界，视作 `None`。
        Some(DataType::Empty) | None => Ok(None),
        // 是其他无法处理的类型，返回一个描述性错误。
        Some(other) => Err(format!("期望一个数值，但得到类型 {}", other.type_name())),
    }
}

/// 使用事务和分批次策略将记录插入数据库，以提高性能并减少内存占用。
async fn batch_insert_records(
    pool: &SqlitePool,
    records: Vec<TransactionRecord>,
) -> Result<i64, ImportError> {
    // 用于累计总共插入的行数。
    let mut total_inserted = 0;
    // 定义每个批次的大小。
    const BATCH_SIZE: usize = 500;

    // `chunks` 方法将 `records` Vec 分割成多个大小不超过 `BATCH_SIZE` 的切片。
    for chunk in records.chunks(BATCH_SIZE) {
        // 为每个批次开始一个新的数据库事务。
        let mut tx = pool.begin().await?;
        // 遍历当前批次中的每一条记录。
        for record in chunk {
            // 准备 SQL 插入语句。`?` 是 `sqlx` 中用于参数绑定的占位符。
            let result = sqlx::query(
                r#"
                INSERT INTO transactions (
                    account, transaction_type, transaction_time, amount, balance,
                    counterparty_account, counterparty_name, details
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            // 按顺序将 `record` 的字段绑定到 SQL 语句的占位符上。
            .bind(&record.account)
            .bind(&record.transaction_type)
            .bind(&record.transaction_time)
            .bind(&record.amount)
            .bind(&record.balance)
            .bind(&record.counterparty_account)
            .bind(&record.counterparty_name)
            .bind(&record.details)
            // 在事务中执行查询。
            .execute(&mut *tx)
            .await?;
            // 累加受影响的行数（通常是1）。
            total_inserted += result.rows_affected();
        }
        // 如果批次中的所有插入都成功，则提交事务。
        tx.commit().await?;
    }
    // 返回总共插入的记录数。
    Ok(total_inserted as i64)
}