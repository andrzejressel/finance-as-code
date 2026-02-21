#![cfg_attr(docsrs, feature(doc_cfg))]

pub use finance_as_code_budget_core::FileReader;
pub use finance_as_code_budget_core::TransactionHolder;
use finance_as_code_budget_core::map_bank_transaction_to_transaction;
pub use finance_as_code_budget_core::readers::LocalDirectorySource;
pub use finance_as_code_budget_core::readers::Source;
pub use finance_as_code_budget_core::sink::Sink;
use log::info;
use rootcause::prelude::ResultExt;

#[cfg(feature="source_lunchflow")]
#[cfg_attr(docsrs, doc(cfg(any(feature = "source_lunchflow", feature = "all"))))]
pub mod lunchflow {
    pub use finance_as_code_budget_source_lunchflow::LunchFlowDownloaderConfig;
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
pub mod treeline {
    pub use finance_as_code_budget_sink_treeline::SinkTreelineOptions;
    pub use finance_as_code_budget_sink_treeline::create_treeline_sink;
}

#[cfg(feature = "sink_lunchmoney")]
#[cfg_attr(docsrs, doc(cfg(any(feature = "sink_lunchmoney", feature = "all"))))]
pub mod lunchmoney {
    pub use finance_as_code_budget_sink_lunchmoney::LunchMoneyAccountName;
    pub use finance_as_code_budget_sink_lunchmoney::LunchMoneyApiKey;
    pub use finance_as_code_budget_sink_lunchmoney::LunchMoneySinkConfig;
    pub use finance_as_code_budget_sink_lunchmoney::create_lunchmoney_sink;
}

pub fn run(
    sources: Vec<Box<dyn Source>>,
    // transformers: Vec<Box<dyn Transformer>>,
    sinks: Vec<Box<dyn Sink>>,
) -> rootcause::Result<()> {
    colog::init();

    let mut holders = Vec::new();

    for source in sources {
        info!("Running source {}", source.name());
        holders.push(
            source
                .read()
                .context_with(|| format!("Failed to read from source {}", source.name()))?,
        );
        info!("Finished running source {}", source.name());
    }

    let holder = TransactionHolder::combine_vec(holders);
    let bank_transactions = holder.into_transactions();
    let transactions = map_bank_transaction_to_transaction(bank_transactions)
        .context("Failed to map bank transactions to transactions")?;

    for sink in sinks {
        info!("Writing to sink {}", sink.name());
        sink.write(&transactions)
            .context_with(|| format!("Failed to write to sink {}", sink.name()))?;
        info!("Finished writing to sink {}", sink.name());
    }

    Ok(())
}
