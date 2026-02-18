# budget/source/mbank — Agents Guide

Parses mBank CSV exports.

## Non-obvious behaviour

**Encoding**: the files are Windows-1250, not UTF-8. Do not open them as UTF-8 or Polish characters will corrupt.

**CSV start marker**: the actual data rows do not start at line 1. The parser scans for the `#Data księgowania` header line and treats everything below it as CSV. Lines before that header (including `#Waluta` for currency) are metadata, not data rows.

**Amount format**: numbers use a Polish locale format — spaces as thousands separators, comma as decimal separator (e.g. `1 234,56`). Standard Rust number parsing will fail on these.

**Snapshot tests**: update with `just update-test-snapshots`.
