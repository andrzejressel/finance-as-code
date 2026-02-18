# setup/github — Agents Guide

Pulumi IaC program managing GitHub repo settings (branch protection, labels). Deploy with `just run`.

## Non-obvious behaviour

**`src/github.rs` is generated**: produced by `build.rs` at compile time via `pulumi_gestalt_build::generate("github", "5.26.0")`. Do not edit it manually.

**Required status checks are auto-discovered**: `github_workflow.rs` parses `.github/workflows/ci.yml` and expands matrix jobs into their full names. Adding a new CI job automatically makes it a required check on the next `just run` — no manual update needed here.
