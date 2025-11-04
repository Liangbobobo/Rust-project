#![allow(dead_code)] //消除未使用type警告

use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, DeriveEntityModel, EnumIter};

// 使用 sea-orm 定义一个名为 `transaction_records` 的表实体
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "transaction_records")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub serial_number: i64, // 序号 (作为主键)

    #[sea_orm(column_type = "Text", unique)]
    pub payment_order_id: String, // 支付订单号 (唯一)

    #[sea_orm(column_type = "Text", nullable)]
    pub transaction_type: Option<String>, // 交易类型

    #[sea_orm(column_type = "Text", nullable)]
    pub payment_method: Option<String>, // 支付类型

    #[sea_orm(column_type = "Text", nullable)]
    pub entry_exit_flag: Option<String>, // 交易主体的出入账标识

    #[sea_orm(nullable)]
    pub transaction_time: Option<DateTime>, // 交易时间

    #[sea_orm(column_type = "Text", nullable)]
    pub currency: Option<String>, // 币种

    #[sea_orm(column_type = "Decimal(Some((16, 4)))", nullable)]
    pub transaction_amount: Option<Decimal>, // 交易金额

    #[sea_orm(column_type = "Text", nullable)]
    pub transaction_serial_number: Option<String>, // 交易流水号

    #[sea_orm(column_type = "Text", nullable)]
    pub transaction_balance: Option<String>, // 交易余额

    #[sea_orm(column_type = "Text", nullable)]
    pub payee_bank_code: Option<String>, // 收款方银行卡所属银行机构编码

    #[sea_orm(column_type = "Text", nullable)]
    pub payee_bank_name: Option<String>, // 收款方银行卡所属银行名称

    #[sea_orm(column_type = "Text", nullable)]
    pub payee_bank_card_number: Option<String>, // 收款方银行卡所属银行卡号

    #[sea_orm(column_type = "Text", nullable)]
    pub payee_payment_account: Option<String>, // 收款方的支付帐号

    #[sea_orm(column_type = "Text", nullable)]
    pub pos_machine_number: Option<String>, // 消费pos机编号

    #[sea_orm(column_type = "Text", nullable)]
    pub payee_merchant_id: Option<String>, // 收款方的商户号

    #[sea_orm(column_type = "Text", nullable)]
    pub payee_merchant_name: Option<String>, // 收款方的商户名称

    #[sea_orm(column_type = "Text", nullable)]
    pub payer_bank_code: Option<String>, // 付款方银行卡所属银行机构编码

    #[sea_orm(column_type = "Text", nullable)]
    pub payer_bank_name: Option<String>, // 付款方银行卡所属银行名称

    #[sea_orm(column_type = "Text", nullable)]
    pub payer_bank_card_number: Option<String>, // 付款方银行卡所属银行卡号

    #[sea_orm(column_type = "Text", nullable)]
    pub payer_payment_account: Option<String>, // 付款方的支付帐号

    #[sea_orm(column_type = "Text", nullable)]
    pub device_type: Option<String>, // 交易设备类型

    #[sea_orm(column_type = "Text", nullable)]
    pub device_ip: Option<String>, // 交易支付设备ip

    #[sea_orm(column_type = "Text", nullable)]
    pub mac_address: Option<String>, // mac地址

    #[sea_orm(column_type = "Text", nullable)]
    pub longitude: Option<String>, // 交易地点经度

    #[sea_orm(column_type = "Text", nullable)]
    pub latitude: Option<String>, // 交易地点纬度

    #[sea_orm(column_type = "Text", nullable)]
    pub device_id: Option<String>, // 交易设备号

    #[sea_orm(column_type = "Text", nullable)]
    pub external_channel_serial: Option<String>, // 银行外部渠道交易流水号

    #[sea_orm(column_type = "Text", nullable)]
    pub payment_type_2: Option<String>, // 支付类型 (重复字段)

    #[sea_orm(column_type = "Text", nullable)]
    pub payment_account: Option<String>, // 支付帐号

    #[sea_orm(column_type = "Text", nullable)]
    pub remarks: Option<String>, // 备注
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
