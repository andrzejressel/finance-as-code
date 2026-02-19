use crate::ApiKey;
use crate::dto::{
    DeleteTransactionsRequest, GetTransactionsParams, PutTransactionsRequest,
    PutTransactionsResponse, TransactionDto,
};
use rootcause::prelude::ResultExt;
use rootcause::{Result, bail};

/// Single-page response used internally by the pagination loop.
#[derive(serde::Deserialize)]
struct PageResponse {
    transactions: Vec<TransactionDto>,
    has_more: bool,
}

/// Abstraction over the Lunch Money v2 `/transactions` endpoints.
///
/// A `MockLunchMoneyApi` is generated automatically in test builds via `mockall`.
#[cfg_attr(test, mockall::automock)]
pub trait LunchMoneyApi {
    /// Fetch every matching transaction, following `has_more` / `offset` pagination
    /// automatically until the full result set has been retrieved.
    fn get_all_transactions(&self, params: &GetTransactionsParams) -> Result<Vec<TransactionDto>>;

    /// Update up to 500 transactions in a single call (`PUT /transactions`).
    fn put_transactions(&self, request: &PutTransactionsRequest)
    -> Result<PutTransactionsResponse>;

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

    fn fetch_page(&self, params: &GetTransactionsParams) -> Result<PageResponse> {
        let response = self
            .client
            .get(format!("{}/transactions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key.value()))
            .query(params)
            .send()
            .context("Failed to send GET /transactions request")?;

        if response.status().is_success() {
            Ok(response
                .json::<PageResponse>()
                .context("Failed to deserialize GET /transactions response")?)
        } else {
            let status = response.status();
            let body = response.text().context_with(|| {
                format!(
                    "Failed to read error body from GET /transactions (status {})",
                    status
                )
            })?;
            bail!("GET /transactions returned error: {} - {}", status, body);
        }
    }
}

impl LunchMoneyApi for LunchMoneyClient {
    fn get_all_transactions(&self, params: &GetTransactionsParams) -> Result<Vec<TransactionDto>> {
        let mut all_transactions = Vec::new();
        let limit = params.limit.unwrap_or(1000);
        let mut offset = params.offset.unwrap_or(0);

        loop {
            let page_params = GetTransactionsParams {
                start_date: params.start_date.clone(),
                end_date: params.end_date.clone(),
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
        let response = self
            .client
            .put(format!("{}/transactions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key.value()))
            .json(request)
            .send()
            .context("Failed to send PUT /transactions request")?;

        if response.status().is_success() {
            Ok(response
                .json::<PutTransactionsResponse>()
                .context("Failed to deserialize PUT /transactions response")?)
        } else {
            let status = response.status();
            let body = response.text().context_with(|| {
                format!(
                    "Failed to read error body from PUT /transactions (status {})",
                    status
                )
            })?;
            bail!("PUT /transactions returned error: {} - {}", status, body);
        }
    }

    fn delete_transactions(&self, request: &DeleteTransactionsRequest) -> Result<()> {
        let response = self
            .client
            .delete(format!("{}/transactions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key.value()))
            .json(request)
            .send()
            .context("Failed to send DELETE /transactions request")?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().context_with(|| {
                format!(
                    "Failed to read error body from DELETE /transactions (status {})",
                    status
                )
            })?;
            bail!("DELETE /transactions returned error: {} - {}", status, body);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ApiKey;
    use crate::dto::{
        DeleteTransactionsRequest, GetTransactionsParams, PutTransactionsRequest,
        PutTransactionsResponse, TransactionDto, UpdateTransactionDto,
    };
    use googletest::prelude::*;
    use httpmock::MockServer;
    use serde_json::json;

    fn tx(id: i64, date: &str, payee: &str, notes: Option<&str>) -> TransactionDto {
        TransactionDto {
            id,
            date: date.to_string(),
            amount: "100.0000".to_string(),
            currency: "USD".to_string(),
            payee: payee.to_string(),
            notes: notes.map(str::to_string),
        }
    }

    #[test]
    fn get_all_transactions_paginates_through_all_pages() -> googletest::Result<()> {
        let server = MockServer::start();

        let mock_page1 = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/transactions")
                .header("Authorization", "Bearer test_key")
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
}
