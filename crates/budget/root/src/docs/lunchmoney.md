# Lunch Money quick start

Use this module when you want to prepare categories and import pipeline transactions into Lunch Money.

## Replace category hierarchy

Use this setup when you want to reset Lunch Money categories and apply your own hierarchy before importing transactions.

```rust,no_run
use finance_as_code_budget::{
    Setup, run,
    lunchmoney::{
        CategoryHierarchyItem, ChildCategoryHierarchyItem, LunchMoneyCategorySetupConfig,
        create_lunchmoney_category_setup,
    },
};
# use finance_as_code_budget::__private::{create_sink, create_source};

fn main() {
    let setups: Vec<Box<dyn Setup>> = vec![Box::new(create_lunchmoney_category_setup(
        LunchMoneyCategorySetupConfig::builder()
            .api_key("lunchmoney_api_key")
            .categories(vec![
                CategoryHierarchyItem::builder()
                    .name("Transport")
                    .description("Transportation costs")
                    .is_income(false)
                    .exclude_from_budget(false)
                    .exclude_from_totals(false)
                    .children(vec![ChildCategoryHierarchyItem::builder().name("Fuel").build()])
                    .build(),
            ])
            .build(),
    ))];

    let sources = vec![create_source()];
    let sinks = vec![create_sink()];

    run(setups, sources, vec![], sinks).expect("Pipeline run failed");
}
```

## Lunch Money sink only

This sink fully replaces transactions for the selected account: existing transactions in that account are removed before import.

```rust,no_run
use finance_as_code_budget::{
    Sink, run,
    lunchmoney::{LunchMoneySinkConfig, create_lunchmoney_sink},
};
# use finance_as_code_budget::__private::create_source;

fn main() {
    let sources = vec![create_source()];

    let sinks: Vec<Box<dyn Sink>> = vec![Box::new(create_lunchmoney_sink(
        LunchMoneySinkConfig::builder()
            .api_key("lunchmoney_api_key")
            .account_name("My Account")
            .build(),
    ))];

    run(vec![], sources, vec![], sinks).expect("Pipeline run failed");
}
```
