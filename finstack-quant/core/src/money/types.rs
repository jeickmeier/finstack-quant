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
use crate::decimal::{decimal_to_f64_nearest, f64_to_decimal, f64_to_decimal_model_value};
use crate::{Error, InputError};
use core::fmt;

use super::rounding::{
    repr_add, repr_div, repr_div_f64, repr_mul, repr_mul_f64, repr_sub, round_decimal,
    validate_rounding_scale,
};

/// Format an integer string (optionally prefixed by `-`) with thousands
/// separator `sep`.
pub(super) fn group_thousands(int_str: &str, sep: char) -> String {
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
    pub(super) decimals: Option<usize>,
    /// Whether to prepend the ISO-4217 currency code.
    pub(super) show_currency: bool,
    /// Optional thousands separator (e.g., `Some(',')`). `None` disables grouping.
    pub(super) group: Option<char>,
    /// Rounding mode for the displayed value.
    pub(super) rounding: RoundingMode,
}

impl Default for FormatOpts {
    /// Declared presentation defaults (record 012 #10), matching `Display`:
    /// currency minor-unit decimals, currency code shown, no grouping and
    /// nearest-even display rounding. This is not a financial rounding default.
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
    /// Maximum supported display precision: Decimal's scale ceiling (28).
    pub const MAX_DECIMALS: usize = 28;

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
    ///   selects the declared presentation default, nearest-even (Bankers).
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
    /// use finstack_quant_core::config::RoundingMode;
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::{FormatOpts, Money};
    ///
    /// let opts = FormatOpts::new(Some(2), Some(true), Some(","), Some(RoundingMode::Bankers))
    ///     .expect("bounded precision");
    /// let amount = Money::new(1234.5, Currency::USD).expect("valid amount");
    /// assert_eq!(amount.format_with(opts), "USD 1,234.50");
    /// assert!(FormatOpts::new(None, None, Some(", "), None).is_err());
    /// ```
    pub fn new(
        decimals: Option<usize>,
        show_currency: Option<bool>,
        group: Option<&str>,
        rounding: Option<RoundingMode>,
    ) -> Result<Self, Error> {
        let show_currency = show_currency.unwrap_or(true);
        let rounding = rounding.unwrap_or(RoundingMode::Bankers);
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
                        ));
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
        let dp = opts
            .decimals
            .unwrap_or_else(|| usize::from(self.currency.decimals()));
        // `dp` is at most `FormatOpts::MAX_DECIMALS` (28) by construction.
        let mut rounded = round_decimal(self.amount, dp as u32, opts.rounding);
        // A displayed zero carries no sign (`-0.001` at 2 places is `0.00`).
        if rounded.is_zero() {
            rounded.set_sign_positive(true);
        }
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
    /// Ingests the `f64` exactly through its shortest round-trip decimal text
    /// ([`f64_to_decimal`]). Use [`Self::from_decimal`] when the caller already
    /// holds an exact Decimal, and [`Self::from_model_value`] for a pricer's
    /// model output.
    ///
    /// # Errors
    ///
    /// Returns `InputError::NonFiniteValue` for NaN or infinity, or
    /// `InputError::ConversionOverflow` when the shortest text is outside the
    /// Decimal domain (too large, or nonzero below `1e-28`).
    ///
    /// # Arguments
    ///
    /// * `amount` - Finite monetary quantity in major currency units before rounding
    /// * `currency` - ISO-4217 currency that defines scale, rounding, and display units
    pub fn new(amount: f64, currency: Currency) -> Result<Self, Error> {
        Self::try_new_impl(amount, currency, None)
    }

    /// Construct from a pricer's model value, rounding explicitly to
    /// Decimal's 28-digit scale (half-even).
    ///
    /// The named model-value boundary (owner decision on
    /// CORE-CONVENTIONS-012 B2b, [`f64_to_decimal_model_value`]): a model
    /// value below `0.5e-28` becomes an exact zero. Use it only for model
    /// outputs (PVs, expected losses, projected cashflows); input admission
    /// uses [`Self::new`].
    ///
    /// # Errors
    ///
    /// Returns `InputError::NonFiniteValue` for NaN or infinity, or
    /// `InputError::ConversionOverflow` when the magnitude is too large for
    /// the internal Decimal type.
    ///
    /// # Arguments
    ///
    /// * `amount` - Finite model value in major currency units
    /// * `currency` - ISO-4217 currency of the amount
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    ///
    /// assert!(Money::new(1e-30, Currency::USD).is_err());
    /// assert_eq!(Money::from_model_value(1e-30, Currency::USD)?.amount(), 0.0);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn from_model_value(amount: f64, currency: Currency) -> Result<Self, Error> {
        Ok(Self::from_decimal(
            f64_to_decimal_model_value(amount)?,
            currency,
        ))
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
    /// exact decimal text and parsed via [`crate::decimal::parse_decimal`]).
    /// Use this for notionals where IEEE 754 rounding of the input would be
    /// unacceptable. A negative zero is normalized to zero.
    ///
    /// Every `Decimal` has a finite `f64` view for [`Self::amount`], so
    /// construction cannot fail.
    ///
    /// # Arguments
    ///
    /// * `amount` - Finite monetary quantity in major currency units before rounding
    /// * `currency` - ISO-4217 currency that defines scale, rounding, and display units
    #[must_use]
    pub fn from_decimal(amount: rust_decimal::Decimal, currency: Currency) -> Self {
        let amount = if amount.is_zero() {
            rust_decimal::Decimal::ZERO
        } else {
            amount
        };
        Self { amount, currency }
    }

    /// Construct from exact decimal text, rejecting any inexact representation.
    ///
    /// `amount` is parsed directly as a `rust_decimal::Decimal` — fixed-point
    /// text such as `"1234.56"` or scientific text such as `"1.2345e3"` —
    /// without converting through `f64`. The text must be exactly
    /// representable within Decimal's 96-bit coefficient and its maximum
    /// fractional scale of 28 digits; values needing more precision are
    /// rejected rather than silently rounded. The grammar and exponent rules
    /// are those of [`crate::decimal::parse_decimal`], which also restores
    /// Money's wire amount.
    ///
    /// The currency is a display/arithmetic tag only: no FX conversion is
    /// performed.
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
    /// Without an ingest scale override for `currency` the amount is kept
    /// exactly (record 012 #9); with one, it is rounded once to that scale in
    /// the policy's selected mode.
    ///
    /// # Arguments
    ///
    /// * `amount` - Exact monetary amount in major currency units, rounded without conversion through f64.
    /// * `currency` - ISO-4217 currency whose ingest override, if any, selects the scale.
    /// * `cfg` - Configuration supplying the optional currency scale override and the rounding mode it requires.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when `currency` has an ingest scale override
    /// but `cfg` selects no rounding mode.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::config::{FinstackConfig, RoundingMode};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use rust_decimal::Decimal;
    ///
    /// let mut cfg = FinstackConfig::default();
    /// let exact = Decimal::new(1, 3); // 0.001
    /// // No override: preserved exactly even though a config is supplied.
    /// let kept = Money::from_decimal_with_config(exact, Currency::USD, &cfg).expect("exact");
    /// assert_eq!(kept.amount_decimal(), exact);
    ///
    /// cfg.rounding.ingest_scale.set_scale(Currency::USD, 2).expect("valid scale");
    /// assert!(Money::from_decimal_with_config(exact, Currency::USD, &cfg).is_err());
    /// cfg.rounding.mode = Some(RoundingMode::Ceil);
    /// let rounded = Money::from_decimal_with_config(exact, Currency::USD, &cfg).expect("rounded");
    /// assert_eq!(rounded.amount_decimal(), Decimal::new(1, 2));
    /// ```
    pub fn from_decimal_with_config(
        amount: rust_decimal::Decimal,
        currency: Currency,
        cfg: &FinstackConfig,
    ) -> Result<Self, Error> {
        let amount = match cfg.ingest_rounding(currency)? {
            Some((scale, mode)) => round_decimal(amount, scale, mode),
            None => amount,
        };
        Ok(Self::from_decimal(amount, currency))
    }

    /// Fallible constructor using an explicit configuration for rounding.
    ///
    /// The `f64` is first admitted exactly as [`Self::new`] does; only then is
    /// the configured ingest rounding applied (see
    /// [`Self::from_decimal_with_config`]), so configuration cannot rescue an
    /// unrepresentable input.
    ///
    /// # Errors
    ///
    /// Returns `InputError::NonFiniteValue` for NaN or infinity,
    /// `InputError::ConversionOverflow` when the value is outside the Decimal
    /// domain, or `Error::Validation` when `currency` has an ingest scale
    /// override but `cfg` selects no rounding mode.
    ///
    /// # Arguments
    ///
    /// * `amount` - Finite monetary quantity in major currency units before rounding
    /// * `currency` - ISO-4217 currency that defines scale, rounding, and display units
    /// * `cfg` - Configuration supplying the optional currency ingest override and the rounding mode it requires
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
        let exact = f64_to_decimal(amount)?;
        match cfg {
            Some(cfg) => Self::from_decimal_with_config(exact, currency, cfg),
            None => Ok(Self::from_decimal(exact, currency)),
        }
    }

    /// Amount accessor (by value).
    ///
    /// This converts the internal [`Decimal`](rust_decimal::Decimal)
    /// representation to binary64 using exact decimal-to-binary single rounding
    /// (ties to even). Values needing full fixed-point precision should keep
    /// using [`Self::amount_decimal`].
    #[inline]
    pub fn amount(&self) -> f64 {
        decimal_to_f64_nearest(self.amount)
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
    /// `InputError::ConversionOverflow` when the sum cannot keep every digit
    /// down to the currency's minor unit (`1e28 + 0.01` USD is an error, not
    /// `1e28`). Digits below the minor unit that do not fit in Decimal's 28-29
    /// digits are rounded once, half-even; a representable sum is exact.
    ///
    /// # Arguments
    ///
    /// * `rhs` - Right-hand operand combined with this value under the documented arithmetic rules.
    #[must_use = "returns new Money if currencies match"]
    #[inline]
    pub fn checked_add(self, rhs: Self) -> Result<Self, Error> {
        ensure_same_currency(&self, &rhs)?;
        Ok(Self::from_decimal(
            repr_add(self.amount, rhs.amount, u32::from(self.currency.decimals()))?,
            self.currency,
        ))
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
    /// `InputError::ConversionOverflow` when the difference cannot keep every
    /// digit down to the currency's minor unit. Digits below the minor unit
    /// that do not fit are rounded once, half-even; a representable
    /// difference is exact.
    ///
    /// # Arguments
    ///
    /// * `rhs` - Right-hand operand combined with this value under the documented arithmetic rules.
    #[must_use = "returns new Money if currencies match"]
    #[inline]
    pub fn checked_sub(self, rhs: Self) -> Result<Self, Error> {
        ensure_same_currency(&self, &rhs)?;
        Ok(Self::from_decimal(
            repr_sub(self.amount, rhs.amount, u32::from(self.currency.decimals()))?,
            self.currency,
        ))
    }

    /// Multiply by a Decimal scalar (record 012 #7).
    ///
    /// The product is exact when representable; otherwise it is the exact
    /// product rounded once, half-even, at the greatest scale (at most 28
    /// decimal places) whose coefficient fits. Signed and zero factors are
    /// allowed. No currency minor-unit rounding is applied.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use rust_decimal::Decimal;
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// let m = Money::from_decimal_str("100.10", Currency::USD)?;
    /// assert_eq!(m.checked_mul(Decimal::new(25, 1))?.amount_decimal().to_string(), "250.250");
    /// // A nonzero product below 1e-28 is an underflow error, not zero.
    /// let tiny = Money::from_decimal_str("0.0000000000000000000000000001", Currency::USD)?;
    /// assert!(tiny.checked_mul(Decimal::new(1, 1)).is_err());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Arguments
    ///
    /// * `factor` - Signed dimensionless Decimal multiplier; zero gives zero.
    ///
    /// # Errors
    ///
    /// Returns `InputError::ConversionOverflow` when the product's magnitude
    /// exceeds the Decimal range, or `InputError::DecimalUnderflow` when a
    /// nonzero product rounds to zero.
    #[must_use = "returns new Money on success"]
    pub fn checked_mul(self, factor: rust_decimal::Decimal) -> Result<Self, Error> {
        Ok(Self::from_decimal(
            repr_mul(self.amount, factor)?,
            self.currency,
        ))
    }

    /// Multiply by an `f64` scalar: the named float adapter for
    /// [`Self::checked_mul`].
    ///
    /// The scalar is admitted by its shortest round-trip decimal text
    /// (record 012 #4), so `0.1` multiplies by exactly `0.1`; the product then
    /// follows [`Self::checked_mul`].
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
    /// Returns `InputError::NonFiniteValue` for a NaN or infinite scalar,
    /// `InputError::ConversionOverflow` when the scalar's text is outside the
    /// Decimal domain or the product overflows, or
    /// `InputError::DecimalUnderflow` when a nonzero product rounds to zero.
    ///
    /// # Arguments
    ///
    /// * `rhs` - Finite dimensionless binary64 multiplier, admitted exactly by its shortest round-trip text.
    #[must_use = "returns new Money on success"]
    #[inline]
    pub fn checked_mul_f64(self, rhs: f64) -> Result<Self, Error> {
        Ok(Self::from_decimal(
            repr_mul_f64(self.amount, rhs)?,
            self.currency,
        ))
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
        // Zero stays the canonical unsigned zero (record 012 #1).
        Self::from_decimal(-self.amount, self.currency)
    }

    /// Divide by a nonzero Decimal scalar (record 012 #7).
    ///
    /// The quotient follows the same bounded rule as [`Self::checked_mul`]:
    /// exact when representable, otherwise rounded once, half-even, at the
    /// greatest scale whose coefficient fits.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use rust_decimal::Decimal;
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// let m = Money::from((1_i64, Currency::USD));
    /// let third = m.checked_div(Decimal::from(3))?;
    /// assert_eq!(third.amount_decimal().to_string(), "0.3333333333333333333333333333");
    /// assert!(m.checked_div(Decimal::ZERO).is_err());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Arguments
    ///
    /// * `divisor` - Nonzero signed dimensionless Decimal divisor.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation("division by zero")` for a zero divisor,
    /// `InputError::ConversionOverflow` when the quotient's magnitude exceeds
    /// the Decimal range, or `InputError::DecimalUnderflow` when a nonzero
    /// quotient rounds to zero.
    #[must_use = "returns new Money on success"]
    pub fn checked_div(self, divisor: rust_decimal::Decimal) -> Result<Self, Error> {
        Ok(Self::from_decimal(
            repr_div(self.amount, divisor)?,
            self.currency,
        ))
    }

    /// Divide by an `f64` scalar: the named float adapter for
    /// [`Self::checked_div`].
    ///
    /// The scalar is admitted by its shortest round-trip decimal text
    /// (record 012 #4) before the Decimal division.
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
    /// Returns `Error::Validation("division by zero")` for `0.0` or `-0.0`,
    /// `InputError::NonFiniteValue` for NaN or infinity,
    /// `InputError::ConversionOverflow` when the scalar's text is outside the
    /// Decimal domain or the quotient overflows, or
    /// `InputError::DecimalUnderflow` when a nonzero quotient rounds to zero.
    ///
    /// # Arguments
    ///
    /// * `rhs` - Finite nonzero dimensionless binary64 divisor, admitted exactly by its shortest round-trip text.
    #[must_use = "returns new Money on success"]
    #[inline]
    pub fn checked_div_f64(self, rhs: f64) -> Result<Self, Error> {
        Ok(Self::from_decimal(
            repr_div_f64(self.amount, rhs)?,
            self.currency,
        ))
    }

    /// Dimensionless binary64 ratio of two same-currency amounts
    /// (record 012 #5).
    ///
    /// Each exact amount is rounded once to the nearest binary64 and the two
    /// are divided in binary64, so the relative error against the exact
    /// Decimal ratio is at most about `1.5 * f64::EPSILON`. No Decimal
    /// quotient is formed, so a ratio outside Decimal's range is still
    /// returned. A zero numerator returns `0.0`.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// let a = Money::from((3_i64, Currency::USD));
    /// let b = Money::from((4_i64, Currency::USD));
    /// assert_eq!(a.ratio(b)?, 0.75);
    /// assert!(a.ratio(Money::zero(Currency::USD)).is_err());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Arguments
    ///
    /// * `rhs` - Nonzero denominator in the same currency; no FX conversion is performed.
    ///
    /// # Errors
    ///
    /// Returns `Error::CurrencyMismatch` when the currencies differ, or
    /// `Error::Validation("division by zero")` for a zero denominator.
    pub fn ratio(self, rhs: Self) -> Result<f64, Error> {
        ensure_same_currency(&self, &rhs)?;
        if rhs.amount.is_zero() {
            return Err(Error::Validation("division by zero".into()));
        }
        if self.amount.is_zero() {
            return Ok(0.0);
        }
        Ok(decimal_to_f64_nearest(self.amount) / decimal_to_f64_nearest(rhs.amount))
    }

    /// Return the exact fractional change `(self - baseline) / baseline` in the same currency.
    ///
    /// Evaluates `(self - baseline)` in `Decimal` via `checked_sub` when representable and
    /// converts the numerator and denominator cleanly to `f64`, falling back to `f64`
    /// difference only when opposite-signed extreme `Decimal` values exceed `Decimal::MAX`.
    /// Rejects zero baselines even when `self` is also zero.
    ///
    /// # Arguments
    /// - `baseline` - Non-zero baseline money amount in the same currency as `self`.
    ///
    /// # Errors
    /// Returns [`Error::CurrencyMismatch`](crate::Error::CurrencyMismatch) if currencies
    /// differ, or [`Error::Validation`](crate::Error::Validation) if `baseline` is zero or
    /// the resulting ratio is non-finite or underflows to zero for distinct amounts.
    pub fn proportional_change(self, baseline: Self) -> Result<f64, Error> {
        ensure_same_currency(&self, &baseline)?;
        if baseline.amount.is_zero() {
            return Err(Error::Validation("division by zero".into()));
        }
        if self.amount == baseline.amount {
            return Ok(0.0);
        }
        let denom = decimal_to_f64_nearest(baseline.amount);
        let ratio = match self.amount.checked_sub(baseline.amount) {
            Some(delta) => decimal_to_f64_nearest(delta) / denom,
            None => (decimal_to_f64_nearest(self.amount) - denom) / denom,
        };
        if !ratio.is_finite() || ratio == 0.0 {
            return Err(Error::Validation(
                "proportional change ratio overflowed or underflowed f64".into(),
            ));
        }
        Ok(ratio)
    }

    /// Return the absolute amount in the same currency without rounding.
    pub fn abs(self) -> Self {
        Self {
            amount: self.amount.abs(),
            currency: self.currency,
        }
    }

    /// Financially round the exact amount to an explicit scale and mode
    /// (record 012 #8), preserving its currency.
    ///
    /// There is no default scale or mode. An explicit round may intentionally
    /// produce zero (`1e-28` at scale 2 is `0`), which is normalized like any
    /// other zero.
    ///
    /// # Arguments
    ///
    /// * `scale` - Decimal places in major currency units, in `0..=28`.
    /// * `mode` - Rounding rule applied at that decimal quantum.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when `scale` exceeds 28.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::config::RoundingMode;
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    ///
    /// let m = Money::from_decimal_str("2.5", Currency::USD).expect("exact");
    /// assert_eq!(m.round(0, RoundingMode::Bankers).expect("scale 0").amount_decimal().to_string(), "2");
    /// assert_eq!(m.round(0, RoundingMode::MidpointAwayFromZero).expect("scale 0").amount_decimal().to_string(), "3");
    /// assert!(m.round(29, RoundingMode::Bankers).is_err());
    /// ```
    pub fn round(self, scale: u32, mode: RoundingMode) -> Result<Self, Error> {
        validate_rounding_scale(scale)?;
        Ok(Self::from_decimal(
            round_decimal(self.amount, scale, mode),
            self.currency,
        ))
    }

    /// Convert this amount with an already-resolved FX rate (record 012 #11).
    ///
    /// `rate` is the exact number of target units per one source unit and must
    /// be strictly positive. For the same source and target currency the rate
    /// must be exactly one; a nonunit rate is rejected rather than ignored.
    /// The rate is validated before any shortcut. For distinct currencies the
    /// amount is multiplied by [`Self::checked_mul`]'s bounded rule and
    /// relabelled; no target minor-unit rounding is applied (round at the
    /// reporting boundary instead).
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use rust_decimal::Decimal;
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// let eur = Money::from_decimal_str("100.00", Currency::EUR)?;
    /// let usd = eur.convert_at_rate(Currency::USD, Decimal::new(10825, 4))?;
    /// assert_eq!(usd.amount_decimal().to_string(), "108.250000");
    /// assert!(eur.convert_at_rate(Currency::EUR, Decimal::new(101, 2)).is_err());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Arguments
    ///
    /// * `to` - Target currency of the converted amount.
    /// * `rate` - Strictly positive Decimal FX rate in target units per one source unit; exactly `1` when `to` equals the source currency.
    ///
    /// # Errors
    ///
    /// Returns `InputError::InvalidFxRate` for a rate that is not strictly
    /// positive, `InputError::NonUnitSameCurrencyRate` for a same-currency
    /// rate other than one, or the [`Self::checked_mul`] overflow/underflow
    /// errors.
    pub fn convert_at_rate(self, to: Currency, rate: rust_decimal::Decimal) -> crate::Result<Self> {
        if rate <= rust_decimal::Decimal::ZERO {
            return Err(InputError::InvalidFxRate {
                from: self.currency,
                to,
                rate: decimal_to_f64_nearest(rate),
            }
            .into());
        }
        if self.currency == to {
            if rate != rust_decimal::Decimal::ONE {
                return Err(InputError::NonUnitSameCurrencyRate { currency: to, rate }.into());
            }
            return Ok(self);
        }
        Ok(Self::from_decimal(repr_mul(self.amount, rate)?, to))
    }

    /// Convert with an `f64` FX rate: the named float adapter for
    /// [`Self::convert_at_rate`].
    ///
    /// The rate must be finite and strictly positive; it is admitted by its
    /// shortest round-trip decimal text (record 012 #4) and then delegated, so
    /// a same-currency rate must be exactly `1.0`.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// let eur = Money::from((100_i64, Currency::EUR));
    /// assert_eq!(eur.convert_at_rate_f64(Currency::USD, 1.1)?.amount(), 110.0);
    /// assert!(eur.convert_at_rate_f64(Currency::USD, f64::NAN).is_err());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Arguments
    ///
    /// * `to` - Target currency of the converted amount.
    /// * `rate` - Finite, strictly positive binary64 FX rate in target units per one source unit; exactly `1.0` when `to` equals the source currency.
    ///
    /// # Errors
    ///
    /// Returns `InputError::InvalidFxRate` for a non-finite or non-positive
    /// rate, `InputError::ConversionOverflow` when its text is outside the
    /// Decimal domain, and otherwise the [`Self::convert_at_rate`] errors.
    pub fn convert_at_rate_f64(self, to: Currency, rate: f64) -> crate::Result<Self> {
        let rate = crate::money::fx::validate_fx_rate(self.currency, to, rate)?;
        self.convert_at_rate(to, f64_to_decimal(rate)?)
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
