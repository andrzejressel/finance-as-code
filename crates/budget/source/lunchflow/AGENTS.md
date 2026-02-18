# budget/source/lunchflow — Agents Guide

Downloads and parses Lunchflow API transactions.

## Non-obvious behaviour

**Two-phase design**: the downloader `Source` exists solely for its side effect — it writes `{epoch_seconds}_lunchflow_transactions.json` to disk and returns an empty `TransactionHolder`. A separate `LocalDirectorySource` with `create_lunchflow_file_reader()` must follow it in the sources list to actually read those files.

**Amount precision**: the API returns amounts as `f64`. The file reader converts them to `Decimal` before constructing `Money` to avoid floating-point precision loss. Do not skip this step if modifying the reader.

**Snapshot tests**: update with `just update-test-snapshots`.
