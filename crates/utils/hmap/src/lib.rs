use std::any::{Any, type_name};
use std::collections::HashMap;
use std::hash::Hash;

/// Heterogeneous map with typed accessors.
///
/// Values are stored behind `Any` and recovered with explicit type parameters.
/// This is useful as a low-level building block for type-safe APIs layered on
/// top, for example when keys are enum variants.
///
/// If a key exists but the requested type does not match the stored concrete
/// type, methods panic instead of returning `None`. Missing keys still return
/// `None`.
///
/// # Examples
///
/// ```
/// use finance_as_code_utils_hmap::HMap;
///
/// #[derive(Debug, PartialEq, Eq, Hash)]
/// enum Key {
///     Name,
///     Age,
///     City,
/// }
///
/// let mut map = HMap::new();
/// map.insert(Key::Name, String::from("Alice"));
/// map.insert(Key::Age, 30_i64);
///
/// assert_eq!(map.get::<String>(&Key::Name).map(String::as_str), Some("Alice"));
/// assert_eq!(map.get::<i64>(&Key::Age), Some(&30));
/// assert_eq!(map.get::<String>(&Key::City), None);
/// ```
pub struct HMap<K> {
    values: HashMap<K, Box<dyn Any>>,
}

impl<K> HMap<K>
where
    K: Eq + Hash,
{
    #[must_use]
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn insert<T>(&mut self, key: K, value: T) -> Option<T>
    where
        T: 'static,
    {
        if let Some(existing) = self.values.get(&key) {
            assert_type_match::<T>(existing.as_ref(), "insert");
        }

        self.values
            .insert(key, Box::new(value)).map(|old| old.downcast::<T>().unwrap())
            .map(|old| *old)
    }

    #[must_use]
    pub fn get<T>(&self, key: &K) -> Option<&T>
    where
        T: 'static,
    {
        match self.values.get(key) {
            Some(value) => {
                assert_type_match::<T>(value.as_ref(), "get");
                Some(value.downcast_ref::<T>().unwrap())
            }
            None => None,
        }
    }

    pub fn get_mut<T>(&mut self, key: &K) -> Option<&mut T>
    where
        T: 'static,
    {
        match self.values.get_mut(key) {
            Some(value) => {
                assert_type_match::<T>(value.as_ref(), "get_mut");
                Some(value.downcast_mut::<T>().unwrap())
            }
            None => None,
        }
    }

    pub fn remove<T>(&mut self, key: &K) -> Option<T>
    where
        T: 'static,
    {
        if let Some(existing) = self.values.get(key) {
            assert_type_match::<T>(existing.as_ref(), "remove");
        }

        self.values
            .remove(key).map(|value| value.downcast::<T>().unwrap())
            .map(|value| *value)
    }

    #[must_use]
    pub fn contains_key(&self, key: &K) -> bool {
        self.values.contains_key(key)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }
}

impl<K> Default for HMap<K>
where
    K: Eq + Hash,
{
    fn default() -> Self {
        Self::new()
    }
}

fn assert_type_match<T>(value: &dyn Any, operation: &str)
where
    T: 'static,
{
    assert!(
        value.downcast_ref::<T>().is_some(),
        "type mismatch in HMap::{operation}: requested `{}`",
        type_name::<T>()
    );
}

#[cfg(test)]
mod tests {
    use super::HMap;
    use googletest::prelude::*;

    #[derive(Debug, PartialEq, Eq, Hash)]
    enum Key {
        Name,
        Age,
        IsActive,
    }

    #[test]
    fn stores_and_reads_multiple_types() {
        let mut map = HMap::new();

        map.insert(Key::Name, String::from("Alice"));
        map.insert(Key::Age, 30_i64);

        assert_that!(
            map.get::<String>(&Key::Name).map(String::as_str),
            eq(Some("Alice"))
        );
        assert_that!(map.get::<i64>(&Key::Age), eq(Some(&30)));
        assert_that!(map.len(), eq(2));
    }

    #[test]
    fn gets_values_of_different_types() {
        let mut map = HMap::new();

        map.insert(Key::Name, String::from("Alice"));
        map.insert(Key::Age, 30_i64);
        map.insert(Key::IsActive, true);

        assert_that!(
            map.get::<String>(&Key::Name).map(String::as_str),
            eq(Some("Alice"))
        );
        assert_that!(map.get::<i64>(&Key::Age), eq(Some(&30)));
        assert_that!(map.get::<bool>(&Key::IsActive), eq(Some(&true)));
    }

    #[test]
    #[should_panic(expected = "type mismatch in HMap::get")]
    fn get_panics_for_type_mismatch() {
        let mut map = HMap::new();
        map.insert(Key::Name, String::from("Alice"));

        map.get::<i64>(&Key::Name).unwrap();
    }

    #[test]
    #[should_panic(expected = "type mismatch in HMap::get_mut")]
    fn get_mut_panics_for_type_mismatch() {
        let mut map = HMap::new();
        map.insert(Key::Name, String::from("Alice"));

        map.get_mut::<i64>(&Key::Name).unwrap();
    }

    #[test]
    fn get_mut_updates_value() {
        let mut map = HMap::new();
        map.insert(Key::Age, 30_i64);

        *map.get_mut::<i64>(&Key::Age).expect("age should exist") = 31;

        assert_that!(map.get::<i64>(&Key::Age), eq(Some(&31)));
    }

    #[test]
    fn remove_returns_typed_value() {
        let mut map = HMap::new();
        map.insert(Key::Name, String::from("Alice"));

        let removed = map.remove::<String>(&Key::Name);

        assert_that!(removed.as_deref(), eq(Some("Alice")));
        assert_that!(map.contains_key(&Key::Name), eq(false));
        assert_that!(map.len(), eq(0));
    }

    #[test]
    fn insert_returns_previous_value_for_same_type() {
        let mut map = HMap::new();

        assert_that!(map.insert(Key::Age, 30_i64), none());
        assert_that!(map.insert(Key::Age, 31_i64), eq(Some(30)));
        assert_that!(map.get::<i64>(&Key::Age), eq(Some(&31)));
        assert_that!(map.len(), eq(1));
    }

    #[test]
    #[should_panic(expected = "type mismatch in HMap::insert")]
    fn insert_panics_for_type_mismatch() {
        let mut map = HMap::new();
        map.insert(Key::Age, 31_i64);

        let _ = map.insert(Key::Age, String::from("thirty-one"));
    }

    #[test]
    #[should_panic(expected = "type mismatch in HMap::remove")]
    fn remove_panics_for_type_mismatch() {
        let mut map = HMap::new();
        map.insert(Key::Name, String::from("Alice"));

        let _ = map.remove::<i64>(&Key::Name);
    }

    #[test]
    fn clear_empties_map() {
        let mut map = HMap::new();
        map.insert(Key::Name, String::from("Alice"));
        map.insert(Key::Age, 30_i64);

        map.clear();

        assert_that!(map.is_empty(), eq(true));
        assert_that!(map.len(), eq(0));
    }
}
