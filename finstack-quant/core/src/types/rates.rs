//! Strongly typed financial rate units with explicit conversion semantics.
//!
//! - [`Rate`] stores an `f64` decimal: `0.05` is 5% or 500 bp.
//! - [`Bps`] stores an integer basis-point quote: 1 bp is 0.01% or 0.0001
//!   decimal, and 10,000 bp is 100%.
//!
//! `Rate` serializes as its decimal and rejects non-finite values when
//! deserialized. `Bps` serializes as its integer basis-point quote. The
//! floating-point `Bps` conversion rejects invalid values with validation
//! errors. Arithmetic uses fallible `checked_*` methods; only `Rate` negation
//! is infallible. Percent quotes use [`Rate::from_percent`] and [`Rate::as_percent`].
//!
//! # Example
//!
//! ```rust
//! use finstack_quant_core::types::{Bps, Rate};
//!
//! let rate = Rate::from_decimal(0.05).expect("valid rate fixture");
//! assert_eq!(rate, Rate::from_percent(5.0).expect("valid rate fixture"));
//! assert_eq!(rate, Rate::from_bp(500));
//! assert_eq!(rate.as_percent(), 5.0);
//! assert_eq!(rate.as_bp(), Bps::new(500).as_bp());
//! ```

use std::fmt;
use std::ops::Neg;

use serde::{Deserialize, Serialize};

use crate::math::units::BASIS_POINTS_PER_UNIT;
use crate::Result;
use crate::{Error, InputError, NonFiniteKind};

/// Scale `value` by a power of ten in decimal arithmetic.
///
/// Parse the shortest round-trip decimal form, move its decimal point by the
/// digits of `factor` (a power of ten), and convert back to `f64`. Division
/// shifts the decimal scale exactly; a result that would need more than
/// Decimal's 28 fractional digits, or a value outside Decimal's range, uses the
/// correctly rounded `f64` product or quotient instead, so tiny nonzero inputs
/// keep their magnitude. Zero keeps its sign.
fn scale_decimal(value: f64, factor: u32, divide: bool) -> f64 {
    use rust_decimal::prelude::ToPrimitive;
    let float_result = if divide {
        value / f64::from(factor)
    } else {
        value * f64::from(factor)
    };
    if value == 0.0 {
        return float_result;
    }
    let digits = factor.ilog10();
    rust_decimal::Decimal::from_str_exact(&value.to_string())
        .ok()
        .and_then(|mut decimal| {
            if divide {
                decimal.set_scale(decimal.scale() + digits).ok()?;
                Some(decimal)
            } else if decimal.scale() >= digits {
                decimal.set_scale(decimal.scale() - digits).ok()?;
                Some(decimal)
            } else {
                decimal.checked_mul(rust_decimal::Decimal::from(factor))
            }
        })
        .and_then(|scaled| scaled.to_f64())
        .unwrap_or(float_result)
}

fn ensure_finite(value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(InputError::NonFiniteValue {
            kind: NonFiniteKind::classify(value),
        }
        .into())
    }
}

/// A financial rate stored as an `f64` decimal.
///
/// For example, `0.05` represents 5% or 500 bp.
///
/// # Serde
///
/// Serialized transparently as the inner `f64` decimal. Deserialization
/// routes through [`Rate::from_decimal`], so non-finite values (NaN,
/// ±Infinity) are rejected even for binary formats that can encode them
/// (JSON already rejects them at the parser level).
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize)]
pub struct Rate(f64);

impl<'de> Deserialize<'de> for Rate {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let decimal = f64::deserialize(deserializer)?;
        Rate::from_decimal(decimal).map_err(serde::de::Error::custom)
    }
}

impl Rate {
    /// Create a rate from a decimal value, rejecting non-finite inputs.
    ///
    /// Returns an error if `decimal` is NaN or infinite, preventing silent
    /// corruption of downstream financial calculations.
    ///
    /// # Errors
    ///
    /// Returns `Error::Input(InputError::NonFiniteValue { .. })` when `decimal`
    /// is NaN or infinite.
    ///
    /// # Arguments
    ///
    /// * `decimal` - Rate represented as a decimal, where one percent is `0.01`.
    pub fn from_decimal(decimal: f64) -> Result<Self> {
        ensure_finite(decimal).map(Self)
    }

    /// Create a rate from a percentage value, rejecting non-finite inputs.
    ///
    /// The percent's shortest round-trip decimal form is moved two places, so
    /// `1.1` percent is the decimal whose fractional basis-point value is
    /// `110.0`, rather than the binary quotient `1.1 / 100.0`.
    ///
    /// # Errors
    ///
    /// Returns `Error::Input(InputError::NonFiniteValue { .. })` when
    /// `percent` is NaN or infinite, or when the scaled decimal is non-finite.
    ///
    /// # Arguments
    ///
    /// * `percent` - Rate represented in percentage points, where one percent is `1.0`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::types::Rate;
    ///
    /// assert_eq!(Rate::from_percent(1.1)?.as_bp_f64(), 110.0);
    /// assert_eq!(Rate::from_percent(0.07)?.as_bp_f64(), 7.0);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn from_percent(percent: f64) -> Result<Self> {
        ensure_finite(percent)?;
        Self::from_decimal(scale_decimal(percent, 100, true))
    }

    /// Create a rate from an integer basis-point quote (500 bp = 5%).
    ///
    /// Negative values are accepted because spreads and rates may be negative.
    ///
    /// # Arguments
    ///
    /// * `bp` - Integer basis-point quote, where `500` is 5% (`0.05` decimal).
    ///   Negative quotes are valid because spreads and rates may be negative.
    pub fn from_bp(bp: i32) -> Self {
        Self(bp as f64 / BASIS_POINTS_PER_UNIT)
    }

    /// Return the internal decimal representation (0.05 for 5%).
    #[must_use]
    pub fn as_decimal(self) -> f64 {
        self.0
    }

    /// Return the percentage representation (5.0 for 5%).
    ///
    /// The stored decimal's shortest round-trip form is moved two places, so
    /// a rate built with [`Self::from_percent`] reports that percent back
    /// instead of a binary product such as `1.1000000000000001`.
    #[must_use]
    pub fn as_percent(self) -> f64 {
        scale_decimal(self.0, 100, false)
    }

    /// Get the rate as basis points (rounded to nearest integer).
    ///
    /// # Overflow
    ///
    /// The rounded value is then saturated to the `i32` range by the float-to-int
    /// cast. For rates beyond about `±2_147_483_647` bp
    /// (`±21_474_836.47%`), prefer `(self.as_decimal() * 10_000.0).round() as i64`.
    #[must_use]
    pub fn as_bp(self) -> i32 {
        (self.0 * BASIS_POINTS_PER_UNIT).round() as i32
    }

    /// Get the rate as fractional basis points, without rounding.
    ///
    /// The decimal point of the rate's shortest round-trip decimal form is
    /// moved four places, so a rate typed as `0.0029` reports `29.0` bp rather
    /// than the binary-noise product `0.0029 * 10_000.0 = 28.999999999999996`,
    /// and a sub-bp rate such as `0.00625` keeps its `62.5` bp. Use
    /// [`Self::as_bp`] for the rounded integer quote.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::types::Rate;
    ///
    /// assert_eq!(Rate::from_decimal(0.0029)?.as_bp_f64(), 29.0);
    /// assert_eq!(Rate::from_decimal(0.00625)?.as_bp_f64(), 62.5);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    #[must_use]
    pub fn as_bp_f64(self) -> f64 {
        scale_decimal(self.0, 10_000, false)
    }

    /// The zero rate, represented as decimal value 0.0.
    pub const ZERO: Self = Self(0.0);

    /// Return true when the decimal rate is exactly zero.
    pub fn is_zero(self) -> bool {
        self.0 == 0.0
    }

    /// Return true when the decimal rate is strictly greater than zero.
    pub fn is_positive(self) -> bool {
        self.0 > 0.0
    }

    /// Return true when the decimal rate is strictly less than zero.
    pub fn is_negative(self) -> bool {
        self.0 < 0.0
    }

    /// Return the rate's absolute value without changing its representation.
    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }

    /// Add two decimal rates and reject a non-finite result.
    ///
    /// # Errors
    ///
    /// Returns NonFiniteValue if the sum overflows to infinity or is otherwise
    /// non-finite.
    pub fn checked_add(self, rhs: Self) -> Result<Self> {
        Self::from_decimal(self.0 + rhs.0)
    }

    /// Subtract two decimal rates and reject a non-finite result.
    ///
    /// # Errors
    ///
    /// Returns NonFiniteValue if the difference is non-finite.
    pub fn checked_sub(self, rhs: Self) -> Result<Self> {
        Self::from_decimal(self.0 - rhs.0)
    }

    /// Scale the decimal rate by rhs and reject a non-finite result.
    ///
    /// # Errors
    ///
    /// Returns NonFiniteValue if rhs or the product is non-finite.
    pub fn checked_mul(self, rhs: f64) -> Result<Self> {
        Self::from_decimal(self.0 * rhs)
    }

    /// Divide the decimal rate by rhs.
    ///
    /// # Errors
    ///
    /// Returns Invalid for a zero divisor or NonFiniteValue for a non-finite
    /// quotient.
    pub fn checked_div(self, rhs: f64) -> Result<Self> {
        if rhs == 0.0 {
            return Err(crate::Error::Validation(
                "cannot divide a rate by zero".to_string(),
            ));
        }
        Self::from_decimal(self.0 / rhs)
    }
}

impl fmt::Display for Rate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.4}%", self.as_percent())
    }
}

impl std::str::FromStr for Rate {
    type Err = Error;

    /// Parse a rate quote with an optional unit suffix.
    ///
    /// Accepted forms (surrounding whitespace and the space before the unit
    /// are ignored; units are case-insensitive):
    ///
    /// - `"0.05"` — plain decimal fraction (5%)
    /// - `"5%"` — percent
    /// - `"25bp"` / `"25bps"` — basis points (fractional bp such as `"62.5bp"`
    ///   are accepted, since `Rate` is decimal-backed)
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when the text is empty, the numeric part
    /// does not parse, or the value is non-finite.
    fn from_str(s: &str) -> Result<Self> {
        let trimmed = s.trim();
        let invalid = || {
            Error::Validation(format!("invalid rate {s:?}: expected a decimal (\"0.05\"), percent (\"5%\") or basis-point (\"25bp\") quote"))
        };
        if trimmed.is_empty() {
            return Err(invalid());
        }
        let lower = trimmed.to_ascii_lowercase();
        let (number, divisor) = if let Some(num) = lower.strip_suffix("bps") {
            (num, BASIS_POINTS_PER_UNIT)
        } else if let Some(num) = lower.strip_suffix("bp") {
            (num, BASIS_POINTS_PER_UNIT)
        } else if let Some(num) = lower.strip_suffix('%') {
            let percent: f64 = num.trim().parse().map_err(|_| invalid())?;
            return Self::from_percent(percent);
        } else {
            (lower.as_str(), 1.0)
        };
        let value: f64 = number.trim().parse().map_err(|_| invalid())?;
        Self::from_decimal(value / divisor)
    }
}

impl From<Rate> for f64 {
    fn from(rate: Rate) -> Self {
        rate.as_decimal()
    }
}

impl Neg for Rate {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

/// An integer basis-point quote.
///
/// - 1 bp = 0.01% = 0.0001 decimal
/// - 100 bp = 1%
/// - 10,000 bp = 100%
///
/// Values are stored and serialized as `i32` basis points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Bps(i32);

impl Bps {
    /// Create a basis-point value from an integer quote.
    pub fn new(bp: i32) -> Self {
        Self(bp)
    }

    /// Create a Bps value from a floating-point basis-point amount.
    ///
    /// Accepts a `f64` (e.g., `25.0` for 25 bp) and validates that it is a
    /// finite, **whole** number of basis points. `Bps` is integer-backed, so
    /// silently rounding a sub-bp spread (e.g. an FRN margin of 62.5 bp)
    /// would change instrument economics; fractional input is rejected.
    /// Use a decimal [`Rate`] when sub-bp precision matters.
    ///
    /// # Errors
    ///
    /// - `Error::Input(InputError::NonFiniteValue { .. })` when `bp` is NaN or infinite.
    /// - `Error::Input(InputError::FractionalBasisPoints { .. })` when `bp` is not a
    ///   whole number of basis points.
    /// - `Error::Input(InputError::ConversionOverflow)` when `bp` cannot be
    ///   represented as an `i32` basis-point quote.
    ///
    /// # Arguments
    ///
    /// * `bp` - Finite whole number of basis points (for example `25.0` for
    ///   25 bp). Fractional values are rejected because `Bps` is integer-backed.
    pub fn try_new(bp: f64) -> Result<Self> {
        ensure_finite(bp)?;
        if bp.fract() != 0.0 {
            return Err(InputError::FractionalBasisPoints { value: bp }.into());
        }
        if bp < i32::MIN as f64 || bp > i32::MAX as f64 {
            return Err(InputError::ConversionOverflow.into());
        }
        Ok(Self(bp as i32))
    }

    /// Return the integer basis-point quote.
    pub fn as_bp(self) -> i32 {
        self.0
    }

    /// Return the decimal-rate representation (100 bp = 0.01).
    pub fn as_decimal(self) -> f64 {
        self.0 as f64 / BASIS_POINTS_PER_UNIT
    }

    /// Return the percentage representation (100 bp = 1.0).
    pub fn as_percent(self) -> f64 {
        self.0 as f64 / 100.0
    }

    /// Convert the basis-point quote to a Rate decimal wrapper.
    pub fn as_rate(self) -> Rate {
        Rate::from_bp(self.0)
    }

    /// The zero basis-point quote.
    pub const ZERO: Self = Self(0);

    /// Return true when the quote is exactly zero basis points.
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Return true when the quote is strictly positive.
    pub fn is_positive(self) -> bool {
        self.0 > 0
    }

    /// Return true when the quote is strictly negative.
    pub fn is_negative(self) -> bool {
        self.0 < 0
    }

    /// Return the absolute basis-point quote, rejecting `i32::MIN`.
    ///
    /// # Errors
    ///
    /// Returns `InputError::ConversionOverflow` when the quote is `i32::MIN`,
    /// whose absolute value has no `i32` representation.
    pub fn checked_abs(self) -> Result<Self> {
        self.0
            .checked_abs()
            .map(Self)
            .ok_or_else(|| InputError::ConversionOverflow.into())
    }

    /// Add two basis-point values, rejecting integer overflow.
    ///
    /// # Errors
    ///
    /// Returns `InputError::ConversionOverflow` when the sum cannot be
    /// represented as an `i32` basis-point quote.
    pub fn checked_add(self, rhs: Self) -> Result<Self> {
        self.0
            .checked_add(rhs.0)
            .map(Self)
            .ok_or_else(|| InputError::ConversionOverflow.into())
    }

    /// Subtract two basis-point values, rejecting integer overflow.
    ///
    /// # Errors
    ///
    /// Returns `InputError::ConversionOverflow` when the difference cannot be
    /// represented as an `i32` basis-point quote.
    pub fn checked_sub(self, rhs: Self) -> Result<Self> {
        self.0
            .checked_sub(rhs.0)
            .map(Self)
            .ok_or_else(|| InputError::ConversionOverflow.into())
    }

    /// Multiply by an integer scalar, rejecting integer overflow.
    ///
    /// # Errors
    ///
    /// Returns `InputError::ConversionOverflow` when the product cannot be
    /// represented as an `i32` basis-point quote.
    pub fn checked_mul(self, rhs: i32) -> Result<Self> {
        self.0
            .checked_mul(rhs)
            .map(Self)
            .ok_or_else(|| InputError::ConversionOverflow.into())
    }

    /// Divide by an integer scalar, rejecting zero divisors.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` for a zero divisor, or
    /// `InputError::ConversionOverflow` for the unrepresentable
    /// `i32::MIN / -1` quotient.
    pub fn checked_div(self, rhs: i32) -> Result<Self> {
        if rhs == 0 {
            return Err(crate::Error::Validation(
                "cannot divide basis points by zero".to_string(),
            ));
        }
        self.0
            .checked_div(rhs)
            .map(Self)
            .ok_or_else(|| InputError::ConversionOverflow.into())
    }

    /// Negate the basis-point value, rejecting integer overflow.
    ///
    /// # Errors
    ///
    /// Returns `InputError::ConversionOverflow` when negating `i32::MIN`.
    pub fn checked_neg(self) -> Result<Self> {
        self.0
            .checked_neg()
            .map(Self)
            .ok_or_else(|| InputError::ConversionOverflow.into())
    }
}

impl fmt::Display for Bps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}bp", self.0)
    }
}

/// Whole basis points convert losslessly; this backs `impl Into<Bps>`
/// parameters such as floating-rate spreads.
impl From<i32> for Bps {
    fn from(bp: i32) -> Self {
        Self::new(bp)
    }
}

impl From<Bps> for i32 {
    fn from(bp: Bps) -> Self {
        bp.as_bp()
    }
}

// NOTE: There is intentionally no `impl From<Bps> for f64`.
//
// There is no `From<Bps> for f64`: `Bps::try_new` reads an f64 as a
// basis-point count (25.0 → 25bp), so an f64 conversion returning the decimal
// (25bp → 0.0025) would be asymmetric with it. Use [`Bps::as_decimal`],
// [`Bps::as_percent`], or `f64::from(bp.as_bp())`.

/// Converts to whole basis points, **rounding** to the nearest bp — `Bps` is an
/// integer type, so sub-bp precision (common for credit/swap spreads) is lost.
/// Keep the [`Rate`] (or use [`Rate::as_decimal`]) when full precision matters.
impl From<Rate> for Bps {
    fn from(rate: Rate) -> Self {
        Bps::new(rate.as_bp())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_bp_accessors_scale_without_binary_noise() {
        let rate = |x: f64| Rate::from_decimal(x).expect("finite rate");
        assert_eq!(rate(0.0029).as_bp_f64(), 29.0);
        assert_eq!(rate(0.00625).as_bp_f64(), 62.5);
        assert_eq!(rate(-0.0001).as_bp_f64(), -1.0);
        // Rounded integer accessors are unchanged.
        assert_eq!(rate(0.00625).as_bp(), 63);
        let percent = |x: f64| Rate::from_percent(x).expect("finite percent");
        assert_eq!(percent(1.1).as_bp_f64(), 110.0);
        assert_eq!(percent(0.07).as_bp_f64(), 7.0);
        assert_eq!(percent(0.29).as_bp_f64(), 29.0);
        assert_eq!(percent(1.1).as_percent(), 1.1);
        assert_eq!(percent(5.0), rate(0.05));
    }

    #[test]
    fn from_percent_keeps_tiny_magnitudes_ordering_and_signed_zero() {
        let percent = |x: f64| Rate::from_percent(x).expect("finite percent").as_decimal();
        // Beyond Decimal's 28 fractional digits the f64 quotient is used.
        assert_eq!(percent(1e-27), 1e-27 / 100.0);
        assert!(percent(9.4e-27) < percent(9.49e-27));
        assert!(percent(9.49e-27) < percent(9.5e-27));
        assert!(percent(-0.0).is_sign_negative());
        assert!(Rate::from_decimal(-0.0)
            .unwrap()
            .as_percent()
            .is_sign_negative());
    }

    #[test]
    fn bps_checked_abs_rejects_the_unrepresentable_minimum() {
        assert_eq!(Bps::new(-25).checked_abs().unwrap(), Bps::new(25));
        assert!(Bps::new(i32::MIN).checked_abs().is_err());
    }

    #[test]
    fn fractional_bp_accessors_preserve_tiny_finite_values() {
        for value in [1e-30, -1e-30, 1e-300, -1e-300, f64::from_bits(1)] {
            assert_eq!(
                Rate::from_decimal(value).expect("finite rate").as_bp_f64(),
                value * 10_000.0
            );
        }
    }

    #[test]
    fn rate_from_str_accepts_decimal_percent_and_bp() {
        assert_eq!(
            "0.05".parse::<Rate>().unwrap(),
            Rate::from_decimal(0.05).expect("valid rate fixture")
        );
        assert_eq!(
            "5%".parse::<Rate>().unwrap(),
            Rate::from_percent(5.0).expect("valid rate fixture")
        );
        assert_eq!(
            " 5 % ".parse::<Rate>().unwrap(),
            Rate::from_percent(5.0).expect("valid rate fixture")
        );
        assert_eq!("25bp".parse::<Rate>().unwrap(), Rate::from_bp(25));
        assert_eq!("25BPS".parse::<Rate>().unwrap(), Rate::from_bp(25));
        assert_eq!("25 bps".parse::<Rate>().unwrap(), Rate::from_bp(25));
        assert!(("62.5bp".parse::<Rate>().unwrap().as_decimal() - 0.00625).abs() < 1e-15);
        assert_eq!(
            "-1%".parse::<Rate>().unwrap(),
            Rate::from_percent(-1.0).expect("valid rate fixture")
        );
    }

    #[test]
    fn rate_from_str_rejects_garbage() {
        assert!("".parse::<Rate>().is_err());
        assert!("abc".parse::<Rate>().is_err());
        assert!("5%%".parse::<Rate>().is_err());
        assert!("inf".parse::<Rate>().is_err());
        assert!("nan%".parse::<Rate>().is_err());
        assert!("bp".parse::<Rate>().is_err());
    }

    #[test]
    fn bp_try_new_rejects_non_finite_values() {
        assert_eq!(Bps::try_new(12.0).expect("whole bp").as_bp(), 12);

        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(Bps::try_new(bad).is_err(), "try_new must reject {bad}");
        }
    }

    #[test]
    fn bp_try_new_rejects_fractional_basis_points() {
        for fractional in [12.4, 62.5, -0.5, 12.6] {
            let err = Bps::try_new(fractional).expect_err("fractional bp must be rejected");
            assert!(
                err.to_string().contains("whole number"),
                "unexpected error for {fractional}: {err}"
            );
        }
        // Whole-valued floats (including negatives and zero) are accepted.
        assert_eq!(Bps::try_new(-25.0).unwrap().as_bp(), -25);
        assert_eq!(Bps::try_new(0.0).unwrap().as_bp(), 0);
    }

    #[test]
    fn bp_try_new_rejects_values_that_round_outside_i32() {
        assert_eq!(Bps::try_new(i32::MAX as f64).unwrap().as_bp(), i32::MAX);
        assert_eq!(Bps::try_new(i32::MIN as f64).unwrap().as_bp(), i32::MIN);
        assert!(Bps::try_new(i32::MAX as f64 + 1.0).is_err());
        assert!(Bps::try_new(i32::MIN as f64 - 1.0).is_err());
    }

    #[test]
    fn serde_round_trip() {
        let rate = Rate::from_percent(2.5).expect("valid rate fixture");
        let json = serde_json::to_string(&rate).expect("JSON serialization should succeed in test");
        let deserialized: Rate =
            serde_json::from_str(&json).expect("JSON deserialization should succeed in test");
        assert_eq!(rate, deserialized);

        let bp = Bps::new(250);
        let json = serde_json::to_string(&bp).expect("JSON serialization should succeed in test");
        let deserialized: Bps =
            serde_json::from_str(&json).expect("JSON deserialization should succeed in test");
        assert_eq!(bp, deserialized);
    }

    /// Deserialization must route through the validating constructors so
    /// non-finite values cannot enter via formats that can encode them
    /// (JSON rejects NaN at the parser level, but binary formats do not).
    #[test]
    fn deserialize_rejects_non_finite() {
        use serde::de::value::Error as ValueError;
        use serde::de::IntoDeserializer;

        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let rate: std::result::Result<Rate, ValueError> =
                Rate::deserialize(bad.into_deserializer());
            assert!(rate.is_err(), "Rate must reject {bad}");
        }

        // Finite values still deserialize.
        let rate: std::result::Result<Rate, ValueError> =
            Rate::deserialize(0.05_f64.into_deserializer());
        assert_eq!(
            rate.expect("finite rate"),
            Rate::from_decimal(0.05).expect("valid rate fixture")
        );
    }
}
