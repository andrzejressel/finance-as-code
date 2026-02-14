INSERT into sys_balance_snapshots (snapshot_id, account_id, balance, snapshot_time, created_at, updated_at, source) FROM
(SELECT uuid() as snapshot_id, ? as account_id,  closing_balance as balance, (day || ' 23:59:59')::TIMESTAMPTZ as snapshot_time, now(), now(), 'manual' as source FROM (WITH range AS (
    SELECT MIN(transaction_date) as start_date, MAX(transaction_date) as end_date
    FROM sys_transactions
),
calendar AS ( SELECT DATE(unnest(day)) as day FROM (SELECT generate_series(start_date, end_date, INTERVAL 1 DAY) AS day FROM range)),
daily_net AS (
    SELECT transaction_date, SUM(amount) as daily_total
    FROM sys_transactions
    GROUP BY transaction_date
)
SELECT
    c.day,
    -- Show 0 if no transaction happened that day
    COALESCE(d.daily_total, 0) as daily_change,
    -- Calculate rolling sum over the continuous timeline
    SUM(COALESCE(d.daily_total, 0)) OVER (
        ORDER BY c.day ASC
        ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    ) as closing_balance
FROM calendar c
         LEFT JOIN daily_net d ON c.day = d.transaction_date
ORDER BY c.day));