//! `deny_unknown_fields` enforcement for instrument structs that use `#[serde(flatten)]`.
//!
//! serde's native `#[serde(deny_unknown_fields)]` is documented as incompatible
//! with `#[serde(flatten)]` — neither on the struct that holds the flattened
//! field nor on the flattened struct itself (the `FlatMapDeserializer` path that
//! drives flattening has no notion of "unknown" fields, so the rejection is
//! silently skipped). This leaves flatten-based instruments — notably the
//! commodity family, which all flatten a shared `CommodityUnderlyingParams` — in
//! violation of the workspace invariant that *unknown fields are denied on
//! inbound types*.
//!
//! [`UnknownFieldGuard`] restores that invariant without changing the flat wire
//! format. It is added as a trailing `#[serde(flatten)]` field: by the time it is
//! deserialized, every field claimed by the outer struct and by the preceding
//! flattened struct has been consumed, so the guard sees exactly the leftover
//! (i.e. unrecognized) keys. Its `Deserialize` impl errors on the first such key.
//! It is a zero-sized type, serializes to nothing, and is excluded from generated
//! JSON schemas via `#[schemars(skip)]`, so neither the wire format nor the
//! committed schemas change.

use std::fmt;

use serde::de::{Deserializer, Error as DeError, MapAccess, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

/// Zero-sized flatten field that rejects any unrecognized key during
/// deserialization, restoring `deny_unknown_fields` semantics for structs that
/// use `#[serde(flatten)]`.
///
/// See the [module documentation](self) for why this is needed and how it works.
///
/// # Usage
///
/// Add as the final field of a flatten-based instrument struct:
///
/// ```ignore
/// #[serde(flatten)]
/// #[schemars(skip)]
/// #[builder(default)]
/// unknown_fields: UnknownFieldGuard,
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnknownFieldGuard;

impl Serialize for UnknownFieldGuard {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Flattening requires a map-like value; the guard contributes no keys.
        serializer.collect_map(std::iter::empty::<((), ())>())
    }
}

impl<'de> Deserialize<'de> for UnknownFieldGuard {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct GuardVisitor;

        impl<'de> Visitor<'de> for GuardVisitor {
            type Value = UnknownFieldGuard;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("no unknown fields")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                if let Some(key) = map.next_key::<String>()? {
                    return Err(A::Error::custom(format!("unknown field `{key}`")));
                }
                Ok(UnknownFieldGuard)
            }
        }

        deserializer.deserialize_map(GuardVisitor)
    }
}
