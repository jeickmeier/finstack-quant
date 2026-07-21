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
//! use finstack_quant_core::market_data::term_structures::DiscountCurve;
//! use finstack_quant_core::dates::Date;
//! use time::Month;
//! # use finstack_quant_core::math::interp::InterpStyle;
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
    /// Exact typed recipe used to replay calibration after quote shocks.
    pub(crate) rate_calibration_recipe: Option<super::RateCalibrationRecipe>,
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
    /// Exact typed calibration replay recipe.
    #[serde(default)]
    pub rate_calibration_recipe: Option<super::RateCalibrationRecipe>,
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
            rate_calibration_recipe: curve.rate_calibration_recipe,
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
            .rate_calibration_recipe_opt(state.rate_calibration_recipe)
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
    /// Construct a flat continuously-compounded discount curve.
    ///
    /// The curve uses the minimal two-knot representation
    /// `(0, 1)` and `(1, exp(-rate))`, log-linear interpolation, and
    /// flat-forward extrapolation. This preserves `DF(t) = exp(-rate * t)`
    /// for every non-negative maturity.
    ///
    /// # Errors
    ///
    /// Returns an error when `continuous_rate` is non-finite or its one-year
    /// discount factor cannot be represented as a finite positive value.
    pub fn flat(id: impl AsRef<str>, base_date: Date, continuous_rate: f64) -> crate::Result<Self> {
        if !continuous_rate.is_finite() {
            return Err(crate::Error::Validation(
                "DiscountCurve: flat continuous rate must be finite".to_string(),
            ));
        }
        let one_year_df = crate::math::Compounding::Continuous.df_from_rate(continuous_rate, 1.0);
        if !one_year_df.is_finite() || one_year_df <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "DiscountCurve: flat continuous rate {continuous_rate} produces an invalid discount factor"
            )));
        }

        Self::builder(id.as_ref())
            .base_date(base_date)
            .knots([(0.0, 1.0), (1.0, one_year_df)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::FlatForward)
            .validation(ValidationMode::Raw {
                allow_non_monotonic: continuous_rate < 0.0,
                forward_floor: None,
            })
            .build()
    }

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

    /// Exact typed conventions and quotes used to calibrate this curve.
    #[inline]
    pub fn rate_calibration_recipe(&self) -> Option<&super::RateCalibrationRecipe> {
        self.rate_calibration_recipe.as_ref()
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
    /// use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// use finstack_quant_core::dates::Date;
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
    /// use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// use finstack_quant_core::dates::Date;
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
    /// use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// use finstack_quant_core::dates::Date;
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
    /// use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// # use time::macros::date;
    /// # fn main() -> finstack_quant_core::Result<()> {
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

    /// Whether validation permits increasing discount factors.
    #[inline]
    pub fn allows_non_monotonic(&self) -> bool {
        self.allow_non_monotonic
    }

    /// Minimum implied forward rate accepted by validation, if configured.
    #[inline]
    pub fn min_forward_rate(&self) -> Option<f64> {
        self.min_forward_rate
    }

    /// Batch evaluation of discount factors for multiple times.
    #[inline]
    #[must_use]
    pub fn df_batch(&self, times: &[f64]) -> Vec<f64> {
        let mut out = vec![0.0; times.len()];
        self.df_batch_into(times, &mut out);
        out
    }

    /// Batch evaluation of discount factors into a caller-provided buffer,
    /// avoiding a per-call allocation in hot loops (e.g. Monte Carlo time grids).
    ///
    /// Writes `min(times.len(), out.len())` discount factors; callers should size
    /// `out` to match `times`.
    #[inline]
    pub fn df_batch_into(&self, times: &[f64], out: &mut [f64]) {
        for (slot, &t) in out.iter_mut().zip(times) {
            *slot = self.df(t);
        }
    }

    /// Compute consecutive forward rates over a strictly increasing time grid,
    /// evaluating each discount factor once and reusing the shared endpoint
    /// between adjacent intervals.
    ///
    /// Returns `times.len() - 1` forward rates, where element `k` is the
    /// (continuously-compounded) forward over `[times[k], times[k+1]]`, matching
    /// [`forward`](Self::forward) exactly. This is roughly 2x cheaper than calling
    /// `forward` in a loop, which recomputes both discount factors — and their
    /// binary searches — for every interval.
    ///
    /// # Errors
    ///
    /// Returns an error if `times` has fewer than two points, any time is
    /// non-finite, the grid is not strictly increasing, any interval is shorter
    /// than `min_forward_tenor`, or any discount factor is non-positive.
    #[must_use = "computed forward rates should not be discarded"]
    pub fn forward_grid(&self, times: &[f64]) -> crate::Result<Vec<f64>> {
        if times.len() < 2 {
            return Err(crate::error::InputError::Invalid.into());
        }
        // Evaluate each DF once.
        let mut dfs = vec![0.0; times.len()];
        for (slot, &t) in dfs.iter_mut().zip(times) {
            if !t.is_finite() {
                return Err(crate::error::InputError::Invalid.into());
            }
            let df = self.df(t);
            if !(df.is_finite() && df > 0.0) {
                return Err(crate::error::InputError::Invalid.into());
            }
            *slot = df;
        }
        let mut forwards = Vec::with_capacity(times.len() - 1);
        for k in 0..times.len() - 1 {
            let (t1, t2) = (times[k], times[k + 1]);
            if t2 <= t1 || (t2 - t1) < self.min_forward_tenor {
                return Err(crate::error::InputError::Invalid.into());
            }
            forwards.push(-(dfs[k + 1] / dfs[k]).ln() / (t2 - t1));
        }
        Ok(forwards)
    }

    /// Fallible: discount factor on a specific date `date` using explicit day-count `dc`.
    ///
    /// # Errors
    ///
    /// Propagates a failure from `dc.signed_year_fraction` for the curve base
    /// date and `date`.
    #[inline]
    #[must_use = "computed discount factor should not be discarded"]
    pub fn df_on_date(&self, date: Date, dc: crate::dates::DayCount) -> crate::Result<f64> {
        let t = if date == self.base {
            0.0
        } else {
            dc.signed_year_fraction(self.base, date, DayCountContext::default())?
        };
        Ok(self.df(t))
    }

    /// Fallible: discount factor on a specific date `date` using the curve's day-count.
    ///
    /// # Errors
    ///
    /// Propagates a failure while computing the curve day-count fraction from
    /// the base date to `date`.
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
    ///
    /// # Errors
    ///
    /// Propagates failures while computing either date's curve year fraction,
    /// and returns `Error::Validation` when an evaluated discount factor is
    /// non-finite or non-positive.
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
    /// use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// use finstack_quant_core::math::Compounding;
    /// use finstack_quant_core::dates::Date;
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
    /// # Ok::<(), finstack_quant_core::Error>(())
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

    /// Apply a bump specification in-place, mutating values and rebuilding the interpolator.
    ///
    /// This avoids allocating intermediate `Vec<(f64, f64)>`, skips ID generation,
    /// and skips sort/validation (bumps preserve knot ordering).
    ///
    /// # Performance
    ///
    /// Clones the value array and the interpolator's consumed knot/value inputs,
    /// but avoids cloning the full curve and its calibration recipe.
    pub(crate) fn bump_in_place(
        &mut self,
        spec: &crate::market_data::bumps::BumpSpec,
    ) -> crate::Result<()> {
        use crate::market_data::bumps::BumpType;

        spec.validate_finite()?;
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

        // Clone only values; assign after the fallible interpolator build to
        // preserve failure atomicity.
        let mut dfs = self.dfs.clone();
        match spec.bump_type {
            BumpType::Parallel => {
                for (df, &t) in dfs.iter_mut().zip(self.knots.iter()) {
                    *df *= (-bump_rate * t).exp();
                }
            }
            BumpType::TriangularKeyRate {
                prev_bucket,
                target_bucket,
                next_bucket,
            } => {
                // Reject malformed bucket grids (e.g. infinite sentinels)
                // before mutating: a non-finite neighbour yields NaN weights
                // and corrupts the curve.
                super::common::validate_triangular_bucket_grid(
                    prev_bucket,
                    target_bucket,
                    next_bucket,
                )?;
                for (df, &t) in dfs.iter_mut().zip(self.knots.iter()) {
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
        let interp = super::common::build_interp_input_error(
            self.style,
            self.knots.clone(),
            dfs.clone(),
            self.extrapolation,
            true,
        )?;
        self.dfs = dfs;
        self.interp = interp;
        Ok(())
    }

    /// Create a new curve with a parallel rate bump applied in basis points (fallible).
    ///
    /// Uses df_bumped(t) = df_original(t) * exp(-bump * t), where bump = bp / 10_000.
    ///
    /// # Errors
    ///
    /// Returns an error when the bumped knots violate this curve's interpolation,
    /// discount-factor monotonicity, or forward-rate validation policy.
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
    /// use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// use time::macros::date;
    /// # fn main() -> finstack_quant_core::Result<()> {
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
    /// use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// use time::macros::date;
    /// # fn main() -> finstack_quant_core::Result<()> {
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
    /// across all `finstack-quant-core` term structures (discount, forward, hazard,
    /// inflation, price, vol-index, vol-surface, base-correlation). Instrument
    /// types in `finstack-quant-valuations` use a different convention —
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
    /// [`DiscountCurveBuilder::validation`]. All interpolation styles —
    /// including the default MonotoneConvex — support increasing-DF
    /// (negative-rate) inputs; MonotoneConvex auto-detects negative discrete
    /// forwards and skips its Hagan-West positivity amelioration so negative
    /// rates interpolate faithfully.
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
            rate_calibration_recipe: None,
            calibration_ois_cutoff_days: None,
            fx_policy: None,
        }
    }

    /// Create a builder pre-populated with this curve's data but a new ID.
    pub fn to_builder_with_id(&self, new_id: impl Into<CurveId>) -> DiscountCurveBuilder {
        self.metadata_builder(new_id)
            .knots(self.knots.iter().copied().zip(self.dfs.iter().copied()))
    }

    /// Rebuild this curve with replacement knots while preserving all metadata.
    ///
    /// This retains interpolation, extrapolation, validation policy, calibration
    /// provenance, minimum forward tenor, and FX policy.
    ///
    /// # Errors
    ///
    /// Returns an error when replacement knots violate the preserved curve
    /// validation, interpolation, or forward-rate constraints.
    pub fn rebuild_with_knots<I>(&self, knots: I) -> crate::Result<Self>
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        self.metadata_builder(self.id.clone()).knots(knots).build()
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
            .rate_calibration_recipe_opt(self.rate_calibration_recipe.clone())
            .calibration_ois_cutoff_days_opt(self.calibration_ois_cutoff_days)
            .fx_policy_opt(self.fx_policy.clone())
            .apply_non_monotonic_settings(self.allow_non_monotonic, self.min_forward_rate)
    }

    /// Create a forward curve from this discount curve.
    ///
    /// For single-curve bootstrapping, this creates a fixed-tenor simple-rate
    /// forward curve using:
    /// `F(t, t+tau) = (DF(t) / DF(t+tau) - 1) / tau`.
    ///
    /// # Arguments
    ///
    /// * `forward_id` - Identifier for the resulting forward curve
    /// * `tenor_years` - Tenor of the forward rate in years
    /// * `interp_style` - Optional interpolation style; defaults to `Linear` if `None`
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when `tenor_years` is non-finite or not
    /// strictly positive, `InputError::TooFewPoints` when the discount curve
    /// has fewer than two knots, or an error when the derived forward curve
    /// fails validation.
    pub fn to_forward_curve(
        &self,
        forward_id: impl Into<CurveId>,
        tenor_years: f64,
        interp_style: Option<InterpStyle>,
    ) -> crate::Result<super::forward_curve::ForwardCurve> {
        use super::forward_curve::ForwardCurve;

        if !tenor_years.is_finite() || tenor_years <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "forward tenor must be finite and positive, got {tenor_years}"
            )));
        }

        // Monotone-convex is a discount-factor interpolation strategy and must
        // not be applied to already-derived forward-rate ordinates.
        let style = interp_style.unwrap_or(InterpStyle::Linear);

        // Calculate forward rates at each knot point
        let mut forward_rates = Vec::with_capacity(self.knots.len());

        // Ensure we have enough points
        if self.knots.len() < 2 {
            return Err(crate::error::InputError::TooFewPoints.into());
        }

        for &t in self.knots.iter() {
            let df_start = self.df(t);
            let df_end = self.df(t + tenor_years);
            if !df_start.is_finite() || !df_end.is_finite() || df_start <= 0.0 || df_end <= 0.0 {
                return Err(crate::Error::Validation(format!(
                    "cannot derive forward at t={t}: invalid discount factors \
                     DF(t)={df_start}, DF(t+tenor)={df_end}"
                )));
            }
            let forward_rate = (df_start / df_end - 1.0) / tenor_years;
            if !forward_rate.is_finite() {
                return Err(crate::Error::Validation(format!(
                    "derived non-finite forward rate at t={t}"
                )));
            }
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

impl ValidationMode {
    /// Resolve the public binding preset and its optional forward-rate floor.
    ///
    /// Bindings expose the two safe presets by name while keeping [`Self::Raw`]
    /// available only to canonical Rust callers.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` for an unsupported preset, a floor supplied
    /// with `market_standard`, a missing floor for `negative_rate_friendly`, or
    /// a non-finite floor.
    pub fn from_preset(name: &str, forward_floor: Option<f64>) -> crate::Result<Self> {
        match name {
            "market_standard" => {
                if forward_floor.is_some() {
                    return Err(crate::Error::Validation(
                        "forward_floor is only valid with validation_mode='negative_rate_friendly'"
                            .to_string(),
                    ));
                }
                Ok(Self::MarketStandard)
            }
            "negative_rate_friendly" => {
                let forward_floor = forward_floor.ok_or_else(|| {
                    crate::Error::Validation(
                        "forward_floor is required with validation_mode='negative_rate_friendly'"
                            .to_string(),
                    )
                })?;
                if !forward_floor.is_finite() {
                    return Err(crate::Error::Validation(
                        "forward_floor must be finite".to_string(),
                    ));
                }
                Ok(Self::NegativeRateFriendly { forward_floor })
            }
            other => Err(crate::Error::Validation(format!(
                "unknown DiscountCurve validation_mode {other:?}; expected 'market_standard' or 'negative_rate_friendly'"
            ))),
        }
    }
}

/// Fluent builder for [`DiscountCurve`].
///
/// Typical usage chains `base_date`, `knots`, and `interp` (optional)
/// before calling [`DiscountCurveBuilder::build`].
///
/// # Examples
/// ```rust
/// use finstack_quant_core::market_data::term_structures::DiscountCurve;
/// use finstack_quant_core::math::interp::InterpStyle;
/// use finstack_quant_core::dates::Date;
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
    pub(crate) rate_calibration_recipe: Option<super::RateCalibrationRecipe>,
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
    /// use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// # use time::macros::date;
    /// # fn main() -> finstack_quant_core::Result<()> {
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

    /// Attach an exact typed calibration replay recipe.
    pub fn rate_calibration_recipe(mut self, recipe: super::RateCalibrationRecipe) -> Self {
        self.rate_calibration_recipe = Some(recipe);
        self
    }

    /// Optionally attach an exact typed calibration replay recipe.
    pub fn rate_calibration_recipe_opt(
        mut self,
        recipe: Option<super::RateCalibrationRecipe>,
    ) -> Self {
        self.rate_calibration_recipe = recipe;
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
    pub fn build_for_solver(mut self) -> crate::Result<DiscountCurve> {
        let base = self.base.ok_or(crate::error::InputError::Invalid)?;
        if self.points.len() < 2 {
            return Err(crate::error::InputError::TooFewPoints.into());
        }

        if self.points.iter().any(|&(_, df)| df <= 0.0) {
            return Err(crate::error::InputError::NonPositiveValue.into());
        }

        let (knots, dfs) = split_points(std::mem::take(&mut self.points));
        self.finish(base, knots, dfs)
    }

    /// Validate input and create the [`DiscountCurve`].
    ///
    /// If the first knot time is `> 0.0`, automatically prepends `(0.0, 1.0)` to
    /// ensure the round-trip invariant `DF(0) = 1.0` (ISDA/QuantLib standard).
    ///
    /// # Errors
    ///
    /// Returns an error when the base date is missing, fewer than two knots are
    /// supplied after zero-time anchoring, a discount factor is non-positive,
    /// knots are invalid, or the configured monotonicity, forward-rate, or
    /// interpolation constraints are violated.
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

        let (knots_vec, dfs_vec): (Vec<f64>, Vec<f64>) =
            split_points(std::mem::take(&mut self.points));
        crate::math::interp::utils::validate_knots(&knots_vec)?;

        if !self.allow_non_monotonic {
            validate_monotonic_df(&knots_vec, &dfs_vec)?;
        }

        if let Some(min_fwd) = self.min_forward_rate {
            validate_forward_rates(&knots_vec, &dfs_vec, min_fwd)?;
        }

        self.finish(base, knots_vec, dfs_vec)
    }

    fn finish(self, base: Date, knots: Vec<f64>, dfs: Vec<f64>) -> crate::Result<DiscountCurve> {
        let knots = knots.into_boxed_slice();
        let dfs = dfs.into_boxed_slice();

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
            rate_calibration_recipe: self.rate_calibration_recipe,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_curve_uses_continuous_rate_at_all_maturities() {
        let base =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid base date");

        let curve = DiscountCurve::flat("USD-OIS", base, 0.04).expect("flat discount curve");

        assert_eq!(curve.len(), 2);
        assert_eq!(curve.interp_style(), InterpStyle::LogLinear);
        assert_eq!(curve.extrapolation(), ExtrapolationPolicy::FlatForward);
        for t in [0.0_f64, 0.25, 1.0, 5.0, 30.0] {
            assert!((curve.df(t) - (-0.04 * t).exp()).abs() < 1e-12);
        }
        assert!((curve.forward(2.0, 9.0).expect("flat forward") - 0.04).abs() < 1e-12);
    }

    #[test]
    fn flat_curve_supports_zero_and_negative_continuous_rates() {
        let base =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid base date");

        for rate in [0.0, -0.01] {
            let curve = DiscountCurve::flat("EUR-OIS", base, rate).expect("flat discount curve");
            for t in [0.0_f64, 0.25, 1.0, 5.0, 30.0] {
                let expected = crate::math::Compounding::Continuous.df_from_rate(rate, t);
                assert!((curve.df(t) - expected).abs() < 1e-12);
            }
            assert!((curve.forward(2.0, 9.0).expect("flat forward") - rate).abs() < 1e-12);
        }
    }

    #[test]
    fn flat_curve_matches_manually_built_continuous_curve() {
        let base =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid base date");
        let rate = 0.04;
        let one_year_df = crate::math::Compounding::Continuous.df_from_rate(rate, 1.0);
        let flat = DiscountCurve::flat("USD-OIS", base, rate).expect("flat discount curve");
        let manual = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots([(0.0, 1.0), (1.0, one_year_df)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::FlatForward)
            .validation(ValidationMode::Raw {
                allow_non_monotonic: false,
                forward_floor: None,
            })
            .build()
            .expect("manual continuous curve");

        for t in [0.0_f64, 0.25, 1.0, 5.0, 30.0] {
            assert!((flat.df(t) - manual.df(t)).abs() < 1e-12);
        }
    }

    #[test]
    fn legacy_rate_calibration_metadata_defaults_recipe_safely() {
        let legacy = serde_json::json!({
            "id": "USD-OIS",
            "base": "2025-01-02",
            "day_count": "Act365F",
            "knot_points": [[0.0, 1.0], [5.0, 0.8]],
            "interp_style": "linear",
            "extrapolation": "flat_forward",
            "rate_calibration": {
                "index_id": "USD-SOFR-OIS",
                "currency": "USD",
                "quotes": [{
                    "quote_type": "swap",
                    "tenor": "5Y",
                    "rate": 0.04
                }]
            }
        });

        let curve: DiscountCurve = serde_json::from_value(legacy).expect("legacy serialized curve");
        let serialized = serde_json::to_value(curve).expect("serialize legacy curve");
        let restored: DiscountCurve =
            serde_json::from_value(serialized.clone()).expect("round-trip legacy curve");

        assert!(
            serialized["rate_calibration_recipe"].is_null(),
            "legacy metadata must default to no replay recipe"
        );
        assert!(restored.rate_calibration().is_some());
        assert!(restored.rate_calibration_recipe().is_none());
    }

    #[test]
    fn rebuild_with_knots_retains_permissive_validation_policy() {
        let base =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid base date");
        let source = DiscountCurve::builder("USD-NEGATIVE")
            .base_date(base)
            .knots([(0.0, 1.0), (1.0, 1.01), (2.0, 0.99)])
            .min_forward_tenor(0.000_123)
            .validation(ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: Some(-0.02),
            })
            .build()
            .expect("permissive source curve");

        let rebuilt = source
            .rebuild_with_knots([(0.0, 1.0), (1.0, 1.011), (2.0, 0.991)])
            .expect("metadata-preserving rebuild");
        let serialized = serde_json::to_value(rebuilt).expect("serialize rebuilt curve");

        assert_eq!(serialized["allow_non_monotonic"], true);
        assert_eq!(serialized["min_forward_rate"], -0.02);
        assert_eq!(serialized["min_forward_tenor"], 0.000_123);
    }
}
