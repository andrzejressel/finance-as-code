# budget/transformer/ai-lua — Agents Guide

This crate converts natural-language transaction rules into Lua scripts (via Gemini) and executes them using the Lua transformer runtime.

## Non-obvious behaviour

**Lazy AI generation**: `create_ai_lua_builder(...)` only wires dependencies. Lua code is loaded/generated on the first `transform(...)` call.

**Two-level cache**: generated Lua is cached in memory (`lua_code`) and persisted on disk (`JsonFileMap`). After first load, in-memory cache wins for the lifetime of that transformer instance.

**Cache key is exact text**: `user_description` is used verbatim as the cache key. Any wording change (even whitespace/punctuation) causes a cache miss and a new generation.

**Markdown cleanup**: model output is sanitized by stripping fenced code markers (```lua ... ``` / ``` ... ```) before execution.

**Execution sandbox is delegated**: this crate executes code through `finance_as_code_budget_transformer_lua::DefaultLuaExecutor`, so Lua safety restrictions are defined in `budget/transformer/lua`.

**Fail-fast errors**: generation failures, cache I/O failures, and Lua runtime failures are returned as errors with context; transactions are not silently dropped on these failures.

## Prompt Contract

**Keep prompt and runtime API in sync**: `create_system_prompt()` defines the Lua API contract sent to Gemini. If transaction fields/methods or return semantics change in `budget/transformer/lua`, update the prompt and user docs in `budget/root/src/docs/lua-ai.md` together.

**Response schema is strict**: AI output must deserialize into `GeneratedLuaResponse { lua_code: String }`. Prompt changes must preserve that JSON shape.

## Usage pattern

Prefer the `bon` builder:

```rust
use finance_as_code_budget_transformer_ai_lua::{AiLuaConfig, create_ai_lua_builder};
use std::path::PathBuf;

let transformer = create_ai_lua_builder(
    AiLuaConfig::builder()
        .name("auto-categorize")
        .user_description("If description contains 'Walmart', set category tag to 'Grocery'")
        .api_key("gemini_api_key")
        .cache_path(PathBuf::from(".cache/lua-transformer.json"))
        .build(),
)?;
```

`cache_path` should point to a JSON cache file (parent directories are created automatically by `JsonFileMap`).
