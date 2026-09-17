//! Money type, conversions, formatting, and arithmetic operations.
//!
//! [`Money`] stores amounts as decimals to preserve decimal precision while
//! retaining ergonomic APIs for arithmetic and formatting.
//! Instances retain their [`Currency`] tag and refuse to mix currencies unless
//! explicitly converted via [`super::fx::FxProvider`].
//!
//! Note: Formatting is intentionally non-locale. Separators are ASCII and
//! currency code precedes the amount (e.g., "USD 1,234.56"). Use
//! [`Money::format_with_config`] or wrap at the UI layer if locale-aware
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
use crate::dates::Date;
use crate::error::{Error, InputError, NonFiniteKind};
use core::fmt;
use core::ops::{AddAssign, Div, DivAssign, Mul, MulAssign, SubAssign};

use super::rounding::{
    amount_from_repr, repr_add, repr_div_f64, repr_mul_f64, repr_sub, try_repr_div_f64,
    try_repr_mul_f64, try_round_f64, AmountRepr,
};

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
    pub decimals: Option<usize>,
    /// Whether to prepend the ISO-4217 currency code.
    pub show_currency: bool,
    /// Optional thousands separator (e.g., `Some(',')`). `None` disables grouping.
    pub group: Option<char>,
    /// Rounding mode for the displayed value.
    pub rounding: RoundingMode,
}

impl Default for FormatOpts {
    /// Defaults: 2 decimals, currency code shown, `','` grouping, Bankers rounding.
    fn default() -> Self {
        Self {
            decimals: Some(2),
            show_currency: true,
            group: Some(','),
            rounding: RoundingMode::Bankers,
        }
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
    amount: AmountRepr,
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
    /// before reaching this constructor.
    ///
    /// # Errors
    ///
    /// Returns `InputError::ConversionOverflow` when the Decimal cannot be
    /// converted to the finite `f64` view used by [`Self::amount`].
    ///
    /// # Arguments
    ///
    /// * `amount` - Finite monetary quantity in major currency units before rounding
    /// * `currency` - ISO-4217 currency that defines scale, rounding, and display units
    pub fn from_decimal(amount: rust_decimal::Decimal, currency: Currency) -> Result<Self, Error> {
        // rust_decimal::Decimal models a fixed-precision number with no
        // NaN/Infinity representation, so finiteness is structural. The
        // remaining concern is converting back to f64 for downstream f64-only
        // code paths — guard at construction time so we never hold a value
        // that can't round-trip through Money::amount().
        if rust_decimal::prelude::ToPrimitive::to_f64(&amount).is_none() {
            return Err(Error::Input(InputError::ConversionOverflow));
        }
        Ok(Self { amount, currency })
    }

    /// Construct from an exact Decimal using the configured ingest rounding policy.
    ///
    /// # Arguments
    ///
    /// * `amount` - Exact monetary amount in major currency units, rounded without conversion through f64.
    /// * `currency` - ISO-4217 currency used to select the ingest scale.
    /// * `cfg` - Configuration supplying the currency scale override (or max(6, ISO minor-unit digits)) and rounding mode.
    ///
    /// # Errors
    ///
    /// Returns a conversion error if the rounded amount cannot provide an f64 view.
    pub fn from_decimal_with_config(
        amount: rust_decimal::Decimal,
        currency: Currency,
        cfg: &FinstackConfig,
    ) -> Result<Self, Error> {
        let scale = cfg.ingest_scale(currency).min(28) as i32;
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
    fn ingest_rounding_params(
        currency: Currency,
        cfg: Option<&FinstackConfig>,
    ) -> (u32, RoundingMode) {
        match cfg {
            Some(cfg) => (cfg.ingest_scale(currency), cfg.rounding.mode),
            None => (u32::from(currency.decimals()), RoundingMode::Bankers),
        }
    }

    #[inline]
    fn try_new_finite(
        amount: f64,
        currency: Currency,
        cfg: Option<&FinstackConfig>,
    ) -> Result<Self, Error> {
        let rounded = if let Some(cfg) = cfg {
            let (dp, mode) = Self::ingest_rounding_params(currency, Some(cfg));
            try_round_f64(amount, dp as i32, mode)?
        } else {
            // Shortest round-trip conversion (`Decimal::from_f64`): `0.1_f64`
            // ingests as `0.1`, not the 28-digit IEEE expansion that
            // `from_f64_retain` would embed. `from_f64` can also fail when a
            // finite f64 lies outside Decimal's representable range.
            <AmountRepr as rust_decimal::prelude::FromPrimitive>::from_f64(amount)
                .ok_or(InputError::ConversionOverflow)?
        };
        Ok(Self {
            amount: rounded,
            currency,
        })
    }

    #[inline]
    fn try_new_impl(
        amount: f64,
        currency: Currency,
        cfg: Option<&FinstackConfig>,
    ) -> Result<Self, Error> {
        if !amount.is_finite() {
            return Err(Error::Input(InputError::NonFiniteValue {
                kind: NonFiniteKind::classify(amount),
            }));
        }
        // IEEE negative zero would become the Decimal "-0", which serializes
        // as "-0" and re-parses as "0"; normalize so wire round trips are
        // identities.
        let amount = if amount == 0.0 { 0.0 } else { amount };
        Self::try_new_finite(amount, currency, cfg)
    }

    /// Amount accessor (by value).
    ///
    /// This converts the internal [`Decimal`](rust_decimal::Decimal) representation to `f64` for
    /// ergonomic display and binding use. Values outside the finite `f64`
    /// range should avoid this lossy view and keep values on Decimal-backed APIs.
    #[inline]
    pub fn amount(&self) -> f64 {
        (*self).into_amount()
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

    /// Consume `self` and return just the numeric amount.
    #[inline]
    #[must_use]
    pub fn into_amount(self) -> f64 {
        self.into_parts().0
    }

    /// Consume `self` into `(amount, currency)`.
    #[inline]
    #[must_use]
    pub fn into_parts(self) -> (f64, Currency) {
        (amount_from_repr(self.amount), self.currency)
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
            amount: try_repr_mul_f64(self.amount, rhs)?,
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
    /// Returns `InputError::Invalid` for a zero scalar,
    /// `InputError::NonFiniteValue` for NaN or infinity, or
    /// `InputError::ConversionOverflow` when the quotient is out of range.
    ///
    /// # Arguments
    ///
    /// * `rhs` - Right-hand operand combined with this value under the documented arithmetic rules.
    #[must_use = "returns new Money on success"]
    #[inline]
    pub fn checked_div_f64(self, rhs: f64) -> Result<Self, Error> {
        Ok(Self {
            amount: try_repr_div_f64(self.amount, rhs)?,
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
        use rust_decimal::prelude::ToPrimitive;
        ensure_same_currency(&self, &rhs)?;
        if rhs.amount.is_zero() {
            return Err(Error::Validation("division by zero".into()));
        }
        self.amount
            .checked_div(rhs.amount)
            .and_then(|ratio| ratio.to_f64())
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

    /// Convert this [`Money`] into another currency using an `FxProvider`.
    ///
    /// # Parameters
    /// - `to`: target [`Currency`](crate::currency::Currency)
    /// - `on`: valuation date used for the FX lookup
    /// - `provider`: FX source implementing `FxProvider`
    /// - `policy`: lookup policy hint passed to the provider
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::money::fx::{FxConversionPolicy, FxProvider};
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::Date;
    /// use time::Month;
    ///
    /// struct StaticFx;
    /// impl FxProvider for StaticFx {
    ///     fn rate(
    ///         &self,
    ///         _from: Currency,
    ///         _to: Currency,
    ///         _on: Date,
    ///         _policy: FxConversionPolicy,
    ///     ) -> finstack_quant_core::Result<f64> {
    ///         Ok(1.2)
    ///     }
    /// }
    ///
    /// let eur = Money::from((100_i64, Currency::EUR));
    /// let trade_date = Date::from_calendar_date(2024, Month::January, 2).expect("Valid date");
    /// let usd = eur.convert(
    ///     Currency::USD,
    ///     trade_date,
    ///     &StaticFx,
    ///     FxConversionPolicy::CashflowDate,
    /// ).expect("Currency conversion should succeed");
    /// assert_eq!(usd.amount(), 120.0);
    /// assert_eq!(usd.currency(), Currency::USD);
    /// ```
    ///
    /// # Errors
    ///
    /// Propagates a provider lookup failure. It also returns
    /// `InputError::InvalidFxRate` for a non-finite or non-positive quote, and
    /// `InputError::ConversionOverflow` when applying a valid quote exceeds
    /// Decimal precision.
    pub fn convert(
        self,
        to: Currency,
        on: Date,
        provider: &impl crate::money::fx::FxProvider,
        policy: crate::money::fx::FxConversionPolicy,
    ) -> crate::Result<Self> {
        if self.currency == to {
            return Ok(self);
        }
        let rate = provider.rate(self.currency, to, on, policy)?;
        self.convert_at_rate(to, rate)
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
        let new_amount = try_repr_mul_f64(self.amount, rate)?;
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
        let dp = usize::from(self.currency.decimals());
        f.write_str(&self.format_with(FormatOpts {
            decimals: Some(dp),
            group: None,
            ..FormatOpts::default()
        }))
    }
}

impl Money {
    /// Format this money using an explicit configuration (rounding mode and per-currency scales).
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::config::FinstackConfig;
    ///
    /// let amt = Money::from((10_i64, Currency::USD));
    /// let mut cfg = FinstackConfig::default();
    /// cfg.rounding
    ///     .output_scale
    ///     .overrides
    ///     .insert(Currency::USD, 4);
    /// assert_eq!(amt.format_with_config(&cfg), "USD 10.0000");
    /// ```
    ///
    /// # Arguments
    ///
    /// * `cfg` - Finstack configuration controlling rounding policy and money tolerances
    pub fn format_with_config(&self, cfg: &FinstackConfig) -> String {
        self.format_with(FormatOpts {
            decimals: Some(cfg.output_scale(self.currency) as usize),
            show_currency: true,
            group: None,
            rounding: cfg.rounding.mode,
        })
    }
}

/// Scale a [`Money`] by an `f64`, preserving currency.
///
/// # Panics
/// Panics if `rhs` is non-finite (NaN/±∞) or if the product overflows the
/// underlying `Decimal` range (~7.9e28). For scalars from untrusted or computed
/// input, prefer [`Money::checked_mul_f64`], which returns an error instead.
impl Mul<f64> for Money {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: f64) -> Self::Output {
        Self {
            amount: repr_mul_f64(self.amount, rhs),
            currency: self.currency,
        }
    }
}

/// Divide a [`Money`] by an `f64`, preserving currency.
///
/// # Panics
/// Panics if `rhs` is non-finite, exactly zero, or if the quotient overflows the
/// underlying `Decimal` range. For divisors from untrusted or computed input,
/// prefer [`Money::checked_div_f64`], which returns an error instead.
impl Div<f64> for Money {
    type Output = Self;
    #[inline]
    fn div(self, rhs: f64) -> Self::Output {
        Self {
            amount: repr_div_f64(self.amount, rhs),
            currency: self.currency,
        }
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
                    amount: AmountRepr::from(value.0),
                    currency: value.1,
                }
            }
        }
    )+ };
}

from_integer_tuple!(i64, u64);

impl TryFrom<(f64, Currency)> for Money {
    type Error = Error;
    #[inline]
    fn try_from(value: (f64, Currency)) -> Result<Self, Error> {
        Self::new(value.0, value.1)
    }
}

/// Shorthand for constructing [`Money`] literals.
/// See unit tests and `examples/` for usage.
#[macro_export]
macro_rules! money {
    ($amount:expr, $code:ident) => {
        $crate::money::Money::new($amount, $crate::currency::Currency::$code)
    };
}

// Unchecked arithmetic – currency must match or panic
// NOTE: AddAssign and SubAssign require matching currencies. Currency
// mismatch will always panic regardless of build type. For explicit error
// handling, use `checked_add` and `checked_sub` which return `Result<Money, Error>`.

impl AddAssign for Money {
    /// Adds another [`Money`] value to this one in place.
    ///
    /// # Panics
    ///
    /// Panics if `rhs` has a different currency or if the addition
    /// overflows `Decimal`. For fallible arithmetic, use
    /// [`Money::checked_add`] which returns `Result`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::currency::Currency;
    ///
    /// let mut total = Money::from((100_i64, Currency::USD));
    /// total += Money::from((50_i64, Currency::USD));
    /// assert_eq!(total.amount(), 150.0);
    /// ```
    #[track_caller]
    #[allow(clippy::panic)]
    fn add_assign(&mut self, rhs: Self) {
        // Always fail loudly on currency mismatch; silent no-ops are correctness bugs.
        assert!(
            self.currency == rhs.currency,
            "Currency mismatch in Money::add_assign: lhs={}, rhs={}",
            self.currency,
            rhs.currency
        );
        self.amount = repr_add(self.amount, rhs.amount)
            .unwrap_or_else(|_| panic!("Decimal overflow in Money::add_assign"));
    }
}

impl SubAssign for Money {
    /// Subtracts another [`Money`] value from this one in place.
    ///
    /// # Panics
    ///
    /// Panics if `rhs` has a different currency or if the subtraction
    /// overflows `Decimal`. For fallible arithmetic, use
    /// [`Money::checked_sub`] which returns `Result`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::currency::Currency;
    ///
    /// let mut total = Money::from((100_i64, Currency::USD));
    /// total -= Money::from((30_i64, Currency::USD));
    /// assert_eq!(total.amount(), 70.0);
    /// ```
    #[track_caller]
    #[allow(clippy::panic)]
    fn sub_assign(&mut self, rhs: Self) {
        // Always fail loudly on currency mismatch; silent no-ops are correctness bugs.
        assert!(
            self.currency == rhs.currency,
            "Currency mismatch in Money::sub_assign: lhs={}, rhs={}",
            self.currency,
            rhs.currency
        );
        self.amount = repr_sub(self.amount, rhs.amount)
            .unwrap_or_else(|_| panic!("Decimal overflow in Money::sub_assign"));
    }
}

impl MulAssign<f64> for Money {
    fn mul_assign(&mut self, rhs: f64) {
        self.amount = repr_mul_f64(self.amount, rhs);
    }
}

impl DivAssign<f64> for Money {
    fn div_assign(&mut self, rhs: f64) {
        self.amount = repr_div_f64(self.amount, rhs);
    }
}

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
    fn macro_constructs_money() {
        let m = crate::money!(250.0, GBP).expect("valid money literal");
        assert_eq!(m.amount(), 250.0);
        assert_eq!(m.currency(), Currency::GBP);
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
        let formatted = m.format_with(FormatOpts::default());
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
    #[should_panic(expected = "Money division requires finite")]
    fn division_by_zero_panics() {
        let _ = Money::from((10_i64, Currency::USD)) / 0.0;
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
        assert!(money.checked_div_f64(0.0).is_err());
        assert!(money.checked_div_f64(f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn money_operators_accept_finite_scalars() {
        let money = Money::from((12_i64, Currency::USD));

        assert_eq!((money * 2.0).amount(), 24.0);
        assert_eq!((money / 3.0).amount(), 4.0);
    }

    #[test]
    fn money_assign_operators_accept_finite_scalars() {
        let mut money = Money::from((12_i64, Currency::USD));

        money *= 2.5;
        assert_eq!(money.amount(), 30.0);
        assert_eq!(money.currency(), Currency::USD);

        money /= 4.0;
        assert_eq!(money.amount(), 7.5);
        assert_eq!(money.currency(), Currency::USD);
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
    #[should_panic(expected = "Money multiplication requires finite")]
    fn multiply_by_nan_panics() {
        let _ = Money::from((10_i64, Currency::USD)) * f64::NAN;
    }

    struct NaNProvider;
    impl crate::money::fx::FxProvider for NaNProvider {
        fn rate(
            &self,
            _from: Currency,
            _to: Currency,
            _on: Date,
            _policy: crate::money::fx::FxConversionPolicy,
        ) -> crate::Result<f64> {
            Ok(f64::NAN)
        }
    }

    #[test]
    fn convert_rejects_non_finite_rate() {
        let usd = Money::from((5_i64, Currency::USD));
        let date =
            Date::from_calendar_date(2024, time::Month::January, 1).expect("Valid test date");
        let res = usd.convert(
            Currency::EUR,
            date,
            &NaNProvider,
            crate::money::fx::FxConversionPolicy::CashflowDate,
        );
        assert!(res.is_err());
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
            Err(crate::error::Error::Input(
                crate::error::InputError::NonFiniteValue { kind }
            )) if kind == crate::error::NonFiniteKind::NaN
        ));
    }

    #[test]
    fn try_new_returns_error_for_positive_infinity() {
        let result = Money::new(f64::INFINITY, Currency::EUR);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(crate::error::Error::Input(
                crate::error::InputError::NonFiniteValue { kind }
            )) if kind == crate::error::NonFiniteKind::PosInfinity
        ));
    }

    #[test]
    fn try_new_returns_error_for_negative_infinity() {
        let result = Money::new(f64::NEG_INFINITY, Currency::GBP);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(crate::error::Error::Input(
                crate::error::InputError::NonFiniteValue { kind }
            )) if kind == crate::error::NonFiniteKind::NegInfinity
        ));
    }

    #[test]
    fn try_new_with_config_succeeds_for_finite_values() {
        let mut cfg = FinstackConfig::default();
        cfg.rounding.ingest_scale.overrides.insert(Currency::USD, 3);
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
        cfg.rounding.ingest_scale.overrides.insert(Currency::USD, 2);
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
        let eur = Money::from_decimal(amount, Currency::EUR).expect("representable amount");
        let usd = eur
            .convert_at_rate(Currency::USD, 2.0)
            .expect("valid conversion");

        assert_eq!(
            usd.amount_decimal(),
            rust_decimal::Decimal::from_str("20000000000000000.2").expect("valid decimal")
        );
    }

    #[test]
    #[should_panic(expected = "Currency mismatch")]
    fn add_assign_panics_on_currency_mismatch() {
        let mut usd = Money::from((100_i64, Currency::USD));
        let eur = Money::from((50_i64, Currency::EUR));
        usd += eur;
    }

    #[test]
    #[should_panic(expected = "Currency mismatch")]
    fn sub_assign_panics_on_currency_mismatch() {
        let mut usd = Money::from((100_i64, Currency::USD));
        let eur = Money::from((50_i64, Currency::EUR));
        usd -= eur;
    }

    #[test]
    fn add_assign_succeeds_for_matching_currencies() {
        let mut total = Money::from((100_i64, Currency::USD));
        total += Money::from((50_i64, Currency::USD));
        assert_eq!(total.amount(), 150.0);
    }

    #[test]
    fn sub_assign_succeeds_for_matching_currencies() {
        let mut total = Money::from((100_i64, Currency::USD));
        total -= Money::from((30_i64, Currency::USD));
        assert_eq!(total.amount(), 70.0);
    }

    fn hash_of(money: Money) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        money.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn hash_matches_equality_across_decimal_scale() {
        let a = Money::from_decimal(rust_decimal::Decimal::new(10, 1), Currency::USD)
            .expect("1.0 is representable");
        let b = Money::from_decimal(rust_decimal::Decimal::new(100, 2), Currency::USD)
            .expect("1.00 is representable");
        assert_eq!(a, b);
        assert_eq!(hash_of(a), hash_of(b));
    }
}
