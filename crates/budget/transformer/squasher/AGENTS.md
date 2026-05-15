# budget/transformer/squasher — Agents Guide

This crate provides a batch transaction transformer that replaces all transactions in a date range with one aggregated transaction.

## Non-obvious behaviour

**Inclusive range**: squashing matches transactions where `from_date <= transaction.date <= to_date`.

**Full replacement**: matching transactions are removed and replaced by exactly one synthetic transaction, so the total amount is preserved.

**Currency safety**: if matching transactions contain mixed currencies, the transformer does not squash and returns the original input unchanged.
