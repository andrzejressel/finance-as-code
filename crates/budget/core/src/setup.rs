use rootcause::Result;

/// A trait for performing side effects before the main pipeline runs.
///
/// Setup implementations are run before sources and can perform operations like
/// downloading files, creating directories, or other preparatory work.
pub trait Setup {
    fn name(&self) -> &str;
    fn run(&self) -> Result<()>;
}
