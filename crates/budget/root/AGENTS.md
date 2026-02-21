# budget/root — Agents Guide

Public facade crate. Contains only re-exports and the `run()` orchestration function — no logic of its own.

## Non-obvious behaviour

**Lunchflow source ordering**: `create_lunchflow_downloader` must be listed before `LocalDirectorySource` in the sources vec. The downloader's `read()` writes a JSON file to disk as a side effect and returns an empty holder; only then will `LocalDirectorySource` find that file.

**Features prefixed with `__`** (`__sink_treeline__nonbundled_duckdb`, `__all_nonbundled_duckdb`) are internal combinators — do not use them directly from outside the workspace.

**Do not leak implementation details**: `budget/root` is a facade crate and must only re-export stable, user-facing APIs (constructors, configs, traits that are part of the public contract). Do not re-export internal helper services from lower-level crates (e.g. API-layer upload/deletion services).
