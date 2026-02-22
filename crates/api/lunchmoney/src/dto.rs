use chrono::{DateTime, FixedOffset, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Minimal transaction as returned by `GET /transactions`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransactionDto {
    pub id: i64,
    pub date: NaiveDate,
    pub amount: Decimal,
    pub currency: String,
    pub payee: String,
    pub notes: Option<String>,
}

/// Child category nested inside a category group, as returned by
/// `GET /categories?format=nested`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChildCategoryDto {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub is_income: bool,
    pub exclude_from_budget: bool,
    pub exclude_from_totals: bool,
    pub updated_at: DateTime<FixedOffset>,
    pub created_at: DateTime<FixedOffset>,
    pub is_group: bool,
    pub group_id: Option<i64>,
    pub archived: bool,
    pub archived_at: Option<DateTime<FixedOffset>>,
    pub order: Option<i64>,
    pub collapsed: bool,
}

/// Top-level category (group or standalone), as returned by
/// `GET /categories?format=nested`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CategoryDto {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub is_income: bool,
    pub exclude_from_budget: bool,
    pub exclude_from_totals: bool,
    pub updated_at: DateTime<FixedOffset>,
    pub created_at: DateTime<FixedOffset>,
    pub is_group: bool,
    pub group_id: Option<i64>,
    pub children: Vec<ChildCategoryDto>,
    pub archived: bool,
    pub archived_at: Option<DateTime<FixedOffset>>,
    pub order: Option<i64>,
    pub collapsed: bool,
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
    pub start_date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<NaiveDate>,
    /// Filter transactions by manual account id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manual_account_id: Option<i64>,
    /// Maximum number of transactions to return (1–2000, default 1000).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Offset into the result set; used for pagination together with
    /// `has_more`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
}

/// A single transaction entry for `POST /transactions` (bulk insert).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InsertTransactionDto {
    pub date: NaiveDate,
    pub amount: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_id: Option<i64>,
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
/// Only `Some` fields are serialised and sent to the API; `None` fields are
/// omitted. `id` must identify an existing transaction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateTransactionDto {
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<NaiveDate>,
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
