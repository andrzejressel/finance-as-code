# api/actual — Agents Guide

Low-level client for the Actual HTTP API wrapper.

Primary schema source in this repository: `swagger.json` (synced from the `jhonderson/actual-http-api:26.3.0` Docker image via `skills/actual-swagger-sync`).

## Non-obvious behaviour

- **Authentication model**: requests use `x-api-key` header. Budget scope is path-based via `budgetSyncId`.
- **Optional encryption header**: `budget-encryption-password` is optional and only sent when configured.
- **Create transaction response**: create endpoint returns a general message (for example `"ok"`), not created transaction ids.
- **Retry behavior**: this crate follows the shared `crates/api` baseline policy (retry `HTTP 5xx` with exponential backoff). It does not implement provider-specific `429` handling.

## Schema refresh

- **Local schema path**: `crates/api/actual/swagger.json`.
- **Update skill**: use `skills/actual-swagger-sync` to refresh the schema from `jhonderson/actual-http-api:26.3.0`.