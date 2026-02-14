use crate::Transaction;

pub trait Sink {
    fn name(&self) -> &str;
    fn write(&self, transactions: &[Transaction]) -> rootcause::Result<()>;
}
