use rootcause::Result;

/// A trait for performing side effects before the main pipeline runs.
///
/// Setup implementations are run before sources and can perform operations like
/// downloading files, creating directories, or other preparatory work.
pub trait Setup {
    fn name(&self) -> &str;
    fn run(&self) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestSetup {
        name: String,
        should_fail: bool,
    }

    impl Setup for TestSetup {
        fn name(&self) -> &str {
            &self.name
        }

        fn run(&self) -> Result<()> {
            if self.should_fail {
                rootcause::bail!("Test setup failed")
            }
            Ok(())
        }
    }

    #[test]
    fn test_setup_trait_can_succeed() {
        let setup = TestSetup {
            name: "test".to_string(),
            should_fail: false,
        };
        assert!(setup.run().is_ok());
        assert_eq!(setup.name(), "test");
    }

    #[test]
    fn test_setup_trait_can_fail() {
        let setup = TestSetup {
            name: "test".to_string(),
            should_fail: true,
        };
        assert!(setup.run().is_err());
    }
}
