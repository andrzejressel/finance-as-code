use finance_as_code_budget_core::Transaction;
use finance_as_code_budget_core::sink::Sink;
use rootcause::Result;

pub struct LunchMoneySink;

pub fn create_lunchmoney_sink() -> impl Sink {
    LunchMoneySink
}

impl Sink for LunchMoneySink {
    fn name(&self) -> &str {
        "Lunch Money"
    }

    fn write(&self, _transactions: &[Transaction]) -> Result<()> {
        todo!("Implement Lunch Money sink")
    }
}
