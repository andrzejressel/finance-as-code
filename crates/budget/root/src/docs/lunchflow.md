# Lunch Flow quick start

Use this module when you want to download transactions from Lunch Flow and feed them into the pipeline.

## Download from Lunch Flow

Use this setup when you want to fetch transactions from Lunch Flow and save them to a local directory.

```rust,no_run
use finance_as_code_budget::{
    Setup, run,
    lunchflow::{LunchFlowDownloaderConfig, create_lunchflow_downloader},
};
# use finance_as_code_budget::__private::{create_sink, create_source};

fn main() {
    let lunchflow_dir = "path/to/lunchflow/dir";

    let setups: Vec<Box<dyn Setup>> = vec![Box::new(
        create_lunchflow_downloader(
            LunchFlowDownloaderConfig::builder()
                .account_id(123_i64)
                .api_key("lunchflow_api_key")
                .local_directory(lunchflow_dir)
                .build(),
        )
        .expect("Failed to create Lunch Flow downloader"),
    )];

    let sources = vec![create_source()];
    let sinks = vec![create_sink()];

    run(setups, sources, vec![], sinks).expect("Pipeline run failed");
}
```

## Lunch Flow source only

Use this source when you want to import previously downloaded Lunch Flow files from a local directory.

```rust,no_run
use finance_as_code_budget::{
    LocalDirectorySource, Source, run,
    lunchflow::create_lunchflow_file_reader,
};
# use finance_as_code_budget::__private::create_sink;

fn main() {
    let lunchflow_dir = "path/to/lunchflow/dir";

    let sources: Vec<Box<dyn Source>> = vec![Box::new(
        LocalDirectorySource::new(lunchflow_dir, create_lunchflow_file_reader())
            .expect("Failed to create local directory source"),
    )];

    let sinks = vec![create_sink()];

    run(vec![], sources, vec![], sinks).expect("Pipeline run failed");
}
```
