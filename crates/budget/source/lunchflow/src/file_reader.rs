use crate::api::LunchFlowTransactions;
use finance_as_code_budget_core::{BankTransaction, FileReader, TransactionHolder};
use rootcause::option_ext::OptionExt;
use rootcause::prelude::ResultExt;
use rootcause::*;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use rusty_money::iso::find;

pub(crate) struct LunchflowFileReader {}

impl FileReader for LunchflowFileReader {
    fn name(&self) -> &str {
        "LunchflowFileReader"
    }

    fn read_file(&self, file_data: &[u8]) -> Result<TransactionHolder> {
        let s = std::str::from_utf8(file_data)
            .context("Failed to convert file data to UTF-8 string")?;

        let transactions: LunchFlowTransactions =
            serde_json::from_str(s).context("Failed to deserialize LunchFlowTransactions")?;

        let mut bank_transactions = Vec::new();
        for tx in transactions.transactions {
            let currency = find(&tx.currency)
                .context_with(|| format!("Failed to find currency for code: [{}]", tx.currency))?;

            let decimal = Decimal::from_f64(tx.amount)
                .context_with(|| format!("Failed to convert amount to Decimal: [{}]", tx.amount))?;

            let amount = rusty_money::Money::from_decimal(decimal, currency);

            let description = format!(
                "{} | {}",
                tx.merchant.unwrap_or("".to_string()),
                tx.description.unwrap_or("".to_string())
            );

            let date = chrono::NaiveDate::parse_from_str(&tx.date, "%Y-%m-%d")
                .context_with(|| format!("Failed to parse date: [{}]", tx.date))?;

            let bank_tx = BankTransaction::builder()
                .amount(amount)
                .description(description)
                .date(date)
                .build();

            bank_transactions.push(bank_tx);
        }

        Ok(TransactionHolder::new(bank_transactions))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lunchflow_file_reader() {
        let reader = LunchflowFileReader {};

        let json = include_str!("assets/lunchflow.json");
        let transactions = reader.read_file(json.as_bytes()).unwrap();

        insta::assert_debug_snapshot!(transactions);
    }
}
