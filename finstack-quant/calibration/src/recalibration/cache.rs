//! Per-key once-only memo shared by the rate and hazard recalibration caches.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

/// Concurrent memo that computes each key's value at most once.
///
/// Every key owns its own mutex, so concurrent callers requesting the same key
/// share the in-flight computation while different keys proceed in parallel.
/// Failed computations are not cached, preserving the original error for the
/// caller. Poisoned locks are recovered because the guarded state is only ever
/// `None` or a fully constructed `Arc<V>`.
pub(crate) struct KeyedOnceCache<K, V> {
    entries: Mutex<HashMap<K, Slot<V>>>,
}

/// Per-key slot: `None` while the first computation is in flight.
type Slot<V> = Arc<Mutex<Option<Arc<V>>>>;

impl<K, V> Default for KeyedOnceCache<K, V> {
    fn default() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }
}

impl<K: Eq + Hash, V> KeyedOnceCache<K, V> {
    /// Number of keys that have an entry (computed or in flight).
    pub(crate) fn len(&self) -> usize {
        match self.entries.lock() {
            Ok(entries) => entries.len(),
            Err(poisoned) => poisoned.into_inner().len(),
        }
    }

    /// Return the value for `key`, computing it with `compute` on first use.
    ///
    /// When `cache` is `None` the value is computed directly and not memoised,
    /// which is the uncached public replay path.
    ///
    /// # Arguments
    ///
    /// * `key` - Memo key identifying the recalibration request.
    /// * `compute` - Fallible constructor invoked at most once per key.
    pub(crate) fn get_or_compute(
        &self,
        key: K,
        compute: impl FnOnce() -> finstack_quant_core::Result<V>,
    ) -> finstack_quant_core::Result<Arc<V>> {
        let entry = {
            let mut entries = match self.entries.lock() {
                Ok(entries) => entries,
                Err(poisoned) => poisoned.into_inner(),
            };
            Arc::clone(
                entries
                    .entry(key)
                    .or_insert_with(|| Arc::new(Mutex::new(None))),
            )
        };
        let mut cached = match entry.lock() {
            Ok(cached) => cached,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(value) = cached.as_ref() {
            return Ok(Arc::clone(value));
        }
        let value = Arc::new(compute()?);
        *cached = Some(Arc::clone(&value));
        Ok(value)
    }
}
