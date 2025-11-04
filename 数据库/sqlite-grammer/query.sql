-- 有点鸡肋,可以通过wps中排序实现同样功能
-- 所有收款账户payee_payment_account,与主体之间的交易
-- 对其他列同样适用,只需要改变列名
WITH RankedAccounts AS (
  SELECT *, COUNT(*) OVER (PARTITION BY payee_payment_account) AS occurrence_count
  FROM transaction_records
)
SELECT *
FROM RankedAccounts
WHERE occurrence_count > 0;

--视图不存储数据,对需要进一步查询的操作,可以存储在视图中