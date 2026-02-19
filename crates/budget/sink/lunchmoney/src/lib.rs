use finance_as_code_api_lunchmoney::api::LunchMoneyApi;
use finance_as_code_api_lunchmoney::dto::{
    DeleteTransactionsRequest, InsertTransactionDto, PostTransactionsRequest,
};
use finance_as_code_budget_core::sink::Sink;
use finance_as_code_budget_core::Transaction;
use log::{info, warn};
use rootcause::option_ext::OptionExt;
use rootcause::prelude::ResultExt;
use rootcause::Result;

const MAX_TRANSACTIONS_PER_DELETE_REQUEST: usize = 500;
const MAX_TRANSACTIONS_PER_INSERT_REQUEST: usize = 50;

#[derive(Clone, Debug)]
pub struct LunchMoneyApiKey(String);

impl LunchMoneyApiKey {
    pub fn new(key: String) -> Self {
        Self(key)
    }

    pub(crate) fn value(&self) -> &str {
        &self.0
    }
}

impl From<String> for LunchMoneyApiKey {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for LunchMoneyApiKey {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Clone, Debug)]
pub struct LunchMoneyAccountName(String);

impl LunchMoneyAccountName {
    pub fn new(name: String) -> Self {
        Self(name)
    }

    pub(crate) fn value(&self) -> &str {
        &self.0
    }
}

impl From<String> for LunchMoneyAccountName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for LunchMoneyAccountName {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(bon::Builder, Clone, Debug)]
pub struct LunchMoneySinkConfig {
    #[builder(into)]
    pub(crate) api_key: LunchMoneyApiKey,
    #[builder(into)]
    pub(crate) account_name: LunchMoneyAccountName,
}

pub struct LunchMoneySink {
    config: LunchMoneySinkConfig,
}

pub fn create_lunchmoney_sink(config: LunchMoneySinkConfig) -> impl Sink {
    LunchMoneySink { config }
}

impl Sink for LunchMoneySink {
    fn name(&self) -> &str {
        "Lunch Money"
    }

    fn write(&self, transactions: &[Transaction]) -> Result<()> {
        let client = finance_as_code_api_lunchmoney::api::LunchMoneyClient::new(
            "https://api.lunchmoney.dev/v2".to_string(),
            self.config.api_key.value().into(),
        );

        let account_id = Self::get_account_id_for_account_name(&self.config.account_name, &client)
            .context("failed to get account ID")?;

        let all_transactions = client
            .get_all_transactions(
                &finance_as_code_api_lunchmoney::dto::GetTransactionsParams {
                    manual_account_id: Some(account_id),
                    ..Default::default()
                },
            )
            .context("failed to get existing transactions for account")?;

        if !all_transactions.is_empty() {
            info!(
                "Remove existing '{}' transactions from Lunch Money account '{}'",
                all_transactions.len(),
                self.config.account_name.value()
            );

            let total_transactions_to_delete = all_transactions.len();
            let total_delete_chunks =
                total_transactions_to_delete.div_ceil(MAX_TRANSACTIONS_PER_DELETE_REQUEST);
            let mut deleted_transactions = 0;

            for (chunk_index, chunk) in all_transactions
                .chunks(MAX_TRANSACTIONS_PER_DELETE_REQUEST)
                .enumerate()
            {
                let ids: Vec<_> = chunk.iter().map(|transaction| transaction.id).collect();
                let chunk_size = ids.len();

                client
                    .delete_transactions(&DeleteTransactionsRequest { ids })
                    .context("failed to delete existing transactions")?;

                deleted_transactions += chunk_size;
                info!(
                    "Deleted chunk {}/{} ({} transactions); processed {}/{} existing transactions for account '{}'",
                    chunk_index + 1,
                    total_delete_chunks,
                    chunk_size,
                    deleted_transactions,
                    total_transactions_to_delete,
                    self.config.account_name.value()
                );
            }
            //
            // client
            //     .delete_transactions(&DeleteTransactionsRequest {
            //         ids: all_transactions
            //             .into_iter()
            //             .map(|transaction| transaction.id)
            //             .collect(),
            //     })
            //     .context("failed to delete existing transactions")?;
        } else {
            info!(
                "No existing transactions found in Lunch Money account '{}'",
                self.config.account_name.value()
            );
        }

        if transactions.is_empty() {
            info!(
                "No transactions to add to Lunch Money account '{}'; skipping insert",
                self.config.account_name.value()
            );
            return Ok(());
        }

        info!(
            "Adding '{}' transactions to Lunch Money account '{}'",
            transactions.len(),
            self.config.account_name.value()
        );

        let transactions = transactions.to_vec();
        let total_transactions_to_insert = transactions.len();
        let total_insert_chunks =
            total_transactions_to_insert.div_ceil(MAX_TRANSACTIONS_PER_INSERT_REQUEST);
        let mut inserted_transactions = 0;

        for (chunk_index, chunk) in transactions
            .chunks(MAX_TRANSACTIONS_PER_INSERT_REQUEST)
            .enumerate()
        {
            let chunk_size = chunk.len();
            let transactions = chunk
                .iter()
                .map(|transaction| Self::to_insert_transaction(transaction, account_id))
                .collect();

            let r = client
                .post_transactions(&PostTransactionsRequest { transactions })
                .context("failed to post transactions to Lunch Money")
                .context_with(|| format!("Failed chunk: {:?}", chunk));

            if r.is_err() {
                info!(
                    "Insert chunk {}/{} failed ({} transactions); retrying one by one for account '{}'",
                    chunk_index + 1,
                    total_insert_chunks,
                    chunk_size,
                    self.config.account_name.value()
                );

                warn!("Error inserting chunk: {:?}", r.err());

                for transaction in chunk {
                    client
                        .post_transactions(&PostTransactionsRequest {
                            transactions: vec![Self::to_insert_transaction(
                                transaction,
                                account_id,
                            )],
                        })
                        .context("failed to post transaction to Lunch Money")
                        .context_with(|| format!("Failed transaction: {:?}", transaction))?;

                    inserted_transactions += 1;
                    info!(
                        "Inserted retried transaction; processed {}/{} transactions for account '{}'",
                        inserted_transactions,
                        total_transactions_to_insert,
                        self.config.account_name.value()
                    );

                    // info!("Failed transaction: {:?}", transaction);
                }
            } else {
                inserted_transactions += chunk_size;
                info!(
                    "Inserted chunk {}/{} ({} transactions); processed {}/{} transactions for account '{}'",
                    chunk_index + 1,
                    total_insert_chunks,
                    chunk_size,
                    inserted_transactions,
                    total_transactions_to_insert,
                    self.config.account_name.value()
                );
            }
        }

        Ok(())
    }
}

impl LunchMoneySink {
    fn to_insert_transaction(transaction: &Transaction, account_id: i64) -> InsertTransactionDto {
        InsertTransactionDto {
            date: transaction.date.format("%Y-%m-%d").to_string(),
            amount: -*transaction.amount.amount(),
            currency: Some(transaction.amount.currency().iso_alpha_code.to_lowercase()),
            notes: Some(transaction.description.clone()),
            payee: None,
            manual_account_id: Some(account_id),
            external_id: Some(transaction.id.to_string()),
        }
    }

    fn get_account_id_for_account_name(
        account_name: &LunchMoneyAccountName,
        api_client: &impl LunchMoneyApi,
    ) -> Result<i64> {
        let manual_accounts = api_client
            .get_all_manual_accounts()
            .context("Failed to retrieve Lunch Money manual accounts")?;

        Ok(manual_accounts
            .into_iter()
            .find(|manual_account| manual_account.name == account_name.value())
            .map(|manual_account| manual_account.id)
            .context_with(|| format!("Account with name '{}' not found", account_name.value()))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finance_as_code_api_lunchmoney::api::MockLunchMoneyApi;
    use finance_as_code_api_lunchmoney::dto::ManualAccountDto;
    use googletest::prelude::*;

    #[test]
    fn get_account_id_for_account_name_returns_account_id() -> googletest::Result<()> {
        let mut api_client = MockLunchMoneyApi::new();
        api_client
            .expect_get_all_manual_accounts()
            .times(1)
            .return_once(|| {
                Ok(vec![
                    ManualAccountDto {
                        id: 1,
                        name: "Cash Wallet".to_string(),
                    },
                    ManualAccountDto {
                        id: 2,
                        name: "Savings Jar".to_string(),
                    },
                ])
            });

        let account_id = LunchMoneySink::get_account_id_for_account_name(
            &LunchMoneyAccountName::from("Savings Jar"),
            &api_client,
        )
        .unwrap();

        verify_that!(account_id, eq(2))?;
        Ok(())
    }

    #[test]
    fn get_account_id_for_account_name_returns_error_when_account_is_missing(
    ) -> googletest::Result<()> {
        let mut api_client = MockLunchMoneyApi::new();
        api_client
            .expect_get_all_manual_accounts()
            .times(1)
            .return_once(|| {
                Ok(vec![ManualAccountDto {
                    id: 1,
                    name: "Cash Wallet".to_string(),
                }])
            });

        let error = LunchMoneySink::get_account_id_for_account_name(
            &LunchMoneyAccountName::from("Savings Jar"),
            &api_client,
        )
        .unwrap_err();

        verify_that!(
            error.to_string(),
            contains_substring("Account with name 'Savings Jar' not found")
        )?;
        Ok(())
    }
}
