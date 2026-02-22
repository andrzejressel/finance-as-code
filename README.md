# Finance as Code Budget

Finance as Code is a personal finance automation project that moves transactions from different inputs into budgeting destinations in a repeatable, code-driven way.

Goal: define your data flow once, then run it reliably to keep your tools in sync.

Sources are where transactions come from (for example bank CSV exports or online APIs). Sinks are where normalized transactions are written (for example local apps or external services).

## Supported sources and sinks

Affiliate disclosure: links to Lunch Flow and Lunch Money may be affiliate links. If you use them, I may earn a commission at no extra cost to you.

### Sources

#### Banks

| Source                             | Feature        | Supported |
|------------------------------------|----------------|-----------|
| [mBank CSV](https://www.mbank.pl/) | `source_mbank` | Yes       |

#### Online Services

| Source                                                                           | Feature            | Supported |
|----------------------------------------------------------------------------------|--------------------|-----------|
| [Lunch Flow](https://www.lunchflow.app/?ref=andrzej-ressel&sub1=finance-as-code) | `source_lunchflow` | Yes       |

### Sinks

#### Local Applications

| Sink                                | Feature         | Supported |
|-------------------------------------|-----------------|-----------|
| [Treeline](https://treeline.money/) | `sink_treeline` | Yes       |

#### Services

| Sink                                                  | Feature           | Supported |
|-------------------------------------------------------|-------------------|-----------|
| [Lunch Money](https://lunchmoney.app/?refer=jgrrpmpw) | `sink_lunchmoney` | Yes       |

Notes:
- For Lunch Flow, keep `create_lunchflow_downloader(...)` before `LocalDirectorySource(...)` in the `sources` vector.
- `all` enables all source/sink features.
- `all_with_db` enables all source/sink features and `bundled_db` for Treeline.

## Quick start

This library is not published on crates.io yet. Add it from GitHub with the feature set you need:

```toml
[dependencies]
finance_as_code_budget = { git = "https://github.com/andrzejressel/finance-as-code.git", features = ["all_with_db"] }
```

`all_with_db` enables all sources/sinks and bundles DuckDB for Treeline. If you have a weaker PC you can try using `all` instead which uses prebuilt duckdb:

```toml
[dependencies]
finance_as_code_budget = { git = "https://github.com/andrzejressel/finance-as-code.git", features = ["all"] }
```

In that case however you must set `DUCKDB_DOWNLOAD_LIB="1"` in your environment variables to allow downloading the duckdb library. You can automate it by creating `.cargo/config.toml` with the following content:

```toml
[env]
DUCKDB_DOWNLOAD_LIB = "1"
```

## Quick start usage: Lunch Flow -> Lunch Money

```rust,no_run
use finance_as_code_budget::{
    LocalDirectorySource, Sink, Source,
    lunchflow::{
        LunchFlowDownloaderConfig, create_lunchflow_downloader, create_lunchflow_file_reader,
    },
    lunchmoney::{LunchMoneySinkConfig, create_lunchmoney_sink},
    run,
};

fn main() {
    let lunchflow_dir = "path/to/lunchflow/dir";

    let sources: Vec<Box<dyn Source>> = vec![
        // Keep downloader before LocalDirectorySource for Lunch Flow files.
        Box::new(create_lunchflow_downloader(
            LunchFlowDownloaderConfig::builder()
                .account_id(123_i64)
                .api_key("lunchflow_api_key")
                .local_directory(lunchflow_dir)
                .build(),
        )
        .expect("Failed to create Lunchflow downloader")),
        Box::new(LocalDirectorySource::new(
            lunchflow_dir,
            create_lunchflow_file_reader(),
        )
        .expect("Failed to create local directory source")),
    ];

    let sinks: Vec<Box<dyn Sink>> = vec![Box::new(create_lunchmoney_sink(
        LunchMoneySinkConfig::builder()
            .api_key("lunchmoney_api_key")
            .account_name("My Account")
            .build(),
    ))];

    run(sources, sinks).expect("Pipeline run failed");
}
```

## Quick start usage: Treeline sink

```rust,no_run
use finance_as_code_budget::{
    Sink, run,
    treeline::{SinkTreelineOptions, create_treeline_sink},
};
# use finance_as_code_budget::__private::create_source;

fn main() {
    let sources = vec![create_source()];

    let sinks: Vec<Box<dyn Sink>> = vec![Box::new(create_treeline_sink(
        SinkTreelineOptions::builder()
            .account_name("My Treeline Account")
            .build(),
    ))];

    run(sources, sinks).expect("Pipeline run failed");
}
```

The Treeline sink performs a full replacement for the selected account (existing
transactions and balances for that account are removed before import).
