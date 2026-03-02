# budget/root — Agents Guide

Public facade crate. Contains only re-exports and the `run()` orchestration function — no logic of its own.

## Non-obvious behaviour

**Lunchflow source ordering**: `create_lunchflow_downloader` must be listed before `LocalDirectorySource` in the sources vec. The downloader's `read()` writes a JSON file to disk as a side effect and returns an empty holder; only then will `LocalDirectorySource` find that file.

**Do not leak implementation details**: `budget/root` is a facade crate and must only re-export stable, user-facing APIs (constructors, configs, traits that are part of the public contract). Do not re-export internal helper services from lower-level crates (e.g. API-layer upload/deletion services).

## Documentation guidelines

**User-facing docs should describe behavior, not internals**: in README and `src/docs/*.md`, avoid technical method names such as ``read()`` or ``write()`` unless strictly necessary to disambiguate API usage. Prefer plain language like "downloads data", "imports transactions", or "replaces existing account data".

**Keep examples approachable**: prioritize end-user flow and outcomes over implementation details. Mention side effects (for example, full replacement in Treeline) in product terms.

**Use a plain `main` in docs**: in README and `src/docs/*.md` Rust snippets, prefer `fn main() { ... }` (no return type). Handle fallible setup and `run(...)` with `expect(...)` and clear messages.

**Prefer `bon` builders over struct literals**: when a type exposes a `bon::Builder`, never construct that type directly with a struct literal in docs examples.
