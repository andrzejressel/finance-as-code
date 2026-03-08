use finance_as_code_utils_gemini::{ContentGenerator, GeminiClient};
use googletest::prelude::*;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema, PartialEq, Eq)]
struct DemoResponse {
    system_seen: String,
    user_seen: String,
    verdict: String,
}

#[test]
fn live_google_api_uses_system_user_prompt_and_deserializes() {
    let Ok(api_key) = std::env::var("GOOGLE_API_KEY") else {
        eprintln!("Skipping live Gemini integration test because GOOGLE_API_KEY is not set");
        return;
    };
    let client = GeminiClient::create(api_key);

    let system_marker = "SYSTEM_MARKER_A1B2C3";
    let user_marker = "USER_MARKER_X9Y8Z7";
    let system_prompt = "You are a strict validator. Reply using the required JSON shape only.";
    let user_prompt = format!(
        "Set system_seen to '{system_marker}', user_seen to '{user_marker}', and verdict to 'ok'."
    );

    let response: DemoResponse = client
        .generate_content(system_prompt, &user_prompt)
        .expect("live Gemini call should succeed");

    assert_that!(
        &response,
        eq(&DemoResponse {
            system_seen: system_marker.to_string(),
            user_seen: user_marker.to_string(),
            verdict: "ok".to_string(),
        })
    );
}
