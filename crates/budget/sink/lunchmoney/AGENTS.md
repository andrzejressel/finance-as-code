# budget/sink/lunchmoney — Agents Guide

Writes transactions to Lunch Money via the Lunch Money v2 API.

## Non-obvious behaviour

**Notes column**: Transaction descriptions are stored in a separate `notes` field in Lunch Money. The `to_insert_transaction` function maps `transaction.description` to `InsertTransactionDto.notes`. This keeps the original transaction description intact without affecting the main payee field.

**Destructive write**: `Sink::write()` deletes all existing transactions for the target account before uploading new ones. There is no merge or upsert — every run is a full replacement. This ensures consistency but means the sink should only be used for accounts fully managed by finance-as-code.

**Account scope**: The deletion only affects transactions in the specific manual account identified by `account_name`. Other Lunch Money accounts and transactions remain untouched. However, only use this sink for accounts that are fully managed via finance-as-code to avoid losing manually-entered transactions.

**Category mapping**: All category names referenced in transactions must exist in Lunch Money before running the sink. Unknown categories will cause the pipeline to fail with an error listing the missing category names.
