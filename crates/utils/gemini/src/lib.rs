use anyhow::{Context, Result};
use genai::adapter::AdapterKind;
use genai::chat::{ChatMessage, ChatRequest};
use genai::resolver::{AuthData, AuthResolver, Endpoint, ServiceTargetResolver};
use genai::{Client, ModelIden, ServiceTarget};

/// Client for interacting with the Google Gemini API using the `genai` crate.
pub struct GeminiClient {
    client: Client,
}

impl GeminiClient {
    /// Creates a new `GeminiClient` with the given API key.
    pub fn create(api_key: String) -> Self {
        let auth_resolver = Self::create_auth_resolver(api_key);

        let builder = Client::builder().with_auth_resolver(auth_resolver);

        Self {
            client: builder.build(),
        }
    }

    /// Creates a new `GeminiClient` with the given API key and base URL.
    pub fn create_with_base_url(api_key: String, base_url: String) -> Self {
        let auth_resolver = Self::create_auth_resolver(api_key);

        let resolver = ServiceTargetResolver::from_resolver_fn(move |mut target: ServiceTarget| {
            if matches!(target.model.adapter_kind, AdapterKind::Gemini) {
                target.endpoint = Endpoint::from_owned(format!("{}/v1beta", base_url));
            }
            Ok(target)
        });

        let builder = Client::builder()
            .with_auth_resolver(auth_resolver)
            .with_service_target_resolver(resolver);

        Self {
            client: builder.build(),
        }
    }

    /// Sends a prompt to the Gemini API and returns the generated response.
    ///
    /// This uses the `gemini-2.5-flash` model.
    pub fn generate_content(&self, prompt: &str) -> Result<String> {
        let req = ChatRequest::new(vec![ChatMessage::user(prompt)]);

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("Failed to create tokio runtime")?;

        rt.block_on(async {
            let res = self
                .client
                .exec_chat("gemini-2.5-flash", req, None)
                .await
                .context("Failed to execute Gemini request via genai")?;

            Ok(res
                .first_text()
                .context("No content in Gemini response")?
                .to_string())
        })
    }

    fn create_auth_resolver(api_key: String) -> AuthResolver {
        AuthResolver::from_resolver_fn(move |model_iden: ModelIden| {
            if matches!(model_iden.adapter_kind, AdapterKind::Gemini) {
                Ok(Some(AuthData::Key(api_key.clone())))
            } else {
                Ok(None)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use httpmock::MockServer;
    use serde_json::json;

    #[test]
    fn test_generate_content() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST);
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!({
                    "candidates": [{
                        "content": {
                            "parts": [{"text": "world"}],
                            "role": "model"
                        }
                    }]
                }));
        });

        let client = GeminiClient::create_with_base_url("test_key".to_string(), server.base_url());
        let response = client.generate_content("hello").unwrap();

        assert_that!(response, eq("world"));
        mock.assert();
    }
}
