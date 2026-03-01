# budget/source/lunchflow — Agents Guide

Downloads and parses Lunchflow API transactions.

## Non-obvious behaviour

**Two-phase design**: the downloader implements `Setup` (not `Source`) and exists solely for its side effect — it writes `{epoch_seconds}_lunchflow_transactions.json` to disk. It must be run as a setup before sources. A separate `LocalDirectorySource` with `create_lunchflow_file_reader()` reads those files as a source.

**Amount precision**: the API returns amounts as `f64`. The file reader converts them to `Decimal` before constructing `Money` to avoid floating-point precision loss. Do not skip this step if modifying the reader.

**Snapshot tests**: update with `just update-test-snapshots`.
