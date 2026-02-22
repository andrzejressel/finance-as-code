# utils/hmap — Agents Guide

Runtime heterogeneous map keyed by user-defined key type (`K`). Values are stored as `Any` and recovered through typed accessors.

## Non-obvious behaviour

**Typed lookups panic on mismatch**: `get::<T>`, `get_mut::<T>`, and `remove::<T>` return `None` only when the key is missing. If the key exists with a different concrete type, these methods panic.

**`insert` panics on mismatch**: `insert::<T>` returns `Some(previous)` when replacing an existing value of the same concrete type `T`; replacing a different type panics.
