# utils/gemini — Agents Guide

Utility for interacting with the Google Gemini API via the `genai` crate.

## Non-obvious behaviour

**Synchronous wrapper around async client**: `GeminiClient::generate_content` creates its own single-threaded `tokio` runtime to block on the underlying `genai` async calls. This allows the utility to be used in synchronous contexts without requiring the caller to be `async`.

**Hardcoded model**: the current implementation is hardcoded to use `gemini-2.5-flash`.

**Explicit constructors**: the client provides `create(api_key)` for standard usage and `create_with_base_url(api_key, base_url)` for testing or custom endpoints. When using a custom base URL, the client automatically appends `/v1beta` for Gemini compatibility.
