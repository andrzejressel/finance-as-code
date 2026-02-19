use duckdb::Connection;
use finance_as_code_budget_core::Transaction;
use rootcause::compat::IntoRootcause;
use rootcause::compat::anyhow1::IntoAnyhow;
use rootcause::prelude::ResultExt;
use rootcause::*;
use treeline_core::Transaction as TreelineTransaction;
use treeline_core::TreelineContext;
use treeline_core::adapters::duckdb::DuckDbRepository;
use uuid::Uuid;

const GENERATE_BALANCES_SQL_QUERY: &str = include_str!("generate_balances.sql");
const REMOVE_ACCOUNT_BALANCES_SQL_QUERY: &str = include_str!("remove_account_balances.sql");
const REMOVE_ACCOUNT_TRANSACTIONS_SQL_QUERY: &str = include_str!("remove_account_transactions.sql");

pub(crate) fn clean_account(context: &TreelineContext, account_id: Uuid) -> Result<()> {
    context
        .repository
        .execute_sql_with_params(
            REMOVE_ACCOUNT_BALANCES_SQL_QUERY,
            &[serde_json::Value::String(account_id.to_string())],
        )
        .into_rootcause()
        .context_with(|| format!("Failed to remove balances from [{}]", account_id))?;

    context
        .repository
        .execute_sql_with_params(
            REMOVE_ACCOUNT_TRANSACTIONS_SQL_QUERY,
            &[serde_json::Value::String(account_id.to_string())],
        )
        .into_rootcause()
        .context_with(|| format!("Failed to remove transactions from [{}]", account_id))?;

    Ok(())
}

pub(crate) fn regenerate_balances(context: &TreelineContext, account_id: Uuid) -> Result<()> {
    context
        .repository
        .execute_sql_with_params(
            GENERATE_BALANCES_SQL_QUERY,
            &[serde_json::Value::String(account_id.to_string())],
        )
        .into_rootcause()
        .context_with(|| format!("Failed to regenerate balances for account [{}]", account_id))?;

    Ok(())
}

pub(crate) fn add_transactions(
    context: &TreelineContext,
    account_id: Uuid,
    transactions: &[Transaction],
) -> Result<()> {
    context.repository.use_connection_rootcause(move |conn| {
        let mut appender = conn
            .appender_with_columns(
                "sys_transactions",
                &[
                    "transaction_id",
                    "account_id",
                    "amount",
                    "description",
                    "transaction_date",
                    "posted_date",
                ],
            )
            .context("Failed to create appender for sys_transactions")?;

        let treeline_txs = transactions
            .iter()
            .map(|tx| to_treeline_transaction(tx, account_id))
            .collect::<Vec<_>>();

        for tx in treeline_txs {
            appender
                .append_row([
                    &tx.id.to_string(),
                    &tx.account_id.to_string(),
                    &tx.amount.to_string(),
                    &tx.description.clone().unwrap_or_default(),
                    &tx.transaction_date.format("%Y-%m-%d").to_string(),
                    &tx.posted_date.format("%Y-%m-%d").to_string(),
                ])
                .context_with(|| format!("Failed to append transaction [{:?}]", tx))?;
        }

        Ok(())
    })
}

trait DuckDbRepositoryExt {
    fn use_connection_rootcause(&self, f: impl FnOnce(&Connection) -> Result<()>) -> Result<()>;
}

impl DuckDbRepositoryExt for DuckDbRepository {
    fn use_connection_rootcause(&self, f: impl FnOnce(&Connection) -> Result<()>) -> Result<()> {
        self.use_connection(|conn| f(conn).into_anyhow())
            .into_rootcause()
    }
}

pub(crate) fn to_treeline_transaction(tx: &Transaction, account_id: Uuid) -> TreelineTransaction {
    let mut t = TreelineTransaction::new(tx.id, account_id, *tx.amount.amount(), tx.date);
    t.description = Some(tx.get_full_description());
    t
}
