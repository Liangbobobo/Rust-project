// 第一版,将xlsx文件内容导入sqlite数据库

// --- 依赖项导入 ---
use anyhow::{Result, anyhow};
use calamine::{Data, Reader, Xlsx, open_workbook};
use chrono::NaiveDateTime;
use sea_orm::entity::prelude::{DateTime, Decimal};
use sea_orm::{
    ActiveValue, ConnectionTrait, Database, DatabaseConnection, EntityTrait, Schema,
    sea_query::OnConflict,
};
use std::str::FromStr;

// --- 内部模块声明 ---
mod transaction;
use transaction::{ActiveModel, Entity};

// --- 全局配置常量 ---
const DATABASE_URL: &str = "sqlite://./transactions.db?mode=rwc";
const XLSX_FILE_PATH: &str = "./1.xlsx";

#[tokio::main]
async fn main() -> Result<()> {
    let db = Database::connect(DATABASE_URL).await?;
    println!("数据库连接成功。");

    create_table_if_not_exists(&db).await?;
    println!("数据表检查/创建成功。");

    import_data(&db, XLSX_FILE_PATH).await?;

    Ok(())
}

// 使用稳定版 API 重构，现在逻辑非常清晰
async fn create_table_if_not_exists(db: &DatabaseConnection) -> Result<()> {
    let builder = db.get_database_backend();
    let schema = Schema::new(builder);

    // 构建一个"如果不存在则创建"的语句
    let mut stmt = schema.create_table_from_entity(Entity);
    stmt.if_not_exists();

    // 执行构建好的语句
    db.execute(builder.build(&stmt)).await?;
    Ok(())
}

async fn import_data(db: &DatabaseConnection, file_path: &str) -> Result<()> {
    println!("开始从 '{}' 读取数据...", file_path);
    let mut workbook: Xlsx<_> = open_workbook(file_path)?;

    let sheet_name = workbook
        .sheet_names()
        .get(0)
        .ok_or_else(|| anyhow!("XLSX文件中没有找到工作表"))?
        .clone();

    // 修复了之前版本中奇怪的 range 获取逻辑，恢复到标准的处理方式
    let range = workbook.worksheet_range(&sheet_name)?;

    let active_models: Vec<ActiveModel> = range
        .rows()
        .skip(1)
        .map(row_to_active_model)
        .filter_map(|result| {
            if let Err(e) = &result {
                eprintln!("警告: 数据解析失败: {}，已跳过。", e);
            }
            result.ok()
        })
        .collect();

    if active_models.is_empty() {
        println!("没有找到可导入的数据。");
        return Ok(());
    }

    println!(
        "共读取 {} 条有效数据，准备写入数据库...",
        active_models.len()
    );

    // 在 payment_order_id 冲突时，什么都不做
    let on_conflict = OnConflict::column(transaction::Column::PaymentOrderId)
        .do_nothing()
        .to_owned();

    let result = Entity::insert_many(active_models)
        .on_conflict(on_conflict)
        .exec(db)
        .await;

    match result {
        Ok(res) => {
            println!("数据导入成功,最后插入的ID: {:?}", res.last_insert_id);
        }
        Err(err) => {
            // 通过匹配错误文本来识别“没有记录被插入”的情况
            if err.to_string().contains("None of the records are inserted") {
                println!("所有记录都已存在于数据库中，没有新数据被插入。");
            } else {
                // 对于其他所有错误，正常抛出
                return Err(err.into());
            }
        }
    }
    Ok(())
}

fn row_to_active_model(row: &[Data]) -> Result<ActiveModel> {
    let get_string = |idx: usize| -> Option<String> {
        row.get(idx).and_then(|v| {
            let s = v.to_string();
            if s.is_empty() || s == "-" {
                None
            } else {
                Some(s)
            }
        })
    };

    let serial_number: i64 = get_string(0)
        .ok_or_else(|| anyhow!("第0列 '序号' 不能为空"))?
        .parse()?;

    let payment_order_id = get_string(1).ok_or_else(|| anyhow!("第1列 '支付订单号' 不能为空"))?;

    let transaction_time: Option<DateTime> =
        get_string(5).and_then(|s| NaiveDateTime::parse_from_str(&s, "%Y%m%d%H%M%S").ok());

    let transaction_amount: Option<Decimal> =
        get_string(7).and_then(|s| Decimal::from_str(&s).ok());

    let model = ActiveModel {
        serial_number: ActiveValue::Set(serial_number),
        payment_order_id: ActiveValue::Set(payment_order_id),
        transaction_type: ActiveValue::Set(get_string(2)),
        payment_method: ActiveValue::Set(get_string(3)),
        entry_exit_flag: ActiveValue::Set(get_string(4)),
        transaction_time: ActiveValue::Set(transaction_time),
        currency: ActiveValue::Set(get_string(6)),
        transaction_amount: ActiveValue::Set(transaction_amount),
        transaction_serial_number: ActiveValue::Set(get_string(8)),
        transaction_balance: ActiveValue::Set(get_string(9)),
        payee_bank_code: ActiveValue::Set(get_string(10)),
        payee_bank_name: ActiveValue::Set(get_string(11)),
        payee_bank_card_number: ActiveValue::Set(get_string(12)),
        payee_payment_account: ActiveValue::Set(get_string(13)),
        pos_machine_number: ActiveValue::Set(get_string(14)),
        payee_merchant_id: ActiveValue::Set(get_string(15)),
        payee_merchant_name: ActiveValue::Set(get_string(16)),
        payer_bank_code: ActiveValue::Set(get_string(17)),
        payer_bank_name: ActiveValue::Set(get_string(18)),
        payer_bank_card_number: ActiveValue::Set(get_string(19)),
        payer_payment_account: ActiveValue::Set(get_string(20)),
        device_type: ActiveValue::Set(get_string(21)),
        device_ip: ActiveValue::Set(get_string(22)),
        mac_address: ActiveValue::Set(get_string(23)),
        longitude: ActiveValue::Set(get_string(24)),
        latitude: ActiveValue::Set(get_string(25)),
        device_id: ActiveValue::Set(get_string(26)),
        external_channel_serial: ActiveValue::Set(get_string(27)),
        payment_type_2: ActiveValue::Set(get_string(28)),
        payment_account: ActiveValue::Set(get_string(29)),
        remarks: ActiveValue::Set(get_string(30)),
    };
    Ok(model)
}
