# AI-Powered Lua Transformer

This module provides [`AiLuaTransformer`] that uses AI (Google Gemini) to generate Lua scripts from natural language descriptions, allowing you to categorize and transform transactions using plain English rules.

The generated Lua code is automatically cached, so subsequent transformations with the same description don't require additional API calls.

## Quick Start

```rust
use finance_as_code_budget_transformer_ai_lua::AiLuaTransformer;
use finance_as_code_budget_core::transformer::Transformer;

// Create a transformer that categorizes transactions based on description
let transformer = AiLuaTransformer::builder()
    .name("grocery-categorizer")
    .description("if transaction description contains 'Walmart' then set category tag to 'Grocery'")
    .api_key("your-gemini-api-key")
    .build()
    .unwrap();

// Use it in your transformer pipeline
let transactions = vec![/* your transactions */];
let transformed: Vec<_> = transactions
    .into_iter()
    .flat_map(|tx| transformer.transform(tx))
    .collect();
```

## How It Works

1. **You provide a natural language description** of the transformation rule (e.g., "if description contains 'Walmart' then category is 'Grocery'")
2. **AI generates Lua code** that implements your rule using the Gemini API
3. **Code is cached** locally using the description as the key
4. **Lua code is executed** on each transaction using the existing Lua transformer

## Caching

The generated Lua code is cached in two places:

1. **In-memory**: For fastest access during the current session
2. **On-disk**: JSON file (default: `.ai-lua-cache.json`) for persistence across runs

This means:
- First transformation with a description calls the Gemini API
- Subsequent transformations (even after restart) use the cached code
- No API calls are made for cached descriptions

### Custom Cache Location

```rust
use std::path::PathBuf;

let transformer = AiLuaTransformer::builder()
    .name("categorizer")
    .description("if description contains 'UBER' then category is 'Transport'")
    .api_key("your-api-key")
    .cache_path(PathBuf::from("./my-cache.json"))
    .build()
    .unwrap();
```

## Examples

### Categorize grocery transactions

```rust
use finance_as_code_budget_transformer_ai_lua::AiLuaTransformer;
use finance_as_code_budget_core::transformer::Transformer;

let transformer = AiLuaTransformer::builder()
    .name("grocery")
    .description("if description contains 'Walmart', 'Target', or 'Costco' then set category tag to 'Groceries'")
    .api_key("your-api-key")
    .build()
    .unwrap();
```

### Categorize transportation

```rust
let transformer = AiLuaTransformer::builder()
    .name("transport")
    .description("if description contains 'UBER', 'LYFT', or 'TAXI' then set category tag to 'Transportation'")
    .api_key("your-api-key")
    .build()
    .unwrap();
```

### Filter out internal transfers

```rust
let transformer = AiLuaTransformer::builder()
    .name("filter-internal")
    .description("remove transactions that contain 'INTERNAL TRANSFER' in description")
    .api_key("your-api-key")
    .build()
    .unwrap();
```

### Add processed tag to all transactions

```rust
let transformer = AiLuaTransformer::builder()
    .name("add-processed-tag")
    .description("add a tag 'processed' with value 'true' to every transaction")
    .api_key("your-api-key")
    .build()
    .unwrap();
```

### Complex categorization

```rust
let transformer = AiLuaTransformer::builder()
    .name("smart-categorizer")
    .description("
        categorize transactions based on description:
        - 'SHELL', 'BP', 'CHEVRON' -> category: 'Fuel'
        - 'STARBUCKS', 'MCDONALDS' -> category: 'Dining'
        - 'NETFLIX', 'SPOTIFY' -> category: 'Entertainment'
        - 'AMAZON' -> category: 'Shopping'
    ")
    .api_key("your-api-key")
    .build()
    .unwrap();
```

## Using Custom Gemini Endpoint

If you want to use a custom Gemini endpoint (e.g., for testing or self-hosted models):

```rust
let transformer = AiLuaTransformer::builder()
    .name("test-transformer")
    .description("add test tag")
    .api_key("your-api-key")
    .base_url("https://your-custom-endpoint.com")
    .build()
    .unwrap();
```

## Full Pipeline Example

```rust
use finance_as_code_budget::{
    LocalDirectorySource, Setup, Sink, Source, Transformer,
    lunchflow::{
        LunchFlowDownloaderConfig, create_lunchflow_downloader, create_lunchflow_file_reader,
    },
    lunchmoney::{LunchMoneySinkConfig, create_lunchmoney_sink},
    transformer_ai_lua::AiLuaTransformer,
    run,
};

fn main() {
    let lunchflow_dir = "path/to/lunchflow/dir";

    // Create AI-powered transformers
    let grocery_transformer = AiLuaTransformer::builder()
        .name("grocery-categorizer")
        .description("if description contains 'Walmart' or 'Target' then set category tag to 'Groceries'")
        .api_key("your-gemini-api-key")
        .build()
        .unwrap();

    let transport_transformer = AiLuaTransformer::builder()
        .name("transport-categorizer")
        .description("if description contains 'UBER' or 'LYFT' then set category tag to 'Transport'")
        .api_key("your-gemini-api-key")
        .build()
        .unwrap();

    let transformers: Vec<Box<dyn Transformer>> = vec![
        Box::new(grocery_transformer),
        Box::new(transport_transformer),
    ];

    let setups: Vec<Box<dyn Setup>> = vec![
        Box::new(create_lunchflow_downloader(
            LunchFlowDownloaderConfig::builder()
                .account_id(123_i64)
                .api_key("lunchflow_api_key")
                .local_directory(lunchflow_dir)
                .build(),
        )
        .expect("Failed to create Lunch Flow downloader")),
    ];

    let sources: Vec<Box<dyn Source>> = vec![
        Box::new(LocalDirectorySource::new(
            lunchflow_dir,
            create_lunchflow_file_reader(),
        )
        .expect("Failed to create local directory source")),
    ];

    let sinks: Vec<Box<dyn Sink>> = vec![Box::new(create_lunchmoney_sink(
        LunchMoneySinkConfig::builder()
            .api_key("lunchmoney_api_key")
            .account_name("My Account")
            .build(),
    ))];

    run(setups, sources, transformers, sinks).expect("Pipeline run failed");
}
```

## Lua API (Generated Code)

The AI generates Lua code that has access to these transaction fields and methods:

### Read-only fields:
- `transaction.id` - UUID as string
- `transaction.date` - Date as string
- `transaction.amount` - Amount as string
- `transaction.currency` - Currency code as string

### Read-write fields:
- `transaction.description` - Transaction description (string)
- `transaction.counterparty` - The other party (string)

### Methods:
- `transaction:get_tag(key)` - Get tag value by key, returns string or nil
- `transaction:set_tag(key, value)` - Set a tag with key and value (both strings)
- `transaction:split()` - Create a copy of the transaction (without tags)

### Return values:
- Return nothing → modified `transaction` is used
- Return `nil` → transaction is dropped (filtered out)
- Return single transaction → that transaction is used
- Return `{tx1, tx2, ...}` → multiple transactions

## Error Handling

If the Gemini API fails or generates invalid Lua code:
- An error is logged
- An empty vector is returned for that transaction
- The transaction is effectively dropped

For production use, consider:
1. Testing your descriptions thoroughly before deploying
2. Caching generated code (done automatically)
3. Having fallback transformers for critical rules

## API Key

Get your Gemini API key from [Google AI Studio](https://aistudio.google.com/app/apikey).

## Cost Considerations

- First transformation with each unique description calls the API
- Cached descriptions don't incur API costs
- Use specific, reusable descriptions to minimize API calls

## Limitations

1. **AI-generated code may not be perfect** - Always test your transformations
2. **Requires internet connection** for first-time code generation
3. **Rate limits** apply based on your Gemini API plan
4. **Description must be clear** - Ambiguous descriptions may generate incorrect code

## See Also

- [`LuaTransformer`](lua.md) - Direct Lua scripting without AI
- [`GeminiClient`](../utils/gemini/) - Low-level Gemini API client
