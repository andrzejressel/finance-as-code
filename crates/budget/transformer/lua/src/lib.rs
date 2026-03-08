#![doc = include_str!("../../../root/src/docs/lua.md")]

use finance_as_code_budget_core::Transaction;
use finance_as_code_budget_core::transformer::Transformer;
use mlua::Lua;
use mlua::LuaOptions;
use mlua::StdLib;
use mlua::UserData;
use mlua::UserDataFields;
use mlua::UserDataMethods;
use std::cell::RefCell;

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
        // Create a safe Lua environment without IO, OS, PACKAGE, DEBUG, and COROUTINE libraries
        // This prevents scripts from performing file I/O, executing system commands,
        // loading packages, using debug introspection, or creating coroutines.
        let safe_libs = StdLib::TABLE
            | StdLib::STRING
            | StdLib::MATH
            | StdLib::UTF8;

        let lua = Lua::new_with(safe_libs, LuaOptions::default())
            .expect("Failed to create safe Lua environment");

        let result: mlua::Result<Vec<Transaction>> = (|| {
            // We put the transaction in a global variable wrapped in LuaTransaction.
            let lua_tx = LuaTransaction {
                inner: RefCell::new(transaction),
            };
            lua.globals().set("transaction", lua_tx)?;

            let result_multivalue: mlua::MultiValue = lua.load(&self.script).eval()?;

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
    use chrono::NaiveDate;
    use finance_as_code_budget_core::TagMap;
    use googletest::prelude::*;
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

    /// Helper function for testing that exposes the Result type.
    /// This allows tests to verify that errors are actually thrown.
    fn transform_with_result(
        transformer: &LuaTransformer,
        transaction: Transaction,
    ) -> mlua::Result<Vec<Transaction>> {
        let safe_libs = StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8;

        let lua = Lua::new_with(safe_libs, LuaOptions::default())
            .expect("Failed to create safe Lua environment");

        let lua_tx = LuaTransaction {
            inner: RefCell::new(transaction),
        };
        lua.globals().set("transaction", lua_tx)?;

        let result_multivalue: mlua::MultiValue = lua.load(&transformer.script).eval()?;

        if result_multivalue.is_empty() {
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

    #[test]
    fn transformer_blocks_io_open() {
        let script = r#"
            local f = io.open("/etc/passwd", "r")
            if f then
                f:close()
            end
        "#;
        let transformer = LuaTransformer::new("test", script);
        let tx = create_test_transaction();

        let result = transform_with_result(&transformer, tx);

        // Should get an error about 'io' being undefined
        assert_that!(result.is_err(), eq(true));
        let error_msg = result.unwrap_err().to_string();
        assert_that!(error_msg, contains_substring("io"));
    }

    #[test]
    fn transformer_blocks_io_write() {
        let script = r#"
            local f = io.open("/tmp/test.txt", "w")
            if f then
                f:write("malicious content")
                f:close()
            end
        "#;
        let transformer = LuaTransformer::new("test", script);
        let tx = create_test_transaction();

        let result = transform_with_result(&transformer, tx);

        // Should get an error about 'io' being undefined
        assert_that!(result.is_err(), eq(true));
        let error_msg = result.unwrap_err().to_string();
        assert_that!(error_msg, contains_substring("io"));
    }

    #[test]
    fn transformer_blocks_io_popen() {
        let script = r#"
            local handle = io.popen("ls")
            if handle then
                handle:close()
            end
        "#;
        let transformer = LuaTransformer::new("test", script);
        let tx = create_test_transaction();

        let result = transform_with_result(&transformer, tx);

        // Should get an error about 'io' being undefined
        assert_that!(result.is_err(), eq(true));
        let error_msg = result.unwrap_err().to_string();
        assert_that!(error_msg, contains_substring("io"));
    }

    #[test]
    fn transformer_blocks_os_execute() {
        let script = r#"
            os.execute("echo 'malicious command'")
        "#;
        let transformer = LuaTransformer::new("test", script);
        let tx = create_test_transaction();

        let result = transform_with_result(&transformer, tx);

        // Should get an error about 'os' being undefined
        assert_that!(result.is_err(), eq(true));
        let error_msg = result.unwrap_err().to_string();
        assert_that!(error_msg, contains_substring("os"));
    }

    #[test]
    fn transformer_blocks_os_remove() {
        let script = r#"
            os.remove("/tmp/test.txt")
        "#;
        let transformer = LuaTransformer::new("test", script);
        let tx = create_test_transaction();

        let result = transform_with_result(&transformer, tx);

        // Should get an error about 'os' being undefined
        assert_that!(result.is_err(), eq(true));
        let error_msg = result.unwrap_err().to_string();
        assert_that!(error_msg, contains_substring("os"));
    }

    #[test]
    fn transformer_blocks_os_rename() {
        let script = r#"
            os.rename("/tmp/old.txt", "/tmp/new.txt")
        "#;
        let transformer = LuaTransformer::new("test", script);
        let tx = create_test_transaction();

        let result = transform_with_result(&transformer, tx);

        // Should get an error about 'os' being undefined
        assert_that!(result.is_err(), eq(true));
        let error_msg = result.unwrap_err().to_string();
        assert_that!(error_msg, contains_substring("os"));
    }

    #[test]
    fn transformer_blocks_package_loadlib() {
        let script = r#"
            package.loadlib("/lib/x86_64-linux-gnu/libc.so.6", "malloc")
        "#;
        let transformer = LuaTransformer::new("test", script);
        let tx = create_test_transaction();

        let result = transform_with_result(&transformer, tx);

        // Should get an error about 'package' being undefined
        assert_that!(result.is_err(), eq(true));
        let error_msg = result.unwrap_err().to_string();
        assert_that!(error_msg, contains_substring("package"));
    }

    #[test]
    fn transformer_blocks_debug_getinfo() {
        let script = r#"
            debug.getinfo(1)
        "#;
        let transformer = LuaTransformer::new("test", script);
        let tx = create_test_transaction();

        let result = transform_with_result(&transformer, tx);

        // Should get an error about 'debug' being undefined
        assert_that!(result.is_err(), eq(true));
        let error_msg = result.unwrap_err().to_string();
        assert_that!(error_msg, contains_substring("debug"));
    }

    #[test]
    fn transformer_blocks_coroutine_create() {
        let script = r#"
            local co = coroutine.create(function()
                return transaction
            end)
        "#;
        let transformer = LuaTransformer::new("test", script);
        let tx = create_test_transaction();

        let result = transform_with_result(&transformer, tx);

        // Should get an error about 'coroutine' being undefined
        assert_that!(result.is_err(), eq(true));
        let error_msg = result.unwrap_err().to_string();
        assert_that!(error_msg, contains_substring("coroutine"));
    }

    #[test]
    fn transformer_allows_safe_string_operations() {
        let script = r#"
            transaction.description = string.upper(transaction.description)
        "#;
        let transformer = LuaTransformer::new("test", script);
        let tx = create_test_transaction();

        let result = transformer.transform(tx);

        // Safe operations should work
        assert_that!(result.len(), eq(1));
        assert_that!(result[0].description.as_str(), eq("TEST TRANSACTION"));
    }

    #[test]
    fn transformer_allows_safe_math_operations() {
        let script = r#"
            local amount_num = tonumber(transaction.amount)
            if amount_num then
                local rounded = math.floor(amount_num + 0.5)
                transaction:set_tag("rounded", tostring(rounded))
            end
        "#;
        let transformer = LuaTransformer::new("test", script);
        let tx = create_test_transaction();

        let result = transformer.transform(tx);

        // Safe operations should work
        assert_that!(result.len(), eq(1));
        assert_that!(
            result[0].tags.get::<String>(&"rounded".to_string()),
            some(eq(&"100".to_string()))
        );
    }

    #[test]
    fn transformer_allows_safe_table_operations() {
        let script = r#"
            local items = {"apple", "banana", "cherry"}
            table.insert(items, "date")
            transaction:set_tag("count", tostring(#items))
        "#;
        let transformer = LuaTransformer::new("test", script);
        let tx = create_test_transaction();

        let result = transformer.transform(tx);

        // Safe operations should work
        assert_that!(result.len(), eq(1));
        assert_that!(
            result[0].tags.get::<String>(&"count".to_string()),
            some(eq(&"4".to_string()))
        );
    }
}
