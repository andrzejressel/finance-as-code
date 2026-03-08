#![doc = include_str!("../../../root/src/docs/lua.md")]

use finance_as_code_budget_core::Transaction;
use finance_as_code_budget_core::transformer::Transformer;
use mlua::Lua;
use mlua::UserData;
use mlua::UserDataFields;
use mlua::UserDataMethods;
use std::cell::RefCell;

/// Trait for executing Lua scripts on transactions.
///
/// This trait allows mocking Lua execution in tests.
#[cfg_attr(any(test, feature = "mock"), mockall::automock)]
pub trait LuaExecutor {
    /// Executes a Lua script on a transaction and returns the transformed transactions.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the transformer (used for error reporting)
    /// * `script` - The Lua script to execute
    /// * `transaction` - The transaction to transform
    ///
    /// # Returns
    ///
    /// A vector of transformed transactions. May be empty if the transaction was dropped.
    fn execute(&self, name: &str, script: &str, transaction: Transaction) -> Vec<Transaction>;
}

/// Default Lua executor that uses the mlua crate.
pub struct DefaultLuaExecutor;

impl LuaExecutor for DefaultLuaExecutor {
    fn execute(&self, name: &str, script: &str, transaction: Transaction) -> Vec<Transaction> {
        let lua = Lua::new();

        let result: mlua::Result<Vec<Transaction>> = (|| {
            // We put the transaction in a global variable wrapped in LuaTransaction.
            let lua_tx = LuaTransaction {
                inner: RefCell::new(transaction),
            };
            lua.globals().set("transaction", lua_tx)?;

            let result_multivalue: mlua::MultiValue = lua.load(script).eval()?;

            if result_multivalue.is_empty() {
                // Script returned nothing, use global transaction
                let ud: mlua::AnyUserData = lua.globals().get("transaction")?;
                let lt = ud.take::<LuaTransaction>()?;
                return Ok(vec![lt.inner.into_inner()]);
            }

            let result_value = result_multivalue.front().unwrap();

            if result_value.is_nil() {
                return Ok(vec![]);
            }

            if let Some(ud) = result_value.as_userdata() {
                let lt = ud.take::<LuaTransaction>()?;
                Ok(vec![lt.inner.into_inner()])
            } else if let Some(table) = result_value.as_table() {
                let mut txs = Vec::new();
                for res in table.sequence_values::<mlua::Value>() {
                    let val = res?;
                    if let Some(ud) = val.as_userdata() {
                        let lt = ud.take::<LuaTransaction>()?;
                        txs.push(lt.inner.into_inner());
                    }
                }
                Ok(txs)
            } else {
                let ud: mlua::AnyUserData = lua.globals().get("transaction")?;
                let lt = ud.take::<LuaTransaction>()?;
                Ok(vec![lt.inner.into_inner()])
            }
        })();

        match result {
            Ok(transactions) => transactions,
            Err(e) => {
                eprintln!("Lua transformation failed for {}: {:?}", name, e);
                vec![]
            }
        }
    }
}

/// A transformer that executes Lua scripts on transactions.
pub struct LuaTransformer {
    name: String,
    script: String,
}

/// Wrapper for Transaction to expose it to Lua.
struct LuaTransaction {
    inner: RefCell<Transaction>,
}

impl UserData for LuaTransaction {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("id", |_, this| {
            let borrow = this.inner.borrow();
            Ok(borrow.id.to_string())
        });
        fields.add_field_method_get("date", |_, this| {
            let borrow = this.inner.borrow();
            Ok(borrow.date.to_string())
        });
        fields.add_field_method_get("description", |_, this| {
            let borrow = this.inner.borrow();
            Ok(borrow.description.clone())
        });
        fields.add_field_method_set("description", |_, this, val: String| {
            let mut borrow = this.inner.borrow_mut();
            borrow.description = val;
            Ok(())
        });
        fields.add_field_method_get("counterparty", |_, this| {
            let borrow = this.inner.borrow();
            Ok(borrow.counterparty.clone())
        });
        fields.add_field_method_set("counterparty", |_, this, val: String| {
            let mut borrow = this.inner.borrow_mut();
            borrow.counterparty = val;
            Ok(())
        });
        fields.add_field_method_get("amount", |_, this| {
            let borrow = this.inner.borrow();
            Ok(borrow.amount.amount().to_string())
        });
        fields.add_field_method_get("currency", |_, this| {
            let borrow = this.inner.borrow();
            Ok(borrow.amount.currency().to_string())
        });
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get_tag", |_, this, key: String| {
            let borrow = this.inner.borrow();
            Ok(borrow.tags.get::<String>(&key).cloned())
        });
        methods.add_method_mut("set_tag", |_, this, (key, value): (String, String)| {
            let mut borrow = this.inner.borrow_mut();
            borrow.tags.insert(key, value);
            Ok(())
        });

        // Since we can't clone HMap, split() will create a new transaction with same
        // fields but NO tags. This is a limitation of the current HMap design
        // if we don't want to change it.
        methods.add_method("split", |_, this, ()| {
            let borrow = this.inner.borrow();

            let new_tx = Transaction {
                id: uuid::Uuid::new_v4(), // Give it a new ID
                date: borrow.date,
                description: borrow.description.clone(),
                counterparty: borrow.counterparty.clone(),
                amount: borrow.amount,
                other_side_account_number: borrow.other_side_account_number.clone(),
                tags: finance_as_code_utils_hmap::HMap::new(),
            };

            Ok(LuaTransaction {
                inner: RefCell::new(new_tx),
            })
        });
    }
}

impl LuaTransformer {
    /// Creates a new Lua transformer.
    pub fn new(name: impl Into<String>, script: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            script: script.into(),
        }
    }
}

impl Transformer for LuaTransformer {
    fn name(&self) -> &str {
        &self.name
    }

    fn transform(&self, transaction: Transaction) -> Vec<Transaction> {
        let executor = DefaultLuaExecutor;
        executor.execute(&self.name, &self.script, transaction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use finance_as_code_budget_core::TagMap;
    use googletest::assert_that;
    use googletest::prelude::eq;
    use googletest::prelude::some;
    use rusty_money::Money;
    use rusty_money::iso::USD;
    use uuid::Uuid;

    fn create_test_transaction() -> Transaction {
        Transaction {
            id: Uuid::parse_str("c65d0f0e-f7a4-4df4-a9e2-cd75ecdc77f6").unwrap(),
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            description: "Test transaction".to_string(),
            counterparty: "Test counterparty".to_string(),
            amount: Money::from_major(100, USD),
            other_side_account_number: None,
            tags: TagMap::new(),
        }
    }

    #[test]
    fn transformer_returns_configured_name() {
        let transformer = LuaTransformer::new("test-lua-transformer", "return transaction");

        assert_that!(transformer.name(), eq("test-lua-transformer"));
    }

    #[test]
    fn transformer_modifies_transaction() {
        let script = r#"
            transaction.description = transaction.description .. " (modified)"
            transaction:set_tag("source", "lua")
        "#;
        let transformer = LuaTransformer::new("test", script);
        let tx = create_test_transaction();

        let result = transformer.transform(tx);

        assert_that!(result.len(), eq(1));
        assert_that!(
            result[0].description.as_str(),
            eq("Test transaction (modified)")
        );
        assert_that!(
            result[0].tags.get::<String>(&"source".to_string()),
            some(eq(&"lua".to_string()))
        );
    }

    #[test]
    fn transformer_can_return_multiple_transactions() {
        let script = r#"
            local tx2 = transaction:split()
            tx2.description = "Second"
            return {transaction, tx2}
        "#;
        let transformer = LuaTransformer::new("test", script);
        let tx = create_test_transaction();

        let result = transformer.transform(tx);

        assert_that!(result.len(), eq(2));
        assert_that!(result[0].description.as_str(), eq("Test transaction"));
        assert_that!(result[1].description.as_str(), eq("Second"));
    }

    #[test]
    fn transformer_can_drop_transaction() {
        let script = "return nil";
        let transformer = LuaTransformer::new("test", script);
        let tx = create_test_transaction();

        let result = transformer.transform(tx);

        assert_that!(result.len(), eq(0));
    }
}
