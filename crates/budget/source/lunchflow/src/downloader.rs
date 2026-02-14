use crate::api::LunchFlowApi;
use crate::LunchFlowAccountId;
use finance_as_code_budget_core::TransactionHolder;
use finance_as_code_budget_core::readers::Source;
use log::info;
use rootcause::prelude::ResultExt;
use rootcause::*;
use std::path::PathBuf;

#[cfg_attr(test, mockall::automock)]
pub(crate) trait Clock {
    fn get_seconds_since_epoch(&self) -> u64;
}

pub(crate) struct RealClock;

impl RealClock {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Clock for RealClock {
    fn get_seconds_since_epoch(&self) -> u64 {
        let now = std::time::SystemTime::now();
        now.duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs()
    }
}

pub(crate) struct LunchflowDownloader {
    api: Box<dyn crate::api::LunchFlowApi>,
    account_id: LunchFlowAccountId,
    dir: PathBuf,
    clock: Box<dyn Clock>,
}

impl LunchflowDownloader {
    pub(crate) fn new(
        api: impl LunchFlowApi + 'static,
        account_id: LunchFlowAccountId,
        dir: PathBuf,
        clock: impl Clock + 'static,
    ) -> Result<Self> {
        if !dir.exists() {
            bail!("{:?} does not exist", dir);
        }
        if !dir.is_dir() {
            bail!("{:?} is not a directory", dir);
        }
        Ok(Self {
            api: Box::new(api),
            account_id,
            dir,
            clock: Box::new(clock),
        })
    }
}

impl Source for LunchflowDownloader {
    fn name(&self) -> String {
        "LunchflowDownloader".to_string()
    }

    fn read(&self) -> Result<TransactionHolder> {
        info!(
            "Getting transactions from LunchFlow API for account_id: [{}]",
            self.account_id.value()
        );
        let lunchflow_transactions = self
            .api
            .get_transactions(&self.account_id)
            .context("Failed to get transactions from LunchFlow API")?;

        let file_name = format!(
            "{}_lunchflow_transactions.json",
            self.clock.get_seconds_since_epoch()
        );

        let file = self.dir.join(file_name);
        let content = serde_json::to_string(&lunchflow_transactions)
            .context("Failed to serialize transactions to JSON")?;

        std::fs::write(&file, content)
            .context_with(|| format!("Failed to write transactions to file [{:?}]", file))?;

        info!("Transactions successfully written to file [{:?}]", file);
        Ok(TransactionHolder::empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{LunchFlowTransaction, LunchFlowTransactions, MockLunchFlowApi};
    use finance_as_code_budget_core::readers::Source;
    use googletest::assert_that;
    use googletest::prelude::eq;
    
    
    

    #[test]
    fn test_lunchflow_downloader() {
        let temp_dir = tempfile::tempdir()
            .context("Failed to create temporary directory")
            .expect("Failed to create temporary directory");
        let transactions = LunchFlowTransactions {
            transactions: vec![LunchFlowTransaction {
                id: Some("txn_123".to_string()),
                account_id: 1,
                amount: 100.0,
                currency: "USD".to_string(),
                date: "2024-01-01".to_string(),
                merchant: Some("Example Store".to_string()),
                description: Some("Purchase at Example Store".to_string()),
                is_pending: Some(false),
            }],
        };

        let mut mock_api = MockLunchFlowApi::new();
        let value = transactions.clone();
        mock_api
            .expect_get_transactions()
            .with(mockall::predicate::eq(LunchFlowAccountId::new(1)))
            .returning(move |_| Ok(value.clone()));

        let mut mock_clock = MockClock::new();
        mock_clock
            .expect_get_seconds_since_epoch()
            .return_const(1234567890u64);

        let downloader = LunchflowDownloader::new(
            mock_api,
            LunchFlowAccountId::new(1),
            temp_dir.path().to_path_buf(),
            mock_clock,
        )
        .expect("Failed to create LunchflowDownloader");

        let result = downloader.read().unwrap();

        assert_that!(result.number_of_transactions(), eq(0));

        let expected_file = temp_dir
            .path()
            .join("1234567890_lunchflow_transactions.json");
        assert!(expected_file.exists(), "Expected file was not created");
        let content =
            std::fs::read_to_string(expected_file).expect("Failed to read the created file");

        insta::assert_snapshot!(content);
    }
}
