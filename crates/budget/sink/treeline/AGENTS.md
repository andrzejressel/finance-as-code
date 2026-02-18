# budget/sink/treeline — Agents Guide

Writes transactions into Treeline's local DuckDB database.

## Non-obvious behaviour

**Destructive write**: `Sink::write()` deletes all existing transactions and balances for the target account before inserting. There is no merge or upsert — every run is a full replacement.

**Two `Transaction` types**: `treeline-core::Transaction` and `finance_as_code_budget_core::Transaction` are different types. `queries.rs` maps between them. Do not confuse them when modifying query code.

**Database location**: resolved from `TREELINE_DIR` env var, falling back to `~/.treeline`. Encryption key from `TL_DB_KEY` (raw key) or `TL_DB_PASSWORD` (password), if set.
