//! Bounded, thread-safe cache for decoded instrument artifacts.

use std::collections::VecDeque;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use finstack_quant_core::FxHashMap;
use finstack_quant_valuations::instruments::{Instrument, MarketDependencies};

const DEFAULT_MAX_ENTRIES: usize = 4_096;
const DEFAULT_MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct CacheKey {
    content_hash: String,
    instrument_contract_version: u32,
    canonical_version: String,
}

impl CacheKey {
    pub(super) fn new(
        content_hash: String,
        instrument_contract_version: u32,
        canonical_version: &str,
    ) -> Self {
        Self {
            content_hash,
            instrument_contract_version,
            canonical_version: canonical_version.to_string(),
        }
    }
}

#[derive(Clone)]
pub(super) struct CachedArtifact {
    pub(super) instrument: Arc<dyn Instrument>,
    pub(super) dependencies: MarketDependencies,
}

struct CacheEntry {
    artifact: CachedArtifact,
    encoded_bytes: usize,
}

struct CacheState {
    entries: FxHashMap<CacheKey, CacheEntry>,
    least_recently_used: VecDeque<CacheKey>,
    encoded_bytes: usize,
    decode_count: usize,
}

impl CacheState {
    fn new() -> Self {
        Self {
            entries: FxHashMap::default(),
            least_recently_used: VecDeque::new(),
            encoded_bytes: 0,
            decode_count: 0,
        }
    }

    fn touch(&mut self, key: &CacheKey) {
        if let Some(index) = self
            .least_recently_used
            .iter()
            .position(|candidate| candidate == key)
        {
            self.least_recently_used.remove(index);
        }
        self.least_recently_used.push_back(key.clone());
    }

    fn evict_one(&mut self) -> bool {
        let Some(key) = self.least_recently_used.pop_front() else {
            return false;
        };
        if let Some(entry) = self.entries.remove(&key) {
            self.encoded_bytes = self.encoded_bytes.saturating_sub(entry.encoded_bytes);
        }
        true
    }
}

/// Shared content-addressed cache for decoded instrument artifacts.
///
/// Entries are keyed by canonical content hash, instrument contract version,
/// and canonicalization version. The least-recently-used entry is evicted when
/// either the entry or encoded-byte budget would be exceeded. Returned
/// [`Arc`] values remain valid after eviction.
///
/// Cache misses are decoded while holding the write lock, so concurrent
/// requests for one key perform at most one successful decode. If a thread
/// panics while holding the standard-library lock, later operations recover
/// the protected state from the poisoned lock rather than failing permanently.
pub struct InstrumentArtifactCache {
    state: RwLock<CacheState>,
    max_entries: usize,
    max_bytes: usize,
}

impl InstrumentArtifactCache {
    /// Create a cache with a caller-selected entry budget.
    ///
    /// The encoded-byte budget remains the bounded 64 MiB default. A capacity
    /// of zero disables retention while preserving correct materialization.
    ///
    /// # Arguments
    ///
    /// * `max_entries` - Maximum number of decoded artifacts retained.
    ///
    /// # Returns
    ///
    /// An empty thread-safe cache with the requested entry budget.
    #[must_use]
    pub fn with_capacity(max_entries: usize) -> Self {
        Self::with_limits(max_entries, DEFAULT_MAX_BYTES)
    }

    /// Create a cache with explicit entry and encoded-byte budgets.
    ///
    /// # Arguments
    ///
    /// * `max_entries` - Maximum number of decoded artifacts retained.
    /// * `max_bytes` - Maximum sum of source-envelope bytes represented by
    ///   retained artifacts.
    ///
    /// # Returns
    ///
    /// An empty thread-safe cache with both requested budgets.
    #[must_use]
    pub fn with_limits(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            state: RwLock::new(CacheState::new()),
            max_entries,
            max_bytes,
        }
    }

    /// Return the current number of retained decoded artifacts.
    ///
    /// # Returns
    ///
    /// Entry count bounded by the cache's configured capacity.
    #[must_use]
    pub fn len(&self) -> usize {
        read_state(&self.state).entries.len()
    }

    /// Return whether the cache currently retains no decoded artifacts.
    ///
    /// # Returns
    ///
    /// `true` when [`Self::len`] is zero.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return the cumulative number of successful cache-miss decodes.
    ///
    /// This instrumentation distinguishes unique semantic decodes from
    /// position count and allows callers to verify warm-cache behavior.
    ///
    /// # Returns
    ///
    /// Successful artifact decodes performed by this cache instance.
    #[must_use]
    pub fn decode_count(&self) -> usize {
        read_state(&self.state).decode_count
    }

    pub(super) fn get_or_decode<F>(
        &self,
        key: CacheKey,
        encoded_bytes: usize,
        decode: F,
    ) -> crate::Result<(CachedArtifact, bool)>
    where
        F: FnOnce() -> crate::Result<CachedArtifact>,
    {
        // Decode while holding the write lock so concurrent loads cannot
        // compile the same content-addressed artifact more than once.
        let mut state = write_state(&self.state);
        if let Some(entry) = state.entries.get(&key) {
            let artifact = entry.artifact.clone();
            state.touch(&key);
            return Ok((artifact, true));
        }

        let artifact = decode()?;
        state.decode_count = state.decode_count.saturating_add(1);
        if self.max_entries == 0 || self.max_bytes == 0 || encoded_bytes > self.max_bytes {
            return Ok((artifact, false));
        }

        while state.entries.len() >= self.max_entries
            || state.encoded_bytes.saturating_add(encoded_bytes) > self.max_bytes
        {
            if !state.evict_one() {
                break;
            }
        }

        state.encoded_bytes = state.encoded_bytes.saturating_add(encoded_bytes);
        state.entries.insert(
            key.clone(),
            CacheEntry {
                artifact: artifact.clone(),
                encoded_bytes,
            },
        );
        state.touch(&key);
        Ok((artifact, false))
    }
}

impl Default for InstrumentArtifactCache {
    fn default() -> Self {
        Self::with_limits(DEFAULT_MAX_ENTRIES, DEFAULT_MAX_BYTES)
    }
}

fn read_state(lock: &RwLock<CacheState>) -> RwLockReadGuard<'_, CacheState> {
    match lock.read() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn write_state(lock: &RwLock<CacheState>) -> RwLockWriteGuard<'_, CacheState> {
    match lock.write() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;

    use super::CacheKey;
    use super::InstrumentArtifactCache;
    use finstack_quant_core::FxHashMap;

    #[test]
    fn same_hash_with_different_version_axes_misses() {
        let baseline = CacheKey::new("sha256:same".to_string(), 1, "c1");
        let mut entries = FxHashMap::default();
        entries.insert(baseline, ());

        assert!(!entries.contains_key(&CacheKey::new("sha256:same".to_string(), 2, "c1")));
        assert!(!entries.contains_key(&CacheKey::new("sha256:same".to_string(), 1, "c2")));
    }

    #[test]
    fn poisoned_std_lock_recovers_for_reads_and_writes() {
        let cache = Arc::new(InstrumentArtifactCache::default());
        let poisoner = Arc::clone(&cache);
        let result = thread::spawn(move || {
            let _guard = poisoner
                .state
                .write()
                .expect("lock starts healthy before deliberate poison");
            panic!("deliberately poison cache lock");
        })
        .join();
        assert!(result.is_err());

        assert!(cache.is_empty());
        super::write_state(&cache.state).decode_count = 7;
        assert_eq!(cache.decode_count(), 7);
    }
}
