pub mod api;
pub mod dto;

/// Bearer token used to authenticate against the Lunch Money v2 API.
#[derive(Debug, Clone)]
pub struct ApiKey(String);

impl ApiKey {
    pub fn new(key: String) -> Self {
        Self(key)
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}
