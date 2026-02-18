# api/lunchmoney — Agents Guide

Low-level client for the Lunch Money v2 API.

API Documentation: https://alpha.lunchmoney.dev/v2/docs

## Key Components

**LunchMoneyApi Trait**: The primary interface for interacting with Lunch Money. It is auto-mockable via `mockall` for testing.

**LunchMoneyClient**: The production implementation using `reqwest::blocking`.

## Non-obvious behaviour

**Automatic Pagination**: `get_all_transactions` automatically follows `has_more` and `offset` flags to fetch the entire result set in a single call.

**Partial Updates**: `put_transactions` uses `UpdateTransactionDto` which only serializes `Some` fields, allowing for surgical updates to existing transactions.

**Error Handling**: Uses `rootcause` for context-aware error reporting, capturing both HTTP status codes and response bodies on failure.
