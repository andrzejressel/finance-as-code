use bon::Builder;
use finance_as_code_budget_core::transformer::Transformer;
use finance_as_code_budget_core::Transaction;
use finance_as_code_budget_transformer_lua::{DefaultLuaExecutor, LuaExecutor};
use finance_as_code_utils_file_map::{FileStringMap, JsonFileMap};
use finance_as_code_utils_gemini::{ContentGenerator, GeminiClient};
use log::{debug, info, warn};
use rootcause::prelude::ResultExt;
use rootcause::Result;
use std::cell::RefCell;
use std::path::PathBuf;

#[derive(Builder)]
pub struct AiLuaConfig {
    #[builder(into)]
    name: String,
    #[builder(into)]
    user_description: String,
    #[builder(into)]
    api_key: String,
    #[builder(into)]
    cache_path: PathBuf,
}

pub fn create_ai_lua_builder(config: AiLuaConfig) -> Result<impl Transformer> {
    let gemini_builder = GeminiClient::create(config.api_key);
    let cache = JsonFileMap::new(&config.cache_path)
        .context_with(|| format!("Failed to initialize cache at {:?}", config.cache_path))?;
    let lua_executor = DefaultLuaExecutor;

    Ok(AiLuaTransformer {
        name: config.name,
        user_description: config.user_description,
        content_generator: gemini_builder,
        lua_executor,
        cache: RefCell::new(cache),
        lua_code: RefCell::new(None),
    })
}

/// A transformer that uses AI to generate Lua code from natural language
/// descriptions and executes it on transactions.
///
/// The generated Lua code is cached using the user description as the key,
/// so subsequent transformations with the same description don't require
/// additional API calls.
pub struct AiLuaTransformer<G: ContentGenerator, E: LuaExecutor, KV: FileStringMap> {
    name: String,
    user_description: String,
    content_generator: G,
    lua_executor: E,
    cache: RefCell<KV>,
    lua_code: RefCell<Option<String>>,
}

/// Type alias for the default AI Lua transformer with real implementations.
pub type DefaultAiLuaTransformer = AiLuaTransformer<GeminiClient, DefaultLuaExecutor, JsonFileMap>;

impl<G, E, KV> AiLuaTransformer<G, E, KV>
where
    G: ContentGenerator,
    E: LuaExecutor,
    KV: FileStringMap,
{
    /// Generates Lua code from the user description using the content generator.
    ///
    /// # Errors
    ///
    /// Returns an error if the content generator fails or returns invalid content.
    fn generate_lua_code(&self) -> Result<String> {
        let prompt = self.create_prompt();
        debug!("Sending prompt to AI for Lua code generation");

        let response = self
            .content_generator
            .generate_content(&prompt)
            .context("Failed to generate Lua code from AI")?;

        // Clean up the response - remove markdown code blocks if present
        let lua_code = self.clean_lua_code(&response);

        debug!("Generated Lua code: {}", lua_code);
        Ok(lua_code)
    }

    /// Creates the prompt for AI to generate Lua code.
    fn create_prompt(&self) -> String {
        format!(
            r#"You are a code generator that creates Lua scripts for financial transaction transformation.

## Task
Generate a Lua script based on this user requirement:
{}

## Lua API
You have access to a global `transaction` object with these fields and methods:

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
- `transaction:split()` - Create a copy of the transaction (without tags), returns new transaction object

### Return values:
- Return nothing (or no return statement) → modified `transaction` is used
- Return `nil` → transaction is dropped (filtered out)
- Return single transaction → that transaction is used
- Return `{{tx1, tx2, ...}}` → multiple transactions (table/array)

## Examples

### Example 1: Add tag based on description
User: "if description contains 'Walmart' then set category tag to 'Grocery'"
Lua:
if transaction.description:lower():match("walmart") then
    transaction:set_tag("category", "Grocery")
end

### Example 2: Modify description
User: "add prefix 'PROCESSED: ' to description"
Lua:
transaction.description = "PROCESSED: " .. transaction.description

### Example 3: Filter out transactions
User: "remove transactions with 'INTERNAL' in description"
Lua:
if transaction.description:match("INTERNAL") then
    return nil
end

### Example 4: Split transaction
User: "split transaction into two equal parts"
Lua:
local tx2 = transaction:split()
return {{transaction, tx2}}

## Rules
1. Return ONLY the Lua code, no explanations or markdown
2. Do not wrap in ```lua ... ``` blocks
3. Use proper Lua syntax (then/end, local variables, etc.)
4. Keep the code simple and efficient
5. Handle edge cases gracefully

Generate the Lua script now:"#,
            self.user_description
        )
    }

    /// Cleans the generated Lua code by removing markdown code block markers.
    fn clean_lua_code(&self, code: &str) -> String {
        let code = code.trim();

        // Remove markdown code block markers if present
        let code = code.strip_prefix("```lua").unwrap_or(code);
        let code = code.strip_prefix("```").unwrap_or(code);
        let code = code.strip_suffix("```").unwrap_or(code);

        code.trim().to_string()
    }

    /// Gets or generates the Lua code for this transformer.
    ///
    /// # Errors
    ///
    /// Returns an error if code generation fails.
    fn get_lua_code(&self) -> Result<String> {
        let lua_code_cell = &self.lua_code;

        // Check if we already have the code in memory
        if let Some(ref code) = *lua_code_cell.borrow() {
            debug!("Using in-memory Lua code");
            return Ok(code.clone());
        }

        // Try to get from cache
        if let Some(cached_code) = self.cache.borrow().get(&self.user_description) {
            debug!(
                "Using cached Lua code for description: {}",
                self.user_description
            );
            let code = cached_code.to_string();
            *lua_code_cell.borrow_mut() = Some(code.clone());
            return Ok(code);
        }

        // Generate new code
        info!(
            "Generating Lua code for description: {}",
            self.user_description
        );
        let lua_code = self.generate_lua_code()?;

        // Cache the generated code
        self.cache
            .borrow_mut()
            .put(&self.user_description, &lua_code)
            .context("Failed to cache generated Lua code")?;

        // Store in memory for faster subsequent calls
        *lua_code_cell.borrow_mut() = Some(lua_code.clone());

        Ok(lua_code)
    }
}


impl<G, E, KV> Transformer for AiLuaTransformer<G, E, KV>
where
    G: ContentGenerator,
    E: LuaExecutor,
    KV: FileStringMap,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn transform(&self, transaction: Transaction) -> Vec<Transaction> {
        let lua_code = match self.get_lua_code() {
            Ok(code) => code,
            Err(e) => {
                warn!("Failed to get Lua code: {:?}", e);
                return vec![];
            }
        };

        self.lua_executor
            .execute(&self.name, &lua_code, transaction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use finance_as_code_budget_core::TagMap;
    use finance_as_code_budget_transformer_lua::MockLuaExecutor;
    use finance_as_code_utils_gemini::MockContentGenerator;
    use googletest::assert_that;
    use googletest::prelude::eq;
    use googletest::prelude::some;
    use rusty_money::iso::USD;
    use rusty_money::Money;
    use std::path::Path;
    use tempfile::NamedTempFile;
    use uuid::Uuid;

    type MockAiLuaTransformer = AiLuaTransformer<MockContentGenerator, MockLuaExecutor, JsonFileMap>;

    fn create_transformer_with_mocks(
        name: &str,
        user_description: &str,
        mock_generator: MockContentGenerator,
        mock_executor: MockLuaExecutor,
        cache_path: &Path,
    ) -> MockAiLuaTransformer {
        let cache = JsonFileMap::new(cache_path).unwrap();

        MockAiLuaTransformer {
            name: name.to_string(),
            user_description: user_description.to_string(),
            content_generator: mock_generator,
            lua_executor: mock_executor,
            cache: RefCell::new(cache),
            lua_code: RefCell::new(None),
        }
    }

    fn create_test_transaction() -> Transaction {
        Transaction {
            id: Uuid::parse_str("c65d0f0e-f7a4-4df4-a9e2-cd75ecdc77f6").unwrap(),
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            description: "WALMART STORE #1234".to_string(),
            counterparty: "Walmart".to_string(),
            amount: Money::from_major(100, USD),
            other_side_account_number: None,
            tags: TagMap::new(),
        }
    }

    #[test]
    fn transformer_returns_configured_name() {
        let temp_file = NamedTempFile::new().unwrap();
        let mock_generator = MockContentGenerator::new();
        let mock_executor = MockLuaExecutor::new();

        let transformer = create_transformer_with_mocks(
            "test-ai-transformer",
            "test description",
            mock_generator,
            mock_executor,
            temp_file.path(),
        );

        assert_that!(transformer.name(), eq("test-ai-transformer"));
    }

    #[test]
    fn transformer_uses_cached_lua_code() {
        let temp_file = NamedTempFile::new().unwrap();
        let cache_path = temp_file.path().to_path_buf();

        // Pre-populate cache with known Lua code
        let mut cache = JsonFileMap::new(&cache_path).unwrap();
        cache
            .put(
                "if description contains 'Walmart' then category is 'Grocery'",
                r#"transaction:set_tag("category", "Grocery")"#,
            )
            .unwrap();

        let mock_generator = MockContentGenerator::new();
        let mut mock_executor = MockLuaExecutor::new();

        // Setup mock to return a transformed transaction
        let expected_tx = create_test_transaction();
        mock_executor
            .expect_execute()
            .times(1)
            .returning(move |_, _, tx| {
                let mut tx = tx;
                tx.tags
                    .insert("category".to_string(), "Grocery".to_string());
                vec![tx]
            });

        let transformer = create_transformer_with_mocks(
            "test-cached",
            "if description contains 'Walmart' then category is 'Grocery'",
            mock_generator,
            mock_executor,
            &cache_path,
        );

        let tx = create_test_transaction();
        let result = transformer.transform(tx);

        assert_that!(result.len(), eq(1));
        assert_that!(
            result[0].tags.get::<String>(&"category".to_string()),
            some(eq(&"Grocery".to_string()))
        );
    }

    #[test]
    fn transformer_generates_lua_code_via_mock() {
        let temp_file = NamedTempFile::new().unwrap();
        let cache_path = temp_file.path().to_path_buf();

        let mut mock_generator = MockContentGenerator::new();
        let mut mock_executor = MockLuaExecutor::new();

        // Setup mock generator to return Lua code
        mock_generator
            .expect_generate_content()
            .times(1)
            .returning(|_| Ok(r#"transaction:set_tag("category", "Grocery")"#.to_string()));

        // Setup mock executor to return a transformed transaction
        mock_executor
            .expect_execute()
            .times(1)
            .returning(|_, _, tx| {
                let mut tx = tx;
                tx.tags
                    .insert("category".to_string(), "Grocery".to_string());
                vec![tx]
            });

        let transformer = create_transformer_with_mocks(
            "test-api",
            "if description contains 'Walmart' then category is 'Grocery'",
            mock_generator,
            mock_executor,
            &cache_path,
        );

        let tx = create_test_transaction();
        let result = transformer.transform(tx);

        assert_that!(result.len(), eq(1));
        assert_that!(
            result[0].tags.get::<String>(&"category".to_string()),
            some(eq(&"Grocery".to_string()))
        );
    }

    #[test]
    fn transformer_cleans_markdown_from_generated_code() {
        let temp_file = NamedTempFile::new().unwrap();
        let cache_path = temp_file.path().to_path_buf();

        let mut mock_generator = MockContentGenerator::new();
        let mut mock_executor = MockLuaExecutor::new();

        // Setup mock generator to return Lua code with markdown
        mock_generator
            .expect_generate_content()
            .times(1)
            .returning(|_| Ok("```lua\ntransaction:set_tag(\"test\", \"value\")\n```".to_string()));

        // Setup mock executor
        mock_executor
            .expect_execute()
            .times(1)
            .returning(|_, _, tx| {
                let mut tx = tx;
                tx.tags.insert("test".to_string(), "value".to_string());
                vec![tx]
            });

        let transformer = create_transformer_with_mocks(
            "test-clean",
            "add test tag",
            mock_generator,
            mock_executor,
            &cache_path,
        );

        let tx = create_test_transaction();
        let result = transformer.transform(tx);

        assert_that!(result.len(), eq(1));
        assert_that!(
            result[0].tags.get::<String>(&"test".to_string()),
            some(eq(&"value".to_string()))
        );
    }

    #[test]
    fn transformer_uses_memory_cache_on_second_call() {
        let temp_file = NamedTempFile::new().unwrap();
        let cache_path = temp_file.path().to_path_buf();

        let mut mock_generator = MockContentGenerator::new();
        let mut mock_executor = MockLuaExecutor::new();

        // Generator should only be called once
        mock_generator
            .expect_generate_content()
            .times(1)
            .returning(|_| Ok(r#"transaction:set_tag("cached", "true")"#.to_string()));

        // Executor called twice
        mock_executor
            .expect_execute()
            .times(2)
            .returning(|_, _, tx| {
                let mut tx = tx;
                tx.tags.insert("cached".to_string(), "true".to_string());
                vec![tx]
            });

        let transformer = create_transformer_with_mocks(
            "test-memory-cache",
            "add cached tag",
            mock_generator,
            mock_executor,
            &cache_path,
        );

        let tx1 = create_test_transaction();
        let tx2 = create_test_transaction();

        // First call - should generate and cache
        let _ = transformer.transform(tx1);
        // Second call - should use memory cache
        let _ = transformer.transform(tx2);
    }

    #[test]
    fn transformer_handles_generator_error() {
        let temp_file = NamedTempFile::new().unwrap();
        let cache_path = temp_file.path().to_path_buf();

        let mut mock_generator = MockContentGenerator::new();
        let mock_executor = MockLuaExecutor::new();

        // Setup mock generator to return error
        mock_generator
            .expect_generate_content()
            .times(1)
            .returning(|_| Err(rootcause::report!("API error")));

        let transformer = create_transformer_with_mocks(
            "test-error",
            "test description",
            mock_generator,
            mock_executor,
            &cache_path,
        );

        let tx = create_test_transaction();
        let result = transformer.transform(tx);

        // Should return empty vector on error
        assert_that!(result.len(), eq(0));
    }
}
