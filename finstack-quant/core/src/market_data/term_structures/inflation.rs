//! Inflation curves for projected CPI/RPI index levels and annual growth.
//!
//! Represents a projected term structure of CPI (Consumer Price Index) levels
//! anchored at `(0.0, I₀)` on an explicit [`DayCount`] clock (plus a holiday
//! calendar when using `Bus/252`), explicit interpolation style (`Linear`,
//! `CubicHermite`, or `LogLinear`), and explicit extrapolation policy (`Flat`,
//! `Extend`, or `Error`).
//!
//! # Source Boundary (`CORE-MARKET-10`)
//!
//! An [`InflationCurve`] holds **unlagged** projected CPI levels; contractual
//! observation lags, daily reference-month weights, publication cutoffs, and
//! deflation floors belong to contracts and the shared valuation resolver
//! rather than the curve source. Conditioning the curve preserves its original
//! source clock, anchor date, and absolute CPI levels unchanged.
//!
//! # Mathematical Representation
//!
//! ```text
//! I(t) = projected CPI index level at time t >= 0
//! π(t₁, t₂) = exp(ln(I(t₂) / I(t₁)) / (t₂ - t₁)) - 1  (annualized CAGR)
//! ```
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::dates::{Date, DayCount};
//! use finstack_quant_core::market_data::term_structures::InflationCurve;
//! use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
//! use time::Month;
//!
//! let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let ic = InflationCurve::builder("US-CPI")
//!     .base_date(base)
//!     .day_count(DayCount::Act365F)
//!     .knots([(0.0, 300.0), (5.0, 327.0)])
//!     .interp(InterpStyle::LogLinear)
//!     .extrapolation(ExtrapolationPolicy::Extend)
//!     .build()
//!     .expect("InflationCurve builder should succeed");
//! assert!(ic.inflation_rate(0.0, 5.0).expect("valid interval") > 0.0);
//! ```
//!
//! # References
//!
//! - **Inflation Markets**:
//!   - Deacon, M., Derry, A., & Mirfendereski, D. (2004). *Inflation-Indexed Securities:
//!     Bonds, Swaps and Other Derivatives* (2nd ed.). Wiley Finance. `docs/REFERENCES.md#deacon-derry-mirfendereski-2004`
//!   - Kerkhof, J. (2005). "Inflation Derivatives Explained." *Journal of Derivatives
//!     Accounting*, 2(1), 1-19. `docs/REFERENCES.md#kerkhof-2005`
//!
//! - **Curve Construction**:
//!   - Hurd, M., & Relleen, J. (2006). "Estimating the Inflation Risk Premium."
//!     Bank of England Quarterly Bulletin, Q2 2006. `docs/REFERENCES.md#kerkhof-2005`
//!   - Fleckenstein, M., Longstaff, F. A., & Lustig, H. (2017). "Deflation Risk."
//!     *Review of Financial Studies*, 30(8), 2719-2760. `docs/REFERENCES.md#kerkhof-2005`

use super::common::factor::dated_points_to_knots;
use super::common::{
    CurveCalendar, signed_curve_year_fraction, stable_positive_log_ratio, validate_curve_clock,
};
use crate::dates::{Date, DayCount, DayCountContext};
use crate::market_data::bumps::{BumpSpec, BumpType, Bumpable};
use crate::market_data::scalars::InflationSeries;
use crate::math::interp::{
    ExtrapolationPolicy, Interp, InterpStyle, exact_f64_eq, validate_knots,
    validate_positive_series,
};
use crate::math::stats;
use crate::types::CurveId;

/// Projected CPI/RPI index-level curve on an explicit day-count clock.
///
/// Stores finite strictly positive CPI levels beginning with the explicit
/// origin pillar `(0.0, I₀)` and at least one positive-time pillar. Interpolation
/// supports [`InterpStyle::Linear`], [`InterpStyle::CubicHermite`], and
/// [`InterpStyle::LogLinear`], with literal [`ExtrapolationPolicy`] tails
/// (`Flat`, `Extend`, `Error`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawInflationCurve", into = "RawInflationCurve")]
pub struct InflationCurve {
    series: Option<InflationSeries>,
    id: CurveId,
    /// Anchor date represented by `t = 0.0` (`I₀`).
    base_date: Date,
    /// Explicit day-count convention mapping dates on/after `base_date` to curve time.
    day_count: DayCount,
    /// Optional resolved holiday calendar (required for `Bus/252`).
    calendar: Option<CurveCalendar>,
    /// Strictly ascending knot times starting at `0.0` with length `>= 2`.
    knots: Box<[f64]>,
    /// Finite, strictly positive projected CPI levels corresponding to `knots`.
    cpi_levels: Box<[f64]>,
    /// Scalar interpolator built from `knots` and `cpi_levels`.
    interp: Interp,
}

/// Raw canonical JSON/wire state of an [`InflationCurve`].
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct RawInflationCurve {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    series: Option<InflationSeries>,
    /// Curve identifier.
    pub id: String,
    /// Anchor date represented by the `t = 0.0` CPI pillar.
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub base_date: Date,
    /// Explicit day-count convention.
    pub day_count: DayCount,
    /// Optional registered holiday calendar identifier (`"brbd"`, `"usny+target2"`, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar_id: Option<String>,
    /// Strictly increasing `(time_years, cpi_level)` pillars starting at `0.0`.
    pub knot_points: Vec<(f64, f64)>,
    /// Explicit scalar interpolation style (`linear`, `cubic_hermite`, or `log_linear`).
    pub interp_style: InterpStyle,
    /// Explicit extrapolation policy (`flat`, `extend`, or `error`).
    pub extrapolation: ExtrapolationPolicy,
}

impl From<InflationCurve> for RawInflationCurve {
    fn from(curve: InflationCurve) -> Self {
        Self {
            id: curve.id.to_string(),
            series: curve.series.clone(),
            base_date: curve.base_date,
            day_count: curve.day_count,
            calendar_id: curve.calendar.as_ref().map(|cal| cal.id.clone()),
            knot_points: curve.knot_points().collect(),
            interp_style: curve.interp.style(),
            extrapolation: curve.interp.extrapolation(),
        }
    }
}

impl TryFrom<RawInflationCurve> for InflationCurve {
    type Error = crate::Error;

    fn try_from(state: RawInflationCurve) -> crate::Result<Self> {
        InflationCurve::builder(state.id)
            .series(state.series)
            .base_date(state.base_date)
            .day_count(state.day_count)
            .calendar_id_opt(state.calendar_id)
            .knots(state.knot_points)
            .interp(state.interp_style)
            .extrapolation(state.extrapolation)
            .build()
    }
}

impl InflationCurve {
    /// Construct an inflation curve from dated projected CPI levels.
    ///
    /// # Arguments
    /// * `id` - Identifier used to select the market source.
    /// * `base_date` - Anchor date defining zero time on the curve clock.
    /// * `points` - Strictly increasing observation dates with the first observation on the base date.
    ///   Finite positive unlagged CPI levels in index points.
    /// * `day_count` - Explicit convention for date-to-time conversion.
    /// * `interp` - Interpolation of positive CPI levels, such as log-linear.
    /// * `extrapolation` - Outside-grid policy, such as flat continuation.
    /// * `calendar_id` - Explicit holiday calendar when required by the day count.
    ///
    /// # Errors
    /// Rejects unsupported clocks, invalid levels or rates, and malformed date grids.
    pub fn from_dates(
        id: impl Into<CurveId>,
        base_date: Date,
        points: &[(Date, f64)],
        day_count: DayCount,
        interp: InterpStyle,
        extrapolation: ExtrapolationPolicy,
        calendar_id: Option<&str>,
    ) -> crate::Result<Self> {
        let id = id.into();
        let calendar = validate_curve_clock("InflationCurve", id.as_str(), day_count, calendar_id)?;
        let knots = dated_points_to_knots(
            "InflationCurve",
            base_date,
            points,
            day_count,
            calendar.as_ref(),
        )?;
        Self::builder(id)
            .base_date(base_date)
            .day_count(day_count)
            .calendar_id_opt(calendar_id.map(str::to_owned))
            .knots(knots)
            .interp(interp)
            .extrapolation(extrapolation)
            .build()
    }

    /// Return publication metadata used to qualify projected CPI comparisons.
    pub fn get_series(&self) -> Option<&InflationSeries> {
        self.series.as_ref()
    }

    /// Start building an [`InflationCurve`] with explicit required conventions.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier used to key the curve inside a market context.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> InflationCurveBuilder {
        InflationCurveBuilder {
            series: None,
            id: id.into(),
            base_date: None,
            day_count: None,
            calendar_id: None,
            points: Vec::new(),
            style: None,
            extrapolation: None,
        }
    }

    /// Projected CPI level at non-negative finite time `t` (years from `base_date`).
    ///
    /// Returns exact stored knot levels on exact knot hits and evaluates the
    /// configured interpolator and extrapolation policy otherwise, rejecting
    /// `t < 0.0`, non-finite times, and non-positive or non-finite interpolated results.
    ///
    /// # Arguments
    ///
    /// * `t` - Finite non-negative curve time in years from [`base_date`](Self::base_date).
    ///
    /// # Errors
    ///
    /// Returns an error if `t` is non-finite or negative, if `t` lies outside the
    /// knots under [`ExtrapolationPolicy::Error`], or if interpolation produces
    /// a non-finite or non-positive CPI level.
    #[inline]
    #[must_use = "computed CPI level should not be discarded"]
    pub fn cpi(&self, t: f64) -> crate::Result<f64> {
        if !t.is_finite() || t < 0.0 {
            return Err(crate::Error::Validation(format!(
                "InflationCurve '{}' requires finite non-negative time t >= 0.0, got {t}",
                self.id
            )));
        }
        let val = self.interp.interp(t)?;
        if !val.is_finite() || val <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "InflationCurve '{}' CPI at t={t:.6} must be finite and strictly positive, got {val}",
                self.id
            )));
        }
        Ok(val)
    }

    /// Projected CPI level on calendar `date` (`date >= base_date`) using the curve clock.
    ///
    /// # Arguments
    ///
    /// * `date` - Calendar target date on or after [`base_date`](Self::base_date).
    ///
    /// # Errors
    ///
    /// Returns an error if `date < self.base_date`, if day-count year-fraction
    /// mapping fails, or if [`cpi`](Self::cpi) rejects the mapped coordinate.
    #[inline]
    #[must_use = "computed CPI level should not be discarded"]
    pub fn cpi_on_date(&self, date: Date) -> crate::Result<f64> {
        if date < self.base_date {
            return Err(crate::Error::Validation(format!(
                "InflationCurve '{}' query date {date} precedes base_date {}",
                self.id, self.base_date
            )));
        }
        let t = signed_curve_year_fraction(
            self.base_date,
            date,
            self.day_count,
            self.calendar.as_ref(),
        )?;
        self.cpi(t)
    }

    /// Annualized compound CPI growth rate (CAGR) between `t1` and `t2`.
    ///
    /// Evaluates `exp(ln(I(t2) / I(t1)) / (t2 - t1)) - 1` using stable
    /// positive-level log-difference arithmetic on finite `0.0 <= t1 < t2`.
    ///
    /// # Arguments
    ///
    /// * `t1` - Finite non-negative start time in years from [`base_date`](Self::base_date).
    /// * `t2` - Finite end time strictly greater than `t1`.
    ///
    /// # Errors
    ///
    /// Returns an error if the interval is invalid (`t1 < 0`, `t2 <= t1`, non-finite,
    /// or non-positive interval width), if either endpoint [`cpi`](Self::cpi) query
    /// fails, or if the annual decimal rate overflows, rounds onto `-1.0`, or
    /// underflows a non-zero growth ratio to `0.0`.
    pub fn inflation_rate(&self, t1: f64, t2: f64) -> crate::Result<f64> {
        let dt = t2 - t1;
        if !t1.is_finite() || !t2.is_finite() || t1 < 0.0 || !dt.is_finite() || dt <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "InflationCurve '{}' inflation_rate requires finite 0.0 <= t1 < t2 and finite dt, got [{t1}, {t2}]",
                self.id
            )));
        }
        let i1 = self.cpi(t1)?;
        let i2 = self.cpi(t2)?;
        if i1.to_bits() == i2.to_bits() {
            return Ok(0.0);
        }
        let delta_log = stable_positive_log_ratio(i2, i1)?;
        if delta_log == 0.0 {
            return Ok(0.0);
        }
        let annual_log = delta_log / dt;
        if !annual_log.is_finite() {
            return Err(crate::Error::Validation(format!(
                "InflationCurve '{}' annual log growth overflowed on [{t1}, {t2}]",
                self.id
            )));
        }
        let rate = annual_log.exp_m1();
        if !rate.is_finite() || rate <= -1.0 || rate == 0.0 {
            return Err(crate::Error::Validation(format!(
                "InflationCurve '{}' annualized inflation rate {rate} is outside the valid quote domain (-1, +inf)",
                self.id
            )));
        }
        Ok(rate)
    }

    /// Curve identifier.
    #[inline]
    pub fn id(&self) -> &CurveId {
        &self.id
    }

    /// Reference observation date represented by the `t = 0.0` CPI pillar.
    #[inline]
    pub fn base_date(&self) -> Date {
        self.base_date
    }

    /// Day-count convention used by this curve.
    #[inline]
    pub fn day_count(&self) -> DayCount {
        self.day_count
    }

    /// Registered holiday calendar id of the curve's time axis, if any.
    #[must_use]
    pub fn calendar_id(&self) -> Option<&str> {
        self.calendar.as_ref().map(|cal| cal.id.as_str())
    }

    /// Day-count context constructed from this curve's resolved calendar, if any.
    #[must_use]
    pub fn day_count_context(&self) -> DayCountContext<'static> {
        DayCountContext {
            calendar: self.calendar.as_ref().map(|cal| cal.calendar),
            ..DayCountContext::default()
        }
    }

    /// Origin CPI level `I₀` at `t = 0.0`.
    #[inline]
    pub fn base_cpi(&self) -> f64 {
        self.cpi_levels[0]
    }

    /// Knot times in years from [`base_date`](Self::base_date), starting at `0.0`.
    #[inline]
    pub fn knots(&self) -> &[f64] {
        &self.knots
    }

    /// Projected CPI index levels at each knot.
    #[inline]
    pub fn cpi_levels(&self) -> &[f64] {
        &self.cpi_levels
    }

    /// Iterator over paired `(time_years, cpi_level)` pillars stored on this curve.
    pub fn knot_points(&self) -> impl Iterator<Item = (f64, f64)> + '_ {
        self.knots
            .iter()
            .copied()
            .zip(self.cpi_levels.iter().copied())
    }

    /// Scalar interpolation style used between pillars.
    #[inline]
    pub fn interp_style(&self) -> InterpStyle {
        self.interp.style()
    }

    /// Extrapolation policy beyond the final pillar.
    #[inline]
    pub fn extrapolation(&self) -> ExtrapolationPolicy {
        self.interp.extrapolation()
    }

    /// Number of knot points stored on the curve.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.knots.len()
    }

    /// Returns `true` if the curve stores no knot points.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.knots.is_empty()
    }

    /// Create a builder pre-populated with this curve's state and `new_id`.
    ///
    /// # Arguments
    ///
    /// * `new_id` - Identifier assigned to the rebuilt curve.
    pub fn to_builder_with_id(&self, new_id: impl Into<CurveId>) -> InflationCurveBuilder {
        self.metadata_builder(new_id).knots(self.knot_points())
    }

    fn metadata_builder(&self, id: impl Into<CurveId>) -> InflationCurveBuilder {
        InflationCurve::builder(id)
            .series(self.series.clone())
            .base_date(self.base_date)
            .day_count(self.day_count)
            .calendar_id_opt(self.calendar_id().map(str::to_owned))
            .interp(self.interp.style())
            .extrapolation(self.interp.extrapolation())
    }
}

/// Fluent builder for [`InflationCurve`].
///
/// Requires explicit `base_date`, `day_count` (plus `calendar_id` for
/// `Bus/252`), `knots` (at least two points starting with exact `(0.0, I₀)`),
/// `interp` (`Linear`, `CubicHermite`, or `LogLinear`), and `extrapolation`.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::{Date, DayCount};
/// use finstack_quant_core::market_data::term_structures::InflationCurve;
/// use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
/// use time::Month;
///
/// let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
/// let curve = InflationCurve::builder("US-CPI")
///     .base_date(base)
///     .day_count(DayCount::Act365F)
///     .knots([(0.0, 300.0), (5.0, 327.0)])
///     .interp(InterpStyle::LogLinear)
///     .extrapolation(ExtrapolationPolicy::Extend)
///     .build()
///     .expect("InflationCurve builder should succeed");
/// assert_eq!(curve.base_cpi(), 300.0);
/// ```
pub struct InflationCurveBuilder {
    series: Option<InflationSeries>,
    id: CurveId,
    base_date: Option<Date>,
    day_count: Option<DayCount>,
    calendar_id: Option<String>,
    points: Vec<(f64, f64)>,
    style: Option<InterpStyle>,
    extrapolation: Option<ExtrapolationPolicy>,
}

impl InflationCurveBuilder {
    /// Set the publication identity and units of the projected CPI levels.
    ///
    /// # Arguments
    ///
    /// * `series` - Publisher series, reference base and seasonal adjustment. An unqualified curve can price instruments but cannot measure changes between snapshots.
    pub fn series(mut self, series: Option<InflationSeries>) -> Self {
        self.series = series;
        self
    }

    /// Set the **base date** (`t = 0.0` anchor).
    ///
    /// # Arguments
    ///
    /// * `d` - Anchor date represented by the `(0.0, I₀)` pillar.
    pub fn base_date(mut self, d: Date) -> Self {
        self.base_date = Some(d);
        self
    }

    /// Set the explicit day-count convention for date-to-time mapping.
    ///
    /// # Arguments
    ///
    /// * `day_count` - Supported complete day count (cannot need coupon metadata).
    pub fn day_count(mut self, day_count: DayCount) -> Self {
        self.day_count = Some(day_count);
        self
    }

    /// Set the registered holiday calendar of the curve's time axis.
    ///
    /// # Arguments
    ///
    /// * `calendar_id` - Registered calendar id (or `+`-joined union); required for `Bus/252`.
    pub fn calendar_id(mut self, calendar_id: impl Into<String>) -> Self {
        self.calendar_id = Some(calendar_id.into());
        self
    }

    /// Optionally set the time-axis holiday calendar; `None` clears it.
    ///
    /// # Arguments
    ///
    /// * `calendar_id` - Optional registered holiday calendar identifier.
    pub fn calendar_id_opt(mut self, calendar_id: Option<String>) -> Self {
        self.calendar_id = calendar_id;
        self
    }

    /// Append `(time_years, cpi_level)` pillars starting from `(0.0, I₀)`.
    ///
    /// # Arguments
    ///
    /// * `pts` - Strictly increasing `(t, I)` pairs beginning at exact `t = 0.0`
    ///   with finite strictly positive index levels.
    pub fn knots<I>(mut self, pts: I) -> Self
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        self.points.extend(pts);
        self
    }

    /// Set the scalar interpolation style (`Linear`, `CubicHermite`, or `LogLinear`).
    ///
    /// # Arguments
    ///
    /// * `style` - Interpolation method between CPI pillars.
    pub fn interp(mut self, style: InterpStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Set the extrapolation policy beyond the last pillar (`Flat`, `Extend`, or `Error`).
    ///
    /// # Arguments
    ///
    /// * `extrapolation` - Out-of-range tail policy for `t > knots.last()`.
    pub fn extrapolation(mut self, extrapolation: ExtrapolationPolicy) -> Self {
        self.extrapolation = Some(extrapolation);
        self
    }

    /// Validate all inputs and build the [`InflationCurve`].
    ///
    /// # Errors
    ///
    /// Returns an error if `base_date`, `day_count`, `interp`, or `extrapolation`
    /// is missing; if `day_count` is unsupported or missing a required calendar;
    /// if `interp` is `MonotoneConvex` or `PiecewiseQuadraticForward`; if fewer
    /// than two pillars are provided; if knots are non-finite, unsorted, or do
    /// not begin at exact `0.0`; or if any CPI level is non-finite or `<= 0.0`.
    pub fn build(self) -> crate::Result<InflationCurve> {
        let base_date = self.base_date.ok_or_else(|| {
            crate::Error::Validation(format!("InflationCurve '{}' requires base_date", self.id))
        })?;
        let day_count = self.day_count.ok_or_else(|| {
            crate::Error::Validation(format!("InflationCurve '{}' requires day_count", self.id))
        })?;
        let style = self.style.ok_or_else(|| {
            crate::Error::Validation(format!(
                "InflationCurve '{}' requires interp style",
                self.id
            ))
        })?;
        let extrapolation = self.extrapolation.ok_or_else(|| {
            crate::Error::Validation(format!(
                "InflationCurve '{}' requires extrapolation policy",
                self.id
            ))
        })?;

        if matches!(
            style,
            InterpStyle::MonotoneConvex | InterpStyle::PiecewiseQuadraticForward
        ) {
            return Err(crate::Error::Validation(format!(
                "InflationCurve '{}' does not support {style:?} interpolation; use Linear, CubicHermite, or LogLinear",
                self.id
            )));
        }

        let calendar = validate_curve_clock(
            "InflationCurve",
            self.id.as_str(),
            day_count,
            self.calendar_id.as_deref(),
        )?;

        let (kvec, cvec): (Vec<f64>, Vec<f64>) = self.points.into_iter().unzip();
        validate_knots(&kvec)?;
        if !exact_f64_eq(kvec[0], 0.0) {
            return Err(crate::Error::Validation(format!(
                "InflationCurve '{}' requires explicit (0.0, I_0) origin as first pillar, got first t={}",
                self.id, kvec[0]
            )));
        }
        validate_positive_series(&cvec)?;

        let knots = kvec.into_boxed_slice();
        let cpi_levels = cvec.into_boxed_slice();
        let interp = style.build_enum(knots.clone(), cpi_levels.clone(), extrapolation)?;

        if let Some(series) = &self.series {
            series.validate()?;
        }
        Ok(InflationCurve {
            series: self.series,
            id: self.id,
            base_date,
            day_count,
            calendar,
            knots,
            cpi_levels,
            interp,
        })
    }
}

/// Evaluate `level * exp(log_delta)` without spurious intermediate overflow/underflow.
#[inline]
fn scale_positive_level_log(level: f64, log_delta: f64) -> Option<f64> {
    if log_delta.abs() < 700.0 {
        let factor = log_delta.exp();
        let prod = level * factor;
        if prod.is_finite() && prod > 0.0 && (level.is_subnormal() || prod.is_normal()) {
            return Some(prod);
        }
    }
    let val = (level.ln() + log_delta).exp();
    (val.is_finite() && val > 0.0).then_some(val)
}

/// Evaluate `I_0 * d^t * (1 + exp(v))^t` stably when `d > exp(h)` (`v = h - ln(d) < 0`).
#[inline]
fn positive_dominated_shift_level(
    base_cpi: f64,
    level: f64,
    t: f64,
    d: f64,
    v: f64,
) -> Option<f64> {
    let e0 = stats::frexp_exp(base_cpi);
    let m0 = stats::ldexp(base_cpi, -e0);
    let ed = stats::frexp_exp(d);
    let md = stats::ldexp(d, -ed);

    if exact_f64_eq(t, 1.0) {
        let base_times_d = stats::ldexp(m0 * md, e0 + ed);
        if base_times_d.is_finite() && base_times_d > 0.0 {
            let val = level + base_times_d;
            if val.is_finite() && val > 0.0 {
                return Some(val);
            }
        }
    }

    let correction = if v > -745.0 { v.exp().ln_1p() } else { 0.0 };
    let exp_val = (m0.ln() + t * md.ln())
        + (f64::from(e0) + t * f64::from(ed)) * std::f64::consts::LN_2
        + t * correction;
    let val = exp_val.exp();
    (exp_val.is_finite() && val.is_finite() && val > 0.0).then_some(val)
}

fn bump_pillar_cpi(
    curve_id: &CurveId,
    t: f64,
    level: f64,
    base_cpi: f64,
    shift: f64,
    is_multiplicative: bool,
    weight: f64,
) -> crate::Result<f64> {
    if !is_multiplicative {
        if exact_f64_eq(shift, 0.0) || exact_f64_eq(weight, 0.0) {
            return Ok(level);
        }
        let d = shift * weight;
        if !d.is_finite() || d == 0.0 {
            return Err(crate::Error::Validation(format!(
                "InflationCurve '{}' weighted additive shift underflowed at t={t:.6}",
                curve_id
            )));
        }

        let h = stable_positive_log_ratio(level, base_cpi)? / t;
        if !h.is_finite() {
            return Err(crate::Error::Validation(format!(
                "InflationCurve '{}' source annual log growth overflowed at t={t:.6}",
                curve_id
            )));
        }
        let log_abs_d = d.abs().ln();
        let new_level = if d < 0.0 {
            if h <= log_abs_d {
                return Err(crate::Error::Validation(format!(
                    "InflationCurve '{}' additive bump at t={t:.6} drives annual gross growth non-positive (h={h}, d={d})",
                    curve_id
                )));
            }
            let u = h - log_abs_d;
            let delta_h = if u <= 1.0 {
                (-(-u).exp_m1()).ln()
            } else {
                (-(-u).exp()).ln_1p()
            };
            scale_positive_level_log(level, t * delta_h)
        } else if h >= log_abs_d {
            let u = log_abs_d - h;
            let delta_h = if u > -745.0 { u.exp().ln_1p() } else { 0.0 };
            scale_positive_level_log(level, t * delta_h)
        } else {
            let v = h - log_abs_d;
            positive_dominated_shift_level(base_cpi, level, t, d, v)
        };

        return new_level.ok_or_else(|| {
            crate::Error::Validation(format!(
                "InflationCurve '{}' bumped CPI level at t={t:.6} is not finite and positive",
                curve_id
            ))
        });
    }

    if exact_f64_eq(weight, 0.0) || exact_f64_eq(shift, 1.0) {
        return Ok(level);
    }
    let log_m = if exact_f64_eq(weight, 1.0) {
        shift.ln()
    } else {
        let m = (1.0 - weight) + shift * weight;
        if !m.is_finite() || m <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "InflationCurve '{}' multiplicative gross multiplier must be finite and positive at t={t:.6}, got {m}",
                curve_id
            )));
        }
        m.ln()
    };
    let log_delta = t * log_m;
    if !log_delta.is_finite() {
        return Err(crate::Error::Validation(format!(
            "InflationCurve '{}' multiplicative log shift overflowed at t={t:.6}",
            curve_id
        )));
    }
    scale_positive_level_log(level, log_delta).ok_or_else(|| {
        crate::Error::Validation(format!(
            "InflationCurve '{}' bumped CPI level at t={t:.6} is not finite and positive",
            curve_id
        ))
    })
}

impl Bumpable for InflationCurve {
    fn apply_bump(&self, spec: BumpSpec) -> crate::Result<Self> {
        let (shift, is_multiplicative) = spec.resolve_standard_values_or_error(
            "InflationCurve",
            "only supports Additive/{RateBp,Percent,Fraction} or Multiplicative/Factor",
        )?;
        if is_multiplicative && (!shift.is_finite() || shift <= 0.0) {
            return Err(crate::Error::Validation(format!(
                "InflationCurve '{}' Multiplicative/Factor bump requires finite k > 0.0, got {shift}",
                self.id
            )));
        }

        if let BumpType::TriangularKeyRate {
            prev_bucket,
            target_bucket,
            ..
        } = spec.bump_type
            && (prev_bucket.is_some_and(|prev| prev < 0.0)
                || target_bucket <= 0.0
                || !self
                    .knots
                    .get(1..)
                    .is_some_and(|pos| pos.iter().any(|&k| exact_f64_eq(k, target_bucket))))
        {
            return Err(crate::Error::Validation(format!(
                "InflationCurve '{}' key-rate target {target_bucket} must match an exact positive source pillar",
                self.id
            )));
        }

        let base_cpi = self.base_cpi();
        let mut bumped_points = Vec::with_capacity(self.knots.len());
        bumped_points.push((self.knots[0], base_cpi));

        for (&t, &cpi) in self.knots[1..].iter().zip(self.cpi_levels[1..].iter()) {
            let weight = spec.bump_type.weight(t);
            let bumped =
                bump_pillar_cpi(&self.id, t, cpi, base_cpi, shift, is_multiplicative, weight)?;
            bumped_points.push((t, bumped));
        }

        self.metadata_builder(self.id.clone())
            .knots(bumped_points)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market_data::bumps::{BumpMode, BumpUnits};
    use time::Month;

    fn sample_curve() -> InflationCurve {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
        InflationCurve::builder("US-CPI")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 300.0), (1.0, 306.0), (2.0, 312.0)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .build()
            .expect("InflationCurve builder should succeed in test")
    }

    #[test]
    fn builder_requires_all_explicit_conventions_and_origin() {
        let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        // Missing origin or non-zero first knot rejected
        assert!(
            InflationCurve::builder("US-CPI")
                .base_date(base)
                .day_count(DayCount::Act365F)
                .knots([(1.0, 306.0), (2.0, 312.0)])
                .interp(InterpStyle::LogLinear)
                .extrapolation(ExtrapolationPolicy::Flat)
                .build()
                .is_err()
        );
        // Single origin knot without positive pillar rejected
        assert!(
            InflationCurve::builder("US-CPI")
                .base_date(base)
                .day_count(DayCount::Act365F)
                .knots([(0.0, 300.0)])
                .interp(InterpStyle::LogLinear)
                .extrapolation(ExtrapolationPolicy::Flat)
                .build()
                .is_err()
        );
        // Negative first knot rejected
        assert!(
            InflationCurve::builder("US-CPI")
                .base_date(base)
                .day_count(DayCount::Act365F)
                .knots([(-0.5, 297.0), (0.0, 300.0), (1.0, 306.0)])
                .interp(InterpStyle::LogLinear)
                .extrapolation(ExtrapolationPolicy::Flat)
                .build()
                .is_err()
        );
        // Unsupported interpolation styles rejected
        for unsupported in [
            InterpStyle::MonotoneConvex,
            InterpStyle::PiecewiseQuadraticForward,
        ] {
            assert!(
                InflationCurve::builder("US-CPI")
                    .base_date(base)
                    .day_count(DayCount::Act365F)
                    .knots([(0.0, 300.0), (1.0, 306.0)])
                    .interp(unsupported)
                    .extrapolation(ExtrapolationPolicy::Flat)
                    .build()
                    .is_err()
            );
        }
        // Bus252 without calendar rejected
        assert!(
            InflationCurve::builder("US-CPI")
                .base_date(base)
                .day_count(DayCount::Bus252)
                .knots([(0.0, 300.0), (1.0, 306.0)])
                .interp(InterpStyle::Linear)
                .extrapolation(ExtrapolationPolicy::Flat)
                .build()
                .is_err()
        );
    }

    #[test]
    fn checked_queries_and_subnormals_behave_per_adr() {
        let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let ic = sample_curve();
        assert!((ic.cpi(1.0).unwrap() - 306.0).abs() < 1e-12);
        assert!(ic.cpi(-0.01).is_err());
        assert!(ic.cpi(f64::NAN).is_err());
        assert!(
            ic.cpi_on_date(Date::from_calendar_date(2024, Month::December, 31).unwrap())
                .is_err()
        );

        // Annual growth 300 -> 306 in 1y is 2%, in 0.5y is 4.04%
        assert!((ic.inflation_rate(0.0, 1.0).unwrap() - 0.02).abs() < 1e-12);
        let half_year = InflationCurve::builder("HALF")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 300.0), (0.5, 306.0)])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .build()
            .unwrap();
        assert!((half_year.inflation_rate(0.0, 0.5).unwrap() - 0.0404).abs() < 1e-12);

        // Severe deflation exp(-40) survives direct CPI lookup without quote overflow,
        // while annual quote rounding to -1 errors cleanly.
        let exp_minus_40 = InflationCurve::builder("DEFL")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, (-40.0_f64).exp())])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .build()
            .unwrap();
        assert_eq!(exp_minus_40.cpi(1.0).unwrap(), (-40.0_f64).exp());
        assert!(exp_minus_40.inflation_rate(0.0, 1.0).is_err());

        // Neighboring huge levels preserve nonzero annual rate via checked_log_cross_ratio.
        let big = 1e300_f64;
        let next_big = f64::from_bits(big.to_bits() + 1);
        let huge_curve = InflationCurve::builder("HUGE")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, big), (1.0, next_big)])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .build()
            .unwrap();
        let tiny_rate = huge_curve.inflation_rate(0.0, 1.0).unwrap();
        assert!((tiny_rate - 1.487016908477783e-16).abs() < 1e-28);
    }

    #[test]
    fn shocks_follow_gross_annual_growth_and_extreme_domain_identities() {
        let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let curve = InflationCurve::builder("CPI-MULT")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 300.0), (1.0, 306.0), (5.0, 300.0 * 1.02_f64.powi(5))])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .build()
            .unwrap();

        // Additive +100bp gives 309 at 1y (annual gross 1.02 + 0.01 = 1.03)
        let add = curve.apply_bump(BumpSpec::parallel_bp(100.0)).unwrap();
        assert_eq!(add.id().as_str(), "CPI-MULT");
        assert!((add.cpi(1.0).unwrap() - 309.0).abs() < 1e-12);

        // Multiplicative factor 1.10 scales gross growth 1.02 -> 1.122 (12.2% annual rate)
        let mult = curve.apply_bump(BumpSpec::multiplier(1.10)).unwrap();
        assert!((mult.cpi(1.0).unwrap() - 336.6).abs() < 1e-11);
        assert!((mult.inflation_rate(0.0, 1.0).unwrap() - 0.122).abs() < 1e-12);
        assert!((mult.inflation_rate(0.0, 5.0).unwrap() - 0.122).abs() < 1e-12);

        // Exact identity preserves bit-identical source levels (even for extreme deflation)
        let defl = InflationCurve::builder("DEFL-ID")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, (-40.0_f64).exp())])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .build()
            .unwrap();
        let ident = defl.apply_bump(BumpSpec::parallel_bp(0.0)).unwrap();
        assert_eq!(ident.cpi_levels(), defl.cpi_levels());

        // Origin, unknown, and zero/negative factor bumps are rejected
        assert!(
            curve
                .apply_bump(BumpSpec::triangular_key_rate_first_bp(0.0, 1.0, 10.0))
                .is_err()
        );
        assert!(
            curve
                .apply_bump(BumpSpec::triangular_key_rate_bp(1.0, 2.5, 5.0, 10.0))
                .is_err()
        );
        assert!(
            curve
                .apply_bump(BumpSpec {
                    mode: BumpMode::Multiplicative,
                    units: BumpUnits::Factor,
                    value: 0.0,
                    bump_type: BumpType::Parallel,
                })
                .is_err()
        );

        // Extreme cross-magnitude additive rescue: I0=1e300, I1=1e-300, t=1, d=1e-300 -> 1.0
        let cross = InflationCurve::builder("CROSS")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1e300), (1.0, 1e-300)])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .build()
            .unwrap();
        let rescued = cross
            .apply_bump(BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Fraction,
                value: 1e-300,
                bump_type: BumpType::Parallel,
            })
            .unwrap();
        assert!((rescued.cpi(1.0).unwrap() - 1.0).abs() < 1e-12);

        // Extreme tiny gross multiplier k=1e-300 on I0=1, I1=1e300 -> 1.0
        let giant = InflationCurve::builder("GIANT")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 1e300)])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .build()
            .unwrap();
        let scaled_down = giant.apply_bump(BumpSpec::multiplier(1e-300)).unwrap();
        assert!((scaled_down.cpi(1.0).unwrap() - 1.0).abs() < 1e-12);
    }
}
