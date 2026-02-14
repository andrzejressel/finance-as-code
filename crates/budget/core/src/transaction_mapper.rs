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

    for bank_tx  in bank_txs {
        // Additional entropy for the transaction made in the same day, in the same place for the same amount
        let counter = counters.entry(bank_tx.description.clone()).or_insert(0);
        *counter += 1;

        let stable_id = generate_stable_id(&bank_tx, *counter);
        let transaction = Transaction {
            id: Uuid::new_v5(&Uuid::NAMESPACE_OID, &stable_id),
            date: bank_tx.date,
            description: bank_tx.description,
            amount: bank_tx.amount,
            other_side_account_number: bank_tx.other_side_account_number.clone(),
        };
        transactions.push(transaction);
    }

    verify_unique_ids(&transactions).context("There are multiple transactons with the same ID")?;

    Ok(transactions)
}

fn verify_unique_ids(transactions: &[Transaction]) -> Result<()> {
    let sorted_transactions = {
        let mut sorted = transactions.to_vec();
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
        "{}-{}-{}-{:?}-{}",
        bank_tx.date,
        bank_tx.description,
        bank_tx.amount,
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
                    amount: Money::from_major(100, USD),
                    other_side_account_number: None,
                },
                Transaction {
                    id: Uuid::new_v4(),
                    date: Default::default(),
                    description: "Tx2".to_string(),
                    amount: Money::from_major(100, USD),
                    other_side_account_number: None,
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
                    amount: Money::from_major(100, USD),
                    other_side_account_number: None,
                },
                Transaction {
                    id: duplicate_id,
                    date: Default::default(),
                    description: "Tx2".to_string(),
                    amount: Money::from_major(100, USD),
                    other_side_account_number: None,
                },
            ];
            assert!(verify_unique_ids(&transactions).is_err());
        }
    }

    mod map_bank_transaction_to_transaction {
        use crate::transaction_mapper::map_bank_transaction_to_transaction;
        use crate::{BankTransaction, Transaction};
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
                    amount: Money::from_major(100, USD),
                    other_side_account_number: None,
                },
                BankTransaction {
                    date: Default::default(),
                    description: "Tx".to_string(),
                    amount: Money::from_major(100, USD),
                    other_side_account_number: None,
                },
            ];

            let transactions = map_bank_transaction_to_transaction(bank_txs).unwrap();
            assert_eq!(transactions.len(), 2);
            assert_ne!(transactions[0].id, transactions[1].id);
            assert_that!(
                transactions,
                eq(&vec![
                    Transaction {
                        id: Uuid::from_str("95b38cc2-acbe-588d-a454-216ac8962a0e").unwrap(),
                        date: Default::default(),
                        description: "Tx".to_string(),
                        amount: Money::from_major(100, USD),
                        other_side_account_number: None
                    },
                    Transaction {
                        id: Uuid::from_str("dad079cb-7189-553b-a9a2-974e422f99db").unwrap(),
                        date: Default::default(),
                        description: "Tx".to_string(),
                        amount: Money::from_major(100, USD),
                        other_side_account_number: None
                    },
                ])
            );
        }
    }
}
