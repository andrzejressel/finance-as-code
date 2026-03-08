use rootcause::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

/// Trait for file-based key-value storage with string keys and string values.
pub trait FileStringMap {
    /// Retrieves a value by key. Returns `None` if the key doesn't exist.
    fn get(&self, key: &str) -> Option<&str>;

    /// Inserts a key-value pair. Returns the previous value if the key existed.
    /// Persists changes to the file immediately.
    fn put(&mut self, key: &str, value: &str) -> Result<Option<String>>;

    /// Returns the path to the underlying file.
    fn path(&self) -> &Path;
}

/// A file-based string-to-string map with in-memory caching.
///
/// This struct stores key-value pairs in a JSON file. It maintains an in-memory
/// cache to avoid reading the file on every operation. Changes are persisted
/// to the file immediately when `put` is called.
///
/// If the file does not exist, it will be created automatically.
///
/// # Examples
///
/// ```
/// use finance_as_code_utils_file_map::{FileStringMap, JsonFileMap};
/// use tempfile::NamedTempFile;
///
/// let temp_file = NamedTempFile::new().unwrap();
/// let path = temp_file.path().to_path_buf();
///
/// let mut map = JsonFileMap::new(&path).unwrap();
///
/// let _ = map.put("name", "Alice");
/// assert_eq!(map.get("name"), Some("Alice"));
/// assert_eq!(map.get("unknown"), None);
/// ```
pub struct JsonFileMap {
    path: PathBuf,
    cache: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Default)]
struct FileData {
    data: HashMap<String, String>,
}

impl JsonFileMap {
    /// Creates a new `JsonFileMap` backed by the specified file.
    ///
    /// If the file exists, its contents are loaded into the cache.
    /// If the file does not exist, an empty map is created and the file is
    /// initialized. If the file exists but is empty, it is treated as an
    /// empty map.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file exists but contains invalid JSON (and is not empty)
    /// - The file exists but cannot be read
    /// - The file does not exist and cannot be created
    /// - The parent directory does not exist and cannot be created
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Ensure parent directory exists
        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent)?;
        }

        let cache = if path.exists() {
            // Check if file is empty
            let metadata = fs::metadata(&path)?;
            if metadata.len() == 0 {
                // File exists but is empty, initialize it
                Self::initialize_empty_file(&path)?;
                HashMap::new()
            } else {
                Self::load_from_file(&path)?
            }
        } else {
            // Create empty file
            Self::initialize_empty_file(&path)?;
            HashMap::new()
        };

        Ok(Self { path, cache })
    }

    fn initialize_empty_file(path: &Path) -> Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        let empty_data = FileData::default();
        serde_json::to_writer(&mut writer, &empty_data)?;
        writer.flush()?;
        Ok(())
    }

    fn load_from_file(path: &Path) -> Result<HashMap<String, String>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let data: FileData = serde_json::from_reader(reader)?;
        Ok(data.data)
    }

    fn persist_to_file(&self) -> Result<()> {
        let file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&self.path)?;

        let mut writer = BufWriter::new(file);
        let data = FileData {
            data: self.cache.clone(),
        };
        serde_json::to_writer(&mut writer, &data)?;
        writer.flush()?;
        Ok(())
    }
}

impl FileStringMap for JsonFileMap {
    fn get(&self, key: &str) -> Option<&str> {
        self.cache.get(key).map(|s| s.as_str())
    }

    fn put(&mut self, key: &str, value: &str) -> Result<Option<String>> {
        let previous = self.cache.insert(key.to_string(), value.to_string());
        self.persist_to_file()?;
        Ok(previous)
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use tempfile::NamedTempFile;

    fn create_temp_map() -> (NamedTempFile, JsonFileMap) {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();
        let map = JsonFileMap::new(&path).unwrap();
        (temp_file, map)
    }

    #[test]
    fn creates_new_file_when_not_exists() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("new_map.json");

        assert_that!(&path.exists(), is_false());

        let _map = JsonFileMap::new(&path).unwrap();

        assert_that!(&path.exists(), is_true());
    }

    #[test]
    fn creates_parent_directories_if_needed() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("nested").join("dir").join("map.json");

        assert_that!(&path.exists(), is_false());

        let _map = JsonFileMap::new(&path).unwrap();

        assert_that!(&path.exists(), is_true());
    }

    #[test]
    fn stores_and_retrieves_values() {
        let (_temp_file, mut map) = create_temp_map();

        map.put("name", "Alice").unwrap();
        map.put("city", "Warsaw").unwrap();

        assert_that!(map.get("name"), some(eq("Alice")));
        assert_that!(map.get("city"), some(eq("Warsaw")));
        assert_that!(map.get("unknown"), none());
    }

    #[test]
    fn returns_previous_value_on_put() {
        let (_temp_file, mut map) = create_temp_map();

        assert_that!(map.put("key", "value1").unwrap(), none());
        assert_that!(map.put("key", "value2").unwrap(), some(eq("value1")));
        assert_that!(map.get("key"), some(eq("value2")));
    }

    #[test]
    fn persists_data_to_file() {
        let (temp_file, mut map) = create_temp_map();

        map.put("name", "Alice").unwrap();
        map.put("age", "30").unwrap();

        // Read file contents directly
        let contents = fs::read_to_string(temp_file.path()).unwrap();
        let data: serde_json::Value = serde_json::from_str(&contents).unwrap();

        assert_that!(data["data"]["name"].as_str(), some(eq("Alice")));
        assert_that!(data["data"]["age"].as_str(), some(eq("30")));
    }

    #[test]
    fn loads_existing_data_from_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        // Write initial data
        let initial_data = r#"{"data":{"existing":"value"}}"#;
        fs::write(&path, initial_data).unwrap();

        let map = JsonFileMap::new(&path).unwrap();

        assert_that!(map.get("existing"), some(eq("value")));
    }

    #[test]
    fn path_returns_correct_path() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();
        let map = JsonFileMap::new(&path).unwrap();

        assert_that!(map.path(), eq(path.as_path()));
    }

    #[test]
    fn handles_special_characters_in_keys_and_values() {
        let (_temp_file, mut map) = create_temp_map();

        map.put("key with spaces", "value with spaces").unwrap();
        map.put("unicode", "日本語").unwrap();
        map.put("special", "\"quotes\" and \\backslash").unwrap();

        assert_that!(map.get("key with spaces"), some(eq("value with spaces")));
        assert_that!(map.get("unicode"), some(eq("日本語")));
        assert_that!(map.get("special"), some(eq("\"quotes\" and \\backslash")));
    }

    #[test]
    fn returns_error_on_invalid_json_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        // Write invalid JSON (non-empty)
        fs::write(&path, "not valid json").unwrap();

        let result = JsonFileMap::new(&path);

        assert_that!(result.is_err(), is_true());
    }
    #[test]
    fn handles_empty_existing_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Create an empty file.
        fs::write(path, "").unwrap();

        let mut map = JsonFileMap::new(path).unwrap();
        assert_that!(map.get("any"), none());

        map.put("foo", "bar").unwrap();
        assert_that!(map.get("foo"), some(eq("bar")));

        // Re-load to check persistence
        let map2 = JsonFileMap::new(path).unwrap();
        assert_that!(map2.get("foo"), some(eq("bar")));
    }
}
