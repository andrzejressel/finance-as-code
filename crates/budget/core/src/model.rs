use chrono::Datelike;
use itertools::Itertools;
use std::fmt::Display;
use uuid::Uuid;

pub fn join_non_empty(parts: &[&str], separator: &str) -> String {
    parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .join(separator)
}

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
        if !(1..=12).contains(&month) {
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
}
