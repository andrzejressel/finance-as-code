// Based on https://github.com/avl/datetime_literal/blob/master/src/lib.rs

pub mod __private {
    pub use chrono;
}

#[macro_export]
macro_rules! datetime {
    ( $year:literal-$month:literal-$day:literal) => {
        const {
            #[allow(clippy::zero_prefixed_literal)]
            $crate::__private::chrono::NaiveDateTime::new(
                $crate::__private::chrono::NaiveDate::from_ymd_opt($year, $month, $day)
                    .expect("date values must be in valid range"),
                $crate::__private::chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            )
        }
    };
    ( $year:literal-$month:literal-$day:literal $hour:literal:$min:literal:$second:literal) => {
        const {
            #[allow(clippy::zero_prefixed_literal)]
            $crate::__private::chrono::NaiveDateTime::new(
                $crate::__private::chrono::NaiveDate::from_ymd_opt($year, $month, $day)
                    .expect("date values must be in valid range"),
                $crate::__private::chrono::NaiveTime::from_hms_opt($hour, $min, $second)
                    .expect("time values must be in valid range"),
            )
        }
    };
}

#[macro_export]
macro_rules! date {
    ( $year:literal-$month:literal-$day:literal) => {
        const {
            #[allow(clippy::zero_prefixed_literal)]
            $crate::__private::chrono::NaiveDate::from_ymd_opt($year, $month, $day)
                .expect("date values must be in valid range")
        }
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_datetime() {
        const DT: chrono::NaiveDateTime = datetime!(2023-08-15 12:30:45);
        assert_eq!(
            DT,
            chrono::NaiveDate::from_ymd_opt(2023, 8, 15)
                .unwrap()
                .and_hms_opt(12, 30, 45)
                .unwrap()
        );
    }

    #[test]
    fn test_date() {
        const D: chrono::NaiveDate = date!(2023 - 08 - 15);
        assert_eq!(D, chrono::NaiveDate::from_ymd_opt(2023, 8, 15).unwrap());
    }
}
