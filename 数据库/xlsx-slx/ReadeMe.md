还想实现一个功能,由于我在创建sqlite表时,定义的结构如下sqlx::query(
          r#"
          CREATE TABLE IF NOT EXISTS transactions (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              bill_type TEXT NOT NULL     --所有人及账单类型,自定义的列
              account TEXT NOT NULL,      --支付帐号
              transaction_type TEXT,      --交易主体的出入账标识
              transaction_time TEXT,      --交易时间
              amount INTEGER,             --交易金额
              balance INTEGER,            --交易余额
              counterparty_account TEXT,  --收款方的支付帐号
              counterparty_name TEXT,     --收款方的商户名称
              remark TEXT,                --备注
              details TEXT,               -- 原始流水中其他的列
              created_at TEXT NOT NULL DEFAULT(strftime('%Y-%m-%d%H:%M:%S', 'now', 'utc'))
          )
          "#, 其中id INTEGER PRIMARY KEY AUTOINCREMENT,还没有插入值,这一列bill_type TEXT NOT NULL
  --所有人及账单类型,自定义的列.我想在数据全部插入之后,通过查询支付账号和收款方的支付账号两列相同的情况下,将收款方的商户名称作为所有人,将导入


2.
根据xlsx文件中的哪些内容,自动判断文件是支付宝 微信 还是银行卡

3.
对于csv文件其api与xlsx文件完全不同.那么是让用户自己转换,还是增加一个crate专门处理csv文件?
