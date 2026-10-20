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

mod value_serializer;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::InputError;

use value_serializer::CanonicalValueError;

/// Canonicalization algorithm version mixed into content-hash preimages.
pub const CANONICAL_VERSION: &str = "c1";

const HASH_DOMAIN: &[u8] = b"finstack-canon/";
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

/// Serialize a value to deterministic, compact, key-sorted JSON bytes.
///
/// The value is serialized exactly once into a checked JSON tree. Every
/// floating-point value emitted during that pass must be finite.
///
/// # Arguments
///
/// * `value` - Serializable value whose JSON object keys will be sorted
///   recursively while array order and scalar representations remain unchanged.
///
/// # Errors
///
/// Returns [`crate::Error::Input`] when a serialized `f32` or `f64` is not
/// finite, or [`crate::Error::Internal`] when the value's serde implementation
/// cannot produce JSON.
pub fn to_canonical_bytes<T: Serialize>(value: &T) -> crate::Result<Vec<u8>> {
    let json_value = value_serializer::to_value(value).map_err(canonical_value_error)?;
    canonical_bytes_of_value(&json_value)
}

/// Serialize an existing JSON value to deterministic, compact, key-sorted bytes.
///
/// # Arguments
///
/// * `value` - JSON tree whose object keys will be sorted recursively while
///   array order and scalar representations remain unchanged.
///
/// # Errors
///
/// Returns [`crate::Error::Internal`] if the canonical JSON tree cannot be
/// encoded.
pub fn canonical_bytes_of_value(value: &Value) -> crate::Result<Vec<u8>> {
    // `serde_json::Map` is a `BTreeMap` (the `preserve_order` feature is off),
    // so object keys are already byte-ordered; see `map_keys_iterate_sorted`.
    serde_json::to_vec(value)
        .map_err(|error| crate::Error::internal(format!("canonical JSON encoding failed: {error}")))
}

/// Compute a versioned SHA-256 digest over a value's canonical JSON bytes.
///
/// The preimage is `b"finstack-canon/" || CANONICAL_VERSION || b"\0" || bytes`.
/// The returned string embeds the digest algorithm as `"sha256:<64-hex>"`;
/// [`CANONICAL_VERSION`] remains a separate version axis for manifests and
/// cache keys.
///
/// # Arguments
///
/// * `value` - Serializable value to canonicalize and hash, including any
///   extension metadata present in its JSON representation.
///
/// # Errors
///
/// Returns the same serialization and non-finite-number errors as
/// [`to_canonical_bytes`].
pub fn content_hash<T: Serialize>(value: &T) -> crate::Result<String> {
    let canonical = to_canonical_bytes(value)?;
    let mut hasher = Sha256::new();
    hasher.update(HASH_DOMAIN);
    hasher.update(CANONICAL_VERSION.as_bytes());
    hasher.update(b"\0");
    hasher.update(canonical);
    let digest = hasher.finalize();

    let mut encoded = String::with_capacity("sha256:".len() + digest.len() * 2);
    encoded.push_str("sha256:");
    for byte in digest {
        encoded.push(HEX_DIGITS[usize::from(byte >> 4)] as char);
        encoded.push(HEX_DIGITS[usize::from(byte & 0x0f)] as char);
    }
    Ok(encoded)
}

fn canonical_value_error(error: CanonicalValueError) -> crate::Error {
    match error {
        CanonicalValueError::NonFinite(kind) => {
            crate::Error::from(InputError::NonFiniteValue { kind })
        }
        CanonicalValueError::Custom(message) => {
            crate::Error::internal(format!("canonical JSON serialization failed: {message}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `canonical_bytes_of_value` relies on `serde_json::Map` being ordered
    /// (the `preserve_order` feature must stay off). Guard that assumption.
    #[test]
    fn map_keys_iterate_sorted() {
        let mut map = serde_json::Map::new();
        for key in ["zeta", "alpha", "mid", "Beta"] {
            map.insert(key.to_string(), Value::Null);
        }
        let keys: Vec<&str> = map.keys().map(String::as_str).collect();
        assert_eq!(keys, ["Beta", "alpha", "mid", "zeta"]);

        let bytes = canonical_bytes_of_value(&Value::Object(map)).expect("encodes");
        assert_eq!(
            String::from_utf8(bytes).expect("utf8"),
            r#"{"Beta":null,"alpha":null,"mid":null,"zeta":null}"#
        );
    }
}
