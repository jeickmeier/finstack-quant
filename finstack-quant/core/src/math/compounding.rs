//! Compounding convention definitions for interest rate calculations.
//!
//! This module defines the [`Compounding`] enum which specifies how interest rates
//! should be quoted or converted. Different markets and instruments use different
//! compounding conventions, and this enum provides a unified way to specify the
//! desired convention.
//!
//! # Market Conventions
//!
//! | Convention | Common Usage | Formula (rate from DF) |
//! |------------|--------------|------------------------|
//! | Continuous | Internal calculations, curve construction | r = -ln(DF) / t |
//! | Semi-Annual | US Treasury, corporate bonds | r = 2 × (DF^(-1/(2t)) - 1) |
//! | Quarterly | Some floating rate notes | r = 4 × (DF^(-1/(4t)) - 1) |
//! | Simple | Money market (< 1Y), deposits | r = (1/DF - 1) / t |
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::math::Compounding;
//! use std::num::NonZeroU32;
//!
//! // Continuous compounding (most common for quant calculations)
//! let cont = Compounding::Continuous;
//!
//! // Semi-annual compounding (US Treasury convention)
//! let semi = Compounding::SEMI_ANNUAL;
//!
//! // Custom periodic compounding
//! let monthly = Compounding::Periodic(NonZeroU32::new(12).unwrap());
//!
//! // Simple interest (money market)
//! let simple = Compounding::Simple;
//! ```
//!
//! # References
//!
//! - Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.).
//!   Pearson. Chapter 4 (Interest Rates). `docs/REFERENCES.md#hull-options-futures`
//! - Brigo, D., & Mercurio, F. (2006). *Interest Rate Models - Theory and Practice*
//!   (2nd ed.). Springer. Chapter 1 (Definitions and Notation). `docs/REFERENCES.md#brigo-mercurio-2006-interest-rate-models`

use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

/// Compounding convention for interest rates.
///
/// Used to specify how interest rates should be quoted or converted.
/// All variants produce mathematically equivalent discount factors when
/// applied consistently.
///
/// # Relationship Between Conventions
///
/// For a given discount factor DF at time t, the rates under different
/// conventions are related by:
///
/// ```text
/// DF = e^(-r_cc × t)                    [Continuous]
///    = (1 + r_ann)^(-t)                 [Periodic(1)]
///    = (1 + r_per/n)^(-n×t)             [Periodic(n)]
///    = 1 / (1 + r_simple × t)           [Simple]
/// ```
///
/// # Ordering of Rates
///
/// For a common discount factor below one, annual and simple rates both exceed
/// the continuous rate. Their ordering depends on the maturity `t` in years:
/// `r_annual > r_simple` for `0 < t < 1`, they agree at `t = 1`, and
/// `r_simple > r_annual` for `t > 1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Compounding {
    /// Continuous compounding: r = -ln(DF) / t
    ///
    /// Most common for internal calculations, curve construction, and
    /// quantitative finance models. Provides the simplest mathematical
    /// properties (additive over time).
    Continuous,

    /// Periodic compounding with n periods per year: r = n × (DF^(-1/(n×t)) - 1)
    ///
    /// Common values:
    /// - n=1: Annual (UK gilts, European government bonds and annual zero rates)
    /// - n=2: Semi-annual (US Treasury, corporate bonds)
    /// - n=4: Quarterly (some FRNs)
    /// - n=12: Monthly (rare, some retail products)
    ///
    /// Note: Uses [`NonZeroU32`] to prevent division by zero at compile time.
    Periodic(NonZeroU32),

    /// Simple interest (no compounding): r = (1/DF - 1) / t
    ///
    /// Used for money market instruments with maturity < 1 year, including:
    /// - Interbank deposits
    /// - T-bills and commercial paper
    /// - SOFR, SONIA, €STR fixings
    ///
    /// Typically paired with ACT/360 (USD, EUR) or ACT/365F (GBP) day counts.
    Simple,
}

/// Helper const fn to create `NonZeroU32` in const context.
///
/// The fallback branch is only present to satisfy const evaluation without
/// introducing `panic!` in production code; all current call sites pass
/// non-zero compile-time literals.
const fn nonzero_u32(n: u32) -> NonZeroU32 {
    match NonZeroU32::new(n) {
        Some(v) => v,
        None => NonZeroU32::MIN,
    }
}

impl Compounding {
    /// Semi-annual compounding (n=2).
    ///
    /// Standard for US Treasury bonds and most US corporate bonds.
    pub const SEMI_ANNUAL: Self = Compounding::Periodic(nonzero_u32(2));

    /// Quarterly compounding (n=4).
    ///
    /// Used for some floating rate notes and quarterly-paying bonds.
    pub const QUARTERLY: Self = Compounding::Periodic(nonzero_u32(4));

    /// Monthly compounding (n=12).
    ///
    /// Relatively rare in wholesale markets; more common in retail products.
    pub const MONTHLY: Self = Compounding::Periodic(nonzero_u32(12));

    /// Returns the number of compounding periods per year, if applicable.
    ///
    /// - `Periodic(n)` returns `Some(n)`
    /// - `Continuous` and `Simple` return `None`
    #[must_use]
    pub fn periods_per_year(&self) -> Option<u32> {
        match self {
            Compounding::Periodic(n) => Some(n.get()),
            Compounding::Continuous | Compounding::Simple => None,
        }
    }

    /// Convert an interest rate to a discount factor for time `t` (in years).
    ///
    /// Uses the compounding convention of `self`:
    ///
    /// ```text
    /// Continuous:   DF = exp(-r × t)
    /// Periodic(n):  DF = (1 + r/n)^(-n × t)
    /// Simple:       DF = 1 / (1 + r × t)
    /// ```
    ///
    /// # Arguments
    ///
    /// * `rate` - Annualized interest rate as a decimal (`0.05` = 5%) quoted in `self`.
    /// * `t` - Non-negative maturity in years on the curve day-count clock.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`](crate::Error::Validation) when `rate` or `t`
    /// is non-finite, `t < 0.0`, `rate` violates the convention domain even at
    /// `t == 0.0` (`1 + r/n <= 0` for `Periodic(n)`, `1 + r × t <= 0` for `Simple`),
    /// required intermediate scaling underflows a non-zero rate, or the computed
    /// discount factor is non-finite or underflows to `<= 0.0`.
    ///
    /// # References
    ///
    /// Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.),
    /// Chapter 4 — Interest Rates. `docs/REFERENCES.md#hull-options-futures`
    #[inline]
    pub fn df_from_rate(&self, rate: f64, t: f64) -> crate::Result<f64> {
        if !rate.is_finite() || !t.is_finite() || t < 0.0 {
            return Err(crate::Error::Validation(format!(
                "df_from_rate requires finite rate and non-negative finite t (got rate={rate}, t={t})"
            )));
        }
        let df = match self {
            Compounding::Continuous => {
                if t == 0.0 || rate == 0.0 {
                    return Ok(1.0);
                }
                (-rate * t).exp()
            }
            Compounding::Periodic(_) => {
                let c = self.instantaneous_rate(rate)?;
                if t == 0.0 || rate == 0.0 {
                    return Ok(1.0);
                }
                (-c * t).exp()
            }
            Compounding::Simple => {
                let denominator = rate.mul_add(t, 1.0);
                if !denominator.is_finite() || denominator <= 0.0 {
                    return Err(crate::Error::Validation(format!(
                        "simple compounding denominator 1 + rate * t must be strictly positive (got {denominator} for rate={rate}, t={t})"
                    )));
                }
                if t == 0.0 || rate == 0.0 {
                    return Ok(1.0);
                }
                1.0 / denominator
            }
        };
        if !df.is_finite() || df <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "df_from_rate overflowed or underflowed to non-positive/non-finite DF ({df}) for {self}, rate={rate}, t={t}"
            )));
        }
        Ok(df)
    }

    /// Convert a discount factor to an interest rate for time `t` (in years).
    ///
    /// Inverts the discount-factor formula for the compounding convention of `self`:
    ///
    /// ```text
    /// Continuous: r = -ln(DF) / t
    /// Periodic(n): r = n × (DF^(-1/(n×t)) - 1)
    /// Simple: r = (1/DF - 1) / t
    /// ```
    ///
    /// # Arguments
    ///
    /// * `df` - Strictly positive, finite discount factor.
    /// * `t` - Strictly positive, finite maturity in years on the curve day-count clock.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`](crate::Error::Validation) when `df <= 0.0` or
    /// non-finite, when `t <= 0.0` or non-finite, or when the converted rate is
    /// non-finite or underflows a non-unit factor (`df != 1.0`) to zero.
    ///
    /// # References
    ///
    /// Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.),
    /// Chapter 4 — Interest Rates. `docs/REFERENCES.md#hull-options-futures`
    #[inline]
    pub fn rate_from_df(&self, df: f64, t: f64) -> crate::Result<f64> {
        if !df.is_finite() || df <= 0.0 || !t.is_finite() || t <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "rate_from_df requires finite positive df and strictly positive finite t (got df={df}, t={t})"
            )));
        }
        if df.to_bits() == 1.0_f64.to_bits() {
            return Ok(0.0);
        }
        let ln_df = if (df - 1.0).abs() < 0.5 {
            (df - 1.0).ln_1p()
        } else {
            df.ln()
        };
        if !ln_df.is_finite() {
            return Err(crate::Error::Validation(format!(
                "rate_from_df failed to compute finite log discount factor for df={df}"
            )));
        }
        let rate = match self {
            Compounding::Continuous => -ln_df / t,
            Compounding::Periodic(n) => {
                let n = f64::from(n.get());
                let c = -ln_df / t;
                if !c.is_finite() {
                    return Err(crate::Error::Validation(format!(
                        "rate_from_df continuous equivalent overflowed for df={df}, t={t}"
                    )));
                }
                let x = c / n;
                if c != 0.0 && x == 0.0 {
                    return Err(crate::Error::Validation(format!(
                        "rate_from_df intermediate scaling c/n underflowed for {self}, df={df}, t={t}"
                    )));
                }
                let r = n * x.exp_m1();
                if r / n <= -1.0 {
                    return Err(crate::Error::Validation(format!(
                        "rate_from_df inverse rounded onto invalid periodic boundary -n for {self}, df={df}, t={t}"
                    )));
                }
                r
            }
            Compounding::Simple => {
                let denom = df * t;
                let r = if denom.is_finite() && denom > 0.0 {
                    (1.0 - df) / denom
                } else {
                    (1.0 / df - 1.0) / t
                };
                if r.mul_add(t, 1.0) <= 0.0 {
                    return Err(crate::Error::Validation(format!(
                        "rate_from_df inverse rounded onto invalid simple denominator for df={df}, t={t}"
                    )));
                }
                r
            }
        };
        if !rate.is_finite() || rate == 0.0 {
            return Err(crate::Error::Validation(format!(
                "rate_from_df overflowed or underflowed to zero for non-unit df={df}, t={t}"
            )));
        }
        Ok(rate)
    }

    /// Continuous-equivalent (instantaneous) rate of `rate` quoted under `self`.
    #[inline]
    fn instantaneous_rate(&self, rate: f64) -> crate::Result<f64> {
        match self {
            Compounding::Continuous | Compounding::Simple => {
                if !rate.is_finite() {
                    return Err(crate::Error::Validation(format!(
                        "rate must be finite (got {rate})"
                    )));
                }
                Ok(if rate == 0.0 { 0.0 } else { rate })
            }
            Compounding::Periodic(n) => {
                let n = f64::from(n.get());
                let x = rate / n;
                if !x.is_finite() || x <= -1.0 {
                    return Err(crate::Error::Validation(format!(
                        "periodic compounding factor 1 + rate/n must be positive and finite (got rate={rate}, n={n})"
                    )));
                }
                if rate == 0.0 {
                    return Ok(0.0);
                }
                if x == 0.0 {
                    return Err(crate::Error::Validation(format!(
                        "periodic rate/n underflowed non-zero rate ({rate}) for n={n}"
                    )));
                }
                let c = n * x.ln_1p();
                if !c.is_finite() || c == 0.0 {
                    return Err(crate::Error::Validation(format!(
                        "instantaneous continuous rate overflowed or underflowed for rate={rate}, n={n}"
                    )));
                }
                Ok(c)
            }
        }
    }

    /// Rate quoted under `self` equivalent to the continuous rate `c` as `t → 0`.
    #[inline]
    fn rate_from_instantaneous(&self, c: f64) -> crate::Result<f64> {
        if !c.is_finite() {
            return Err(crate::Error::Validation(format!(
                "continuous rate must be finite (got {c})"
            )));
        }
        if c == 0.0 {
            return Ok(0.0);
        }
        match self {
            Compounding::Continuous | Compounding::Simple => Ok(c),
            Compounding::Periodic(n) => {
                let n = f64::from(n.get());
                let x = c / n;
                if x == 0.0 {
                    return Err(crate::Error::Validation(format!(
                        "periodic c/n underflowed non-zero continuous rate ({c}) for n={n}"
                    )));
                }
                let r = n * x.exp_m1();
                if !r.is_finite() || r == 0.0 || r / n <= -1.0 {
                    return Err(crate::Error::Validation(format!(
                        "periodic inverse rate overflowed or underflowed for c={c}, n={n}"
                    )));
                }
                Ok(r)
            }
        }
    }

    /// Continuous-rate shift produced by adding `spread` to a rate quoted
    /// under `self`.
    ///
    /// A spread quoted on a compounding basis is added to the base rate on
    /// that same basis, so the equivalent continuous shift depends on the
    /// level of the base rate:
    ///
    /// ```text
    /// Periodic(n): shift = n × ln(1 + (spread / n) × exp(-c / n))
    /// Continuous:  shift = spread
    /// ```
    ///
    /// This is how a bond-equivalent (semiannual) option-adjusted spread is
    /// applied to each short rate of a lattice built on a semiannual curve.
    /// It is an instantaneous (`t → 0`) quote restatement, not a finite-tenor
    /// `Simple` zero-spread shift.
    ///
    /// # Arguments
    ///
    /// * `continuous_rate` - Base rate as a continuously compounded decimal
    ///   annual rate (0.04 = 4%).
    /// * `spread` - Spread as a decimal on the `self` compounding basis
    ///   (0.01 = 100 bp), added to the base rate restated on that basis.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`](crate::Error::Validation) when `continuous_rate`
    /// or `spread` is non-finite, when the shifted periodic base `exp(c/n) + spread/n`
    /// is non-positive or non-finite, or when a non-zero `spread` underflows to zero
    /// or produces a non-finite shift.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_core::math::Compounding;
    ///
    /// let base = 0.04_f64;
    /// assert_eq!(Compounding::Continuous.continuous_spread_shift(base, 0.01).unwrap(), 0.01);
    ///
    /// // Semiannual: (1 + (z + s)/2) per half year, not (1 + z/2)(1 + s/2).
    /// let shift = Compounding::SEMI_ANNUAL.continuous_spread_shift(base, 0.01).unwrap();
    /// let z = 2.0 * ((base / 2.0).exp() - 1.0);
    /// let expected = 2.0 * (1.0 + (z + 0.01) / 2.0).ln() - base;
    /// assert!((shift - expected).abs() < 1e-15);
    /// ```
    #[inline]
    pub fn continuous_spread_shift(&self, continuous_rate: f64, spread: f64) -> crate::Result<f64> {
        if !continuous_rate.is_finite() || !spread.is_finite() {
            return Err(crate::Error::Validation(format!(
                "continuous_spread_shift requires finite inputs (got continuous_rate={continuous_rate}, spread={spread})"
            )));
        }
        if spread == 0.0 {
            return Ok(0.0);
        }
        let shift = match self {
            Compounding::Continuous | Compounding::Simple => spread,
            Compounding::Periodic(n) => {
                let n = f64::from(n.get());
                let c_n = continuous_rate / n;
                if c_n >= -500.0 {
                    let ratio = (spread / n) * (-c_n).exp();
                    if !ratio.is_finite() || ratio <= -1.0 {
                        return Err(crate::Error::Validation(format!(
                            "continuous_spread_shift shifted periodic base is non-positive or non-finite for c={continuous_rate}, spread={spread}, n={n}"
                        )));
                    }
                    n * ratio.ln_1p()
                } else {
                    let base = c_n.exp() + spread / n;
                    if !base.is_finite() || base <= 0.0 {
                        return Err(crate::Error::Validation(format!(
                            "continuous_spread_shift shifted periodic base is non-positive for c={continuous_rate}, spread={spread}, n={n}"
                        )));
                    }
                    n * base.ln() - continuous_rate
                }
            }
        };
        if !shift.is_finite() || shift == 0.0 {
            return Err(crate::Error::Validation(format!(
                "continuous_spread_shift overflowed or underflowed to zero for c={continuous_rate}, spread={spread}, compounding={self}"
            )));
        }
        Ok(shift)
    }

    /// Convert a rate quoted under `self` to the equivalent rate under `to`.
    ///
    /// Routes through the equivalent continuous log-rate without materializing
    /// an intermediate `DF(t)`, avoiding `exp(-r × t)` underflow/overflow at
    /// long maturities and cancellation at tiny tenors.
    ///
    /// When `t == 0.0`, returns the `t → 0` limit of the conversion.
    ///
    /// # Arguments
    ///
    /// * `rate` - Annualized interest rate as a decimal (`0.05` = 5%) quoted in `self`.
    /// * `t` - Non-negative maturity in years on the curve day-count clock.
    /// * `to` - Target compounding convention for the converted rate.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`](crate::Error::Validation) when `rate` or `t`
    /// is non-finite, `t < 0.0`, `rate` violates the domain of `self`
    /// (`1 + r/n <= 0` for `Periodic(n)` or `1 + r × t <= 0` for `Simple`), or the
    /// converted rate overflows, underflows a non-zero input to zero, or hits
    /// an invalid target domain boundary.
    ///
    /// # References
    ///
    /// Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.),
    /// Chapter 4 — Interest Rates. `docs/REFERENCES.md#hull-options-futures`
    #[inline]
    pub fn convert_rate(&self, rate: f64, t: f64, to: &Compounding) -> crate::Result<f64> {
        if !rate.is_finite() || !t.is_finite() || t < 0.0 {
            return Err(crate::Error::Validation(format!(
                "convert_rate requires finite rate and non-negative finite t (got rate={rate}, t={t})"
            )));
        }
        let continuous = match self {
            Compounding::Continuous => {
                if rate == 0.0 {
                    0.0
                } else {
                    rate
                }
            }
            Compounding::Periodic(_) => self.instantaneous_rate(rate)?,
            Compounding::Simple => {
                if t == 0.0 {
                    if rate == 0.0 { 0.0 } else { rate }
                } else {
                    let rt = rate * t;
                    if !rt.is_finite() || rt <= -1.0 {
                        return Err(crate::Error::Validation(format!(
                            "convert_rate simple source rate domain violated (1 + r*t = {}) for rate={rate}, t={t}",
                            1.0 + rt
                        )));
                    }
                    if rate == 0.0 {
                        0.0
                    } else if rt == 0.0 {
                        return Err(crate::Error::Validation(format!(
                            "convert_rate intermediate product rate*t underflowed for rate={rate}, t={t}"
                        )));
                    } else {
                        let c = rt.ln_1p() / t;
                        if !c.is_finite() || c == 0.0 {
                            return Err(crate::Error::Validation(format!(
                                "convert_rate continuous equivalent overflowed or underflowed for rate={rate}, t={t}"
                            )));
                        }
                        c
                    }
                }
            }
        };
        if self == to {
            return Ok(if rate == 0.0 { 0.0 } else { rate });
        }
        if rate == 0.0 {
            return Ok(0.0);
        }
        if t == 0.0 {
            return to.rate_from_instantaneous(continuous);
        }
        let out = match to {
            Compounding::Continuous => continuous,
            Compounding::Periodic(_) => return to.rate_from_instantaneous(continuous),
            Compounding::Simple => {
                let ct = continuous * t;
                if ct == 0.0 {
                    return Err(crate::Error::Validation(format!(
                        "convert_rate intermediate product c*t underflowed for rate={rate}, t={t}"
                    )));
                }
                let r = ct.exp_m1() / t;
                if r.mul_add(t, 1.0) <= 0.0 {
                    return Err(crate::Error::Validation(format!(
                        "convert_rate target simple denominator collapsed to non-positive for rate={rate}, t={t}"
                    )));
                }
                r
            }
        };
        if !out.is_finite() || out == 0.0 {
            return Err(crate::Error::Validation(format!(
                "convert_rate target rate overflowed or underflowed to zero for {self} -> {to}, rate={rate}, t={t}"
            )));
        }
        Ok(out)
    }
}

impl std::str::FromStr for Compounding {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "continuous" => Ok(Self::Continuous),
            "simple" => Ok(Self::Simple),
            "semi_annual" => Ok(Self::SEMI_ANNUAL),
            "quarterly" => Ok(Self::QUARTERLY),
            "monthly" => Ok(Self::MONTHLY),
            other => other
                .strip_prefix("periodic(")
                .and_then(|rest| rest.strip_suffix(')'))
                .filter(|digits| {
                    !digits.is_empty()
                        && !digits.starts_with('0')
                        && digits.bytes().all(|b| b.is_ascii_digit())
                })
                .and_then(|digits| digits.parse::<u32>().ok())
                .and_then(|n| std::num::NonZeroU32::new(n).map(Self::Periodic))
                .ok_or_else(|| {
                    crate::Error::Validation(format!(
                        "unknown compounding {other:?}; expected one of continuous, simple, \
                         semi_annual, quarterly, monthly or periodic(n)"
                    ))
                }),
        }
    }
}

impl std::fmt::Display for Compounding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Compounding::Continuous => write!(f, "continuous"),
            Compounding::Periodic(n) => match n.get() {
                2 => write!(f, "semi_annual"),
                4 => write!(f, "quarterly"),
                12 => write!(f, "monthly"),
                other => write!(f, "periodic({})", other),
            },
            Compounding::Simple => write!(f, "simple"),
        }
    }
}

/// Discount factor of one cashflow under a zero rate shifted by an additive
/// z-spread in an explicitly selected quote compounding.
///
/// Shifts the zero rate implied by `df_base` in `compounding` by `z` without
/// materializing unstable intermediate periodic rates at short tenors. The
/// compounding is the instrument's quote convention, never derived from its
/// payment frequency (record CORE-CONVENTIONS-007 #9).
///
/// # Arguments
///
/// * `df_base` - Positive, finite base-curve discount factor for the cashflow.
///   Must equal `1.0` when `t == 0.0`.
/// * `t` - Non-negative cashflow time in years on the discount curve clock.
/// * `z` - Additive decimal spread on the zero rate in `compounding`.
/// * `compounding` - Quote compounding of the zero rate and its spread.
///
/// # Errors
///
/// Returns `Error::Validation` when `df_base` is non-finite or non-positive
/// (a curve-data error), when `t` or `z` is non-finite, when `t < 0.0`, when
/// `t == 0.0` and `df_base != 1.0`, or when the shifted rate has no
/// valid discount factor in `compounding` (a periodic base
/// `1 + (rate + z) / m` or simple base `1 + (rate + z) t` that is not
/// positive), which only happens for spreads too negative to be meaningful.
pub fn z_spread_discount_factor(
    df_base: f64,
    t: f64,
    z: f64,
    compounding: Compounding,
) -> crate::Result<f64> {
    if !df_base.is_finite() || df_base <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "z_spread_discount_factor: non-positive or non-finite base discount factor ({df_base}); \
             this is a curve-data error"
        )));
    }
    if !t.is_finite() || !z.is_finite() {
        return Err(crate::Error::Validation(format!(
            "z_spread_discount_factor: non-finite input (t={t}, z={z})"
        )));
    }
    if t < 0.0 {
        return Err(crate::Error::Validation(format!(
            "z_spread_discount_factor: cashflow time t must be non-negative (got {t})"
        )));
    }
    if t == 0.0 {
        if df_base.to_bits() != 1.0_f64.to_bits() {
            return Err(crate::Error::Validation(format!(
                "z_spread_discount_factor: df_base must equal 1.0 at t == 0.0 (got {df_base})"
            )));
        }
        return Ok(1.0);
    }
    if z == 0.0 {
        return Ok(df_base);
    }

    let df = match compounding {
        Compounding::Continuous => (df_base.ln() - z * t).exp(),
        Compounding::Periodic(m) => {
            let m_f = f64::from(m.get());
            let ln_df_base = df_base.ln();
            let u = -ln_df_base / (m_f * t);
            let ln_df = if u >= -500.0 {
                // Covers all df_base <= 1.0 (including tiny tenors where u >> 709)
                // and moderate negative-rate curves without exp(u) overflow.
                let ratio = (z / m_f) * (-u).exp();
                if !ratio.is_finite() || ratio <= -1.0 {
                    return Err(crate::Error::Validation(format!(
                        "z_spread_discount_factor: non-positive periodic compounding factor \
                         (1 + (z/m)*exp(-u) = {}) for z={z:.6e}, t={t:.6e} in {compounding:?}; \
                         spread is too negative for this cashflow",
                        1.0 + ratio
                    )));
                }
                ln_df_base - (m_f * t) * ratio.ln_1p()
            } else {
                // Extreme df_base > 1.0 at tiny tenors: u < -500 so exp(u) < 1e-217.
                let base = u.exp() + z / m_f;
                if !base.is_finite() || base <= 0.0 {
                    return Err(crate::Error::Validation(format!(
                        "z_spread_discount_factor: non-positive periodic compounding base ({base}) \
                         for z={z:.6e}, t={t:.6e} in {compounding:?}; \
                         spread is too negative for this cashflow"
                    )));
                }
                -(m_f * t) * base.ln()
            };
            ln_df.exp()
        }
        Compounding::Simple => {
            let inv_df_base = 1.0 / df_base;
            let shifted_base = z.mul_add(t, inv_df_base);
            if !inv_df_base.is_finite() || !shifted_base.is_finite() || shifted_base <= 0.0 {
                return Err(crate::Error::Validation(format!(
                    "z_spread_discount_factor: non-positive simple compounding base ({shifted_base}) \
                     for z={z:.6e}, t={t:.6e} in {compounding:?}; \
                     spread is too negative for this cashflow"
                )));
            }
            1.0 / shifted_base
        }
    };

    if !df.is_finite() || df <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "z_spread_discount_factor: spread {z:.6e} gives no finite positive discount factor \
             in {compounding:?}"
        )));
    }
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compounding_constants() {
        assert_eq!(
            Compounding::SEMI_ANNUAL,
            Compounding::Periodic(NonZeroU32::new(2).expect("2 is non-zero"))
        );
        assert_eq!(
            Compounding::QUARTERLY,
            Compounding::Periodic(NonZeroU32::new(4).expect("4 is non-zero"))
        );
        assert_eq!(
            Compounding::MONTHLY,
            Compounding::Periodic(NonZeroU32::new(12).expect("12 is non-zero"))
        );
    }

    #[test]
    fn test_periods_per_year() {
        assert_eq!(Compounding::Continuous.periods_per_year(), None);
        assert_eq!(
            Compounding::Periodic(std::num::NonZeroU32::MIN).periods_per_year(),
            Some(1)
        );
        assert_eq!(Compounding::SEMI_ANNUAL.periods_per_year(), Some(2));
        assert_eq!(Compounding::QUARTERLY.periods_per_year(), Some(4));
        assert_eq!(Compounding::MONTHLY.periods_per_year(), Some(12));
        assert_eq!(Compounding::Simple.periods_per_year(), None);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", Compounding::Continuous), "continuous");
        assert_eq!(
            format!("{}", Compounding::Periodic(std::num::NonZeroU32::MIN)),
            "periodic(1)"
        );
        assert_eq!(format!("{}", Compounding::SEMI_ANNUAL), "semi_annual");
        assert_eq!(format!("{}", Compounding::QUARTERLY), "quarterly");
        assert_eq!(format!("{}", Compounding::MONTHLY), "monthly");
        assert_eq!(format!("{}", Compounding::Simple), "simple");
    }

    #[test]
    fn annual_uses_only_the_periodic_one_representation() {
        let annual = Compounding::Periodic(NonZeroU32::MIN);
        assert_eq!(
            "periodic(1)"
                .parse::<Compounding>()
                .expect("canonical label"),
            annual
        );
        assert!("annual".parse::<Compounding>().is_err());
        assert!(serde_json::from_str::<Compounding>(r#""annual""#).is_err());
        assert_eq!(
            serde_json::to_string(&annual).expect("serialize"),
            r#"{"periodic":1}"#
        );
        for rate in [-0.5_f64, -1e-12, 0.0, 1e-12, 0.05, 1.5] {
            for time in [0.25, 1.0, 10.0] {
                let df = annual.df_from_rate(rate, time).expect("valid df");
                assert!((df - (-time * rate.ln_1p()).exp()).abs() <= 1e-13 * df.max(1.0));
                assert!((annual.rate_from_df(df, time).expect("valid rate") - rate).abs() < 1e-12);
            }
            let continuous = annual
                .convert_rate(rate, 0.0, &Compounding::Continuous)
                .expect("valid conversion");
            assert!((continuous - rate.ln_1p()).abs() < 1e-14);
            assert!(
                (Compounding::Continuous
                    .convert_rate(continuous, 0.0, &annual)
                    .expect("valid back conversion")
                    - rate)
                    .abs()
                    < 1e-12
            );
        }
    }

    #[test]
    fn test_serde_roundtrip() {
        let variants = [
            Compounding::Continuous,
            Compounding::Periodic(std::num::NonZeroU32::MIN),
            Compounding::SEMI_ANNUAL,
            Compounding::Simple,
        ];

        for variant in variants {
            let json = serde_json::to_string(&variant).expect("serialize");
            let deserialized: Compounding = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(variant, deserialized);
        }
    }

    // ── Rate conversion tests ───────────────────────────────────────────

    /// All compounding conventions used in round-trip tests.
    fn all_conventions() -> Vec<Compounding> {
        vec![
            Compounding::Continuous,
            Compounding::Periodic(std::num::NonZeroU32::MIN),
            Compounding::SEMI_ANNUAL,
            Compounding::QUARTERLY,
            Compounding::MONTHLY,
            Compounding::Simple,
        ]
    }

    const TOL: f64 = 1e-12;

    #[test]
    fn test_df_rate_roundtrip_all_conventions() {
        let rate = 0.05;
        let t = 2.0;
        for conv in all_conventions() {
            let df = conv.df_from_rate(rate, t).expect("valid df");
            let recovered = conv.rate_from_df(df, t).expect("valid rate");
            assert!(
                (recovered - rate).abs() < TOL,
                "{conv}: rate round-trip failed: {recovered} vs {rate}",
            );
        }
    }

    #[test]
    fn test_rate_df_roundtrip_all_conventions() {
        let df = 0.92;
        let t = 1.5;
        for conv in all_conventions() {
            let rate = conv.rate_from_df(df, t).expect("valid rate");
            let recovered = conv.df_from_rate(rate, t).expect("valid df");
            assert!(
                (recovered - df).abs() < TOL,
                "{conv}: DF round-trip failed: {recovered} vs {df}",
            );
        }
    }

    #[test]
    fn test_convert_rate_roundtrip_cross_convention() {
        let rate = 0.06;
        let t = 3.0;
        let conventions = all_conventions();
        for from in &conventions {
            for to in &conventions {
                let converted = from.convert_rate(rate, t, to).expect("valid convert");
                let back = to.convert_rate(converted, t, from).expect("valid back");
                assert!(
                    (back - rate).abs() < TOL,
                    "{from}->{to}->{from}: convert round-trip failed: {back} vs {rate}",
                );
            }
        }
    }

    #[test]
    fn test_convert_rate_same_convention_is_identity() {
        let rate = 0.04;
        let t = 5.0;
        for conv in all_conventions() {
            let converted = conv.convert_rate(rate, t, &conv).expect("valid convert");
            assert!(
                (converted - rate).abs() < TOL,
                "{conv}: self-conversion should be identity, got {converted}",
            );
        }
    }

    #[test]
    fn test_known_value_continuous_5pct_1y() {
        let df = Compounding::Continuous
            .df_from_rate(0.05, 1.0)
            .expect("valid df");
        let expected = (-0.05_f64).exp(); // ≈ 0.951229424500714
        assert!(
            (df - expected).abs() < TOL,
            "Continuous 5% 1Y: DF = {df}, expected {expected}",
        );
    }

    #[test]
    fn test_known_value_annual_5pct_1y() {
        let df = Compounding::Periodic(std::num::NonZeroU32::MIN)
            .df_from_rate(0.05, 1.0)
            .expect("valid df");
        let expected = 1.0 / 1.05; // ≈ 0.952380952380952
        assert!(
            (df - expected).abs() < TOL,
            "Annual 5% 1Y: DF = {df}, expected {expected}",
        );
    }

    #[test]
    fn test_known_value_simple_5pct_half_year() {
        let df = Compounding::Simple
            .df_from_rate(0.05, 0.5)
            .expect("valid df");
        let expected = 1.0 / (1.0 + 0.05 * 0.5); // = 1 / 1.025
        assert!(
            (df - expected).abs() < TOL,
            "Simple 5% 0.5Y: DF = {df}, expected {expected}",
        );
    }

    #[test]
    fn test_known_value_semiannual_5pct_1y() {
        let df = Compounding::SEMI_ANNUAL
            .df_from_rate(0.05, 1.0)
            .expect("valid df");
        let expected = (1.0 + 0.05 / 2.0_f64).powf(-2.0); // (1.025)^-2
        assert!(
            (df - expected).abs() < TOL,
            "Semi-annual 5% 1Y: DF = {df}, expected {expected}",
        );
    }

    #[test]
    fn test_edge_case_t_zero() {
        for conv in all_conventions() {
            let df = conv.df_from_rate(0.10, 0.0).expect("valid at t=0");
            assert_eq!(df, 1.0, "{conv}: df_from_rate(_, 0) should be 1.0");
            assert!(
                conv.rate_from_df(0.95, 0.0).is_err(),
                "{conv}: rate_from_df(_, 0) must reject t == 0.0"
            );
        }
    }

    #[test]
    fn test_edge_case_zero_rate() {
        let t = 2.0;
        for conv in all_conventions() {
            let df = conv.df_from_rate(0.0, t).expect("valid df");
            assert!(
                (df - 1.0).abs() < TOL,
                "{conv}: zero rate should give DF = 1.0, got {df}",
            );
        }
    }

    #[test]
    fn test_edge_case_high_rate() {
        let rate = 1.0; // 100%
        let t = 1.0;
        for conv in all_conventions() {
            let df = conv.df_from_rate(rate, t).expect("valid df");
            assert!(df > 0.0, "{conv}: DF must be positive for high rate");
            assert!(df < 1.0, "{conv}: DF must be < 1 for positive rate");
            let recovered = conv.rate_from_df(df, t).expect("valid rate");
            assert!(
                (recovered - rate).abs() < TOL,
                "{conv}: high-rate round-trip failed: {recovered} vs {rate}",
            );
        }
    }

    #[test]
    fn test_convert_rate_t_zero_returns_limit() {
        // t → 0 limit of the conversion, not the old 0.0 sentinel.
        let rate = 0.05;
        let t = 0.0;

        // continuous → annual: e^r − 1
        let result = Compounding::Continuous
            .convert_rate(rate, t, &Compounding::Periodic(std::num::NonZeroU32::MIN))
            .expect("valid convert");
        assert!(
            (result - rate.exp_m1()).abs() < 1e-15,
            "continuous→annual at t=0 should be e^r - 1, got {result}"
        );

        // annual → continuous: ln(1 + r)
        let result = Compounding::Periodic(std::num::NonZeroU32::MIN)
            .convert_rate(rate, t, &Compounding::Continuous)
            .expect("valid convert");
        assert!(
            (result - rate.ln_1p()).abs() < 1e-15,
            "annual→continuous at t=0 should be ln(1+r), got {result}"
        );

        // continuous → semi-annual: 2(e^{r/2} − 1)
        let result = Compounding::Continuous
            .convert_rate(rate, t, &Compounding::SEMI_ANNUAL)
            .expect("valid convert");
        assert!(
            (result - 2.0 * (rate / 2.0).exp_m1()).abs() < 1e-15,
            "continuous→semi-annual at t=0 should be 2(e^(r/2)-1), got {result}"
        );

        // same convention: identity
        let result = Compounding::Continuous
            .convert_rate(rate, t, &Compounding::Continuous)
            .expect("valid convert");
        assert!((result - rate).abs() < 1e-15);

        let at_tiny_t = Compounding::Continuous
            .convert_rate(
                rate,
                1e-9,
                &Compounding::Periodic(std::num::NonZeroU32::MIN),
            )
            .expect("valid convert");
        let at_zero = Compounding::Continuous
            .convert_rate(rate, 0.0, &Compounding::Periodic(std::num::NonZeroU32::MIN))
            .expect("valid convert");
        assert!(
            (at_tiny_t - at_zero).abs() < 1e-7,
            "t=0 limit {at_zero} should match tiny-tenor conversion {at_tiny_t}"
        );
    }

    #[test]
    fn test_simple_df_from_rate_rejects_non_positive_denominator() {
        assert!(Compounding::Simple.df_from_rate(-2.0, 0.5).is_err());
    }

    #[test]
    fn test_simple_rate_from_df_rejects_non_positive_discount_factor() {
        assert!(Compounding::Simple.rate_from_df(0.0, 1.0).is_err());
        assert!(Compounding::Simple.rate_from_df(-0.5, 1.0).is_err());
    }

    #[test]
    fn test_compounding_t_exactly_zero_contract() {
        for conv in all_conventions() {
            assert_eq!(
                conv.df_from_rate(0.10, 0.0).expect("valid at t=0"),
                1.0,
                "{conv}: df_from_rate(_, 0.0) must be 1.0"
            );
            assert!(
                conv.rate_from_df(0.95, 0.0).is_err(),
                "{conv}: rate_from_df(_, 0.0) must error"
            );
        }
    }

    #[test]
    fn test_compounding_very_small_t_is_not_zero() {
        let t = 1.0e-10;
        for conv in all_conventions() {
            let df = conv.df_from_rate(0.10, t).expect("valid df");
            assert!(df.is_finite(), "{conv}: df must be finite for tiny t");
        }
    }

    #[test]
    fn test_rate_ordering_simple_gt_annual_gt_continuous() {
        let df = 0.90;
        let t = 2.0;
        let r_cont = Compounding::Continuous
            .rate_from_df(df, t)
            .expect("valid rate");
        let r_ann = Compounding::Periodic(std::num::NonZeroU32::MIN)
            .rate_from_df(df, t)
            .expect("valid rate");
        let r_simple = Compounding::Simple.rate_from_df(df, t).expect("valid rate");

        assert!(
            r_simple > r_ann,
            "r_simple ({r_simple}) should be > r_annual ({r_ann})",
        );
        assert!(
            r_ann > r_cont,
            "r_annual ({r_ann}) should be > r_continuous ({r_cont})",
        );
    }

    #[test]
    fn test_rate_from_df_errors_for_invalid_discount_factors() {
        let bad_dfs = [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY];
        let t = 1.0;
        for conv in all_conventions() {
            for &df in &bad_dfs {
                assert!(
                    conv.rate_from_df(df, t).is_err(),
                    "{conv}: rate_from_df({df}, {t}) should error",
                );
            }
        }
    }

    #[test]
    fn test_rate_ordering_periodic_between_annual_and_continuous() {
        let df = 0.90;
        let t = 2.0;
        let r_cont = Compounding::Continuous
            .rate_from_df(df, t)
            .expect("valid rate");
        let r_semi = Compounding::SEMI_ANNUAL
            .rate_from_df(df, t)
            .expect("valid rate");
        let r_ann = Compounding::Periodic(std::num::NonZeroU32::MIN)
            .rate_from_df(df, t)
            .expect("valid rate");

        assert!(
            r_ann > r_semi,
            "r_annual ({r_ann}) should be > r_semi ({r_semi})",
        );
        assert!(
            r_semi > r_cont,
            "r_semi ({r_semi}) should be > r_continuous ({r_cont})",
        );
    }

    #[test]
    fn test_all_conventions_agree_on_df() {
        let r_cont = 0.05;
        let t = 2.5;
        let df_expected = Compounding::Continuous
            .df_from_rate(r_cont, t)
            .expect("valid df");

        for conv in all_conventions() {
            let r_conv = Compounding::Continuous
                .convert_rate(r_cont, t, &conv)
                .expect("valid convert");
            let df_conv = conv.df_from_rate(r_conv, t).expect("valid df");
            assert!(
                (df_conv - df_expected).abs() < TOL,
                "{conv}: DF mismatch after conversion: {df_conv} vs {df_expected}",
            );
        }
    }

    #[test]
    fn compounding_fromstr_display_roundtrip() {
        use std::str::FromStr;

        let variants = [
            Compounding::Continuous,
            Compounding::Simple,
            Compounding::Periodic(std::num::NonZeroU32::MIN),
            Compounding::SEMI_ANNUAL,
            Compounding::QUARTERLY,
            Compounding::MONTHLY,
        ];
        for v in variants {
            let s = v.to_string();
            let parsed = Compounding::from_str(&s).expect("roundtrip parse should succeed");
            assert_eq!(v, parsed, "roundtrip failed for {s}");
        }
        for rejected in ["semiannual", "Semi-Annual", " semi_annual", "SEMI_ANNUAL"] {
            assert!(rejected.parse::<Compounding>().is_err());
        }
        assert!(Compounding::from_str("invalid").is_err());
    }

    #[test]
    fn periodic_label_round_trips_for_every_frequency() {
        for n in [1_u32, 3, 6, 52, 365] {
            let periodic = Compounding::Periodic(std::num::NonZeroU32::new(n).expect("non-zero"));
            let label = periodic.to_string();
            assert_eq!(
                label.parse::<Compounding>().expect("round trip"),
                periodic,
                "{label}"
            );
        }
        assert_eq!(
            Compounding::Periodic(std::num::NonZeroU32::MIN).to_string(),
            "periodic(1)"
        );
        for rejected in [
            "periodic(0)",
            "periodic(01)",
            "periodic(+2)",
            "periodic( 2)",
            "periodic()",
            "periodic(-2)",
            "periodic(2",
            "periodic 2",
        ] {
            assert!(rejected.parse::<Compounding>().is_err(), "{rejected}");
        }
    }

    #[test]
    fn z_spread_discount_factor_short_tenor_periodic_does_not_overflow() {
        let df = z_spread_discount_factor(0.998, 1e-6, 0.01, Compounding::SEMI_ANNUAL)
            .expect("short-tenor periodic z-spread should remain finite");
        assert!(df.is_finite() && df > 0.0);
        assert!((df - 0.998).abs() < 1e-12);

        let df_neg = z_spread_discount_factor(1.002, 1e-6, 0.01, Compounding::SEMI_ANNUAL)
            .expect("short-tenor DF > 1 periodic z-spread should remain finite");
        assert!(df_neg.is_finite() && df_neg > 0.0);

        let df_1y = z_spread_discount_factor(0.95, 1.0, 0.01, Compounding::SEMI_ANNUAL)
            .expect("1Y periodic z-spread");
        let base_r = Compounding::SEMI_ANNUAL
            .rate_from_df(0.95, 1.0)
            .expect("valid base rate");
        let expected_1y = Compounding::SEMI_ANNUAL
            .df_from_rate(base_r + 0.01, 1.0)
            .expect("valid shifted df");
        assert!((df_1y - expected_1y).abs() < 1e-15);
    }

    #[test]
    fn z_spread_discount_factor_validates_time_and_t_zero_df() {
        assert!(z_spread_discount_factor(0.99, -0.1, 0.01, Compounding::Continuous).is_err());
        assert!(z_spread_discount_factor(0.95, 0.0, 0.01, Compounding::Continuous).is_err());
        assert!(z_spread_discount_factor(1.0 + 1e-14, 0.0, 0.01, Compounding::Continuous).is_err());
        let at_zero = z_spread_discount_factor(1.0, 0.0, 0.01, Compounding::SEMI_ANNUAL)
            .expect("unit DF at t == 0 is valid");
        assert_eq!(at_zero, 1.0);
    }

    #[test]
    fn df_from_rate_and_convert_rate_validate_domain_at_t_zero_and_extreme_tenors() {
        assert!(Compounding::SEMI_ANNUAL.df_from_rate(-2.0, 0.0).is_err());
        assert!(Compounding::SEMI_ANNUAL.df_from_rate(-2.5, 0.0).is_err());
        assert!(
            Compounding::SEMI_ANNUAL
                .convert_rate(-2.5, 0.0, &Compounding::Continuous)
                .is_err()
        );

        for conv in all_conventions() {
            assert!(conv.df_from_rate(0.05, -0.25).is_err());
            assert!(conv.rate_from_df(0.95, -0.25).is_err());
            assert!(
                conv.convert_rate(0.05, -0.25, &Compounding::Continuous)
                    .is_err()
            );
        }

        // Long-horizon high-rate DF underflow to 0.0 is rejected, while analytical rate conversion succeeds.
        let r_semi = 5.0;
        let t_long = 500.0;
        assert!(
            Compounding::SEMI_ANNUAL
                .df_from_rate(r_semi, t_long)
                .is_err()
        );
        let r_quart = Compounding::SEMI_ANNUAL
            .convert_rate(r_semi, t_long, &Compounding::QUARTERLY)
            .expect("analytical convert preserves extreme tenors");
        assert!(r_quart.is_finite());
        let round_trip = Compounding::QUARTERLY
            .convert_rate(r_quart, t_long, &Compounding::SEMI_ANNUAL)
            .expect("round-trip convert");
        assert!((round_trip - r_semi).abs() < 1e-12);
    }
}
