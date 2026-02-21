use crate::api::LunchMoneyApi;
use crate::dto::{DeleteTransactionsRequest, TransactionDto};
use log::info;
use rootcause::Result;
use rootcause::prelude::ResultExt;

const MAX_TRANSACTIONS_PER_DELETE_REQUEST: usize = 500;

#[cfg_attr(any(test, feature = "mock"), mockall::automock)]
pub trait LunchMoneyTransactionsDeletionService {
    fn delete_transactions(
        &self,
        api_client: &dyn LunchMoneyApi,
        account_name: &str,
        transactions: &[TransactionDto],
    ) -> Result<()>;
}

pub struct DefaultLunchMoneyTransactionsDeletionService;

impl LunchMoneyTransactionsDeletionService for DefaultLunchMoneyTransactionsDeletionService {
    fn delete_transactions(
        &self,
        api_client: &dyn LunchMoneyApi,
        account_name: &str,
        transactions: &[TransactionDto],
    ) -> Result<()> {
        if transactions.is_empty() {
            info!(
                "No existing transactions found in Lunch Money account '{}'",
                account_name
            );
            return Ok(());
        }

        info!(
            "Remove existing '{}' transactions from Lunch Money account '{}'",
            transactions.len(),
            account_name
        );

        let total_transactions_to_delete = transactions.len();
        let total_delete_chunks =
            total_transactions_to_delete.div_ceil(MAX_TRANSACTIONS_PER_DELETE_REQUEST);
        let mut deleted_transactions = 0;

        for (chunk_index, chunk) in transactions
            .chunks(MAX_TRANSACTIONS_PER_DELETE_REQUEST)
            .enumerate()
        {
            let ids: Vec<_> = chunk.iter().map(|transaction| transaction.id).collect();
            let chunk_size = ids.len();

            api_client
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
                account_name
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::MockLunchMoneyApi;
    use mockall::Sequence;
    use rust_decimal::dec;

    fn transaction(id: i64) -> TransactionDto {
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
    fn delete_transactions_sends_requests_in_chunks() {
        let mut api_client = MockLunchMoneyApi::new();
        let mut sequence = Sequence::new();

        api_client
            .expect_delete_transactions()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(|request| request.ids.len() == 500)
            .return_once(|_| Ok(()));
        api_client
            .expect_delete_transactions()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(|request| request.ids.len() == 1)
            .return_once(|_| Ok(()));

        let service = DefaultLunchMoneyTransactionsDeletionService;
        let transactions: Vec<_> = (1..=501).map(transaction).collect();

        service
            .delete_transactions(&api_client, "Savings Jar", &transactions)
            .unwrap();
    }

    #[test]
    fn delete_transactions_returns_ok_without_api_calls_when_empty() {
        let api_client = MockLunchMoneyApi::new();
        let service = DefaultLunchMoneyTransactionsDeletionService;

        service
            .delete_transactions(&api_client, "Savings Jar", &[])
            .unwrap();
    }
}
