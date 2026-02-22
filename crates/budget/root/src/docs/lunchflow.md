# Lunch Flow quick start

Use this source when you want to download transactions from Lunch Flow and feed them into the pipeline.

The downloader writes a JSON file to the local directory as a side effect, so it must run before `LocalDirectorySource`.

```rust,no_run
use finance_as_code_budget::{
    LocalDirectorySource, Source, run,
    lunchflow::{
        LunchFlowDownloaderConfig, create_lunchflow_downloader, create_lunchflow_file_reader,
    },
};
# use finance_as_code_budget::__private::create_sink;

fn main() {
    let lunchflow_dir = "path/to/lunchflow/dir";

    let sources: Vec<Box<dyn Source>> = vec![
        Box::new(create_lunchflow_downloader(
            LunchFlowDownloaderConfig::builder()
                .account_id(123_i64)
                .api_key("lunchflow_api_key")
                .local_directory(lunchflow_dir)
                .build(),
        )
        .expect("Failed to create Lunch Flow downloader")),
        Box::new(LocalDirectorySource::new(
            lunchflow_dir,
            create_lunchflow_file_reader(),
        )
        .expect("Failed to create local directory source")),
    ];

    let sinks = vec![create_sink()];

    run(sources, vec![], sinks).expect("Pipeline run failed");
}
```
