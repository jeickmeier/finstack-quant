//! Forward rate curves for simple term floating-rate indices.
//!
//! A forward curve represents expected future simple rates for a specific
//! tenor-index projection (e.g., 3-month SOFR term, 6-month EURIBOR). These
//! curves are essential for pricing floating-rate instruments and calculating
//! forward-looking cash flows in swaps and floating-rate notes.
//!
//! # Financial Concept
//!
//! The forward rate f(t₁, t₂) is the rate agreed today for borrowing/lending
//! from time t₁ to t₂:
//! ```text
//! f(t₁, t₂) = [DF(t₁) / DF(t₂) - 1] / (t₂ - t₁)
//!
//! For a fixed-tenor index (e.g., 3M):
//! f(t) = forward rate resetting at time t for the index tenor
//! ```
//!
//! # Market Construction
//!
//! Forward curves are typically bootstrapped from:
//! - **Futures**: SOFR futures, Eurodollar futures (liquid up to ~5 years)
//! - **FRA** (Forward Rate Agreements): OTC quotes for forward rates
//! - **Swaps**: Float leg expectations from swap rates
//! - **Basis spreads**: Tenor basis between different index tenors
//!
//! # Index Conventions
//!
//! This type stores simple tenor forwards plus day-count/reset-lag metadata.
//! It does **not** model overnight compounded-in-arrears fixings, observation
//! shifts, or lookbacks. Use it for term indices or already-compounded term
//! projections. Overnight RFR instruments need a separate compounding model.
//!
//! # Use Cases
//!
//! - **Floating-rate note pricing**: Project future coupon payments
//! - **Interest rate swap valuation**: Mark-to-market floating leg
//! - **Cap/floor pricing**: Forward rates determine intrinsic value
//! - **Basis swap pricing**: Spread between different index tenors
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::market_data::term_structures::ForwardCurve;
//! use finstack_quant_core::math::interp::InterpStyle;
//! use finstack_quant_core::dates::Date;
//! use time::Month;
//!
//! let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let fc = ForwardCurve::builder("USD-SOFR3M", 0.25)
//!     .base_date(base)
//!     .knots([(0.0, 0.03), (5.0, 0.04)])
//!     .interp(InterpStyle::Linear)
//!     .build()
//!     .expect("ForwardCurve builder should succeed");
//! assert!(fc.rate(1.0) > 0.0);
//! ```
//!
//! # References
//!
//! - Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.).
//!   Chapters 4-6 (Forward rates and curve construction).
//! - Andersen, L., & Piterbarg, V. (2010). *Interest Rate Modeling*.
//!   Volume 1, Chapter 3 (Multi-curve framework).
//! - Ametrano, F. M., & Bianchetti, M. (2013). "Everything You Always Wanted to
//!   Know About Multiple Interest Rate Curve Bootstrapping but Were Afraid to Ask."
//!   SSRN Working Paper.

use super::common::{
    build_interp_allow_any_values, infer_forward_curve_defaults, roll_knots, split_points,
    triangular_weight,
};
use crate::math::interp::{ExtrapolationPolicy, InterpStyle};
use crate::{
    currency::Currency,
    dates::{Date, DayCount, DayCountContext},
    error::InputError,
    market_data::traits::{Forward, TermStructure},
    math::integration::simpson_rule,
    math::interp::types::Interp,
    types::CurveId,
};

/// Market quote metadata used to build a forward curve.
///
/// This optional sidecar lets risk calculations shock the original projection
/// quotes and re-bootstrap the curve instead of directly bumping fitted forward
/// knots.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForwardCurveRateCalibration {
    /// Rate index used by the projection instruments.
    pub index_id: String,
    /// Currency of the calibrated curve.
    pub currency: Currency,
    /// Discount curve used while calibrating projection instruments.
    pub discount_curve_id: CurveId,
    /// Benchmark rate quotes used for calibration.
    pub quotes: Vec<ForwardCurveRateQuote>,
}

/// A single benchmark rate quote used to calibrate a forward curve.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForwardCurveRateQuote {
    /// Money-market deposit quote.
    Deposit {
        /// Tenor string, such as `3M`.
        tenor: String,
        /// Quoted rate in decimal form.
        rate: f64,
    },
    /// Forward-rate agreement quote.
    Fra {
        /// FRA start date.
        start: Date,
        /// FRA end date.
        end: Date,
        /// Quoted rate in decimal form.
        rate: f64,
    },
    /// Interest-rate swap quote.
    Swap {
        /// Swap tenor string, such as `5Y`.
        tenor: String,
        /// Fixed rate in decimal form.
        rate: f64,
        /// Optional floating-leg spread in decimal form.
        spread_decimal: Option<f64>,
    },
    /// Tenor-basis quote versus the discount/reference curve.
    Basis {
        /// Basis maturity tenor string, such as `6M` or `2Y`.
        tenor: String,
        /// Quoted basis spread in decimal form.
        spread_decimal: f64,
    },
}

/// Forward rate curve for a simple floating-rate index with fixed tenor.
///
/// Represents expected future simple rates for a specific tenor-index projection
/// (e.g., 3-month SOFR term, 6-month EURIBOR). Stores simple forward rates at
/// knot times and interpolates between them.
///
/// # Index Components
///
/// - **Tenor**: Index accrual period (e.g., 0.25 years = 3 months)
/// - **Reset lag**: Days from fixing date to effective date
/// - **Day count**: Convention for accrual (usually Act/360 or Act/365F)
///
/// # Thread Safety
///
/// Immutable after construction; safe to share via `Arc<ForwardCurve>`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawForwardCurve", into = "RawForwardCurve")]
pub struct ForwardCurve {
    id: CurveId,
    base: Date,
    /// Business days from fixing to spot using positive T-minus semantics.
    reset_lag: i32,
    /// Day-count basis used for accrual.
    day_count: DayCount,
    /// Index tenor in **years** (0.25 = 3M).
    tenor: f64,
    /// Knot times in **years** (strictly increasing, first may be 0.0).
    knots: Box<[f64]>,
    /// Simple forward rates (e.g. 0.025 = 2.5 %).
    forwards: Box<[f64]>,
    /// Optional contractual reset/end-date boundaries, separate from interpolation knots.
    projection_grid: Option<Box<[f64]>>,
    interp: Interp,
    /// Optional market quotes used to bootstrap this curve.
    rate_calibration: Option<ForwardCurveRateCalibration>,
    /// Exact typed recipe used to replay calibration after quote shocks.
    rate_calibration_recipe: Option<super::RateCalibrationRecipe>,
    /// Opaque FX policy stamp; see [`DiscountCurve::fx_policy`].
    fx_policy: Option<String>,
}

/// Raw serializable state of ForwardCurve
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawForwardCurve {
    /// Curve identifier
    pub id: String,
    /// Base date
    pub base: Date,
    /// Reset lag in business days
    pub reset_lag: i32,
    /// Day count convention
    pub day_count: DayCount,
    /// Index tenor in years
    pub tenor: f64,
    /// Time/value pairs used to construct the curve
    pub knot_points: Vec<(f64, f64)>,
    /// Optional contractual reset/end-date boundaries.
    ///
    /// Curves without this field retain legacy fixed numeric-tenor DF stepping.
    #[serde(default)]
    pub projection_grid: Option<Vec<f64>>,
    /// Interpolation style
    pub interp_style: InterpStyle,
    /// Extrapolation policy
    pub extrapolation: ExtrapolationPolicy,
    /// Optional market quotes used to bootstrap this curve.
    #[serde(default)]
    pub rate_calibration: Option<ForwardCurveRateCalibration>,
    /// Exact typed calibration replay recipe.
    #[serde(default)]
    pub rate_calibration_recipe: Option<super::RateCalibrationRecipe>,
    /// Opaque FX policy stamp; see [`super::DiscountCurve::fx_policy`].
    #[serde(default)]
    pub fx_policy: Option<String>,
}

impl From<ForwardCurve> for RawForwardCurve {
    fn from(curve: ForwardCurve) -> Self {
        let knot_points: Vec<(f64, f64)> = curve
            .knots
            .iter()
            .zip(curve.forwards.iter())
            .map(|(&t, &fwd)| (t, fwd))
            .collect();

        RawForwardCurve {
            id: curve.id.to_string(),
            base: curve.base,
            reset_lag: curve.reset_lag,
            day_count: curve.day_count,
            tenor: curve.tenor,
            knot_points,
            projection_grid: curve.projection_grid.map(Vec::from),
            interp_style: curve.interp.style(),
            extrapolation: curve.interp.extrapolation(),
            rate_calibration: curve.rate_calibration,
            rate_calibration_recipe: curve.rate_calibration_recipe,
            fx_policy: curve.fx_policy,
        }
    }
}

impl TryFrom<RawForwardCurve> for ForwardCurve {
    type Error = crate::Error;

    fn try_from(state: RawForwardCurve) -> crate::Result<Self> {
        ForwardCurve::builder(state.id, state.tenor)
            .base_date(state.base)
            .reset_lag(state.reset_lag)
            .day_count(state.day_count)
            .knots(state.knot_points)
            .projection_grid_opt(state.projection_grid)
            .interp(state.interp_style)
            .extrapolation(state.extrapolation)
            .rate_calibration_opt(state.rate_calibration)
            .rate_calibration_recipe_opt(state.rate_calibration_recipe)
            .fx_policy_opt(state.fx_policy)
            .build()
    }
}

impl ForwardCurve {
    /// Start building a forward curve for `id` with tenor `tenor_years`.
    ///
    /// **Defaults:** The builder infers day-count and reset-lag conventions from
    /// the curve ID when possible, then uses Linear interpolation with FlatForward
    /// extrapolation.
    ///
    /// **Build-vs-query basis trap:** the inferred day-count converts dated
    /// pillars to year fractions at build time and query dates back at lookup
    /// time. Because inference is substring-based, renaming the curve ID can
    /// silently change the basis (Act/360 vs Act/365F shifts every pillar time
    /// by ~1.4%) and the reset lag. Set [`ForwardCurveBuilder::day_count`] and
    /// [`ForwardCurveBuilder::reset_lag`] explicitly when conventions matter;
    /// each day-count inference is logged at `debug` level.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>, tenor_years: f64) -> ForwardCurveBuilder {
        let id: CurveId = id.into();
        let defaults = infer_forward_curve_defaults(id.as_str());
        // Epoch date - unwrap_or provides defensive fallback for infallible operation
        let base =
            Date::from_calendar_date(1970, time::Month::January, 1).unwrap_or(time::Date::MIN);
        ForwardCurveBuilder {
            id,
            base,
            base_is_set: false,
            reset_lag: defaults.reset_lag_business_days,
            day_count: defaults.day_count,
            tenor: tenor_years,
            points: Vec::new(),
            projection_grid: None,
            style: InterpStyle::Linear,
            min_forward_rate: None,
            extrapolation: ExtrapolationPolicy::FlatForward,
            rate_calibration: None,
            rate_calibration_recipe: None,
            fx_policy: None,
        }
    }

    /// Forward rate starting at time `t` (in years) for the curve’s tenor.
    #[inline]
    #[must_use]
    pub fn rate(&self, t: f64) -> f64 {
        self.interp.interp(t)
    }

    /// Simple forward rate implied by projection discount factors between `t1` and `t2`.
    ///
    /// This is the period rate coherent with [`Self::df`]:
    ///
    /// ```text
    /// rate = (df(t1) / df(t2) - 1) / (t2 - t1)
    /// ```
    ///
    /// Use this for an arbitrary term period. For an index fixing at a reset
    /// date, use [`Self::rate`]. [`Self::rate_period`] instead returns the
    /// Simpson-rule integral average used by overnight compounding sub-windows.
    ///
    /// # Errors
    ///
    /// Returns an error when either time is non-finite, `t2 <= t1`, or an
    /// implied projection discount factor cannot be calculated.
    #[must_use = "computed forward rate should not be discarded"]
    pub fn rate_between(&self, t1: f64, t2: f64) -> crate::Result<f64> {
        if !(t1.is_finite() && t2.is_finite()) {
            return Err(InputError::Invalid.into());
        }
        if t2 <= t1 {
            return Err(crate::Error::Validation(format!(
                "ForwardCurve::rate_between requires t2 > t1; got t1={t1}, t2={t2}"
            )));
        }

        let log_growth = self.projection_log_df(t1)? - self.projection_log_df(t2)?;
        let rate = log_growth.exp_m1() / (t2 - t1);
        if !rate.is_finite() {
            return Err(crate::Error::Validation(format!(
                "Invalid implied forward rate for {} over [{t1}, {t2}]: {rate}",
                self.id.as_str()
            )));
        }
        Ok(rate)
    }

    /// Reset lag in business days from fixing to spot.
    #[inline]
    pub fn reset_lag(&self) -> i32 {
        self.reset_lag
    }

    /// Day-count convention used for this index.
    #[inline]
    pub fn day_count(&self) -> DayCount {
        self.day_count
    }

    /// Index tenor in **years** (e.g. 0.25 = 3M).
    #[inline]
    pub fn tenor(&self) -> f64 {
        self.tenor
    }

    /// Raw knot times used to bootstrap the curve.
    #[inline]
    pub fn knots(&self) -> &[f64] {
        &self.knots
    }

    /// Raw simple forward rates at each knot.
    #[inline]
    pub fn forwards(&self) -> &[f64] {
        &self.forwards
    }

    /// Contractual reset/end-date boundaries used for projection DFs, when present.
    ///
    /// This grid is independent of interpolation knots. `None` means the curve
    /// uses legacy fixed numeric-tenor stepping from zero.
    #[inline]
    pub fn projection_grid(&self) -> Option<&[f64]> {
        self.projection_grid.as_deref()
    }

    /// Curve identifier.
    #[inline]
    pub fn id(&self) -> &CurveId {
        &self.id
    }
    /// Valuation **base date**.
    #[inline]
    pub fn base_date(&self) -> Date {
        self.base
    }

    /// Interpolation style used by this curve.
    #[inline]
    pub fn interp_style(&self) -> InterpStyle {
        self.interp.style()
    }

    /// Extrapolation policy used by this curve.
    #[inline]
    pub fn extrapolation(&self) -> ExtrapolationPolicy {
        self.interp.extrapolation()
    }

    /// Market quote metadata used to build this curve, when available.
    #[inline]
    pub fn rate_calibration(&self) -> Option<&ForwardCurveRateCalibration> {
        self.rate_calibration.as_ref()
    }

    /// Exact typed conventions and quotes used to calibrate this curve.
    #[inline]
    pub fn rate_calibration_recipe(&self) -> Option<&super::RateCalibrationRecipe> {
        self.rate_calibration_recipe.as_ref()
    }

    /// Opaque FX policy stamp set by the curve constructor; see
    /// [`super::DiscountCurve::fx_policy`] for the contract.
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

    /// Simpson-rule integral average rate over `[t1, t2]`.
    ///
    /// This is appropriate for averaging short overnight observation
    /// sub-windows, not for deriving the simple term forward over an arbitrary
    /// projection interval. Use [`Self::rate_between`] for the latter.
    ///
    /// # NaN contract
    ///
    /// Returns [`f64::NAN`] (rather than an error) if `t2 < t1` — misordered
    /// arguments are a caller bug, and changing the signature to `Result`
    /// would be too breaking. A `debug_assert` fires in debug builds and a
    /// `tracing` warning is emitted in release builds; callers must be
    /// prepared for NaN to propagate if they pass misordered times.
    #[inline]
    #[must_use]
    pub fn rate_period(&self, t1: f64, t2: f64) -> f64 {
        debug_assert!(
            t2 >= t1,
            "ForwardCurve::rate_period requires t1 <= t2 (got t1={t1}, t2={t2})"
        );
        if t2 < t1 {
            tracing::warn!(
                curve_id = %self.id,
                t1 = t1,
                t2 = t2,
                "ForwardCurve::rate_period called with t2 < t1; returning NaN. \
                 This is likely a caller bug — time arguments should satisfy t1 <= t2.",
            );
            return f64::NAN;
        }
        // Market-standard interpretation: average forward over the interval.
        //
        // We approximate the integral average of the interpolated forward curve:
        //   avg = (1 / (t2 - t1)) * ∫_{t1}^{t2} f(t) dt
        //
        // Use fixed-segment Simpson's rule for determinism (no adaptive stepping).
        // This is materially better than endpoint averaging for curved/interpolated shapes.
        let dt = t2 - t1;
        if dt <= 1e-12 {
            return self.rate(t1);
        }

        let n: usize = if dt > 20.0 {
            32
        } else if dt > 5.0 {
            16
        } else {
            8
        };
        simpson_rule(|t| self.rate(t), t1, t2, n).map_or(f64::NAN, |integral| integral / dt)
    }

    /// Logarithm of the implied projection discount factor from zero to `t`.
    fn projection_log_df(&self, t: f64) -> crate::Result<f64> {
        if !t.is_finite() {
            return Err(InputError::Invalid.into());
        }
        if t < 0.0 {
            return Err(crate::Error::Validation(format!(
                "ForwardCurve df(t) requires t >= 0; got t={t}"
            )));
        }
        if t == 0.0 {
            return Ok(0.0);
        }

        let tau = self.tenor;
        if !tau.is_finite() || tau <= 0.0 {
            return Err(InputError::Invalid.into());
        }

        let mut log_df = 0.0_f64;
        let mut cur = 0.0_f64;
        let advance = |start: f64, end: f64, log_df: &mut f64| -> crate::Result<()> {
            let dt = end - start;
            if dt <= 0.0 {
                return Ok(());
            }
            let forward = self.rate(start);
            let accrual = forward * dt;
            let denom = 1.0 + accrual;
            if !denom.is_finite() || denom <= 0.0 {
                return Err(crate::Error::Validation(format!(
                    "Invalid implied projection DF step for {}: t={start:.6} -> {end:.6}, forward={forward:.6}, denom={denom:.6}",
                    self.id.as_str(),
                )));
            }
            *log_df -= accrual.ln_1p();
            Ok(())
        };

        if let Some(grid) = &self.projection_grid {
            for &boundary in grid.iter().skip(1) {
                if cur >= t {
                    break;
                }
                let nxt = boundary.min(t);
                advance(cur, nxt, &mut log_df)?;
                cur = nxt;
            }
        }

        while cur < t {
            let nxt = (cur + tau).min(t);
            if nxt <= cur {
                return Err(crate::Error::Validation(format!(
                    "ForwardCurve projection step made no progress at t={cur} toward {t}"
                )));
            }
            advance(cur, nxt, &mut log_df)?;
            cur = nxt;
        }
        Ok(log_df)
    }

    /// Implied **projection discount factor** from `0` to `t` (years).
    ///
    /// This is a convenience for Bloomberg-style curve inspection where a projection curve
    /// is displayed with both forward rates and an implied discount factor curve.
    ///
    /// With an explicit [`Self::projection_grid`], projection discount factors
    /// chain the contractual reset/end intervals independently of interpolation
    /// knots:
    ///
    /// ```text
    /// DF(0) = 1
    /// DF(reset_end) = DF(reset_start) / (1 + F(reset_start) * dt)
    /// ```
    ///
    /// This preserves fixed-tenor quote meaning when calendar adjustment makes
    /// a contractual period differ from the numeric tenor (for example, a 3M
    /// Act/360 period spanning 91 or 92 days). Without an explicit grid, the
    /// legacy behavior is retained: fixed numeric-tenor stepping from zero.
    ///
    /// Notes
    /// -----
    /// - This is **not** a discount curve used for PV discounting; it is an *implied projection DF*.
    /// - Explicit contractual intervals come from `projection_grid`.
    /// - Curves without a grid, and times beyond an explicit grid, step by `tenor_years`.
    /// - This is a simple-rate chaining helper, not an overnight compounded-in-arrears engine.
    #[must_use = "computed discount factor should not be discarded"]
    pub fn df(&self, t: f64) -> crate::Result<f64> {
        let df = self.projection_log_df(t)?.exp();
        if !df.is_finite() || df <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "Invalid implied projection DF for {} at t={t}: {df}",
                self.id.as_str()
            )));
        }
        Ok(df)
    }

    /// Implied projection discount factor on a calendar date using the curve's day-count.
    ///
    /// # Errors
    ///
    /// Returns an error if year fraction or discount factor calculation fails.
    #[inline]
    #[must_use = "computed discount factor should not be discarded"]
    pub fn df_on_date_curve(&self, date: Date) -> crate::Result<f64> {
        let t = if date == self.base {
            0.0
        } else {
            self.day_count
                .year_fraction(self.base, date, DayCountContext::default())?
        };
        self.df(t)
    }

    /// Create a builder pre-populated with this curve's data but a new ID.
    pub fn to_builder_with_id(&self, new_id: impl Into<CurveId>) -> ForwardCurveBuilder {
        self.metadata_builder(new_id).knots(
            self.knots
                .iter()
                .copied()
                .zip(self.forwards.iter().copied()),
        )
    }

    /// Builder pre-populated with this curve's full metadata but **no** knots.
    /// Shared by all rebuild-style operations (bumps, rolls) so that no
    /// metadata field (reset lag, day-count, interpolation, extrapolation,
    /// rate_calibration, fx_policy) is dropped.
    pub(crate) fn metadata_builder(&self, new_id: impl Into<CurveId>) -> ForwardCurveBuilder {
        ForwardCurve::builder(new_id, self.tenor)
            .base_date(self.base)
            .reset_lag(self.reset_lag)
            .day_count(self.day_count)
            .interp(self.interp.style())
            .extrapolation(self.interp.extrapolation())
            .projection_grid_opt(self.projection_grid.as_deref().map(<[f64]>::to_vec))
            .rate_calibration_opt(self.rate_calibration.clone())
            .rate_calibration_recipe_opt(self.rate_calibration_recipe.clone())
            .fx_policy_opt(self.fx_policy.clone())
    }

    /// Create a new curve with a key-rate bump applied at a target time `t` (in years) (fallible).
    ///
    /// Create a new curve with a triangular key-rate bump using explicit bucket neighbors.
    ///
    /// This is the market-standard key-rate DV01 implementation (per Tuckman/Fabozzi)
    /// where the triangular weight is defined by the **bucket grid**, not curve knots.
    /// This ensures that the sum of all bucketed DV01s equals the parallel DV01.
    ///
    /// # Mathematical Foundation
    ///
    /// The triangular weight function for bucket at `target` with neighbors `prev` and `next`:
    /// - w(t) = 0                                    if t ≤ prev
    /// - w(t) = (t - prev) / (target - prev)        if prev < t ≤ target
    /// - w(t) = (next - t) / (next - target)        if target < t < next
    /// - w(t) = 0                                    if t ≥ next
    ///
    /// The forward rate is then bumped: `rate_bumped = rate + bump * weight`
    ///
    /// # Key Property: Unity Partition
    ///
    /// For any time t, the sum of all bucket weights equals 1.0:
    /// `Σᵢ wᵢ(t) = 1.0`
    ///
    /// This ensures: **sum of bucketed DV01 = parallel DV01**
    ///
    /// # Arguments
    /// * `prev_bucket` - Previous bucket time in years (`None` for the first bucket)
    /// * `target_bucket` - Target bucket time in years (peak of the triangle)
    /// * `next_bucket` - Next bucket time in years (`None` for the last bucket;
    ///   never pass `f64::INFINITY` — non-finite bounds are rejected)
    /// * `bp` - Bump size in basis points (100bp = 1%)
    ///
    /// # Returns
    /// A new forward curve with the triangular key-rate bump applied.
    ///
    /// # Errors
    /// Returns an error if the bumped curve violates validation constraints.
    ///
    /// # Examples
    /// ```ignore
    /// use finstack_quant_core::market_data::term_structures::ForwardCurve;
    /// use time::macros::date;
    /// # fn main() -> finstack_quant_core::Result<()> {
    ///
    /// let base_date = date!(2025 - 01 - 01);
    /// let curve = ForwardCurve::builder("USD_SOFR_3M", 0.25)
    ///     .base_date(base_date)
    ///     .knots(vec![(1.0, 0.045), (2.0, 0.048), (5.0, 0.050), (10.0, 0.052)])
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
    ///
    /// `prev_bucket = None` makes this the first bucket (flat-left half
    /// triangle); `next_bucket = None` makes this the last bucket
    /// (flat-right half triangle). These preserve the unity-partition
    /// invariant at the wings.
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
        super::common::validate_triangular_bucket_grid(prev_bucket, target_bucket, next_bucket)?;

        let bump_rate = bp / 10_000.0;
        let bumped_rates: Vec<(f64, f64)> = self
            .knots
            .iter()
            .zip(self.forwards.iter())
            .map(|(&knot_t, &rate)| {
                let weight = triangular_weight(knot_t, prev_bucket, target_bucket, next_bucket);
                (knot_t, rate + bump_rate * weight)
            })
            .collect();

        let new_id = crate::market_data::bumps::id_bump_bp(self.id.as_str(), bp);
        // Thread the full metadata (rate_calibration, fx_policy, …).
        self.metadata_builder(new_id).knots(bumped_rates).build()
    }

    /// Rebuild only the interpolator from the current knots and forward rates.
    fn rebuild_interp(&mut self) -> crate::Result<()> {
        self.interp = super::common::build_interp_allow_any_values(
            self.interp.style(),
            self.knots.clone(),
            self.forwards.clone(),
            self.interp.extrapolation(),
        )?;
        Ok(())
    }

    /// Apply a bump specification in-place, mutating values and rebuilding the interpolator.
    pub(crate) fn bump_in_place(
        &mut self,
        spec: &crate::market_data::bumps::BumpSpec,
    ) -> crate::Result<()> {
        use crate::market_data::bumps::BumpType;

        spec.validate_finite()?;
        let (val, is_multiplicative) = spec.resolve_standard_values().ok_or_else(|| {
            crate::error::InputError::UnsupportedBump {
                reason: format!(
                    "ForwardCurve bump requires Additive or Multiplicative values, got {:?}/{:?}",
                    spec.mode, spec.units
                ),
            }
        })?;

        let mut bumped = self.clone();
        match spec.bump_type {
            BumpType::Parallel => {
                if is_multiplicative {
                    for fwd in bumped.forwards.iter_mut() {
                        *fwd *= val;
                    }
                } else {
                    for fwd in bumped.forwards.iter_mut() {
                        *fwd += val;
                    }
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
                for (fwd, &t) in bumped.forwards.iter_mut().zip(bumped.knots.iter()) {
                    let weight = super::common::triangular_weight(
                        t,
                        prev_bucket,
                        target_bucket,
                        next_bucket,
                    );
                    if is_multiplicative {
                        *fwd *= 1.0 + (val - 1.0) * weight;
                    } else {
                        *fwd += val * weight;
                    }
                }
            }
        }
        bumped.rebuild_interp()?;
        *self = bumped;
        Ok(())
    }

    /// Create a new curve with a parallel rate bump applied in basis points (fallible).
    ///
    /// Adds the bump amount (converted from bp) to all forward rates uniformly.
    ///
    /// Returns an error if the bumped curve violates validation constraints.
    pub fn with_parallel_bump(&self, bp: f64) -> crate::Result<Self> {
        let bump_rate = bp / 10_000.0;
        let bumped_points: Vec<(f64, f64)> = self
            .knots
            .iter()
            .zip(self.forwards.iter())
            .map(|(&t, &rate)| (t, rate + bump_rate))
            .collect();

        // Derive new ID with suffix
        let new_id = crate::market_data::bumps::id_bump_bp(self.id.as_str(), bp);

        // Rebuild preserving the full metadata (interpolation, extrapolation,
        // rate_calibration, fx_policy, …).
        self.metadata_builder(new_id).knots(bumped_points).build()
    }

    /// Roll the curve forward by a specified number of days.
    ///
    /// This creates a new curve with:
    /// - Base date advanced by `days`
    /// - Knot times shifted backwards (t' = t - dt_years)
    /// - Points with t' <= 0 are filtered out (expired)
    /// - Forward rates are preserved (no carry/theta adjustment)
    ///
    /// This is the "constant curves" or "pure roll-down" scenario where forward
    /// rates at each calendar date remain the same, but maturity times are
    /// re-measured from the new base date.
    ///
    /// # Arguments
    /// * `days` - Number of days to roll forward
    ///
    /// # Returns
    /// A new forward curve with updated base date and shifted knots.
    ///
    /// # Errors
    /// Returns an error if fewer than 2 knot points remain after filtering expired points.
    ///
    /// # Examples
    /// ```ignore
    /// use finstack_quant_core::market_data::term_structures::ForwardCurve;
    /// use time::macros::date;
    /// # fn main() -> finstack_quant_core::Result<()> {
    ///
    /// let base_date = date!(2025 - 01 - 01);
    /// let curve = ForwardCurve::builder("USD_SOFR_3M", 0.25)
    ///     .base_date(base_date)
    ///     .knots(vec![(0.5, 0.045), (1.0, 0.048), (2.0, 0.050), (5.0, 0.052)])
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

        // Preserve the live forward at the new origin. Merely shifting and
        // dropping expired knots loses the interpolation segment containing
        // `dt_years` and can materially change the rolled curve at t=0.
        let mut rolled_points = Vec::with_capacity(self.knots.len() + 1);
        rolled_points.push((0.0, self.rate(dt_years)));
        rolled_points.extend(roll_knots(&self.knots, &self.forwards, dt_years));

        if rolled_points.len() < 2 {
            return Err(crate::error::InputError::TooFewPoints.into());
        }

        // Thread the full metadata and override the base date.
        let projection_grid = self.projection_grid.as_ref().map(|grid| {
            let mut rolled = Vec::with_capacity(grid.len());
            rolled.push(0.0);
            rolled.extend(
                grid.iter()
                    .copied()
                    .filter(|time| *time > dt_years)
                    .map(|time| time - dt_years),
            );
            rolled
        });

        self.metadata_builder(self.id.clone())
            .base_date(new_base)
            .projection_grid_opt(projection_grid)
            .knots(rolled_points)
            .build()
    }
}

/// Fluent builder for [`ForwardCurve`].
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::market_data::term_structures::ForwardCurve;
/// use finstack_quant_core::dates::Date;
/// use time::Month;
///
/// let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
/// let curve = ForwardCurve::builder("USD_SOFR_3M", 0.25)
///     .base_date(base)
///     .knots([(1.0, 0.045), (2.0, 0.048), (5.0, 0.050)])
///     .build()
///     .expect("ForwardCurve builder should succeed");
/// assert!(curve.rate(2.0) > 0.0);
/// ```
pub struct ForwardCurveBuilder {
    id: CurveId,
    base: Date,
    base_is_set: bool,
    reset_lag: i32,
    day_count: DayCount,
    tenor: f64,
    points: Vec<(f64, f64)>,
    projection_grid: Option<Vec<f64>>,
    style: InterpStyle,
    min_forward_rate: Option<f64>,
    extrapolation: ExtrapolationPolicy,
    rate_calibration: Option<ForwardCurveRateCalibration>,
    rate_calibration_recipe: Option<super::RateCalibrationRecipe>,
    fx_policy: Option<String>,
}

impl ForwardCurveBuilder {
    /// Set the curve’s valuation **base date**.
    pub fn base_date(mut self, d: Date) -> Self {
        self.base = d;
        self.base_is_set = true;
        self
    }
    /// Override the **reset lag** (fixing → spot) in business days.
    pub fn reset_lag(mut self, lag: i32) -> Self {
        self.reset_lag = lag;
        self
    }

    /// Choose the **day-count** convention.
    pub fn day_count(mut self, dc: DayCount) -> Self {
        self.day_count = dc;
        self
    }
    /// Supply knot points `(t, fwd)`.
    pub fn knots<I>(mut self, pts: I) -> Self
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        self.points.extend(pts);
        self
    }

    /// Set contractual reset/end-date boundaries for projection DF chaining.
    pub fn projection_grid<I>(mut self, projection_grid: I) -> Self
    where
        I: IntoIterator<Item = f64>,
    {
        self.projection_grid = Some(projection_grid.into_iter().collect());
        self
    }

    /// Optionally set contractual reset/end-date projection boundaries.
    pub fn projection_grid_opt(mut self, projection_grid: Option<Vec<f64>>) -> Self {
        self.projection_grid = projection_grid;
        self
    }
    /// Select interpolation style for this forward curve.
    pub fn interp(mut self, style: InterpStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the extrapolation policy for out-of-bounds evaluation.
    pub fn extrapolation(mut self, policy: ExtrapolationPolicy) -> Self {
        self.extrapolation = policy;
        self
    }

    /// Enforce a minimum forward rate across the provided knot points.
    pub fn min_forward_rate(mut self, min_rate: f64) -> Self {
        self.min_forward_rate = Some(min_rate);
        self
    }

    /// Attach market quote metadata used to bootstrap this curve.
    pub fn rate_calibration(mut self, calibration: ForwardCurveRateCalibration) -> Self {
        self.rate_calibration = Some(calibration);
        self
    }

    /// Optionally attach market quote metadata used to bootstrap this curve.
    pub fn rate_calibration_opt(
        mut self,
        calibration: Option<ForwardCurveRateCalibration>,
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

    /// Stamp an opaque FX policy on the curve. See [`ForwardCurve::fx_policy`].
    pub fn fx_policy(mut self, policy: impl Into<String>) -> Self {
        self.fx_policy = Some(policy.into());
        self
    }

    /// Optionally stamp an FX policy; `None` is a no-op. Used by serde
    /// round-trip and curve builders that propagate metadata.
    pub fn fx_policy_opt(mut self, policy: Option<String>) -> Self {
        self.fx_policy = policy;
        self
    }

    /// Validate input and build the [`ForwardCurve`].
    pub fn build(self) -> crate::Result<ForwardCurve> {
        if !self.base_is_set {
            return Err(InputError::Invalid.into());
        }
        if !self.tenor.is_finite() || self.tenor <= 0.0 {
            return Err(InputError::Invalid.into());
        }
        if self.reset_lag < 0 {
            return Err(crate::Error::Validation(format!(
                "ForwardCurve reset_lag must be non-negative business days; got {}",
                self.reset_lag
            )));
        }
        if self.points.len() < 2 {
            return Err(InputError::TooFewPoints.into());
        }
        let (kvec, fvec): (Vec<f64>, Vec<f64>) = split_points(self.points);
        crate::math::interp::utils::validate_knots(&kvec)?;
        if let Some(min_fwd) = self.min_forward_rate {
            for (i, &f) in fvec.iter().enumerate() {
                if f < min_fwd {
                    return Err(crate::Error::Validation(format!(
                        "Forward rate below minimum at t={:.6}: fwd={:.8} < min={:.8} (index {})",
                        kvec[i], f, min_fwd, i
                    )));
                }
            }
        }
        let projection_grid = self
            .projection_grid
            .map(|grid| {
                let last_knot = *kvec.last().ok_or(InputError::TooFewPoints)?;
                if grid.len() < 2
                    || grid.iter().any(|time| !time.is_finite() || *time < 0.0)
                    || grid[0].abs() > 1e-12
                    || grid.windows(2).any(|window| window[1] <= window[0])
                    || grid.last().is_none_or(|last| *last < last_knot)
                {
                    return Err(crate::Error::Validation(format!(
                        "ForwardCurve projection_grid must start at 0, be finite, non-negative, strictly increasing, and cover the last interpolation knot ({last_knot})"
                    )));
                }
                Ok(grid.into_boxed_slice())
            })
            .transpose()?;
        let knots = kvec.into_boxed_slice();
        let forwards = fvec.into_boxed_slice();
        // Use allow_any_values to support negative forward rates
        // (common in EUR, CHF, JPY markets since 2014)
        let interp = build_interp_allow_any_values(
            self.style,
            knots.clone(),
            forwards.clone(),
            self.extrapolation,
        )?;
        Ok(ForwardCurve {
            id: self.id,
            base: self.base,
            reset_lag: self.reset_lag,
            day_count: self.day_count,
            tenor: self.tenor,
            knots,
            forwards,
            projection_grid,
            interp,
            rate_calibration: self.rate_calibration,
            rate_calibration_recipe: self.rate_calibration_recipe,
            fx_policy: self.fx_policy,
        })
    }
}

// -----------------------------------------------------------------------------
// Minimal trait implementations for polymorphism where needed
// -----------------------------------------------------------------------------

impl Forward for ForwardCurve {
    #[inline]
    fn rate(&self, t: f64) -> f64 {
        self.rate(t)
    }
}

impl TermStructure for ForwardCurve {
    #[inline]
    fn id(&self) -> &CurveId {
        &self.id
    }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    fn sample_forward() -> ForwardCurve {
        ForwardCurve::builder("USD-LIB3M", 0.25)
            .base_date(
                Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date"),
            )
            .knots([(0.0, 0.03), (1.0, 0.04)])
            .build()
            .expect("ForwardCurve builder should succeed with valid test data")
    }

    #[test]
    fn interpolates_rate() {
        let fc = sample_forward();
        assert!((fc.rate(0.5) - 0.035).abs() < 1e-12);
    }

    #[test]
    fn point_average_and_discount_factor_implied_forwards_are_distinct_on_steep_curve() {
        let fc = ForwardCurve::builder("USD-LIB3M", 0.25)
            .base_date(
                Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date"),
            )
            .knots([(0.0, 0.01), (1.0, 0.21)])
            .build()
            .expect("ForwardCurve builder should succeed with valid test data");
        let (t1, t2) = (0.25, 0.75);

        let point_rate = fc.rate(t1);
        let integrated_average = fc.rate_period(t1, t2);
        let df_implied_rate = fc
            .rate_between(t1, t2)
            .expect("strictly increasing finite times should produce a forward rate");

        assert!((point_rate - integrated_average).abs() > 1e-6);
        assert!((point_rate - df_implied_rate).abs() > 1e-6);
        assert!((integrated_average - df_implied_rate).abs() > 1e-6);
        assert!(
            (df_implied_rate
                - (fc.df(t1).expect("valid DF") / fc.df(t2).expect("valid DF") - 1.0) / (t2 - t1))
                .abs()
                < 1e-14
        );
        assert!(fc.rate_between(t1, t1).is_err());
        assert!(fc.rate_between(t2, t1).is_err());
    }

    #[test]
    fn tiny_positive_intervals_preserve_finite_forward_rate() {
        let curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(
                Date::from_calendar_date(2025, time::Month::January, 1).expect("valid test date"),
            )
            .knots([(0.0, 0.05), (1.0, 0.05)])
            .build()
            .expect("flat forward curve");

        for dt in [5e-13, 1e-14, 1e-16] {
            let rate = curve
                .rate_between(0.0, dt)
                .expect("small positive interval");
            assert!(rate.is_finite());
            assert!(
                (rate - 0.05).abs() < 1e-12,
                "dt={dt}: expected 5%, got {rate}"
            );
        }
    }

    #[test]
    fn reset_grid_preserves_off_grid_fixed_tenor_quote_meaning() {
        let t_3m = 91.0 / 360.0;
        let t_6m = 183.0 / 360.0;
        let first_rate = 0.047;
        let second_rate = 0.0485;
        let curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(
                Date::from_calendar_date(2025, time::Month::January, 1).expect("valid test date"),
            )
            .day_count(DayCount::Act360)
            .knots([(0.0, first_rate), (t_3m, second_rate)])
            .projection_grid([0.0, t_3m, t_6m])
            .build()
            .expect("off-grid reset curve should build");

        assert!((curve.rate(0.0) - first_rate).abs() < 1e-14);
        assert!((curve.rate(t_3m) - second_rate).abs() < 1e-14);
        assert!(
            (curve.rate_between(0.0, t_3m).expect("first reset period") - first_rate).abs() < 1e-14
        );
        assert!(
            (curve.rate_between(t_3m, t_6m).expect("second reset period") - second_rate).abs()
                < 1e-14
        );
    }

    #[test]
    fn contractual_projection_grid_survives_serde_round_trip() {
        let terminal_time = 183.0 / 360.0;
        let projection_grid = [0.0, 91.0 / 360.0, terminal_time];
        let curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(
                Date::from_calendar_date(2025, time::Month::January, 1).expect("valid test date"),
            )
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.047), (91.0 / 360.0, 0.0485)])
            .projection_grid(projection_grid)
            .build()
            .expect("reset-grid curve should build");

        let json = serde_json::to_string(&curve).expect("serialize forward curve");
        let restored: ForwardCurve =
            serde_json::from_str(&json).expect("deserialize forward curve");

        assert_eq!(restored.projection_grid(), Some(projection_grid.as_slice()));
        assert_eq!(restored.knots(), curve.knots());
        assert_eq!(restored.forwards(), curve.forwards());
        assert!(
            (restored
                .rate_between(91.0 / 360.0, terminal_time)
                .expect("restored reset period")
                - 0.0485)
                .abs()
                < 1e-14
        );
    }

    #[test]
    fn full_rate_calibration_recipe_retains_one_day_cutoff() {
        let json = serde_json::json!({
            "id": "USD-SOFR",
            "base": "2025-01-02",
            "reset_lag": 0,
            "day_count": "Act365F",
            "tenor": 1.0,
            "knot_points": [[0.0, 0.04], [5.0, 0.04]],
            "interp_style": "linear",
            "extrapolation": "flat_forward",
            "rate_calibration": {
                "index_id": "USD-SOFR-OIS",
                "currency": "USD",
                "discount_curve_id": "USD-OIS",
                "quotes": [{
                    "swap": {
                        "tenor": "5Y",
                        "rate": 0.04,
                        "spread_decimal": null
                    }
                }]
            },
            "rate_calibration_recipe": {
                "method": {
                    "global_solve": {
                        "use_analytical_jacobian": true
                    }
                },
                "curve_day_count": "Act365F",
                "ois_compounding": {
                    "compounded_with_rate_cutoff": {
                        "cutoff_days": 1
                    }
                },
                "role": {
                    "projection": {
                        "discount_curve_id": "USD-OIS"
                    }
                }
            }
        });

        let curve: ForwardCurve = serde_json::from_value(json).expect("full calibration recipe");
        let serialized = serde_json::to_value(curve).expect("serialize full recipe");
        let restored: ForwardCurve =
            serde_json::from_value(serialized.clone()).expect("round-trip full recipe");

        assert_eq!(
            serialized["rate_calibration_recipe"]["ois_compounding"]["compounded_with_rate_cutoff"]
                ["cutoff_days"],
            1
        );
        assert_eq!(
            serialized["rate_calibration_recipe"]["role"]["projection"]["discount_curve_id"],
            "USD-OIS"
        );
        let recipe = restored.rate_calibration_recipe().expect("restored recipe");
        assert!(matches!(
            recipe.ois_compounding.as_ref(),
            Some(
                crate::market_data::term_structures::RateCalibrationOisCompounding::CompoundedWithRateCutoff {
                    cutoff_days
                }
            ) if *cutoff_days == 1
        ));
    }

    #[test]
    fn contractual_projection_grid_rejects_invalid_boundaries_and_coverage() {
        let base =
            Date::from_calendar_date(2025, time::Month::January, 1).expect("valid test date");
        for grid in [
            vec![-0.01, 0.25, 0.5],
            vec![0.0, f64::NAN, 0.5],
            vec![0.0, 0.5, 0.25],
            vec![0.1, 0.25, 0.5],
            vec![0.0, 0.20],
        ] {
            let error = ForwardCurve::builder("USD-SOFR-3M", 0.25)
                .base_date(base)
                .knots([(0.0, 0.047), (0.25, 0.048)])
                .projection_grid(grid)
                .build()
                .expect_err("invalid contractual grid must be rejected");
            assert!(error.to_string().contains("projection_grid"));
        }
    }

    #[test]
    fn legacy_sparse_serde_keeps_numeric_tenor_df_economics() {
        let json = serde_json::json!({
            "id": "USD-SOFR-3M",
            "base": "2025-01-01",
            "reset_lag": 2,
            "day_count": "Act360",
            "tenor": 0.25,
            "knot_points": [[0.0, 0.04], [1.0, 0.05], [5.0, 0.06]],
            "interp_style": "linear",
            "extrapolation": "flat_forward"
        });
        let curve: ForwardCurve =
            serde_json::from_value(json).expect("legacy sparse curve should deserialize");

        assert_eq!(curve.projection_grid(), None);
        let expected = (0..4).fold(1.0, |df, step| {
            let reset = step as f64 * 0.25;
            df / (1.0 + curve.rate(reset) * 0.25)
        });
        assert!((curve.df(1.0).expect("legacy DF") - expected).abs() < 1e-14);
    }

    #[test]
    fn failed_bump_in_place_is_atomic() {
        let mut curve = sample_forward();
        let before_forwards = curve.forwards().to_vec();
        let before_rate = curve.rate(0.5);

        let error = curve
            .bump_in_place(&crate::market_data::bumps::BumpSpec::parallel_bp(f64::NAN))
            .expect_err("non-finite bump must fail");

        assert!(error.to_string().contains("finite"));
        assert_eq!(curve.forwards(), before_forwards.as_slice());
        assert_eq!(curve.rate(0.5).to_bits(), before_rate.to_bits());
    }

    // Reversed times are a caller bug: debug builds fire a `debug_assert`,
    // release builds return NaN (documented NaN contract on `rate_period`).
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "rate_period requires t1 <= t2")]
    fn rate_period_reversed_times_debug_asserts() {
        let fc = sample_forward();
        let _ = fc.rate_period(1.0, 0.5);
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn rate_period_reversed_times_returns_nan() {
        let fc = sample_forward();
        assert!(fc.rate_period(1.0, 0.5).is_nan());
    }

    #[test]
    fn tail_continuity_with_flatforward_extrapolation() {
        // Test that FlatForward extrapolation maintains stable tail forwards
        let base =
            Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date");
        let fc = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(base)
            .knots([(0.0, 0.03), (1.0, 0.035), (5.0, 0.04)])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::FlatForward)
            .build()
            .expect("ForwardCurve builder should succeed with valid test data");

        // Rate at last knot and beyond should be continuous
        let rate_at_last = fc.rate(5.0);
        let rate_beyond = fc.rate(10.0);

        // FlatForward should maintain the rate (or slope)
        let abs_diff = (rate_beyond - rate_at_last).abs();
        assert!(
            abs_diff < 0.01,
            "Forward rate tail discontinuity: rate_at_last={:.6}, rate_beyond={:.6}",
            rate_at_last,
            rate_beyond
        );
    }

    #[test]
    fn default_uses_flatforward_extrapolation() {
        // Verify new market-standard default extrapolation
        let base =
            Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date");
        let fc = ForwardCurve::builder("TEST", 0.25)
            .base_date(base)
            .knots([(0.0, 0.03), (1.0, 0.04)])
            .build()
            .expect("ForwardCurve builder should succeed with valid test data");

        // With FlatForward, tail rate should be stable (not zero)
        let rate_tail = fc.rate(5.0);
        assert!(
            rate_tail > 0.02,
            "Tail forward should remain positive with FlatForward: {:.6}",
            rate_tail
        );
    }

    #[test]
    fn builder_infers_market_conventions_from_curve_id() {
        let base =
            Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date");

        let sofr_term = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(base)
            .knots([(0.0, 0.03), (1.0, 0.04)])
            .build()
            .expect("USD-SOFR-3M curve should build");
        assert_eq!(sofr_term.day_count(), DayCount::Act360);
        assert_eq!(sofr_term.reset_lag(), 2);

        let sonia = ForwardCurve::builder("GBP-SONIA", 1.0 / 365.0)
            .base_date(base)
            .knots([(0.0, 0.03), (1.0, 0.035)])
            .build()
            .expect("GBP-SONIA curve should build");
        assert_eq!(sonia.day_count(), DayCount::Act365F);
        assert_eq!(sonia.reset_lag(), 0);

        let generic = ForwardCurve::builder("TEST", 0.25)
            .base_date(base)
            .knots([(0.0, 0.03), (1.0, 0.035)])
            .build()
            .expect("Generic forward curve should build");
        assert_eq!(generic.reset_lag(), 0);
    }

    #[test]
    fn roll_forward_uses_curve_day_count() {
        let base =
            Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date");
        let curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(base)
            .day_count(DayCount::Act360)
            .knots([(0.05, 0.03), (0.15, 0.035), (0.30, 0.04)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("ForwardCurve builder should succeed with valid test data");

        // Roll 36 days => Act/360 year fraction = 36/360 = 0.1
        let rolled = curve.roll_forward(36).expect("roll_forward should succeed");
        let ks = rolled.knots();
        assert_eq!(
            ks.len(),
            3,
            "Rolled curve should contain a new-origin anchor and two future knots"
        );
        // Original knots were at 0.05, 0.15, 0.30
        // After rolling 0.1 years: anchor at 0, -0.05 (expired), 0.05, 0.20
        assert!(ks[0].abs() < 1e-12, "Expected a zero-time anchor");
        assert!(
            (ks[1] - 0.05).abs() < 1e-12,
            "Expected 0.15 - 0.10 = 0.05, got {}",
            ks[1]
        );
        assert!(
            (ks[2] - 0.20).abs() < 1e-12,
            "Expected 0.30 - 0.10 = 0.20, got {}",
            ks[2]
        );
    }

    #[test]
    fn roll_forward_preserves_shaped_linear_curve() {
        let base =
            Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date");
        let curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(base)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.02), (1.0, 0.10), (2.0, 0.02)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("valid shaped forward curve");

        let rolled = curve.roll_forward(180).expect("roll should succeed");
        for t in [0.0, 0.25, 0.5, 1.0, 1.5] {
            let expected = curve.rate(t + 0.5);
            let actual = rolled.rate(t);
            assert!(
                (actual - expected).abs() < 1e-12,
                "t={t}: rolled={actual}, original shifted={expected}"
            );
        }
    }
}
