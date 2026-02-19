use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Minimal transaction as returned by `GET /transactions`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransactionDto {
    pub id: i64,
    pub date: String,
    pub amount: Decimal,
    pub currency: String,
    pub payee: String,
    pub notes: Option<String>,
}

/// Manual account as returned by `GET /manual_accounts`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManualAccountDto {
    pub id: i64,
    pub name: String,
}

/// Optional query parameters for `GET /transactions`.
///
/// All fields default to `None`; unset fields are omitted from the request.
#[derive(Debug, Clone, Default, Serialize)]
pub struct GetTransactionsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    /// Filter transactions by manual account id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manual_account_id: Option<i64>,
    /// Maximum number of transactions to return (1–2000, default 1000).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Offset into the result set; used for pagination together with `has_more`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
}

/// A single transaction entry for `POST /transactions` (bulk insert).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InsertTransactionDto {
    pub date: String,
    pub amount: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manual_account_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
}

/// Request body for `POST /transactions` (bulk insert, 1-500 transactions).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PostTransactionsRequest {
    pub transactions: Vec<InsertTransactionDto>,
}

/// Response envelope for `POST /transactions` (bulk insert).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PostTransactionsResponse {
    pub transactions: Vec<TransactionDto>,
}

/// A single transaction update entry for `PUT /transactions`.
///
/// Only `Some` fields are serialised and sent to the API; `None` fields are omitted.
/// `id` must identify an existing transaction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateTransactionDto {
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Request body for `PUT /transactions` (bulk update, 1–500 transactions).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PutTransactionsRequest {
    pub transactions: Vec<UpdateTransactionDto>,
}

/// Response envelope for `PUT /transactions` (bulk update).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PutTransactionsResponse {
    pub transactions: Vec<TransactionDto>,
}

/// Request body for `DELETE /transactions` (bulk delete).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeleteTransactionsRequest {
    pub ids: Vec<i64>,
}
