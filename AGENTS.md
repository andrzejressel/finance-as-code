# Agents Guide

## Environment Setup

This project uses [mise](https://mise.jdx.dev/) as the single tool version manager for all dev dependencies.

### Prerequisites

- **Rust** — install via [rustup](https://rustup.rs/). The toolchain version is pinned in `rust-toolchain.toml` and will be activated automatically by rustup.
- **mise** — install from https://mise.jdx.dev/getting-started.html.

### Bootstrap

```sh
mise install # Will install Just required for the next step
just install
```

`mise install` installs all tools declared in `.mise.toml` (including `just` itself). `just install` then installs the remaining Rust toolchain components.

> **Never use `latest` as a version in `.mise.toml`.** All tools must be pinned to an explicit version to ensure reproducible environments across machines and CI. When adding or upgrading a tool, look up the current release and pin it exactly.

### Environment variables

`.cargo/config.toml` sets the following env var for all Cargo invocations:

```toml
DUCKDB_DOWNLOAD_LIB = "1"
```

This forces DuckDB to download its native library at build time rather than linking against a system installation. No manual DuckDB installation is required.

## Verifying Your Work

After making changes, run `just fmt` to format and fix lints, then `just test` to run the full test suite. Both must pass before considering work done.
