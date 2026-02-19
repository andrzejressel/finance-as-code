use crate::ApiKey;
use crate::dto::{
    DeleteTransactionsRequest, GetTransactionsParams, ManualAccountDto, PostTransactionsRequest,
    PostTransactionsResponse, PutTransactionsRequest, PutTransactionsResponse, TransactionDto,
};
use log::warn;
use reqwest::StatusCode;
use reqwest::header::{HeaderMap, RETRY_AFTER};
use rootcause::prelude::ResultExt;
use rootcause::{Result, bail};
use std::time::Duration;

const MAX_RATE_LIMIT_RETRIES: u32 = 3;
const DEFAULT_RETRY_AFTER_SECS: u64 = 60;

/// Single-page response used internally by the pagination loop.
#[derive(serde::Deserialize)]
struct PageResponse {
    transactions: Vec<TransactionDto>,
    has_more: bool,
}

/// Response envelope for `GET /manual_accounts`.
#[derive(serde::Deserialize)]
struct ManualAccountsResponse {
    manual_accounts: Vec<ManualAccountDto>,
}

/// Abstraction over the Lunch Money v2 `/transactions` endpoints.
///
/// A `MockLunchMoneyApi` is generated automatically in test builds via `mockall`.
#[cfg_attr(test, mockall::automock)]
pub trait LunchMoneyApi {
    /// Fetch all manual accounts (`GET /manual_accounts`).
    fn get_all_manual_accounts(&self) -> Result<Vec<ManualAccountDto>>;

    /// Fetch every matching transaction, following `has_more` / `offset` pagination
    /// automatically until the full result set has been retrieved.
    fn get_all_transactions(&self, params: &GetTransactionsParams) -> Result<Vec<TransactionDto>>;

    /// Update up to 500 transactions in a single call (`PUT /transactions`).
    fn put_transactions(&self, request: &PutTransactionsRequest)
    -> Result<PutTransactionsResponse>;

    /// Insert up to 500 transactions in a single call (`POST /transactions`).
    fn post_transactions(
        &self,
        request: &PostTransactionsRequest,
    ) -> Result<PostTransactionsResponse>;

    /// Delete multiple transactions in one call (`DELETE /transactions`).
    fn delete_transactions(&self, request: &DeleteTransactionsRequest) -> Result<()>;
}

/// Real HTTP client backed by the Lunch Money v2 REST API.
pub struct LunchMoneyClient {
    base_url: String,
    api_key: ApiKey,
    client: reqwest::blocking::Client,
}

impl LunchMoneyClient {
    pub fn new(base_url: String, api_key: ApiKey) -> Self {
        Self {
            base_url,
            api_key,
            client: reqwest::blocking::Client::new(),
        }
    }

    fn send_with_rate_limit_retry<F>(
        &self,
        mut build_request: F,
        operation: &str,
    ) -> Result<reqwest::blocking::Response>
    where
        F: FnMut() -> reqwest::blocking::RequestBuilder,
    {
        for attempt in 0..=MAX_RATE_LIMIT_RETRIES {
            let response = build_request()
                .send()
                .context_with(|| format!("Failed to send {operation} request"))?;

            if response.status() != StatusCode::TOO_MANY_REQUESTS {
                return Ok(response);
            }

            let retry_after_seconds = parse_retry_after_seconds(response.headers());
            let attempt_number = attempt + 1;
            let total_attempts = MAX_RATE_LIMIT_RETRIES + 1;

            if attempt == MAX_RATE_LIMIT_RETRIES {
                warn!(
                    "{operation} hit rate limit (attempt {attempt_number}/{total_attempts}); no retries left"
                );
                return Ok(response);
            }

            warn!(
                "{operation} hit rate limit (attempt {attempt_number}/{total_attempts}); retrying in {retry_after_seconds}s"
            );

            std::thread::sleep(Duration::from_secs(retry_after_seconds));
        }

        unreachable!("retry loop must always return")
    }

    fn fetch_page(&self, params: &GetTransactionsParams) -> Result<PageResponse> {
        let response = self.send_with_rate_limit_retry(
            || {
                self.client
                    .get(format!("{}/transactions", self.base_url))
                    .header("Authorization", format!("Bearer {}", self.api_key.value()))
                    .query(params)
            },
            "GET /transactions",
        )?;

        if response.status().is_success() {
            Ok(response
                .json::<PageResponse>()
                .context("Failed to deserialize GET /transactions response")?)
        } else {
            let status = response.status();
            let headers = format_headers(response.headers());
            let body = response.text().context_with(|| {
                format!(
                    "Failed to read error body from GET /transactions (status {})",
                    status
                )
            })?;
            bail!(
                "GET /transactions returned error: {} | headers: {} | body: {}",
                status,
                headers,
                body
            );
        }
    }
}

fn parse_retry_after_seconds(headers: &HeaderMap) -> u64 {
    headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_RETRY_AFTER_SECS)
}

fn format_headers(headers: &HeaderMap) -> String {
    headers
        .iter()
        .map(|(name, value)| format!("{}={}", name, value.to_str().unwrap_or("<non-UTF8>")))
        .collect::<Vec<_>>()
        .join(", ")
}

impl LunchMoneyApi for LunchMoneyClient {
    fn get_all_manual_accounts(&self) -> Result<Vec<ManualAccountDto>> {
        let response = self.send_with_rate_limit_retry(
            || {
                self.client
                    .get(format!("{}/manual_accounts", self.base_url))
                    .header("Authorization", format!("Bearer {}", self.api_key.value()))
            },
            "GET /manual_accounts",
        )?;

        if response.status().is_success() {
            Ok(response
                .json::<ManualAccountsResponse>()
                .context("Failed to deserialize GET /manual_accounts response")?
                .manual_accounts)
        } else {
            let status = response.status();
            let headers = format_headers(response.headers());
            let body = response.text().context_with(|| {
                format!(
                    "Failed to read error body from GET /manual_accounts (status {})",
                    status
                )
            })?;
            bail!(
                "GET /manual_accounts returned error: {} | headers: {} | body: {}",
                status,
                headers,
                body
            );
        }
    }

    fn get_all_transactions(&self, params: &GetTransactionsParams) -> Result<Vec<TransactionDto>> {
        let mut all_transactions = Vec::new();
        let limit = params.limit.unwrap_or(1000);
        let mut offset = params.offset.unwrap_or(0);

        loop {
            let page_params = GetTransactionsParams {
                start_date: params.start_date.clone(),
                end_date: params.end_date.clone(),
                manual_account_id: params.manual_account_id,
                limit: Some(limit),
                offset: Some(offset),
            };

            let response = self.fetch_page(&page_params)?;
            all_transactions.extend(response.transactions);

            if !response.has_more {
                break;
            }

            offset += limit;
        }

        Ok(all_transactions)
    }

    fn put_transactions(
        &self,
        request: &PutTransactionsRequest,
    ) -> Result<PutTransactionsResponse> {
        let response = self.send_with_rate_limit_retry(
            || {
                self.client
                    .put(format!("{}/transactions", self.base_url))
                    .header("Authorization", format!("Bearer {}", self.api_key.value()))
                    .json(request)
            },
            "PUT /transactions",
        )?;

        if response.status().is_success() {
            Ok(response
                .json::<PutTransactionsResponse>()
                .context("Failed to deserialize PUT /transactions response")?)
        } else {
            let status = response.status();
            let headers = format_headers(response.headers());
            let body = response.text().context_with(|| {
                format!(
                    "Failed to read error body from PUT /transactions (status {})",
                    status
                )
            })?;
            bail!(
                "PUT /transactions returned error: {} | headers: {} | body: {}",
                status,
                headers,
                body
            );
        }
    }

    fn post_transactions(
        &self,
        request: &PostTransactionsRequest,
    ) -> Result<PostTransactionsResponse> {
        let response = self.send_with_rate_limit_retry(
            || {
                self.client
                    .post(format!("{}/transactions", self.base_url))
                    .header("Authorization", format!("Bearer {}", self.api_key.value()))
                    .json(request)
            },
            "POST /transactions",
        )?;

        if response.status().is_success() {
            Ok(response
                .json::<PostTransactionsResponse>()
                .context("Failed to deserialize POST /transactions response")?)
        } else {
            let status = response.status();
            let headers = format_headers(response.headers());
            let body = response.text().context_with(|| {
                format!(
                    "Failed to read error body from POST /transactions (status {})",
                    status
                )
            })?;
            bail!(
                "POST /transactions returned error: {} | headers: {} | body: {}",
                status,
                headers,
                body
            );
        }
    }

    fn delete_transactions(&self, request: &DeleteTransactionsRequest) -> Result<()> {
        let response = self.send_with_rate_limit_retry(
            || {
                self.client
                    .delete(format!("{}/transactions", self.base_url))
                    .header("Authorization", format!("Bearer {}", self.api_key.value()))
                    .json(request)
            },
            "DELETE /transactions",
        )?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let headers = format_headers(response.headers());
            let body = response.text().context_with(|| {
                format!(
                    "Failed to read error body from DELETE /transactions (status {})",
                    status
                )
            })?;
            bail!(
                "DELETE /transactions returned error: {} | headers: {} | body: {}",
                status,
                headers,
                body
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ApiKey;
    use crate::dto::{
        DeleteTransactionsRequest, GetTransactionsParams, InsertTransactionDto, ManualAccountDto,
        PostTransactionsRequest, PostTransactionsResponse, PutTransactionsRequest,
        PutTransactionsResponse, TransactionDto, UpdateTransactionDto,
    };
    use googletest::prelude::*;
    use httpmock::MockServer;
    use reqwest::header::{HeaderMap, HeaderValue};
    use rust_decimal::dec;
    use serde_json::json;

    fn tx(id: i64, date: &str, payee: &str, notes: Option<&str>) -> TransactionDto {
        TransactionDto {
            id,
            date: date.to_string(),
            amount: dec!(100.0000),
            currency: "USD".to_string(),
            payee: payee.to_string(),
            notes: notes.map(str::to_string),
        }
    }

    #[test]
    fn get_all_manual_accounts_sends_request_and_parses_response() -> googletest::Result<()> {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/manual_accounts")
                .header("Authorization", "Bearer test_key");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!({
                    "manual_accounts": [
                        { "id": 1, "name": "Cash Wallet", "display_name": "Cash" },
                        { "id": 2, "name": "Savings Jar", "display_name": "Credit" }
                    ]
                }));
        });

        let client = LunchMoneyClient::new(server.url(""), ApiKey::new("test_key".to_string()));
        let accounts = client.get_all_manual_accounts().unwrap();

        verify_that!(
            accounts,
            eq(&vec![
                ManualAccountDto {
                    id: 1,
                    name: "Cash Wallet".to_string(),
                },
                ManualAccountDto {
                    id: 2,
                    name: "Savings Jar".to_string(),
                },
            ])
        )?;

        mock.assert();
        Ok(())
    }

    #[test]
    fn get_all_transactions_paginates_through_all_pages() -> googletest::Result<()> {
        let server = MockServer::start();

        let mock_page1 = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/transactions")
                .header("Authorization", "Bearer test_key")
                .query_param("manual_account_id", "42")
                .query_param("limit", "2")
                .query_param("offset", "0");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!({
                    "transactions": [
                        {"id": 1, "date": "2024-01-01", "amount": "100.0000",
                         "currency": "USD", "payee": "Payee 1", "notes": null},
                        {"id": 2, "date": "2024-01-02", "amount": "100.0000",
                         "currency": "USD", "payee": "Payee 2", "notes": null}
                    ],
                    "has_more": true
                }));
        });

        let mock_page2 = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/transactions")
                .header("Authorization", "Bearer test_key")
                .query_param("manual_account_id", "42")
                .query_param("limit", "2")
                .query_param("offset", "2");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!({
                    "transactions": [
                        {"id": 3, "date": "2024-01-03", "amount": "100.0000",
                         "currency": "USD", "payee": "Payee 3", "notes": null}
                    ],
                    "has_more": false
                }));
        });

        let client = LunchMoneyClient::new(server.url(""), ApiKey::new("test_key".to_string()));
        let transactions = client
            .get_all_transactions(&GetTransactionsParams {
                manual_account_id: Some(42),
                limit: Some(2),
                ..Default::default()
            })
            .unwrap();

        verify_that!(
            transactions,
            eq(&vec![
                tx(1, "2024-01-01", "Payee 1", None),
                tx(2, "2024-01-02", "Payee 2", None),
                tx(3, "2024-01-03", "Payee 3", None),
            ])
        )?;

        mock_page1.assert();
        mock_page2.assert();
        Ok(())
    }

    #[test]
    fn put_transactions_sends_and_receives_correctly() -> googletest::Result<()> {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::PUT)
                .path("/transactions")
                .header("Authorization", "Bearer test_key")
                .json_body(json!({
                    "transactions": [{"id": 1, "notes": "Updated notes"}]
                }));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!({
                    "transactions": [
                        {
                            "id": 1,
                            "date": "2024-01-01",
                            "amount": "100.0000",
                            "currency": "USD",
                            "payee": "Payee 1",
                            "notes": "Updated notes"
                        }
                    ]
                }));
        });

        let client = LunchMoneyClient::new(server.url(""), ApiKey::new("test_key".to_string()));
        let response = client
            .put_transactions(&PutTransactionsRequest {
                transactions: vec![UpdateTransactionDto {
                    id: 1,
                    date: None,
                    amount: None,
                    currency: None,
                    payee: None,
                    notes: Some("Updated notes".to_string()),
                }],
            })
            .unwrap();

        verify_that!(
            response,
            eq(&PutTransactionsResponse {
                transactions: vec![tx(1, "2024-01-01", "Payee 1", Some("Updated notes"))],
            })
        )?;

        mock.assert();
        Ok(())
    }

    #[test]
    fn delete_transactions_sends_ids_to_bulk_delete_endpoint() -> googletest::Result<()> {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path("/transactions")
                .header("Authorization", "Bearer test_key")
                .json_body(json!({"ids": [1, 2, 3]}));
            then.status(204);
        });

        let client = LunchMoneyClient::new(server.url(""), ApiKey::new("test_key".to_string()));
        client
            .delete_transactions(&DeleteTransactionsRequest { ids: vec![1, 2, 3] })
            .unwrap();

        mock.assert();
        Ok(())
    }

    #[test]
    fn post_transactions_sends_and_receives_correctly() -> googletest::Result<()> {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/transactions")
                .header("Authorization", "Bearer test_key")
                .json_body(json!({
                    "transactions": [
                        {
                            "date": "2024-01-01",
                            "amount": "100.0000",
                            "currency": "USD",
                            "payee": "Payee 1",
                            "manual_account_id": 7,
                            "external_id": "external_1"
                        }
                    ]
                }));
            then.status(201)
                .header("Content-Type", "application/json")
                .json_body(json!({
                    "transactions": [
                        {
                            "id": 1,
                            "date": "2024-01-01",
                            "amount": "100.0000",
                            "currency": "USD",
                            "payee": "Payee 1",
                            "notes": null
                        }
                    ],
                    "skipped_duplicates": []
                }));
        });

        let client = LunchMoneyClient::new(server.url(""), ApiKey::new("test_key".to_string()));
        let response = client
            .post_transactions(&PostTransactionsRequest {
                transactions: vec![InsertTransactionDto {
                    date: "2024-01-01".to_string(),
                    amount: dec!(100.0000),
                    currency: Some("USD".to_string()),
                    payee: Some("Payee 1".to_string()),
                    notes: None,
                    manual_account_id: Some(7),
                    external_id: Some("external_1".to_string()),
                }],
            })
            .unwrap();

        verify_that!(
            response,
            eq(&PostTransactionsResponse {
                transactions: vec![tx(1, "2024-01-01", "Payee 1", None)],
            })
        )?;

        mock.assert();
        Ok(())
    }

    #[test]
    fn parse_retry_after_seconds_parses_header_and_uses_default_fallbacks() -> googletest::Result<()>
    {
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, HeaderValue::from_static("17"));
        verify_that!(parse_retry_after_seconds(&headers), eq(17))?;

        headers.insert(RETRY_AFTER, HeaderValue::from_static("not-a-number"));
        verify_that!(
            parse_retry_after_seconds(&headers),
            eq(DEFAULT_RETRY_AFTER_SECS)
        )?;

        headers.remove(RETRY_AFTER);
        verify_that!(
            parse_retry_after_seconds(&headers),
            eq(DEFAULT_RETRY_AFTER_SECS)
        )?;
        Ok(())
    }

    #[test]
    fn get_all_manual_accounts_retries_rate_limited_requests() -> googletest::Result<()> {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/manual_accounts")
                .header("Authorization", "Bearer test_key");
            then.status(429)
                .header("Content-Type", "application/json")
                .header("Retry-After", "0")
                .json_body(json!({
                    "message": "Too Many Requests",
                    "errors": [{ "errMsg": "Too many requests, please try again later." }]
                }));
        });

        let client = LunchMoneyClient::new(server.url(""), ApiKey::new("test_key".to_string()));
        let error = client.get_all_manual_accounts().unwrap_err();
        let error_string = error.to_string();

        verify_that!(
            error_string.as_str(),
            contains_substring("GET /manual_accounts returned error: 429 Too Many Requests")
        )?;
        verify_that!(error_string.as_str(), contains_substring("retry-after=0"))?;
        mock.assert_calls((MAX_RATE_LIMIT_RETRIES + 1) as usize);
        Ok(())
    }
}
