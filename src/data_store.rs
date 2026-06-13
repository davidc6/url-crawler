use mockall::predicate::*;
use mockall::*;
use std::collections::HashMap;
use std::fmt::Debug;

#[derive(Debug, PartialEq)]
pub struct DataStoreEntry {
    pub visited: bool,
    pub urls_found: Vec<String>,
}

#[automock]
pub trait DataStore: Debug {
    fn add(&mut self, key: String, value: Option<String>);
    fn visited(&mut self, key: &str);
    fn has_visited(&self, key: &str) -> bool;
    fn exists(&self, key: &str) -> bool;
    fn get<'a>(&'a self, key: &str) -> Option<&'a DataStoreEntry>;
}

#[derive(Debug, PartialEq)]
pub struct Store {
    data: HashMap<String, DataStoreEntry>,
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

impl Store {
    pub fn new() -> Self {
        Store {
            data: HashMap::<String, DataStoreEntry>::new(),
        }
    }
}

impl DataStore for Store {
    fn add(&mut self, key: String, value: Option<String>) {
        let item = self.data.get_mut(&key);

        if let Some(item) = item {
            if let Some(value) = value {
                return item.urls_found.push(value);
            }
        }

        self.data.insert(
            key.clone(),
            DataStoreEntry {
                visited: false,
                urls_found: vec![],
            },
        );

        if let Some(value) = value {
            let item = self.data.get_mut(&key);

            if let Some(item) = item {
                item.urls_found.push(value)
            }
        }
    }

    fn exists(&self, key: &str) -> bool {
        if let Some(_val) = self.data.get(key) {
            return true;
        }
        false
    }

    fn get<'a>(&'a self, key: &str) -> Option<&'a DataStoreEntry> {
        self.data.get(key)
    }

    fn visited(&mut self, key: &str) {
        let item = self.data.get_mut(key);

        if let Some(item) = item {
            item.visited = true
        }
    }

    fn has_visited(&self, key: &str) -> bool {
        if let Some(key) = self.data.get(key) {
            return key.visited;
        }
        false
    }
}

#[cfg(test)]
mod data_store_tests {
    use crate::data_store::DataStoreEntry;

    use super::{DataStore, Store};

    #[test]
    fn data_store_adds_key_and_value_correctly() {
        let mut s = Store::new();
        let key = "key".to_owned();
        let val = "val".to_owned();

        s.add(key.clone(), Some(val.clone()));

        assert!(s.exists(&key.clone()));
        assert_eq!(
            s.get(&key),
            Some(&DataStoreEntry {
                visited: false,
                urls_found: vec![val]
            })
        );
    }

    #[test]
    fn data_store_updates_value_correctly() {
        let mut s = Store::new();
        let key = "key".to_owned();
        let val = "val".to_owned();
        let val2 = "val2".to_owned();

        s.add(key.clone(), Some(val.clone()));
        s.add(key.clone(), Some(val2.clone()));

        assert!(s.exists(&key.clone()));
        assert_eq!(
            s.get(&key),
            Some(&DataStoreEntry {
                visited: false,
                urls_found: vec![val, val2]
            })
        );
    }

    #[test]
    fn data_store_adds_key_without_value_correctly() {
        let mut s = Store::new();
        let key = "key".to_owned();

        s.add(key.clone(), None);

        assert!(s.exists(&key.clone()));
        assert_eq!(
            s.get(&key),
            Some(&DataStoreEntry {
                visited: false,
                urls_found: vec![]
            })
        );
    }

    #[test]
    fn data_store_set_has_visited_state_to_false_if_not_visited() {
        let mut s = Store::new();
        let key = "key".to_owned();
        let val = Some("val".to_owned());

        s.add(key.clone(), val.clone());

        assert!(!s.has_visited(&key.clone()));
    }

    #[test]
    fn data_store_set_has_visited_state_to_true_if_visited() {
        let mut s = Store::new();
        let key = "key".to_owned();
        let val = Some("val".to_owned());

        s.add(key.clone(), val.clone());
        s.visited(&key);

        assert!(s.has_visited(&key.clone()));
    }
}
