//! A few recently used values, shared by clones.

use std::{
    collections::HashMap,
    hash::Hash,
    sync::{Arc, Mutex},
};

pub(crate) struct Cache<K, V> {
    capacity: usize,
    entries: Arc<Mutex<Entries<K, V>>>,
}

struct Entries<K, V> {
    values: HashMap<K, (Arc<V>, u64)>,
    clock: u64,
}

impl<K, V> Clone for Cache<K, V> {
    fn clone(&self) -> Self {
        Self {
            capacity: self.capacity,
            entries: Arc::clone(&self.entries),
        }
    }
}

impl<K: Eq + Hash + Clone, V> Cache<K, V> {
    /// A cache that keeps at most `capacity` values.
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: Arc::new(Mutex::new(Entries {
                values: HashMap::new(),
                clock: 0,
            })),
        }
    }

    pub(crate) fn get(&self, key: &K) -> Option<Arc<V>> {
        let mut entries = self.entries.lock().expect("cache lock");
        entries.clock += 1;
        let clock = entries.clock;
        let (value, used) = entries.values.get_mut(key)?;
        *used = clock;
        Some(Arc::clone(value))
    }

    /// Keeps `value`, forgetting the least recently used value when full.
    pub(crate) fn insert(&self, key: K, value: V) -> Arc<V> {
        let mut entries = self.entries.lock().expect("cache lock");
        entries.clock += 1;
        let clock = entries.clock;
        if entries.values.len() >= self.capacity
            && !entries.values.contains_key(&key)
            && let Some(oldest) = entries
                .values
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(key, _)| key.clone())
        {
            entries.values.remove(&oldest);
        }
        let value = Arc::new(value);
        entries.values.insert(key, (Arc::clone(&value), clock));
        value
    }

    /// Forgets every value.
    pub(crate) fn clear(&self) {
        self.entries.lock().expect("cache lock").values.clear();
    }
}
