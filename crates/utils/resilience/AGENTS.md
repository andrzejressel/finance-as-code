# utils/resilience - Agents Guide

Shared retry helpers for transient failures.

## Non-obvious behaviour

`max_retries` means retries after the initial attempt. Total attempts are `max_retries + 1`.

Wait durations are exponential (`initial_wait`, `2x`, `4x`, ...) but capped at `max_wait`.
