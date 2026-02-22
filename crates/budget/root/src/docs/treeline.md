# Treeline quick start

Use this sink when you want to write pipeline transactions into Treeline.

This sink fully replaces data for the selected account: existing transactions and balances are removed before import.

## Treeline sink only (mock source)

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
