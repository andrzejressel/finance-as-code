use crate::dto::{CreateTransactionRequest, GeneralResponseMessage};
use crate::{ApiKey, BudgetEncryptionPassword, BudgetSyncId};
use finance_as_code_utils_resilience::{
    ExponentialBackoff, RetryError, retry_with_exponential_backoff_selective,
};
use reqwest::blocking::RequestBuilder;
use rootcause::prelude::ResultExt;
use rootcause::{Result, report};
use std::time::Duration;

/// Retries on HTTP 5xx error responses using exponential backoff.
const ERROR_RETRIES: u32 = 3;
const ERROR_INITIAL_WAIT: Duration = Duration::from_secs(1);
const ERROR_MAX_WAIT: Duration = Duration::from_secs(30);

fn default_error_backoff() -> ExponentialBackoff {
    ExponentialBackoff::new(ERROR_RETRIES, ERROR_INITIAL_WAIT, ERROR_MAX_WAIT)
}

/// Configuration for [`ActualClient`].
#[derive(Debug, Clone)]
pub struct ActualClientConfig {
    pub base_url: String,
    pub api_key: ApiKey,
    pub budget_sync_id: BudgetSyncId,
    pub budget_encryption_password: Option<BudgetEncryptionPassword>,
}

impl ActualClientConfig {
    pub fn new(base_url: String, api_key: ApiKey, budget_sync_id: BudgetSyncId) -> Self {
        Self {
            base_url,
            api_key,
            budget_sync_id,
            budget_encryption_password: None,
        }
    }

    pub fn with_budget_encryption_password(
        mut self,
        budget_encryption_password: BudgetEncryptionPassword,
    ) -> Self {
        self.budget_encryption_password = Some(budget_encryption_password);
        self
    }
}

/// Abstraction over the subset of Actual transaction endpoints used by the
/// project.
///
/// A `MockActualApi` is generated automatically in test builds via `mockall`.
#[cfg_attr(any(test, feature = "mock"), mockall::automock)]
pub trait ActualApi {
    /// Create a single transaction in the provided account.
    fn create_transaction(
        &self,
        account_id: &str,
        request: &CreateTransactionRequest,
    ) -> Result<()>;

    /// Delete a single transaction by id.
    fn delete_transaction(&self, transaction_id: &str) -> Result<()>;
}

/// Real HTTP client backed by the Actual HTTP API.
pub struct ActualClient {
    config: ActualClientConfig,
    client: reqwest::blocking::Client,
    error_backoff: ExponentialBackoff,
}

impl ActualClient {
    pub fn new(config: ActualClientConfig) -> Self {
        Self {
            config,
            client: reqwest::blocking::Client::builder()
                .timeout(Some(Duration::from_secs(120)))
                .build()
                .expect("Failed to build reqwest client"),
            error_backoff: default_error_backoff(),
        }
    }

    /// Constructs a client with fast (near-zero) backoff durations for use in
    /// tests, where real waits would make the suite prohibitively slow.
    #[cfg(test)]
    fn new_for_test(config: ActualClientConfig) -> Self {
        Self {
            config,
            client: reqwest::blocking::Client::builder()
                .timeout(Some(Duration::from_secs(120)))
                .build()
                .expect("Failed to build reqwest client"),
            error_backoff: ExponentialBackoff::new(
                ERROR_RETRIES,
                Duration::from_millis(1),
                Duration::from_millis(2),
            ),
        }
    }

    fn with_common_headers(&self, request_builder: RequestBuilder) -> RequestBuilder {
        let request_builder = request_builder.header("x-api-key", self.config.api_key.value());
        if let Some(password) = &self.config.budget_encryption_password {
            return request_builder.header("budget-encryption-password", password.value());
        }
        request_builder
    }

    fn send<F>(&self, build_request: F, operation: &str) -> Result<reqwest::blocking::Response>
    where
        F: Fn() -> RequestBuilder,
    {
        retry_with_exponential_backoff_selective(
            &format!("{operation} (error)"),
            self.error_backoff,
            || {
                let response = build_request().send().map_err(|error| {
                    RetryError::Fatal(report!("Failed to send {operation} request: {error}"))
                })?;

                if response.status().is_server_error() {
                    let status = response.status();
                    let headers = format_headers(response.headers());
                    let body = response.text().unwrap_or_default();
                    return Err(RetryError::Retryable(report!(
                        "{operation} returned error: {status} | headers: {headers} | body: {body}"
                    )));
                }
                if response.status().is_client_error() {
                    let status = response.status();
                    let headers = format_headers(response.headers());
                    let body = response.text().unwrap_or_default();
                    return Err(RetryError::Fatal(report!(
                        "{operation} returned error: {status} | headers: {headers} | body: {body}"
                    )));
                }

                Ok(response)
            },
        )
    }
}

fn format_headers(headers: &reqwest::header::HeaderMap) -> String {
    headers
        .iter()
        .map(|(name, value)| format!("{}={}", name, value.to_str().unwrap_or("<non-UTF8>")))
        .collect::<Vec<_>>()
        .join(", ")
}

impl ActualApi for ActualClient {
    fn create_transaction(
        &self,
        account_id: &str,
        request: &CreateTransactionRequest,
    ) -> Result<()> {
        let operation = "POST /budgets/{budgetSyncId}/accounts/{accountId}/transactions";
        let response = self
            .send(
                || {
                    self.with_common_headers(self.client.post(format!(
                        "{}/budgets/{}/accounts/{}/transactions",
                        self.config.base_url,
                        self.config.budget_sync_id.value(),
                        account_id
                    )))
                    .json(request)
                },
                operation,
            )
            .context("failed to create transaction in Actual")?;

        let _response_message = response
            .json::<GeneralResponseMessage>()
            .context("failed to deserialize create transaction response")?;

        Ok(())
    }

    fn delete_transaction(&self, transaction_id: &str) -> Result<()> {
        let operation = "DELETE /budgets/{budgetSyncId}/transactions/{transactionId}";
        let response = self
            .send(
                || {
                    self.with_common_headers(self.client.delete(format!(
                        "{}/budgets/{}/transactions/{}",
                        self.config.base_url,
                        self.config.budget_sync_id.value(),
                        transaction_id
                    )))
                },
                operation,
            )
            .context("failed to delete transaction in Actual")?;

        let _response_message = response
            .json::<GeneralResponseMessage>()
            .context("failed to deserialize delete transaction response")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::TransactionDto;
    use googletest::prelude::*;
    use httpmock::Method::{DELETE, POST};
    use httpmock::MockServer;
    use serde_json::json;

    fn base_config(server: &MockServer) -> ActualClientConfig {
        ActualClientConfig::new(
            server.url(""),
            ApiKey::new("test_key".to_string()),
            BudgetSyncId::new("budget-sync-1".to_string()),
        )
    }

    #[test]
    fn create_transaction_sends_request_and_parses_response() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/budgets/budget-sync-1/accounts/account-1/transactions")
                .header("x-api-key", "test_key")
                .json_body(json!({
                    "learnCategories": false,
                    "runTransfers": false,
                    "transaction": {
                        "account": "account-1",
                        "date": "2026-03-10",
                        "amount": -1250,
                        "payee_name": "Coffee Shop",
                        "cleared": false
                    }
                }));
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!({ "message": "ok" }));
        });

        let client = ActualClient::new_for_test(base_config(&server));
        client
            .create_transaction(
                "account-1",
                &CreateTransactionRequest {
                    learn_categories: Some(false),
                    run_transfers: Some(false),
                    transaction: TransactionDto {
                        account: "account-1".to_string(),
                        date: "2026-03-10".to_string(),
                        amount: Some(-1250),
                        payee: None,
                        payee_name: Some("Coffee Shop".to_string()),
                        imported_payee: None,
                        category: None,
                        notes: None,
                        imported_id: None,
                        transfer_id: None,
                        cleared: Some(false),
                        subtransactions: None,
                    },
                },
            )
            .unwrap();

        mock.assert();
    }

    #[test]
    fn delete_transaction_sends_request_and_parses_response() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(DELETE)
                .path("/budgets/budget-sync-1/transactions/tx-1")
                .header("x-api-key", "test_key");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!({ "message": "Transaction deleted" }));
        });

        let client = ActualClient::new_for_test(base_config(&server));
        client.delete_transaction("tx-1").unwrap();

        mock.assert();
    }

    #[test]
    fn create_transaction_sends_encryption_header_when_configured() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/budgets/budget-sync-1/accounts/account-1/transactions")
                .header("x-api-key", "test_key")
                .header("budget-encryption-password", "secret");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!({ "message": "ok" }));
        });

        let client =
            ActualClient::new_for_test(base_config(&server).with_budget_encryption_password(
                BudgetEncryptionPassword::new("secret".to_string()),
            ));
        client
            .create_transaction(
                "account-1",
                &CreateTransactionRequest {
                    learn_categories: None,
                    run_transfers: None,
                    transaction: TransactionDto {
                        account: "account-1".to_string(),
                        date: "2026-03-10".to_string(),
                        amount: Some(-1000),
                        payee: None,
                        payee_name: Some("Store".to_string()),
                        imported_payee: None,
                        category: None,
                        notes: None,
                        imported_id: None,
                        transfer_id: None,
                        cleared: Some(true),
                        subtransactions: None,
                    },
                },
            )
            .unwrap();

        mock.assert();
    }

    #[test]
    fn create_transaction_does_not_send_encryption_header_when_unset() {
        let server = MockServer::start();
        let without_header_mock = server.mock(|when, then| {
            when.method(POST)
                .path("/budgets/budget-sync-1/accounts/account-1/transactions")
                .header("x-api-key", "test_key");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!({ "message": "ok" }));
        });
        let with_header_mock = server.mock(|when, then| {
            when.method(POST)
                .path("/budgets/budget-sync-1/accounts/account-1/transactions")
                .header("x-api-key", "test_key")
                .header("budget-encryption-password", "secret");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!({ "message": "ok" }));
        });

        let client = ActualClient::new_for_test(base_config(&server));
        client
            .create_transaction(
                "account-1",
                &CreateTransactionRequest {
                    learn_categories: None,
                    run_transfers: None,
                    transaction: TransactionDto {
                        account: "account-1".to_string(),
                        date: "2026-03-10".to_string(),
                        amount: Some(-1000),
                        payee: None,
                        payee_name: Some("Store".to_string()),
                        imported_payee: None,
                        category: None,
                        notes: None,
                        imported_id: None,
                        transfer_id: None,
                        cleared: Some(true),
                        subtransactions: None,
                    },
                },
            )
            .unwrap();

        without_header_mock.assert_calls(1);
        with_header_mock.assert_calls(0);
    }

    #[test]
    fn retries_on_server_error_and_returns_error_when_retries_exhausted() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(DELETE)
                .path("/budgets/budget-sync-1/transactions/tx-1")
                .header("x-api-key", "test_key");
            then.status(500)
                .header("Content-Type", "application/json")
                .json_body(json!({ "error": "internal error" }));
        });

        let client = ActualClient::new_for_test(base_config(&server));
        let error = client.delete_transaction("tx-1").unwrap_err();

        assert_that!(
            error.to_string(),
            contains_substring(
                "DELETE /budgets/{budgetSyncId}/transactions/{transactionId} returned error: 500 Internal Server Error"
            )
        );
        mock.assert_calls((ERROR_RETRIES + 1) as usize);
    }

    #[test]
    fn does_not_retry_on_429_client_error() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(DELETE)
                .path("/budgets/budget-sync-1/transactions/tx-1")
                .header("x-api-key", "test_key");
            then.status(429)
                .header("Retry-After", "1")
                .header("Content-Type", "application/json")
                .json_body(json!({ "error": "rate limit" }));
        });

        let client = ActualClient::new_for_test(base_config(&server));
        let error = client.delete_transaction("tx-1").unwrap_err();

        assert_that!(
            error.to_string(),
            contains_substring(
                "DELETE /budgets/{budgetSyncId}/transactions/{transactionId} returned error: 429 Too Many Requests"
            )
        );
        mock.assert_calls(1);
    }
}
