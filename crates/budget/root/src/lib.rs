#![doc = include_str!("../../../../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub use finance_as_code_budget_core::FileReader;
pub use finance_as_code_budget_core::TransactionHolder;
pub use finance_as_code_budget_core::readers::LocalDirectorySource;
pub use finance_as_code_budget_core::readers::Source;
pub use finance_as_code_budget_core::run;
pub use finance_as_code_budget_core::sink::Sink;

pub use finance_as_code_budget_core::BankTransaction;
pub use finance_as_code_budget_core::Transaction;

pub mod transformer {
    pub use finance_as_code_budget_core::transformer::Transformer;
    pub use finance_as_code_budget_core::transformer::create_single_transaction_transformer;
}

#[cfg(feature = "source_lunchflow")]
#[cfg_attr(docsrs, doc(cfg(any(feature = "source_lunchflow", feature = "all"))))]
#[doc = include_str!("docs/lunchflow.md")]
pub mod lunchflow {
    pub use finance_as_code_budget_source_lunchflow::LunchFlowDownloaderConfig;
    pub use finance_as_code_budget_source_lunchflow::LunchFlowDownloaderConfigBuilder;
    pub use finance_as_code_budget_source_lunchflow::create_lunchflow_downloader;
    pub use finance_as_code_budget_source_lunchflow::create_lunchflow_file_reader;
}

#[cfg(feature = "source_mbank")]
#[cfg_attr(docsrs, doc(cfg(any(feature = "source_mbank", feature = "all"))))]
pub mod mbank {
    pub use finance_as_code_budget_source_mbank::create_mbank_file_reader;
}

#[cfg(feature = "sink_treeline")]
#[cfg_attr(docsrs, doc(cfg(any(feature = "sink_treeline", feature = "all"))))]
#[doc = include_str!("docs/treeline.md")]
pub mod treeline {
    pub use finance_as_code_budget_sink_treeline::SinkTreelineOptions;
    pub use finance_as_code_budget_sink_treeline::SinkTreelineOptionsBuilder;
    pub use finance_as_code_budget_sink_treeline::create_treeline_sink;
}

#[cfg(feature = "sink_lunchmoney")]
#[cfg_attr(docsrs, doc(cfg(any(feature = "sink_lunchmoney", feature = "all"))))]
pub mod lunchmoney {
    pub use finance_as_code_budget_sink_lunchmoney::LunchMoneyAccountName;
    pub use finance_as_code_budget_sink_lunchmoney::LunchMoneyApiKey;
    pub use finance_as_code_budget_sink_lunchmoney::LunchMoneySinkConfig;
    pub use finance_as_code_budget_sink_lunchmoney::LunchMoneySinkConfigBuilder;
    pub use finance_as_code_budget_sink_lunchmoney::LunchMoneyTags;
    pub use finance_as_code_budget_sink_lunchmoney::create_lunchmoney_sink;
}

#[doc(hidden)]
pub mod __private {
    use finance_as_code_budget_core::readers::Source;
    use finance_as_code_budget_core::sink::Sink;

    pub fn create_sink() -> Box<dyn Sink> {
        panic!("Should not be invoked")
    }

    pub fn create_source() -> Box<dyn Source> {
        panic!("Should not be invoked")
    }
}
