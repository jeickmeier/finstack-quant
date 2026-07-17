//! `deny_unknown_fields` enforcement for structs that use `#[serde(flatten)]`.
//!
//! serde's native `#[serde(deny_unknown_fields)]` is documented as incompatible
//! with `#[serde(flatten)]`.
//! [`UnknownFieldGuard`](crate::serde_guard::UnknownFieldGuard) restores strictness without
//! changing the flat wire format: when used as the final flattened field, it
//! sees only keys not consumed by preceding fields and rejects the first one.
//! The zero-sized type serializes to no keys.

use core::fmt;

use serde::de::{Deserializer, Error as DeError, MapAccess, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

/// Zero-sized flatten field that rejects any unrecognized key during
/// deserialization.
///
/// Add this as the final `#[serde(flatten)]` field and exclude it from generated
/// schemas with `#[schemars(skip)]`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnknownFieldGuard;

impl Serialize for UnknownFieldGuard {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_map(core::iter::empty::<((), ())>())
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

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("no unknown fields")
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
