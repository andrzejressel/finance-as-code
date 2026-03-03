use crate::ApiKey;
use crate::dto::{
    CategoryDto, CreateCategoryRequest, DeleteTransactionsRequest, GetTransactionsParams,
    ManualAccountDto, PostTransactionsRequest, PostTransactionsResponse, PutTransactionsRequest,
    PutTransactionsResponse, TransactionDto,
};
use finance_as_code_utils_resilience::{
    ExponentialBackoff, RetryError, retry_with_exponential_backoff_selective,
};
use log::warn;
use reqwest::StatusCode;
use reqwest::header::{HeaderMap, RETRY_AFTER};
use rootcause::prelude::ResultExt;
use rootcause::{Result, bail, report};
use std::time::Duration;

/// Number of times to retry a request that receives HTTP 429.
const RATE_LIMIT_RETRIES: u32 = 3;
/// Fallback sleep duration when no `Retry-After` header is present.
const RATE_LIMIT_DEFAULT_WAIT: Duration = Duration::from_secs(60);

/// Retries on any HTTP 4xx/5xx error response using exponential backoff.
const ERROR_RETRIES: u32 = 3;
const ERROR_INITIAL_WAIT: Duration = Duration::from_secs(1);
const ERROR_MAX_WAIT: Duration = Duration::from_secs(30);

fn default_error_backoff() -> ExponentialBackoff {
    ExponentialBackoff::new(ERROR_RETRIES, ERROR_INITIAL_WAIT, ERROR_MAX_WAIT)
}

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

/// Response envelope for `GET /categories`.
#[derive(serde::Deserialize)]
struct CategoriesResponse {
    categories: Vec<CategoryDto>,
}

/// Abstraction over the Lunch Money v2 `/transactions` endpoints.
///
/// A `MockLunchMoneyApi` is generated automatically in test builds via
/// `mockall`.
#[cfg_attr(any(test, feature = "mock"), mockall::automock)]
pub trait LunchMoneyApi {
    /// Fetch all manual accounts (`GET /manual_accounts`).
    fn get_all_manual_accounts(&self) -> Result<Vec<ManualAccountDto>>;

    /// Fetch all categories in nested format (`GET /categories?format=nested`).
    fn get_all_categories(&self) -> Result<Vec<CategoryDto>>;

    /// Create a category or category group (`POST /categories`).
    fn create_category(&self, request: &CreateCategoryRequest) -> Result<CategoryDto>;

    /// Fetch every matching transaction, following `has_more` / `offset`
    /// pagination automatically until the full result set has been
    /// retrieved.
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

    /// Force-delete a category or category group (`DELETE
    /// /categories/{id}?force=true`).
    fn delete_category(&self, id: i64) -> Result<()>;
}

/// Real HTTP client backed by the Lunch Money v2 REST API.
pub struct LunchMoneyClient {
    base_url: String,
    api_key: ApiKey,
    client: reqwest::blocking::Client,
    error_backoff: ExponentialBackoff,
    /// Pluggable sleep function used by the 429 retry loop. In production this
    /// is `std::thread::sleep`; in tests it can be replaced with a no-op.
    sleep_fn: Box<dyn Fn(Duration) + Send + Sync>,
}

impl LunchMoneyClient {
    pub fn new(base_url: String, api_key: ApiKey) -> Self {
        Self {
            base_url,
            api_key,
            client: reqwest::blocking::Client::builder()
                .timeout(Some(Duration::from_secs(120)))
                .build()
                .expect("Failed to build reqwest client"),
            error_backoff: default_error_backoff(),
            sleep_fn: Box::new(std::thread::sleep),
        }
    }

    /// Constructs a client with fast (near-zero) backoff durations for use in
    /// tests, where real waits would make the suite prohibitively slow.
    #[cfg(test)]
    fn new_for_test(base_url: String, api_key: ApiKey) -> Self {
        Self {
            base_url,
            api_key,
            client: reqwest::blocking::Client::builder()
                .timeout(Some(Duration::from_secs(120)))
                .build()
                .expect("Failed to build reqwest client"),
            error_backoff: ExponentialBackoff::new(
                ERROR_RETRIES,
                Duration::from_millis(1),
                Duration::from_millis(2),
            ),
            sleep_fn: Box::new(|_| {}),
        }
    }

    /// Inner layer: retries on HTTP 429 by sleeping the duration from the
    /// `Retry-After` response header (falling back to
    /// [`RATE_LIMIT_DEFAULT_WAIT`] when the header is absent or unparseable).
    /// Returns the response as soon as it is not a 429, or returns an error
    /// after [`RATE_LIMIT_RETRIES`] exhausted retries.
    fn send_with_rate_limit_retry<F>(
        &self,
        build_request: F,
        operation: &str,
    ) -> Result<reqwest::blocking::Response>
    where
        F: Fn() -> reqwest::blocking::RequestBuilder,
    {
        let total_attempts = RATE_LIMIT_RETRIES + 1;
        for attempt in 1..=total_attempts {
            let response = build_request()
                .send()
                .context_with(|| format!("Failed to send {operation} request"))?;

            if response.status() != StatusCode::TOO_MANY_REQUESTS {
                return Ok(response);
            }

            let retry_after = parse_retry_after_seconds(response.headers());
            let headers = format_headers(response.headers());

            if attempt == total_attempts {
                warn!(
                    "{operation} hit rate limit (attempt {attempt}/{total_attempts}): \
                     retry-after={retry_after}s | headers: {headers}; no retries left"
                );
                bail!(
                    "{operation} hit rate limit; retry-after={retry_after}s | headers: {headers}"
                );
            }

            warn!(
                "{operation} hit rate limit (attempt {attempt}/{total_attempts}): \
                 retry-after={retry_after}s | headers: {headers}; retrying after {retry_after}s"
            );
            (self.sleep_fn)(Duration::from_secs(retry_after));
        }

        unreachable!("retry loop must always return")
    }

    /// Outer layer: sends a request through the 429 inner loop, then retries
    /// the whole thing on HTTP 5xx server errors using exponential backoff
    /// (via the resilience crate).
    ///
    /// 4xx client errors are returned immediately as fatal (deterministic
    /// failures that retrying cannot fix). 429-exhaustion from the inner loop
    /// is also propagated immediately without further outer retries.
    fn send<F>(&self, build_request: F, operation: &str) -> Result<reqwest::blocking::Response>
    where
        F: Fn() -> reqwest::blocking::RequestBuilder,
    {
        retry_with_exponential_backoff_selective(
            &format!("{operation} (error)"),
            self.error_backoff,
            || {
                let response = self
                    .send_with_rate_limit_retry(&build_request, operation)
                    .map_err(RetryError::Fatal)?;

                if response.status().is_server_error() {
                    let status = response.status();
                    let headers = format_headers(response.headers());
                    let body = response.text().unwrap_or_default();
                    return Err(RetryError::Retryable(
                        report!(
                            "{operation} returned error: {status} | headers: {headers} | body: {body}"
                        ),
                    ));
                }
                if response.status().is_client_error() {
                    let status = response.status();
                    let headers = format_headers(response.headers());
                    let body = response.text().unwrap_or_default();
                    return Err(RetryError::Fatal(
                        report!(
                            "{operation} returned error: {status} | headers: {headers} | body: {body}"
                        ),
                    ));
                }
                Ok(response)
            },
        )
    }

    fn fetch_page(&self, params: &GetTransactionsParams) -> Result<PageResponse> {
        let response = self.send(
            || {
                self.client
                    .get(format!("{}/transactions", self.base_url))
                    .header("Authorization", format!("Bearer {}", self.api_key.value()))
                    .query(params)
            },
            "GET /transactions",
        )?;

        Ok(response
            .json::<PageResponse>()
            .context("Failed to deserialize GET /transactions response")?)
    }
}

fn parse_retry_after_seconds(headers: &HeaderMap) -> u64 {
    headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_else(|| RATE_LIMIT_DEFAULT_WAIT.as_secs())
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
        let response = self.send(
            || {
                self.client
                    .get(format!("{}/manual_accounts", self.base_url))
                    .header("Authorization", format!("Bearer {}", self.api_key.value()))
            },
            "GET /manual_accounts",
        )?;

        Ok(response
            .json::<ManualAccountsResponse>()
            .context("Failed to deserialize GET /manual_accounts response")?
            .manual_accounts)
    }

    fn get_all_transactions(&self, params: &GetTransactionsParams) -> Result<Vec<TransactionDto>> {
        let mut all_transactions = Vec::new();
        let limit = params.limit.unwrap_or(1000);
        let mut offset = params.offset.unwrap_or(0);

        loop {
            let page_params = GetTransactionsParams {
                start_date: params.start_date,
                end_date: params.end_date,
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

    fn get_all_categories(&self) -> Result<Vec<CategoryDto>> {
        let response = self.send(
            || {
                self.client
                    .get(format!("{}/categories", self.base_url))
                    .header("Authorization", format!("Bearer {}", self.api_key.value()))
                    .query(&[("format", "nested")])
            },
            "GET /categories",
        )?;

        Ok(response
            .json::<CategoriesResponse>()
            .context("Failed to deserialize GET /categories response")?
            .categories)
    }

    fn create_category(&self, request: &CreateCategoryRequest) -> Result<CategoryDto> {
        let response = self.send(
            || {
                self.client
                    .post(format!("{}/categories", self.base_url))
                    .header("Authorization", format!("Bearer {}", self.api_key.value()))
                    .json(request)
            },
            "POST /categories",
        )?;

        Ok(response
            .json::<CategoryDto>()
            .context("Failed to deserialize POST /categories response")?)
    }

    fn put_transactions(
        &self,
        request: &PutTransactionsRequest,
    ) -> Result<PutTransactionsResponse> {
        let response = self.send(
            || {
                self.client
                    .put(format!("{}/transactions", self.base_url))
                    .header("Authorization", format!("Bearer {}", self.api_key.value()))
                    .json(request)
            },
            "PUT /transactions",
        )?;

        Ok(response
            .json::<PutTransactionsResponse>()
            .context("Failed to deserialize PUT /transactions response")?)
    }

    fn post_transactions(
        &self,
        request: &PostTransactionsRequest,
    ) -> Result<PostTransactionsResponse> {
        let response = self.send(
            || {
                self.client
                    .post(format!("{}/transactions", self.base_url))
                    .header("Authorization", format!("Bearer {}", self.api_key.value()))
                    .json(request)
            },
            "POST /transactions",
        )?;

        Ok(response
            .json::<PostTransactionsResponse>()
            .context("Failed to deserialize POST /transactions response")?)
    }

    fn delete_transactions(&self, request: &DeleteTransactionsRequest) -> Result<()> {
        self.send(
            || {
                self.client
                    .delete(format!("{}/transactions", self.base_url))
                    .header("Authorization", format!("Bearer {}", self.api_key.value()))
                    .json(request)
            },
            "DELETE /transactions",
        )?;
        Ok(())
    }

    fn delete_category(&self, id: i64) -> Result<()> {
        self.send(
            || {
                self.client
                    .delete(format!("{}/categories/{}", self.base_url, id))
                    .header("Authorization", format!("Bearer {}", self.api_key.value()))
                    .query(&[("force", "true")])
            },
            &format!("DELETE /categories/{id}"),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ApiKey;
    use crate::dto::{
        CategoryDto, ChildCategoryDto, CreateCategoryRequest, DeleteTransactionsRequest,
        GetTransactionsParams, InsertTransactionDto, ManualAccountDto, PostTransactionsRequest,
        PostTransactionsResponse, PutTransactionsRequest, PutTransactionsResponse, TransactionDto,
        UpdateTransactionDto,
    };
    use chrono::{DateTime, FixedOffset};
    use finance_as_code_utils_chrono::date;
    use googletest::prelude::*;
    use httpmock::MockServer;
    use reqwest::header::{HeaderMap, HeaderValue};
    use rust_decimal::dec;
    use serde_json::json;

    fn tx(id: i64, date: chrono::NaiveDate, payee: &str, notes: Option<&str>) -> TransactionDto {
        TransactionDto {
            id,
            date,
            amount: dec!(100.0000),
            currency: "USD".to_string(),
            payee: payee.to_string(),
            notes: notes.map(str::to_string),
        }
    }

    fn dt(value: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(value).unwrap()
    }

    #[test]
    fn get_all_manual_accounts_sends_request_and_parses_response() {
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

        let client =
            LunchMoneyClient::new_for_test(server.url(""), ApiKey::new("test_key".to_string()));
        let accounts = client.get_all_manual_accounts().unwrap();

        assert_that!(
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
        );

        mock.assert();
    }

    #[test]
    fn get_all_transactions_paginates_through_all_pages() {
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

        let client =
            LunchMoneyClient::new_for_test(server.url(""), ApiKey::new("test_key".to_string()));
        let transactions = client
            .get_all_transactions(&GetTransactionsParams {
                manual_account_id: Some(42),
                limit: Some(2),
                ..Default::default()
            })
            .unwrap();

        assert_that!(
            transactions,
            eq(&vec![
                tx(1, date!(2024 - 01 - 01), "Payee 1", None),
                tx(2, date!(2024 - 01 - 02), "Payee 2", None),
                tx(3, date!(2024 - 01 - 03), "Payee 3", None),
            ])
        );

        mock_page1.assert();
        mock_page2.assert();
    }

    #[test]
    fn put_transactions_sends_and_receives_correctly() {
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

        let client =
            LunchMoneyClient::new_for_test(server.url(""), ApiKey::new("test_key".to_string()));
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

        assert_that!(
            response,
            eq(&PutTransactionsResponse {
                transactions: vec![tx(
                    1,
                    date!(2024 - 01 - 01),
                    "Payee 1",
                    Some("Updated notes")
                )],
            })
        );

        mock.assert();
    }

    #[test]
    fn delete_transactions_sends_ids_to_bulk_delete_endpoint() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path("/transactions")
                .header("Authorization", "Bearer test_key")
                .json_body(json!({"ids": [1, 2, 3]}));
            then.status(204);
        });

        let client =
            LunchMoneyClient::new_for_test(server.url(""), ApiKey::new("test_key".to_string()));
        client
            .delete_transactions(&DeleteTransactionsRequest { ids: vec![1, 2, 3] })
            .unwrap();

        mock.assert();
    }

    #[test]
    fn delete_category_force_sends_correct_request() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path("/categories/83")
                .query_param("force", "true")
                .header("Authorization", "Bearer test_key");
            then.status(204);
        });

        let client =
            LunchMoneyClient::new_for_test(server.url(""), ApiKey::new("test_key".to_string()));
        client.delete_category(83).unwrap();

        mock.assert();
    }

    #[test]
    fn delete_category_returns_error_on_failure() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path("/categories/543210")
                .query_param("force", "true")
                .header("Authorization", "Bearer test_key");
            then.status(404)
                .header("Content-Type", "application/json")
                .json_body(json!({
                    "message": "Not Found",
                    "errors": [{"errMsg": "There is no category with the id: 543210."}]
                }));
        });

        let client =
            LunchMoneyClient::new_for_test(server.url(""), ApiKey::new("test_key".to_string()));
        let error = client.delete_category(543210).unwrap_err();
        let error_string = error.to_string();

        assert_that!(
            error_string.as_str(),
            contains_substring("DELETE /categories/543210 returned error: 404 Not Found")
        );
        assert_that!(
            error_string.as_str(),
            contains_substring("content-type=application/json")
        );
        assert_that!(
            error_string.as_str(),
            contains_substring("There is no category with the id: 543210")
        );

        mock.assert();
    }

    #[test]
    fn post_transactions_sends_and_receives_correctly() {
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
                            "category_id": 501,
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

        let client =
            LunchMoneyClient::new_for_test(server.url(""), ApiKey::new("test_key".to_string()));
        let response = client
            .post_transactions(&PostTransactionsRequest {
                transactions: vec![InsertTransactionDto {
                    date: date!(2024 - 01 - 01),
                    amount: dec!(100.0000),
                    currency: Some("USD".to_string()),
                    payee: Some("Payee 1".to_string()),
                    category_id: Some(501),
                    notes: None,
                    manual_account_id: Some(7),
                    external_id: Some("external_1".to_string()),
                }],
            })
            .unwrap();

        assert_that!(
            response,
            eq(&PostTransactionsResponse {
                transactions: vec![tx(1, date!(2024 - 01 - 01), "Payee 1", None)],
            })
        );

        mock.assert();
    }

    #[test]
    fn parse_retry_after_seconds_parses_header_and_uses_default_fallbacks() {
        let fallback = RATE_LIMIT_DEFAULT_WAIT.as_secs();

        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, HeaderValue::from_static("17"));
        assert_that!(parse_retry_after_seconds(&headers), eq(17));

        headers.insert(RETRY_AFTER, HeaderValue::from_static("not-a-number"));
        assert_that!(parse_retry_after_seconds(&headers), eq(fallback));

        headers.remove(RETRY_AFTER);
        assert_that!(parse_retry_after_seconds(&headers), eq(fallback));
    }

    #[test]
    fn get_all_manual_accounts_retries_rate_limited_requests() {
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

        let client =
            LunchMoneyClient::new_for_test(server.url(""), ApiKey::new("test_key".to_string()));
        let error = client.get_all_manual_accounts().unwrap_err();
        let error_string = error.to_string();

        assert_that!(
            error_string.as_str(),
            contains_substring("GET /manual_accounts hit rate limit")
        );
        assert_that!(error_string.as_str(), contains_substring("retry-after=0s"));
        mock.assert_calls((RATE_LIMIT_RETRIES + 1) as usize);
    }

    #[test]
    fn get_all_categories_returns_nested_categories() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/categories")
                .header("Authorization", "Bearer test_key")
                .query_param("format", "nested");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!({
                    "categories": [
                        {
                            "id": 86,
                            "name": "Automobile",
                            "description": "Auto related categories",
                            "is_income": false,
                            "exclude_from_budget": false,
                            "exclude_from_totals": false,
                            "updated_at": "2025-02-28T09:49:03.238Z",
                            "created_at": "2025-01-28T09:49:03.238Z",
                            "is_group": true,
                            "group_id": null,
                            "children": [
                                {
                                    "id": 315174,
                                    "name": "Fuel",
                                    "description": "Fuel and gas expenses",
                                    "is_income": false,
                                    "exclude_from_budget": false,
                                    "exclude_from_totals": false,
                                    "updated_at": "2025-02-28T09:49:03.238Z",
                                    "created_at": "2025-01-28T09:49:03.238Z",
                                    "is_group": false,
                                    "group_id": 86,
                                    "archived": false,
                                    "archived_at": null,
                                    "order": 1,
                                    "collapsed": false
                                }
                            ],
                            "archived": false,
                            "archived_at": null,
                            "order": 2,
                            "collapsed": false
                        },
                        {
                            "id": 83,
                            "name": "Rent",
                            "description": "Monthly Rent",
                            "is_income": false,
                            "exclude_from_budget": false,
                            "exclude_from_totals": false,
                            "updated_at": "2025-02-28T09:49:03.225Z",
                            "created_at": "2025-01-28T09:49:03.225Z",
                            "is_group": false,
                            "group_id": null,
                            "children": [],
                            "archived": false,
                            "archived_at": null,
                            "order": 0,
                            "collapsed": false
                        }
                    ]
                }));
        });

        let client =
            LunchMoneyClient::new_for_test(server.url(""), ApiKey::new("test_key".to_string()));
        let categories = client.get_all_categories().unwrap();

        assert_that!(
            categories,
            eq(&vec![
                CategoryDto {
                    id: 86,
                    name: "Automobile".to_string(),
                    description: Some("Auto related categories".to_string()),
                    is_income: false,
                    exclude_from_budget: false,
                    exclude_from_totals: false,
                    updated_at: dt("2025-02-28T09:49:03.238Z"),
                    created_at: dt("2025-01-28T09:49:03.238Z"),
                    is_group: true,
                    group_id: None,
                    children: vec![ChildCategoryDto {
                        id: 315174,
                        name: "Fuel".to_string(),
                        description: Some("Fuel and gas expenses".to_string()),
                        is_income: false,
                        exclude_from_budget: false,
                        exclude_from_totals: false,
                        updated_at: dt("2025-02-28T09:49:03.238Z"),
                        created_at: dt("2025-01-28T09:49:03.238Z"),
                        is_group: false,
                        group_id: Some(86),
                        archived: false,
                        archived_at: None,
                        order: Some(1),
                        collapsed: false,
                    }],
                    archived: false,
                    archived_at: None,
                    order: Some(2),
                    collapsed: false,
                },
                CategoryDto {
                    id: 83,
                    name: "Rent".to_string(),
                    description: Some("Monthly Rent".to_string()),
                    is_income: false,
                    exclude_from_budget: false,
                    exclude_from_totals: false,
                    updated_at: dt("2025-02-28T09:49:03.225Z"),
                    created_at: dt("2025-01-28T09:49:03.225Z"),
                    is_group: false,
                    group_id: None,
                    children: vec![],
                    archived: false,
                    archived_at: None,
                    order: Some(0),
                    collapsed: false,
                }
            ])
        );

        mock.assert();
    }

    #[test]
    fn get_all_categories_preserves_timezone_offsets() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/categories")
                .header("Authorization", "Bearer test_key")
                .query_param("format", "nested");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!({
                    "categories": [
                        {
                            "id": 1,
                            "name": "Timezone Test",
                            "description": null,
                            "is_income": false,
                            "exclude_from_budget": false,
                            "exclude_from_totals": false,
                            "updated_at": "2025-06-15T10:00:00+00:00",
                            "created_at": "2025-01-01T15:30:00+05:30",
                            "is_group": true,
                            "group_id": null,
                            "children": [
                                {
                                    "id": 2,
                                    "name": "Child",
                                    "description": null,
                                    "is_income": false,
                                    "exclude_from_budget": false,
                                    "exclude_from_totals": false,
                                    "updated_at": "2025-04-20T18:00:00+09:00",
                                    "created_at": "2025-04-19T08:15:00-04:00",
                                    "is_group": false,
                                    "group_id": 1,
                                    "archived": true,
                                    "archived_at": "2025-03-10T08:45:00-03:00",
                                    "order": null,
                                    "collapsed": false
                                }
                            ],
                            "archived": false,
                            "archived_at": null,
                            "order": null,
                            "collapsed": true
                        }
                    ]
                }));
        });

        let client =
            LunchMoneyClient::new_for_test(server.url(""), ApiKey::new("test_key".to_string()));
        let categories = client.get_all_categories().unwrap();

        assert_that!(categories[0].updated_at.offset().local_minus_utc(), eq(0));
        assert_that!(
            categories[0].created_at.offset().local_minus_utc(),
            eq(5 * 3600 + 30 * 60)
        );
        assert_that!(
            categories[0].children[0]
                .updated_at
                .offset()
                .local_minus_utc(),
            eq(9 * 3600)
        );
        assert_that!(
            categories[0].children[0]
                .archived_at
                .as_ref()
                .unwrap()
                .offset()
                .local_minus_utc(),
            eq(-(3 * 3600))
        );

        mock.assert();
    }

    #[test]
    fn get_all_categories_returns_error_on_failure() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/categories")
                .header("Authorization", "Bearer test_key")
                .query_param("format", "nested");
            then.status(401)
                .header("Content-Type", "application/json")
                .json_body(json!({
                    "message": "Unauthorized"
                }));
        });

        let client =
            LunchMoneyClient::new_for_test(server.url(""), ApiKey::new("test_key".to_string()));
        let error = client.get_all_categories().unwrap_err();
        let error_string = error.to_string();

        assert_that!(
            error_string.as_str(),
            contains_substring("GET /categories returned error: 401 Unauthorized")
        );
        assert_that!(
            error_string.as_str(),
            contains_substring("content-type=application/json")
        );
        assert_that!(error_string.as_str(), contains_substring("Unauthorized"));

        mock.assert();
    }

    #[test]
    fn get_all_categories_handles_missing_children_field() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/categories")
                .header("Authorization", "Bearer test_key")
                .query_param("format", "nested");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!({
                    "categories": [
                        {
                            "id": 42,
                            "name": "Food",
                            "description": null,
                            "is_income": false,
                            "exclude_from_budget": false,
                            "exclude_from_totals": false,
                            "updated_at": "2025-02-28T09:49:03.225Z",
                            "created_at": "2025-01-28T09:49:03.225Z",
                            "is_group": false,
                            "group_id": null,
                            "archived": false,
                            "archived_at": null,
                            "order": 0,
                            "collapsed": false
                        }
                    ]
                }));
        });

        let client =
            LunchMoneyClient::new_for_test(server.url(""), ApiKey::new("test_key".to_string()));
        let categories = client.get_all_categories().unwrap();

        assert_that!(
            categories,
            eq(&vec![CategoryDto {
                id: 42,
                name: "Food".to_string(),
                description: None,
                is_income: false,
                exclude_from_budget: false,
                exclude_from_totals: false,
                updated_at: dt("2025-02-28T09:49:03.225Z"),
                created_at: dt("2025-01-28T09:49:03.225Z"),
                is_group: false,
                group_id: None,
                children: vec![],
                archived: false,
                archived_at: None,
                order: Some(0),
                collapsed: false,
            }])
        );

        mock.assert();
    }

    #[test]
    fn create_category_sends_request_and_parses_response() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/categories")
                .header("Authorization", "Bearer test_key")
                .json_body(json!({
                    "name": "API Created Category",
                    "description": "Test description of created category",
                    "is_income": false,
                    "exclude_from_budget": true,
                    "exclude_from_totals": false,
                    "is_group": false
                }));
            then.status(201)
                .header("Content-Type", "application/json")
                .json_body(json!({
                    "id": 90,
                    "name": "API Created Category",
                    "description": "Test description of created category",
                    "is_income": false,
                    "exclude_from_budget": true,
                    "exclude_from_totals": false,
                    "updated_at": "2025-05-26T19:56:52.699Z",
                    "created_at": "2025-05-26T19:56:52.699Z",
                    "is_group": false,
                    "group_id": null,
                    "archived": false,
                    "archived_at": null,
                    "order": null,
                    "collapsed": false
                }));
        });

        let client =
            LunchMoneyClient::new_for_test(server.url(""), ApiKey::new("test_key".to_string()));
        let category = client
            .create_category(&CreateCategoryRequest {
                name: "API Created Category".to_string(),
                description: Some("Test description of created category".to_string()),
                is_income: Some(false),
                exclude_from_budget: Some(true),
                exclude_from_totals: Some(false),
                is_group: Some(false),
                group_id: None,
                order: None,
            })
            .unwrap();

        assert_that!(
            category,
            eq(&CategoryDto {
                id: 90,
                name: "API Created Category".to_string(),
                description: Some("Test description of created category".to_string()),
                is_income: false,
                exclude_from_budget: true,
                exclude_from_totals: false,
                updated_at: dt("2025-05-26T19:56:52.699Z"),
                created_at: dt("2025-05-26T19:56:52.699Z"),
                is_group: false,
                group_id: None,
                children: vec![],
                archived: false,
                archived_at: None,
                order: None,
                collapsed: false,
            })
        );

        mock.assert();
    }

    #[test]
    fn create_category_group_sends_request_and_parses_response() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/categories")
                .header("Authorization", "Bearer test_key")
                .json_body(json!({
                    "name": "API Created Category Group",
                    "is_group": true
                }));
            then.status(201)
                .header("Content-Type", "application/json")
                .json_body(json!({
                    "id": 91,
                    "name": "API Created Category Group",
                    "description": null,
                    "is_income": false,
                    "exclude_from_budget": false,
                    "exclude_from_totals": false,
                    "updated_at": "2025-05-27T19:59:45.053Z",
                    "created_at": "2025-05-27T19:59:45.053Z",
                    "is_group": true,
                    "group_id": null,
                    "archived": false,
                    "archived_at": null,
                    "order": null,
                    "collapsed": false,
                    "children": [
                        {
                            "id": 83,
                            "name": "Rent",
                            "description": "Monthly Rent",
                            "is_income": false,
                            "exclude_from_budget": false,
                            "exclude_from_totals": false,
                            "updated_at": "2025-02-28T09:49:03.225Z",
                            "created_at": "2025-01-28T09:49:03.225Z",
                            "is_group": false,
                            "group_id": 91,
                            "archived": false,
                            "archived_at": null,
                            "order": 1,
                            "collapsed": false
                        },
                        {
                            "id": 315174,
                            "name": "Fuel",
                            "description": null,
                            "is_income": false,
                            "exclude_from_budget": false,
                            "exclude_from_totals": false,
                            "updated_at": "2025-05-27T19:59:45.053Z",
                            "created_at": "2025-05-27T19:59:45.053Z",
                            "is_group": false,
                            "group_id": 91,
                            "archived": false,
                            "archived_at": null,
                            "order": 2,
                            "collapsed": false
                        }
                    ]
                }));
        });

        let client =
            LunchMoneyClient::new_for_test(server.url(""), ApiKey::new("test_key".to_string()));
        let category_group = client
            .create_category(&CreateCategoryRequest {
                name: "API Created Category Group".to_string(),
                description: None,
                is_income: None,
                exclude_from_budget: None,
                exclude_from_totals: None,
                is_group: Some(true),
                group_id: None,
                order: None,
            })
            .unwrap();

        assert_that!(category_group.id, eq(91));
        assert_that!(category_group.is_group, eq(true));
        assert_that!(category_group.children.len(), eq(2));
        assert_that!(category_group.children[0].id, eq(83));
        assert_that!(category_group.children[1].name.as_str(), eq("Fuel"));

        mock.assert();
    }

    #[test]
    fn create_category_with_group_id_sends_request_and_parses_response() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/categories")
                .header("Authorization", "Bearer test_key")
                .json_body(json!({
                    "name": "Fuel",
                    "group_id": 91
                }));
            then.status(201)
                .header("Content-Type", "application/json")
                .json_body(json!({
                    "id": 315174,
                    "name": "Fuel",
                    "description": null,
                    "is_income": false,
                    "exclude_from_budget": false,
                    "exclude_from_totals": false,
                    "updated_at": "2025-05-27T19:59:45.053Z",
                    "created_at": "2025-05-27T19:59:45.053Z",
                    "is_group": false,
                    "group_id": 91,
                    "archived": false,
                    "archived_at": null,
                    "order": 2,
                    "collapsed": false
                }));
        });

        let client =
            LunchMoneyClient::new_for_test(server.url(""), ApiKey::new("test_key".to_string()));
        let category = client
            .create_category(&CreateCategoryRequest {
                name: "Fuel".to_string(),
                description: None,
                is_income: None,
                exclude_from_budget: None,
                exclude_from_totals: None,
                is_group: None,
                group_id: Some(91),
                order: None,
            })
            .unwrap();

        assert_that!(category.id, eq(315174));
        assert_that!(category.group_id, some(eq(91)));
        assert_that!(category.is_group, eq(false));

        mock.assert();
    }

    #[test]
    fn create_category_returns_error_on_failure() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/categories")
                .header("Authorization", "Bearer test_key")
                .json_body(json!({
                    "name": "Bad Category Group",
                    "is_group": true,
                    "group_id": 83
                }));
            then.status(400)
                .header("Content-Type", "application/json")
                .json_body(json!({
                    "message": "Invalid Request Body",
                    "errors": [
                        {
                            "errMsg": "Cannot specify a 'group_id' in request body if 'is_group' is also true"
                        }
                    ]
                }));
        });

        let client =
            LunchMoneyClient::new_for_test(server.url(""), ApiKey::new("test_key".to_string()));
        let error = client
            .create_category(&CreateCategoryRequest {
                name: "Bad Category Group".to_string(),
                description: None,
                is_income: None,
                exclude_from_budget: None,
                exclude_from_totals: None,
                is_group: Some(true),
                group_id: Some(83),
                order: None,
            })
            .unwrap_err();
        let error_string = error.to_string();

        assert_that!(
            error_string.as_str(),
            contains_substring("POST /categories returned error: 400 Bad Request")
        );
        assert_that!(
            error_string.as_str(),
            contains_substring("content-type=application/json")
        );
        assert_that!(
            error_string.as_str(),
            contains_substring("Cannot specify a 'group_id' in request body")
        );

        mock.assert();
    }
}
