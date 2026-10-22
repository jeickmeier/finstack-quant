//! Non-finite `f64` wire sentinels and the `NonFiniteFields` trait.

use serde::{Deserialize, Serialize};

/// Serde field adapter for an `f64` that may legitimately be non-finite.
///
/// `serde_json` writes `f64::INFINITY` and `f64::NAN` as JSON `null` and then
/// refuses to read `null` back as an `f64`, so a field documented to reach
/// `+∞` silently loses its value on a `to_json` / `from_json` round trip.
/// Ratio metrics hit this whenever a denominator is zero — a profit factor
/// with wins and no losses is `+∞`, not missing data.
///
/// This adapter encodes non-finite values as the strings `"inf"`, `"-inf"`,
/// and `"nan"`, leaving finite values as ordinary JSON numbers.
///
/// # Examples
/// ```rust
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize, PartialEq, Debug)]
/// struct Ratios {
///     #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
///     profit_factor: f64,
/// }
///
/// let json = serde_json::to_string(&Ratios { profit_factor: f64::INFINITY })?;
/// assert_eq!(json, r#"{"profit_factor":"inf"}"#);
/// assert_eq!(serde_json::from_str::<Ratios>(&json)?.profit_factor, f64::INFINITY);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub mod non_finite_f64 {
    use serde::{Deserialize, Serialize};

    /// Wire form: a number when finite, a sentinel string otherwise.
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wire {
        Number(f64),
        Sentinel(String),
    }

    /// Serialize an `f64`, encoding non-finite values as sentinel strings.
    ///
    /// # Arguments
    ///
    /// * `value` - Value to encode; `±∞` and `NaN` become strings.
    /// * `serializer` - Serde serializer receiving the number or sentinel.
    ///
    /// # Errors
    ///
    /// Propagates any failure from the underlying serializer.
    pub fn serialize<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        super::NonFiniteF64Wire::from(*value).serialize(serializer)
    }

    /// Deserialize an `f64` that may arrive as a sentinel string.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying a number or sentinel.
    ///
    /// # Errors
    ///
    /// Returns an error when the value is `null`, or is neither a number nor a
    /// recognized sentinel (`"inf"`, `"-inf"`, `"nan"`).
    pub fn deserialize<'de, D>(deserializer: D) -> Result<f64, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match Wire::deserialize(deserializer)? {
            Wire::Number(value) => Ok(value),
            Wire::Sentinel(text) => parse_sentinel(&text).ok_or_else(|| {
                serde::de::Error::custom(format!(
                    "expected a number or one of \"inf\", \"-inf\", \"nan\"; got {:?}",
                    text.trim().to_ascii_lowercase()
                ))
            }),
        }
    }

    /// Decode one sentinel string written by [`serialize`].
    ///
    /// Hosts whose number type represents `±∞` and `NaN` natively (JavaScript)
    /// use this to turn a serialized sentinel back into a number, so the
    /// sentinel vocabulary has a single owner.
    ///
    /// # Arguments
    ///
    /// * `text` - Candidate sentinel. Matching ignores surrounding whitespace
    ///   and ASCII case; `"inf"` maps to `+∞`, `"-inf"` to `-∞`, and `"nan"`
    ///   to `NaN`. Other spellings are rejected.
    ///
    /// # Returns
    ///
    /// The decoded value, or `None` when `text` is not a recognized sentinel.
    #[must_use]
    pub fn parse_sentinel(text: &str) -> Option<f64> {
        match text.trim().to_ascii_lowercase().as_str() {
            "inf" => Some(f64::INFINITY),
            "-inf" => Some(f64::NEG_INFINITY),
            "nan" => Some(f64::NAN),
            _ => None,
        }
    }
}

/// An `f64` in the [`non_finite_f64`] wire form: a JSON number when finite,
/// otherwise one of the strings `"inf"`, `"-inf"` or `"nan"`.
///
/// Contract types store plain `f64` fields with
/// `#[serde(with = "finstack_quant_core::wire::non_finite_f64")]` and name this
/// type in `#[schemars(with = ...)]`, so the generated schema describes the
/// sentinel strings the serializer actually writes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum NonFiniteF64Wire {
    /// A finite value, written as a JSON number.
    Finite(f64),
    /// A non-finite value, written as its sentinel string.
    NonFinite(NonFiniteSentinel),
}

/// Sentinel string the [`non_finite_f64`] adapter writes for a non-finite `f64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum NonFiniteSentinel {
    /// `+∞`, written as `"inf"`.
    Inf,
    /// `-∞`, written as `"-inf"`.
    #[serde(rename = "-inf")]
    NegInf,
    /// `NaN`, written as `"nan"`.
    Nan,
}

impl From<f64> for NonFiniteF64Wire {
    fn from(value: f64) -> Self {
        if value.is_nan() {
            Self::NonFinite(NonFiniteSentinel::Nan)
        } else if value == f64::INFINITY {
            Self::NonFinite(NonFiniteSentinel::Inf)
        } else if value == f64::NEG_INFINITY {
            Self::NonFinite(NonFiniteSentinel::NegInf)
        } else {
            Self::Finite(value)
        }
    }
}

/// Serde adapter for a `Vec<f64>` whose elements may be `±∞` or `NaN`.
///
/// Each element uses the [`non_finite_f64`] wire form (a number when finite, a
/// sentinel string otherwise), so a series with undefined points survives a
/// JSON round trip instead of turning into `null`s that cannot be read back.
///
/// # Examples
///
/// ```rust
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct Series {
///     #[serde(with = "finstack_quant_core::wire::non_finite_f64_seq")]
///     values: Vec<f64>,
/// }
///
/// let json = serde_json::to_string(&Series { values: vec![1.5, f64::NAN] })?;
/// assert_eq!(json, r#"{"values":[1.5,"nan"]}"#);
/// let back: Series = serde_json::from_str(&json)?;
/// assert!(back.values[1].is_nan());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub mod non_finite_f64_seq {
    use serde::{Deserialize, Serialize};

    /// One element in the [`super::non_finite_f64`] wire form.
    #[derive(Serialize, Deserialize)]
    #[serde(transparent)]
    struct Element(#[serde(with = "super::non_finite_f64")] f64);

    /// Serialize a slice of `f64`, encoding non-finite elements as sentinels.
    ///
    /// # Arguments
    ///
    /// * `values` - Elements to encode in order; `±∞` and `NaN` become strings.
    /// * `serializer` - Serde serializer receiving the sequence.
    ///
    /// # Errors
    ///
    /// Propagates any failure from the underlying serializer.
    pub fn serialize<S>(values: &[f64], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_seq(values.iter().map(|value| Element(*value)))
    }

    /// Deserialize a sequence whose elements may be sentinel strings.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde deserializer supplying the sequence.
    ///
    /// # Errors
    ///
    /// Returns an error when an element is `null` or is neither a number nor a
    /// recognized sentinel (see [`super::non_finite_f64::parse_sentinel`]).
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<f64>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Vec::<Element>::deserialize(deserializer)?
            .into_iter()
            .map(|Element(value)| value)
            .collect())
    }
}

/// Names the top-level fields of a result type that serialize through
/// [`non_finite_f64`].
///
/// Those fields reach JSON as sentinel strings when they are `±∞` or `NaN`.
/// Hosts with a native non-finite number type (JavaScript) read this list to
/// restore the numbers after serialization, instead of keeping their own copy
/// of the field names. Every implementation carries a unit test that fills
/// each `f64` field with `NaN` and checks that exactly these keys come out as
/// sentinel strings, so the list cannot drift from the serde attributes.
pub trait NonFiniteFields {
    /// Serialized names of the fields annotated with
    /// `#[serde(with = "finstack_quant_core::wire::non_finite_f64")]`.
    const NON_FINITE_FIELDS: &'static [&'static str];
}
