//! Rate and percentage types with conversion utilities.
//!
//! This module provides strongly-typed wrappers for financial rates and percentages,
//! with clear conversion semantics and arithmetic operations. The types prevent
//! confusion between different representations of the same concept (e.g., 0.05 vs
//! 5% vs 500 bps).
//!
//! # Types
//!
//! - [`Rate`]: Interest rates stored as decimal (0.05 = 5%)
//! - [`Bps`]: Basis points (1 bp = 0.01%, 10,000 bps = 100%)
//! - [`Percentage`]: Percentage values (5.0 = 5%)
//!
//! # Conversions
//!
//! All three types are interconvertible:
//! ```text
//! Rate <-> Bps <-> Percentage
//!
//! Examples:
//! - Rate::from_percent(5.0) = 0.05 decimal
//! - Bps::new(500) = 5.0% = 0.05 decimal
//! - Percentage::new(5.0) = 5.0% = 0.05 decimal
//! ```
//!
//! # Examples
//!
//! ## Basic rate operations
//!
//! ```rust
//! use finstack_quant_core::types::{Rate, Bps, Percentage};
//!
//! // Create rates in different formats
//! let rate1 = Rate::from_percent(5.0);
//! let rate2 = Rate::from_bps(500);
//! let rate3 = Rate::from_decimal(0.05);
//!
//! assert_eq!(rate1, rate2);
//! assert_eq!(rate2, rate3);
//!
//! // Convert between formats
//! assert_eq!(rate1.as_percent(), 5.0);
//! assert_eq!(rate1.as_bps(), 500);
//! assert_eq!(rate1.as_decimal(), 0.05);
//! ```
//!
//! ## Arithmetic operations
//!
//! ```rust
//! use finstack_quant_core::types::Rate;
//!
//! let base_rate = Rate::from_percent(3.0);
//! let spread = Rate::from_bps(50); // 0.5%
//!
//! let total = base_rate + spread;
//! assert!((total.as_percent() - 3.5).abs() < 1e-10);
//!
//! // Rate arithmetic supports scaling
//! let doubled = total * 2.0;
//! assert!((doubled.as_percent() - 7.0).abs() < 1e-10);
//! ```
//!
//! ## Using basis points for precision
//!
//! ```rust
//! use finstack_quant_core::types::Bps;
//!
//! // Basis points are ideal for small spreads
//! let spread = Bps::new(25); // 0.25%
//! assert_eq!(spread.as_percent(), 0.25);
//!
//! // Arithmetic in basis points
//! let total_spread = spread + Bps::new(10);
//! assert_eq!(total_spread.as_bps(), 35);
//! ```
//!
//! # Industry Conventions
//!
//! Basis points are the standard unit for quoting spreads and small rate changes
//! in fixed income markets:
//! - **Credit spreads**: Quoted in bps (e.g., "50 bps over SOFR")
//! - **Swap spreads**: Quoted in bps (e.g., "5y swap spread at 25 bps")
//! - **Yield changes**: Often reported in bps (e.g., "yields up 10 bps")
//!
//! # See Also
//!
//! - [`Rate`] - Primary rate type with conversion methods
//! - [`Bps`] - Basis points for precise spread quoting
//! - [`Percentage`] - Display-friendly percentage values

use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Sub};

use serde::{Deserialize, Serialize};

use crate::error::{Error, InputError, NonFiniteKind};
use crate::Result;

/// Classify a non-finite `f64` value into a `NonFiniteKind`.
///
/// The caller is responsible for ensuring `v` is not finite before calling.
fn non_finite_kind(v: f64) -> NonFiniteKind {
    if v.is_nan() {
        NonFiniteKind::NaN
    } else if v.is_sign_positive() {
        NonFiniteKind::PosInfinity
    } else {
        NonFiniteKind::NegInfinity
    }
}

/// A financial rate (e.g., interest rate, discount rate).
///
/// Internally stored as a decimal value where 0.05 represents 5%. This is the
/// standard representation for financial calculations (compounding, discounting).
///
/// # Representation
///
/// Rates are stored as `f64` decimals:
/// - 1% = 0.01
/// - 5% = 0.05
/// - 100% = 1.0
///
/// # Conversions
///
/// Provides methods to convert to/from:
/// - **Decimal**: The internal representation (0.05 for 5%)
/// - **Percentage**: Human-readable format (5.0 for 5%)
/// - **Basis points**: Fixed income market convention (500 bps for 5%)
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::types::Rate;
///
/// // Create from different formats
/// let r1 = Rate::from_decimal(0.05);
/// let r2 = Rate::from_percent(5.0);
/// let r3 = Rate::from_bps(500);
/// assert_eq!(r1, r2);
/// assert_eq!(r2, r3);
///
/// // Use in calculations
/// let notional = 1_000_000.0;
/// let interest = notional * r1.as_decimal();
/// assert_eq!(interest, 50_000.0);
/// ```
///
/// # Thread Safety
///
/// `Rate` is `Copy` and contains only a single `f64`, making it trivially
/// `Send + Sync`.
///
/// # Serde
///
/// Serialized transparently as the inner `f64` decimal. Deserialization
/// routes through [`Rate::try_from_decimal`], so non-finite values (NaN,
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
        Rate::try_from_decimal(decimal).map_err(serde::de::Error::custom)
    }
}

impl Rate {
    /// Create a rate from a decimal value (0.05 = 5%).
    ///
    /// # Panics
    ///
    /// Panics if `decimal` is not finite. Use [`try_from_decimal`](Self::try_from_decimal)
    /// for untrusted or external input.
    pub fn from_decimal(decimal: f64) -> Self {
        assert!(
            decimal.is_finite(),
            "Rate::from_decimal called with non-finite value: {decimal}"
        );
        Self(decimal)
    }

    /// Create a rate from a decimal value, rejecting non-finite inputs.
    ///
    /// Returns an error if `decimal` is NaN or infinite, preventing silent
    /// corruption of downstream financial calculations.
    ///
    /// # Errors
    ///
    /// Returns `Error::Input(InputError::NonFiniteValue { .. })` when `decimal`
    /// is NaN or infinite.
    pub fn try_from_decimal(decimal: f64) -> Result<Self> {
        if !decimal.is_finite() {
            return Err(InputError::NonFiniteValue {
                kind: non_finite_kind(decimal),
            }
            .into());
        }
        Ok(Self::from_decimal(decimal))
    }

    /// Create a rate from a percentage value (5.0 = 5%).
    ///
    /// # Panics
    ///
    /// Panics if `percent` is not finite. Use [`try_from_decimal`](Self::try_from_decimal)
    /// with manual conversion for untrusted input.
    pub fn from_percent(percent: f64) -> Self {
        assert!(
            percent.is_finite(),
            "Rate::from_percent called with non-finite value: {percent}"
        );
        Self(percent / 100.0)
    }

    /// Create a rate from basis points (500 bps = 5%)
    pub fn from_bps(bps: i32) -> Self {
        Self(bps as f64 / 10_000.0)
    }

    /// Get the rate as a decimal value
    #[must_use]
    pub fn as_decimal(self) -> f64 {
        self.0
    }

    /// Get the rate as a percentage value
    #[must_use]
    pub fn as_percent(self) -> f64 {
        self.0 * 100.0
    }

    /// Get the rate as basis points (rounded to nearest integer).
    ///
    /// # Overflow
    ///
    /// The rounded value is then saturated to the `i32` range by the float-to-int
    /// cast. For rates beyond about `±2_147_483_647` bps
    /// (`±21_474_836.47%`), prefer `(self.as_decimal() * 10_000.0).round() as i64`.
    #[must_use]
    pub fn as_bps(self) -> i32 {
        (self.0 * 10_000.0).round() as i32
    }

    /// Zero rate constant
    pub const ZERO: Self = Self(0.0);

    /// Check if the rate is zero
    pub fn is_zero(self) -> bool {
        self.0 == 0.0
    }

    /// Check if the rate is positive
    pub fn is_positive(self) -> bool {
        self.0 > 0.0
    }

    /// Check if the rate is negative
    pub fn is_negative(self) -> bool {
        self.0 < 0.0
    }

    /// Get the absolute value of the rate
    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }

    /// Add two rates, rejecting non-finite results.
    pub fn checked_add(self, rhs: Self) -> Result<Self> {
        Self::try_from_decimal(self.0 + rhs.0)
    }

    /// Subtract two rates, rejecting non-finite results.
    pub fn checked_sub(self, rhs: Self) -> Result<Self> {
        Self::try_from_decimal(self.0 - rhs.0)
    }

    /// Multiply by a scalar, rejecting non-finite results.
    pub fn checked_mul(self, rhs: f64) -> Result<Self> {
        Self::try_from_decimal(self.0 * rhs)
    }

    /// Divide by a scalar, rejecting zero divisors and non-finite results.
    pub fn checked_div(self, rhs: f64) -> Result<Self> {
        if rhs == 0.0 {
            return Err(InputError::Invalid.into());
        }
        Self::try_from_decimal(self.0 / rhs)
    }

    /// Negate the rate, rejecting non-finite results.
    pub fn checked_neg(self) -> Result<Self> {
        Self::try_from_decimal(-self.0)
    }
}

impl fmt::Display for Rate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.4}%", self.as_percent())
    }
}

impl From<f64> for Rate {
    /// Prefer the fallible [`Rate::try_from_decimal`] for untrusted inputs; this
    /// panics on non-finite values and is retained only for backward compatibility
    /// with `rate.into()` call sites.
    fn from(decimal: f64) -> Self {
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
// route through [`Rate::from_decimal`]. For inputs that may be untrusted or
// computed, use the `checked_*` variants ([`Rate::checked_add`], etc.), which
// return an error instead of panicking.
impl Add for Rate {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::from_decimal(self.0 + rhs.0)
    }
}

impl Sub for Rate {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::from_decimal(self.0 - rhs.0)
    }
}

impl Mul<f64> for Rate {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        Self::from_decimal(self.0 * rhs)
    }
}

impl Neg for Rate {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

/// Basis points - a unit of measure for rates (1 bp = 0.01%).
///
/// Commonly used in financial markets where small rate differences matter.
/// One basis point equals one hundredth of a percent:
/// - 1 bp = 0.01% = 0.0001 decimal
/// - 100 bps = 1%
/// - 10,000 bps = 100%
///
/// # Industry Usage
///
/// Basis points are the standard unit for quoting:
/// - **Credit spreads**: Corporate bond spread over benchmark (e.g., "150 bps")
/// - **Swap spreads**: Swap rate vs government bond yield
/// - **Rate changes**: Central bank moves (e.g., "Fed raised rates by 25 bps")
/// - **Fees**: Management fees (e.g., "expense ratio of 50 bps")
///
/// # Precision
///
/// Stored as `i32` to avoid floating-point accumulation errors when
/// aggregating small spreads or fees.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::types::Bps;
///
/// // Corporate bond spread over SOFR
/// let credit_spread = Bps::new(150); // 1.5%
/// assert_eq!(credit_spread.as_percent(), 1.5);
/// assert_eq!(credit_spread.as_decimal(), 0.015);
///
/// // Fee calculation
/// let management_fee = Bps::new(50); // 0.5%
/// let aum = 100_000_000.0; // $100M
/// let annual_fee = aum * management_fee.as_decimal();
/// assert_eq!(annual_fee, 500_000.0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Bps(i32);

impl Bps {
    /// Create a new Bps value
    pub fn new(bps: i32) -> Self {
        Self(bps)
    }

    /// Create a Bps value from a floating-point basis-point amount, rejecting non-finite inputs.
    ///
    /// Accepts a `f64` (e.g., `25.0` for 25 bps), validates finiteness, then rounds to
    /// the nearest integer. Use this when the raw input originates from an external source
    /// that could supply NaN or infinity.
    ///
    /// # Errors
    ///
    /// Returns `Error::Input(InputError::NonFiniteValue { .. })` when `bps` is NaN or infinite.
    pub fn try_new(bps: f64) -> Result<Self> {
        if !bps.is_finite() {
            return Err(InputError::NonFiniteValue {
                kind: non_finite_kind(bps),
            }
            .into());
        }
        Ok(Self(bps.round() as i32))
    }

    /// Get the basis points as an integer
    pub fn as_bps(self) -> i32 {
        self.0
    }

    /// Convert to decimal representation
    pub fn as_decimal(self) -> f64 {
        self.0 as f64 / 10_000.0
    }

    /// Convert to percentage representation
    pub fn as_percent(self) -> f64 {
        self.0 as f64 / 100.0
    }

    /// Convert to Rate
    pub fn as_rate(self) -> Rate {
        Rate::from_bps(self.0)
    }

    /// Zero basis points constant
    pub const ZERO: Self = Self(0);

    /// Check if zero
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Check if positive
    pub fn is_positive(self) -> bool {
        self.0 > 0
    }

    /// Check if negative
    pub fn is_negative(self) -> bool {
        self.0 < 0
    }

    /// Get absolute value
    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }

    /// Add two basis-point values, rejecting integer overflow.
    pub fn checked_add(self, rhs: Self) -> Result<Self> {
        self.0
            .checked_add(rhs.0)
            .map(Self)
            .ok_or_else(|| InputError::ConversionOverflow.into())
    }

    /// Subtract two basis-point values, rejecting integer overflow.
    pub fn checked_sub(self, rhs: Self) -> Result<Self> {
        self.0
            .checked_sub(rhs.0)
            .map(Self)
            .ok_or_else(|| InputError::ConversionOverflow.into())
    }

    /// Multiply by an integer scalar, rejecting integer overflow.
    pub fn checked_mul(self, rhs: i32) -> Result<Self> {
        self.0
            .checked_mul(rhs)
            .map(Self)
            .ok_or_else(|| InputError::ConversionOverflow.into())
    }

    /// Divide by an integer scalar, rejecting zero divisors.
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
    fn from(bps: i32) -> Self {
        Self::new(bps)
    }
}

impl From<Bps> for i32 {
    fn from(bps: Bps) -> Self {
        bps.as_bps()
    }
}

// NOTE: There is intentionally no `impl From<Bps> for f64`.
//
// `TryFrom<f64> for Bps` reads a basis-point *count* (25.0 → 25bp), so a
// `From<Bps> for f64` returning the *decimal* (25bp → 0.0025) made the f64
// conversions asymmetric: round-tripping 25bp through f64 yielded 0bp
// . Use the explicit
// accessors instead: [`Bps::as_decimal`], [`Bps::as_percent`], or
// `f64::from(bps.as_bps())` for the raw bp count.

// Arithmetic operations for Bps
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

/// A percentage value.
///
/// Provides a clear type for percentage values with human-readable display
/// formatting. Unlike [`Rate`], this stores values in percentage terms
/// (5.0 for 5%) rather than decimal (0.05).
///
/// # Use Cases
///
/// - Display to end users (formatted as "X.XX%")
/// - Configuration files where percentages are more intuitive
/// - Volatility quoting (e.g., "20% implied volatility")
/// - Performance metrics (e.g., "12.5% return")
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::types::Percentage;
///
/// let vol = Percentage::new(20.0); // 20% volatility
/// assert_eq!(vol.as_percent(), 20.0);
/// assert_eq!(vol.as_decimal(), 0.20);
///
/// // Display formatting
/// assert_eq!(format!("{}", vol), "20.00%");
/// ```
///
/// # Serde
///
/// Serialized transparently as the inner `f64` percentage value.
/// Deserialization routes through [`Percentage::try_new`], so non-finite
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
        Percentage::try_new(percent).map_err(serde::de::Error::custom)
    }
}

impl Percentage {
    /// Create a new percentage (12.5 = 12.5%).
    ///
    /// # Panics
    ///
    /// Panics if `percent` is not finite. Use [`try_new`](Self::try_new)
    /// for untrusted or external input.
    pub fn new(percent: f64) -> Self {
        assert!(
            percent.is_finite(),
            "Percentage::new called with non-finite value: {percent}"
        );
        Self(percent)
    }

    /// Create a percentage value, rejecting non-finite inputs.
    ///
    /// Returns an error if `pct` is NaN or infinite, preventing silent
    /// corruption of downstream financial calculations.
    ///
    /// # Errors
    ///
    /// Returns `Error::Input(InputError::NonFiniteValue { .. })` when `pct`
    /// is NaN or infinite.
    pub fn try_new(pct: f64) -> Result<Self> {
        if !pct.is_finite() {
            return Err(InputError::NonFiniteValue {
                kind: non_finite_kind(pct),
            }
            .into());
        }
        Ok(Self::new(pct))
    }

    /// Get the percentage value
    pub fn as_percent(self) -> f64 {
        self.0
    }

    /// Convert to decimal representation
    pub fn as_decimal(self) -> f64 {
        self.0 / 100.0
    }

    /// Convert to Rate
    pub fn as_rate(self) -> Rate {
        Rate::from_percent(self.0)
    }

    /// Convert to basis points
    pub fn as_bps(self) -> i32 {
        (self.0 * 100.0).round() as i32
    }

    /// Zero percentage constant
    pub const ZERO: Self = Self(0.0);

    /// Check if zero
    pub fn is_zero(self) -> bool {
        self.0 == 0.0
    }

    /// Check if positive
    pub fn is_positive(self) -> bool {
        self.0 > 0.0
    }

    /// Check if negative
    pub fn is_negative(self) -> bool {
        self.0 < 0.0
    }

    /// Get absolute value
    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }

    /// Add two percentages, rejecting non-finite results.
    pub fn checked_add(self, rhs: Self) -> Result<Self> {
        Self::try_new(self.0 + rhs.0)
    }

    /// Subtract two percentages, rejecting non-finite results.
    pub fn checked_sub(self, rhs: Self) -> Result<Self> {
        Self::try_new(self.0 - rhs.0)
    }

    /// Multiply by a scalar, rejecting non-finite results.
    pub fn checked_mul(self, rhs: f64) -> Result<Self> {
        Self::try_new(self.0 * rhs)
    }

    /// Divide by a scalar, rejecting zero divisors and non-finite results.
    pub fn checked_div(self, rhs: f64) -> Result<Self> {
        if rhs == 0.0 {
            return Err(InputError::Invalid.into());
        }
        Self::try_new(self.0 / rhs)
    }

    /// Negate the percentage, rejecting non-finite results.
    pub fn checked_neg(self) -> Result<Self> {
        Self::try_new(-self.0)
    }
}

impl fmt::Display for Percentage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}%", self.0)
    }
}

impl From<f64> for Percentage {
    /// Prefer the fallible [`Percentage::try_new`]; this panics on non-finite input
    /// and remains only for backward compatibility.
    fn from(percent: f64) -> Self {
        Self::new(percent)
    }
}

impl TryFrom<f64> for Bps {
    type Error = Error;

    /// Fallible conversion from basis-point `f64`. Rejects `NaN` and
    /// `±Infinity`.
    fn try_from(bps: f64) -> Result<Self> {
        Self::try_new(bps)
    }
}

impl From<Percentage> for f64 {
    fn from(pct: Percentage) -> Self {
        pct.as_percent()
    }
}

// Arithmetic operations for Percentage.
//
// As with [`Rate`], these operators panic on a non-finite result (they route
// through [`Percentage::new`]). Use the `checked_*` variants for untrusted or
// computed inputs.
impl Add for Percentage {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.0 + rhs.0)
    }
}

impl Sub for Percentage {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.0 - rhs.0)
    }
}

impl Mul<f64> for Percentage {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        Self::new(self.0 * rhs)
    }
}

impl Neg for Percentage {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

// Cross-conversions between types
/// Converts to whole basis points, **rounding** to the nearest bp — `Bps` is an
/// integer type, so sub-bp precision (common for credit/swap spreads) is lost.
/// Keep the [`Rate`] (or use [`Rate::as_decimal`]) when full precision matters.
impl From<Rate> for Bps {
    fn from(rate: Rate) -> Self {
        Bps::new(rate.as_bps())
    }
}

impl From<Rate> for Percentage {
    fn from(rate: Rate) -> Self {
        Percentage::new(rate.as_percent())
    }
}

impl From<Bps> for Rate {
    fn from(bps: Bps) -> Self {
        Rate::from_bps(bps.as_bps())
    }
}

impl From<Bps> for Percentage {
    fn from(bps: Bps) -> Self {
        Percentage::new(bps.as_percent())
    }
}

impl From<Percentage> for Rate {
    fn from(pct: Percentage) -> Self {
        Rate::from_percent(pct.as_percent())
    }
}

/// Converts to whole basis points, **rounding** to the nearest bp — `Bps` is an
/// integer type, so sub-bp precision is lost.
impl From<Percentage> for Bps {
    fn from(pct: Percentage) -> Self {
        Bps::new(pct.as_bps())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_creation_and_conversion() {
        let rate = Rate::from_percent(2.5);
        assert_eq!(rate.as_decimal(), 0.025);
        assert_eq!(rate.as_percent(), 2.5);
        assert_eq!(rate.as_bps(), 250);

        let rate2 = Rate::from_bps(250);
        assert_eq!(rate, rate2);

        let rate3 = Rate::from_decimal(0.025);
        assert_eq!(rate, rate3);
    }

    #[test]
    fn rate_arithmetic() {
        let rate1 = Rate::from_percent(2.0);
        let rate2 = Rate::from_percent(1.5);

        // Use approximate equality for floating point comparisons
        let eps = 1e-15;

        let result_add = rate1 + rate2;
        let expected_add = Rate::from_percent(3.5);
        assert!((result_add.as_decimal() - expected_add.as_decimal()).abs() < eps);

        let result_sub = rate1 - rate2;
        let expected_sub = Rate::from_percent(0.5);
        assert!((result_sub.as_decimal() - expected_sub.as_decimal()).abs() < eps);

        let result_mul = rate1 * 2.0;
        let expected_mul = Rate::from_percent(4.0);
        assert!((result_mul.as_decimal() - expected_mul.as_decimal()).abs() < eps);

        let result_div = rate1.checked_div(2.0).unwrap();
        let expected_div = Rate::from_percent(1.0);
        assert!((result_div.as_decimal() - expected_div.as_decimal()).abs() < eps);

        let result_neg = -rate1;
        let expected_neg = Rate::from_percent(-2.0);
        assert!((result_neg.as_decimal() - expected_neg.as_decimal()).abs() < eps);
    }

    #[test]
    fn bps_creation_and_conversion() {
        let bps = Bps::new(250);
        assert_eq!(bps.as_bps(), 250);
        assert_eq!(bps.as_decimal(), 0.025);
        assert_eq!(bps.as_percent(), 2.5);
    }

    #[test]
    fn bps_try_new_and_try_from_reject_non_finite_values() {
        assert_eq!(Bps::try_new(12.4).expect("finite bps").as_bps(), 12);
        assert_eq!(Bps::try_from(12.6).expect("finite bps").as_bps(), 13);

        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(Bps::try_new(bad).is_err(), "try_new must reject {bad}");
            assert!(Bps::try_from(bad).is_err(), "TryFrom must reject {bad}");
        }
    }

    #[test]
    fn percentage_creation_and_conversion() {
        let pct = Percentage::new(12.5);
        assert_eq!(pct.as_percent(), 12.5);
        assert_eq!(pct.as_decimal(), 0.125);
        assert_eq!(pct.as_bps(), 1250);
    }

    #[test]
    fn cross_conversions() {
        let rate = Rate::from_percent(2.5);
        let bps: Bps = rate.into();
        let pct: Percentage = rate.into();

        assert_eq!(bps.as_bps(), 250);
        assert_eq!(pct.as_percent(), 2.5);

        let rate_from_bps: Rate = bps.into();
        let rate_from_pct: Rate = pct.into();

        assert_eq!(rate, rate_from_bps);
        assert_eq!(rate, rate_from_pct);
    }

    #[test]
    fn constants_and_predicates() {
        assert!(Rate::ZERO.is_zero());
        assert!(!Rate::ZERO.is_positive());
        assert!(!Rate::ZERO.is_negative());

        let positive = Rate::from_percent(1.0);
        assert!(positive.is_positive());
        assert!(!positive.is_negative());

        let negative = Rate::from_percent(-1.0);
        assert!(negative.is_negative());
        assert!(!negative.is_positive());

        assert_eq!(negative.abs(), positive);
    }

    #[test]
    fn serde_round_trip() {
        let rate = Rate::from_percent(2.5);
        let json = serde_json::to_string(&rate).expect("JSON serialization should succeed in test");
        let deserialized: Rate =
            serde_json::from_str(&json).expect("JSON deserialization should succeed in test");
        assert_eq!(rate, deserialized);

        let bps = Bps::new(250);
        let json = serde_json::to_string(&bps).expect("JSON serialization should succeed in test");
        let deserialized: Bps =
            serde_json::from_str(&json).expect("JSON deserialization should succeed in test");
        assert_eq!(bps, deserialized);

        let pct = Percentage::new(12.5);
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
        assert_eq!(rate.expect("finite rate"), Rate::from_decimal(0.05));
    }
}
