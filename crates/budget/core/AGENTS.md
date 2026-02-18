# budget/core — Agents Guide

Central domain library. No I/O, no HTTP, no DuckDB.

## Non-obvious behaviour

**Stable ID generation** (`transaction_mapper.rs`): the per-description occurrence counter is keyed by description only, not by date or amount. If two sources emit the same description in different orders, their transactions get each other's IDs. Sources must produce transactions in a stable, deterministic order.

**TransactionHolder combine semantics**: when two holders overlap in date range, the second one's days overwrite the first's at day-level granularity — not row-level merge. A day present in both holders will contain only the second holder's transactions for that day.

**`FileReader` is auto-mockable**: `mockall::automock` is applied to the trait, so `MockFileReader` is available in tests without any manual mock implementation.
