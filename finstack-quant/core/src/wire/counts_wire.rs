//! Count adapters: `count`, `counts` and `signed_count`.

/// Serde adapter for a count that hosts must receive as an ordinary number.
///
/// Result payloads that embed a Monte Carlo `ValuationResult` are serialized
/// for JavaScript with 64-bit integers mapped to `BigInt`, because MC seeds
/// span the full `u64` range. A `usize` count next to such a result would
/// otherwise also arrive as a `BigInt`. This adapter writes the count as a
/// `u32`, which every serializer emits as a plain number; the JSON text is
/// unchanged. Deserialization accepts any unsigned integer that fits `usize`.
///
/// # Examples
///
/// ```rust
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct Summary {
///     #[serde(with = "finstack_quant_core::wire::count")]
///     num_steps: usize,
/// }
///
/// let json = serde_json::to_string(&Summary { num_steps: 3 })?;
/// assert_eq!(json, r#"{"num_steps":3}"#);
/// assert_eq!(serde_json::from_str::<Summary>(&json)?.num_steps, 3);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub mod count {
    use serde::Deserialize;

    /// Serialize a `usize` count as a `u32`.
    ///
    /// # Arguments
    ///
    /// * `value` - Non-negative count to encode; must not exceed `u32::MAX`.
    /// * `serializer` - Serde serializer receiving the 32-bit count.
    ///
    /// # Errors
    ///
    /// Returns a serializer error when `value` exceeds `u32::MAX`.
    pub fn serialize<S>(value: &usize, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = u32::try_from(*value).map_err(serde::ser::Error::custom)?;
        serializer.serialize_u32(value)
    }

    /// Deserialize a `usize` count.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying a non-negative integer.
    ///
    /// # Errors
    ///
    /// Returns a deserializer error when the value is not a non-negative
    /// integer that fits `usize`.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<usize, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        usize::deserialize(deserializer)
    }
}

/// Serde adapter for a list of counts or indices that hosts must receive as
/// ordinary numbers.
///
/// The list form of [`count`]: each element is written as a `u32` so that a
/// BigInt-preserving JavaScript serializer still emits plain numbers.
pub mod counts {
    use serde::ser::SerializeSeq;
    use serde::Deserialize;

    /// Serialize a list of `usize` values as `u32` elements.
    ///
    /// # Arguments
    ///
    /// * `values` - Non-negative counts or indices; each must not exceed `u32::MAX`.
    /// * `serializer` - Serde serializer receiving the sequence.
    ///
    /// # Errors
    ///
    /// Returns a serializer error when any element exceeds `u32::MAX`.
    pub fn serialize<S>(values: &[usize], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(values.len()))?;
        for value in values {
            let value = u32::try_from(*value).map_err(serde::ser::Error::custom)?;
            seq.serialize_element(&value)?;
        }
        seq.end()
    }

    /// Deserialize a list of `usize` values.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying a sequence of
    ///   non-negative integers.
    ///
    /// # Errors
    ///
    /// Returns a deserializer error when the value is not a sequence of
    /// non-negative integers that fit `usize`.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<usize>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Vec::<usize>::deserialize(deserializer)
    }
}

/// Serde adapter for a signed count (for example a day span) that hosts must
/// receive as an ordinary number.
///
/// The signed form of [`count`]: the value is written as an `i32` so that a
/// BigInt-preserving JavaScript serializer still emits a plain number.
pub mod signed_count {
    use serde::Deserialize;

    /// Serialize an `i64` count as an `i32`.
    ///
    /// # Arguments
    ///
    /// * `value` - Signed count to encode; must lie within the `i32` range.
    /// * `serializer` - Serde serializer receiving the 32-bit count.
    ///
    /// # Errors
    ///
    /// Returns a serializer error when `value` lies outside the `i32` range.
    pub fn serialize<S>(value: &i64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = i32::try_from(*value).map_err(serde::ser::Error::custom)?;
        serializer.serialize_i32(value)
    }

    /// Deserialize an `i64` count.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying an integer.
    ///
    /// # Errors
    ///
    /// Returns a deserializer error when the value is not an integer that
    /// fits `i64`.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<i64, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        i64::deserialize(deserializer)
    }
}
