# budget/transformer/lua — Agents Guide

This crate provides a Lua-based transaction transformer, allowing users to modify, filter, or split transactions at runtime using Lua scripts without recompiling the Rust pipeline.

## Non-obvious behaviour

**Irrecoverable errors**: If a Lua script fails (syntax error or runtime panic), the transformer returns an **empty vector**, effectively dropping the transaction. Due to ownership rules, the original transaction cannot be recovered once passed to the Lua engine.

**`clone()` tag behavior**: When a transaction is cloned within Lua, the resulting copy has a **new UUID** and **preserves all tags**. The cloned transaction is a full deep copy of the original transaction.

**Amount representation**: Amounts are exposed to Lua as **strings** to maintain decimal precision and avoid floating-point issues during script execution.

**Documentation**: When changing internal logic, also change the documentation. It is referenced on top of `lib.rs`