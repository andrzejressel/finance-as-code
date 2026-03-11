# api/actual — Agents Guide

Low-level client for the Actual HTTP API wrapper.

Primary schema source in this repository: `swagger.json` (downloaded from the private Actual API instance).

## Non-obvious behaviour

- **Authentication model**: requests use `x-api-key` header. Budget scope is path-based via `budgetSyncId`.
- **Optional encryption header**: `budget-encryption-password` is optional and only sent when configured.
- **Create transaction response**: create endpoint returns a general message (for example `"ok"`), not created transaction ids.
- **Retry behavior**: this crate follows the shared `crates/api` baseline policy (retry `HTTP 5xx` with exponential backoff). It does not implement provider-specific `429` handling.
