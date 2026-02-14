pub mod model;
pub mod readers;
pub mod sink;
mod transaction_mapper;
mod transactions_holder;

use bon::Builder;
use chrono::NaiveDate;
use nonempty_collections::*;
use rootcause::Result;
use rusty_money::Money;
use rusty_money::iso::Currency;
use serde::Serialize;

pub use transaction_mapper::map_bank_transaction_to_transaction;
pub use transactions_holder::TransactionHolder;

#[derive(Builder, Clone, Debug, Serialize, PartialEq, Hash)]
pub struct BankTransaction {
    #[builder(into)]
    pub date: NaiveDate,
    #[builder(into)]
    pub description: String,
    #[builder(into)]
    pub amount: Money<'static, Currency>,
    pub other_side_account_number: Option<NonEmptyString>, // TODO: Make it a struct
}

#[derive(Builder, Clone, Debug, Serialize, PartialEq)]
pub struct Transaction {
    pub id: uuid::Uuid,
    #[builder(into)]
    pub date: NaiveDate,
    #[builder(into)]
    pub description: String,
    #[builder(into)]
    pub amount: Money<'static, Currency>,
    pub other_side_account_number: Option<NonEmptyString>, // TODO: Make it a struct
}

impl Transaction {
    pub fn from_bank_transaction(bank_tx: BankTransaction) -> Self {
        Transaction {
            id: uuid::Uuid::new_v4(),
            date: bank_tx.date,
            description: bank_tx.description,
            amount: bank_tx.amount,
            other_side_account_number: bank_tx.other_side_account_number,
        }
    }
}

#[cfg_attr(test, mockall::automock)]
pub trait FileReader {
    fn name(&self) -> &str;
    fn read_file(&self, file_data: &[u8]) -> Result<TransactionHolder>;
}

#[derive(Clone, Debug, Serialize, PartialEq, Hash)]
pub struct NonEmptyString(String);

impl NonEmptyString {
    pub fn new(s: String) -> Option<Self> {
        if s.trim().is_empty() {
            None
        } else {
            Some(NonEmptyString(s))
        }
    }

    pub fn new_str(s: &str) -> Option<Self> {
        NonEmptyString::new(s.to_string())
    }
}

impl Into<String> for NonEmptyString {
    fn into(self) -> String {
        self.0
    }
}

impl AsRef<str> for NonEmptyString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
