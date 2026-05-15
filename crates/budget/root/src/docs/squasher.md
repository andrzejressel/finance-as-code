# Squasher Transformer

This module provides a date-range transformer that replaces all matching transactions with one aggregated transaction.

Use it when you want to compress many entries (for example daily card authorizations) into a single summary row while keeping the same net amount.

## Quick Start

```rust,no_run
use chrono::NaiveDate;
use finance_as_code_budget_core::run;
# use finance_as_code_budget_core::__private::{create_sink, create_source};
use finance_as_code_budget_core::transformer_squasher::{SquasherConfig, create_squasher};

fn main() {
    let squasher = create_squasher(
        SquasherConfig::builder()
            .name("squash-january")
            .from_date(NaiveDate::from_ymd_opt(2025, 1, 1).expect("valid from date"))
            .to_date(NaiveDate::from_ymd_opt(2025, 1, 31).expect("valid to date"))
            .squashed_name("January Card Summary")
            .build(),
    );

    let sources = vec![create_source()];
    let sinks = vec![create_sink()];

    run(vec![], sources, vec![Box::new(squasher)], sinks).expect("Pipeline run failed");
}
```

## Behaviour

- Range matching is inclusive: `from_date <= tx.date <= to_date`.
- Matching transactions are removed and replaced with one synthetic transaction.
- The synthetic transaction uses `squashed_name` as description and `to_date` as date.
- If matching transactions use multiple currencies, the transformer returns input unchanged.
