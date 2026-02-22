use finance_as_code_budget_core::TagMap;

/// Extension trait for `TagMap` to set and get LunchMoney specific tags.
pub trait LunchMoneyTags {
    /// Sets the category name for a transaction. Categories can be found in [Setup -> Categories](https://my.lunchmoney.app/categories)
    fn set_category_name(&mut self, category_name: String);
    fn get_category_name(&self) -> Option<&String>;
}

impl LunchMoneyTags for TagMap {
    fn set_category_name(&mut self, category_name: String) {
        self.insert(CATEGORY_NAME_KEY.to_string(), category_name);
    }

    fn get_category_name(&self) -> Option<&String> {
        self.get(&CATEGORY_NAME_KEY.to_string())
    }
}

const CATEGORY_NAME_KEY: &str = "lunchmoney_category_name";

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::assert_that;
    use googletest::prelude::*;

    #[test]
    fn test_set_and_get_category_name() {
        let mut tags = TagMap::new();
        let category_name = "Groceries".to_string();

        tags.set_category_name(category_name.clone());
        assert_that!(tags.get_category_name(), some(eq(&category_name)));
    }
}
