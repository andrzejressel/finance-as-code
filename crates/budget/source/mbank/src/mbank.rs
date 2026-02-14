use bon::Builder;
use chrono::NaiveDate;
use encoding_rs::WINDOWS_1250;
use finance_as_code_budget_core::{
    BankTransaction, FileReader, NonEmptyString, TransactionHolder,
};
use rootcause::option_ext::OptionExt;
use rootcause::prelude::ResultExt;
use rootcause::*;
use rust_decimal::{Decimal, dec};
use rusty_money::{Money, iso};
use std::str::FromStr;

/// Parses Polish currency format (e.g., "-1 234,56" or "3,57") and returns only the numeric value.
// Function to parse amount
fn parse_amount(amount_str: &str) -> Result<Decimal> {
    let trimmed = amount_str.trim();

    if trimmed == "-" || trimmed.is_empty() {
        return Ok(dec!(0));
    }

    // Format: "-2,72" or "14,27" or "1 234,56"
    // Sometimes it might have currency suffix " PLN" (though usually separate column or header)
    // The previous implementation handled spaces and commas.

    // Remove spaces and replace comma with dot for parsing
    let normalized = trimmed.replace(' ', "").replace(',', "."); // Handle non-breaking space

    // Filter out chars that are not digits, dot or minus
    let clean_str: String = normalized
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();

    let value = Decimal::from_str(&clean_str)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        .context(format!("Cannot parse amount '{}'", amount_str))?;

    Ok(value)
}

#[derive(Builder)]
pub(crate) struct MBankReader {}

impl FileReader for MBankReader {
    fn name(&self) -> &str {
        "MBankReader"
    }
    fn read_file(&self, file_data: &[u8]) -> Result<TransactionHolder> {
        // 1. Decode from Windows-1250 to UTF-8

        let (cow, _encoding_used, _had_errors) = WINDOWS_1250.decode(file_data);
        let content = cow.into_owned();
        let lines: Vec<&str> = content.lines().collect();

        // 2. Find Currency from Header
        let currency_row_index = lines
            .iter()
            .position(|line| line.starts_with("#Waluta"))
            .context("Failed to find (#Waluta) header")?;

        // Next line after #Waluta should contain the currency code e.g. "PLN;"
        if currency_row_index + 1 >= lines.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Currency value missing after header",
            )
            .into());
        }
        let currency_line = lines[currency_row_index + 1];
        let currency_code = currency_line.trim().trim_end_matches(';');

        // Map any error to default Report type
        let currency = iso::find(currency_code)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Unknown currency code: {}", currency_code),
                )
            })
            .context("Is the currency code valid?")?;

        // 3. Find Data Header
        let header_index = lines
            .iter()
            .position(|line| line.starts_with("#Data księgowania"))
            .context("Data header (#Data księgowania) not found")?;

        // 4. Parse CSV
        // Content from header line onwards and remove last lines
        let csv_content = lines[header_index..lines.len() - 1].join("\n");
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b';')
            .has_headers(true)
            .from_reader(csv_content.as_bytes());

        let mut transactions = Vec::new();

        for result in reader.records() {
            let record = result.context("Failed to read CSV record")?;

            // Index 0: Data księgowania
            // Index 1: Opis operacji
            // Index 2: Tytuł
            // Index 3: Nadawca/Odbiorca
            // Index 4: Numer konta
            // Index 5: Kwota
            // Index 6: Saldo po operacji

            if record.len() < 7 {
                bail!("CSV record has insufficient fields: [{:?}]", record);
            }

            let date_str = record.get(0).unwrap_or("").trim();
            let description = record.get(2).unwrap_or("").trim();
            let title = record.get(3).unwrap_or("").trim();
            let sender_receiver = record.get(4).unwrap_or("").trim();
            let account_number = record.get(5).unwrap_or("").trim();
            let amount_str = record.get(6).unwrap_or("").trim();

            if date_str.is_empty() || amount_str.is_empty() {
                continue;
            }

            // Parse date "2025-12-29"
            let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .context(format!("Failed to parse date '{}'", date_str))?;

            let amount_decimal = parse_amount(amount_str)?;
            let money_amount = Money::from_decimal(amount_decimal, currency);

            let full_description = format!("{} | {} | {}", description, title, sender_receiver);
            let clean_desc = full_description.trim().trim_matches('|').trim().to_string();

            let account_number = account_number.trim_matches(|c| c == '\'');

            let transaction = BankTransaction::builder()
                .date(date)
                .description(clean_desc)
                .amount(money_amount)
                .maybe_other_side_account_number(NonEmptyString::new(account_number.to_string()))
                .build();

            transactions.push(transaction);
        }

        Ok(TransactionHolder::new(transactions))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    mod parse_amount {
        use super::*;

        #[test]
        fn test_parse_amount_positive() {
            let result = parse_amount("1 234,56").unwrap();
            assert_eq!(result, dec!(1234.56));
        }

        #[test]
        fn test_parse_amount_negative() {
            let result = parse_amount("-1 234,56").unwrap();
            assert_eq!(result, dec!(-1234.56));
        }

        #[test]
        fn test_parse_amount_simple() {
            let result = parse_amount("3,57").unwrap();
            assert_eq!(result, dec!(3.57));
        }

        #[test]
        fn test_parse_amount_with_space() {
            let result = parse_amount(" 1 234,56 ").unwrap();
            assert_eq!(result, dec!(1234.56));
        }

        #[test]
        fn test_parse_amount_dash() {
            let result = parse_amount("-").unwrap();
            assert_eq!(result, dec!(0));
        }
    }

    use std::path::PathBuf;

    fn get_test_asset_path(filename: &str) -> PathBuf {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        PathBuf::from(manifest_dir)
            .join("src")
            .join("assets")
            .join("mbank")
            .join(filename)
    }

    #[test]
    fn test_mbank_snapshot_operations_summary() {
        let path = get_test_asset_path("mbank_operations_summary.csv");
        let content = std::fs::read(&path).expect("failed to read file");

        let reader = MBankReader::builder().build();
        let transactions = reader.read_file(&content).expect("failed to parse file");

        insta::assert_debug_snapshot!(transactions);
    }
}
