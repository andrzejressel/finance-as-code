use finance_as_code_budget_core::TagMap;

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
        self.get(CATEGORY_NAME_KEY)
    }
}

const CATEGORY_NAME_KEY: &str = "lunchmoney_category_name";
