// 对原始流水抽象出来的数据模型
//对应数据库中的 transactions 表结构
use serde::{Serialize,Deserialize};
use rust_decimal::Decimal;
use chrono::NaiveDateTime;
// [优化] 引入 FromRow，允许直接把 SQL 查询结果映射回这个结构体
// 这对于你后续写分析算法（读取数据）至关重要
use sqlx::FromRow;

//在数据清洗中，如果你想去除完全重复的行（Excel 常因复制粘贴产生重复行），拥有 PartialEq 允许你直接使用 records.dedup() 或进行 == 比对。
#[derive(Debug,Default,Clone,Serialize,Deserialize,FromRow, PartialEq)]
pub struct TransactionRecord{

        // ==========================================
    // 数据库元数据区
    // ==========================================
    
    /// **数据库主键 ID**
    /// - 写入时: 保持为 `None` (由 SQLite 自增生成)。
    /// - 读取时: 自动映射 SQLite 生成的唯一 ID。
    /// - 分析用: 作为图算法中“交易边”的唯一标识符。
    #[sqlx(default)] // 告诉 sqlx，如果 INSERT 时没这字段也没关系，SELECT 时才有
    pub id: Option<i64>,



      // ==========================================
    // 核心身份区 (Identity Section)
    // 用于图算法中的“节点 (Node)”标识
    // ==========================================

    /// **本方账号 / 支付账号**
    /// 这是资金流出的起点或流入的终点。
    /// - 类型 `Option<String>`: 允许为空（如某些特殊费率行），允许非数字字符（如邮箱账号）。
    pub account:Option<String>,

    // **本方账号扩展信息**
    //后续增加的账户所有人,账户类型,调取时长等多个字段的信息,可以以json结构转为String进行存入,且方便后续 SQLite 原生支持从 JSON 字符串中取值
    // 这对于分析“同人不同号”的情况非常重要。
    pub account_info:Option<String>,

    /// **交易类型 / 出入账标识**
    /// 例如：“转账”、“消费”、“收入”、“提现”、“手续费”。
    /// 这是判断资金流向（+ / -）的重要依据。
    pub transaction_type: Option<String>,

     // --- 双重存储策略：时间 (Dual Storage: Time) ---

    /// **[标准时间] 交易时间** (清洗后)
    /// - 目标格式: `YYYY-MM-DD HH:MM:SS` (ISO8601)
    /// - 用途: 数据库 SQL 筛选、时间窗口分析（快进快出）、排序。
    /// - 来源: 从 Excel 的日期对象转换而来。
    pub transaction_time: Option<NaiveDateTime>, 

    /// **[原始时间] 交易时间** (清洗前)
    /// - 格式: 原始 Excel 文本 (可能是 "45292.5" 或 "2024年1月1日")
    /// - 用途: **司法取证**与**审计**。如果标准时间清洗错误，可以通过此字段追溯原始值。
    pub transaction_time_raw: Option<String>,

     // --- 双重存储策略：金额 (Dual Storage: Amount) ---
    // 这是整个模型中最关键的设计，确保“既能算，又能查”。

    /// **[标准金额] 交易金额** (清洗后)
    /// - 类型: `Decimal` (无损高精度小数)
    /// - 用途: 求和、比率计算、阈值过滤。在 SQLite 中将存储为 TEXT 类型以保留精度。
    /// - 清洗逻辑: 会去除逗号、货币符号，并将浮点数转为定点数。
    pub amount: Option<Decimal>,
    
    /// **[原始金额] 交易金额** (清洗前)
    /// - 格式: 原始 Excel 文本 (可能是 "￥1,000.00" 或 "待确认")
    /// - 用途: 如果金额被识别为文本无法计算，原始信息会留在这里，确保证据链完整。
    pub amount_raw: Option<String>,

    /// **[标准余额] 交易后余额**
    /// 同样使用 Decimal 保证精度，用于校验交易链是否连续。
    pub balance: Option<Decimal>,
    
    /// **[原始余额] 交易后余额**
    pub balance_raw: Option<String>,


    // ==========================================
    // 关联方信息区 (Counterparty Section)
    // 资金追踪图中的“目标节点”
    // ==========================================
      /// **对手账号 / 收款方账号**
    /// 如果此字段为空，通常意味着这是一笔手续费或内部系统调整。
    pub counterparty_account: Option<String>,

    /// **对手户名 / 商户名称**
    /// 例如：“张三”、“京东商城”。分析“谁拿走了钱”的关键。
    pub counterparty_name: Option<String>,

    /// **对手扩展信息**
    /// 包含对方的开户行支行、省份、城市等地理位置信息，用于“异地反常交易”分析。
    pub counterparty_info: Option<String>,

    // ==========================================
    // 线索挖掘区 (Mining Section)
    // 用于文本分析和兜底
    // ==========================================

    /// **备注 / 摘要**
    /// 这里蕴含着极高的信息熵。未来可通过 LLM (AI) 提取其中的商品名、关系（"借款"、"还款"）。
    /// 需要进行 String trim 清洗。
    pub remark: Option<String>,

    /// **全量详情 (JSON)**
    /// - 这是一个兜底的黑匣子字段。
    /// - **机制**: 程序会将 Excel 中所有不属于上述固定字段的列（例如 "终端号"、"操作员"），
    ///   全部打包成一个 JSON 字符串存入这里。
    /// - **意义**: 保证宁可存得冗余，绝对不丢掉任何一个可能破案的线索字段。
    pub details: Option<String>, 
}