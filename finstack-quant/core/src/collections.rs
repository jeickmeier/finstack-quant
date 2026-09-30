//! Re-exports for hash-based collections with deterministic, fast hashing.
//!
//! This module provides standardized type aliases for hash maps and sets using
//! [`FxHasher`](rustc_hash::FxHasher) from `rustc-hash`. FxHash is a fast,
//! non-cryptographic hasher optimized for integer keys and small values—ideal
//! for internal caches, DAG node lookups, and curve/surface registries.
//!
//! # Why FxHash?
//!
//! - **Determinism (one build target only)**: No random seed initialization,
//!   so iteration order is reproducible for a given insertion sequence *on one
//!   build target*. It differs between 64-bit native and wasm32 builds, because
//!   the FxHash multiplier and mixing depend on pointer width. Never let
//!   `HashMap`/`HashSet` iteration order reach results, floating-point
//!   reductions, error selection or emitted `Vec` order; iterate a
//!   `BTreeMap`/`IndexMap` or sort first.
//! - **Speed**: ~2× faster than the default `RandomState` hasher for integer keys.
//! - **Simplicity**: One hash implementation across the crate.
//!
//! # When to use `BTreeMap` or `IndexMap` instead
//!
//! Use `std::collections::BTreeMap` when you need **stable, sorted iteration**
//! (e.g., serializing snapshots for golden tests or deterministic JSON output),
//! and `indexmap::IndexMap` when iteration must follow insertion order. Both
//! iterate identically on every target.
//!
//! # Example
//!
//! ```
//! use finstack_quant_core::{HashMap, HashSet};
//!
//! let mut prices: HashMap<&str, f64> = HashMap::default();
//! prices.insert("AAPL", 150.0);
//! prices.insert("GOOG", 2800.0);
//!
//! let mut seen: HashSet<u64> = HashSet::default();
//! seen.insert(42);
//! ```

pub use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
