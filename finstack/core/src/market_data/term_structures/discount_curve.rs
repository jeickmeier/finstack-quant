//! Discount factor curves for present value calculations.
//!
//! A discount curve represents the time value of money, mapping future dates to
//! present values. This is the fundamental building block for pricing all fixed
//! income securities and derivatives.
//!
//! # Financial Concept
//!
//! The discount factor DF(t) is the present value of $1 received at time t:
//! ```text
//! DF(t) = PV($1 at time t)
//!       = e^(-r(t) * t)
//!
//! where r(t) is the continuously compounded zero rate at maturity t
//! ```
//!
//! # Market Construction
//!
//! Discount curves are typically bootstrapped from liquid market instruments:
//! - **Money market**: Overnight rates (SOFR, €STR, SONIA)
//! - **Futures**: SOFR futures, Eurodollar futures
//! - **Swaps**: Fixed-float interest rate swaps (par rates)
//! - **Bonds**: Government bonds (when OIS not available)
//!
//! # Interpolation Methods
//!
//! The curve supports multiple interpolation schemes via [`crate::math::interp::InterpStyle`]:
//! - **Linear**: Simple, but may create arbitrage
//! - **LogLinear**: Constant zero rates between knots
//! - **MonotoneConvex**: Smooth, no-arbitrage (Hagan-West algorithm)
//! - **CubicHermite**: Shape-preserving cubic (requires monotone input for no-arb)
//! - **PiecewiseQuadraticForward**: Smooth forward curve (C²), commonly used for display
//!
//! # Use Cases
//!
//! - **Bond pricing**: Discount future coupons and principal
//! - **Swap valuation**: Mark-to-market fixed and floating legs
//! - **Option pricing**: Discount expected payoffs
//! - **Risk metrics**: DV01, duration, convexity calculation
//!
//! # Extrapolation Behavior and Limitations
//!
//! The curve supports two extrapolation policies via [`ExtrapolationPolicy`]:
//!
//! - **`FlatZero`** (conservative): Returns the discount factor at the boundary knot.
//!   Beyond the last knot, this implies zero forward rates. Use for risk management
//!   where you want to avoid assumptions about unobserved rates.
//!
//! - **`FlatForward`** (default): Extends the curve using the forward rate at the
//!   boundary. This is the market standard for production curves.
//!
//! ## Warning: Ultra-Long Tenor Extrapolation
//!
//! When extrapolating significantly beyond the last curve knot (e.g., pricing a 50Y
//! instrument from a 10Y curve), be aware of the following limitations:
//!
//! 1. **Model uncertainty**: Extrapolated forward rates are not market-implied.
//!    For tenors 2× beyond the last knot, consider the extrapolation unreliable.
//!
//! 2. **Risk sensitivity**: Greeks computed in extrapolated regions may be
//!    misleading. The curve has no sensitivity to rates beyond its last pillar.
//!
//! 3. **Regulatory considerations**: Basel III/IV and Solvency II have specific
//!    requirements for ultra-long rate extrapolation (Smith-Wilson, UFR methods).
//!    This implementation does not include regulatory extrapolation methods.
//!
//! **Best practice**: If you frequently price instruments beyond your curve's last
//! pillar, either:
//! - Extend the curve with appropriate long-dated instruments (e.g., 30Y, 50Y swaps)
//! - Use a regulatory-compliant extrapolation method for insurance/pension valuations
//! - Apply explicit haircuts or uncertainty bands to extrapolated values
//!
//! ## Example
//! ```rust
//! use finstack_core::market_data::term_structures::DiscountCurve;
//! use finstack_core::dates::Date;
//! use time::Month;
//! # use finstack_core::math::interp::InterpStyle;
//!
//! let curve = DiscountCurve::builder("USD-OIS")
//!     .base_date(Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"))
//!     .knots([(0.0, 1.0), (5.0, 0.9)])
//!     .interp(InterpStyle::MonotoneConvex)
//!     .build()
//!     .expect("DiscountCurve builder should succeed");
//! assert!(curve.df(3.0) < 1.0);
//! ```
//!
//! # References
//!
//! - **Curve Construction and Bootstrapping**:
//!   - Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.).
//!     Pearson. Chapters 4-7.
//!   - Andersen, L., & Piterbarg, V. (2010). *Interest Rate Modeling* (3 vols).
//!     Atlantic Financial Press. Volume 1, Chapters 2-3.
//!
//! - **Interpolation Methods**:
//!   - Hagan, P. S., & West, G. (2006). "Interpolation Methods for Curve Construction."
//!     *Applied Mathematical Finance*, 13(2), 89-129.
//!   - Hagan, P. S., & West, G. (2008). "Methods for Constructing a Yield Curve."
//!     *Wilmott Magazine*, May 2008.
//!
//! - **Industry Standards**:
//!   - OpenGamma (2013). "Interest Rate Instruments and Market Conventions Guide."
//!   - ISDA (2006). "2006 ISDA Definitions." Sections on discount factors and rates.

use super::common::{
    build_interp_input_error, infer_discount_curve_day_count, roll_knots, split_points,
    triangular_weight,
};
use crate::math::interp::{ExtrapolationPolicy, InterpStyle};
use crate::{
    currency::Currency,
    dates::{Date, DayCount, DayCountContext},
    market_data::traits::{Discounting, TermStructure},
    math::interp::types::Interp,
    types::CurveId,
};

/// Default minimum forward rate tenor in years (~30 seconds).
///
/// Very short tenors cause precision degradation in the formula (z2 - z1) / (t2 - t1)
/// due to catastrophic cancellation when z1*t1 ≈ z2*t2.
///
/// This constant can be overridden via [`DiscountCurveBuilder::min_forward_tenor`].
pub const DEFAULT_MIN_FORWARD_TENOR: f64 = 1e-6;

/// Market quote metadata used to build a discount curve.
///
/// This is optional sidecar data for risk calculations that need to shock the
/// original benchmark quotes and re-bootstrap instead of applying direct
/// zero-rate bumps to the fitted curve.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscountCurveRateCalibration {
    /// Rate index used by the benchmark instruments.
    pub index_id: String,
    /// Currency of the calibrated curve.
    pub currency: Currency,
    /// Benchmark rate quotes used for calibration.
    pub quotes: Vec<DiscountCurveRateQuote>,
}

/// A single benchmark rate quote used to calibrate a discount curve.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscountCurveRateQuote {
    /// Instrument type represented by the quote.
    pub quote_type: DiscountCurveRateQuoteType,
    /// Tenor string, such as `3M` or `5Y`.
    pub tenor: String,
    /// Quoted rate in decimal form.
    pub rate: f64,
}

/// Supported benchmark quote instruments for discount-curve quote metadata.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscountCurveRateQuoteType {
    /// Money-market deposit quote.
    Deposit,
    /// Interest-rate swap quote.
    Swap,
}

/// Piece-wise discount factor curve supporting several interpolation styles.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawDiscountCurve", into = "RawDiscountCurve")]
pub struct DiscountCurve {
    pub(crate) id: CurveId,
    pub(crate) base: Date,
    /// Day-count basis used to convert dates → time for discounting.
    pub(crate) day_count: DayCount,
    /// Knot times in **years**.
    pub(crate) knots: Box<[f64]>,
    /// Discount factors (unitless).
    pub(crate) dfs: Box<[f64]>,
    pub(crate) interp: Interp,
    /// Interpolation style (stored for serialization and bumping)
    pub(crate) style: InterpStyle,
    /// Extrapolation policy (stored for serialization and bumping)
    pub(crate) extrapolation: ExtrapolationPolicy,
    /// Minimum forward rate floor used during validation, if any.
    pub(crate) min_forward_rate: Option<f64>,
    /// Whether non-monotonic discount factors were explicitly allowed.
    pub(crate) allow_non_monotonic: bool,
    /// Minimum tenor for forward rate calculations (configurable)
    pub(crate) min_forward_tenor: f64,
    /// Optional market quotes used to bootstrap this curve.
    pub(crate) rate_calibration: Option<DiscountCurveRateCalibration>,
    /// Rate cut-off (business days) of the OIS compounding convention this
    /// curve was *calibrated* under, when bootstrapped with a
    /// `CompoundedWithRateCutoff` override.
    ///
    /// `None` = calibrated under a non-cut-off convention (registry default),
    /// or not calibrated from instruments at all (hand-built curve).
    ///
    /// Stored as a plain scalar (no dependency on the valuations-crate
    /// `FloatingLegCompounding` enum). Single-curve OIS pricing consults this
    /// to decide whether the `1/DF(start,end)` compounded fast path is
    /// self-consistent with the curve's own calibration. It is stamped on
    /// *both* the intermediate solver curves and the final curve so that the
    /// bootstrap-internal swap repricing and downstream pricing agree.
    pub(crate) calibration_ois_cutoff_days: Option<i32>,
    /// Opaque FX policy stamp when bootstrap used cross-currency assumptions
    /// (XCCY basis, FX triangulation). Propagated to `ResultsMeta.fx_policy_applied`
    /// for instruments that depend on this curve.
    pub(crate) fx_policy: Option<String>,
}

/// Raw serializable state of DiscountCurve
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDiscountCurve {
    /// Curve identifier
    pub id: String,
    /// Base date
    pub base: Date,
    /// Day count convention for discount time basis
    #[serde(default = "default_day_count")]
    pub day_count: DayCount,
    /// Time/value pairs used to construct the curve
    pub knot_points: Vec<(f64, f64)>,
    /// Interpolation style
    pub interp_style: InterpStyle,
    /// Extrapolation policy
    pub extrapolation: ExtrapolationPolicy,
    /// Minimum forward rate floor (if set)
    #[serde(default)]
    pub min_forward_rate: Option<f64>,
    /// Whether non-monotonic DFs are allowed (dangerous override)
    #[serde(default)]
    pub allow_non_monotonic: bool,
    /// Minimum tenor for forward rate calculations
    #[serde(default = "default_min_forward_tenor")]
    pub min_forward_tenor: f64,
    /// Optional market quotes used to bootstrap this curve.
    #[serde(default)]
    pub rate_calibration: Option<DiscountCurveRateCalibration>,
    /// OIS cut-off (business days) the curve was calibrated under, if any.
    #[serde(default)]
    pub calibration_ois_cutoff_days: Option<i32>,
    /// Opaque FX policy stamp; see [`DiscountCurve::fx_policy`]. Defaults to
    /// `None` so curve JSON written before this field existed deserializes
    /// cleanly.
    #[serde(default)]
    pub fx_policy: Option<String>,
}

fn default_min_forward_tenor() -> f64 {
    DEFAULT_MIN_FORWARD_TENOR
}

impl From<DiscountCurve> for RawDiscountCurve {
    fn from(curve: DiscountCurve) -> Self {
        let knot_points: Vec<(f64, f64)> = curve
            .knots
            .iter()
            .zip(curve.dfs.iter())
            .map(|(&t, &df)| (t, df))
            .collect();

        RawDiscountCurve {
            id: curve.id.to_string(),
            base: curve.base,
            day_count: curve.day_count,
            knot_points,
            interp_style: curve.style,
            extrapolation: curve.extrapolation,
            min_forward_rate: curve.min_forward_rate,
            allow_non_monotonic: curve.allow_non_monotonic,
            min_forward_tenor: curve.min_forward_tenor,
            rate_calibration: curve.rate_calibration,
            calibration_ois_cutoff_days: curve.calibration_ois_cutoff_days,
            fx_policy: curve.fx_policy,
        }
    }
}

impl TryFrom<RawDiscountCurve> for DiscountCurve {
    type Error = crate::Error;

    fn try_from(state: RawDiscountCurve) -> crate::Result<Self> {
        DiscountCurve::builder(state.id)
            .base_date(state.base)
            .day_count(state.day_count)
            .knots(state.knot_points)
            .interp(state.interp_style)
            .extrapolation(state.extrapolation)
            .min_forward_tenor(state.min_forward_tenor)
            .rate_calibration_opt(state.rate_calibration)
            .calibration_ois_cutoff_days_opt(state.calibration_ois_cutoff_days)
            .fx_policy_opt(state.fx_policy)
            .validation(ValidationMode::Raw {
                allow_non_monotonic: state.allow_non_monotonic,
                forward_floor: state.min_forward_rate,
            })
            .build()
    }
}

fn default_day_count() -> DayCount {
    // Default for omitted field.
    DayCount::Act365F
}

impl DiscountCurve {
    /// Unique identifier of the curve.
    #[inline]
    pub fn id(&self) -> &CurveId {
        &self.id
    }

    /// Base (valuation) date of the curve.
    #[inline]
    pub fn base_date(&self) -> Date {
        self.base
    }

    /// Day-count basis used for discount time mapping.
    #[inline]
    pub fn day_count(&self) -> DayCount {
        self.day_count
    }

    /// Interpolation style used by this curve.
    #[inline]
    pub fn interp_style(&self) -> InterpStyle {
        self.style
    }

    /// Extrapolation policy used by this curve.
    #[inline]
    pub fn extrapolation(&self) -> ExtrapolationPolicy {
        self.extrapolation
    }

    /// Market quote metadata used to build this curve, when available.
    #[inline]
    pub fn rate_calibration(&self) -> Option<&DiscountCurveRateCalibration> {
        self.rate_calibration.as_ref()
    }

    /// OIS rate cut-off (business days) this curve was calibrated under, if any.
    ///
    /// Returns `None` for curves calibrated under a non-cut-off convention or
    /// hand-built curves with no calibration provenance.
    #[inline]
    pub fn calibration_ois_cutoff_days(&self) -> Option<i32> {
        self.calibration_ois_cutoff_days
    }

    /// Opaque FX policy stamp from curve construction, if any.
    ///
    /// Propagated onto `ResultsMeta.fx_policy_applied` for dependent instruments.
    #[inline]
    pub fn fx_policy(&self) -> Option<&str> {
        self.fx_policy.as_deref()
    }

    /// Number of knot points in the curve.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.knots.len()
    }

    /// Returns `true` if the curve has no knot points.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.knots.is_empty()
    }

    /// Continuously-compounded zero rate.
    ///
    /// Formula: `r_cc = -ln(DF) / t`
    #[must_use]
    #[inline]
    pub fn zero(&self, t: f64) -> f64 {
        if t == 0.0 {
            return 0.0;
        }
        -self.df(t).ln() / t
    }

    /// Annually-compounded zero rate (bond equivalent yield convention).
    ///
    /// This is the rate quoted for most bonds and is commonly used by
    /// Bloomberg for displaying zero rates.
    ///
    /// Formula: `r_annual = DF^(-1/t) - 1`
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_core::market_data::term_structures::DiscountCurve;
    /// use finstack_core::dates::Date;
    /// use time::Month;
    ///
    /// let curve = DiscountCurve::builder("USD-OIS")
    ///     .base_date(Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"))
    ///     .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.80)])
    ///     .build()
    ///     .expect("DiscountCurve should build");
    ///
    /// // At 1Y, DF = 0.95, so annual rate = 0.95^(-1) - 1 ≈ 5.26%
    /// let annual_rate = curve.zero_annual(1.0);
    /// assert!((annual_rate - 0.0526).abs() < 0.001);
    /// ```
    #[inline]
    pub fn zero_annual(&self, t: f64) -> f64 {
        if t == 0.0 {
            return 0.0;
        }
        self.df(t).powf(-1.0 / t) - 1.0
    }

    /// Periodically-compounded zero rate with `n` compounding periods per year.
    ///
    /// Common values for `n`:
    /// - 1: Annual (same as `zero_annual`)
    /// - 2: Semi-annual (US Treasury convention)
    /// - 4: Quarterly
    /// - 12: Monthly
    ///
    /// Formula: `r_periodic = n * (DF^(-1/(n*t)) - 1)`
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_core::market_data::term_structures::DiscountCurve;
    /// use finstack_core::dates::Date;
    /// use time::Month;
    ///
    /// let curve = DiscountCurve::builder("USD-OIS")
    ///     .base_date(Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"))
    ///     .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.80)])
    ///     .build()
    ///     .expect("DiscountCurve should build");
    ///
    /// // Semi-annual compounded rate at 1Y
    /// let semi_annual_rate = curve.zero_periodic(1.0, 2);
    /// // Annual rate should equal periodic with n=1
    /// let annual_via_periodic = curve.zero_periodic(1.0, 1);
    /// assert!((curve.zero_annual(1.0) - annual_via_periodic).abs() < 1e-12);
    /// ```
    #[inline]
    pub fn zero_periodic(&self, t: f64, n: u32) -> f64 {
        if t == 0.0 || n == 0 {
            return 0.0;
        }
        let n_f = n as f64;
        n_f * (self.df(t).powf(-1.0 / (n_f * t)) - 1.0)
    }

    /// Simple interest (money market) zero rate.
    ///
    /// Returns the simple interest rate (no compounding) implied by the discount factor.
    /// This is the standard convention for money market instruments with tenors under 1 year,
    /// including deposits, CDs, T-bills, and short-term rate fixings.
    ///
    /// # Compounding Convention
    ///
    /// **Simple interest means NO compounding.** Interest accrues linearly:
    /// - Future Value = Principal × (1 + rate × time)
    /// - This differs from annually compounded rates which compound once per year
    ///
    /// # Formula
    ///
    /// ```text
    /// r_simple = (1/DF - 1) / t
    /// ```
    ///
    /// Derived from the simple interest present value formula: `DF(t) = 1 / (1 + r × t)`
    ///
    /// # Market Standards
    ///
    /// Simple interest is the market convention for:
    /// - **USD**: SOFR, Fed Funds, T-bills, CDs, deposits (< 1Y tenor)
    /// - **EUR**: €STR, Euribor fixings
    /// - **GBP**: SONIA
    /// - **Most markets**: Interbank deposits, repo rates
    ///
    /// **Day count**: Typically paired with ACT/360 (USD, EUR) or ACT/365F (GBP).
    ///
    /// # Bloomberg Equivalent
    ///
    /// This matches Bloomberg's simple interest zero rate output when compounding
    /// is set to "Simple" in curve display screens (e.g., SWPM, SWCV).
    ///
    /// # Comparison with Other Rate Conventions
    ///
    /// For a given discount factor at time t:
    /// - `zero()` returns continuously compounded rate: `r_cc = -ln(DF) / t`
    /// - `zero_annual()` returns annually compounded: `r_annual = DF^(-1/t) - 1`
    /// - `zero_simple()` returns simple interest: `r_simple = (1/DF - 1) / t`
    ///
    /// For positive rates and t > 0: `r_simple > r_annual > r_cc`
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_core::market_data::term_structures::DiscountCurve;
    /// use finstack_core::dates::Date;
    /// use time::Month;
    ///
    /// let curve = DiscountCurve::builder("USD-OIS")
    ///     .base_date(Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"))
    ///     .knots([(0.0, 1.0), (0.25, 0.99), (1.0, 0.95)])
    ///     .build()
    ///     .expect("DiscountCurve should build");
    ///
    /// // At 3M (0.25Y), DF = 0.99, so simple rate = (1/0.99 - 1) / 0.25 ≈ 4.04%
    /// let simple_rate = curve.zero_simple(0.25);
    /// assert!((simple_rate - 0.0404).abs() < 0.001);
    /// ```
    #[inline]
    pub fn zero_simple(&self, t: f64) -> f64 {
        if t == 0.0 {
            return 0.0;
        }
        (1.0 / self.df(t) - 1.0) / t
    }

    /// Continuously-compounded forward rate between `t1` and `t2`.
    ///
    /// The forward rate `f(t1, t2)` satisfies `DF(t2) = DF(t1) · exp(-f · (t2 − t1))`,
    /// so equivalently
    ///
    /// ```text
    /// f(t1, t2) = -ln(DF(t2) / DF(t1)) / (t2 - t1).
    /// ```
    ///
    /// This is the form evaluated here. The algebraically equivalent
    /// zero-rate form `(z2·t2 − z1·t1) / (t2 − t1)` (with `z·t =
    /// -ln(DF)`) round-trips each endpoint through an extra division
    /// and multiplication — two wasted ulps — and costs two `ln`
    /// evaluations instead of one. The current form avoids both and
    /// matches the canonical identity to ~1 ulp even at sub-
    /// millisecond tenors.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `t1` or `t2` is non-finite
    /// - `t2 <= t1`
    /// - `(t2 − t1) < min_forward_tenor` (configurable, default ~30 seconds) to avoid
    ///   numerical precision issues from catastrophic cancellation
    /// - either `DF(t1)` or `DF(t2)` is non-positive (pathological curve)
    ///
    /// # Configuring Minimum Tenor
    ///
    /// The minimum forward tenor can be customized when building the curve:
    /// ```ignore
    /// use finstack_core::market_data::term_structures::DiscountCurve;
    /// # use time::macros::date;
    /// # fn main() -> finstack_core::Result<()> {
    /// let curve = DiscountCurve::builder("USD")
    ///     .base_date(date!(2025-01-01))
    ///     .knots([(0.0, 1.0), (1.0, 0.95)])
    ///     .min_forward_tenor(1e-8)  // Allow very short tenors
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    #[must_use = "computed forward rate should not be discarded"]
    pub fn forward(&self, t1: f64, t2: f64) -> crate::Result<f64> {
        if !t1.is_finite() || !t2.is_finite() || t2 <= t1 {
            return Err(crate::error::InputError::Invalid.into());
        }
        if (t2 - t1) < self.min_forward_tenor {
            return Err(crate::error::InputError::Invalid.into());
        }
        let df1 = self.df(t1);
        let df2 = self.df(t2);
        if !(df1.is_finite() && df1 > 0.0 && df2.is_finite() && df2 > 0.0) {
            return Err(crate::error::InputError::Invalid.into());
        }
        Ok(-(df2 / df1).ln() / (t2 - t1))
    }

    /// Get the minimum forward tenor configured for this curve.
    #[inline]
    pub fn min_forward_tenor(&self) -> f64 {
        self.min_forward_tenor
    }

    /// Batch evaluation of discount factors for multiple times.
    #[inline]
    #[must_use]
    pub fn df_batch(&self, times: &[f64]) -> Vec<f64> {
        times.iter().map(|&t| self.df(t)).collect()
    }

    /// Fallible: discount factor on a specific date `date` using explicit day-count `dc`.
    #[inline]
    #[must_use = "computed discount factor should not be discarded"]
    pub fn df_on_date(&self, date: Date, dc: crate::dates::DayCount) -> crate::Result<f64> {
        let t = if date == self.base {
            0.0
        } else {
            dc.year_fraction(self.base, date, DayCountContext::default())?
        };
        Ok(self.df(t))
    }

    /// Fallible: discount factor on a specific date `date` using the curve's day-count.
    #[inline]
    #[must_use = "computed discount factor should not be discarded"]
    pub fn df_on_date_curve(&self, date: Date) -> crate::Result<f64> {
        let t = self.year_fraction_to(date)?;
        Ok(self.df(t))
    }

    /// Fallible: discount factor from `from` to `to` using the curve's day-count.
    ///
    /// This is the canonical helper for the common "relative DF" pattern:
    /// `DF(from→to) = DF(0→to) / DF(0→from)`.
    ///
    /// Works for both forward and backward date order. Returns `1.0` when
    /// `from == to`.
    #[inline]
    #[must_use = "computed discount factor should not be discarded"]
    pub fn df_between_dates(&self, from: Date, to: Date) -> crate::Result<f64> {
        if from == to {
            return Ok(1.0);
        }

        let df_from = self.df_on_date_curve(from)?;
        if !df_from.is_finite() || df_from <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Invalid discount factor on 'from' date ({from}): {df_from}"
            )));
        }

        let df_to = self.df_on_date_curve(to)?;
        if !df_to.is_finite() || df_to <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Invalid discount factor on 'to' date ({to}): {df_to}"
            )));
        }
        Ok(df_to / df_from)
    }

    /// Returns the zero rate for a given date with specified compounding convention.
    ///
    /// This is the unified method for obtaining zero rates under any compounding convention.
    /// It replaces the individual `zero_on_date`, `zero_annual_on_date`, `zero_periodic_on_date`,
    /// and `zero_simple_on_date` methods.
    ///
    /// # Arguments
    /// * `date` - Target date for the zero rate
    /// * `compounding` - Compounding convention (Continuous, Annual, Periodic(n), Simple)
    ///
    /// # Mathematical Formulas
    ///
    /// For a discount factor `df` and time `t`:
    ///
    /// | Compounding | Formula | Use Case |
    /// |-------------|---------|----------|
    /// | Continuous | r = -ln(df) / t | Internal calculations, curve building |
    /// | Annual | r = df^(-1/t) - 1 | Bond markets (UK, Europe) |
    /// | Periodic(n) | r = n × (df^(-1/(n×t)) - 1) | US Treasuries (n=2), corporates |
    /// | Simple | r = (1/df - 1) / t | Money market (< 1Y) |
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_core::market_data::term_structures::DiscountCurve;
    /// use finstack_core::math::Compounding;
    /// use finstack_core::dates::Date;
    /// use time::Month;
    ///
    /// let anchor = Date::from_calendar_date(2024, Month::January, 2).unwrap();
    /// // Build a flat 5% curve (df at 1Y = exp(-0.05 * 1) ≈ 0.9512)
    /// let curve = DiscountCurve::builder("USD-OIS")
    ///     .base_date(anchor)
    ///     .knots([(0.0, 1.0), (1.0, (-0.05_f64).exp())])
    ///     .build()?;
    /// let target = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    ///
    /// // Continuous rate (default for internal calculations)
    /// let r_cont = curve.zero_rate_on_date(target, Compounding::Continuous)?;
    ///
    /// // Annual rate (for European bonds)
    /// let r_ann = curve.zero_rate_on_date(target, Compounding::Annual)?;
    ///
    /// // Semi-annual rate (for US Treasuries)
    /// let r_semi = curve.zero_rate_on_date(target, Compounding::SEMI_ANNUAL)?;
    ///
    /// // Simple rate (for money market)
    /// let r_simple = curve.zero_rate_on_date(target, Compounding::Simple)?;
    /// # Ok::<(), finstack_core::Error>(())
    /// ```
    ///
    /// # Errors
    /// Returns an error if the date is before the anchor.
    #[inline]
    #[must_use = "computed zero rate should not be discarded"]
    pub fn zero_rate_on_date(
        &self,
        date: Date,
        compounding: crate::math::Compounding,
    ) -> crate::Result<f64> {
        let t = self.year_fraction_to(date)?;
        Ok(self.zero_rate(t, compounding))
    }

    /// Returns the zero rate for a given year fraction with specified compounding.
    ///
    /// This is the unified method for obtaining zero rates under any compounding convention.
    ///
    /// # Arguments
    /// * `t` - Year fraction from the anchor date
    /// * `compounding` - Compounding convention (Continuous, Annual, Periodic(n), Simple)
    ///
    /// # Edge Cases
    /// - For t = 0, all compounding conventions return 0.0 (instantaneous rate is undefined)
    #[inline]
    #[must_use]
    pub fn zero_rate(&self, t: f64, compounding: crate::math::Compounding) -> f64 {
        use crate::math::Compounding;
        match compounding {
            Compounding::Continuous => self.zero(t),
            Compounding::Annual => self.zero_annual(t),
            Compounding::Periodic(n) => self.zero_periodic(t, n.get()),
            Compounding::Simple => self.zero_simple(t),
        }
    }

    /// Simple forward rate between two dates using the curve's day-count.
    ///
    /// This is equivalent to `curve.forward(t1, t2)` where `t1` and `t2` are
    /// year fractions from base date using the curve's day-count convention.
    ///
    /// # Errors
    ///
    /// Returns an error if year fraction calculation fails or if the forward
    /// rate calculation fails.
    #[inline]
    #[must_use = "computed forward rate should not be discarded"]
    pub fn forward_on_dates(&self, d1: Date, d2: Date) -> crate::Result<f64> {
        let t1 = self.year_fraction_to(d1)?;
        let t2 = self.year_fraction_to(d2)?;
        self.forward(t1, t2)
    }

    /// Helper: compute year fraction from base date to target date using curve's day-count.
    #[inline]
    fn year_fraction_to(&self, date: Date) -> crate::Result<f64> {
        super::common::year_fraction_to(self.base, date, self.day_count)
    }

    /// Rebuild only the interpolator from the current knots and discount factors.
    ///
    /// Skips sort/validation -- caller must ensure data invariants hold.
    ///
    /// # Performance
    ///
    /// This call clones `self.knots` and `self.dfs` (both `Box<[f64]>`) into
    /// a fresh interpolator, since the interpolator consumes its inputs.
    /// On scenario × pillar × curve bump cycles this is on the hot path
    /// (one clone-pair per bump). The planned long-term fix is to migrate
    /// the storage type to `Arc<[f64]>` and make the interpolator accept
    /// `Arc<[f64]>` so this clone becomes a refcount bump; that touches
    /// ~47 call sites across `math::interp` and all curve types and is
    /// deferred to a follow-up change-set, gated by the contention
    /// benchmarks (`benches/curve_bumps_parallel.rs`, future).
    fn rebuild_interp(&mut self) -> crate::Result<()> {
        self.interp = super::common::build_interp_input_error(
            self.style,
            self.knots.clone(),
            self.dfs.clone(),
            self.extrapolation,
            true,
        )?;
        Ok(())
    }

    /// Apply a bump specification in-place, mutating values and rebuilding the interpolator.
    ///
    /// This avoids allocating intermediate `Vec<(f64, f64)>`, skips ID generation,
    /// and skips sort/validation (bumps preserve knot ordering).
    pub(crate) fn bump_in_place(
        &mut self,
        spec: &crate::market_data::bumps::BumpSpec,
    ) -> crate::Result<()> {
        use crate::market_data::bumps::BumpType;

        let (val, is_multiplicative) = spec.resolve_standard_values().ok_or_else(|| {
            crate::error::InputError::UnsupportedBump {
                reason: format!(
                    "DiscountCurve only supports Additive/{{RateBp,Percent,Fraction}} bumps, got {:?}/{:?}",
                    spec.mode, spec.units
                ),
            }
        })?;
        if is_multiplicative {
            return Err(crate::error::InputError::UnsupportedBump {
                reason: "DiscountCurve does not support Multiplicative bumps".to_string(),
            }
            .into());
        }
        let bump_rate = val;

        match spec.bump_type {
            BumpType::Parallel => {
                for (df, &t) in self.dfs.iter_mut().zip(self.knots.iter()) {
                    *df *= (-bump_rate * t).exp();
                }
            }
            BumpType::TriangularKeyRate {
                prev_bucket,
                target_bucket,
                next_bucket,
            } => {
                for (df, &t) in self.dfs.iter_mut().zip(self.knots.iter()) {
                    let weight = super::common::triangular_weight(
                        t,
                        prev_bucket,
                        target_bucket,
                        next_bucket,
                    );
                    *df *= (-bump_rate * weight * t).exp();
                }
            }
        }
        self.rebuild_interp()
    }

    /// Create a new curve with a parallel rate bump applied in basis points (fallible).
    ///
    /// Uses df_bumped(t) = df_original(t) * exp(-bump * t), where bump = bp / 10_000.
    ///
    /// Returns an error if the bumped curve violates validation constraints.
    pub fn with_parallel_bump(&self, bp: f64) -> crate::Result<Self> {
        let bump_rate = bp / 10_000.0;
        let bumped_points: Vec<(f64, f64)> = self
            .knots
            .iter()
            .zip(self.dfs.iter())
            .map(|(&t, &df)| (t, df * (-bump_rate * t).exp()))
            .collect();

        // Derive new ID with suffix
        let new_id = crate::market_data::bumps::id_bump_bp(self.id.as_str(), bp);

        // Rebuild preserving the full metadata (interpolation, extrapolation,
        // calibration settings, fx_policy, non-monotonic settings).
        self.metadata_builder(new_id).knots(bumped_points).build()
    }

    /// Create a new curve with a triangular key-rate bump using explicit bucket neighbors.
    ///
    /// This is the market-standard key-rate DV01 implementation (per Tuckman/Fabozzi)
    /// where the triangular weight is defined by the **bucket grid**, not curve knots.
    /// This ensures that the sum of all bucketed DV01s equals the parallel DV01.
    ///
    /// # Mathematical Foundation
    ///
    /// For a zero rate bump δr applied with triangular weight w(t):
    /// ```text
    /// DF_bumped(t) = DF(t) × exp(-w(t) × δr × t)
    /// ```
    ///
    /// The triangular weight function for an **interior** bucket at `target`
    /// with neighbours `prev = Some(p)` and `next = Some(n)`:
    /// - w(t) = 0                                    if t ≤ p
    /// - w(t) = (t − p) / (target − p)               if p < t ≤ target
    /// - w(t) = (n − t) / (n − target)               if target < t < n
    /// - w(t) = 0                                    if t ≥ n
    ///
    /// For the **first bucket** (`prev = None`) the rising edge is replaced
    /// by a flat 1.0 for `t ≤ target`; for the **last bucket**
    /// (`next = None`) the falling edge is replaced by a flat 1.0 for
    /// `t > target`.
    ///
    /// # Key Property: Unity Partition
    ///
    /// When `prev = None` is used for the first bucket and `next = None`
    /// for the last bucket, the weights of the full bucket set sum to 1.0
    /// at any time t covered by any bucket:
    /// `Σᵢ wᵢ(t) = 1.0`
    ///
    /// This ensures: **sum of bucketed DV01 = parallel DV01**.
    ///
    /// # Arguments
    /// * `prev_bucket` - Previous bucket time in years; `None` for the first bucket
    /// * `target_bucket` - Target bucket time in years (peak of the triangle)
    /// * `next_bucket` - Next bucket time in years; `None` for the last bucket
    /// * `bp` - Bump size in basis points (100bp = 1%)
    ///
    /// # Returns
    /// A new discount curve with the triangular key-rate bump applied.
    ///
    /// # Errors
    /// Returns an error if the bumped curve violates validation constraints.
    ///
    /// # Examples
    /// ```ignore
    /// use finstack_core::market_data::term_structures::DiscountCurve;
    /// use time::macros::date;
    /// # fn main() -> finstack_core::Result<()> {
    ///
    /// let base_date = date!(2025 - 01 - 01);
    /// let curve = DiscountCurve::builder("USD_OIS")
    ///     .base_date(base_date)
    ///     .knots(vec![(1.0, 0.98), (2.0, 0.96), (5.0, 0.90), (10.0, 0.80)])
    ///     .build()
    ///     ?;
    ///
    /// // Apply 10bp bump at 5Y interior bucket with neighbours at 3Y and 7Y
    /// let bumped = curve.with_triangular_key_rate_bump_neighbors(
    ///     Some(3.0), 5.0, Some(7.0), 10.0,
    /// )?;
    /// # let _ = bumped;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_triangular_key_rate_bump_neighbors(
        &self,
        prev_bucket: Option<f64>,
        target_bucket: f64,
        next_bucket: Option<f64>,
        bp: f64,
    ) -> crate::Result<Self> {
        if self.knots.len() < 2 {
            return self.with_parallel_bump(bp);
        }

        // Validate bucket grid ordering. Each finite bound must satisfy
        // prev < target < next.
        if !target_bucket.is_finite() {
            return Err(crate::error::InputError::Invalid.into());
        }
        if let Some(p) = prev_bucket {
            if !p.is_finite() || p >= target_bucket {
                return Err(crate::error::InputError::Invalid.into());
            }
        }
        if let Some(n) = next_bucket {
            if !n.is_finite() || target_bucket >= n {
                return Err(crate::error::InputError::Invalid.into());
            }
        }

        let bump_rate = bp / 10_000.0;
        let bumped_points: Vec<(f64, f64)> = self
            .knots
            .iter()
            .zip(self.dfs.iter())
            .map(|(&knot_t, &df)| {
                // Triangular weight based on BUCKET grid (not curve knots!)
                let weight = triangular_weight(knot_t, prev_bucket, target_bucket, next_bucket);
                // r_bumped = r + w × δr
                // DF_bumped = exp(-r_bumped × t) = DF × exp(-w × δr × t)
                (knot_t, df * (-bump_rate * weight * knot_t).exp())
            })
            .collect();

        let new_id = crate::market_data::bumps::id_bump_bp(self.id.as_str(), bp);
        // Rebuild preserving the full metadata (including fx_policy).
        self.metadata_builder(new_id).knots(bumped_points).build()
    }

    /// Roll the curve forward by a specified number of days, realizing forwards.
    ///
    /// This creates a new curve with:
    /// - Base date advanced by `days`
    /// - Knot times shifted backwards (t' = t - dt_years)
    /// - Points with t' <= 0 are filtered out (expired)
    /// - Discount factors renormalized by the realized forward:
    ///   `DF_new(t - dt) = DF_old(t) / DF_old(dt)`
    ///
    /// These are **realized-forward** semantics (per the 2026-06-09 core quant
    /// review): forwards realize as the curve rolls, so a flat curve stays
    /// flat under roll, and a roll-then-reprice theta captures both carry and
    /// roll-down. The relationship to present values is
    /// `PV(rolled curve, T - dt) = PV(old curve, T) / DF_old(dt)` — i.e. the
    /// rolled PV is the forward value of the old PV to the new base date.
    /// This aligns the discount curve with the hazard, forward, inflation,
    /// and price/vol-index curve rolls, which already realize forwards.
    ///
    /// # Arguments
    /// * `days` - Number of days to roll forward
    ///
    /// # Returns
    /// A new discount curve with updated base date and renormalized knots.
    ///
    /// # Errors
    /// Returns an error if fewer than 2 knot points remain after filtering
    /// expired points, or if `DF_old(dt)` is not positive and finite (which
    /// can only happen if extrapolation past the last knot misbehaves).
    ///
    /// # Examples
    /// ```ignore
    /// use finstack_core::market_data::term_structures::DiscountCurve;
    /// use time::macros::date;
    /// # fn main() -> finstack_core::Result<()> {
    ///
    /// let base_date = date!(2025 - 01 - 01);
    /// let curve = DiscountCurve::builder("USD_OIS")
    ///     .base_date(base_date)
    ///     .knots(vec![(0.5, 0.99), (1.0, 0.98), (2.0, 0.96), (5.0, 0.90)])
    ///     .build()
    ///     ?;
    ///
    /// // Roll 6 months forward - the 0.5Y point expires
    /// let rolled = curve.roll_forward(182)?;
    /// assert!(rolled.knots().len() < curve.knots().len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn roll_forward(&self, days: i64) -> crate::Result<Self> {
        let new_base = self.base + time::Duration::days(days);
        let dt_years =
            self.day_count
                .year_fraction(self.base, new_base, DayCountContext::default())?;

        // Realized-forward renormalization: divide every rolled DF by the
        // old curve's DF at the roll horizon, interpolated in the curve's own
        // time basis (the same `dt_years` the knots are shifted by).
        let df_dt = self.df(dt_years);
        if !df_dt.is_finite() || df_dt <= 0.0 {
            return Err(crate::error::InputError::NonPositiveValue.into());
        }

        let rolled_points: Vec<(f64, f64)> = roll_knots(&self.knots, &self.dfs, dt_years)
            .into_iter()
            .map(|(t, df)| (t, df / df_dt))
            .collect();

        if rolled_points.len() < 2 {
            return Err(crate::error::InputError::TooFewPoints.into());
        }

        // Note: knots inside (0, dt] are dropped by `roll_knots` (expired).
        // `build()` re-prepends a (0.0, 1.0) knot, which is now exactly
        // correct: DF_new(0) = DF_old(dt) / DF_old(dt) = 1.

        // Thread the full metadata (including fx_policy) and override the base.
        self.metadata_builder(self.id.clone())
            .base_date(new_base)
            .knots(rolled_points)
            .build()
    }

    /// Discount factor at time `t` (helper calling the underlying interpolator).
    #[must_use]
    #[inline]
    pub fn df(&self, t: f64) -> f64 {
        self.interp.interp(t)
    }

    /// Raw knot times (t) in **years** passed at construction.
    #[inline]
    pub fn knots(&self) -> &[f64] {
        &self.knots
    }

    /// Raw discount factors corresponding to each knot.
    #[inline]
    pub fn dfs(&self) -> &[f64] {
        &self.dfs
    }

    /// Builder entry-point.
    ///
    /// Takes the curve identifier as a required argument because every curve
    /// is uniquely keyed by its `CurveId`, and the remaining parameters
    /// (`base`, `day_count`, interpolation, etc.) all have sensible defaults.
    /// This makes `DiscountCurve::builder("USD-OIS")` both concise and
    /// self-documenting.
    ///
    /// **Design note:** This `Type::builder(id)` pattern is used consistently
    /// across all `finstack-core` term structures (discount, forward, hazard,
    /// inflation, price, vol-index, vol-surface, base-correlation). Instrument
    /// types in `finstack-valuations` use a different convention —
    /// `Type::builder()` with no args — because instruments have many
    /// required fields where named setters are more practical than positional
    /// arguments. See the `FinancialBuilder` derive macro docs for the full
    /// rationale.
    ///
    /// **Note:** Monotonic discount factor validation is enabled by default to ensure
    /// no-arbitrage conditions. Use [`DiscountCurveBuilder::validation`] with
    /// [`ValidationMode::Raw`] if you need to disable this validation (not
    /// recommended for production use).
    ///
    /// **Defaults:** The builder infers a market day-count from the curve ID when
    /// possible (for example `USD-OIS -> Act360`, `GBP-SONIA -> Act365F`). Synthetic
    /// IDs without a market hint fall back to `Act365F`. Interpolation defaults to
    /// MonotoneConvex with FlatForward extrapolation.
    ///
    /// **Build-vs-query basis trap:** the day-count basis is used both to convert
    /// dated pillars to year fractions at build time and to convert query dates
    /// back at lookup time. Because inference is substring-based, *renaming* the
    /// curve ID (e.g. `USD-SOFR` → `OIS-1`) can silently change the inferred
    /// basis and shift every pillar time by ~1.4% (Act/360 vs Act/365F). When the
    /// basis matters, set [`DiscountCurveBuilder::day_count`] explicitly instead
    /// of relying on inference; each inference is logged at `debug` level.
    ///
    /// **Negative rates:** the default [`ValidationMode::MarketStandard`] enforces
    /// monotonic discount factors with a -50bp implied-forward floor. For deeply
    /// negative-rate markets (CHF, JPY, EUR historical), pass
    /// [`ValidationMode::NegativeRateFriendly`] (or `Raw`) via
    /// [`DiscountCurveBuilder::validation`].
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> DiscountCurveBuilder {
        let id: CurveId = id.into();
        let day_count = infer_discount_curve_day_count(id.as_str());
        DiscountCurveBuilder {
            id,
            base: None,
            day_count,
            points: Vec::new(),
            style: InterpStyle::MonotoneConvex,
            extrapolation: ExtrapolationPolicy::FlatForward,
            min_forward_rate: None,     // No floor by default
            allow_non_monotonic: false, // Strict validation by default
            min_forward_tenor: DEFAULT_MIN_FORWARD_TENOR, // Default ~30 seconds
            rate_calibration: None,
            calibration_ois_cutoff_days: None,
            fx_policy: None,
        }
    }

    /// Create a builder pre-populated with this curve's data but a new ID.
    pub fn to_builder_with_id(&self, new_id: impl Into<CurveId>) -> DiscountCurveBuilder {
        self.metadata_builder(new_id)
            .knots(self.knots.iter().copied().zip(self.dfs.iter().copied()))
    }

    /// Builder pre-populated with this curve's full metadata but **no** knots.
    /// Shared by all rebuild-style operations (bumps, rolls) so that no
    /// metadata field (day-count, interpolation, extrapolation, calibration
    /// settings, fx_policy, non-monotonic settings) is dropped.
    pub(crate) fn metadata_builder(&self, new_id: impl Into<CurveId>) -> DiscountCurveBuilder {
        DiscountCurve::builder(new_id)
            .base_date(self.base)
            .day_count(self.day_count)
            .interp(self.style)
            .extrapolation(self.extrapolation)
            .min_forward_tenor(self.min_forward_tenor)
            .rate_calibration_opt(self.rate_calibration.clone())
            .calibration_ois_cutoff_days_opt(self.calibration_ois_cutoff_days)
            .fx_policy_opt(self.fx_policy.clone())
            .apply_non_monotonic_settings(self.allow_non_monotonic, self.min_forward_rate)
    }

    /// Create a forward curve from this discount curve.
    ///
    /// For single-curve bootstrapping, this creates a forward curve from the
    /// discount factors using the formula:
    /// f(t) = -d/dt[ln(DF(t))] = -1/DF(t) * dDF/dt
    ///
    /// For discrete points, we use: f(t) ≈ (DF(t) - DF(t+dt)) / (dt * DF(t+dt))
    ///
    /// # Arguments
    ///
    /// * `forward_id` - Identifier for the resulting forward curve
    /// * `tenor_years` - Tenor of the forward rate in years
    /// * `interp_style` - Optional interpolation style; defaults to `MonotoneConvex` if `None`
    pub fn to_forward_curve(
        &self,
        forward_id: impl Into<CurveId>,
        tenor_years: f64,
        interp_style: Option<InterpStyle>,
    ) -> crate::Result<super::forward_curve::ForwardCurve> {
        use super::forward_curve::ForwardCurve;

        // Default to the discount curve's own interpolation style
        // (`MonotoneConvex`) so the derived forward curve is shape-consistent
        // with its parent rather than defaulting to plain linear.
        let style = interp_style.unwrap_or(InterpStyle::MonotoneConvex);

        // Calculate forward rates at each knot point
        let mut forward_rates = Vec::with_capacity(self.knots.len());

        // Ensure we have enough points
        if self.knots.len() < 2 {
            return Err(crate::error::InputError::TooFewPoints.into());
        }

        for i in 0..self.knots.len() {
            let t = self.knots[i];
            // Allow exact float comparison for well-known sentinel values (t=0, DF(0)=1).
            #[allow(clippy::float_cmp)]
            let forward_rate = if i == 0 {
                // First point: use next point for forward difference
                let t_next = self.knots[1];
                let df = self.dfs[0];
                let df_next = self.dfs[1];
                let dt = t_next - t;

                if dt > 0.0 && df_next > 0.0 && df > 0.0 {
                    // Forward to the next point. When t = 0 and DF(0) = 1 this
                    // reduces to -ln(DF_next)/t_next (the spot rate to the next
                    // point), so no separate t=0 case is needed.
                    (df / df_next).ln() / dt
                } else if t > 0.0 && df > 0.0 {
                    // Use spot rate
                    (-df.ln()) / t
                } else {
                    return Err(crate::error::InputError::Invalid.into());
                }
            } else if i < self.knots.len() - 1 {
                // Interior points: average of left and right segment forward rates
                // to reduce bias from non-uniform knot spacing.
                // f_left  = ln(DF_{i-1}/DF_i) / (t_i - t_{i-1})
                // f_right = ln(DF_i/DF_{i+1}) / (t_{i+1} - t_i)
                // f_i ≈ 0.5 * (f_left + f_right)
                let t_prev = self.knots[i - 1];
                let t_next = self.knots[i + 1];
                let df_prev = self.dfs[i - 1];
                let df_curr = self.dfs[i];
                let df_next = self.dfs[i + 1];

                let dt_left = t - t_prev;
                let dt_right = t_next - t;
                if dt_left > 0.0
                    && dt_right > 0.0
                    && df_prev > 0.0
                    && df_curr > 0.0
                    && df_next > 0.0
                {
                    let f_left = (df_prev / df_curr).ln() / dt_left;
                    let f_right = (df_curr / df_next).ln() / dt_right;
                    0.5 * (f_left + f_right)
                } else {
                    return Err(crate::error::InputError::Invalid.into());
                }
            } else {
                // Last point: use backward difference
                let t_prev = self.knots[i - 1];
                let df = self.dfs[i];
                let df_prev = self.dfs[i - 1];
                let dt = t - t_prev;

                if dt > 0.0 && df > 0.0 && df_prev > 0.0 {
                    (df_prev / df).ln() / dt
                } else {
                    return Err(crate::error::InputError::Invalid.into());
                }
            };

            forward_rates.push((t, forward_rate));
        }

        // Build forward curve with the specified interpolation style
        ForwardCurve::builder(forward_id, tenor_years)
            .base_date(self.base)
            .day_count(self.day_count)
            .knots(forward_rates)
            .interp(style)
            .build()
    }
}

// -----------------------------------------------------------------------------
// Minimal trait implementation for polymorphism where needed
// -----------------------------------------------------------------------------

impl Discounting for DiscountCurve {
    #[inline]
    fn base_date(&self) -> Date {
        self.base
    }

    #[inline]
    fn df(&self, t: f64) -> f64 {
        self.interp.interp(t)
    }

    #[inline]
    fn day_count(&self) -> DayCount {
        self.day_count
    }
}

impl TermStructure for DiscountCurve {
    #[inline]
    fn id(&self) -> &CurveId {
        &self.id
    }
}

// -----------------------------------------------------------------------------
// Builder
// -----------------------------------------------------------------------------

/// Validation preset for [`DiscountCurveBuilder::validation`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ValidationMode {
    /// Enforce monotonic (non-increasing) discount factors and a -50bp
    /// forward-rate floor. This is the recommended mode for production curves.
    MarketStandard,
    /// Relax monotonicity to support negative-rate regimes while keeping a
    /// safety floor on implied forwards.
    NegativeRateFriendly {
        /// Minimum allowed implied forward rate (in decimal).
        forward_floor: f64,
    },
    /// Fully raw mode for solver / calibration use: explicit over both
    /// monotonicity and (optional) forward-rate floor.
    Raw {
        /// Skip monotonicity checks when `true`.
        allow_non_monotonic: bool,
        /// Optional implied forward-rate floor.
        forward_floor: Option<f64>,
    },
}

/// Fluent builder for [`DiscountCurve`].
///
/// Typical usage chains `base_date`, `knots`, and `interp` (optional)
/// before calling [`DiscountCurveBuilder::build`].
///
/// # Examples
/// ```rust
/// use finstack_core::market_data::term_structures::DiscountCurve;
/// use finstack_core::math::interp::InterpStyle;
/// use finstack_core::dates::Date;
/// use time::Month;
///
/// let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
/// let curve = DiscountCurve::builder("USD-OIS")
///     .base_date(base)
///     .knots([(0.0, 1.0), (5.0, 0.9)])
///     .interp(InterpStyle::Linear)
///     .build()
///     .expect("DiscountCurve builder should succeed");
/// assert!(curve.df(2.0) < 1.0);
/// ```
pub struct DiscountCurveBuilder {
    pub(crate) id: CurveId,
    /// Valuation / base date. `None` until [`Self::base_date`] is called;
    /// [`Self::build`] requires `Some(_)` and errors on `None`.
    pub(crate) base: Option<Date>,
    pub(crate) day_count: DayCount,
    pub(crate) points: Vec<(f64, f64)>, // (t, df)
    pub(crate) style: InterpStyle,
    pub(crate) extrapolation: ExtrapolationPolicy,
    pub(crate) min_forward_rate: Option<f64>,
    pub(crate) allow_non_monotonic: bool,
    pub(crate) min_forward_tenor: f64,
    pub(crate) rate_calibration: Option<DiscountCurveRateCalibration>,
    pub(crate) calibration_ois_cutoff_days: Option<i32>,
    pub(crate) fx_policy: Option<String>,
}

impl DiscountCurveBuilder {
    /// Override the default **base date** (valuation date).
    pub fn base_date(mut self, d: Date) -> Self {
        self.base = Some(d);
        self
    }
    /// Choose the day-count basis for discount time mapping.
    pub fn day_count(mut self, dc: DayCount) -> Self {
        self.day_count = dc;
        self
    }
    /// Supply knot points `(t, df)` where *t* is the year fraction and *df*
    /// the discount factor.
    pub fn knots<I>(mut self, pts: I) -> Self
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        self.points.extend(pts);
        self
    }
    /// Select interpolation style for this curve.
    pub fn interp(mut self, style: InterpStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the extrapolation policy for out-of-bounds evaluation.
    pub fn extrapolation(mut self, policy: ExtrapolationPolicy) -> Self {
        self.extrapolation = policy;
        self
    }

    /// Select the validation policy for the curve.
    pub fn validation(mut self, mode: ValidationMode) -> Self {
        match mode {
            ValidationMode::MarketStandard => {
                self.allow_non_monotonic = false;
                self.min_forward_rate = Some(-0.005);
            }
            ValidationMode::NegativeRateFriendly { forward_floor } => {
                self.allow_non_monotonic = true;
                self.min_forward_rate = Some(forward_floor);
            }
            ValidationMode::Raw {
                allow_non_monotonic,
                forward_floor,
            } => {
                self.allow_non_monotonic = allow_non_monotonic;
                self.min_forward_rate = forward_floor;
            }
        }
        self
    }

    /// Set a custom minimum tenor for forward rate calculations.
    ///
    /// The forward rate calculation `f(t1, t2) = (z2*t2 - z1*t1) / (t2 - t1)` suffers
    /// from catastrophic cancellation when `(t2 - t1)` is very small. This threshold
    /// prevents such precision issues.
    ///
    /// # Default
    ///
    /// The default value is [`DEFAULT_MIN_FORWARD_TENOR`](crate::market_data::term_structures::DEFAULT_MIN_FORWARD_TENOR)
    /// (~30 seconds or 1e-6 years).
    ///
    /// # Use Cases
    ///
    /// - Set to a smaller value (e.g., `1e-8`) for high-frequency intraday operations
    /// - Set to a larger value (e.g., `1e-4`) for daily curve operations with coarse data
    ///
    /// # Example
    ///
    /// ```ignore
    /// use finstack_core::market_data::term_structures::DiscountCurve;
    /// # use time::macros::date;
    /// # fn main() -> finstack_core::Result<()> {
    /// let curve = DiscountCurve::builder("USD")
    ///     .base_date(date!(2025-01-01))
    ///     .knots([(0.0, 1.0), (1.0, 0.95)])
    ///     .min_forward_tenor(1e-8)  // Allow sub-second tenors
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn min_forward_tenor(mut self, tenor: f64) -> Self {
        self.min_forward_tenor = tenor;
        self
    }

    /// Attach market quote metadata used to bootstrap this curve.
    pub fn rate_calibration(mut self, calibration: DiscountCurveRateCalibration) -> Self {
        self.rate_calibration = Some(calibration);
        self
    }

    /// Optionally attach market quote metadata used to bootstrap this curve.
    pub fn rate_calibration_opt(
        mut self,
        calibration: Option<DiscountCurveRateCalibration>,
    ) -> Self {
        self.rate_calibration = calibration;
        self
    }

    /// Record the OIS rate cut-off (business days) this curve was calibrated
    /// under. Pass `Some(days)` only when the bootstrap used a
    /// `CompoundedWithRateCutoff` convention; leave unset otherwise.
    pub fn calibration_ois_cutoff_days_opt(mut self, cutoff_days: Option<i32>) -> Self {
        self.calibration_ois_cutoff_days = cutoff_days;
        self
    }

    /// Stamp an opaque FX policy on the curve.
    ///
    /// Use when the bootstrap involved an FX-sensitive assumption (XCCY basis
    /// adjustment, FX matrix triangulation, etc.) and the policy must be
    /// surfaced on downstream valuation result envelopes.
    pub fn fx_policy(mut self, policy: impl Into<String>) -> Self {
        self.fx_policy = Some(policy.into());
        self
    }

    /// Optionally stamp an FX policy. `None` is a no-op (the field stays
    /// unset). Used by the serde round-trip path.
    pub fn fx_policy_opt(mut self, policy: Option<String>) -> Self {
        self.fx_policy = policy;
        self
    }

    pub(crate) fn apply_non_monotonic_settings(
        mut self,
        allow_non_monotonic: bool,
        min_forward_rate: Option<f64>,
    ) -> Self {
        self.allow_non_monotonic = allow_non_monotonic;
        self.min_forward_rate = min_forward_rate;
        self
    }

    /// Build the curve with minimal validation for solver use.
    ///
    /// This method skips monotonicity validation and forward rate checks, providing
    /// faster curve construction for iterative solving where the curve is temporary.
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    /// - At least 2 knot points are provided
    /// - All discount factors are positive
    /// - Knots are sorted in ascending order
    ///
    /// This is an internal optimization for calibration solvers.
    /// For general use, prefer [`Self::build`] which includes full validation.
    #[doc(hidden)]
    pub fn build_for_solver(self) -> crate::Result<DiscountCurve> {
        let base = self.base.ok_or(crate::error::InputError::Invalid)?;
        if self.points.len() < 2 {
            return Err(crate::error::InputError::TooFewPoints.into());
        }

        if self.points.iter().any(|&(_, df)| df <= 0.0) {
            return Err(crate::error::InputError::NonPositiveValue.into());
        }

        let (knots_vec, dfs_vec): (Vec<f64>, Vec<f64>) = split_points(self.points);

        let knots = knots_vec.into_boxed_slice();
        let dfs = dfs_vec.into_boxed_slice();

        let interp = build_interp_input_error(
            self.style,
            knots.clone(),
            dfs.clone(),
            self.extrapolation,
            true,
        )?;

        Ok(DiscountCurve {
            id: self.id,
            base,
            day_count: self.day_count,
            knots,
            dfs,
            interp,
            style: self.style,
            extrapolation: self.extrapolation,
            min_forward_rate: self.min_forward_rate,
            allow_non_monotonic: self.allow_non_monotonic,
            min_forward_tenor: self.min_forward_tenor,
            rate_calibration: self.rate_calibration,
            calibration_ois_cutoff_days: self.calibration_ois_cutoff_days,
            fx_policy: self.fx_policy,
        })
    }

    /// Validate input and create the [`DiscountCurve`].
    ///
    /// If the first knot time is `> 0.0`, automatically prepends `(0.0, 1.0)` to
    /// ensure the round-trip invariant `DF(0) = 1.0` (ISDA/QuantLib standard).
    pub fn build(mut self) -> crate::Result<DiscountCurve> {
        let base = self.base.ok_or(crate::error::InputError::Invalid)?;
        if !self.points.is_empty() {
            self.points.sort_by(|a, b| a.0.total_cmp(&b.0));
            let first_t = self.points[0].0;
            if first_t > 1e-14 {
                self.points.insert(0, (0.0, 1.0));
            }
        }

        if self.points.len() < 2 {
            return Err(crate::error::InputError::TooFewPoints.into());
        }
        if self.points.iter().any(|&(_, df)| df <= 0.0) {
            return Err(crate::error::InputError::NonPositiveValue.into());
        }

        let (knots_vec, dfs_vec): (Vec<f64>, Vec<f64>) = split_points(self.points);
        crate::math::interp::utils::validate_knots(&knots_vec)?;

        if !self.allow_non_monotonic {
            validate_monotonic_df(&knots_vec, &dfs_vec)?;
        } else if self.style == InterpStyle::MonotoneConvex {
            validate_monotone_convex_compatible_df(&knots_vec, &dfs_vec)?;
        }

        if let Some(min_fwd) = self.min_forward_rate {
            validate_forward_rates(&knots_vec, &dfs_vec, min_fwd)?;
        }

        let knots = knots_vec.into_boxed_slice();
        let dfs = dfs_vec.into_boxed_slice();

        let interp = build_interp_input_error(
            self.style,
            knots.clone(),
            dfs.clone(),
            self.extrapolation,
            true,
        )?;

        Ok(DiscountCurve {
            id: self.id,
            base,
            day_count: self.day_count,
            knots,
            dfs,
            interp,
            style: self.style,
            extrapolation: self.extrapolation,
            min_forward_rate: self.min_forward_rate,
            allow_non_monotonic: self.allow_non_monotonic,
            min_forward_tenor: self.min_forward_tenor,
            rate_calibration: self.rate_calibration,
            calibration_ois_cutoff_days: self.calibration_ois_cutoff_days,
            fx_policy: self.fx_policy,
        })
    }
}

// ---------------------------------------------------------------------------
// Builder validation helpers (private to this module)
// ---------------------------------------------------------------------------

/// Validate that discount factors are monotone (non-increasing) within tolerance.
///
/// Non-monotonic discount factors violate no-arbitrage conditions and will
/// produce incorrect pricing results.
fn validate_monotonic_df(knots: &[f64], dfs: &[f64]) -> crate::Result<()> {
    if let Some((i, prev, curr)) = crate::math::interp::utils::find_monotone_violation(dfs, 1e-14) {
        return Err(crate::Error::Validation(format!(
            "Discount factors must be non-increasing: DF(t={:.4}) = {:.12} > DF(t={:.4}) = {:.12}",
            knots[i + 1],
            curr,
            knots[i],
            prev
        )));
    }
    Ok(())
}

/// Validate DF input compatibility with MonotoneConvex interpolation.
///
/// MonotoneConvex (Hagan-West) requires a positive, non-increasing DF term structure.
fn validate_monotone_convex_compatible_df(knots: &[f64], dfs: &[f64]) -> crate::Result<()> {
    if let Some((i, prev, curr)) = crate::math::interp::utils::find_monotone_violation(dfs, 1e-14) {
        return Err(crate::Error::Validation(format!(
            "InterpStyle::MonotoneConvex requires non-increasing discount factors. \
             Found DF(t={:.4}) = {:.12} > DF(t={:.4}) = {:.12}. \
             Use LogLinear/Linear (and allow_non_monotonic) for negative-rate / increasing-DF inputs, \
             or fix the input curve.",
            knots[i + 1],
            curr,
            knots[i],
            prev
        )));
    }
    Ok(())
}

/// Validate that implied forward rates are above a minimum threshold.
///
/// Forward rates are calculated as: f(t1, t2) = -ln(DF(t2)/DF(t1)) / (t2 - t1)
///
/// Excessively negative forward rates (below the specified floor) indicate
/// either data errors or unrealistic market conditions.
fn validate_forward_rates(knots: &[f64], dfs: &[f64], min_rate: f64) -> crate::Result<()> {
    for (knot_pair, df_pair) in knots.windows(2).zip(dfs.windows(2)) {
        let dt = knot_pair[1] - knot_pair[0];
        if dt <= 0.0 {
            continue;
        }

        let fwd = -(df_pair[1] / df_pair[0]).ln() / dt;

        if fwd < min_rate {
            return Err(crate::Error::Validation(format!(
                "Forward rate {:.4}% (decimal: {:.6}) between t={:.4} and t={:.4} is below minimum {:.4}% (decimal: {:.6}). \
                 This may indicate a data error or create arbitrage opportunities.",
                fwd * 100.0, fwd, knot_pair[0], knot_pair[1], min_rate * 100.0, min_rate
            )));
        }
    }
    Ok(())
}
