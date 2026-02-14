use chrono::{Datelike, Month, Months};
use std::fmt::Display;
use uuid::Uuid;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct AccountId(Uuid);

impl AccountId {
    pub fn new(uuid: Uuid) -> Self {
        AccountId(uuid)
    }

    pub fn value(&self) -> Uuid {
        self.0
    }
}

impl Display for AccountId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MonthYear {
    month: u32,
    year: i32,
}

impl MonthYear {
    pub fn new(month: u32, year: i32) -> Self {
        if month < 1 || month > 12 {
            panic!("Month must be between 1 and 12");
        }
        MonthYear { month, year }
    }

    pub fn from_date(date: chrono::NaiveDate) -> Self {
        MonthYear {
            month: date.month(),
            year: date.year(),
        }
    }

    fn get_first_and_last_day(&self) -> (chrono::NaiveDate, chrono::NaiveDate) {
        let first_day = chrono::NaiveDate::from_ymd_opt(self.year, self.month, 1).unwrap();
        let last_day = first_day
            .checked_add_months(Months::new(1))
            .unwrap()
            .pred_opt()
            .unwrap();
        (first_day, last_day)
    }
}
