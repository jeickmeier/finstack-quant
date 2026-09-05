//! Strongly typed financial rate units with explicit conversion semantics.
//!
//! - [`Rate`] stores an `f64` decimal: `0.05` is 5% or 500 bp.
//! - [`Percentage`] stores an `f64` percentage: `5.0` is 5% or 0.05 decimal.
//! - [`Bps`] stores an integer basis-point quote: 1 bp is 0.01% or 0.0001
//!   decimal, and 10,000 bp is 100%.
//!
//! `Rate` and `Percentage` serialize in their stored floating-point units and
//! reject non-finite values when deserialized. `Bps` serializes as its integer
//! basis-point quote. [`Percentage::new`] and the floating-point `Bps`
//! conversion reject invalid values with validation errors.
//!
//! # Example
//!
//! ```rust
//! use finstack_quant_core::types::{Bps, Percentage, Rate};
//!
//! let rate = Rate::from_decimal(0.05).expect("valid rate fixture");
//! assert_eq!(rate, Rate::from_percent(5.0).expect("valid rate fixture"));
//! assert_eq!(rate, Rate::from_bp(500));
//! let percentage = Percentage::new(5.0).expect("finite percentage");
//! assert_eq!(rate.as_percent(), percentage.as_percent());
//! assert_eq!(rate.as_bp(), Bps::new(500).as_bp());
//! ```

use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Sub};

use serde::{Deserialize, Serialize};

use crate::error::{Error, InputError, NonFiniteKind};
use crate::Result;
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
        if !decimal.is_finite() {
            return Err(InputError::NonFiniteValue {
                kind: NonFiniteKind::classify(decimal),
            }
            .into());
        }
        Ok(Self(decimal))
    }

    /// Create a rate from a percentage value, rejecting non-finite inputs.
    ///
    /// # Errors
    ///
    /// Returns `Error::Input(InputError::NonFiniteValue { .. })` when
    /// `percent` is NaN or infinite.
    ///
    /// # Arguments
    ///
    /// * `percent` - Rate represented in percentage points, where one percent is `1.0`.
    pub fn from_percent(percent: f64) -> Result<Self> {
        Self::from_decimal(percent / 100.0)
    }

    /// Create a rate from an integer basis-point quote (500 bp = 5%).
    ///
    /// Negative values are accepted because spreads and rates may be negative.
    pub fn from_bp(bp: i32) -> Self {
        Self(bp as f64 / 10_000.0)
    }

    /// Return the internal decimal representation (0.05 for 5%).
    #[must_use]
    pub fn as_decimal(self) -> f64 {
        self.0
    }

    /// Return the percentage representation (5.0 for 5%).
    #[must_use]
    pub fn as_percent(self) -> f64 {
        self.0 * 100.0
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
        (self.0 * 10_000.0).round() as i32
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
            return Err(InputError::Invalid.into());
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
            (num, 10_000.0)
        } else if let Some(num) = lower.strip_suffix("bp") {
            (num, 10_000.0)
        } else if let Some(num) = lower.strip_suffix('%') {
            (num, 100.0)
        } else {
            (lower.as_str(), 1.0)
        };
        let value: f64 = number.trim().parse().map_err(|_| invalid())?;
        Self::from_decimal(value / divisor)
    }
}

impl TryFrom<f64> for Rate {
    type Error = Error;

    /// Fallible conversion from a decimal rate, where `0.05` represents 5%.
    fn try_from(decimal: f64) -> Result<Self> {
        Self::from_decimal(decimal)
    }
}

impl From<Rate> for f64 {
    fn from(rate: Rate) -> Self {
        rate.as_decimal()
    }
}

// Arithmetic operations.
//
// These operators panic if the result is non-finite (e.g. overflow of two large
// finite rates to ±∞, or multiplication by a non-finite scalar), because they
// check finiteness before storing the result. For inputs that may be untrusted or
// computed, use the `checked_*` variants ([`Rate::checked_add`], etc.), which
// return an error instead of panicking.
impl Add for Rate {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        {
            let value = self.0 + rhs.0;
            assert!(
                value.is_finite(),
                "rate arithmetic produced a non-finite value"
            );
            Self(value)
        }
    }
}

impl Sub for Rate {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        {
            let value = self.0 - rhs.0;
            assert!(
                value.is_finite(),
                "rate arithmetic produced a non-finite value"
            );
            Self(value)
        }
    }
}

impl Mul<f64> for Rate {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        {
            let value = self.0 * rhs;
            assert!(
                value.is_finite(),
                "rate arithmetic produced a non-finite value"
            );
            Self(value)
        }
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
        if !bp.is_finite() {
            return Err(InputError::NonFiniteValue {
                kind: NonFiniteKind::classify(bp),
            }
            .into());
        }
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
        self.0 as f64 / 10_000.0
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

    /// Return the absolute basis-point quote.
    pub fn abs(self) -> Self {
        Self(self.0.abs())
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
    /// Returns `InputError::Invalid` for a zero divisor, or
    /// `InputError::ConversionOverflow` for the unrepresentable
    /// `i32::MIN / -1` quotient.
    pub fn checked_div(self, rhs: i32) -> Result<Self> {
        if rhs == 0 {
            return Err(InputError::Invalid.into());
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
// `TryFrom<f64> for Bps` reads a basis-point *count* (25.0 → 25bp), so a
// `From<Bps> for f64` returning the *decimal* (25bp → 0.0025) made the f64
// conversions asymmetric: round-tripping 25bp through f64 yielded 0bp
// . Use the explicit
// accessors instead: [`Bps::as_decimal`], [`Bps::as_percent`], or
// `f64::from(bp.as_bp())` for the raw bp count.

impl Add for Bps {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Bps {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Mul<i32> for Bps {
    type Output = Self;

    fn mul(self, rhs: i32) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Div<i32> for Bps {
    type Output = Self;

    fn div(self, rhs: i32) -> Self::Output {
        Self(self.0 / rhs)
    }
}

impl Neg for Bps {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

/// A percentage stored as an `f64` in percentage units.
///
/// For example, `5.0` represents 5% or 0.05 decimal.
///
/// # Serde
///
/// Serialized transparently as the inner `f64` percentage value.
/// Deserialization routes through [`Percentage::new`], so non-finite
/// values (NaN, ±Infinity) are rejected even for binary formats that can
/// encode them.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize)]
pub struct Percentage(f64);

impl<'de> Deserialize<'de> for Percentage {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let percent = f64::deserialize(deserializer)?;
        Percentage::new(percent).map_err(serde::de::Error::custom)
    }
}

impl Percentage {
    /// Create a percentage value, rejecting non-finite inputs.
    ///
    /// Returns an error if `percent` is NaN or infinite, preventing silent
    /// corruption of downstream financial calculations.
    ///
    /// # Errors
    ///
    /// Returns `Error::Input(InputError::NonFiniteValue { .. })` when `percent`
    /// is NaN or infinite.
    ///
    /// # Arguments
    ///
    /// * `percent` - Percentage points, where `5.0` represents 5%.
    pub fn new(percent: f64) -> Result<Self> {
        if !percent.is_finite() {
            return Err(InputError::NonFiniteValue {
                kind: NonFiniteKind::classify(percent),
            }
            .into());
        }
        Ok(Self(percent))
    }

    /// Return the stored percentage value (5.0 represents 5%).
    pub fn as_percent(self) -> f64 {
        self.0
    }

    /// Return the decimal-rate representation (5.0% becomes 0.05).
    pub fn as_decimal(self) -> f64 {
        self.0 / 100.0
    }

    /// Convert the percentage to a Rate decimal wrapper.
    pub fn as_rate(self) -> Rate {
        Rate(self.0 / 100.0)
    }

    /// Convert to integer basis points, rounding to the nearest basis point.
    pub fn as_bp(self) -> i32 {
        (self.0 * 100.0).round() as i32
    }

    /// The zero percentage value.
    pub const ZERO: Self = Self(0.0);

    /// Return true when the percentage is exactly zero.
    pub fn is_zero(self) -> bool {
        self.0 == 0.0
    }

    /// Return true when the percentage is strictly positive.
    pub fn is_positive(self) -> bool {
        self.0 > 0.0
    }

    /// Return true when the percentage is strictly negative.
    pub fn is_negative(self) -> bool {
        self.0 < 0.0
    }

    /// Return the absolute percentage value.
    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }

    /// Add two percentages, rejecting non-finite results.
    ///
    /// # Errors
    ///
    /// Returns `InputError::NonFiniteValue` when the sum is NaN or infinite.
    pub fn checked_add(self, rhs: Self) -> Result<Self> {
        Self::new(self.0 + rhs.0)
    }

    /// Subtract two percentages, rejecting non-finite results.
    ///
    /// # Errors
    ///
    /// Returns `InputError::NonFiniteValue` when the difference is NaN or
    /// infinite.
    pub fn checked_sub(self, rhs: Self) -> Result<Self> {
        Self::new(self.0 - rhs.0)
    }

    /// Multiply by a scalar, rejecting non-finite results.
    ///
    /// # Errors
    ///
    /// Returns `InputError::NonFiniteValue` when `rhs` or the product is NaN
    /// or infinite.
    pub fn checked_mul(self, rhs: f64) -> Result<Self> {
        Self::new(self.0 * rhs)
    }

    /// Divide by a scalar, rejecting zero divisors and non-finite results.
    ///
    /// # Errors
    ///
    /// Returns `InputError::Invalid` for a zero divisor or
    /// `InputError::NonFiniteValue` for a non-finite quotient.
    pub fn checked_div(self, rhs: f64) -> Result<Self> {
        if rhs == 0.0 {
            return Err(InputError::Invalid.into());
        }
        Self::new(self.0 / rhs)
    }
}

impl fmt::Display for Percentage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}%", self.0)
    }
}

impl TryFrom<f64> for Bps {
    type Error = Error;

    /// Fallible conversion from basis-point `f64`. Rejects `NaN`,
    /// `±Infinity`, and fractional basis points (see [`Bps::try_new`]).
    fn try_from(bp: f64) -> Result<Self> {
        Self::try_new(bp)
    }
}

impl From<Percentage> for f64 {
    fn from(pct: Percentage) -> Self {
        pct.as_percent()
    }
}

impl TryFrom<f64> for Percentage {
    type Error = Error;

    /// Fallible conversion from percentage points, where `5.0` represents 5%.
    fn try_from(percent: f64) -> Result<Self> {
        Self::new(percent)
    }
}

impl Neg for Percentage {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

/// Converts to whole basis points, **rounding** to the nearest bp — `Bps` is an
/// integer type, so sub-bp precision (common for credit/swap spreads) is lost.
/// Keep the [`Rate`] (or use [`Rate::as_decimal`]) when full precision matters.
impl From<Rate> for Bps {
    fn from(rate: Rate) -> Self {
        Bps::new(rate.as_bp())
    }
}

impl From<Rate> for Percentage {
    fn from(rate: Rate) -> Self {
        Percentage(rate.as_percent())
    }
}

impl From<Bps> for Rate {
    fn from(bp: Bps) -> Self {
        Rate::from_bp(bp.as_bp())
    }
}

impl From<Bps> for Percentage {
    fn from(bp: Bps) -> Self {
        Percentage(bp.as_percent())
    }
}

impl From<Percentage> for Rate {
    fn from(pct: Percentage) -> Self {
        Rate(pct.as_percent() / 100.0)
    }
}

/// Converts to whole basis points, **rounding** to the nearest bp — `Bps` is an
/// integer type, so sub-bp precision is lost.
impl From<Percentage> for Bps {
    fn from(pct: Percentage) -> Self {
        Bps::new(pct.as_bp())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn rate_creation_and_conversion() {
        let rate = Rate::from_percent(2.5).expect("valid rate fixture");
        assert_eq!(rate.as_decimal(), 0.025);
        assert_eq!(rate.as_percent(), 2.5);
        assert_eq!(rate.as_bp(), 250);

        let rate2 = Rate::from_bp(250);
        assert_eq!(rate, rate2);

        let rate3 = Rate::from_decimal(0.025).expect("valid rate fixture");
        assert_eq!(rate, rate3);
    }

    #[test]
    fn try_from_percent_rejects_non_finite_values() {
        assert_eq!(
            Rate::from_percent(2.5).expect("finite percent"),
            Rate::from_percent(2.5).expect("valid rate fixture")
        );
        assert!(Rate::from_percent(f64::NAN).is_err());
        assert!(Rate::from_percent(f64::INFINITY).is_err());
        assert!(Rate::from_percent(f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn rate_arithmetic() {
        let rate1 = Rate::from_percent(2.0).expect("valid rate fixture");
        let rate2 = Rate::from_percent(1.5).expect("valid rate fixture");

        // Use approximate equality for floating point comparisons
        let eps = 1e-15;

        let result_add = rate1 + rate2;
        let expected_add = Rate::from_percent(3.5).expect("valid rate fixture");
        assert!((result_add.as_decimal() - expected_add.as_decimal()).abs() < eps);

        let result_sub = rate1 - rate2;
        let expected_sub = Rate::from_percent(0.5).expect("valid rate fixture");
        assert!((result_sub.as_decimal() - expected_sub.as_decimal()).abs() < eps);

        let result_mul = rate1 * 2.0;
        let expected_mul = Rate::from_percent(4.0).expect("valid rate fixture");
        assert!((result_mul.as_decimal() - expected_mul.as_decimal()).abs() < eps);

        let result_div = rate1.checked_div(2.0).unwrap();
        let expected_div = Rate::from_percent(1.0).expect("valid rate fixture");
        assert!((result_div.as_decimal() - expected_div.as_decimal()).abs() < eps);

        let result_neg = -rate1;
        let expected_neg = Rate::from_percent(-2.0).expect("valid rate fixture");
        assert!((result_neg.as_decimal() - expected_neg.as_decimal()).abs() < eps);
    }

    #[test]
    fn bp_creation_and_conversion() {
        let bp = Bps::new(250);
        assert_eq!(bp.as_bp(), 250);
        assert_eq!(bp.as_decimal(), 0.025);
        assert_eq!(bp.as_percent(), 2.5);
    }

    #[test]
    fn bp_try_new_and_try_from_reject_non_finite_values() {
        assert_eq!(Bps::try_new(12.0).expect("whole bp").as_bp(), 12);
        assert_eq!(Bps::try_from(13.0).expect("whole bp").as_bp(), 13);

        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(Bps::try_new(bad).is_err(), "try_new must reject {bad}");
            assert!(Bps::try_from(bad).is_err(), "TryFrom must reject {bad}");
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
            assert!(Bps::try_from(fractional).is_err());
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
    fn percentage_creation_and_conversion() {
        let pct = Percentage::new(12.5).expect("finite percentage");
        assert_eq!(pct.as_percent(), 12.5);
        assert_eq!(pct.as_decimal(), 0.125);
        assert_eq!(pct.as_bp(), 1250);
    }

    #[test]
    fn cross_conversions() {
        let rate = Rate::from_percent(2.5).expect("valid rate fixture");
        let bp: Bps = rate.into();
        let pct: Percentage = rate.into();

        assert_eq!(bp.as_bp(), 250);
        assert_eq!(pct.as_percent(), 2.5);

        let rate_from_bp: Rate = bp.into();
        let rate_from_pct: Rate = pct.into();

        assert_eq!(rate, rate_from_bp);
        assert_eq!(rate, rate_from_pct);
    }

    #[test]
    fn constants_and_predicates() {
        assert!(Rate::ZERO.is_zero());
        assert!(!Rate::ZERO.is_positive());
        assert!(!Rate::ZERO.is_negative());

        let positive = Rate::from_percent(1.0).expect("valid rate fixture");
        assert!(positive.is_positive());
        assert!(!positive.is_negative());

        let negative = Rate::from_percent(-1.0).expect("valid rate fixture");
        assert!(negative.is_negative());
        assert!(!negative.is_positive());

        assert_eq!(negative.abs(), positive);
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

        let pct = Percentage::new(12.5).expect("finite percentage");
        let json = serde_json::to_string(&pct).expect("JSON serialization should succeed in test");
        let deserialized: Percentage =
            serde_json::from_str(&json).expect("JSON deserialization should succeed in test");
        assert_eq!(pct, deserialized);
    }

    /// Deserialization must route through the validating constructors so
    /// non-finite values cannot enter via formats that can encode them
    /// (JSON rejects NaN at the parser level, but binary formats do not).
    /// .
    #[test]
    fn deserialize_rejects_non_finite() {
        use serde::de::value::Error as ValueError;
        use serde::de::IntoDeserializer;

        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let rate: std::result::Result<Rate, ValueError> =
                Rate::deserialize(bad.into_deserializer());
            assert!(rate.is_err(), "Rate must reject {bad}");

            let pct: std::result::Result<Percentage, ValueError> =
                Percentage::deserialize(bad.into_deserializer());
            assert!(pct.is_err(), "Percentage must reject {bad}");
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
