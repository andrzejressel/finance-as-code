# api/lunchmoney — Agents Guide

Low-level client for the Lunch Money v2 API.

API Documentation: https://alpha.lunchmoney.dev/v2/docs

Direct OpenAPI yaml: https://alpha.lunchmoney.dev/v2/openapi

## Directives

**Prioritize Documentation**: ALWAYS use the provided API Documentation link above as your primary source of truth. Do NOT use external web searches for API details unless the documentation link is unreachable or the specific information is demonstrably missing after thorough review of the provided link.

## Key Components

**LunchMoneyApi Trait**: The primary interface for interacting with Lunch Money. It is auto-mockable via `mockall` in this crate's tests and when the crate feature `mock` is enabled (for downstream crate tests).

**LunchMoneyClient**: The production implementation using `reqwest::blocking`.

**Manual Accounts**: `GET /manual_accounts` is available via `get_all_manual_accounts`.

## Non-obvious behaviour

**Automatic Pagination**: `get_all_transactions` automatically follows `has_more` and `offset` flags to fetch the entire result set in a single call.

**Bulk Inserts**: New transactions must be created with `POST /transactions` using insert payloads (not `PUT /transactions`), with a max of 500 transactions per request.

**Partial Updates**: `put_transactions` uses `UpdateTransactionDto` which only serializes `Some` fields, allowing for surgical updates to existing transactions.

**Delete Semantics**: bulk delete (`DELETE /transactions` with `{ ids }`) is treated as success on HTTP `204 No Content`.

**Error Handling**: Uses `rootcause` for context-aware error reporting, capturing HTTP status code, response headers, and response body on failure.

**Rate Limiting**: in addition to the shared `crates/api` `HTTP 5xx` exponential-backoff policy, Lunch Money has extra `HTTP 429 Too Many Requests` handling. The client respects `Retry-After` (seconds) before retrying and falls back to a default wait when that header is missing.
