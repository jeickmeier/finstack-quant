//! Money type, conversions, formatting, and arithmetic operations.
//!
//! [`Money`] stores amounts as decimals to preserve decimal precision while
//! retaining ergonomic APIs for arithmetic and formatting.
//! Instances retain their [`Currency`] tag and refuse to mix currencies unless
//! explicitly converted via [`super::fx::FxProvider`].
//!
//! Note: Formatting is intentionally non-locale. Separators are ASCII and
//! currency code precedes the amount (e.g., "USD 1,234.56"). Use
//! [`Money::format_with`] or wrap at the UI layer if locale-aware
//! presentation is required; the numeric representation remains deterministic
//! and stable for pipelines.
//!
//! # Examples
//! ```rust
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::currency::Currency;
//!
//! let amt = Money::from((100_i64, Currency::USD));
//! assert_eq!(amt.currency(), Currency::USD);
//! assert_eq!(format!("{}", amt), "USD 100.00");
//! ```

use crate::config::{FinstackConfig, RoundingMode};
use crate::currency::Currency;
use crate::decimal::f64_to_decimal;
use crate::{Error, InputError};
use core::fmt;

use super::rounding::{repr_add, repr_div_f64, repr_mul_f64, repr_sub, try_round_f64};

/// Format an integer string (optionally prefixed by `-`) with thousands
/// separator `sep`.
fn group_thousands(int_str: &str, sep: char) -> String {
    let (is_neg, digits) = match int_str.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, int_str),
    };

    let bytes = digits.as_bytes();
    let mut rev: Vec<char> = Vec::with_capacity(bytes.len() + bytes.len() / 3 + 1);
    for (i, &b) in bytes.iter().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            rev.push(sep);
        }
        rev.push(b as char);
    }
    rev.reverse();

    let mut out = String::with_capacity(rev.len() + usize::from(is_neg));
    if is_neg {
        out.push('-');
    }
    for c in rev {
        out.push(c);
    }
    out
}

/// Formatting options for [`Money::format_with`].
///
/// Configuration-aware formatting and Display delegate to this canonical formatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatOpts {
    /// Number of fractional digits. `None` means "use currency default" (from
    /// [`Currency::decimals`]).
    decimals: Option<usize>,
    /// Whether to prepend the ISO-4217 currency code.
    show_currency: bool,
    /// Optional thousands separator (e.g., `Some(',')`). `None` disables grouping.
    group: Option<char>,
    /// Rounding mode for the displayed value.
    rounding: RoundingMode,
}

impl Default for FormatOpts {
    /// Defaults, matching `Display`: currency minor-unit decimals, currency
    /// code shown, no grouping, Bankers rounding.
    fn default() -> Self {
        Self {
            decimals: None,
            show_currency: true,
            group: None,
            rounding: RoundingMode::Bankers,
        }
    }
}

impl FormatOpts {
    /// Maximum supported display precision.
    ///
    /// Host bindings accept precision from untyped inputs, so the bound caps
    /// the worst-case formatted string length before rounding work begins.
    pub const MAX_DECIMALS: usize = 1_000_000;

    /// Build checked formatting options, rejecting an excessive precision bound.
    ///
    /// # Arguments
    ///
    /// * `decimals` - Fractional digits to display; `None` selects the
    ///   currency's ISO-4217 minor units. Must lie in `0..=Self::MAX_DECIMALS`.
    /// * `show_currency` - Whether to prepend the ISO-4217 currency code;
    ///   `None` selects `true`.
    /// * `group` - Optional single-character thousands separator (e.g.
    ///   `Some(',')`); `None` disables grouping.
    /// * `rounding` - Rounding mode applied to the displayed value; `None`
    ///   selects [`RoundingMode::default`] (bankers).
    ///
    /// # Returns
    ///
    /// A validated [`FormatOpts`] carrying the supplied display options.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when `decimals` exceeds
    /// [`Self::MAX_DECIMALS`] or `group` is not exactly one character.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::money::FormatOpts;
    /// use finstack_quant_core::config::RoundingMode;
    ///
    /// let opts = FormatOpts::new(Some(2), Some(true), Some(","), Some(RoundingMode::Bankers))
    ///     .expect("bounded precision");
    /// # let _ = opts;
    /// assert!(FormatOpts::new(None, None, Some(", "), None).is_err());
    /// ```
    pub fn new(
        decimals: Option<usize>,
        show_currency: Option<bool>,
        group: Option<&str>,
        rounding: Option<RoundingMode>,
    ) -> Result<Self, Error> {
        let show_currency = show_currency.unwrap_or(true);
        let rounding = rounding.unwrap_or_default();
        if decimals.is_some_and(|precision| precision > Self::MAX_DECIMALS) {
            return Err(Error::Validation(format!(
                "formatting precision exceeds {}",
                Self::MAX_DECIMALS
            )));
        }
        let group = match group {
            None => None,
            Some(separator) => {
                let mut chars = separator.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => Some(c),
                    _ => {
                        return Err(Error::Validation(
                            "group must be a single character such as ','".to_string(),
                        ))
                    }
                }
            }
        };
        Ok(Self {
            decimals,
            show_currency,
            group,
            rounding,
        })
    }
}

/// Currency-tagged monetary amount with safe arithmetic.
///
/// Values retain decimal precision independently of ISO 4217 display precision.
///
/// When you need configurable rounding during ingestion, use
/// [`Money::new_with_config`].
///
/// # Examples
/// ```rust
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::currency::Currency;
///
/// let notional = Money::from((1_000_000_i64, Currency::EUR));
/// assert_eq!(notional.currency(), Currency::EUR);
/// assert_eq!(notional.amount(), 1_000_000.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Money {
    /// Monetary amount, carried on the wire as an exact decimal string rather
    /// than a JSON number so no precision is lost in transit. Construction with
    /// configuration applies the selected ingest scale; raw construction does not.
    #[serde(with = "crate::wire::decimal")]
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DecimalWire"))]
    amount: rust_decimal::Decimal,
    /// ISO 4217 currency of `amount`. Arithmetic between two `Money` values
    /// requires this to match; there is no implicit conversion.
    currency: Currency,
}

impl Money {
    /// Format an amount with explicit precision, currency display, grouping, and rounding.
    ///
    /// # Arguments
    ///
    /// * `opts` - Display precision (currency minor units when omitted), currency-prefix flag, optional thousands separator, and rounding mode; does not change the stored amount.
    pub fn format_with(&self, opts: FormatOpts) -> String {
        use super::rounding::round_decimal;
        let dp = opts
            .decimals
            .unwrap_or_else(|| usize::from(self.currency.decimals()));
        let rounded = round_decimal(self.amount, dp as i32, opts.rounding);
        let raw = format!("{val:.prec$}", val = rounded, prec = dp);
        let value = match opts.group {
            Some(sep) => {
                let (int_part, frac_part) = match raw.split_once('.') {
                    Some((i, f)) => (i, Some(f)),
                    None => (raw.as_str(), None),
                };
                let int_fmt = group_thousands(int_part, sep);
                match frac_part {
                    Some(frac) => format!("{int_fmt}.{frac}"),
                    None => int_fmt,
                }
            }
            None => raw,
        };
        if opts.show_currency {
            format!("{} {}", self.currency(), value)
        } else {
            value
        }
    }

    /// Construct from a finite f64 without ingest rounding.
    ///
    /// Ingests the `f64` through its shortest round-trippable Decimal form,
    /// preserving its Decimal precision. Use
    /// [`Self::from_decimal`] when the caller already holds an exact Decimal.
    ///
    /// # Errors
    ///
    /// Returns `InputError::NonFiniteValue` for NaN or infinity, or
    /// `InputError::ConversionOverflow` when the finite amount cannot be
    /// represented by the internal Decimal type.
    ///
    /// # Arguments
    ///
    /// * `amount` - Finite monetary quantity in major currency units before rounding
    /// * `currency` - ISO-4217 currency that defines scale, rounding, and display units
    pub fn new(amount: f64, currency: Currency) -> Result<Self, Error> {
        Self::try_new_impl(amount, currency, None)
    }

    /// An exact zero amount in `currency`.
    ///
    /// # Arguments
    ///
    /// * `currency` - ISO-4217 currency tag of the zero amount.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    ///
    /// let zero = Money::zero(Currency::USD);
    /// assert_eq!(zero.amount(), 0.0);
    /// assert_eq!(zero.currency(), Currency::USD);
    /// ```
    #[must_use]
    pub fn zero(currency: Currency) -> Self {
        Self::from_decimal(rust_decimal::Decimal::ZERO, currency)
    }

    /// Construct from a `Decimal` amount, preserving full precision.
    ///
    /// Unlike [`Money::new`], this constructor never goes
    /// through `f64`, so it is the preferred entry point when the caller already
    /// has a high-precision value (e.g., a Python `decimal.Decimal` rendered to
    /// a string and parsed via `Decimal::from_str`). Use this for hedge-fund
    /// notionals where IEEE 754 rounding of the input would be unacceptable.
    ///
    /// `rust_decimal::Decimal` already rejects `NaN` / `Infinity` at parse
    /// time, so callers parsing user-supplied strings will surface those errors
    /// before reaching this constructor. Every `Decimal` has a finite `f64`
    /// view for [`Self::amount`], so construction cannot fail.
    ///
    /// # Arguments
    ///
    /// * `amount` - Finite monetary quantity in major currency units before rounding
    /// * `currency` - ISO-4217 currency that defines scale, rounding, and display units
    #[must_use]
    pub fn from_decimal(amount: rust_decimal::Decimal, currency: Currency) -> Self {
        Self { amount, currency }
    }

    /// Construct from exact decimal text, rejecting any inexact representation.
    ///
    /// `amount` is parsed directly as a `rust_decimal::Decimal` — fixed-point
    /// text such as `"1234.56"` or scientific text such as `"1.2345e3"` —
    /// without converting through `f64`. The text must be exactly
    /// representable within Decimal's 96-bit mantissa and its maximum
    /// fractional scale of 28 digits; values needing more precision are
    /// rejected rather than silently rounded. For scientific input the
    /// mantissa itself is validated with `Decimal::from_str_exact` and the
    /// exponent is applied exactly on the integer coefficient and scale:
    /// excessive target scales are reduced only by removing trailing
    /// coefficient zeroes, so no nonzero digit is ever discarded, and
    /// results beyond the 96-bit mantissa or 28-digit scale are rejected.
    ///
    /// The currency is a display/arithmetic tag only: no FX conversion is
    /// performed. This constructor bypasses the general `Money` serde path,
    /// which tolerates lossy re-rendering of over-precise amounts.
    ///
    /// # Arguments
    ///
    /// * `amount` - Exact decimal amount text in major currency units, in
    ///   fixed-point or scientific notation.
    /// * `currency` - ISO-4217 currency tag stored with the amount.
    ///
    /// # Returns
    ///
    /// A `Money` whose stored Decimal equals the supplied text exactly.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when the text cannot be represented
    /// exactly as a `Decimal` (malformed input, non-finite tokens, mantissa
    /// precision beyond 96 bits, a malformed exponent, or an exponent whose
    /// application would exceed the 28-digit scale or 96-bit mantissa).
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::currency::Currency;
    ///
    /// let money = Money::from_decimal_str("1.245", Currency::USD).expect("exact amount");
    /// assert_eq!(money.amount_decimal().to_string(), "1.245");
    /// ```
    pub fn from_decimal_str(amount: &str, currency: Currency) -> Result<Self, Error> {
        Ok(Self::from_decimal(
            crate::decimal::parse_decimal(amount)?,
            currency,
        ))
    }

    /// Construct from an exact Decimal using the configured ingest rounding policy.
    ///
    /// # Arguments
    ///
    /// * `amount` - Exact monetary amount in major currency units, rounded without conversion through f64.
    /// * `currency` - ISO-4217 currency used to select the ingest scale.
    /// * `cfg` - Configuration supplying the currency scale override (or max(6, ISO minor-unit digits)) and rounding mode.
    #[must_use]
    pub fn from_decimal_with_config(
        amount: rust_decimal::Decimal,
        currency: Currency,
        cfg: &FinstackConfig,
    ) -> Self {
        let scale = cfg.ingest_scale(currency) as i32;
        Self::from_decimal(
            super::rounding::round_decimal(amount, scale, cfg.rounding.mode),
            currency,
        )
    }

    /// Fallible constructor using an explicit configuration for rounding.
    ///
    /// Uses `cfg` to select the ingest scale and rounding mode for `currency`.
    /// This is the configuration-aware counterpart to [`Self::new`].
    ///
    /// # Errors
    ///
    /// Returns `InputError::NonFiniteValue` for NaN or infinity, or
    /// `InputError::ConversionOverflow` when rounding or conversion cannot
    /// produce a representable Decimal amount.
    ///
    /// # Arguments
    ///
    /// * `amount` - Finite monetary quantity in major currency units before rounding
    /// * `currency` - ISO-4217 currency that defines scale, rounding, and display units
    /// * `cfg` - Finstack configuration controlling rounding policy and money tolerances
    pub fn new_with_config(
        amount: f64,
        currency: Currency,
        cfg: &FinstackConfig,
    ) -> Result<Self, Error> {
        Self::try_new_impl(amount, currency, Some(cfg))
    }

    #[inline]
    fn try_new_impl(
        amount: f64,
        currency: Currency,
        cfg: Option<&FinstackConfig>,
    ) -> Result<Self, Error> {
        // IEEE negative zero would become the Decimal "-0", which serializes
        // as "-0" and re-parses as "0"; normalize so wire round trips are
        // identities. `f64_to_decimal` rejects NaN / ±Inf and out-of-range
        // magnitudes, and uses the shortest round-trip conversion so `0.1_f64`
        // ingests as `0.1`, not the 28-digit IEEE expansion.
        let amount = if amount == 0.0 { 0.0 } else { amount };
        let rounded = if let Some(cfg) = cfg {
            try_round_f64(amount, cfg.ingest_scale(currency) as i32, cfg.rounding.mode)?
        } else {
            f64_to_decimal(amount)?
        };
        Ok(Self {
            amount: rounded,
            currency,
        })
    }

    /// Amount accessor (by value).
    ///
    /// This converts the internal [`Decimal`](rust_decimal::Decimal) representation to `f64` for
    /// ergonomic display and binding use. Values outside the finite `f64`
    /// range should avoid this lossy view and keep values on Decimal-backed APIs.
    #[inline]
    pub fn amount(&self) -> f64 {
        self.amount.as_f64()
    }

    /// Lossless amount accessor as the internal [`rust_decimal::Decimal`].
    ///
    /// Unlike [`Money::amount`], no `Decimal -> f64` conversion happens, so
    /// the full accounting-grade precision of the stored amount is preserved.
    /// Prefer this accessor when feeding downstream Decimal-based pipelines
    /// (e.g. Python `decimal.Decimal`, database fixed-point columns).
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::currency::Currency;
    ///
    /// let m = Money::new(1234.56, Currency::USD).expect("valid money fixture");
    /// assert_eq!(m.amount_decimal().to_string(), "1234.56");
    /// ```
    #[inline]
    pub const fn amount_decimal(&self) -> rust_decimal::Decimal {
        self.amount
    }

    /// Currency accessor.
    #[inline]
    pub const fn currency(&self) -> Currency {
        self.currency
    }

    /// Compare exact Decimal amounts after requiring the same currency.
    ///
    /// No conversion through floating point or FX conversion is performed.
    ///
    /// # Arguments
    ///
    /// * `rhs` - Money in the same currency to compare with this amount.
    ///
    /// # Errors
    ///
    /// Returns `Error::CurrencyMismatch` when the currencies differ.
    #[inline]
    pub fn checked_cmp(self, rhs: Self) -> Result<core::cmp::Ordering, Error> {
        ensure_same_currency(&self, &rhs)?;
        Ok(self.amount.cmp(&rhs.amount))
    }

    // Checked arithmetic

    /// Add two amounts, returning an error when currencies do not match.
    ///
    /// `Money` intentionally does **not** implement [`std::ops::Add`]: cross-currency
    /// addition must be fallible, so the currency-checked sum is exposed through this
    /// method rather than a `+` operator that could silently mismatch currencies.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::currency::Currency;
    ///
    /// let lhs = Money::from((50_i64, Currency::USD));
    /// let rhs = Money::from((25_i64, Currency::USD));
    ///
    /// let sum = lhs.checked_add(rhs).expect("Currency match should succeed");
    /// assert_eq!(sum.amount(), 75.0);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `Error::CurrencyMismatch` when the currencies differ, or
    /// `InputError::ConversionOverflow` when the Decimal sum is out of range.
    ///
    /// # Arguments
    ///
    /// * `rhs` - Right-hand operand combined with this value under the documented arithmetic rules.
    #[must_use = "returns new Money if currencies match"]
    #[inline]
    pub fn checked_add(self, rhs: Self) -> Result<Self, Error> {
        ensure_same_currency(&self, &rhs)?;
        Ok(Self {
            amount: repr_add(self.amount, rhs.amount)?,
            currency: self.currency,
        })
    }

    /// Subtract two amounts, returning an error when currencies do not match.
    ///
    /// `Money` intentionally does **not** implement [`std::ops::Sub`]: cross-currency
    /// subtraction must be fallible, so the currency-checked difference is exposed
    /// through this method rather than a `-` operator that could silently mismatch
    /// currencies.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::currency::Currency;
    ///
    /// let lhs = Money::from((50_i64, Currency::USD));
    /// let rhs = Money::from((25_i64, Currency::USD));
    ///
    /// let diff = lhs.checked_sub(rhs).expect("Currency match should succeed");
    /// assert_eq!(diff.amount(), 25.0);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `Error::CurrencyMismatch` when the currencies differ, or
    /// `InputError::ConversionOverflow` when the Decimal difference is out of
    /// range.
    ///
    /// # Arguments
    ///
    /// * `rhs` - Right-hand operand combined with this value under the documented arithmetic rules.
    #[must_use = "returns new Money if currencies match"]
    #[inline]
    pub fn checked_sub(self, rhs: Self) -> Result<Self, Error> {
        ensure_same_currency(&self, &rhs)?;
        Ok(Self {
            amount: repr_sub(self.amount, rhs.amount)?,
            currency: self.currency,
        })
    }

    /// Multiply by an `f64` scalar, returning an error on non-finite or
    /// non-representable values instead of panicking.
    ///
    /// Prefer this over the `*` operator when the scalar may come from
    /// untrusted or computed input.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::currency::Currency;
    /// # fn main() -> finstack_quant_core::Result<()> {
    ///
    /// let m = Money::from((100_i64, Currency::USD));
    /// let doubled = m.checked_mul_f64(2.0)?;
    /// assert_eq!(doubled.amount(), 200.0);
    ///
    /// assert!(m.checked_mul_f64(f64::NAN).is_err());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `InputError::NonFiniteValue` for a NaN or infinite scalar, or
    /// `InputError::ConversionOverflow` when the scalar or product is outside
    /// the Decimal representation range.
    ///
    /// # Arguments
    ///
    /// * `rhs` - Right-hand operand combined with this value under the documented arithmetic rules.
    #[must_use = "returns new Money on success"]
    #[inline]
    pub fn checked_mul_f64(self, rhs: f64) -> Result<Self, Error> {
        Ok(Self {
            amount: repr_mul_f64(self.amount, rhs)?,
            currency: self.currency,
        })
    }

    /// Negate the amount, preserving the currency.
    ///
    /// Negation is exact on the internal `Decimal` representation (it never
    /// round-trips through `f64`) and cannot overflow for any value that was
    /// already constructed, so this method is infallible.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::currency::Currency;
    ///
    /// let m = Money::from((100_i64, Currency::USD));
    /// assert_eq!(m.checked_neg().amount(), -100.0);
    /// assert_eq!(m.checked_neg().checked_neg(), m);
    /// ```
    #[must_use = "returns the negated Money"]
    #[inline]
    pub fn checked_neg(self) -> Self {
        Self {
            amount: -self.amount,
            currency: self.currency,
        }
    }

    /// Divide by an `f64` scalar, returning an error on non-finite, zero, or
    /// non-representable values instead of panicking.
    ///
    /// Prefer this over the `/` operator when the scalar may come from
    /// untrusted or computed input.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::currency::Currency;
    /// # fn main() -> finstack_quant_core::Result<()> {
    ///
    /// let m = Money::from((100_i64, Currency::USD));
    /// let half = m.checked_div_f64(2.0)?;
    /// assert_eq!(half.amount(), 50.0);
    ///
    /// assert!(m.checked_div_f64(0.0).is_err());
    /// assert!(m.checked_div_f64(f64::INFINITY).is_err());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation("division by zero")` for a zero scalar
    /// (the same error [`Self::checked_div`] reports for a zero `Money`
    /// divisor), `InputError::NonFiniteValue` for NaN or infinity, or
    /// `InputError::ConversionOverflow` when the quotient is out of range.
    ///
    /// # Arguments
    ///
    /// * `rhs` - Right-hand operand combined with this value under the documented arithmetic rules.
    #[must_use = "returns new Money on success"]
    #[inline]
    pub fn checked_div_f64(self, rhs: f64) -> Result<Self, Error> {
        Ok(Self {
            amount: repr_div_f64(self.amount, rhs)?,
            currency: self.currency,
        })
    }

    /// Divide by another same-currency amount, returning a dimensionless f64 ratio.
    ///
    /// # Arguments
    ///
    /// * `rhs` - Nonzero divisor in the same currency; no FX conversion is performed.
    ///
    /// # Errors
    ///
    /// Returns a currency mismatch, division-by-zero, or conversion-overflow error.
    pub fn checked_div(self, rhs: Self) -> Result<f64, Error> {
        ensure_same_currency(&self, &rhs)?;
        if rhs.amount.is_zero() {
            return Err(Error::Validation("division by zero".into()));
        }
        self.amount
            .checked_div(rhs.amount)
            .map(|ratio| ratio.as_f64())
            .ok_or(Error::Input(InputError::ConversionOverflow))
    }

    /// Return the absolute amount in the same currency without rounding.
    pub fn abs(self) -> Self {
        Self {
            amount: self.amount.abs(),
            currency: self.currency,
        }
    }

    /// Bankers-round the exact amount while preserving its currency.
    ///
    /// # Arguments
    ///
    /// * `ndigits` - Nonnegative number of decimal places in major currency units; None selects the currency's ISO minor units. Precision above Decimal's capacity leaves the value unchanged.
    ///
    /// # Errors
    ///
    /// Returns a validation error when ndigits is negative.
    pub fn round(self, ndigits: Option<i32>) -> Result<Self, Error> {
        let dp = ndigits.unwrap_or_else(|| i32::from(self.currency.decimals()));
        if dp < 0 {
            return Err(Error::Validation("round(Money, n) requires n >= 0".into()));
        }
        Ok(Self {
            amount: super::rounding::round_decimal(self.amount, dp, RoundingMode::Bankers),
            currency: self.currency,
        })
    }

    /// Convert this amount using an already-resolved FX rate.
    ///
    /// The multiplication remains Decimal-backed and therefore preserves the
    /// stored amount's precision. The supplied rate must be finite and strictly
    /// positive. A same-currency conversion returns the input unchanged.
    ///
    /// # Errors
    ///
    /// Returns `InputError::InvalidFxRate` when `rate` is non-finite or not
    /// strictly positive, or `InputError::ConversionOverflow` when the
    /// converted amount cannot be represented as a Decimal.
    pub fn convert_at_rate(self, to: Currency, rate: f64) -> crate::Result<Self> {
        if self.currency == to {
            return Ok(self);
        }
        let rate = crate::money::fx::validate_fx_rate(self.currency, to, rate)?;
        // Retain full Decimal precision, consistent with `Money::new`. Rounding
        // to the destination minor units here would truncate sub-unit precision
        // mid-calculation, so the result of a chained computation would depend
        // on *where* a `convert` was inserted and could break serial≡parallel
        // determinism. Apply minor-unit rounding only at the reporting boundary
        // (e.g. via `format`), not on every conversion.
        let new_amount = repr_mul_f64(self.amount, rate)?;
        Ok(Self {
            amount: new_amount,
            currency: to,
        })
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Default formatting uses ISO-4217 minor units and bankers rounding.
        // Route through the canonical formatter so `Display` always agrees with
        // `format(currency_decimals, true)` (a raw `{:.prec$}` on the Decimal
        // would truncate instead of round).
        f.write_str(&self.format_with(FormatOpts::default()))
    }
}

// Generic tuple conversions for common numeric primitives.
//
// Integer conversions route through `Decimal::from`, which is exact for the
// full i64/u64 range; casting through `as f64` would silently lose precision
// above 2^53 .
macro_rules! from_integer_tuple {
    ($($t:ty),+) => { $(
        impl From<($t, Currency)> for Money {
            #[inline]
            fn from(value: ($t, Currency)) -> Self {
                Self {
                    amount: rust_decimal::Decimal::from(value.0),
                    currency: value.1,
                }
            }
        }
    )+ };
}

from_integer_tuple!(i64, u64);

/// Ensure two `Money` values share the same currency.
#[inline]
fn ensure_same_currency(lhs: &Money, rhs: &Money) -> Result<(), Error> {
    if lhs.currency != rhs.currency {
        return Err(Error::CurrencyMismatch {
            expected: lhs.currency,
            actual: rhs.currency,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation_and_accessors() {
        let m = Money::from((100_i64, Currency::USD));
        assert_eq!(m.amount(), 100.0);
        assert_eq!(m.currency(), Currency::USD);
    }

    #[test]
    fn checked_comparison_is_exact_and_requires_matching_currencies() {
        use core::cmp::Ordering;
        let lower = Money::from((9_007_199_254_740_992_u64, Currency::USD));
        let higher = Money::from((9_007_199_254_740_993_u64, Currency::USD));
        assert_eq!(lower.amount(), higher.amount());
        assert_eq!(lower.checked_cmp(higher), Ok(Ordering::Less));
        assert_eq!(higher.checked_cmp(lower), Ok(Ordering::Greater));
        assert_eq!(lower.checked_cmp(lower), Ok(Ordering::Equal));
        assert!(matches!(
            lower.checked_cmp(Money::from((1_i64, Currency::EUR))),
            Err(Error::CurrencyMismatch {
                expected: Currency::USD,
                actual: Currency::EUR
            })
        ));
    }

    #[test]
    fn checked_ops() {
        let a = Money::from((50_i64, Currency::USD));
        let b = Money::from((25_i64, Currency::USD));
        let c = a
            .checked_add(b)
            .expect("Currency match should succeed in test");
        assert_eq!(c.amount(), 75.0);
    }

    #[test]
    fn currency_mismatch_error() {
        let usd = Money::from((10_i64, Currency::USD));
        let eur = Money::from((10_i64, Currency::EUR));
        assert!(usd.checked_add(eur).is_err());
    }

    #[test]
    fn tuple_from_conversions() {
        use core::convert::Into;
        let m1: Money = (100_i64, Currency::USD).into();
        assert_eq!(m1.amount(), 100.0);
        let m2: Money = (42_u64, Currency::EUR).into();
        assert_eq!(m2.amount(), 42.0);
    }

    #[test]
    fn grouped_format_handles_negative_values() {
        let m = Money::new(-1234.56, Currency::USD).expect("valid money fixture");
        let formatted = m.format_with(FormatOpts {
            group: Some(','),
            ..FormatOpts::default()
        });
        assert!(
            formatted.starts_with("USD -1,234.56"),
            "formatted output should keep sign on integer part only: {}",
            formatted
        );
    }

    #[test]
    fn new_rejects_non_finite_amounts() {
        assert!(Money::new(f64::NAN, Currency::USD).is_err());
    }

    #[test]
    fn checked_mul_div_f64_accepts_finite_scalars() {
        let money = Money::from((10_i64, Currency::USD));

        assert_eq!(
            money
                .checked_mul_f64(2.5)
                .expect("finite multiplication")
                .amount(),
            25.0
        );
        assert_eq!(
            money
                .checked_div_f64(4.0)
                .expect("finite division")
                .amount(),
            2.5
        );
    }

    #[test]
    fn checked_mul_div_f64_reject_invalid_scalars() {
        let money = Money::from((10_i64, Currency::USD));

        assert!(money.checked_mul_f64(f64::NAN).is_err());
        assert!(money.checked_mul_f64(f64::INFINITY).is_err());
        assert!(money.checked_div_f64(f64::NEG_INFINITY).is_err());
        // A zero scalar and a zero `Money` divisor report the same error.
        let zero = Money::from((0_i64, Currency::USD));
        let division_by_zero = Err(Error::Validation("division by zero".into()));
        assert_eq!(money.checked_div_f64(0.0).map(|_| ()), division_by_zero);
        assert_eq!(money.checked_div_f64(-0.0).map(|_| ()), division_by_zero);
        assert_eq!(money.checked_div(zero).map(|_| ()), division_by_zero);
    }

    #[test]
    fn checked_neg_is_exact_and_keeps_scale() {
        let exact = Money::from_decimal_str("1.2500", Currency::USD).expect("exact");
        let negated = exact.checked_neg();
        assert_eq!(negated.amount_decimal().to_string(), "-1.2500");
        assert_eq!(negated.checked_neg(), exact);
        let tiny = Money::from_decimal_str("1e-27", Currency::USD).expect("exact");
        assert_eq!(tiny.checked_neg().amount_decimal(), -tiny.amount_decimal());
    }

    #[test]
    fn money_deserialize_rejects_unknown_fields() {
        let json = r#"{"amount":10.0,"currency":"USD","unexpected":true}"#;
        let result: std::result::Result<Money, _> = serde_json::from_str(json);

        assert!(result.is_err());
    }

    #[test]
    fn money_json_field_names_are_stable() {
        let json =
            serde_json::to_value(Money::new(10.25, Currency::USD).expect("valid money fixture"))
                .expect("serializes");
        let object = json.as_object().expect("money serializes as an object");

        assert_eq!(object.len(), 2);
        assert!(object.contains_key("amount"));
        assert_eq!(object.get("currency"), Some(&serde_json::json!("USD")));
    }

    #[test]
    fn convert_rejects_non_finite_rate() {
        let usd = Money::from((5_i64, Currency::USD));
        assert!(usd.convert_at_rate(Currency::EUR, f64::NAN).is_err());
    }

    #[test]
    fn amount_does_not_silently_return_zero_for_large_values() {
        // This test documents the fix: large values must NOT silently become 0.
        // Prior to the fix, conversion failure would return 0.0 silently.
        let large_amount = Money::from((1_000_000_000_000_i64, Currency::USD));
        let amount = large_amount.amount();
        assert!(
            amount > 0.0,
            "Large monetary amount must not silently become zero"
        );
        assert!(
            (amount - 1_000_000_000_000.0).abs() < 1e3,
            "Amount should preserve the large value"
        );
    }

    // Fallible constructor tests (Money::new / Money::new_with_config)

    #[test]
    fn try_new_succeeds_for_finite_values() {
        let m = Money::new(123.45, Currency::USD).expect("Finite value should succeed");
        assert!((m.amount() - 123.45).abs() < 1e-10);
        assert_eq!(m.currency(), Currency::USD);
    }

    #[test]
    fn try_new_returns_error_for_nan() {
        let result = Money::new(f64::NAN, Currency::USD);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(crate::Error::Input(
                crate::InputError::NonFiniteValue { kind }
            )) if kind == crate::NonFiniteKind::NaN
        ));
    }

    #[test]
    fn try_new_returns_error_for_positive_infinity() {
        let result = Money::new(f64::INFINITY, Currency::EUR);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(crate::Error::Input(
                crate::InputError::NonFiniteValue { kind }
            )) if kind == crate::NonFiniteKind::PosInfinity
        ));
    }

    #[test]
    fn try_new_returns_error_for_negative_infinity() {
        let result = Money::new(f64::NEG_INFINITY, Currency::GBP);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(crate::Error::Input(
                crate::InputError::NonFiniteValue { kind }
            )) if kind == crate::NonFiniteKind::NegInfinity
        ));
    }

    #[test]
    fn try_new_with_config_succeeds_for_finite_values() {
        let mut cfg = FinstackConfig::default();
        cfg.rounding
            .ingest_scale
            .set_scale(Currency::USD, 3)
            .expect("valid decimal scale");
        let m = Money::new_with_config(1.2345, Currency::USD, &cfg).expect("Finite should succeed");
        assert!((m.amount() - 1.234).abs() < 1e-9);
    }

    #[test]
    fn try_new_preserves_internal_precision_by_default() {
        let m = Money::new(10.005, Currency::USD).expect("Finite should succeed");
        assert!((m.amount() - 10.005).abs() < 1e-12);
    }

    #[test]
    fn try_new_with_config_honors_ingest_scale_override() {
        let mut cfg = FinstackConfig::default();
        cfg.rounding
            .ingest_scale
            .set_scale(Currency::USD, 2)
            .expect("valid decimal scale");
        let m = Money::new_with_config(10.999, Currency::USD, &cfg).expect("Finite should succeed");
        assert!((m.amount() - 11.00).abs() < 1e-12);
    }

    #[test]
    fn try_new_with_config_returns_error_for_non_finite() {
        let cfg = FinstackConfig::default();
        let result = Money::new_with_config(f64::NAN, Currency::USD, &cfg);
        assert!(result.is_err());
    }

    #[test]
    fn try_new_handles_zero() {
        let m = Money::new(0.0, Currency::USD).expect("Zero should succeed");
        assert_eq!(m.amount(), 0.0);
    }

    #[test]
    fn try_new_handles_negative_zero() {
        let m = Money::new(-0.0, Currency::USD).expect("Negative zero should succeed");
        // -0.0 == 0.0 in floating point
        assert_eq!(m.amount(), 0.0);
    }

    #[test]
    fn try_new_handles_very_small_values() {
        let small = 1e-15;
        let m = Money::new(small, Currency::USD).expect("Small value should succeed");
        // Construction preserves the raw finite amount; formatting/rounding is a separate concern.
        assert_eq!(m.amount(), small);
    }

    #[test]
    fn try_new_handles_large_finite_values() {
        let large = 1e15;
        let m = Money::new(large, Currency::USD).expect("Large finite value should succeed");
        assert_eq!(m.amount(), large);
    }

    #[test]
    fn try_new_rejects_finite_values_outside_decimal_range() {
        let result = Money::new(1e100, Currency::USD);
        assert!(matches!(
            result,
            Err(crate::Error::Input(InputError::ConversionOverflow))
        ));
    }

    #[test]
    fn convert_at_rate_preserves_decimal_amount_precision() {
        use core::str::FromStr;

        let amount = rust_decimal::Decimal::from_str("10000000000000000.1").expect("valid decimal");
        let eur = Money::from_decimal(amount, Currency::EUR);
        let usd = eur
            .convert_at_rate(Currency::USD, 2.0)
            .expect("valid conversion");

        assert_eq!(
            usd.amount_decimal(),
            rust_decimal::Decimal::from_str("20000000000000000.2").expect("valid decimal")
        );
    }

    fn hash_of(money: Money) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        money.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn hash_matches_equality_across_decimal_scale() {
        let a = Money::from_decimal(rust_decimal::Decimal::new(10, 1), Currency::USD);
        let b = Money::from_decimal(rust_decimal::Decimal::new(100, 2), Currency::USD);
        assert_eq!(a, b);
        assert_eq!(hash_of(a), hash_of(b));
    }

    #[test]
    fn money_format_from_decimal_str_accepts_exact_text() {
        let wide =
            Money::from_decimal_str("12345678901234567890.12345", Currency::USD).expect("exact");
        assert_eq!(
            wide.amount_decimal().to_string(),
            "12345678901234567890.12345"
        );
        assert_eq!(wide.currency(), Currency::USD);
        let scientific =
            Money::from_decimal_str("1.2345e3", Currency::EUR).expect("exact scientific");
        assert_eq!(scientific.amount_decimal().to_string(), "1234.5");
        assert_eq!(scientific.currency(), Currency::EUR);
        let promoted = Money::from_decimal_str("0.1e29", Currency::USD).expect("exact 1e28");
        assert_eq!(
            promoted.amount_decimal(),
            rust_decimal::Decimal::try_from_i128_with_scale(10_i128.pow(28), 0)
                .expect("1e28 is representable")
        );
        let tiny = Money::from_decimal_str("1.00e-27", Currency::USD).expect("exact 1e-27");
        assert_eq!(tiny.amount_decimal(), rust_decimal::Decimal::new(1, 27));
        assert_eq!(
            tiny.format_with(
                FormatOpts::new(Some(28), Some(false), None, Some(RoundingMode::Bankers))
                    .expect("bounded opts")
            ),
            "0.0000000000000000000000000010"
        );
        let reduced = Money::from_decimal_str("2000e-31", Currency::USD).expect("exact 2e-28");
        assert_eq!(reduced.amount_decimal(), rust_decimal::Decimal::new(2, 28));
        assert_eq!(
            reduced.format_with(
                FormatOpts::new(Some(28), Some(false), None, Some(RoundingMode::Bankers))
                    .expect("bounded opts")
            ),
            "0.0000000000000000000000000002"
        );
        let min = Money::from_decimal_str("-7.9228162514264337593543950335e28", Currency::USD)
            .expect("exact Decimal minimum");
        assert_eq!(min.amount_decimal(), rust_decimal::Decimal::MIN);
        let zero = Money::from_decimal_str("0e999999", Currency::USD).expect("zero exponent");
        assert_eq!(zero.amount_decimal(), rust_decimal::Decimal::ZERO);
    }

    #[test]
    fn money_format_from_decimal_str_rejects_inexact_amounts() {
        for amount in [
            "1.24500000000000000000000000001",
            "1.23450000000000000000000000001e3",
            "NaN",
            "1e-29",
            "1e29",
            "1.23e-28",
        ] {
            let result = Money::from_decimal_str(amount, Currency::USD);
            assert!(
                matches!(result, Err(Error::Validation(_))),
                "expected validation error for {amount}: {result:?}"
            );
        }
    }

    #[test]
    fn money_format_opts_new_bounds_precision() {
        assert!(FormatOpts::new(
            Some(FormatOpts::MAX_DECIMALS),
            Some(true),
            None,
            Some(RoundingMode::Bankers)
        )
        .is_ok());
        let result = FormatOpts::new(
            Some(FormatOpts::MAX_DECIMALS + 1),
            Some(true),
            None,
            Some(RoundingMode::Bankers),
        );
        assert!(matches!(result, Err(Error::Validation(_))));
        assert!(
            FormatOpts::new(None, Some(true), Some(","), Some(RoundingMode::Bankers))
                .expect("unbounded precision")
                .decimals
                .is_none()
        );
    }

    #[test]
    fn money_format_with_honours_returned_rounding_modes() {
        let vectors: &[(&str, &str, RoundingMode, &str)] = &[
            ("1.245", "USD 1.24", RoundingMode::Bankers, "bankers"),
            ("1.255", "USD 1.26", RoundingMode::Bankers, "bankers"),
            (
                "-1.245",
                "USD -1.25",
                RoundingMode::AwayFromZero,
                "away_from_zero",
            ),
            (
                "1.241",
                "USD 1.24",
                RoundingMode::AwayFromZero,
                "away_from_zero",
            ),
            (
                "-1.259",
                "USD -1.25",
                RoundingMode::TowardZero,
                "toward_zero",
            ),
            ("-1.241", "USD -1.25", RoundingMode::Floor, "floor"),
            ("1.249", "USD 1.24", RoundingMode::Floor, "floor"),
            ("1.241", "USD 1.25", RoundingMode::Ceil, "ceil"),
            ("-1.249", "USD -1.24", RoundingMode::Ceil, "ceil"),
        ];
        for (amount, expected, mode, name) in vectors.iter().copied() {
            let money =
                Money::from_decimal_str(amount, Currency::USD).expect("exact vector amount");
            let opts = FormatOpts::new(Some(2), Some(true), Some(","), Some(mode))
                .expect("bounded formatting options");
            assert_eq!(money.format_with(opts), expected, "{name} {amount}");
            assert_eq!(
                money.amount_decimal().to_string(),
                amount,
                "formatting must not mutate the stored amount"
            );
        }
    }
}
