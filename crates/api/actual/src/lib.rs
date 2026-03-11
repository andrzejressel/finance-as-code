pub mod api;
pub mod dto;

/// API key used to authenticate against the Actual HTTP API (`x-api-key`
/// header).
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

impl From<&str> for ApiKey {
    fn from(value: &str) -> Self {
        Self::new(value.to_string())
    }
}

/// Actual budget synchronization id used in path parameters.
#[derive(Debug, Clone)]
pub struct BudgetSyncId(String);

impl BudgetSyncId {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

impl From<&str> for BudgetSyncId {
    fn from(value: &str) -> Self {
        Self::new(value.to_string())
    }
}

/// Optional encryption password sent in the `budget-encryption-password`
/// header.
#[derive(Debug, Clone)]
pub struct BudgetEncryptionPassword(String);

impl BudgetEncryptionPassword {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

impl From<&str> for BudgetEncryptionPassword {
    fn from(value: &str) -> Self {
        Self::new(value.to_string())
    }
}
