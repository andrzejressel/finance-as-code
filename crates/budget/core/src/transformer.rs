use crate::Transaction;

/// [Transformer] allows user to modify transactions between reading and
/// writing. It can be used to split transactions, merge them, change
/// descriptions, etc. It is a powerful tool to customize the data before it is
/// sent to the sink.
pub trait Transformer {
    fn name(&self) -> &str;

    fn transform(&self, transactions: Vec<Transaction>) -> Vec<Transaction>;
}

/// [SingleTransactionTransformer] allows users to define transformation logic
/// for one transaction at a time. It can be automatically used as a batch
/// [Transformer].
pub trait SingleTransactionTransformer {
    fn name(&self) -> &str;

    fn transform_single(&self, transaction: Transaction) -> Vec<Transaction>;
}

impl<T> Transformer for T
where
    T: SingleTransactionTransformer,
{
    fn name(&self) -> &str {
        SingleTransactionTransformer::name(self)
    }

    fn transform(&self, transactions: Vec<Transaction>) -> Vec<Transaction> {
        transactions
            .into_iter()
            .flat_map(|transaction| self.transform_single(transaction))
            .collect()
    }
}

struct SimpleTransactionTransformer {
    name: String,
    transform_fn: Box<dyn Fn(Transaction) -> Vec<Transaction>>,
}

impl SingleTransactionTransformer for SimpleTransactionTransformer {
    fn name(&self) -> &str {
        &self.name
    }

    fn transform_single(&self, transaction: Transaction) -> Vec<Transaction> {
        (self.transform_fn)(transaction)
    }
}

/// Creates a simple transformer that applies the provided transformation
/// function to each transaction. The transformation function takes a single
/// transaction and returns a vector of transactions, allowing for changing
/// transaction data or splitting. The name parameter is used to identify the
/// transformer in logs and error messages.
pub fn create_single_transaction_transformer<F>(
    name: &str,
    transform_fn: F,
) -> impl SingleTransactionTransformer
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
    use crate::TagMap;
    use chrono::NaiveDate;
    use googletest::assert_that;
    use googletest::prelude::eq;
    use rusty_money::Money;
    use rusty_money::iso::USD;
    use uuid::Uuid;

    #[test]
    fn transformer_returns_configured_name() {
        let transformer = create_single_transaction_transformer("test-transformer", |tx| vec![tx]);

        assert_that!(
            SingleTransactionTransformer::name(&transformer),
            eq("test-transformer")
        );
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
                tags: TagMap::new(),
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
            tags: TagMap::new(),
        };

        let result = transformer.transform_single(tx);

        assert_that!(result.len(), eq(2));
        assert_that!(result[0].description.as_str(), eq("Groceries (updated)"));
        assert_that!(result[1].description.as_str(), eq("Synthetic fee"));
    }
}
