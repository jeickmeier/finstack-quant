//! Inflation curves for CPI/RPI modeling and inflation-linked securities.
//!
//! Represents expected future inflation as a term structure of CPI (Consumer
//! Price Index) levels. Used for pricing inflation-linked bonds (TIPS, linkers),
//! inflation swaps, and inflation caps/floors.
//!
//! # Financial Concept
//!
//! The inflation curve maps time to expected CPI index levels:
//! ```text
//! I(t) = CPI index level at time t
//! π(t₁, t₂) = [I(t₂) / I(t₁)]^(1/(t₂-t₁)) - 1  (annualized inflation rate)
//! ```
//!
//! # Market Construction
//!
//! Inflation curves are bootstrapped from:
//! - **Zero-coupon inflation swaps** (ZCIS): Market standard for breakeven inflation
//! - **Inflation-linked bonds**: TIPS (US), Linkers (UK), OATi (France)
//! - **Year-on-year swaps** (YoY): Annual inflation rate swaps
//! - **Seasonality adjustments**: Monthly patterns in published CPI
//!
//! # Curve Types
//!
//! - **Real inflation**: Market expectations from inflation swaps
//! - **Breakeven inflation**: Implied from TIPS vs nominal bond spreads
//! - **Seasonal inflation**: Incorporates month-to-month volatility
//!
//! # Interpolation
//!
//! LogLinear interpolation is standard (constant inflation rate between knots):
//! ```text
//! I(t) = I(t₁) * exp(π * (t - t₁))
//! ```
//!
//! # Use Cases
//!
//! - **TIPS pricing**: Inflation-adjusted principal and coupons
//! - **Inflation swap valuation**: Zero-coupon and year-on-year structures
//! - **Real rate extraction**: Separate nominal rates into real + inflation
//! - **Pension liability valuation**: Inflation-linked obligations
//!
//! # Examples
//!
//! ```rust
//! use finstack_core::market_data::term_structures::InflationCurve;
//! # use finstack_core::math::interp::InterpStyle;
//! # use finstack_core::dates::Date;
//! # use time::Month;
//! # let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let ic = InflationCurve::builder("US-CPI")
//!     .base_date(base)
//!     .base_cpi(300.0)
//!     .knots([(0.0, 300.0), (5.0, 327.0)])
//!     .interp(InterpStyle::LogLinear)
//!     .build()
//!     .expect("InflationCurve builder should succeed");
//! assert!(ic.inflation_rate(0.0, 5.0) > 0.0);
//! ```
//!
//! # References
//!
//! - **Inflation Markets**:
//!   - Deacon, M., Derry, A., & Mirfendereski, D. (2004). *Inflation-Indexed Securities:
//!     Bonds, Swaps and Other Derivatives* (2nd ed.). Wiley Finance.
//!   - Kerkhof, J. (2005). "Inflation Derivatives Explained." *Journal of Derivatives
//!     Accounting*, 2(1), 1-19.
//!
//! - **Curve Construction**:
//!   - Hurd, M., & Relleen, J. (2006). "Estimating the Inflation Risk Premium."
//!     Bank of England Quarterly Bulletin, Q2 2006.
//!   - Fleckenstein, M., Longstaff, F. A., & Lustig, H. (2017). "Deflation Risk."
//!     *Review of Financial Studies*, 30(8), 2719-2760.

use super::common::{build_interp, roll_knots, split_points};
use crate::dates::{Date, DayCount, DayCountContext};
use crate::math::interp::{ExtrapolationPolicy, InterpStyle};
use crate::{
    error::InputError, market_data::traits::TermStructure, math::interp::types::Interp,
    types::CurveId,
};

/// Default indexation lag in months for inflation-linked securities.
///
/// Most inflation-linked bonds (US TIPS, UK IL Gilts, Euro ILBs, JGBi) use a
/// 3-month lag per the Canadian model (ISDA standard). This means the CPI index
/// ratio for a given settlement date is based on CPI values published 3 months
/// prior, with linear interpolation between monthly values.
pub(crate) const DEFAULT_INDEXATION_LAG_MONTHS: u32 = 3;

/// Inflation curve representing CPI/RPI index levels over time.
///
/// Stores CPI index levels at knot times and interpolates between them using
/// the specified interpolation method. LogLinear interpolation (constant
/// inflation rate) is the market standard.
///
/// # Mathematical Representation
///
/// ```text
/// I(t) = CPI index level at time t
/// π(t₁, t₂) = annualized inflation rate from t₁ to t₂
///           = [I(t₂) / I(t₁)]^(1/(t₂-t₁)) - 1  (CAGR formula)
/// ```
///
/// # Indexation Lag
///
/// Inflation-linked bonds use a publication lag (default: 3 months). When
/// computing the CPI index ratio for a settlement date, the curve applies
/// this lag and linearly interpolates between the lagged monthly CPI values.
/// Use [`cpi_with_lag`](Self::cpi_with_lag) for lag-adjusted lookups, or
/// [`cpi`](Self::cpi) for raw (no-lag) lookups.
///
/// # Use Cases
///
/// - TIPS (Treasury Inflation-Protected Securities) pricing
/// - Inflation swap valuation (zero-coupon and year-on-year)
/// - Real rate curve construction (nominal - breakeven = real)
/// - Pension liability modeling with inflation indexation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawInflationCurve", into = "RawInflationCurve")]
pub struct InflationCurve {
    id: CurveId,
    base_cpi: f64,
    /// Base (valuation) date of the curve.
    base_date: Date,
    /// Day-count basis for time conversions.
    day_count: DayCount,
    /// Indexation lag in months (default: 3 for TIPS/linkers).
    indexation_lag_months: u32,
    /// Knot times in **years**.
    knots: Box<[f64]>,
    /// CPI index levels at each knot.
    cpi_levels: Box<[f64]>,
    interp: Interp,
}

/// Raw serializable state of an InflationCurve
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInflationCurve {
    #[serde(flatten)]
    common_id: super::common::StateId,
    /// Base CPI level at t=0
    pub base_cpi: f64,
    /// Base date
    pub base_date: Date,
    /// Day count convention
    #[serde(default = "default_day_count")]
    pub day_count: DayCount,
    /// Indexation lag in months
    #[serde(default = "default_lag")]
    pub indexation_lag_months: u32,
    #[serde(flatten)]
    points: super::common::StateKnotPoints,
    #[serde(flatten)]
    interp: super::common::StateInterp,
}

fn default_day_count() -> DayCount {
    DayCount::Act365F
}

fn default_lag() -> u32 {
    DEFAULT_INDEXATION_LAG_MONTHS
}

impl From<InflationCurve> for RawInflationCurve {
    fn from(curve: InflationCurve) -> Self {
        let knot_points: Vec<(f64, f64)> = curve
            .knots
            .iter()
            .copied()
            .zip(curve.cpi_levels.iter().copied())
            .collect();

        RawInflationCurve {
            common_id: super::common::StateId {
                id: curve.id.to_string(),
            },
            base_cpi: curve.base_cpi,
            base_date: curve.base_date,
            day_count: curve.day_count,
            indexation_lag_months: curve.indexation_lag_months,
            points: super::common::StateKnotPoints { knot_points },
            interp: super::common::StateInterp {
                interp_style: curve.interp.style(),
                extrapolation: curve.interp.extrapolation(),
            },
        }
    }
}

impl TryFrom<RawInflationCurve> for InflationCurve {
    type Error = crate::Error;

    fn try_from(state: RawInflationCurve) -> crate::Result<Self> {
        InflationCurve::builder(state.common_id.id)
            .base_cpi(state.base_cpi)
            .base_date(state.base_date)
            .day_count(state.day_count)
            .indexation_lag_months(state.indexation_lag_months)
            .knots(state.points.knot_points)
            .interp(state.interp.interp_style)
            .build()
    }
}

impl InflationCurve {
    /// Start building an inflation curve with identifier `id`.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_core::market_data::term_structures::InflationCurve;
    /// use finstack_core::math::interp::InterpStyle;
    /// use finstack_core::dates::Date;
    /// use time::Month;
    ///
    /// let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    /// let curve = InflationCurve::builder("US-CPI")
    ///     .base_date(base)
    ///     .base_cpi(300.0)
    ///     .knots([(0.0, 300.0), (5.0, 325.0)])
    ///     .interp(InterpStyle::LogLinear)
    ///     .build()
    ///     .expect("InflationCurve builder should succeed");
    /// assert!(curve.inflation_rate(0.0, 5.0) > 0.0);
    /// ```
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> InflationCurveBuilder {
        let base_date =
            Date::from_calendar_date(1970, time::Month::January, 1).unwrap_or(time::Date::MIN);
        InflationCurveBuilder {
            id: id.into(),
            base_cpi: 100.0,
            base_date,
            base_date_set: false,
            day_count: DayCount::Act365F,
            indexation_lag_months: DEFAULT_INDEXATION_LAG_MONTHS,
            points: Vec::new(),
            style: InterpStyle::LogLinear,
        }
    }

    /// CPI level at time `t` (years), without indexation lag.
    #[must_use]
    pub fn cpi(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return self.base_cpi;
        }
        self.interp.interp(t)
    }

    /// CPI level at time `t` (years), adjusted for the indexation lag.
    ///
    /// For TIPS/linker pricing, the CPI value at settlement is actually the
    /// CPI value from `indexation_lag_months` earlier, linearly interpolated
    /// between monthly CPI values. This method applies that lag.
    ///
    /// # Arguments
    /// * `t` - Time in years from base date (the settlement date)
    ///
    /// # Returns
    /// CPI level corresponding to `t - lag`, where lag is the configured
    /// indexation lag in years.
    #[must_use]
    pub fn cpi_with_lag(&self, t: f64) -> f64 {
        let lag_years = self.indexation_lag_months as f64 / 12.0;
        self.cpi(t - lag_years)
    }

    /// Annualised inflation rate between `t1` and `t2` using CAGR formula.
    ///
    /// Returns [`f64::NAN`] when `t2 <= t1`, or when `t1` / `t2` are non-finite.
    ///
    /// Uses the Compound Annual Growth Rate, which is the market-standard
    /// formula for annualised inflation:
    /// ```text
    /// π(t₁, t₂) = [I(t₂) / I(t₁)]^(1/(t₂-t₁)) - 1
    /// ```
    ///
    /// This correctly compounds and matches QuantLib/Bloomberg conventions.
    /// For short periods (< 1 year), this equals `(I2/I1)^(1/dt) - 1`
    /// rather than the simple linear approximation `(I2/I1 - 1) / dt`.
    #[must_use]
    pub fn inflation_rate(&self, t1: f64, t2: f64) -> f64 {
        if !(t1.is_finite() && t2.is_finite()) || t2 <= t1 {
            return f64::NAN;
        }
        let c1 = self.cpi(t1);
        let c2 = self.cpi(t2);
        let dt = t2 - t1;
        (c2 / c1).powf(1.0 / dt) - 1.0
    }

    /// Simple (non-compounded) inflation rate between `t1` and `t2`.
    ///
    /// Returns [`f64::NAN`] when `t2 <= t1`, or when `t1` / `t2` are non-finite.
    ///
    /// Returns `(I(t2) / I(t1) - 1) / (t2 - t1)`, which is the simple
    /// linear approximation. For most applications, prefer [`inflation_rate`](Self::inflation_rate)
    /// which uses the correct CAGR formula.
    #[must_use]
    pub fn inflation_rate_simple(&self, t1: f64, t2: f64) -> f64 {
        if !(t1.is_finite() && t2.is_finite()) || t2 <= t1 {
            return f64::NAN;
        }
        let c1 = self.cpi(t1);
        let c2 = self.cpi(t2);
        (c2 / c1 - 1.0) / (t2 - t1)
    }

    /// CPI level on a specific calendar date, without indexation lag.
    ///
    /// This is the date-based equivalent of [`cpi`](Self::cpi), consistent with
    /// `DiscountCurve::df_on_date_curve` and `HazardCurve::sp_on_date`.
    ///
    /// # Errors
    ///
    /// Returns an error if the year fraction calculation fails.
    #[inline]
    #[must_use = "computed CPI level should not be discarded"]
    pub fn cpi_on_date(&self, date: Date) -> crate::Result<f64> {
        let t = self.year_fraction_to(date)?;
        Ok(self.cpi(t))
    }

    /// CPI level on a specific calendar date, adjusted for the indexation lag.
    ///
    /// This is the date-based equivalent of [`cpi_with_lag`](Self::cpi_with_lag).
    ///
    /// # Errors
    ///
    /// Returns an error if the year fraction calculation fails.
    #[inline]
    #[must_use = "computed CPI level should not be discarded"]
    pub fn cpi_with_lag_on_date(&self, date: Date) -> crate::Result<f64> {
        let t = self.year_fraction_to(date)?;
        Ok(self.cpi_with_lag(t))
    }

    /// Annualised inflation rate between two calendar dates using CAGR formula.
    ///
    /// This is the date-based equivalent of [`inflation_rate`](Self::inflation_rate).
    ///
    /// # Errors
    ///
    /// Returns an error if the year fraction calculation fails.
    #[inline]
    #[must_use = "computed inflation rate should not be discarded"]
    pub fn inflation_rate_on_dates(&self, d1: Date, d2: Date) -> crate::Result<f64> {
        let t1 = self.year_fraction_to(d1)?;
        let t2 = self.year_fraction_to(d2)?;
        Ok(self.inflation_rate(t1, t2))
    }

    /// Helper: compute year fraction from base date to target date using curve's day-count.
    #[inline]
    fn year_fraction_to(&self, date: Date) -> crate::Result<f64> {
        super::common::year_fraction_to(self.base_date, date, self.day_count)
    }

    /// Base (valuation) date of the curve.
    #[inline]
    pub fn base_date(&self) -> Date {
        self.base_date
    }

    /// Day count convention used by this curve.
    #[inline]
    pub fn day_count(&self) -> DayCount {
        self.day_count
    }

    /// Indexation lag in months.
    #[inline]
    pub fn indexation_lag_months(&self) -> u32 {
        self.indexation_lag_months
    }

    /// Curve identifier.
    #[inline]
    pub fn id(&self) -> &CurveId {
        &self.id
    }

    /// Underlying bootstrap knot times (years).
    #[inline]
    pub fn knots(&self) -> &[f64] {
        &self.knots
    }

    /// CPI levels provided at each knot.
    #[inline]
    pub fn cpi_levels(&self) -> &[f64] {
        &self.cpi_levels
    }

    /// Base CPI level at t = 0.
    #[inline]
    pub fn base_cpi(&self) -> f64 {
        self.base_cpi
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

    /// Create a builder pre-populated with this curve's data but a new ID.
    pub fn to_builder_with_id(&self, new_id: impl Into<CurveId>) -> InflationCurveBuilder {
        InflationCurve::builder(new_id)
            .base_cpi(self.base_cpi)
            .base_date(self.base_date)
            .day_count(self.day_count)
            .indexation_lag_months(self.indexation_lag_months)
            .knots(
                self.knots
                    .iter()
                    .copied()
                    .zip(self.cpi_levels.iter().copied()),
            )
            .interp(self.interp.style())
    }

    /// Roll the curve forward by a specified number of days.
    ///
    /// This creates a new curve with:
    /// - Base date advanced by `days`
    /// - Knot times shifted backwards (t' = t - dt_years)
    /// - Points with t' <= 0 are filtered out (expired)
    /// - CPI levels are preserved
    /// - Base CPI is updated to the interpolated value at the roll time
    ///
    /// # Arguments
    /// * `days` - Number of days to roll forward
    ///
    /// # Returns
    /// A new inflation curve with shifted knots and updated base CPI.
    ///
    /// # Errors
    /// Returns an error if no knot points remain after filtering expired points.
    pub fn roll_forward(&self, days: i64) -> crate::Result<Self> {
        let new_base = self.base_date + time::Duration::days(days);
        let dt_years =
            self.day_count
                .year_fraction(self.base_date, new_base, DayCountContext::default())?;

        // Get the new base CPI by interpolating at the roll time
        let new_base_cpi = self.cpi(dt_years);

        // Shift knots and filter expired points using shared helper
        let rolled_points = roll_knots(&self.knots, &self.cpi_levels, dt_years);

        if rolled_points.is_empty() {
            return Err(crate::error::InputError::TooFewPoints.into());
        }

        InflationCurve::builder(self.id.clone())
            .base_date(new_base)
            .day_count(self.day_count)
            .indexation_lag_months(self.indexation_lag_months)
            .base_cpi(new_base_cpi)
            .knots(rolled_points)
            .build()
    }
}

// Minimal trait implementation for polymorphism where needed

impl TermStructure for InflationCurve {
    #[inline]
    fn id(&self) -> &CurveId {
        &self.id
    }
}

/// Fluent builder for [`InflationCurve`].
///
/// # Examples
///
/// ```rust
/// use finstack_core::market_data::term_structures::InflationCurve;
/// use finstack_core::math::interp::InterpStyle;
/// use finstack_core::dates::Date;
/// use time::Month;
///
/// let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
/// let curve = InflationCurve::builder("US-CPI")
///     .base_date(base)
///     .base_cpi(300.0)
///     .knots([(0.0, 300.0), (5.0, 327.0)])
///     .interp(InterpStyle::LogLinear)
///     .build()
///     .expect("InflationCurve builder should succeed");
/// assert!(curve.inflation_rate(0.0, 5.0) > 0.0);
/// ```
pub struct InflationCurveBuilder {
    id: CurveId,
    base_cpi: f64,
    base_date: Date,
    base_date_set: bool,
    day_count: DayCount,
    indexation_lag_months: u32,
    points: Vec<(f64, f64)>, // (t, cpi)
    style: InterpStyle,
}

impl InflationCurveBuilder {
    /// Set the **base CPI** level at t = 0.
    pub fn base_cpi(mut self, cpi: f64) -> Self {
        self.base_cpi = cpi;
        self
    }

    /// Override the default **base date** (valuation date).
    pub fn base_date(mut self, d: Date) -> Self {
        self.base_date = d;
        self.base_date_set = true;
        self
    }

    /// Choose the day-count basis for time calculations.
    pub fn day_count(mut self, dc: DayCount) -> Self {
        self.day_count = dc;
        self
    }

    /// Set the indexation lag in months.
    ///
    /// Default is 3 months (Canadian model, used by TIPS, IL Gilts, etc.).
    /// Set to 0 to disable lag adjustment.
    pub fn indexation_lag_months(mut self, months: u32) -> Self {
        self.indexation_lag_months = months;
        self
    }

    /// Supply knot points `(t, cpi_level)`.
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

    /// Validate input and build the [`InflationCurve`].
    pub fn build(self) -> crate::Result<InflationCurve> {
        if !self.base_date_set {
            return Err(InputError::Invalid.into());
        }
        if self.points.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }
        crate::math::interp::utils::validate_knots(
            &self.points.iter().map(|p| p.0).collect::<Vec<_>>(),
        )?;
        if self.points.iter().any(|&(_, c)| c <= 0.0) {
            return Err(InputError::NonPositiveValue.into());
        }
        let (kvec, cvec): (Vec<f64>, Vec<f64>) = split_points(self.points);
        crate::math::interp::utils::validate_knots(&kvec)?;
        let knots = kvec.into_boxed_slice();
        let cpi_levels = cvec.into_boxed_slice();
        let interp = build_interp(
            self.style,
            knots.clone(),
            cpi_levels.clone(),
            ExtrapolationPolicy::default(),
        )?;
        Ok(InflationCurve {
            id: self.id,
            base_cpi: self.base_cpi,
            base_date: self.base_date,
            day_count: self.day_count,
            indexation_lag_months: self.indexation_lag_months,
            knots,
            cpi_levels,
            interp,
        })
    }
}

// -----------------------------------------------------------------------------
// Serialization support
// -----------------------------------------------------------------------------

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    fn sample_curve() -> InflationCurve {
        let base =
            Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date");
        InflationCurve::builder("US-CPI")
            .base_date(base)
            .base_cpi(300.0)
            .knots([(0.0, 300.0), (1.0, 306.0), (2.0, 312.0)])
            .build()
            .expect("InflationCurve builder should succeed in test")
    }

    #[test]
    fn cpi_hits_knots() {
        let ic = sample_curve();
        assert!((ic.cpi(1.0) - 306.0).abs() < 1e-9);
    }

    #[test]
    fn inflation_rate_positive() {
        let ic = sample_curve();
        let r = ic.inflation_rate(0.0, 1.0);
        assert!(r > 0.0);
    }

    #[test]
    fn inflation_rate_rejects_non_increasing_times_with_nan() {
        let ic = sample_curve();
        assert!(ic.inflation_rate(1.0, 0.0).is_nan());
        assert!(ic.inflation_rate(1.0, 1.0).is_nan());
    }

    #[test]
    fn inflation_rate_simple_rejects_non_increasing_times_with_nan() {
        let ic = sample_curve();
        assert!(ic.inflation_rate_simple(1.0, 0.0).is_nan());
        assert!(ic.inflation_rate_simple(1.0, 1.0).is_nan());
    }

    #[test]
    fn inflation_rate_rejects_non_finite_times_with_nan() {
        let ic = sample_curve();
        assert!(ic.inflation_rate(f64::NAN, 1.0).is_nan());
        assert!(ic.inflation_rate(0.0, f64::INFINITY).is_nan());
    }

    #[test]
    fn inflation_rate_uses_cagr() {
        let ic = sample_curve();
        // CPI goes from 300 to 306 in 1 year → CAGR = (306/300)^1 - 1 = 2%
        let r = ic.inflation_rate(0.0, 1.0);
        assert!(
            (r - 0.02).abs() < 1e-6,
            "Expected ~2% CAGR inflation rate, got {:.4}%",
            r * 100.0
        );
    }

    #[test]
    fn inflation_rate_simple_differs_from_cagr() {
        let ic = sample_curve();
        let cagr = ic.inflation_rate(0.0, 2.0);
        let simple = ic.inflation_rate_simple(0.0, 2.0);
        // For multi-year periods, CAGR and simple rates should differ
        assert!(
            (cagr - simple).abs() > 1e-8,
            "CAGR ({cagr}) and simple ({simple}) should differ over 2 years"
        );
    }

    #[test]
    fn cpi_with_lag_applies_3_month_lag() {
        let ic = sample_curve();
        // At t=1.0 with 3-month lag, should return CPI at t=0.75
        let lagged = ic.cpi_with_lag(1.0);
        let direct = ic.cpi(0.75);
        assert!(
            (lagged - direct).abs() < 1e-12,
            "Lagged CPI at t=1.0 should equal CPI at t=0.75"
        );
    }

    #[test]
    fn base_date_is_set() {
        let ic = sample_curve();
        let expected =
            Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date");
        assert_eq!(ic.base_date(), expected);
    }

    #[test]
    fn roll_forward_uses_day_count() {
        let base =
            Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid test date");
        let ic = InflationCurve::builder("CPI-ROLL")
            .base_date(base)
            .base_cpi(300.0)
            .knots([
                (0.5, 303.0),
                (1.0, 306.0),
                (2.0, 312.0),
                (5.0, 330.0),
                (10.0, 360.0),
            ])
            .build()
            .expect("InflationCurve builder should succeed in test");

        let rolled = ic.roll_forward(365).expect("roll_forward should succeed");
        // After rolling 365 days (~1 year), base date should advance
        assert!(rolled.base_date() > base);
        // And knot count should decrease as early knots expire
        assert!(rolled.knots().len() < ic.knots().len());
    }
}
