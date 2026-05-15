#![doc = include_str!("../../../root/src/docs/squasher.md")]

use bon::Builder;
use chrono::NaiveDate;
use finance_as_code_budget_core::TagMap;
use finance_as_code_budget_core::Transaction;
use finance_as_code_budget_core::transformer::Transformer;
use log::warn;
use rust_decimal::Decimal;
use rusty_money::FormattableCurrency;
use rusty_money::Money;
use uuid::Uuid;

#[derive(Builder)]
pub struct SquasherConfig {
    #[builder(into)]
    name: String,
    #[builder(into)]
    from_date: NaiveDate,
    #[builder(into)]
    to_date: NaiveDate,
    #[builder(into)]
    squashed_name: String,
}

pub fn create_squasher(config: SquasherConfig) -> impl Transformer {
    SquasherTransformer {
        name: config.name,
        from_date: config.from_date,
        to_date: config.to_date,
        squashed_name: config.squashed_name,
    }
}

struct SquasherTransformer {
    name: String,
    from_date: NaiveDate,
    to_date: NaiveDate,
    squashed_name: String,
}

impl Transformer for SquasherTransformer {
    fn name(&self) -> &str {
        &self.name
    }

    fn transform(&self, transactions: Vec<Transaction>) -> Vec<Transaction> {
        if self.from_date > self.to_date {
            warn!(
                "Squasher '{}' has invalid date range: {} > {}. Returning input unchanged.",
                self.name, self.from_date, self.to_date
            );
            return transactions;
        }

        if contains_mixed_currencies_in_range(&transactions, self.from_date, self.to_date) {
            warn!(
                "Squasher '{}' matched multiple currencies in range {}..={}. Returning input unchanged.",
                self.name, self.from_date, self.to_date
            );
            return transactions;
        }

        let (matched, mut remaining): (Vec<Transaction>, Vec<Transaction>) = transactions
            .into_iter()
            .partition(|tx| tx.date >= self.from_date && tx.date <= self.to_date);

        if matched.is_empty() {
            return remaining;
        }

        let currency = matched
            .first()
            .expect("matched is non-empty")
            .amount
            .currency();
        let amount_sum = matched
            .iter()
            .fold(Decimal::ZERO, |acc, tx| acc + *tx.amount.amount());

        remaining.push(Transaction {
            id: Uuid::new_v4(),
            date: self.to_date,
            description: self.squashed_name.clone(),
            counterparty: "SQUASHED".to_string(),
            amount: Money::from_decimal(amount_sum, currency),
            other_side_account_number: None,
            tags: TagMap::new(),
        });

        remaining.sort_by_key(|tx| tx.date);
        remaining
    }
}

fn contains_mixed_currencies_in_range(
    transactions: &[Transaction],
    from_date: NaiveDate,
    to_date: NaiveDate,
) -> bool {
    let mut first_currency_code: Option<&str> = None;

    for tx in transactions {
        if tx.date < from_date || tx.date > to_date {
            continue;
        }

        let code = tx.amount.currency().code();

        match first_currency_code {
            Some(first) if first != code => return true,
            Some(_) => {}
            None => first_currency_code = Some(code),
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use finance_as_code_budget_core::transformer::Transformer;
    use googletest::assert_that;
    use googletest::prelude::eq;
    use rusty_money::iso::EUR;
    use rusty_money::iso::USD;

    fn tx(date: NaiveDate, description: &str, amount_major: i64) -> Transaction {
        Transaction {
            id: Uuid::new_v4(),
            date,
            description: description.to_string(),
            counterparty: "Counterparty".to_string(),
            amount: Money::from_major(amount_major, USD),
            other_side_account_number: None,
            tags: TagMap::new(),
        }
    }

    #[test]
    fn squashes_all_transactions_in_inclusive_range() {
        let transformer = create_squasher(
            SquasherConfig::builder()
                .name("squash-january")
                .from_date(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap())
                .to_date(NaiveDate::from_ymd_opt(2025, 1, 31).unwrap())
                .squashed_name("January summary")
                .build(),
        );

        let result = transformer.transform(vec![
            tx(
                NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
                "outside-before",
                20,
            ),
            tx(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(), "inside-a", 10),
            tx(
                NaiveDate::from_ymd_opt(2025, 1, 31).unwrap(),
                "inside-b",
                -3,
            ),
            tx(
                NaiveDate::from_ymd_opt(2025, 2, 1).unwrap(),
                "outside-after",
                8,
            ),
        ]);

        assert_that!(result.len(), eq(3));
        assert_that!(result[0].description.as_str(), eq("outside-before"));
        assert_that!(result[1].description.as_str(), eq("January summary"));
        assert_that!(result[1].amount.amount().to_string(), eq("7"));
        assert_that!(result[2].description.as_str(), eq("outside-after"));
    }

    #[test]
    fn keeps_transactions_unchanged_when_no_transactions_match() {
        let transformer = create_squasher(
            SquasherConfig::builder()
                .name("no-match")
                .from_date(NaiveDate::from_ymd_opt(2025, 3, 1).unwrap())
                .to_date(NaiveDate::from_ymd_opt(2025, 3, 31).unwrap())
                .squashed_name("March summary")
                .build(),
        );

        let original = vec![
            tx(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(), "a", 10),
            tx(NaiveDate::from_ymd_opt(2025, 2, 1).unwrap(), "b", 20),
        ];

        let result = transformer.transform(original);

        assert_that!(result.len(), eq(2));
        assert_that!(result[0].description.as_str(), eq("a"));
        assert_that!(result[1].description.as_str(), eq("b"));
    }

    #[test]
    fn returns_original_transactions_when_range_contains_mixed_currencies() {
        let transformer = create_squasher(
            SquasherConfig::builder()
                .name("mixed-currency")
                .from_date(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap())
                .to_date(NaiveDate::from_ymd_opt(2025, 1, 31).unwrap())
                .squashed_name("summary")
                .build(),
        );

        let tx_usd = tx(NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(), "usd", 10);
        let tx_eur = Transaction {
            id: Uuid::new_v4(),
            date: NaiveDate::from_ymd_opt(2025, 1, 20).unwrap(),
            description: "eur".to_string(),
            counterparty: "Counterparty".to_string(),
            amount: Money::from_major(5, EUR),
            other_side_account_number: None,
            tags: TagMap::new(),
        };

        let result = transformer.transform(vec![tx_usd, tx_eur]);

        assert_that!(result.len(), eq(2));
        assert_that!(result[0].description.as_str(), eq("usd"));
        assert_that!(result[1].description.as_str(), eq("eur"));
    }

    #[test]
    fn returns_original_transactions_when_date_range_is_invalid() {
        let transformer = create_squasher(
            SquasherConfig::builder()
                .name("invalid-range")
                .from_date(NaiveDate::from_ymd_opt(2025, 2, 1).unwrap())
                .to_date(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap())
                .squashed_name("summary")
                .build(),
        );

        let result = transformer.transform(vec![tx(
            NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
            "a",
            10,
        )]);

        assert_that!(result.len(), eq(1));
        assert_that!(result[0].description.as_str(), eq("a"));
    }
}
