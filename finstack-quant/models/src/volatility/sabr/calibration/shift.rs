//! Displacement policies and standardized shift ladder for shifted SABR calibration.

#[cfg(doc)]
use super::super::parameters::SabrParameters;
use finstack_quant_core::{Error, Result};

/// Standardized shift ladder for shifted-SABR auto-shift selection.
///
/// Market practice quotes shifted-Black smiles against a small set of
/// standardized shifts (e.g. 1% for EUR/CHF swaptions, 2%/3% for deeply
/// negative short rates) rather than an ad-hoc data-dependent value, so the
/// same surface re-calibrated on a slightly different day does not silently
/// change convention. [`SabrShift::Auto`] rounds the required minimum
/// shift (`−min_rate + 10bp` headroom) *up* to the next rung. Callers that
/// need an exact per-currency convention should pass an explicit shift to
/// [`SabrShift::Fixed`].
const STANDARD_SHIFTS: [f64; 5] = [0.005, 0.01, 0.02, 0.03, 0.04];

/// Round the minimum required shift up to the standardized ladder.
///
/// Errors if rates are so negative that even the largest standardized shift
/// (4%) cannot make all shifted rates positive.
pub(super) fn standard_shift(min_rate: f64) -> Result<f64> {
    let required = (-min_rate + 0.001).max(0.001); // at least 10bp headroom
    STANDARD_SHIFTS
        .iter()
        .copied()
        .find(|&s| s >= required)
        .ok_or_else(|| {
            Error::Validation(format!(
                "SABR auto-shift: minimum rate {min_rate:.6} requires a shift larger than the \
                 maximum standardized shift of 4%; pass an explicit shift via SabrShift::Fixed"
            ))
        })
}

/// Displacement applied to the forward and strikes before a SABR fit.
///
/// Shifted SABR models `F + s` instead of `F`, which keeps the CEV backbone
/// well-defined when the forward or a strike is negative (EUR/CHF/JPY rates).
/// The chosen shift is stored on the calibrated [`SabrParameters`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SabrShift {
    /// No displacement. Inputs are fitted as quoted; negative forwards or
    /// strikes are only meaningful for normal SABR (β = 0).
    None,
    /// Fixed additive displacement in the units of the forward (decimal rate
    /// or price). The shifted forward and strikes must be strictly positive.
    Fixed(f64),
    /// Use the smallest standardized shift (1%, 2%, 3%, 4%) that leaves at
    /// least 10bp of headroom above the most negative forward or strike, or
    /// no shift when every input is non-negative.
    Auto,
}

impl SabrShift {
    /// Host keyword selecting [`SabrShift::Auto`].
    pub const AUTO_KEYWORD: &'static str = "auto";
}

/// Parse the keyword form of a shift policy.
///
/// Only [`SabrShift::AUTO_KEYWORD`] (`"auto"`, case-sensitive) is a keyword.
/// The full host union (`null`, a number, or `"auto"`) is the serde form; see
/// the `Deserialize` impl.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_models::volatility::sabr::SabrShift;
///
/// assert_eq!("auto".parse::<SabrShift>()?, SabrShift::Auto);
/// assert!("AUTO".parse::<SabrShift>().is_err());
/// assert_eq!(SabrShift::Auto.to_string(), "auto");
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
impl std::str::FromStr for SabrShift {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        if s == Self::AUTO_KEYWORD {
            Ok(Self::Auto)
        } else {
            Err(Error::Validation(format!(
                "SABR shift must be null/None, a number, or {:?}; got {s:?}",
                Self::AUTO_KEYWORD
            )))
        }
    }
}

/// `none`, the fixed shift value, or `auto`.
impl std::fmt::Display for SabrShift {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => f.write_str("none"),
            Self::Fixed(shift) => write!(f, "{shift}"),
            Self::Auto => f.write_str(Self::AUTO_KEYWORD),
        }
    }
}

/// Wire form: `null` for [`SabrShift::None`], the number for
/// [`SabrShift::Fixed`], and `"auto"` for [`SabrShift::Auto`] — the union the
/// Python and WASM bindings accept and return.
impl serde::Serialize for SabrShift {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Self::None => serializer.serialize_unit(),
            Self::Fixed(shift) => serializer.serialize_f64(*shift),
            Self::Auto => serializer.serialize_str(Self::AUTO_KEYWORD),
        }
    }
}

/// Reads the wire form written by `Serialize`: `null`, any number, or the
/// `"auto"` keyword (through [`std::str::FromStr`]); booleans and every
/// other shape are rejected.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_models::volatility::sabr::SabrShift;
///
/// let parse = |json: &str| serde_json::from_str::<SabrShift>(json);
/// assert_eq!(parse("null")?, SabrShift::None);
/// assert_eq!(parse("0.02")?, SabrShift::Fixed(0.02));
/// assert_eq!(parse("\"auto\"")?, SabrShift::Auto);
/// assert!(parse("true").is_err());
/// assert_eq!(serde_json::to_string(&SabrShift::Auto)?, "\"auto\"");
/// # Ok::<(), serde_json::Error>(())
/// ```
impl<'de> serde::Deserialize<'de> for SabrShift {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct ShiftVisitor;

        impl serde::de::Visitor<'_> for ShiftVisitor {
            type Value = SabrShift;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("null, a number, or \"auto\"")
            }

            fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<SabrShift, E> {
                Ok(SabrShift::None)
            }

            fn visit_none<E: serde::de::Error>(self) -> std::result::Result<SabrShift, E> {
                Ok(SabrShift::None)
            }

            fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<SabrShift, E> {
                Ok(SabrShift::Fixed(v))
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<SabrShift, E> {
                Ok(SabrShift::Fixed(v as f64))
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<SabrShift, E> {
                Ok(SabrShift::Fixed(v as f64))
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<SabrShift, E> {
                v.parse().map_err(E::custom)
            }

            fn visit_bool<E: serde::de::Error>(
                self,
                _v: bool,
            ) -> std::result::Result<SabrShift, E> {
                Err(E::custom(format!(
                    "SABR shift must be null/None, a number, or {:?}; got bool",
                    SabrShift::AUTO_KEYWORD
                )))
            }
        }

        deserializer.deserialize_any(ShiftVisitor)
    }
}
