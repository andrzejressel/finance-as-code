use crate::{LunchFlowAccountId, LunchFlowApiKey};
use rootcause::prelude::ResultExt;
use rootcause::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LunchFlowTransactions {
    pub transactions: Vec<LunchFlowTransaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LunchFlowTransaction {
    pub id: Option<String>,
    pub account_id: i64,
    pub amount: f64,
    pub currency: String,
    pub date: String,
    pub merchant: Option<String>,
    pub description: Option<String>,
    pub is_pending: Option<bool>,
}

#[cfg_attr(test, mockall::automock)]
pub(crate) trait LunchFlowApi {
    fn get_transactions(&self, account_id: &LunchFlowAccountId) -> Result<LunchFlowTransactions>;
}

pub(crate) struct RealLunchFlowApi {
    url: String,
    api_key: LunchFlowApiKey,
    client: reqwest::blocking::Client,
}

impl RealLunchFlowApi {
    pub(crate) fn new(url: String, api_key: LunchFlowApiKey) -> Self {
        Self {
            url,
            api_key,
            client: reqwest::blocking::Client::builder()
                .timeout(Some(std::time::Duration::from_secs(120)))
                .build()
                .expect("Failed to build reqwest client"),
        }
    }
}

impl LunchFlowApi for RealLunchFlowApi {
    fn get_transactions(&self, account_id: &LunchFlowAccountId) -> Result<LunchFlowTransactions> {
        let response = self
            .client
            .get(format!(
                "{}/accounts/{}/transactions",
                self.url,
                account_id.value()
            ))
            .header("x-api-key", self.api_key.value())
            // .query(&[("accountId", )])
            .send()
            .context("Can't get transactions from LunchFlowApi")?;

        if response.status().is_success() {
            let transactions = response
                .json::<LunchFlowTransactions>()
                .context("Can't deserialize transactions from LunchFlowApi")?;
            Ok(transactions)
        } else {
            let status = response.status();
            let body = response.text().context_with(|| {
                format!(
                    "Failed to get error body from LunchFlowApi response with status {}",
                    status
                )
            })?;
            bail!("LunchFlowApi returned error: {} - {}", status, body);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::LunchFlowApiKey;
    use crate::api::LunchFlowApi;
    use crate::api::{LunchFlowTransaction, LunchFlowTransactions, RealLunchFlowApi};
    use googletest::prelude::*;
    use httpmock::MockServer;

    #[test]
    fn should_deserialize_transactions() {
        let json = r#"
        {
            "transactions": [
                {
                    "id": "txn_123",
                    "accountId": 1,
                    "amount": 100.0,
                    "currency": "USD",
                    "date": "2024-01-01",
                    "merchant": "Example Store",
                    "description": "Purchase at Example Store",
                    "isPending": false
                }
            ]
        }
        "#;

        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/accounts/1/transactions")
                .header("x-api-key", "test_api_key");
            then.status(200)
                .header("Content-Type", "application/json")
                .body(json);
        });

        let api = RealLunchFlowApi::new(
            server.url(""),
            LunchFlowApiKey::new("test_api_key".to_string()),
        );
        let transactions = api.get_transactions(&1.into()).unwrap();

        assert_that!(
            transactions,
            eq(&LunchFlowTransactions {
                transactions: vec![LunchFlowTransaction {
                    id: Some("txn_123".to_string()),
                    account_id: 1,
                    amount: 100.0,
                    currency: "USD".to_string(),
                    date: "2024-01-01".to_string(),
                    merchant: Some("Example Store".to_string()),
                    description: Some("Purchase at Example Store".to_string()),
                    is_pending: Some(false),
                }]
            })
        );

        mock.assert();
    }
}
