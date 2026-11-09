//! Strongly typed financial rate units with explicit conversion semantics.
//!
//! - [`Rate`] stores a finite signed binary64 decimal: `0.05` is 5% or 500 bp.
//!   Decimal names the unit, not `Decimal` storage.
//! - [`Bps`] stores an exact signed whole `i32` basis-point quote: 1 bp is
//!   0.01% or 0.0001 decimal, and 10,000 bp is 100%.
//!
//! Every construction, parse, arithmetic result, negation and restore
//! normalizes signed zero, rejects non-finite values and rejects a nonzero
//! result that rounds to zero. Numeric unit scaling is one ordinary binary64
//! operation (`p / 100`, `b / 10_000`, `r * 100`, `r * 10_000`); text quotes
//! use their own correctly rounded parser. Fractional basis points belong in
//! `Rate`; `Bps` arithmetic is exact or fails.
//!
//! # Example
//!
//! ```rust
//! use finstack_quant_core::types::{Bps, Rate};
//!
//! let rate = Rate::from_decimal(0.05)?;
//! assert_eq!(rate, Rate::from_percent(5.0)?);
//! assert_eq!(rate, Rate::from_bp(500.0)?);
//! assert_eq!(rate.as_percent()?, 5.0);
//! assert_eq!(Bps::new(500).as_rate(), rate);
//! assert_eq!("62.5bp".parse::<Rate>()?.as_bp()?, 62.5);
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```

use std::fmt;
use std::ops::Neg;

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::math::units::BASIS_POINTS_PER_UNIT;
use crate::{Error, InputError, NonFiniteKind};

/// Reject a non-finite value with the shared input error.
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

/// Map `-0.0` to `+0.0`; every other value is unchanged.
#[inline]
fn normalize_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

/// Error for a nonzero exact or lexical value whose binary64 result is zero.
fn underflow(context: &str) -> Error {
    Error::Validation(format!(
        "{context}: a nonzero rate value rounds to zero in binary64"
    ))
}

/// Admit the result of one scaling or arithmetic step.
///
/// `nonzero_source` says whether the exact real result is known to be
/// nonzero, so a zero binary64 result is an underflow rather than a genuine
/// zero.
fn admit(result: f64, nonzero_source: bool, context: &str) -> Result<f64> {
    let result = ensure_finite(result)?;
    if result == 0.0 && nonzero_source {
        return Err(underflow(context));
    }
    Ok(normalize_zero(result))
}

/// A financial rate stored as a finite binary64 decimal.
///
/// For example, `0.05` represents 5% or 500 bp. Negative values, including
/// values below `-1`, are valid at this layer; each financial operation owns
/// its own stricter domain. Signed zero is always normalized to `+0.0`, so
/// equality and ordering use the stored finite value.
///
/// # Serde
///
/// Serialized as the decimal-unit `f64`. Deserialization routes through
/// [`Rate::from_decimal`], so non-finite values are rejected even for binary
/// formats that can encode them, and `-0.0` restores as `+0.0`.
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
    /// Create a rate from a decimal value (`0.05` for 5%).
    ///
    /// The value is stored unchanged apart from signed-zero normalization.
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
        ensure_finite(decimal).map(|value| Self(normalize_zero(value)))
    }

    /// Create a rate from a percentage value with one binary64 `percent / 100`.
    ///
    /// A genuine `f64` input already denotes a binary64 value, so `1.1`
    /// percent is the nearest double to `1.1 / 100`; it can differ by one ULP
    /// from the text quote `"1.1%"`, which is parsed by [`str::parse`].
    ///
    /// # Errors
    ///
    /// Returns `Error::Input(InputError::NonFiniteValue { .. })` for a
    /// non-finite input, and `Error::Validation` when a nonzero percent
    /// rounds to a zero rate.
    ///
    /// # Arguments
    ///
    /// * `percent` - Rate in percentage points, where one percent is `1.0`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::types::Rate;
    ///
    /// assert_eq!(Rate::from_percent(5.0)?.as_decimal(), 0.05);
    /// assert!(Rate::from_percent(f64::from_bits(1)).is_err());
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn from_percent(percent: f64) -> Result<Self> {
        let percent = ensure_finite(percent)?;
        admit(percent / 100.0, percent != 0.0, "percent to rate").map(Self)
    }

    /// Create a rate from a basis-point quote with one binary64 `bp / 10_000`.
    ///
    /// Fractional quotes are accepted (`62.5` bp is `0.00625` decimal), and
    /// negative quotes are valid because spreads and rates may be negative.
    ///
    /// # Errors
    ///
    /// Returns `Error::Input(InputError::NonFiniteValue { .. })` for a
    /// non-finite input, and `Error::Validation` when a nonzero quote rounds
    /// to a zero rate.
    ///
    /// # Arguments
    ///
    /// * `bp` - Basis-point quote, where `500.0` is 5% (`0.05` decimal).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::types::Rate;
    ///
    /// assert_eq!(Rate::from_bp(500.0)?.as_decimal(), 0.05);
    /// assert_eq!(Rate::from_bp(62.5)?.as_decimal(), 62.5 / 10_000.0);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn from_bp(bp: f64) -> Result<Self> {
        let bp = ensure_finite(bp)?;
        admit(
            bp / BASIS_POINTS_PER_UNIT,
            bp != 0.0,
            "basis points to rate",
        )
        .map(Self)
    }

    /// Return the stored decimal value (`0.05` for 5%).
    #[must_use]
    pub fn as_decimal(self) -> f64 {
        self.0
    }

    /// Return the percentage view `decimal * 100` as one binary64 product.
    ///
    /// # Errors
    ///
    /// Returns `Error::Input(InputError::NonFiniteValue { .. })` when the
    /// product overflows, for example for a huge finite rate.
    pub fn as_percent(self) -> Result<f64> {
        admit(self.0 * 100.0, self.0 != 0.0, "rate to percent")
    }

    /// Return the fractional basis-point view `decimal * 10_000` as one
    /// binary64 product, without rounding to a whole quote.
    ///
    /// The view need not round-trip bit-exactly: even a rate built from a
    /// whole basis-point quote can differ from that integer by floating
    /// error. Keep the original [`Bps`] when its exact integer matters.
    ///
    /// # Errors
    ///
    /// Returns `Error::Input(InputError::NonFiniteValue { .. })` when the
    /// product overflows.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::types::Rate;
    ///
    /// assert_eq!(Rate::from_decimal(0.00625)?.as_bp()?, 62.5);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn as_bp(self) -> Result<f64> {
        admit(
            self.0 * BASIS_POINTS_PER_UNIT,
            self.0 != 0.0,
            "rate to basis points",
        )
    }

    /// The zero rate, represented as decimal value `0.0`.
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

    /// Return the rate's absolute value.
    pub fn abs(self) -> Self {
        Self(normalize_zero(self.0.abs()))
    }

    /// Add two decimal rates and reject a non-finite result.
    ///
    /// Exact cancellation to zero is a valid zero result.
    ///
    /// # Errors
    ///
    /// Returns `Error::Input(InputError::NonFiniteValue { .. })` if the sum
    /// overflows.
    ///
    /// # Arguments
    ///
    /// * `rhs` - Decimal rate added to `self`.
    pub fn checked_add(self, rhs: Self) -> Result<Self> {
        admit(self.0 + rhs.0, false, "rate addition").map(Self)
    }

    /// Subtract two decimal rates and reject a non-finite result.
    ///
    /// # Errors
    ///
    /// Returns `Error::Input(InputError::NonFiniteValue { .. })` if the
    /// difference overflows.
    ///
    /// # Arguments
    ///
    /// * `rhs` - Decimal rate subtracted from `self`.
    pub fn checked_sub(self, rhs: Self) -> Result<Self> {
        admit(self.0 - rhs.0, false, "rate subtraction").map(Self)
    }

    /// Scale the decimal rate by a finite dimensionless scalar.
    ///
    /// The scalar is validated before any zero shortcut, so `0 * NaN` is an
    /// error, not zero.
    ///
    /// # Errors
    ///
    /// Returns `Error::Input(InputError::NonFiniteValue { .. })` for a
    /// non-finite scalar or product, and `Error::Validation` when a nonzero
    /// product rounds to zero.
    ///
    /// # Arguments
    ///
    /// * `rhs` - Finite dimensionless multiplier.
    pub fn checked_mul(self, rhs: f64) -> Result<Self> {
        let rhs = ensure_finite(rhs)?;
        admit(
            self.0 * rhs,
            self.0 != 0.0 && rhs != 0.0,
            "rate multiplication",
        )
        .map(Self)
    }

    /// Divide the decimal rate by a finite nonzero dimensionless scalar.
    ///
    /// # Errors
    ///
    /// Returns `Error::Input(InputError::NonFiniteValue { .. })` for a
    /// non-finite scalar or quotient, `Error::Validation` for a zero divisor
    /// of either sign, and `Error::Validation` when a nonzero quotient rounds
    /// to zero.
    ///
    /// # Arguments
    ///
    /// * `rhs` - Finite nonzero dimensionless divisor.
    pub fn checked_div(self, rhs: f64) -> Result<Self> {
        let rhs = ensure_finite(rhs)?;
        if rhs == 0.0 {
            return Err(Error::Validation(
                "cannot divide a rate by zero".to_string(),
            ));
        }
        admit(self.0 / rhs, self.0 != 0.0, "rate division").map(Self)
    }
}

impl fmt::Display for Rate {
    /// Writes the shortest decimal-unit text that parses back to the same
    /// stored value (`0.05` for 5%). This is not a percent presentation or a
    /// financial rounding stage.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unit of a parsed rate quote, as its decimal exponent shift.
#[derive(Clone, Copy)]
enum QuoteUnit {
    Decimal,
    Percent,
    BasisPoints,
}

impl QuoteUnit {
    fn shift(self) -> i64 {
        match self {
            Self::Decimal => 0,
            Self::Percent => -2,
            Self::BasisPoints => -4,
        }
    }
}

/// Split an ASCII-lowercased trimmed quote into its numeric token and unit.
///
/// Whitespace is allowed only between the numeric token and its suffix.
fn split_unit(lower: &str) -> (&str, QuoteUnit) {
    if let Some(num) = lower
        .strip_suffix("bps")
        .or_else(|| lower.strip_suffix("bp"))
    {
        (num.trim_end(), QuoteUnit::BasisPoints)
    } else if let Some(num) = lower.strip_suffix('%') {
        (num.trim_end(), QuoteUnit::Percent)
    } else {
        (lower, QuoteUnit::Decimal)
    }
}

/// Parse a numeric token of grammar
/// `[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?`, apply the
/// unit's decimal exponent shift in checked integer arithmetic, and convert
/// once to the nearest binary64 value (ties-to-even).
///
/// Returns `None` for a grammar violation or an exponent outside `i64`.
fn parse_quote_number(token: &str, unit: QuoteUnit) -> Option<std::result::Result<f64, Error>> {
    let bytes = token.as_bytes();
    let mut index = 0;
    let negative = match bytes.first() {
        Some(b'+') => {
            index = 1;
            false
        }
        Some(b'-') => {
            index = 1;
            true
        }
        _ => false,
    };
    let int_start = index;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    let int_digits = &token[int_start..index];
    let mut frac_digits = "";
    if index < bytes.len() && bytes[index] == b'.' {
        index += 1;
        let frac_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        frac_digits = &token[frac_start..index];
    }
    if int_digits.is_empty() && frac_digits.is_empty() {
        return None;
    }
    let mut exponent: i64 = 0;
    if index < bytes.len() && bytes[index] == b'e' {
        index += 1;
        let exp_start = index;
        if index < bytes.len() && (bytes[index] == b'+' || bytes[index] == b'-') {
            index += 1;
        }
        let digits_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == digits_start {
            return None;
        }
        exponent = token[exp_start..index].parse::<i64>().ok()?;
    }
    if index != bytes.len() {
        return None;
    }
    let digits = format!("{int_digits}{frac_digits}");
    let significant = digits.trim_start_matches('0');
    if significant.is_empty() {
        return Some(Ok(0.0));
    }
    // value = significant * 10^scaled, then the unit shift.
    let frac_len = i64::try_from(frac_digits.len()).ok()?;
    let scaled = i128::from(exponent) - i128::from(frac_len) + i128::from(unit.shift());
    // Order of magnitude of the most significant digit.
    let magnitude = scaled + i128::try_from(significant.len()).ok()? - 1;
    if magnitude > 400 {
        return Some(Err(InputError::NonFiniteValue {
            kind: if negative {
                NonFiniteKind::NegInfinity
            } else {
                NonFiniteKind::PosInfinity
            },
        }
        .into()));
    }
    if magnitude < -400 {
        return Some(Err(underflow("rate quote")));
    }
    let sign = if negative { "-" } else { "" };
    let value = format!("{sign}{significant}e{scaled}")
        .parse::<f64>()
        .ok()?;
    Some(admit(value, true, "rate quote"))
}

impl std::str::FromStr for Rate {
    type Err = Error;

    /// Parse a rate quote with an optional unit suffix.
    ///
    /// After trimming outer whitespace, the numeric token follows
    /// `[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?` with an
    /// optional case-insensitive `%`, `bp` or `bps` suffix. Whitespace is
    /// allowed only between the number and its suffix. A bare number is a
    /// decimal rate (`"5"` is 500%), never a guessed percent. The unit shift
    /// is applied to the exact decimal before one correctly rounded binary64
    /// conversion, so `"1e310bp"` is the finite `1e306`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] for a grammar violation (underscores,
    /// grouping, missing digits, non-finite words, repeated or mixed units,
    /// an exponent outside `i64`) or a nonzero quote that rounds to zero, and
    /// `Error::Input(InputError::NonFiniteValue { .. })` when the quote
    /// overflows binary64.
    fn from_str(s: &str) -> Result<Self> {
        let invalid = || {
            Error::Validation(format!(
                "invalid rate {s:?}: expected a decimal (\"0.05\"), percent (\"5%\") or basis-point (\"25bp\") quote"
            ))
        };
        let lower = s.trim().to_ascii_lowercase();
        let (token, unit) = split_unit(&lower);
        let value = parse_quote_number(token, unit).ok_or_else(invalid)??;
        Ok(Self(value))
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
        Self(normalize_zero(-self.0))
    }
}

/// An exact signed whole basis-point quote.
///
/// - 1 bp = 0.01% = 0.0001 decimal
/// - 100 bp = 1%
/// - 10,000 bp = 100%
///
/// Values are stored and serialized as `i32` basis points. Arithmetic is
/// exact or fails; fractional basis points belong in [`Rate`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Bps(i32);

impl Bps {
    /// Create a basis-point value from an integer quote.
    ///
    /// # Arguments
    ///
    /// * `bp` - Whole basis-point quote, where `500` is 5%.
    pub fn new(bp: i32) -> Self {
        Self(bp)
    }

    /// Create a Bps value from a floating-point basis-point amount.
    ///
    /// Accepts a finite, exactly integral `f64` in the `i32` range (for
    /// example `25.0` for 25 bp), without epsilon, truncation, rounding or
    /// saturation; `-0.0` becomes `0`. Use [`Rate`] for sub-bp precision.
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
        if bp < f64::from(i32::MIN) || bp > f64::from(i32::MAX) {
            return Err(InputError::ConversionOverflow.into());
        }
        Ok(Self(bp as i32))
    }

    /// Return the exact integer basis-point quote.
    pub fn as_bp(self) -> i32 {
        self.0
    }

    /// Return the decimal-rate view `bp / 10_000` (100 bp = 0.01).
    ///
    /// The integer input is exact; the binary64 quotient is always finite.
    pub fn as_decimal(self) -> f64 {
        normalize_zero(f64::from(self.0) / BASIS_POINTS_PER_UNIT)
    }

    /// Return the percentage view `bp / 100` (100 bp = 1.0).
    pub fn as_percent(self) -> f64 {
        normalize_zero(f64::from(self.0) / 100.0)
    }

    /// Convert the basis-point quote to a [`Rate`] (`bp / 10_000`).
    ///
    /// Always valid: an `i32` quote divided by 10,000 is finite, and a
    /// nonzero quote never rounds to zero.
    pub fn as_rate(self) -> Rate {
        Rate(self.as_decimal())
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
    ///
    /// # Arguments
    ///
    /// * `rhs` - Basis-point quote added to `self`.
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
    ///
    /// # Arguments
    ///
    /// * `rhs` - Basis-point quote subtracted from `self`.
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
    ///
    /// # Arguments
    ///
    /// * `rhs` - Integer multiplier.
    pub fn checked_mul(self, rhs: i32) -> Result<Self> {
        self.0
            .checked_mul(rhs)
            .map(Self)
            .ok_or_else(|| InputError::ConversionOverflow.into())
    }

    /// Divide by an integer scalar exactly.
    ///
    /// The quotient must be a whole number of basis points: `3bp / 2` is an
    /// error, not `1bp`. Use [`Rate`] arithmetic when a fractional result is
    /// intended.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` for a zero divisor,
    /// `InputError::ConversionOverflow` for the unrepresentable
    /// `i32::MIN / -1` quotient, and `Error::Validation` for a nonzero
    /// remainder.
    ///
    /// # Arguments
    ///
    /// * `rhs` - Nonzero integer divisor.
    pub fn checked_div(self, rhs: i32) -> Result<Self> {
        if rhs == 0 {
            return Err(Error::Validation(
                "cannot divide basis points by zero".to_string(),
            ));
        }
        let quotient = self
            .0
            .checked_div(rhs)
            .ok_or_else(|| Error::from(InputError::ConversionOverflow))?;
        if self.0.checked_rem(rhs) != Some(0) {
            return Err(Error::Validation(format!(
                "{}bp / {rhs} is not a whole number of basis points",
                self.0
            )));
        }
        Ok(Self(quotient))
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

// NOTE: There is intentionally no `impl From<Bps> for f64`: `Bps::try_new`
// reads an f64 as a basis-point count (25.0 → 25bp), so an f64 conversion
// returning the decimal (25bp → 0.0025) would be asymmetric with it. Use
// [`Bps::as_decimal`], [`Bps::as_percent`], or `f64::from(bp.as_bp())`.

#[cfg(test)]
mod tests {
    use super::*;

    fn rate(x: f64) -> Rate {
        Rate::from_decimal(x).expect("finite rate")
    }

    fn parse(s: &str) -> Result<Rate> {
        s.parse::<Rate>()
    }

    #[test]
    fn units_are_explicit_single_binary64_scalings() {
        assert_eq!(rate(0.05), Rate::from_percent(5.0).expect("percent"));
        assert_eq!(rate(0.05), Rate::from_bp(500.0).expect("bp"));
        assert_eq!(
            Rate::from_bp(62.5).expect("fractional").as_decimal(),
            62.5 / 10_000.0
        );
        assert_eq!(
            Rate::from_percent(1.1).expect("percent").as_decimal(),
            1.1 / 100.0
        );
        assert_eq!(rate(0.00625).as_bp().expect("view"), 0.00625 * 10_000.0);
        assert_eq!(rate(-2.0).as_decimal(), -2.0);
    }

    #[test]
    fn signed_zero_is_normalized_everywhere() {
        let zeros = [
            rate(-0.0),
            Rate::from_percent(-0.0).expect("zero"),
            Rate::from_bp(-0.0).expect("zero"),
            -Rate::ZERO,
            rate(-0.0).abs(),
            rate(1.0).checked_sub(rate(1.0)).expect("cancel"),
            rate(-0.0).checked_mul(5.0).expect("zero product"),
            parse("-0").expect("zero"),
            parse("-0e-10000%").expect("lexical zero"),
            serde_json::from_str::<Rate>("-0.0").expect("restore"),
        ];
        for zero in zeros {
            assert!(zero.as_decimal().is_sign_positive(), "{zero:?}");
            assert!(zero.is_zero());
            assert_eq!(zero, Rate::ZERO);
        }
        assert!(Bps::try_new(-0.0).expect("zero").is_zero());
    }

    #[test]
    fn nonzero_underflow_is_rejected_and_subnormals_are_valid() {
        let tiny = f64::from_bits(1);
        assert!(Rate::from_percent(tiny).is_err());
        assert!(Rate::from_bp(tiny).is_err());
        assert!(rate(tiny).checked_div(4.0).is_err());
        assert!(rate(tiny).checked_mul(0.25).is_err());
        assert_eq!(rate(tiny).as_decimal(), tiny);
        assert_eq!(
            rate(-tiny)
                .checked_mul(1.0)
                .expect("subnormal")
                .as_decimal(),
            -tiny
        );
    }

    #[test]
    fn views_overflow_with_errors() {
        let huge = rate(f64::MAX);
        assert!(huge.as_percent().is_err());
        assert!(huge.as_bp().is_err());
        assert!(rate(0.05).as_percent().is_ok());
    }

    #[test]
    fn scalar_arithmetic_validates_operands_first() {
        assert!(Rate::ZERO.checked_mul(f64::NAN).is_err());
        assert!(Rate::ZERO.checked_mul(f64::INFINITY).is_err());
        assert!(Rate::ZERO.checked_div(f64::INFINITY).is_err());
        assert!(rate(0.05).checked_div(0.0).is_err());
        assert!(rate(0.05).checked_div(-0.0).is_err());
        assert!(rate(f64::MAX).checked_mul(2.0).is_err());
        assert!(rate(f64::MAX).checked_add(rate(f64::MAX)).is_err());
        assert_eq!(
            rate(0.05).checked_mul(0.0).expect("genuine zero"),
            Rate::ZERO
        );
    }

    #[test]
    fn quote_grammar_units_and_rejections() {
        assert_eq!(parse("0.05").expect("decimal"), rate(0.05));
        assert_eq!(parse("5").expect("bare is decimal").as_decimal(), 5.0);
        assert_eq!(parse("5%").expect("percent").as_decimal(), 0.05);
        assert_eq!(parse(" 5 % ").expect("suffix space").as_decimal(), 0.05);
        assert_eq!(parse(".5%").expect("leading dot").as_decimal(), 0.005);
        assert_eq!(parse("5.").expect("trailing dot").as_decimal(), 5.0);
        assert_eq!(
            parse("+1e2 BP").expect("signed exponent").as_decimal(),
            0.01
        );
        assert_eq!(parse("25BPS").expect("bps").as_decimal(), 0.0025);
        assert_eq!(
            parse("62.5bp").expect("fractional bp").as_decimal(),
            0.00625
        );
        assert_eq!(parse("-1%").expect("negative").as_decimal(), -0.01);
        assert_eq!(parse("1.1%").expect("text percent").as_decimal(), 0.011);
        for bad in [
            "",
            "abc",
            "5%%",
            "inf",
            "nan%",
            "NaN",
            "bp",
            "1_000bp",
            "1,000",
            "1e 2%",
            "1bps%",
            "- 5",
            "0x10",
            "e5",
            ".",
            "1e",
            "1e+",
            "5 5%",
            "1e99999999999999999999",
            "0e99999999999999999999%",
        ] {
            assert!(parse(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn quote_scaling_is_exact_before_one_rounding() {
        assert_eq!(
            parse("1e310bp").expect("finite after shift").as_decimal(),
            1e306
        );
        assert!(parse("1e310").is_err());
        assert!(parse("1e-10000%").is_err());
        assert!(parse("0e-10000%").expect("lexical zero").is_zero());
        assert_eq!(parse("0.0029").expect("text").as_decimal(), 0.0029);
        assert_eq!(
            parse("4.9406564584124654e-324")
                .expect("min subnormal")
                .as_decimal(),
            f64::from_bits(1)
        );
        assert!(parse("1e-400").is_err());
        // Exactly halfway between 1.0 and its successor: ties to even (1.0).
        assert_eq!(
            parse("1.00000000000000011102230246251565404236316680908203125")
                .expect("halfway")
                .as_decimal(),
            1.0
        );
        // Just above halfway rounds up; a 100-digit significand is preserved
        // exactly until the single final rounding.
        assert_eq!(
            parse("1.000000000000000111022302462515654042363166809082031250001")
                .expect("above halfway")
                .as_decimal(),
            1.0 + f64::EPSILON
        );
        let long = format!("0.{}5%", "0".repeat(98));
        assert_eq!(parse(&long).expect("long significand").as_decimal(), 5e-101);
    }

    #[test]
    fn display_is_shortest_round_trip_decimal_text() {
        for value in [0.05, 1e-30, -2.5, f64::from_bits(1), f64::MAX, 0.1 + 0.2] {
            let shown = rate(value).to_string();
            assert_eq!(
                parse(&shown).expect("display parses"),
                rate(value),
                "{shown}"
            );
        }
        assert_eq!(rate(0.05).to_string(), "0.05");
        assert_eq!(Bps::new(25).to_string(), "25bp");
    }

    #[test]
    fn bps_admission_is_exact_and_whole() {
        assert_eq!(Bps::try_new(12.0).expect("whole bp").as_bp(), 12);
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 12.4, 62.5, -0.5] {
            assert!(Bps::try_new(bad).is_err(), "{bad}");
        }
        assert_eq!(
            Bps::try_new(f64::from(i32::MAX)).expect("max").as_bp(),
            i32::MAX
        );
        assert_eq!(
            Bps::try_new(f64::from(i32::MIN)).expect("min").as_bp(),
            i32::MIN
        );
        assert!(Bps::try_new(f64::from(i32::MAX) + 1.0).is_err());
        assert!(Bps::try_new(f64::from(i32::MIN) - 1.0).is_err());
        assert_eq!(Bps::new(500).as_rate(), rate(0.05));
        assert_eq!(Bps::new(-25).as_decimal(), -0.0025);
    }

    #[test]
    fn bps_arithmetic_is_exact_or_fails() {
        assert!(Bps::new(3).checked_div(2).is_err());
        assert!(Bps::new(-3).checked_div(2).is_err());
        assert_eq!(Bps::new(-4).checked_div(2).expect("exact"), Bps::new(-2));
        assert_eq!(
            Bps::new(i32::MIN).checked_div(i32::MIN).expect("exact"),
            Bps::new(1)
        );
        assert!(Bps::new(i32::MIN).checked_div(-1).is_err());
        assert!(Bps::new(1).checked_div(0).is_err());
        assert!(Bps::new(i32::MAX).checked_add(Bps::new(1)).is_err());
        assert!(Bps::new(i32::MIN).checked_sub(Bps::new(1)).is_err());
        assert!(Bps::new(i32::MIN).checked_abs().is_err());
        assert!(Bps::new(i32::MIN).checked_neg().is_err());
        assert_eq!(Bps::new(-25).checked_abs().expect("abs"), Bps::new(25));
    }

    #[test]
    fn serde_round_trip_and_rejects_non_finite() {
        use serde::de::IntoDeserializer;
        use serde::de::value::Error as ValueError;

        let original = rate(0.025);
        let json = serde_json::to_string(&original).expect("serialize");
        assert_eq!(
            serde_json::from_str::<Rate>(&json).expect("restore"),
            original
        );
        let bp = Bps::new(250);
        let json = serde_json::to_string(&bp).expect("serialize");
        assert_eq!(json, "250");
        assert_eq!(serde_json::from_str::<Bps>(&json).expect("restore"), bp);
        assert!(serde_json::from_str::<Bps>("2.5").is_err());

        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let restored: std::result::Result<Rate, ValueError> =
                Rate::deserialize(bad.into_deserializer());
            assert!(restored.is_err(), "Rate must reject {bad}");
        }
    }
}
