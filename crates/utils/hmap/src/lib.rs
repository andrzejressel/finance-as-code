use std::any::{type_name, Any};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
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
/// Basic usage:
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
///
/// Requesting the wrong type for an existing key panics:
///
/// ```should_panic
/// use finance_as_code_utils_hmap::HMap;
///
/// #[derive(Debug, PartialEq, Eq, Hash)]
/// enum Key {
///     Age,
/// }
///
/// let mut map = HMap::new();
/// map.insert(Key::Age, 30_i64);
///
/// let _ = map.get::<String>(&Key::Age);
/// ```
#[derive(PartialEq)]
pub struct HMap<K: Eq + Hash> {
    values: HashMap<K, Box<dyn DynValue>>,
}

impl<K> Debug for HMap<K>
where
    K: Eq + Hash + Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut keys = self.values.keys().collect::<Vec<_>>();
        keys.sort_by_key(|k| format!("{:?}", k));

        f.debug_struct("HMap")
            .field("len", &self.values.len())
            .field("keys", &keys)
            .finish()
    }
}

trait DynValue: Any + Debug {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn eq_dyn(&self, other: &dyn DynValue) -> bool;
}

impl<T> DynValue for T
where
    T: Any + Debug + PartialEq,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn eq_dyn(&self, other: &dyn DynValue) -> bool {
        other
            .as_any()
            .downcast_ref::<T>()
            .is_some_and(|other_typed| self == other_typed)
    }
}

impl PartialEq for dyn DynValue {
    fn eq(&self, other: &Self) -> bool {
        self.eq_dyn(other)
    }
}

impl<K: Eq + Hash> HMap<K> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn insert<T>(&mut self, key: K, value: T) -> Option<T>
    where
        T: 'static + Debug + PartialEq,
    {
        match self.values.entry(key) {
            Entry::Occupied(mut entry) => {
                assert_type_match::<T>(entry.get().as_ref(), "insert");
                let previous = entry.insert(Box::new(value));
                let previous_any: Box<dyn Any> = previous;
                Some(*previous_any.downcast::<T>().unwrap())
            }
            Entry::Vacant(entry) => {
                entry.insert(Box::new(value));
                None
            }
        }
    }

    #[must_use]
    pub fn get<T>(&self, key: &K) -> Option<&T>
    where
        T: 'static,
    {
        match self.values.get(key) {
            Some(value) => {
                assert_type_match::<T>(value.as_ref(), "get");
                Some(value.as_ref().as_any().downcast_ref::<T>().unwrap())
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
                Some(value.as_mut().as_any_mut().downcast_mut::<T>().unwrap())
            }
            None => None,
        }
    }

    pub fn remove<T>(&mut self, key: &K) -> Option<T>
    where
        T: 'static + Debug + PartialEq,
    {
        if let Some(existing) = self.values.get(key) {
            assert_type_match::<T>(existing.as_ref(), "remove");
        }

        self.values
            .remove(key)
            .map(|value| {
                let value_any: Box<dyn Any> = value;
                value_any.downcast::<T>().unwrap()
            })
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

fn assert_type_match<T>(value: &dyn DynValue, operation: &str)
where
    T: 'static,
{
    assert!(
        value.as_any().downcast_ref::<T>().is_some(),
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
            some(eq("Alice"))
        );
        assert_that!(map.get::<i64>(&Key::Age), some(eq(&30)));
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
            some(eq("Alice"))
        );
        assert_that!(map.get::<i64>(&Key::Age), some(eq(&30)));
        assert_that!(map.get::<bool>(&Key::IsActive), some(eq(&true)));
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

        assert_that!(map.get::<i64>(&Key::Age), some(eq(&31)));
    }

    #[test]
    fn remove_returns_typed_value() {
        let mut map = HMap::new();
        map.insert(Key::Name, String::from("Alice"));

        let removed = map.remove::<String>(&Key::Name);

        assert_that!(removed.as_deref(), some(eq("Alice")));
        assert_that!(map.contains_key(&Key::Name), is_false());
        assert_that!(map.len(), eq(0));
    }

    #[test]
    fn insert_returns_previous_value_for_same_type() {
        let mut map = HMap::new();

        assert_that!(map.insert(Key::Age, 30_i64), none());
        assert_that!(map.insert(Key::Age, 31_i64), some(eq(30)));
        assert_that!(map.get::<i64>(&Key::Age), some(eq(&31)));
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
    fn insert_different_type_for_same_key_keeps_original_value() {
        let mut map = HMap::new();
        map.insert(Key::Age, 31_i64);

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = map.insert(Key::Age, String::from("thirty-one"));
        }));

        assert_that!(panic.is_err(), is_true());
        assert_that!(map.get::<i64>(&Key::Age), some(eq(&31)));
    }

    #[test]
    #[should_panic(expected = "type mismatch in HMap::insert")]
    fn insert_one_type_then_another_type_for_same_key_panics() {
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

        assert_that!(map.is_empty(), is_true());
        assert_that!(map.len(), eq(0));
    }

    #[test]
    fn partial_eq_is_true_for_equal_non_empty_maps() {
        let mut map1: HMap<Key> = HMap::new();
        let mut map2: HMap<Key> = HMap::new();

        map1.insert(Key::Name, String::from("Alice"));
        map2.insert(Key::Name, String::from("Alice"));

        assert_that!(map1 == map2, is_true());
        assert_that!(map1 == map1, is_true());
    }

    #[test]
    fn partial_eq_is_true_for_maps_with_same_values_in_different_insert_order() {
        let mut map1: HMap<Key> = HMap::new();
        let mut map2: HMap<Key> = HMap::new();

        map1.insert(Key::Name, String::from("Alice"));
        map1.insert(Key::Age, 30_i64);
        map2.insert(Key::Age, 30_i64);
        map2.insert(Key::Name, String::from("Alice"));

        assert_that!(map1 == map2, is_true());
    }

    #[test]
    fn partial_eq_is_false_for_different_values() {
        let mut map1: HMap<Key> = HMap::new();
        let mut map2: HMap<Key> = HMap::new();

        map1.insert(Key::Name, String::from("Alice"));
        map2.insert(Key::Name, String::from("Bob"));

        assert_that!(map1 == map2, is_false());
    }

    #[test]
    fn debug_for_empty_map_shows_zero_len_and_no_keys() {
        let map: HMap<Key> = HMap::new();

        assert_that!(format!("{map:?}"), eq("HMap { len: 0, keys: [] }"));
    }

    #[test]
    fn debug_for_non_empty_map_shows_len_and_sorted_keys() {
        let mut map: HMap<Key> = HMap::new();
        map.insert(Key::Name, String::from("Alice"));
        map.insert(Key::Age, 30_i64);

        assert_that!(format!("{map:?}"), eq("HMap { len: 2, keys: [Age, Name] }"));
    }
}
