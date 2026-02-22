use crate::{BankTransaction, Transaction};
use itertools::Itertools;
use rootcause::Result;
use rootcause::bail;
use rootcause::prelude::ResultExt;
use std::collections::HashMap;
use uuid::Uuid;

pub fn map_bank_transaction_to_transaction(
    bank_txs: Vec<BankTransaction>,
) -> Result<Vec<Transaction>> {
    let mut transactions = Vec::new();
    let mut counters = HashMap::new();

    for bank_tx in bank_txs {
        // Additional entropy for the transaction made in the same day, in the same place for the same amount
        let counter = counters.entry(bank_tx.description.clone()).or_insert(0);
        *counter += 1;

        let stable_id = generate_stable_id(&bank_tx, *counter);
        let transaction = Transaction {
            id: Uuid::new_v5(&Uuid::NAMESPACE_OID, &stable_id),
            date: bank_tx.date,
            description: bank_tx.description,
            counterparty: bank_tx.counterparty,
            amount: bank_tx.amount,
            other_side_account_number: bank_tx.other_side_account_number,
            tags: Default::default(),
        };
        transactions.push(transaction);
    }

    verify_unique_ids(&transactions).context("There are multiple transactons with the same ID")?;

    Ok(transactions)
}

fn verify_unique_ids(transactions: &[Transaction]) -> Result<()> {
    let sorted_transactions = {
        let mut sorted = transactions.iter().collect::<Vec<_>>();
        sorted.sort_by_key(|t| t.id);
        sorted
    };
    for (tr1, tr2) in sorted_transactions.iter().tuple_windows() {
        if tr1.id == tr2.id {
            bail!(
                "Duplicate transaction ID found. Transaction 1: [{:?}], Transaction 2: [{:?}]",
                tr1,
                tr2
            );
        }
    }
    Ok(())
}

fn generate_stable_id(bank_tx: &BankTransaction, counter: u32) -> Vec<u8> {
    let string_to_hash = format!(
        "{}-{}-{}-{}-{:?}-{}",
        bank_tx.date,
        bank_tx.description,
        bank_tx.amount,
        bank_tx.counterparty,
        bank_tx.other_side_account_number,
        counter
    );
    string_to_hash.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    mod verify_unique_ids {
        use super::*;
        use iso::USD;
        use rusty_money::{Money, iso};

        #[test]
        fn test_verify_unique_ids_no_duplicates() {
            let transactions = vec![
                Transaction {
                    id: Uuid::new_v4(),
                    date: Default::default(),
                    description: "Tx1".to_string(),
                    counterparty: "".to_string(),
                    amount: Money::from_major(100, USD),
                    other_side_account_number: None,
                    tags: Default::default(),
                },
                Transaction {
                    id: Uuid::new_v4(),
                    date: Default::default(),
                    description: "Tx2".to_string(),
                    counterparty: "".to_string(),
                    amount: Money::from_major(100, USD),
                    other_side_account_number: None,
                    tags: Default::default(),
                },
            ];
            assert!(verify_unique_ids(&transactions).is_ok());
        }

        #[test]
        fn test_verify_unique_ids_with_duplicates() {
            let duplicate_id = Uuid::new_v4();
            let transactions = vec![
                Transaction {
                    id: duplicate_id,
                    date: Default::default(),
                    description: "Tx1".to_string(),
                    counterparty: "".to_string(),
                    amount: Money::from_major(100, USD),
                    other_side_account_number: None,
                    tags: Default::default(),
                },
                Transaction {
                    id: duplicate_id,
                    date: Default::default(),
                    description: "Tx2".to_string(),
                    counterparty: "".to_string(),
                    amount: Money::from_major(100, USD),
                    other_side_account_number: None,
                    tags: Default::default(),
                },
            ];
            assert!(verify_unique_ids(&transactions).is_err());
        }
    }

    mod map_bank_transaction_to_transaction {
        use crate::BankTransaction;
        use crate::transaction_mapper::map_bank_transaction_to_transaction;
        use googletest::assert_that;
        use googletest::prelude::eq;
        use iso::USD;
        use rusty_money::{Money, iso};
        use std::str::FromStr;
        use uuid::Uuid;

        #[test]
        fn should_handle_the_same_transaction_multiple_times() {
            let bank_txs = vec![
                BankTransaction {
                    date: Default::default(),
                    description: "Tx".to_string(),
                    counterparty: "".to_string(),
                    amount: Money::from_major(100, USD),
                    other_side_account_number: None,
                },
                BankTransaction {
                    date: Default::default(),
                    description: "Tx".to_string(),
                    counterparty: "".to_string(),
                    amount: Money::from_major(100, USD),
                    other_side_account_number: None,
                },
            ];

            let transactions = map_bank_transaction_to_transaction(bank_txs).unwrap();
            assert_that!(transactions.len(), eq(2));
            assert_that!(
                transactions[0].id,
                eq(Uuid::from_str("ef3d18c2-d126-5b14-8161-bd5cafef5814").unwrap())
            );
            assert_that!(
                transactions[1].id,
                eq(Uuid::from_str("79f38cd8-91f6-526b-966b-2910027698b6").unwrap())
            );
            assert_that!(transactions[0].id == transactions[1].id, eq(false));
            assert_that!(transactions[0].tags.len(), eq(0));
            assert_that!(transactions[1].tags.len(), eq(0));
        }
    }
}
