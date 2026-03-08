# Lua-based Transaction Transformer

This module provides [`lua::LuaTransformer`] that executes Lua scripts against financial transactions, allowing runtime customization of transaction processing without recompiling Rust code.

## Quick Start

```rust
use finance_as_code_budget_core::{Transaction, transformer::Transformer};
use finance_as_code_budget_transformer_lua::LuaTransformer;
use rusty_money::{Money, iso::USD};
use chrono::NaiveDate;
use uuid::Uuid;

// Create a transformer that adds tags based on description
let script = r#"
    if transaction.description:match("COFFEE") then
        transaction:set_tag("category", "food-drink")
    end
"#;

let transformer = LuaTransformer::new("auto-categorize", script);

let tx = Transaction {
    id: Uuid::new_v4(),
    date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    description: "COFFEE SHOP".to_string(),
    counterparty: "Local Cafe".to_string(),
    amount: Money::from_major(5, USD),
    other_side_account_number: None,
    tags: finance_as_code_utils_hmap::HMap::new(),
};

let results = transformer.transform(tx);
assert_eq!(results[0].tags.get::<String>(&"category".to_string()).unwrap(), "food-drink");
```

## Lua API

Your Lua scripts have access to a global `transaction` object with these properties and methods:

### Read-only fields:
- `transaction.id` - UUID as string
- `transaction.date` - Date as string
- `transaction.amount` - Amount as string
- `transaction.currency` - Currency code as string

### Read-write fields:
- `transaction.description` - Transaction description
- `transaction.counterparty` - The other party

### Methods:
- `transaction:get_tag(key)` - Get tag value or `nil`
- `transaction:set_tag(key, value)` - Set tag value
- `transaction:clone()` - Create a copy of the transaction with all fields and tags

### Return values:
- Return nothing → modified `transaction` is used
- Return `nil` → transaction is dropped
- Return single transaction → that transaction is used
- Return `{tx1, tx2, ...}` → multiple transactions

## Examples

### Modify description and add tags

```rust
use finance_as_code_budget_core::{Transaction, transformer::Transformer};
use finance_as_code_budget_transformer_lua::LuaTransformer;
use rusty_money::{Money, iso::USD};
use chrono::NaiveDate;
use uuid::Uuid;

let script = r#"
    transaction.description = transaction.description .. " [PROCESSED]"
    transaction:set_tag("processed", "true")
"#;

let transformer = LuaTransformer::new("add-tag", script);
let tx = Transaction {
    id: Uuid::new_v4(),
    date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    description: "Coffee".to_string(),
    counterparty: "Cafe".to_string(),
    amount: Money::from_major(5, USD),
    other_side_account_number: None,
    tags: finance_as_code_utils_hmap::HMap::new(),
};
let result = transformer.transform(tx);
assert_eq!(result[0].description, "Coffee [PROCESSED]");
```

### Split transaction into multiple parts

```rust
use finance_as_code_budget_core::{Transaction, transformer::Transformer};
use finance_as_code_budget_transformer_lua::LuaTransformer;
use rusty_money::{Money, iso::USD};
use chrono::NaiveDate;
use uuid::Uuid;

let script = r#"
    local tax = transaction:clone()
    tax.description = "Tax: " .. transaction.description
    tax:set_tag("category", "tax")
    transaction:set_tag("category", "main")
    return {transaction, tax}
"#;

let transformer = LuaTransformer::new("split-tax", script);
let tx = Transaction {
    id: Uuid::new_v4(),
    date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    description: "Purchase".to_string(),
    counterparty: "Store".to_string(),
    amount: Money::from_major(100, USD),
    other_side_account_number: None,
    tags: finance_as_code_utils_hmap::HMap::new(),
};
let results = transformer.transform(tx);
assert_eq!(results.len(), 2);
assert!(results[1].description.starts_with("Tax:"));
```

### Filter out transactions

```rust
use finance_as_code_budget_core::{Transaction, transformer::Transformer};
use finance_as_code_budget_transformer_lua::LuaTransformer;
use rusty_money::{Money, iso::USD};
use chrono::NaiveDate;
use uuid::Uuid;

let script = r#"
    if transaction.description:match("INTERNAL") then
        return nil  -- Drop this transaction
    end
"#;

let transformer = LuaTransformer::new("filter", script);
let tx = Transaction {
    id: Uuid::new_v4(),
    date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    description: "INTERNAL TRANSFER".to_string(),
    counterparty: "Self".to_string(),
    amount: Money::from_major(1000, USD),
    other_side_account_number: None,
    tags: finance_as_code_utils_hmap::HMap::new(),
};
let results = transformer.transform(tx);
assert_eq!(results.len(), 0);  // Transaction was dropped
```

### Pattern matching and categorization

```rust
use finance_as_code_budget_core::{Transaction, transformer::Transformer};
use finance_as_code_budget_transformer_lua::LuaTransformer;
use rusty_money::{Money, iso::USD};
use chrono::NaiveDate;
use uuid::Uuid;

let script = r#"
    local desc = transaction.description
    if desc:match("UBER") or desc:match("TAXI") then
        transaction:set_tag("category", "transport")
    elseif desc:match("RESTAURANT") or desc:match("CAFE") then
        transaction:set_tag("category", "food")
    end
"#;

let transformer = LuaTransformer::new("categorize", script);
let tx = Transaction {
    id: Uuid::new_v4(),
    date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    description: "UBER TRIP".to_string(),
    counterparty: "Uber".to_string(),
    amount: Money::from_major(15, USD),
    other_side_account_number: None,
    tags: finance_as_code_utils_hmap::HMap::new(),
};
let results = transformer.transform(tx);
assert_eq!(results[0].tags.get::<String>(&"category".to_string()).unwrap(), "transport");
```

## Error Handling

If a Lua script fails (syntax error, runtime error, etc.), an error is printed to stderr and an empty vector is returned. The original transaction cannot be recovered.

```rust
use finance_as_code_budget_core::{Transaction, transformer::Transformer};
use finance_as_code_budget_transformer_lua::LuaTransformer;
use rusty_money::{Money, iso::USD};
use chrono::NaiveDate;
use uuid::Uuid;

let bad_script = "this is not valid lua ][[]";
let transformer = LuaTransformer::new("broken", bad_script);
let tx = Transaction {
    id: Uuid::new_v4(),
    date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    description: "Test".to_string(),
    counterparty: "Test".to_string(),
    amount: Money::from_major(100, USD),
    other_side_account_number: None,
    tags: finance_as_code_utils_hmap::HMap::new(),
};
let results = transformer.transform(tx);
assert_eq!(results.len(), 0);  // Error = empty results
```
