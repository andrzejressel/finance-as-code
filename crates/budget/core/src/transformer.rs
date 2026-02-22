use crate::Transaction;

/// [Transformer] allows user to modify transactions between reading and
/// writing. It can be used to split transactions, merge them, change
/// descriptions, etc. It is a powerful tool to customize the data before it is
/// sent to the sink.
pub trait Transformer {
    fn name(&self) -> &str;

    fn transform(&self, transaction: Transaction) -> Vec<Transaction>;
}

struct SimpleTransactionTransformer {
    name: String,
    transform_fn: Box<dyn Fn(Transaction) -> Vec<Transaction>>,
}

impl Transformer for SimpleTransactionTransformer {
    fn name(&self) -> &str {
        &self.name
    }

    fn transform(&self, transaction: Transaction) -> Vec<Transaction> {
        (self.transform_fn)(transaction)
    }
}

/// Creates a simple transformer that applies the provided transformation
/// function to each transaction. The transformation function takes a single
/// transaction and returns a vector of transactions, allowing for changing
/// transaction data or splitting. The name parameter is used to identify the
/// transformer in logs and error messages.
pub fn create_single_transaction_transformer<F>(name: &str, transform_fn: F) -> impl Transformer
where
    F: Fn(Transaction) -> Vec<Transaction> + 'static,
{
    SimpleTransactionTransformer {
        name: name.to_string(),
        transform_fn: Box::new(transform_fn),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use googletest::assert_that;
    use googletest::prelude::eq;
    use rusty_money::Money;
    use rusty_money::iso::USD;
    use uuid::Uuid;

    #[test]
    fn transformer_returns_configured_name() {
        let transformer = create_single_transaction_transformer("test-transformer", |tx| vec![tx]);

        assert_that!(transformer.name(), eq("test-transformer"));
    }

    #[test]
    fn transformer_uses_provided_transform_function() {
        let transformer = create_single_transaction_transformer("append", |mut tx| {
            tx.description = format!("{} (updated)", tx.description);

            let generated_tx = Transaction {
                id: Uuid::parse_str("6ef8407f-8159-4f85-be0e-c6fc5d5f3f0b").unwrap(),
                date: NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(),
                description: "Synthetic fee".to_string(),
                counterparty: "Budget System".to_string(),
                amount: Money::from_major(1, USD),
                other_side_account_number: None,
            };

            vec![tx, generated_tx]
        });

        let tx = Transaction {
            id: Uuid::parse_str("c65d0f0e-f7a4-4df4-a9e2-cd75ecdc77f6").unwrap(),
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            description: "Groceries".to_string(),
            counterparty: "Local Shop".to_string(),
            amount: Money::from_major(25, USD),
            other_side_account_number: None,
        };

        let result = transformer.transform(tx);

        assert_that!(result.len(), eq(2));
        assert_that!(result[0].description.as_str(), eq("Groceries (updated)"));
        assert_that!(result[1].description.as_str(), eq("Synthetic fee"));
    }
}
