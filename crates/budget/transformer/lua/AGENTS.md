# Lua Transformer

This crate provides a Lua-based transaction transformer for the finance-as-code pipeline.

## Current Implementation

This is a minimal implementation that satisfies the `Transformer` trait. The transformer currently passes transactions through unchanged. Actual Lua script execution logic will be implemented in future iterations.

## Dependencies

- **mlua** — provides Lua bindings for Rust
- **finance_as_code_budget_core** — for the `Transformer` trait and `Transaction` type

## Future Work

- Implement Lua script execution
- Add script loading from files or strings
- Provide a Lua API for accessing and modifying transaction fields
- Add error handling for Lua runtime errors
