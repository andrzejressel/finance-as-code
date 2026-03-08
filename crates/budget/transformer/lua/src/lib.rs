use finance_as_code_budget_core::Transaction;
use finance_as_code_budget_core::transformer::Transformer;
use mlua::Lua;
use mlua::UserData;
use mlua::UserDataFields;
use mlua::UserDataMethods;
use std::cell::RefCell;

/// A Lua-based transaction transformer.
pub struct LuaTransformer {
    name: String,
    script: String,
}

struct LuaTransaction {
    // We use RefCell to allow mutation from Lua, and Option to allow moving the transaction out.
    inner: RefCell<Option<Transaction>>,
}

impl UserData for LuaTransaction {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("id", |_, this| {
            let borrow = this.inner.borrow();
            let tx = borrow.as_ref().ok_or_else(|| mlua::Error::runtime("Transaction already moved out"))?;
            Ok(tx.id.to_string())
        });
        fields.add_field_method_get("date", |_, this| {
            let borrow = this.inner.borrow();
            let tx = borrow.as_ref().ok_or_else(|| mlua::Error::runtime("Transaction already moved out"))?;
            Ok(tx.date.to_string())
        });
        fields.add_field_method_get("description", |_, this| {
            let borrow = this.inner.borrow();
            let tx = borrow.as_ref().ok_or_else(|| mlua::Error::runtime("Transaction already moved out"))?;
            Ok(tx.description.clone())
        });
        fields.add_field_method_set("description", |_, this, val: String| {
            let mut borrow = this.inner.borrow_mut();
            let tx = borrow.as_mut().ok_or_else(|| mlua::Error::runtime("Transaction already moved out"))?;
            tx.description = val;
            Ok(())
        });
        fields.add_field_method_get("counterparty", |_, this| {
            let borrow = this.inner.borrow();
            let tx = borrow.as_ref().ok_or_else(|| mlua::Error::runtime("Transaction already moved out"))?;
            Ok(tx.counterparty.clone())
        });
        fields.add_field_method_set("counterparty", |_, this, val: String| {
            let mut borrow = this.inner.borrow_mut();
            let tx = borrow.as_mut().ok_or_else(|| mlua::Error::runtime("Transaction already moved out"))?;
            tx.counterparty = val;
            Ok(())
        });
        fields.add_field_method_get("amount", |_, this| {
            let borrow = this.inner.borrow();
            let tx = borrow.as_ref().ok_or_else(|| mlua::Error::runtime("Transaction already moved out"))?;
            Ok(tx.amount.amount().to_string())
        });
        fields.add_field_method_get("currency", |_, this| {
            let borrow = this.inner.borrow();
            let tx = borrow.as_ref().ok_or_else(|| mlua::Error::runtime("Transaction already moved out"))?;
            Ok(tx.amount.currency().to_string())
        });
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get_tag", |_, this, key: String| {
            let borrow = this.inner.borrow();
            let tx = borrow.as_ref().ok_or_else(|| mlua::Error::runtime("Transaction already moved out"))?;
            Ok(tx.tags.get::<String>(&key).cloned())
        });
        methods.add_method_mut("set_tag", |_, this, (key, value): (String, String)| {
            let mut borrow = this.inner.borrow_mut();
            let tx = borrow.as_mut().ok_or_else(|| mlua::Error::runtime("Transaction already moved out"))?;
            tx.tags.insert(key, value);
            Ok(())
        });
        
        // Since we can't clone HMap, split() will create a new transaction with same fields but NO tags.
        // This is a limitation of the current HMap design if we don't want to change it.
        methods.add_method("split", |_, this, ()| {
            let borrow = this.inner.borrow();
            let tx = borrow.as_ref().ok_or_else(|| mlua::Error::runtime("Transaction already moved out"))?;
            
            let new_tx = Transaction {
                id: uuid::Uuid::new_v4(), // Give it a new ID
                date: tx.date,
                description: tx.description.clone(),
                counterparty: tx.counterparty.clone(),
                amount: tx.amount.clone(),
                other_side_account_number: tx.other_side_account_number.clone(),
                tags: finance_as_code_utils_hmap::HMap::new(),
            };
            
            Ok(LuaTransaction { inner: RefCell::new(Some(new_tx)) })
        });
    }
}

impl LuaTransformer {
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
        let lua = Lua::new();

        let result: mlua::Result<Vec<Transaction>> = (|| {
            // We need to keep a reference to the primary transaction to take it back later if needed.
            // But LuaUserData takes ownership. 
            // In mlua 0.11, we can't easily share ownership of UserData between Rust and Lua
            // without Arc, but Arc<T> only implements UserData if T does and we'd need to wrap it.
            // Wait, mlua provides a way to use handle? No.
            
            // Let's use a trick: we put it in a global table.
            let lua_tx = LuaTransaction { inner: RefCell::new(Some(transaction)) };
            lua.globals().set("transaction", lua_tx)?;

            let result_multivalue: mlua::MultiValue = lua.load(&self.script).eval()?;

            if result_multivalue.is_empty() {
                // Script returned nothing, use global transaction
                let ud: mlua::AnyUserData = lua.globals().get("transaction")?;
                let lt = ud.borrow::<LuaTransaction>()?;
                return Ok(vec![lt.inner.borrow_mut().take().unwrap()]);
            }

            let result_value = result_multivalue.get(0).unwrap();

            if result_value.is_nil() {
                return Ok(vec![]);
            }

            if let Some(ud) = result_value.as_userdata() {
                let lt = ud.borrow::<LuaTransaction>()?;
                let mut borrow = lt.inner.borrow_mut();
                Ok(vec![borrow.take().ok_or_else(|| mlua::Error::runtime("Transaction already moved out"))?])
            } else if let Some(table) = result_value.as_table() {
                let mut txs = Vec::new();
                for res in table.sequence_values::<mlua::Value>() {
                    let val = res?;
                    if let Some(ud) = val.as_userdata() {
                        let lt = ud.borrow::<LuaTransaction>()?;
                        let mut borrow = lt.inner.borrow_mut();
                        txs.push(borrow.take().ok_or_else(|| mlua::Error::runtime("Transaction already moved out"))?);
                    }
                }
                Ok(txs)
            } else {
                let ud: mlua::AnyUserData = lua.globals().get("transaction")?;
                let lt = ud.borrow::<LuaTransaction>()?;
                Ok(vec![lt.inner.borrow_mut().take().unwrap()])
            }
        })();

        match result {
            Ok(transactions) => transactions,
            Err(e) => {
                eprintln!("Lua transformation failed for {}: {:?}", self.name, e);
                // We can't easily return original transaction here because it was moved.
                // This is a trade-off for not having Clone.
                vec![] 
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finance_as_code_budget_core::TagMap;
    use googletest::assert_that;
    use googletest::prelude::eq;
    use googletest::prelude::some;
    use rusty_money::Money;
    use rusty_money::iso::USD;
    use uuid::Uuid;
    use chrono::NaiveDate;

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
        assert_that!(result[0].description.as_str(), eq("Test transaction (modified)"));
        assert_that!(result[0].tags.get::<String>(&"source".to_string()), some(eq(&"lua".to_string())));
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
