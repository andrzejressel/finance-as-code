# crates/api — Agents Guide

Shared guidance for low-level HTTP API client crates under `crates/api/*`.

## Common rate limit and retry policy

- **Default retry policy for API crates**: retry transient server failures (`HTTP 5xx`) using exponential backoff.
- **Default constants**:
  - retries: `3` (total attempts: `4`)
  - initial wait: `1s`
  - max wait: `30s`
- **Client errors (`HTTP 4xx`) are fatal by default** and should not be retried unless a crate documents a deliberate exception.
- Error messages should include status code, response headers, and response body to keep diagnostics actionable.

## Non-obvious behaviour

- This file defines the baseline policy. Individual API crates may extend it for provider-specific behavior and must document that extension in their own crate `AGENTS.md`.
