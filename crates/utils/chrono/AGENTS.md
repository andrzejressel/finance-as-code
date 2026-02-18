# utils/chrono — Agents Guide

Compile-time `date!` and `datetime!` macros wrapping `chrono::NaiveDate` / `NaiveDateTime`. Invalid values are a **compile error**, not a runtime panic. Primarily a dev-dependency for readable date literals in tests.
