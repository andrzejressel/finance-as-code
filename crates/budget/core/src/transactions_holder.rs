use crate::BankTransaction;
use chrono::NaiveDate;
use nonempty_collections::NEVec;
use nonempty_collections::*;
use std::vec::from_elem;

#[derive(Debug, PartialEq)]
pub struct TransactionHolder {
    variants: TransactionHolderVariants,
}

impl TransactionHolder {
    pub fn empty() -> Self {
        TransactionHolder {
            variants: TransactionHolderVariants::empty(),
        }
    }
    pub fn new(transactions: Vec<BankTransaction>) -> Self {
        TransactionHolder {
            variants: TransactionHolderVariants::from_transactions(transactions),
        }
    }

    pub fn number_of_transactions(&self) -> usize {
        self.variants.number_of_transactions()
    }

    pub fn into_transactions(self) -> Vec<BankTransaction> {
        self.variants.into_transactions()
    }

    pub fn combine(mut self, other: TransactionHolder) -> Self {
        self.variants = self.variants.combine(other.variants);
        self
    }

    pub fn combine_vec(holders: Vec<TransactionHolder>) -> Self {
        holders.into_iter().fold(
            TransactionHolder {
                variants: TransactionHolderVariants::empty(),
            },
            |acc, holder| acc.combine(holder),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
enum TransactionHolderVariants {
    Empty,
    WithItems(TransactionsHolderWithItems),
}

impl TransactionHolderVariants {
    fn empty() -> Self {
        TransactionHolderVariants::Empty
    }

    fn into_transactions(self) -> Vec<BankTransaction> {
        match self {
            TransactionHolderVariants::Empty => vec![],
            TransactionHolderVariants::WithItems(holder) => {
                holder.transactions.into_iter().flatten().collect()
            }
        }
    }

    pub(crate) fn number_of_transactions(&self) -> usize {
        match self {
            TransactionHolderVariants::Empty => 0,
            TransactionHolderVariants::WithItems(holder) => {
                holder.transactions.iter().map(|day| day.len()).sum()
            }
        }
    }

    fn from_transactions(transactions: Vec<BankTransaction>) -> Self {
        match NEVec::try_from_vec(transactions) {
            None => TransactionHolderVariants::Empty,
            Some(transactions) => {
                TransactionHolderVariants::WithItems(group_bank_transactions(transactions))
            }
        }
    }

    fn combine(self, other: TransactionHolderVariants) -> Self {
        combine_holders(self, other)
    }
}

fn combine_holders(
    holder1: TransactionHolderVariants,
    holder2: TransactionHolderVariants,
) -> TransactionHolderVariants {
    match (holder1, holder2) {
        (TransactionHolderVariants::Empty, h) | (h, TransactionHolderVariants::Empty) => h,
        (TransactionHolderVariants::WithItems(h1), TransactionHolderVariants::WithItems(h2)) => {
            let start_date = std::cmp::min(h1.start_date, h2.start_date);
            let end_date = std::cmp::max(h1.end_date, h2.end_date);

            let mut combined_transactions =
                from_elem(vec![], (end_date - start_date).num_days() as usize + 1);

            let relative_start_date_h1 = (h1.start_date - start_date).num_days() as usize;
            let relative_end_date_h1 = (h1.end_date - start_date).num_days() as usize;
            let relative_start_date_h2 = (h2.start_date - start_date).num_days() as usize;
            let relative_end_date_h2 = (h2.end_date - start_date).num_days() as usize;

            let _ = combined_transactions.splice(
                relative_start_date_h1..=relative_end_date_h1,
                h1.transactions.into_iter(),
            );
            let _ = combined_transactions.splice(
                relative_start_date_h2..=relative_end_date_h2,
                h2.transactions.into_iter(),
            );

            TransactionHolderVariants::WithItems(TransactionsHolderWithItems {
                transactions: combined_transactions,
                start_date,
                end_date,
            })
        }
    }
}

fn group_bank_transactions(transactions: NEVec<BankTransaction>) -> TransactionsHolderWithItems {
    let start_date = transactions.nonempty_iter().map(|t| t.date).min();
    let end_date = transactions.nonempty_iter().map(|t| t.date).max();
    let number_of_days = ((end_date - start_date).num_days() + 1) as usize;

    let mut vec = from_elem(vec![], number_of_days);

    for transaction in transactions.into_iter() {
        let day_index = (transaction.date - start_date).num_days() as usize;
        vec[day_index].push(transaction);
    }

    TransactionsHolderWithItems {
        transactions: vec,
        start_date,
        end_date,
    }
}

#[derive(Clone, Debug, PartialEq)]
struct TransactionsHolderWithItems {
    transactions: Vec<Vec<BankTransaction>>,
    start_date: NaiveDate,
    end_date: NaiveDate,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NonEmptyString;
    use finance_as_code_utils_chrono::{date, datetime};
    use googletest::prelude::eq;
    use googletest::prelude::*;
    use googletest::{expect_that, verify_that};
    use rusty_money::{Money, iso};

    #[test]
    fn test_transaction_holder() -> Result<()> {
        let transaction1 = BankTransaction {
            date: date!(2024 - 01 - 01),
            description: "Transaction 1".to_string(),
            amount: Money::from_major(100, iso::USD),
            other_side_account_number: NonEmptyString::new_str("123456789"),
        };
        let transaction2 = BankTransaction {
            date: date!(2024 - 01 - 03),
            description: "Transaction 2".to_string(),
            amount: Money::from_major(200, iso::USD),
            other_side_account_number: NonEmptyString::new_str("987654321"),
        };

        let transactions = vec![transaction1.clone(), transaction2.clone()];

        let holder = group_bank_transactions(NEVec::try_from_vec(transactions).unwrap());

        verify_that!(holder.start_date, eq(date!(2024 - 01 - 01)))?;
        verify_that!(holder.end_date, eq(date!(2024 - 01 - 03)))?;
        verify_that!(
            holder.transactions,
            container_eq(vec![vec![transaction1], vec![], vec![transaction2],])
        )?;
        Ok(())
    }

    mod holder {
        use super::*;

        #[test]
        fn create_holder_from_empty_transactions() -> Result<()> {
            let holder = TransactionHolderVariants::from_transactions(vec![]);
            verify_that!(holder, eq(&TransactionHolderVariants::Empty))?;
            Ok(())
        }

        #[test]
        fn create_holder_from_non_empty_transactions() -> Result<()> {
            let transaction = BankTransaction {
                date: date!(2024 - 01 - 01),
                description: "Transaction".to_string(),
                amount: Money::from_major(100, iso::USD),
                other_side_account_number: NonEmptyString::new_str("123456789"),
            };
            let transaction2 = BankTransaction {
                date: date!(2024 - 01 - 03),
                description: "Transaction 2".to_string(),
                amount: Money::from_major(200, iso::USD),
                other_side_account_number: NonEmptyString::new_str("987654321"),
            };
            let holder = TransactionHolderVariants::from_transactions(vec![
                transaction.clone(),
                transaction2.clone(),
            ]);
            verify_that!(
                holder,
                eq(&TransactionHolderVariants::WithItems(
                    TransactionsHolderWithItems {
                        transactions: vec![vec![transaction], vec![], vec![transaction2],],
                        start_date: date!(2024 - 01 - 01),
                        end_date: date!(2024 - 01 - 03),
                    }
                ))
            )?;
            Ok(())
        }

        #[test]
        fn can_combine_holders() -> Result<()> {
            let transaction1 = BankTransaction {
                date: date!(2024 - 01 - 01),
                description: "Transaction 1".to_string(),
                amount: Money::from_major(100, iso::USD),
                other_side_account_number: NonEmptyString::new_str("123456789"),
            };
            let transaction2 = BankTransaction {
                date: date!(2024 - 01 - 03),
                description: "Transaction 2".to_string(),
                amount: Money::from_major(200, iso::USD),
                other_side_account_number: NonEmptyString::new_str("987654321"),
            };
            let holder1 = TransactionHolderVariants::from_transactions(vec![transaction1.clone()]);
            let holder2 = TransactionHolderVariants::from_transactions(vec![transaction2.clone()]);
            let combined_holder = combine_holders(holder1, holder2);
            verify_that!(
                combined_holder,
                eq(&TransactionHolderVariants::WithItems(
                    TransactionsHolderWithItems {
                        transactions: vec![vec![transaction1], vec![], vec![transaction2],],
                        start_date: date!(2024 - 01 - 01),
                        end_date: date!(2024 - 01 - 03),
                    }
                ))
            )?;
            Ok(())
        }

        #[test]
        fn second_holder_overrides_first_holder() -> Result<()> {
            let transaction1 = BankTransaction {
                date: date!(2024 - 01 - 01),
                description: "Transaction 1".to_string(),
                amount: Money::from_major(100, iso::USD),
                other_side_account_number: NonEmptyString::new_str("123456789"),
            };
            let transaction2 = BankTransaction {
                date: date!(2024 - 01 - 02),
                description: "Transaction 2".to_string(),
                amount: Money::from_major(200, iso::USD),
                other_side_account_number: NonEmptyString::new_str("987654321"),
            };
            // Will be lost because it's between transaction 2 and transaction 4
            let transaction3 = BankTransaction {
                date: date!(2024 - 01 - 03),
                description: "Transaction 3".to_string(),
                amount: Money::from_major(300, iso::USD),
                other_side_account_number: NonEmptyString::new_str("555555555"),
            };
            let transaction4 = BankTransaction {
                date: date!(2024 - 01 - 04),
                description: "Transaction 4".to_string(),
                amount: Money::from_major(400, iso::USD),
                other_side_account_number: NonEmptyString::new_str("666666666"),
            };

            let holder1 = TransactionHolderVariants::from_transactions(vec![
                transaction1.clone(),
                transaction3.clone(),
            ]);
            let holder2 = TransactionHolderVariants::from_transactions(vec![
                transaction2.clone(),
                transaction4.clone(),
            ]);
            let combined_holder = combine_holders(holder1, holder2);
            verify_that!(
                combined_holder,
                eq(&TransactionHolderVariants::WithItems(
                    TransactionsHolderWithItems {
                        transactions: vec![
                            vec![transaction1],
                            vec![transaction2],
                            vec![],
                            vec![transaction4],
                        ],
                        start_date: date!(2024 - 01 - 01),
                        end_date: date!(2024 - 01 - 04),
                    }
                ))
            )?;
            Ok(())
        }
    }
}
