//! Discount-curve construction, queries, and forward derivation.

use super::super::forward_curve::ForwardCurve;
use super::{DiscountCurve, DiscountCurveBuilder, ValidationMode, DEFAULT_MIN_FORWARD_TENOR};
use crate::math::interp::{ExtrapolationPolicy, InterpStyle};
use crate::math::Compounding;
use crate::{
    dates::{Date, DayCount},
    types::CurveId,
};

impl DiscountCurve {
    /// Construct a flat continuously-compounded discount curve.
    ///
    /// The curve uses the minimal two-knot representation
    /// `(0, 1)` and `(1, exp(-rate))`, log-linear interpolation, and
    /// flat-forward extrapolation. This preserves `DF(t) = exp(-rate * t)`
    /// for every non-negative maturity.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique curve identifier (for example `"USD-OIS"`).
    /// * `base_date` - Valuation date anchoring `t = 0`; the curve day count
    ///   is `Act/365F`.
    /// * `continuous_rate` - Continuously-compounded zero rate as a decimal
    ///   fraction (`0.05` is 5%). Magnitudes above `1.0` are rejected because
    ///   they almost always mean a percentage was passed where a decimal was
    ///   expected.
    ///
    /// # Errors
    ///
    /// Returns an error when `continuous_rate` is non-finite, has magnitude
    /// greater than `1.0`, or its one-year discount factor cannot be
    /// represented as a finite positive value.
    pub fn flat(id: impl AsRef<str>, base_date: Date, continuous_rate: f64) -> crate::Result<Self> {
        if !continuous_rate.is_finite() {
            return Err(crate::Error::Validation(
                "DiscountCurve: flat continuous rate must be finite".to_string(),
            ));
        }
        if continuous_rate.abs() > 1.0 {
            return Err(crate::Error::Validation(format!(
                "DiscountCurve: flat continuous rate {continuous_rate} is outside [-1, 1]; \
                 rates are decimal fractions (pass 0.05 for 5%, not 5.0)"
            )));
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

    /// Construct a discount curve from zero-rate pillars.
    ///
    /// Each `(t, r)` pillar is converted to a discount factor under
    /// `compounding` (`DF = exp(-r t)` for continuous, `(1 + r)^-t` for
    /// annual, and so on). A pillar at `t = 0` maps to `DF = 1` regardless of
    /// its rate. The curve uses the builder defaults: `Act/365F` day count,
    /// monotone-convex interpolation, flat-forward extrapolation and
    /// market-standard validation.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique curve identifier (for example `"USD-OIS"`).
    /// * `base_date` - Valuation date anchoring `t = 0`.
    /// * `points` - `(time_years, zero_rate)` pillars with the rate as a
    ///   decimal fraction (`0.05` is 5%); times must be finite, non-negative
    ///   and distinct. Any order is accepted.
    /// * `compounding` - Compounding convention under which `zero_rate` is
    ///   quoted.
    ///
    /// # Errors
    ///
    /// Returns an error when `points` is empty, a time or rate is non-finite,
    /// a rate produces a non-positive discount factor, or the resulting knots
    /// fail curve validation (duplicate times, non-monotonic discount factors,
    /// implied forwards below the market-standard floor).
    pub fn from_zero_rates(
        id: impl Into<CurveId>,
        base_date: Date,
        points: &[(f64, f64)],
        compounding: Compounding,
    ) -> crate::Result<Self> {
        if points.is_empty() {
            return Err(crate::error::InputError::TooFewPoints.into());
        }
        let mut knots = Vec::with_capacity(points.len() + 1);
        for &(t, r) in points {
            if !t.is_finite() || !r.is_finite() {
                return Err(crate::Error::Validation(format!(
                    "DiscountCurve::from_zero_rates requires finite pillars, got (t={t}, r={r})"
                )));
            }
            let df = if t == 0.0 {
                1.0
            } else {
                compounding.df_from_rate(r, t)
            };
            if !df.is_finite() || df <= 0.0 {
                return Err(crate::Error::Validation(format!(
                    "DiscountCurve::from_zero_rates: rate {r} at t={t} gives invalid discount factor {df}"
                )));
            }
            knots.push((t, df));
        }
        if !knots.iter().any(|&(t, _)| t == 0.0) {
            knots.push((0.0, 1.0));
        }
        Self::builder(id).base_date(base_date).knots(knots).build()
    }

    /// Construct a discount curve from dated discount-factor pillars.
    ///
    /// Each pillar date is converted to a year fraction from `base_date`
    /// under `day_count`, and a `(0, 1)` anchor is added when no pillar falls
    /// on `base_date`. Other settings use the builder defaults (monotone-convex
    /// interpolation, flat-forward extrapolation, market-standard validation).
    ///
    /// # Arguments
    ///
    /// * `id` - Unique curve identifier (for example `"USD-OIS"`).
    /// * `base_date` - Valuation date anchoring `t = 0`.
    /// * `points` - `(date, discount_factor)` pillars; dates must not precede
    ///   `base_date` and discount factors must be finite and positive. Any
    ///   order is accepted.
    /// * `day_count` - Day-count convention used to convert pillar dates to
    ///   curve times; `None` uses the `Act/365F` builder default.
    ///
    /// # Errors
    ///
    /// Returns an error when `points` is empty, a pillar date precedes
    /// `base_date`, a year fraction cannot be computed, or the resulting knots
    /// fail curve validation.
    pub fn from_dates(
        id: impl Into<CurveId>,
        base_date: Date,
        points: &[(Date, f64)],
        day_count: Option<DayCount>,
    ) -> crate::Result<Self> {
        if points.is_empty() {
            return Err(crate::error::InputError::TooFewPoints.into());
        }
        let day_count = day_count.unwrap_or(DayCount::Act365F);
        let mut knots = Vec::with_capacity(points.len() + 1);
        for &(date, df) in points {
            if date < base_date {
                return Err(crate::Error::Validation(format!(
                    "DiscountCurve::from_dates pillar {date} precedes base date {base_date}"
                )));
            }
            let t = super::super::common::year_fraction_to(base_date, date, day_count)?;
            knots.push((t, df));
        }
        if !knots.iter().any(|&(t, _)| t == 0.0) {
            knots.push((0.0, 1.0));
        }
        Self::builder(id)
            .base_date(base_date)
            .day_count(day_count)
            .knots(knots)
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

    /// Exact typed conventions and quotes used to calibrate this curve.
    #[inline]
    pub fn rate_calibration(&self) -> Option<&super::super::RateCalibrationRecipe> {
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
    ///
    /// # Arguments
    ///
    /// * `t` - Time from the curve base date in years on the curve's day-count basis; zero returns a zero rate.
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
    /// Shorthand for [`zero_rate`](Self::zero_rate) with
    /// [`Compounding::Annual`]: `r_annual = DF^(-1/t) - 1`.
    #[inline]
    #[must_use]
    pub fn zero_annual(&self, t: f64) -> f64 {
        self.zero_rate(t, Compounding::Annual)
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
    /// ```
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
    ///
    /// # Arguments
    ///
    /// * `t1` - Start year-fraction of the forward or rate interval being queried
    /// * `t2` - End year-fraction of the forward or rate interval being queried
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
    /// Inherent forwarder to [`Discounting::df_between_dates`] so concrete
    /// callers need no trait import: `DF(from→to) = DF(0→to) / DF(0→from)`.
    /// Works for both forward and backward date order. Returns `1.0` when
    /// `from == to`.
    ///
    /// [`Discounting::df_between_dates`]: crate::market_data::traits::Discounting::df_between_dates
    ///
    /// # Errors
    ///
    /// Propagates failures while computing either date's curve year fraction,
    /// and returns `Error::Validation` when an evaluated discount factor is
    /// non-finite or non-positive.
    #[inline]
    #[must_use = "computed discount factor should not be discarded"]
    pub fn df_between_dates(&self, from: Date, to: Date) -> crate::Result<f64> {
        crate::market_data::traits::Discounting::df_between_dates(self, from, to)
    }

    /// Returns the zero rate for a given date with specified compounding convention.
    ///
    /// This is the unified method for obtaining zero rates under any compounding convention.
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
    /// This is the unified method for obtaining zero rates under any compounding
    /// convention. For a discount factor `DF` at time `t`:
    ///
    /// | Convention      | Formula                          | Typical use                              |
    /// |-----------------|----------------------------------|------------------------------------------|
    /// | `Continuous`    | `r = -ln(DF) / t`                | Curve internals, [`zero`](Self::zero)    |
    /// | `Annual`        | `r = DF^(-1/t) - 1`              | Bond-equivalent yields (Bloomberg zeros) |
    /// | `Periodic(n)`   | `r = n · (DF^(-1/(n·t)) - 1)`    | `n = 2` US Treasury, `n = 12` monthly    |
    /// | `Simple`        | `r = (1/DF - 1) / t`             | Money markets < 1Y (SOFR, €STR, SONIA,   |
    /// |                 |                                  | deposits, T-bills); no compounding       |
    ///
    /// For positive rates and `t > 0`: `r_simple > r_annual > r_cc`. The
    /// `Simple` convention matches Bloomberg's zero output with compounding
    /// set to "Simple" (SWPM/SWCV) and is typically paired with ACT/360
    /// (USD, EUR) or ACT/365F (GBP).
    ///
    /// # Arguments
    /// * `t` - Year fraction from the anchor date
    /// * `compounding` - Compounding convention (Continuous, Annual, Periodic(n), Simple)
    ///
    /// # Edge Cases
    /// - For t = 0, all compounding conventions return 0.0 (instantaneous rate is undefined)
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// use finstack_quant_core::math::Compounding;
    /// use finstack_quant_core::dates::Date;
    /// use time::Month;
    ///
    /// let curve = DiscountCurve::builder("USD-OIS")
    ///     .base_date(Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"))
    ///     .knots([(0.0, 1.0), (0.25, 0.99), (1.0, 0.95), (5.0, 0.80)])
    ///     .build()
    ///     .expect("DiscountCurve should build");
    ///
    /// // At 1Y, DF = 0.95, so the annual rate = 0.95^(-1) - 1 ≈ 5.26%
    /// assert!((curve.zero_rate(1.0, Compounding::Annual) - 0.0526).abs() < 0.001);
    /// // At 3M, DF = 0.99, so the simple rate = (1/0.99 - 1) / 0.25 ≈ 4.04%
    /// assert!((curve.zero_rate(0.25, Compounding::Simple) - 0.0404).abs() < 0.001);
    /// // Periodic(1) is the annual convention
    /// let annual = curve.zero_rate(1.0, Compounding::Periodic(1.try_into().unwrap()));
    /// assert!((curve.zero_annual(1.0) - annual).abs() < 1e-12);
    /// ```
    #[inline]
    #[must_use]
    pub fn zero_rate(&self, t: f64, compounding: Compounding) -> f64 {
        if t == 0.0 {
            return 0.0;
        }
        let df = self.df(t);
        match compounding {
            Compounding::Continuous => -df.ln() / t,
            Compounding::Annual => df.powf(-1.0 / t) - 1.0,
            Compounding::Periodic(n) => {
                let n_f = f64::from(n.get());
                n_f * (df.powf(-1.0 / (n_f * t)) - 1.0)
            }
            Compounding::Simple => (1.0 / df - 1.0) / t,
        }
    }

    /// Helper: compute year fraction from base date to target date using curve's day-count.
    #[inline]
    fn year_fraction_to(&self, date: Date) -> crate::Result<f64> {
        super::super::common::year_fraction_to(self.base, date, self.day_count)
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
    /// **Defaults:** Day count is [`DayCount::Act365F`] (QuantLib-style curve
    /// time basis). Set [`DiscountCurveBuilder::day_count`] when the curve must
    /// use a different time axis (for example Act/360). Interpolation defaults
    /// to MonotoneConvex with FlatForward extrapolation. Validation defaults to
    /// [`ValidationMode::MarketStandard`]: monotonic discount factors and a
    /// −50bp implied-forward floor.
    ///
    /// **Negative rates:** for deeply negative-rate markets (CHF, JPY, EUR
    /// historical), pass [`ValidationMode::NegativeRateFriendly`] (or `Raw`) via
    /// [`DiscountCurveBuilder::validation`]. All interpolation styles —
    /// including the default MonotoneConvex — support increasing-DF
    /// (negative-rate) inputs; MonotoneConvex auto-detects negative discrete
    /// forwards and skips its Hagan-West positivity amelioration so negative
    /// rates interpolate faithfully.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> DiscountCurveBuilder {
        DiscountCurveBuilder {
            id: id.into(),
            base: None,
            day_count: DayCount::Act365F,
            points: Vec::new(),
            style: InterpStyle::MonotoneConvex,
            extrapolation: ExtrapolationPolicy::FlatForward,
            min_forward_rate: Some(-0.005),
            allow_non_monotonic: false,
            min_forward_tenor: DEFAULT_MIN_FORWARD_TENOR,
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
    pub(super) fn metadata_builder(&self, new_id: impl Into<CurveId>) -> DiscountCurveBuilder {
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
    ) -> crate::Result<ForwardCurve> {
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
