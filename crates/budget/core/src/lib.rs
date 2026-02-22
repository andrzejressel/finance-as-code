pub mod model;
pub mod readers;
pub mod sink;
mod transaction_mapper;
mod transactions_holder;
pub type TagMap = HMap<String>;
pub mod transformer;

use crate::model::join_non_empty;
use bon::Builder;
use chrono::NaiveDate;
use finance_as_code_utils_hmap::HMap;
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
    pub counterparty: String,
    #[builder(into)]
    pub amount: Money<'static, Currency>,
    pub other_side_account_number: Option<NonEmptyString>, // TODO: Make it a struct
}

#[derive(Builder, Debug, PartialEq)]
pub struct Transaction {
    pub id: uuid::Uuid,
    #[builder(into)]
    pub date: NaiveDate,
    #[builder(into)]
    pub description: String,
    #[builder(into)]
    pub counterparty: String,
    #[builder(into)]
    pub amount: Money<'static, Currency>,
    pub other_side_account_number: Option<NonEmptyString>, // TODO: Make it a struct
    pub tags: TagMap,
}

impl Transaction {
    pub fn get_full_description(&self) -> String {
        join_non_empty(
            &[self.description.as_str(), self.counterparty.as_str()],
            " | ",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Transaction;
    use chrono::NaiveDate;
    use finance_as_code_utils_hmap::HMap;
    use rusty_money::Money;
    use rusty_money::iso::USD;

    #[test]
    fn get_full_description_joins_description_and_counterparty() {
        let tx = Transaction {
            id: uuid::Uuid::new_v4(),
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            description: "Card payment".to_string(),
            counterparty: "Coffee Shop".to_string(),
            amount: Money::from_major(10, USD),
            other_side_account_number: None,
            tags: HMap::new(),
        };

        assert_eq!(tx.get_full_description(), "Card payment | Coffee Shop");
    }

    #[test]
    fn get_full_description_skips_empty_parts() {
        let tx = Transaction {
            id: uuid::Uuid::new_v4(),
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            description: "Card payment".to_string(),
            counterparty: "   ".to_string(),
            amount: Money::from_major(10, USD),
            other_side_account_number: None,
            tags: HMap::new(),
        };

        assert_eq!(tx.get_full_description(), "Card payment");
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

impl From<NonEmptyString> for String {
    fn from(val: NonEmptyString) -> Self {
        val.0
    }
}

impl AsRef<str> for NonEmptyString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
