WITH AllAccounts AS (
  -- 步骤1: 将两列的所有账户ID合并到一个列表中
  SELECT payee_payment_account AS account
  FROM transaction_records
  WHERE payee_payment_account IS NOT NULL -- 排除NULL值
  UNION ALL
  SELECT payer_payment_account AS account
  FROM transaction_records
  WHERE payer_payment_account IS NOT NULL -- 排除NULL值
),
ActiveAccounts AS (
  -- 步骤2: 从合并的列表中，筛选出出现次数超过1次的账户
  SELECT account
  FROM AllAccounts
  GROUP BY account
  HAVING COUNT(*) > 1
)
-- 步骤3: 找出原始记录中，付款方或收款方属于"活跃账户"的所有交易，并排除原始行中包含NULL的记录
SELECT *
FROM transaction_records
WHERE
  (payer_payment_account IN (SELECT account FROM ActiveAccounts)
  OR
  payee_payment_account IN (SELECT account FROM ActiveAccounts))
  AND payee_payment_account IS NOT NULL -- 确保最终结果的收款方不为NULL
  AND payer_payment_account IS NOT NULL; -- 确保最终结果的付款方不为NULL