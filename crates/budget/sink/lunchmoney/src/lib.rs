use itertools::Itertools;
use finance_as_code_api_lunchmoney::api::LunchMoneyApi;
use finance_as_code_api_lunchmoney::dto::{
    DeleteTransactionsRequest, InsertTransactionDto, PostTransactionsRequest,
};
use finance_as_code_budget_core::Transaction;
use finance_as_code_budget_core::sink::Sink;
use log::info;
use rootcause::Result;
use rootcause::option_ext::OptionExt;
use rootcause::prelude::ResultExt;

const MAX_TRANSACTIONS_PER_REQUEST: usize = 500;

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
            
            for chunk in &all_transactions.iter().chunks(MAX_TRANSACTIONS_PER_REQUEST) {
                let ids = chunk.map(|t| &t.id).cloned().collect_vec();
                client
                    .delete_transactions(&DeleteTransactionsRequest {
                        ids
                    })
                    .context("failed to delete existing transactions")?;
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
        
        let mut transactions = transactions.to_vec();

        for chunk in transactions.chunks(MAX_TRANSACTIONS_PER_REQUEST) {
            let transactions = chunk
                .iter()
                .map(|transaction| Self::to_insert_transaction(transaction, account_id))
                .collect();

            let r = client
                .post_transactions(&PostTransactionsRequest { transactions })
                .context("failed to post transactions to Lunch Money")
                .context_with(|| format!("Failed chunk: {:?}", chunk));
            
            if r.is_err() {
                for transaction in chunk {
                    
                    client
                        .post_transactions(&PostTransactionsRequest {
                            transactions: vec![Self::to_insert_transaction(transaction, account_id)],
                        })
                        .context("failed to post transaction to Lunch Money")
                        .context_with(|| format!("Failed transaction: {:?}", transaction))?;
                    
                    // info!("Failed transaction: {:?}", transaction);
                }
            }
        }

        Ok(())
    }
}

impl LunchMoneySink {
    fn to_insert_transaction(transaction: &Transaction, account_id: i64) -> InsertTransactionDto {
        InsertTransactionDto {
            date: transaction.date.format("%Y-%m-%d").to_string(),
            amount: format!("{:.4}", transaction.amount.amount()),
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
    use finance_as_code_api_lunchmoney::dto::{
        DeleteTransactionsRequest, GetTransactionsParams, ManualAccountDto,
        PostTransactionsRequest, PostTransactionsResponse, PutTransactionsRequest,
        PutTransactionsResponse, TransactionDto,
    };
    use googletest::prelude::*;
    use rootcause::Result as RootResult;

    struct FakeLunchMoneyApi {
        manual_accounts: Vec<ManualAccountDto>,
    }

    impl LunchMoneyApi for FakeLunchMoneyApi {
        fn get_all_manual_accounts(&self) -> RootResult<Vec<ManualAccountDto>> {
            Ok(self.manual_accounts.clone())
        }

        fn get_all_transactions(
            &self,
            _params: &GetTransactionsParams,
        ) -> RootResult<Vec<TransactionDto>> {
            panic!("get_all_transactions should not be called in this test")
        }

        fn put_transactions(
            &self,
            _request: &PutTransactionsRequest,
        ) -> RootResult<PutTransactionsResponse> {
            panic!("put_transactions should not be called in this test")
        }

        fn post_transactions(
            &self,
            _request: &PostTransactionsRequest,
        ) -> RootResult<PostTransactionsResponse> {
            panic!("post_transactions should not be called in this test")
        }

        fn delete_transactions(&self, _request: &DeleteTransactionsRequest) -> RootResult<()> {
            panic!("delete_transactions should not be called in this test")
        }
    }

    #[test]
    fn get_account_id_for_account_name_returns_account_id() -> googletest::Result<()> {
        let api_client = FakeLunchMoneyApi {
            manual_accounts: vec![
                ManualAccountDto {
                    id: 1,
                    name: "Cash Wallet".to_string(),
                },
                ManualAccountDto {
                    id: 2,
                    name: "Savings Jar".to_string(),
                },
            ],
        };

        let account_id = LunchMoneySink::get_account_id_for_account_name(
            &LunchMoneyAccountName::from("Savings Jar"),
            &api_client,
        )
        .unwrap();

        verify_that!(account_id, eq(2))?;
        Ok(())
    }

    #[test]
    fn get_account_id_for_account_name_returns_error_when_account_is_missing()
    -> googletest::Result<()> {
        let api_client = FakeLunchMoneyApi {
            manual_accounts: vec![ManualAccountDto {
                id: 1,
                name: "Cash Wallet".to_string(),
            }],
        };

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
