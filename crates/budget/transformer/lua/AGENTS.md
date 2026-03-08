# budget/transformer/lua — Agents Guide

This crate provides a Lua-based transaction transformer, allowing users to modify, filter, or split transactions at runtime using Lua scripts without recompiling the Rust pipeline.

## Security

**Safe Lua environment**: The Lua interpreter runs in a **sandboxed environment** with restricted standard libraries. The following dangerous libraries are **disabled**:

- **`io` library** — File I/O operations are blocked (no `io.open`, `io.popen`, etc.)
- **`os` library** — OS operations are blocked (no `os.execute`, `os.remove`, `os.rename`, etc.)
- **`package` library** — Package loading is blocked (no `package.loadlib`, `require` with C modules, etc.)
- **`debug` library** — Debug introspection is blocked (no `debug.getinfo`, etc.)
- **`coroutine` library** — Coroutine operations are blocked (no `coroutine.create`, `coroutine.resume`, etc.)

**Allowed safe libraries**:

- **`table`** — Table manipulation (insert, remove, concat, etc.)
- **`string`** — String operations (upper, lower, sub, gsub, match, etc.)
- **`math`** — Mathematical functions (floor, ceil, abs, sin, cos, etc.)
- **`utf8`** — UTF-8 string operations

This prevents Lua scripts from performing file I/O, executing system commands, loading external code, or bypassing security measures. All security restrictions are enforced at the mlua level and cannot be bypassed from Lua code.

## Non-obvious behaviour

**Irrecoverable errors**: If a Lua script fails (syntax error or runtime panic), the transformer returns an **empty vector**, effectively dropping the transaction. Due to ownership rules, the original transaction cannot be recovered once passed to the Lua engine.

**`clone()` tag behavior**: When a transaction is cloned within Lua, the resulting copy has a **new UUID** and **preserves all tags**. The cloned transaction is a full deep copy of the original transaction.

**Amount representation**: Amounts are exposed to Lua as **strings** to maintain decimal precision and avoid floating-point issues during script execution.

**Documentation**: When changing internal logic, also change the documentation. It is referenced on top of `lib.rs`