use crate::mbank::MBankReader;
use finance_as_code_budget_core::FileReader;

mod mbank;

pub fn create_mbank_file_reader() -> impl FileReader {
    MBankReader {}
}
