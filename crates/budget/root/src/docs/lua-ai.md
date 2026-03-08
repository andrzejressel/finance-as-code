# AI-Powered Lua Transformer

This module provides [`transformer_ai_lua::AiLuaTransformer`] that translates **natural language descriptions** into Lua scripts using the Gemini AI API and executes them against financial transactions — no Lua knowledge required.

Generated Lua code is cached on disk (keyed by your description), so repeat runs skip the API call entirely.

## Quick Start

```rust,no_run,ignore
use finance_as_code_budget::run;
# use finance_as_code_budget::__private::{create_sink, create_source};
use finance_as_code_budget_transformer_ai_lua::{AiLuaConfig, create_ai_lua_builder};
use std::path::PathBuf;

fn main() {
    let transformer = create_ai_lua_builder(
        AiLuaConfig::builder()
            .name("auto-categorize")
            .user_description("If description contains 'Walmart', set category tag to 'Grocery'")
            .api_key("gemini_api_key")
            .cache_path(PathBuf::from(".cache/lua"))
            .build(),
    )
    .expect("Failed to create AI Lua transformer");

    let sources = vec![create_source()];
    let sinks = vec![create_sink()];

    run(vec![], sources, vec![Box::new(transformer)], sinks).expect("Pipeline run failed");
}
```

## How It Works

On first use, the transformer:

1. Sends your `user_description` plus the full [Lua API reference](crate::lua) to the Gemini AI
2. Receives generd Lua code back
3. Caches the code to `cache_path` (JSON file, keyed by description)
4. Executes the Lua code on each transaction

On subsequent runs the AI call is skipped — the cached Lua is loaded from disk instead.

## Examples

### Categorise by description

```rust,no_run
use finance_as_code_budget_transformer_ai_lua::{AiLuaConfig, create_ai_lua_builder};
use std::path::PathBuf;

let transformer = create_ai_lua_builder(
    AiLuaConfig::builder()
        .name("categorize-food")
        .user_description(
            "Set cateategory tag to 'Food' when description matches 'RESTAURANT' or 'CAFE', \
             and to 'Transport' when description matches 'UBER' or 'TAXI'",
        )
        .api_key("gemini_api_key")
        .cache_path(PathBuf::from(".cache/lua"))
        .build(),
)
.expect("Failed to create transformer");
```

### Filter out internal transfers

```rust,no_run
use finance_as_code_budget_transformer_ai_lua::{AiLuaConfig, create_ai_lua_builder};
use std::path::PathBuf;

let transformer = create_ai_lua_builder(
    AiLuaConfig::builder()
        .name("drop-internal")
        .user_description("Drop any transaction whose description contains 'INTERNAL TRANSFER'")
        .api_key("gemini_api_key")
        .cache_path(PathBuf::from(".cache/lua"))
        .build(),
)
.expect("Failed to create transformer");
```

### Rename counterparty

```rust,no_run
use finance_as_code_budget_transformer_ai_lua::{AiLuaConfig, create_ai_lua_builder};
use std::path::PathBuf;

let transformer = create_ai_lua_builder(
    AiLuaConfig::builder()
        .name("normalize-names")
        .user_description(
            "Replace counterparty with 'Amazon' when description starts with 'AMZN', \
             and with 'Netflix' when counterparty contains 'NFLX'",
        )
        .api_key("gemini_api_key")
        .cache_path(PathBuf::from(".cache/lua"))
        .build(),
)
.expect("Failed to create transformer");
```

## Caching Behaviour

The cache is a JSON file at `cache_path`. The **description string is the cache key** — changing even one character triggers a new AI call and replaces the cached entry. Delete the file to force regeneration.

## Error Handling

If the Gemini API call fails (network error, quota exceeded, invalid key), the transformer logs a warning and returns an empty `Vec` for that transaction — it does not panic.

If the generated Lua has a syntax or runtime error, the same empty-vector behaviour applies (identical to [`crate::lua::LuaTransformer`]).
