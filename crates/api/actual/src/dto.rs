use serde::{Deserialize, Serialize};

/// Transaction payload for `POST
/// /budgets/{budgetSyncId}/accounts/{accountId}/transactions`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransactionDto {
    pub account: String,
    pub date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payee_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imported_payee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imported_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transfer_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cleared: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtransactions: Option<Vec<TransactionDto>>,
}

/// Request body for creating a single transaction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateTransactionRequest {
    #[serde(rename = "learnCategories", skip_serializing_if = "Option::is_none")]
    pub learn_categories: Option<bool>,
    #[serde(rename = "runTransfers", skip_serializing_if = "Option::is_none")]
    pub run_transfers: Option<bool>,
    pub transaction: TransactionDto,
}

/// Request body for creating multiple transactions in a single call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateTransactionsBatchRequest {
    #[serde(rename = "learnCategories", skip_serializing_if = "Option::is_none")]
    pub learn_categories: Option<bool>,
    #[serde(rename = "runTransfers", skip_serializing_if = "Option::is_none")]
    pub run_transfers: Option<bool>,
    pub transactions: Vec<TransactionDto>,
}

/// Request body for importing transactions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImportTransactionsRequest {
    pub transactions: Vec<TransactionDto>,
}

/// Request body for deleting a batch of transactions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeleteTransactionsBatchRequest {
    #[serde(rename = "transactionIds")]
    pub transaction_ids: Vec<String>,
}

/// Response payload returned by import transactions endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImportTransactionsResult {
    pub added: Vec<String>,
    pub updated: Vec<String>,
}

/// Wrapper response for import transactions endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImportTransactionsResponse {
    #[serde(default)]
    pub data: Option<ImportTransactionsResult>,
    #[serde(default)]
    pub added: Vec<String>,
    #[serde(default)]
    pub updated: Vec<String>,
}

impl ImportTransactionsResponse {
    pub fn into_result(self) -> ImportTransactionsResult {
        if let Some(data) = self.data {
            return data;
        }
        ImportTransactionsResult {
            added: self.added,
            updated: self.updated,
        }
    }
}

/// Generic response envelope used by create/delete transaction endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeneralResponseMessage {
    pub message: String,
}
