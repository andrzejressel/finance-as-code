# api/lunchmoney — Agents Guide

Low-level client for the Lunch Money v2 API.

API Documentation: https://alpha.lunchmoney.dev/v2/docs

## Directives

**Prioritize Documentation**: ALWAYS use the provided API Documentation link above as your primary source of truth. Do NOT use external web searches for API details unless the documentation link is unreachable or the specific information is demonstrably missing after thorough review of the provided link.

## Key Components

**LunchMoneyApi Trait**: The primary interface for interacting with Lunch Money. It is auto-mockable via `mockall` for testing.

**LunchMoneyClient**: The production implementation using `reqwest::blocking`.

## Non-obvious behaviour

**Automatic Pagination**: `get_all_transactions` automatically follows `has_more` and `offset` flags to fetch the entire result set in a single call.

**Partial Updates**: `put_transactions` uses `UpdateTransactionDto` which only serializes `Some` fields, allowing for surgical updates to existing transactions.

**Delete Semantics**: bulk delete (`DELETE /transactions` with `{ ids }`) is treated as success on HTTP `204 No Content`.

**Error Handling**: Uses `rootcause` for context-aware error reporting, capturing both HTTP status codes and response bodies on failure.
