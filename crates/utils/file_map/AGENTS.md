# utils/file_map — Agents Guide

File-backed string-to-string map with in-memory caching. Uses JSON for persistence.

## Non-obvious behaviour

**Empty files are initialized**: If the file exists but is empty, it is automatically initialized with an empty JSON object `{ "data": {} }`.

**Invalid JSON returns error**: If the file contains invalid JSON (and is not empty), `new()` returns a `rootcause::Result::Err` instead of panicking.

**Immediate persistence**: Every `put()` call writes the entire map to disk immediately. The in-memory cache is updated first, then the file is overwritten.

**Parent directories created automatically**: If the parent directory of the file path does not exist, it is created automatically.

## Usage pattern

```rust
use finance_as_code_utils_file_map::{FileStringMap, JsonFileMap};

let mut map = JsonFileMap::new("path/to/map.json")?;
map.put("key", "value")?;
let value = map.get("key"); // Option<&str>
```
