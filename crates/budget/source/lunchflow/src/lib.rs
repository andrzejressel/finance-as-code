mod api;
mod downloader;
mod file_reader;

use crate::api::RealLunchFlowApi;
use crate::downloader::{LunchflowDownloader, RealClock};
use crate::file_reader::LunchflowFileReader;
use finance_as_code_budget_core::FileReader;
use finance_as_code_budget_core::readers::Source;
use rootcause::*;
use std::path::PathBuf;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct LunchFlowAccountId(i64);
impl LunchFlowAccountId {
    pub fn new(id: i64) -> Self {
        Self(id)
    }

    pub(crate) fn value(&self) -> i64 {
        self.0
    }
}

impl Into<LunchFlowAccountId> for i64 {
    fn into(self) -> LunchFlowAccountId {
        LunchFlowAccountId(self)
    }
}

#[derive(Clone, Debug)]
pub struct LunchFlowApiKey(String);

impl LunchFlowApiKey {
    pub fn new(key: String) -> Self {
        Self(key)
    }

    pub(crate) fn value(&self) -> &str {
        &self.0
    }
}

impl Into<LunchFlowApiKey> for String {
    fn into(self) -> LunchFlowApiKey {
        LunchFlowApiKey(self)
    }
}

impl Into<LunchFlowApiKey> for &str {
    fn into(self) -> LunchFlowApiKey {
        LunchFlowApiKey(self.to_string())
    }
}

#[derive(bon::Builder, Debug, Clone)]
pub struct LunchFlowDownloaderConfig {
    #[builder(into)]
    pub(crate) account_id: LunchFlowAccountId,
    #[builder(into)]
    pub(crate) api_key: LunchFlowApiKey,
    #[builder(into)]
    pub(crate) local_directory: PathBuf,
}

/// [Source] that does not actually read anything - it triggers download of the file from the Lunchflow API
/// When used it should be first source, so the actual [create_lunchflow_file_reader] will read
/// new file.
pub fn create_lunchflow_downloader(config: LunchFlowDownloaderConfig) -> Result<impl Source> {
    let api = RealLunchFlowApi::new(
        "https://www.lunchflow.app/api/v1".to_string(),
        config.api_key,
    );
    let clock = RealClock::new();
    LunchflowDownloader::new(api, config.account_id, config.local_directory, clock)
}

/// [FileReader] that reads the file downloaded by [create_lunchflow_downloader]
/// Can  be used without the downloader, expected files are raw JSONs from
/// [Lunch Flow API](https://docs.lunchflow.app/api-reference/get-account-transactions).
pub fn create_lunchflow_file_reader() -> impl FileReader {
    LunchflowFileReader {}
}
