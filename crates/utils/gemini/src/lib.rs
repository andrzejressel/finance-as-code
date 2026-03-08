use genai::adapter::AdapterKind;
use genai::chat::ChatResponseFormat::JsonSpec as JsonSpecEnum;
use genai::chat::{ChatMessage, ChatOptions, ChatRequest, JsonSpec};
use genai::resolver::{AuthData, AuthResolver, Endpoint, ServiceTargetResolver};
use genai::{Client, ModelIden, ServiceTarget};
use rootcause::Result;
use rootcause::option_ext::OptionExt;
use rootcause::prelude::ResultExt;
use schemars::{JsonSchema, schema_for};
use serde::de::DeserializeOwned;

/// Trait for generating content using AI.
///
/// This trait allows mocking the AI client in tests.
#[cfg_attr(any(test, feature = "mock"), mockall::automock)]
pub trait ContentGenerator {
    /// Generates typed content from a system prompt and user prompt.
    ///
    /// # Errors
    ///
    /// Returns an error if the API call fails or the response is invalid.
    fn generate_content<T>(&self, system_prompt: &str, user_prompt: &str) -> rootcause::Result<T>
    where
        T: JsonSchema + DeserializeOwned + 'static;
}

/// Client for interacting with the Google Gemini API using the `genai` crate.
pub struct GeminiClient {
    client: Client,
}

impl GeminiClient {
    /// Creates a new `GeminiClient` with the given API key.
    pub fn create(api_key: String) -> Self {
        let auth_resolver = create_auth_resolver(api_key);

        let builder = Client::builder().with_auth_resolver(auth_resolver);

        Self {
            client: builder.build(),
        }
    }

    /// Creates a new `GeminiClient` with the given API key and base URL.
    pub fn create_with_base_url(api_key: String, base_url: String) -> Self {
        let auth_resolver = create_auth_resolver(api_key);

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

    async fn send_gemini_request<T>(&self, req: ChatRequest) -> Result<T>
    where
        T: JsonSchema + DeserializeOwned,
    {
        let schema = schema_for!(T);
        let json_spec = JsonSpec::new(
            "output_schema",
            serde_json::to_value(&schema).context("Failed to convert JSON schema to value")?,
        );

        let res = self
            .client
            .exec_chat(
                "gemini-2.5-flash",
                req,
                Some(&ChatOptions::default().with_response_format(JsonSpecEnum(json_spec))),
            )
            .await
            .into_report()
            .context("Failed to execute gemini-2.5-flash")?;

        let response_text = res
            .first_text()
            .context("Gemini response did not contain any text content")?;

        let result = serde_json::from_str::<T>(response_text).context_with(|| {
            format!(
                "Failed to deserialize Gemini JSON response into requested type. Response: {}",
                response_text
            )
        })?;
        Ok(result)
    }
}

impl ContentGenerator for GeminiClient {
    fn generate_content<T>(&self, system_prompt: &str, user_prompt: &str) -> Result<T>
    where
        T: JsonSchema + DeserializeOwned,
    {
        let req = ChatRequest::new(vec![
            ChatMessage::system(system_prompt),
            ChatMessage::user(user_prompt),
        ]);

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| rootcause::report!("Failed to create tokio runtime: {}", e))?;

        rt.block_on(async { self.send_gemini_request(req).await })
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use httpmock::MockServer;
    use schemars::JsonSchema;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Debug, Deserialize, JsonSchema, PartialEq, Eq)]
    struct TestResponse {
        value: String,
    }

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
                            "parts": [{"text": "{\"value\":\"world\"}"}],
                            "role": "model"
                        }
                    }]
                }));
        });

        let client = GeminiClient::create_with_base_url("test_key".to_string(), server.base_url());
        let response: TestResponse = client.generate_content("you are helpful", "hello").unwrap();

        assert_that!(
            &response,
            eq(&TestResponse {
                value: "world".to_string()
            })
        );
        mock.assert();
    }
}
