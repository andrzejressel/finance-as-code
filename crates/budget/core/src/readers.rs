use crate::{FileReader, TransactionHolder};
use log::info;
use rootcause::prelude::ResultExt;
use rootcause::{Result, bail};
use std::path::PathBuf;

pub trait Source {
    fn name(&self) -> String;
    fn read(&self) -> Result<TransactionHolder>;
}

pub struct LocalDirectorySource {
    dir: PathBuf,
    file_reader: Box<dyn FileReader>,
}

impl LocalDirectorySource {
    pub fn new(dir: PathBuf, file_reader: impl FileReader + 'static) -> Result<impl Source> {
        if !dir.exists() {
            bail!("{:?} does not exist", dir);
        }
        if !dir.is_dir() {
            bail!("{:?} is not a directory", dir);
        }
        Ok(LocalDirectorySource {
            dir,
            file_reader: Box::new(file_reader),
        })
    }
}

impl Source for LocalDirectorySource {
    fn name(&self) -> String {
        format!(
            "LocalDirectorySource([{:?}], [{:?}])",
            self.dir,
            self.file_reader.name()
        )
    }
    fn read(&self) -> Result<TransactionHolder> {
        let mut paths: Vec<PathBuf> = Vec::new();

        for entry in std::fs::read_dir(&self.dir)
            .context_with(|| format!("Cannot read directory [{:?}]", self.dir))?
        {
            let entry =
                entry.context_with(|| format!("Failed to read directory  entry {:?}", self.dir))?;
            if entry
                .file_type()
                .context_with(|| format!("Failed to get file type for entry [{:?}]", entry.path()))?
                .is_file()
            {
                paths.push(entry.path());
            }
        }

        let mut holders = Vec::new();

        paths.sort_by(|p1, p2| {
            natord::compare(
                p1.file_name().unwrap().to_str().unwrap(),
                p2.file_name().unwrap().to_str().unwrap(),
            )
        });

        for path in paths {
            let file_data = std::fs::read(&path)
                .context_with(|| format!("Failed to read file [{:?}]", path))?;
            info!(
                "Reading file [{:?}] with [{:?}]",
                path,
                self.file_reader.name()
            );
            let import_result = self
                .file_reader
                .read_file(&file_data)
                .context_with(|| format!("Failed to read file [{:?}]", path))?;
            info!(
                "Read [{}] transactions successfully",
                import_result.number_of_transactions()
            );
            holders.push(import_result);
        }

        Ok(TransactionHolder::combine_vec(holders))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MockFileReader;
    use googletest::assert_that;
    use mockall::Sequence;
    use mockall::predicate::eq;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn should_return_error_when_provided_directory_does_not_exist() {
        let non_existent_dir = PathBuf::from("non_existent_directory");
        let file_reader = MockFileReader::new();

        let result = LocalDirectorySource::new(non_existent_dir, file_reader);

        assert!(result.is_err());
        let error_message = format!("{}", result.err().unwrap());
        assert!(error_message.contains("does not exist"));
    }

    #[test]
    fn should_return_error_when_provided_directory_is_existing_file() {
        let tmp_file = tempfile::NamedTempFile::new().unwrap();
        let file_reader = MockFileReader::new();

        let result = LocalDirectorySource::new(tmp_file.path().to_owned(), file_reader);

        assert!(result.is_err());
        let error_message = format!("{}", result.err().unwrap());
        assert!(error_message.contains("is not a directory"));
    }

    #[test]
    fn files_are_read_in_natural_order() {
        let tmp_dir = tempfile::tempdir().unwrap();

        let file1 = tmp_dir.path().join("file1.txt");
        let file2 = tmp_dir.path().join("file2.txt");
        let file3 = tmp_dir.path().join("file3.txt");
        let file11 = tmp_dir.path().join("file11.txt");

        let mut file = File::create(&file1).unwrap();
        file.write_all("file1".as_bytes()).unwrap();

        let mut file = File::create(&file2).unwrap();
        file.write_all("file2".as_bytes()).unwrap();

        let mut file = File::create(&file3).unwrap();
        file.write_all("file3".as_bytes()).unwrap();

        let mut file = File::create(&file11).unwrap();
        file.write_all("file11".as_bytes()).unwrap();

        let mut seq = Sequence::new();
        let mut file_reader = MockFileReader::new();
        file_reader
            .expect_read_file()
            .with(eq("file1".as_bytes()))
            .in_sequence(&mut seq)
            .returning(|_| Ok(TransactionHolder::empty()));
        file_reader
            .expect_read_file()
            .with(eq("file2".as_bytes()))
            .in_sequence(&mut seq)
            .returning(|_| Ok(TransactionHolder::empty()));
        file_reader
            .expect_read_file()
            .with(eq("file3".as_bytes()))
            .in_sequence(&mut seq)
            .returning(|_| Ok(TransactionHolder::empty()));
        file_reader
            .expect_read_file()
            .with(eq("file11".as_bytes()))
            .in_sequence(&mut seq)
            .returning(|_| Ok(TransactionHolder::empty()));

        let reader = LocalDirectorySource::new(tmp_dir.path().to_owned(), file_reader).unwrap();

        let import_result = reader.read().unwrap();

        assert_that!(
            import_result,
            googletest::prelude::eq(&TransactionHolder::empty())
        );
    }
}
