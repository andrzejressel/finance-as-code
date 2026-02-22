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

impl From<i64> for LunchFlowAccountId {
    fn from(val: i64) -> Self {
        LunchFlowAccountId(val)
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

impl From<String> for LunchFlowApiKey {
    fn from(val: String) -> Self {
        LunchFlowApiKey(val)
    }
}

impl From<&str> for LunchFlowApiKey {
    fn from(val: &str) -> Self {
        LunchFlowApiKey(val.to_string())
    }
}

#[derive(bon::Builder, Debug, Clone)]
pub struct LunchFlowDownloaderConfig {
    #[builder(into)]
    /// Account ID in Lunch Flow to download transactions from. Can be found in URL when clicking on account in [account view](https://www.lunchflow.app/accounts) or [connections view](https://www.lunchflow.app/connections)
    pub(crate) account_id: LunchFlowAccountId,
    #[builder(into)]
    /// API key for Lunch Flow REST API. Can be created in [Destinations](https://www.lunchflow.app/destinations)
    pub(crate) api_key: LunchFlowApiKey,
    #[builder(into)]
    /// Local directory where downloaded JSON data will be stored. When reading these files using
    /// `LocalDirectorySource::new(...)`, pass the same path so the reader can find the downloaded
    /// files.
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
