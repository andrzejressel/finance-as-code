use finance_as_code_budget_core::Transaction;
use finance_as_code_budget_core::transformer::Transformer;

/// A simple Lua-based transaction transformer.
///
/// This transformer will allow users to write Lua scripts to transform
/// transactions. For now, it's a placeholder implementation that doesn't
/// execute any Lua code.
pub struct LuaTransformer {
    name: String,
}

impl LuaTransformer {
    /// Creates a new Lua transformer with the given name.
    ///
    /// # Example
    ///
    /// ```
    /// use finance_as_code_budget_transformer_lua::LuaTransformer;
    ///
    /// let transformer = LuaTransformer::new("my-lua-transformer");
    /// ```
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl Transformer for LuaTransformer {
    fn name(&self) -> &str {
        &self.name
    }

    fn transform(&self, transaction: Transaction) -> Vec<Transaction> {
        // For now, just pass through the transaction unchanged
        // TODO: Implement actual Lua transformation logic
        vec![transaction]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finance_as_code_budget_core::TagMap;
    use googletest::assert_that;
    use googletest::prelude::eq;

    #[test]
    fn transformer_returns_configured_name() {
        let transformer = LuaTransformer::new("test-lua-transformer");

        assert_that!(transformer.name(), eq("test-lua-transformer"));
    }

    #[test]
    fn transformer_passes_through_transaction_unchanged() {
        use chrono::NaiveDate;
        use rusty_money::Money;
        use rusty_money::iso::USD;
        use uuid::Uuid;

        let transformer = LuaTransformer::new("passthrough");

        let tx = finance_as_code_budget_core::Transaction {
            id: Uuid::parse_str("c65d0f0e-f7a4-4df4-a9e2-cd75ecdc77f6").unwrap(),
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            description: "Test transaction".to_string(),
            counterparty: "Test counterparty".to_string(),
            amount: Money::from_major(100, USD),
            other_side_account_number: None,
            tags: TagMap::new(),
        };

        let result = transformer.transform(tx);

        assert_that!(result.len(), eq(1));
        assert_that!(result[0].description.as_str(), eq("Test transaction"));
    }
}
