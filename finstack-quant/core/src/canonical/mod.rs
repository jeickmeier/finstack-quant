//! Deterministic canonical JSON bytes and versioned content hashes.
//!
//! Canonical JSON objects are sorted recursively by key in UTF-8 code-unit
//! order and encoded without insignificant whitespace. Array order is preserved:
//! positions, scenario operations, statement periods, curve knots, and other
//! arrays remain in producer-defined semantic order. Integers retain their JSON
//! integer form and finite floating-point values use `serde_json`/Ryu's shortest
//! representation. Non-finite floating-point values are rejected rather than
//! silently encoded as `null`.
//!
//! Strings are otherwise untouched. Dates use their producer's ISO string
//! representation, decimals use their producer's string representation, enums
//! follow their serde attributes, and skipped `None` fields remain absent.
//! Producers own any semantic normalization required before canonicalization.
//! Unknown or extension `meta` maps participate in the digest because they are
//! part of the persisted artifact.

mod digest;
mod value_serializer;

pub use digest::{CANONICAL_VERSION, content_hash, to_canonical_bytes};
