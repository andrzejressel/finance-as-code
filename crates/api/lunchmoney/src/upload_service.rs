use crate::api::LunchMoneyApi;
use crate::dto::{InsertTransactionDto, PostTransactionsRequest};
use log::{info, warn};
use rootcause::Result;
use rootcause::prelude::ResultExt;

const MAX_TRANSACTIONS_PER_INSERT_REQUEST: usize = 500;

#[cfg_attr(any(test, feature = "mock"), mockall::automock)]
pub trait LunchMoneyTransactionsUploadService {
    fn upload_transactions(
        &self,
        api_client: &dyn LunchMoneyApi,
        account_name: &str,
        transactions: &[InsertTransactionDto],
    ) -> Result<()>;
}

pub struct DefaultLunchMoneyTransactionsUploadService;

impl LunchMoneyTransactionsUploadService for DefaultLunchMoneyTransactionsUploadService {
    fn upload_transactions(
        &self,
        api_client: &dyn LunchMoneyApi,
        account_name: &str,
        transactions: &[InsertTransactionDto],
    ) -> Result<()> {
        if transactions.is_empty() {
            info!(
                "No transactions to add to Lunch Money account '{}'; skipping insert",
                account_name
            );
            return Ok(());
        }

        info!(
            "Adding '{}' transactions to Lunch Money account '{}'",
            transactions.len(),
            account_name
        );

        let total_transactions_to_insert = transactions.len();
        let total_insert_chunks =
            total_transactions_to_insert.div_ceil(MAX_TRANSACTIONS_PER_INSERT_REQUEST);
        let mut inserted_transactions = 0;

        for (chunk_index, chunk) in transactions
            .chunks(MAX_TRANSACTIONS_PER_INSERT_REQUEST)
            .enumerate()
        {
            let chunk_size = chunk.len();

            let chunk_insert_result = api_client
                .post_transactions(&PostTransactionsRequest {
                    transactions: chunk.to_vec(),
                })
                .context("failed to post transactions to Lunch Money")
                .context_with(|| format!("Failed chunk: {:?}", chunk));

            // https://discord.com/channels/842337014556262411/1134594318414389258/1474122871784869968
            if let Err(error) = chunk_insert_result {
                info!(
                    "Insert chunk {}/{} failed ({} transactions); retrying one by one for account '{}'",
                    chunk_index + 1,
                    total_insert_chunks,
                    chunk_size,
                    account_name
                );

                warn!("Error inserting chunk: {:?}", error);

                for transaction in chunk {
                    api_client
                        .post_transactions(&PostTransactionsRequest {
                            transactions: vec![transaction.clone()],
                        })
                        .context("failed to post transaction to Lunch Money")
                        .context_with(|| format!("Failed transaction: {:?}", transaction))?;

                    inserted_transactions += 1;
                    info!(
                        "Inserted retried transaction; processed {}/{} transactions for account '{}'",
                        inserted_transactions, total_transactions_to_insert, account_name
                    );
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
                    account_name
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::MockLunchMoneyApi;
    use crate::dto::{PostTransactionsResponse, TransactionDto};
    use finance_as_code_utils_chrono::date;
    use mockall::Sequence;
    use rootcause::bail;
    use rust_decimal::dec;

    fn insert_transaction(external_id: &str) -> InsertTransactionDto {
        InsertTransactionDto {
            date: date!(2024 - 01 - 01),
            amount: dec!(10.0),
            currency: Some("usd".to_string()),
            payee: Some("Counterparty".to_string()),
            notes: Some("Description".to_string()),
            manual_account_id: Some(2),
            external_id: Some(external_id.to_string()),
        }
    }

    fn posted_transaction(id: i64) -> TransactionDto {
        TransactionDto {
            id,
            date: "2024-01-01".to_string(),
            amount: dec!(10.0),
            currency: "USD".to_string(),
            payee: "Counterparty".to_string(),
            notes: Some("Description".to_string()),
        }
    }

    #[test]
    fn upload_transactions_sends_transactions_in_chunks() {
        let mut api_client = MockLunchMoneyApi::new();
        let mut sequence = Sequence::new();

        api_client
            .expect_post_transactions()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(|request| request.transactions.len() == 500)
            .return_once(|_| {
                Ok(PostTransactionsResponse {
                    transactions: vec![],
                })
            });
        api_client
            .expect_post_transactions()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(|request| request.transactions.len() == 1)
            .return_once(|_| {
                Ok(PostTransactionsResponse {
                    transactions: vec![posted_transaction(501)],
                })
            });

        let service = DefaultLunchMoneyTransactionsUploadService;
        let transactions: Vec<_> = (1..=501)
            .map(|id| insert_transaction(&id.to_string()))
            .collect();

        service
            .upload_transactions(&api_client, "Savings Jar", &transactions)
            .unwrap();
    }

    #[test]
    fn upload_transactions_retries_chunk_one_by_one_after_failure() {
        let mut api_client = MockLunchMoneyApi::new();
        let mut sequence = Sequence::new();

        api_client
            .expect_post_transactions()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(|request| request.transactions.len() == 2)
            .return_once(|_| {
                bail!("chunk insert failed");
            });
        api_client
            .expect_post_transactions()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(|request| request.transactions.len() == 1)
            .return_once(|_| {
                Ok(PostTransactionsResponse {
                    transactions: vec![posted_transaction(1)],
                })
            });
        api_client
            .expect_post_transactions()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(|request| request.transactions.len() == 1)
            .return_once(|_| {
                Ok(PostTransactionsResponse {
                    transactions: vec![posted_transaction(2)],
                })
            });

        let service = DefaultLunchMoneyTransactionsUploadService;
        let transactions = vec![insert_transaction("1"), insert_transaction("2")];

        service
            .upload_transactions(&api_client, "Savings Jar", &transactions)
            .unwrap();
    }

    #[test]
    fn upload_transactions_returns_ok_without_api_calls_when_empty() {
        let api_client = MockLunchMoneyApi::new();
        let service = DefaultLunchMoneyTransactionsUploadService;

        service
            .upload_transactions(&api_client, "Savings Jar", &[])
            .unwrap();
    }
}
