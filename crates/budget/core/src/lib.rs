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
use log::info;
use rootcause::Result;
use rootcause::prelude::ResultExt;
use rusty_money::Money;
use rusty_money::iso::Currency;
use serde::Serialize;
pub use transaction_mapper::map_bank_transaction_to_transaction;
pub use transactions_holder::TransactionHolder;

/// Runs the end-to-end pipeline: read from all sources, map to transactions,
/// apply all transformers in order, and write the result to all sinks.
///
/// Transformers are chained sequentially. Each transformer receives the output
/// produced by the previous one.
pub fn run(
    sources: Vec<Box<dyn readers::Source>>,
    transformers: Vec<Box<dyn transformer::Transformer>>,
    sinks: Vec<Box<dyn sink::Sink>>,
) -> rootcause::Result<()> {
    colog::init();

    let mut holders = Vec::new();

    for source in sources {
        info!("Running source {}", source.name());
        holders.push(
            source
                .read()
                .context_with(|| format!("Failed to read from source {}", source.name()))?,
        );
        info!("Finished running source {}", source.name());
    }

    let holder = TransactionHolder::combine_vec(holders);
    let bank_transactions = holder.into_transactions();
    let transactions = map_bank_transaction_to_transaction(bank_transactions)
        .context("Failed to map bank transactions to transactions")?;
    let transactions = apply_transformers(transactions, &transformers);

    for sink in sinks {
        info!("Writing to sink {}", sink.name());
        sink.write(&transactions)
            .context_with(|| format!("Failed to write to sink {}", sink.name()))?;
        info!("Finished writing to sink {}", sink.name());
    }

    Ok(())
}

fn apply_transformers(
    mut transactions: Vec<Transaction>,
    transformers: &[Box<dyn transformer::Transformer>],
) -> Vec<Transaction> {
    for transformer in transformers {
        info!("Running transformer {}", transformer.name());
        transactions = transactions
            .into_iter()
            .flat_map(|transaction| transformer.transform(transaction))
            .collect();
        info!("Finished running transformer {}", transformer.name());
    }

    transactions
}

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
    /// Not all banks support splitting description and counterparty, so this method joins them
    /// into a single string for easier matching in transformers.
    pub fn get_full_description(&self) -> String {
        join_non_empty(
            &[self.description.as_str(), self.counterparty.as_str()],
            " | ",
        )
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

#[cfg(test)]
mod tests {
    use super::Transaction;
    use super::*;
    use chrono::NaiveDate;
    use finance_as_code_utils_hmap::HMap;
    use googletest::assert_that;
    use googletest::prelude::eq;
    use rusty_money::Money;
    use rusty_money::iso::USD;
    use uuid::Uuid;

    use super::apply_transformers;

    #[test]
    fn get_full_description_joins_description_and_counterparty() {
        let tx = Transaction {
            id: Uuid::new_v4(),
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
            id: Uuid::new_v4(),
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            description: "Card payment".to_string(),
            counterparty: "   ".to_string(),
            amount: Money::from_major(10, USD),
            other_side_account_number: None,
            tags: HMap::new(),
        };

        assert_eq!(tx.get_full_description(), "Card payment");
    }

    #[test]
    fn apply_transformers_passes_changes_between_transformers() {
        let transformer1: Box<dyn transformer::Transformer> = Box::new(
            transformer::create_single_transaction_transformer("first", |mut tx| {
                tx.description = format!("{}-first", tx.description);
                vec![tx]
            }),
        );
        let transformer2: Box<dyn transformer::Transformer> = Box::new(
            transformer::create_single_transaction_transformer("second", |mut tx| {
                tx.description = format!("{}-second", tx.description);
                vec![tx]
            }),
        );

        let transaction = Transaction {
            id: Uuid::parse_str("7ea48a4b-6f97-4607-a298-15fcf5549df4").unwrap(),
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            description: "base".to_string(),
            counterparty: "Shop".to_string(),
            amount: Money::from_major(10, USD),
            other_side_account_number: None,
            tags: HMap::new(),
        };

        let result = apply_transformers(vec![transaction], &[transformer1, transformer2]);

        assert_that!(result.len(), eq(1));
        assert_that!(result[0].description.as_str(), eq("base-first-second"));
    }

    #[test]
    fn apply_transformers_applies_following_transformers_to_split_transactions() {
        let split_transformer: Box<dyn transformer::Transformer> = Box::new(
            transformer::create_single_transaction_transformer("split", |mut tx| {
                let split_part = Transaction {
                    id: Uuid::parse_str("9cedf634-840a-443d-9b51-95a36cd3bc6a").unwrap(),
                    date: tx.date,
                    description: format!("{}-part-2", tx.description),
                    counterparty: tx.counterparty.clone(),
                    amount: Money::from_major(4, USD),
                    other_side_account_number: None,
                    tags: HMap::new(),
                };
                tx.description = format!("{}-part-1", tx.description);
                tx.amount = Money::from_major(6, USD);

                vec![tx, split_part]
            }),
        );
        let normalize_transformer: Box<dyn transformer::Transformer> = Box::new(
            transformer::create_single_transaction_transformer("normalize", |mut tx| {
                tx.description = format!("{}-normalized", tx.description);
                vec![tx]
            }),
        );

        let transaction = Transaction {
            id: Uuid::parse_str("7ea48a4b-6f97-4607-a298-15fcf5549df4").unwrap(),
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            description: "base".to_string(),
            counterparty: "Shop".to_string(),
            amount: Money::from_major(10, USD),
            other_side_account_number: None,
            tags: HMap::new(),
        };

        let result = apply_transformers(
            vec![transaction],
            &[split_transformer, normalize_transformer],
        );

        assert_that!(result.len(), eq(2));
        assert_that!(result[0].description.as_str(), eq("base-part-1-normalized"));
        assert_that!(result[1].description.as_str(), eq("base-part-2-normalized"));
    }
}
