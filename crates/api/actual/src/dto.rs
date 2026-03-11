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

/// Generic response envelope used by create/delete transaction endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeneralResponseMessage {
    pub message: String,
}
