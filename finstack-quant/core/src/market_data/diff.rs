//! Market change measurement primitives for curves, inflation, FX, and scalars.
//!
//! Each supported market quantity uses an explicit source coordinate or named
//! contract coordinate, validates input domain and comparability invariants
//! before evaluating differences, preserves declared row ordering, and fails
//! closed on invalid or incomparable inputs rather than falling back across
//! market-data families or proxies.
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::dates::DayCount;
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::market_data::diff::{
//!     DateInterval, FactorRateDiffQuery, RateDiffScale, measure_discount_factor_rate_diff,
//! };
//! use finstack_quant_core::market_data::term_structures::DiscountCurve;
//! use time::macros::date;
//!
//! # fn main() -> finstack_quant_core::Result<()> {
//! let base = date!(2025 - 01 - 01);
//! let c0 = DiscountCurve::flat("USD-OIS", base, 0.04, DayCount::Act365F)?;
//! let c1 = DiscountCurve::flat("USD-OIS", base, 0.0425, DayCount::Act365F)?;
//! let m0 = MarketContext::new().try_insert(c0)?;
//! let m1 = MarketContext::new().try_insert(c1)?;
//!
//! let intervals = [DateInterval::new(date!(2025 - 07 - 01), date!(2026 - 07 - 01))?];
//! let query = FactorRateDiffQuery::new(
//!     "USD-OIS",
//!     &intervals,
//!     DayCount::Act365F,
//!     RateDiffScale::BasisPoints,
//! );
//! let result = measure_discount_factor_rate_diff(&query, &m0, &m1)?;
//! assert!((result.rows()[0].diff - 25.0).abs() < 1e-10);
//! # Ok(())
//! # }
//! ```

use std::collections::BTreeSet;
use std::fmt;

use time::Month;

use crate::currency::Currency;
use crate::dates::{Date, DayCount, DayCountContext};
use crate::error::InputError;
use crate::market_data::context::MarketContext;
use crate::math::stats;
use crate::money::Money;
use crate::money::fx::{FxConversionPolicy, FxQuery};
use crate::types::CurveId;
use crate::{Error, Result};

/// Output scale for continuous zero-rate, forward-rate, or growth-rate shifts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateDiffScale {
    /// Rate difference in annual decimal units (`0.0001` = 1 bp).
    Decimal,
    /// Rate difference in basis points (`1.0` = `0.0001` decimal).
    BasisPoints,
}

impl RateDiffScale {
    #[inline]
    const fn scale_factor(self) -> f64 {
        match self {
            Self::Decimal => 1.0,
            Self::BasisPoints => 10_000.0,
        }
    }
}

impl fmt::Display for RateDiffScale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decimal => f.write_str("decimal"),
            Self::BasisPoints => f.write_str("bp"),
        }
    }
}

/// Output scale for native piecewise-constant hazard intensity differences.
///
/// Always measures raw hazard intensity `λ` per source year without converting to
/// par CDS spreads or coupling to recovery assumptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HazardDiffScale {
    /// Intensity difference per source year (`λ₁ - λ₀`).
    Decimal,
    /// Intensity difference scaled by `10_000` (`intensity bp`).
    IntensityBp,
}

impl HazardDiffScale {
    #[inline]
    const fn scale_factor(self) -> f64 {
        match self {
            Self::Decimal => 1.0,
            Self::IntensityBp => 10_000.0,
        }
    }
}

impl fmt::Display for HazardDiffScale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decimal => f.write_str("decimal"),
            Self::IntensityBp => f.write_str("intensity_bp"),
        }
    }
}

/// Output scale for exact algebraic proportional changes `(x₁ - x₀) / x₀`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProportionalDiffScale {
    /// Unscaled decimal fraction (`(x₁ - x₀) / x₀`).
    Fraction,
    /// Percentage points (`100.0 * (x₁ - x₀) / x₀`).
    Percent,
}

impl ProportionalDiffScale {
    #[inline]
    const fn scale_factor(self) -> f64 {
        match self {
            Self::Fraction => 1.0,
            Self::Percent => 100.0,
        }
    }
}

impl fmt::Display for ProportionalDiffScale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fraction => f.write_str("fraction"),
            Self::Percent => f.write_str("percent"),
        }
    }
}

/// Output scale for observed monthly CPI level differences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationDiffScale {
    /// Native CPI index-point difference `I₁ - I₀`.
    NativePoints,
    /// Unannualized relative-level change `(I₁ - I₀) / I₀` on the chosen scale.
    Proportional(ProportionalDiffScale),
}

/// Classification of a same-period monthly CPI comparison across snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationChangeKind {
    /// Both qualified snapshots observed the same index level for that month.
    Unchanged,
    /// The qualified level for the same calendar month changed across snapshots.
    Revision,
    /// Explicit cross-period comparison across distinct `(year, month)` snapshots.
    CrossPeriod,
}

/// Trait implemented by ordered measurement rows that report a homogeneous scalar `f64` diff.
pub trait MeasuredDiffValue {
    /// Return the row's measured difference in its declared unit scale.
    fn measured_diff(&self) -> f64;
}

/// Ordered, non-empty measurement vector returned by term-structure or observation diffs.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DiffMeasurement<Row> {
    rows: Vec<Row>,
}

impl<Row> DiffMeasurement<Row> {
    fn from_validated_rows(rows: Vec<Row>) -> Result<Self> {
        if rows.is_empty() {
            return Err(Error::Validation(
                "measurement row set must not be empty".to_string(),
            ));
        }
        Ok(Self { rows })
    }

    /// Borrow the measured rows in the caller's declared coordinate order.
    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Number of rows in the measurement set (always at least `1`).
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Returns `false` because validated measurements are always non-empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Consume the container and return the owned ordered rows.
    #[must_use]
    pub fn into_rows(self) -> Vec<Row> {
        self.rows
    }
}

impl<Row: MeasuredDiffValue> DiffMeasurement<Row> {
    /// Compute the equal-weight arithmetic mean of the measured row values using compensated summation.
    ///
    /// Uses [`finstack_quant_core::math::stats::mean`](crate::math::stats::mean) so extreme
    /// magnitudes are rescaled without overflow or premature sum collapse.
    ///
    /// # Errors
    /// Returns [`Error::Validation`] if the row set is empty or any row value is non-finite.
    pub fn mean(&self) -> Result<f64> {
        let values: Vec<f64> = self
            .rows
            .iter()
            .map(MeasuredDiffValue::measured_diff)
            .collect();
        let avg = stats::mean(&values)?.ok_or_else(|| {
            Error::Validation("cannot compute mean of an empty measurement set".to_string())
        })?;
        if !avg.is_finite() {
            return Err(Error::Validation(format!(
                "measurement mean produced non-finite value {avg}"
            )));
        }
        Ok(normalize_zero(avg))
    }
}

/// Strict calendar interval `[anchor, maturity]` with `maturity > anchor`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct DateInterval {
    /// Anchor date `V` for the comparison interval.
    pub anchor: Date,
    /// Maturity date `T` for the comparison interval; strictly after `anchor`.
    pub maturity: Date,
}

impl DateInterval {
    /// Construct a validated calendar interval with `maturity > anchor`.
    ///
    /// # Arguments
    /// - `anchor` - Earlier comparison coordinate `V`.
    /// - `maturity` - Later comparison coordinate `T`; must be strictly greater than `anchor`.
    ///
    /// # Errors
    /// Returns [`Error::Validation`] if `maturity <= anchor`.
    pub fn new(anchor: Date, maturity: Date) -> Result<Self> {
        if maturity <= anchor {
            return Err(Error::Validation(format!(
                "interval maturity ({maturity}) must be strictly after anchor ({anchor})"
            )));
        }
        Ok(Self { anchor, maturity })
    }
}

/// Inputs for [`zero_rate_difference`] over two snapshots' factor endpoints `[V, T]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZeroRateDiffParams {
    /// Earlier snapshot factor at the common anchor coordinate `D₀(V)`.
    pub d0_anchor: f64,
    /// Earlier snapshot factor at the common maturity coordinate `D₀(T)`.
    pub d0_maturity: f64,
    /// Later snapshot factor at the common anchor coordinate `D₁(V)`.
    pub d1_anchor: f64,
    /// Later snapshot factor at the common maturity coordinate `D₁(T)`.
    pub d1_maturity: f64,
    /// Strictly positive reporting year fraction `α` corresponding to `(V, T)`.
    pub alpha: f64,
    /// Output rate scale (`Decimal` or `BasisPoints`).
    pub scale: RateDiffScale,
}

/// Compute the four-endpoint continuously compounded rate difference `(ℓ₀ - ℓ₁) / α`.
///
/// Here `ℓₛ = ln(Dₛ(T)) - ln(Dₛ(V))`, so positive output indicates higher rates in snapshot `1`
/// than in snapshot `0`:
/// `Δz = (1 / α) * ln((D₀(T) * D₁(V)) / (D₀(V) * D₁(T)))`.
///
/// Validates `alpha > 0` and strictly positive finite endpoints before evaluating any equal-value
/// path, and uses binary-exponent decomposition with FMA `diff_of_products` and `ln_1p` near unit
/// cross-ratios to avoid overflow and catastrophic cancellation.
///
/// # Arguments
/// - `params` - Validated four-endpoint factors, positive year fraction `α`, and output scale.
///
/// # Errors
/// Returns [`Error::Validation`] if `alpha` is non-finite or non-positive, any factor is
/// non-finite or non-positive, or the scaled output overflows `f64`.
pub fn zero_rate_difference(params: ZeroRateDiffParams) -> Result<f64> {
    if !params.alpha.is_finite() || params.alpha <= 0.0 {
        return Err(Error::Validation(format!(
            "year fraction alpha must be finite and strictly positive, got {}",
            params.alpha
        )));
    }
    for (name, val) in [
        ("d0_anchor", params.d0_anchor),
        ("d0_maturity", params.d0_maturity),
        ("d1_anchor", params.d1_anchor),
        ("d1_maturity", params.d1_maturity),
    ] {
        if !val.is_finite() || val <= 0.0 {
            return Err(Error::Validation(format!(
                "factor '{name}' must be finite and strictly positive, got {val}"
            )));
        }
    }

    let ln_cross = super::term_structures::common::checked_log_cross_ratio(
        params.d0_maturity,
        params.d1_anchor,
        params.d0_anchor,
        params.d1_maturity,
    );
    let out = (ln_cross / params.alpha) * params.scale.scale_factor();
    if !out.is_finite() || (!exact_f64_eq(ln_cross, 0.0) && exact_f64_eq(out, 0.0)) {
        return Err(Error::Validation(format!(
            "zero rate difference overflowed or underflowed f64: ln_cross={ln_cross}, alpha={}",
            params.alpha
        )));
    }
    Ok(normalize_zero(out))
}

/// Measure the continuously compounded zero-rate spread `(r_foreign - r_domestic)`
/// between two explicitly selected [`Discounting`](super::traits::Discounting)
/// sources over a dated reporting interval `(anchor, maturity)`.
///
/// Computes `(1 / alpha) * ln((D_D(T) * D_F(V)) / (D_D(V) * D_F(T)))` after
/// admitting all four absolute factor endpoints on each source's own clock
/// and computing `alpha = day_count.year_fraction(anchor, maturity, day_count_context)? > 0`.
///
/// # Arguments
///
/// * `domestic` - Selected domestic [`Discounting`](super::traits::Discounting) curve provider.
/// * `foreign` - Selected foreign [`Discounting`](super::traits::Discounting) curve provider.
/// * `anchor` - Common interval start date `V`.
/// * `maturity` - Common interval end date `T` (`T > V`).
/// * `day_count` - Explicit reporting day-count convention supplying `alpha`.
/// * `day_count_context` - Complete day-count context (calendar, frequency, bus basis) required by `day_count`.
/// * `scale` - Output rate units (`Decimal` or `BasisPoints`).
///
/// # Errors
///
/// Returns an error if `maturity <= anchor`, `day_count` fails or produces a
/// non-positive `alpha`, any source endpoint factor cannot be admitted, or
/// the difference overflows/underflows `f64`.
pub fn measure_discounting_zero_rate_difference(
    domestic: &dyn super::traits::Discounting,
    foreign: &dyn super::traits::Discounting,
    anchor: Date,
    maturity: Date,
    day_count: DayCount,
    day_count_context: DayCountContext<'_>,
    scale: RateDiffScale,
) -> Result<f64> {
    if maturity <= anchor {
        return Err(Error::Validation(format!(
            "zero-rate difference maturity ({maturity}) must be strictly after anchor ({anchor})"
        )));
    }
    let alpha = day_count.year_fraction(anchor, maturity, day_count_context)?;
    if !alpha.is_finite() || alpha <= 0.0 {
        return Err(Error::Validation(format!(
            "reporting day count {day_count} produced invalid year fraction {alpha} for ({anchor}, {maturity})"
        )));
    }
    let d0_anchor = domestic.df_on_date(anchor)?;
    let d0_maturity = domestic.df_on_date(maturity)?;
    let d1_anchor = foreign.df_on_date(anchor)?;
    let d1_maturity = foreign.df_on_date(maturity)?;
    zero_rate_difference(ZeroRateDiffParams {
        d0_anchor,
        d0_maturity,
        d1_anchor,
        d1_maturity,
        alpha,
        scale,
    })
}

/// Request parameters for measuring rate shifts on discount or forward curves.
#[derive(Debug, Clone)]
pub struct FactorRateDiffQuery<'a> {
    /// Curve identifier in the earlier snapshot `market_t0`.
    pub source_t0: CurveId,
    /// Curve identifier in the later snapshot `market_t1`.
    pub source_t1: CurveId,
    /// Non-empty, duplicate-free ordered slice of `[anchor, maturity]` intervals.
    pub intervals: &'a [DateInterval],
    /// Complete reporting day-count convention used to compute interval `α`.
    pub day_count: DayCount,
    /// Additional day-count context (calendar, frequency, bus basis) required by `day_count`.
    pub day_count_context: DayCountContext<'a>,
    /// Output unit scale (`Decimal` or `BasisPoints`).
    pub scale: RateDiffScale,
}

impl<'a> FactorRateDiffQuery<'a> {
    /// Create a query comparing the same curve identifier across two snapshots.
    ///
    /// # Arguments
    /// - `curve_id` - Curve identifier present in both `market_t0` and `market_t1`.
    /// - `intervals` - Non-empty ordered unique comparison intervals with `maturity > anchor`.
    /// - `day_count` - Reporting day-count basis for interval year fractions.
    /// - `scale` - Output rate scale (`Decimal` or `BasisPoints`).
    #[must_use]
    pub fn new(
        curve_id: impl Into<CurveId>,
        intervals: &'a [DateInterval],
        day_count: DayCount,
        scale: RateDiffScale,
    ) -> Self {
        let id = curve_id.into();
        Self {
            source_t0: id.clone(),
            source_t1: id,
            intervals,
            day_count,
            day_count_context: DayCountContext::default(),
            scale,
        }
    }

    /// Override the earlier and later curve identifiers explicitly when comparing renamed curves.
    ///
    /// # Arguments
    /// - `source_t0` - Curve identifier in the earlier market snapshot.
    /// - `source_t1` - Curve identifier in the later market snapshot.
    #[must_use]
    pub fn with_source_ids(
        mut self,
        source_t0: impl Into<CurveId>,
        source_t1: impl Into<CurveId>,
    ) -> Self {
        self.source_t0 = source_t0.into();
        self.source_t1 = source_t1.into();
        self
    }

    /// Supply a reporting [`DayCountContext`] (e.g., holiday calendar for `Bus252`).
    ///
    /// # Arguments
    /// - `context` - Reporting day-count context passed to [`DayCount::year_fraction`].
    #[must_use]
    pub fn with_day_count_context(mut self, context: DayCountContext<'a>) -> Self {
        self.day_count_context = context;
        self
    }
}

/// Measured continuously compounded factor-rate change on a single `[anchor, maturity]` interval.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RateDiffRow {
    /// Source curve identifier in `market_t0`.
    pub source_t0: CurveId,
    /// Source curve identifier in `market_t1`.
    pub source_t1: CurveId,
    /// Common comparison anchor date `V`.
    pub anchor: Date,
    /// Common comparison maturity date `T`.
    pub maturity: Date,
    /// Reporting day-count convention used to compute `year_fraction`.
    pub day_count: DayCount,
    /// Positive reporting year fraction `α` for `[anchor, maturity]`.
    pub year_fraction: f64,
    /// Output unit scale of `diff`.
    pub scale: RateDiffScale,
    /// Measured rate change `Δz` in `scale` units (positive when rates rose).
    pub diff: f64,
}

impl MeasuredDiffValue for RateDiffRow {
    fn measured_diff(&self) -> f64 {
        self.diff
    }
}

/// Measure discount-curve zero-rate changes across common explicit intervals `[anchor, maturity]`.
///
/// Evaluates `market_t0.get_discount` and `market_t1.get_discount` directly on each curve's own
/// clock (`df_on_date`) with no fallback to forward curves.
///
/// # Arguments
/// - `query` - Factor-rate query specifying source IDs, intervals, day count, and scale.
/// - `market_t0` - Earlier market snapshot.
/// - `market_t1` - Later market snapshot.
///
/// # Errors
/// Returns an error if either discount curve is missing, `intervals` is empty, contains
/// duplicate `(anchor, maturity)` pairs, contains invalid dates (`maturity <= anchor` or
/// `anchor` predating either curve's `source_base_date()`), reporting day-count evaluation
/// fails, or any factor/output is invalid or non-finite.
pub fn measure_discount_factor_rate_diff(
    query: &FactorRateDiffQuery<'_>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<DiffMeasurement<RateDiffRow>> {
    let c0 = market_t0.get_discount(query.source_t0.as_str())?;
    let c1 = market_t1.get_discount(query.source_t1.as_str())?;
    measure_curve_factor_diff(
        query,
        c0.source_base_date(),
        c1.source_base_date(),
        |d| c0.df_on_date(d),
        |d| c1.df_on_date(d),
    )
}

/// Measure forward-curve projection rate changes across common explicit intervals `[anchor, maturity]`.
///
/// Evaluates `market_t0.get_forward` and `market_t1.get_forward` directly (`df_on_date`) with
/// no fallback to discount curves.
///
/// # Arguments
/// - `query` - Factor-rate query specifying source IDs, intervals, day count, and scale.
/// - `market_t0` - Earlier market snapshot.
/// - `market_t1` - Later market snapshot.
///
/// # Errors
/// Returns an error if either forward curve is missing, `intervals` is empty, contains
/// duplicate `(anchor, maturity)` pairs, contains invalid dates (`maturity <= anchor` or
/// `anchor` predating either curve's `source_base_date()`), reporting day-count evaluation
/// fails, or any factor/output is invalid or non-finite.
pub fn measure_forward_factor_rate_diff(
    query: &FactorRateDiffQuery<'_>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<DiffMeasurement<RateDiffRow>> {
    let c0 = market_t0.get_forward(query.source_t0.as_str())?;
    let c1 = market_t1.get_forward(query.source_t1.as_str())?;
    measure_curve_factor_diff(
        query,
        c0.source_base_date(),
        c1.source_base_date(),
        |d| c0.df_on_date(d),
        |d| c1.df_on_date(d),
    )
}

fn measure_curve_factor_diff<F0, F1>(
    query: &FactorRateDiffQuery<'_>,
    src_base_0: Date,
    src_base_1: Date,
    mut eval0: F0,
    mut eval1: F1,
) -> Result<DiffMeasurement<RateDiffRow>>
where
    F0: FnMut(Date) -> Result<f64>,
    F1: FnMut(Date) -> Result<f64>,
{
    if query.intervals.is_empty() {
        return Err(Error::Validation(
            "factor rate diff requires at least one interval".to_string(),
        ));
    }
    let mut seen = BTreeSet::new();
    let mut rows = Vec::with_capacity(query.intervals.len());
    for interval in query.intervals {
        if interval.maturity <= interval.anchor {
            return Err(Error::Validation(format!(
                "interval maturity ({}) must be strictly after anchor ({})",
                interval.maturity, interval.anchor
            )));
        }
        if !seen.insert((interval.anchor, interval.maturity)) {
            return Err(Error::Validation(format!(
                "duplicate comparison interval ({}, {})",
                interval.anchor, interval.maturity
            )));
        }
        if interval.anchor < src_base_0 || interval.anchor < src_base_1 {
            return Err(Error::Validation(format!(
                "interval anchor ({}) predates curve source base dates ({src_base_0}, {src_base_1})",
                interval.anchor
            )));
        }
        let alpha = query.day_count.year_fraction(
            interval.anchor,
            interval.maturity,
            query.day_count_context,
        )?;
        if !alpha.is_finite() || alpha <= 0.0 {
            return Err(Error::Validation(format!(
                "reporting day count {} produced invalid year fraction {alpha} for ({}, {})",
                query.day_count, interval.anchor, interval.maturity
            )));
        }
        let d0_anchor = eval0(interval.anchor)?;
        let d0_maturity = eval0(interval.maturity)?;
        let d1_anchor = eval1(interval.anchor)?;
        let d1_maturity = eval1(interval.maturity)?;
        let diff = zero_rate_difference(ZeroRateDiffParams {
            d0_anchor,
            d0_maturity,
            d1_anchor,
            d1_maturity,
            alpha,
            scale: query.scale,
        })?;
        rows.push(RateDiffRow {
            source_t0: query.source_t0.clone(),
            source_t1: query.source_t1.clone(),
            anchor: interval.anchor,
            maturity: interval.maturity,
            day_count: query.day_count,
            year_fraction: alpha,
            scale: query.scale,
            diff,
        });
    }
    DiffMeasurement::from_validated_rows(rows)
}

/// Request parameters for comparing native piecewise-constant hazard intensities.
#[derive(Debug, Clone)]
pub struct HazardDiffQuery<'a> {
    /// Hazard curve identifier in `market_t0`.
    pub source_t0: CurveId,
    /// Hazard curve identifier in `market_t1`.
    pub source_t1: CurveId,
    /// Strictly increasing non-negative source-year times relative to `source_base_date()`.
    pub times: &'a [f64],
    /// Native intensity output scale (`Decimal` or `IntensityBp`).
    pub scale: HazardDiffScale,
}

impl<'a> HazardDiffQuery<'a> {
    /// Create a native hazard intensity diff query for the same curve ID on both snapshots.
    ///
    /// # Arguments
    /// - `curve_id` - Hazard curve identifier in `market_t0` and `market_t1`.
    /// - `times` - Non-empty strictly increasing non-negative times on the source curve clock.
    /// - `scale` - Output hazard intensity scale.
    #[must_use]
    pub fn new(curve_id: impl Into<CurveId>, times: &'a [f64], scale: HazardDiffScale) -> Self {
        let id = curve_id.into();
        Self {
            source_t0: id.clone(),
            source_t1: id,
            times,
            scale,
        }
    }

    /// Override the earlier and later hazard curve identifiers explicitly.
    ///
    /// # Arguments
    /// - `source_t0` - Hazard curve identifier in the earlier snapshot.
    /// - `source_t1` - Hazard curve identifier in the later snapshot.
    #[must_use]
    pub fn with_source_ids(
        mut self,
        source_t0: impl Into<CurveId>,
        source_t1: impl Into<CurveId>,
    ) -> Self {
        self.source_t0 = source_t0.into();
        self.source_t1 = source_t1.into();
        self
    }
}

/// Native hazard intensity difference at a single source-clock time coordinate.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HazardDiffRow {
    /// Source curve identifier in `market_t0`.
    pub source_t0: CurveId,
    /// Source curve identifier in `market_t1`.
    pub source_t1: CurveId,
    /// Shared original unconditioned source base date.
    pub source_base_date: Date,
    /// Shared source day-count convention.
    pub day_count: DayCount,
    /// Source-clock evaluation time in years from `source_base_date`.
    pub time_years: f64,
    /// Intensity output scale (`Decimal` or `IntensityBp`).
    pub scale: HazardDiffScale,
    /// Measured intensity change `(λ₁ - λ₀) * scale_factor`.
    pub diff: f64,
}

impl MeasuredDiffValue for HazardDiffRow {
    fn measured_diff(&self) -> f64 {
        self.diff
    }
}

/// Measure native hazard intensity differences `λ₁(t) - λ₀(t)` on a compatible source clock.
///
/// Evaluates both curves via [`HazardCurve::source_hazard_rate`](crate::market_data::term_structures::HazardCurve::source_hazard_rate)
/// on their retained pre-conditioning coordinate schedule, without inferring CDS spreads or
/// falling back to discount curves.
///
/// # Arguments
/// - `query` - Hazard query naming source curves, strictly increasing `times`, and scale.
/// - `market_t0` - Earlier market snapshot.
/// - `market_t1` - Later market snapshot.
///
/// # Errors
/// Returns an error if either hazard curve is missing, `source_base_date()` or `day_count()`
/// differs between snapshots, `times` is empty, contains non-finite, negative, duplicate, or
/// unsorted times, or any evaluated hazard rate/diff is non-finite.
pub fn measure_hazard_intensity_diff(
    query: &HazardDiffQuery<'_>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<DiffMeasurement<HazardDiffRow>> {
    if query.times.is_empty() {
        return Err(Error::Validation(
            "hazard intensity diff requires at least one time point".to_string(),
        ));
    }
    let c0 = market_t0.get_hazard(query.source_t0.as_str())?;
    let c1 = market_t1.get_hazard(query.source_t1.as_str())?;
    if c0.source_base_date() != c1.source_base_date() {
        return Err(Error::Validation(format!(
            "hazard curves have incompatible source base dates ({} vs {})",
            c0.source_base_date(),
            c1.source_base_date()
        )));
    }
    if c0.day_count() != c1.day_count() {
        return Err(Error::Validation(format!(
            "hazard curves have incompatible day counts ({} vs {})",
            c0.day_count(),
            c1.day_count()
        )));
    }

    let mut rows = Vec::with_capacity(query.times.len());
    let mut prev_t: Option<f64> = None;
    for &t in query.times {
        if !t.is_finite() || t < 0.0 {
            return Err(Error::Validation(format!(
                "hazard query time must be finite and non-negative, got {t}"
            )));
        }
        if let Some(prev) = prev_t
            && t <= prev
        {
            return Err(Error::Validation(format!(
                "hazard query times must be strictly increasing ({t} after {prev})"
            )));
        }
        prev_t = Some(t);

        let l0 = c0.source_hazard_rate(t)?;
        let l1 = c1.source_hazard_rate(t)?;
        if !l0.is_finite() || !l1.is_finite() {
            return Err(Error::Validation(format!(
                "non-finite hazard rate at t={t}: ({l0}, {l1})"
            )));
        }
        let delta = finite_f64_diff(l0, l1, query.scale.scale_factor())?;
        rows.push(HazardDiffRow {
            source_t0: query.source_t0.clone(),
            source_t1: query.source_t1.clone(),
            source_base_date: c0.source_base_date(),
            day_count: c0.day_count(),
            time_years: t,
            scale: query.scale,
            diff: delta,
        });
    }
    DiffMeasurement::from_validated_rows(rows)
}

/// Request parameters for comparing projected CPI levels across two
/// [`InflationCurve`](crate::market_data::term_structures::InflationCurve) snapshots.
#[derive(Debug, Clone)]
pub struct InflationCpiDiffQuery<'a> {
    /// Inflation curve identifier in `market_t0`.
    pub source_t0: CurveId,
    /// Inflation curve identifier in `market_t1`.
    pub source_t1: CurveId,
    /// Non-empty strictly increasing evaluation dates on or after both curves' base dates.
    pub dates: &'a [Date],
}

impl<'a> InflationCpiDiffQuery<'a> {
    /// Create a projected CPI level comparison query for matching curve IDs.
    ///
    /// # Arguments
    /// - `curve_id` - Inflation curve identifier in both `market_t0` and `market_t1`.
    /// - `dates` - Non-empty strictly increasing calendar dates to evaluate.
    #[must_use]
    pub fn new(curve_id: impl Into<CurveId>, dates: &'a [Date]) -> Self {
        let id = curve_id.into();
        Self {
            source_t0: id.clone(),
            source_t1: id,
            dates,
        }
    }

    /// Override the earlier and later curve identifiers explicitly.
    ///
    /// # Arguments
    /// - `source_t0` - Inflation curve identifier in the earlier snapshot.
    /// - `source_t1` - Inflation curve identifier in the later snapshot.
    #[must_use]
    pub fn with_source_ids(
        mut self,
        source_t0: impl Into<CurveId>,
        source_t1: impl Into<CurveId>,
    ) -> Self {
        self.source_t0 = source_t0.into();
        self.source_t1 = source_t1.into();
        self
    }
}

/// Native projected CPI level difference at a single explicit calendar date.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InflationCpiDiffRow {
    /// Source inflation curve identifier in `market_t0`.
    pub source_t0: CurveId,
    /// Source inflation curve identifier in `market_t1`.
    pub source_t1: CurveId,
    /// Comparison calendar date.
    pub date: Date,
    /// Projected native CPI index-point difference `I₁(date) - I₀(date)`.
    pub diff: f64,
}

impl MeasuredDiffValue for InflationCpiDiffRow {
    fn measured_diff(&self) -> f64 {
        self.diff
    }
}

fn require_matching_inflation_series(
    earlier: Option<&crate::market_data::scalars::InflationSeries>,
    later: Option<&crate::market_data::scalars::InflationSeries>,
) -> Result<()> {
    match (earlier, later) {
        (Some(a), Some(b)) if a == b => Ok(()),
        _ => Err(Error::Validation("CPI comparison requires matching published series, reference base and seasonal adjustment on both snapshots".into())),
    }
}

/// Measure projected CPI level differences `I₁(d) - I₀(d)` directly on
/// [`InflationCurve`](crate::market_data::term_structures::InflationCurve)s.
///
/// Fails closed if either curve is missing or invalid, without falling back to
/// [`InflationIndex`](crate::market_data::scalars::InflationIndex).
///
/// # Arguments
/// - `query` - Projected CPI query naming source curves and strictly increasing `dates`.
/// - `market_t0` - Earlier market snapshot.
/// - `market_t1` - Later market snapshot.
///
/// # Errors
/// Returns an error if either inflation curve is missing, `dates` is empty or not strictly
/// increasing, any date predates either curve's `base_date()`, or any evaluated CPI level or
/// difference is non-positive/non-finite.
pub fn measure_inflation_cpi_level_diff(
    query: &InflationCpiDiffQuery<'_>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<DiffMeasurement<InflationCpiDiffRow>> {
    if query.dates.is_empty() {
        return Err(Error::Validation(
            "projected CPI level diff requires at least one date".to_string(),
        ));
    }
    let c0 = market_t0.get_inflation_curve(query.source_t0.as_str())?;
    let c1 = market_t1.get_inflation_curve(query.source_t1.as_str())?;
    require_matching_inflation_series(c0.get_series(), c1.get_series())?;
    let mut rows = Vec::with_capacity(query.dates.len());
    let mut prev_date: Option<Date> = None;

    for &date in query.dates {
        if let Some(prev) = prev_date
            && date <= prev
        {
            return Err(Error::Validation(format!(
                "projected CPI query dates must be strictly increasing ({date} after {prev})"
            )));
        }
        prev_date = Some(date);
        if date < c0.base_date() || date < c1.base_date() {
            return Err(Error::Validation(format!(
                "query date ({date}) predates inflation curve base dates ({}, {})",
                c0.base_date(),
                c1.base_date()
            )));
        }
        let cpi0 = positive_finite(c0.cpi_on_date(date)?, "cpi0")?;
        let cpi1 = positive_finite(c1.cpi_on_date(date)?, "cpi1")?;
        let diff = finite_f64_diff(cpi0, cpi1, 1.0)?;
        rows.push(InflationCpiDiffRow {
            source_t0: query.source_t0.clone(),
            source_t1: query.source_t1.clone(),
            date,
            diff,
        });
    }
    DiffMeasurement::from_validated_rows(rows)
}

/// Request parameters for comparing projected annual CPI growth rates between snapshots.
#[derive(Debug, Clone)]
pub struct InflationGrowthDiffQuery<'a> {
    /// Inflation curve identifier in `market_t0`.
    pub source_t0: CurveId,
    /// Inflation curve identifier in `market_t1`.
    pub source_t1: CurveId,
    /// Non-empty, duplicate-free ordered slice of `[anchor, maturity]` intervals.
    pub intervals: &'a [DateInterval],
    /// Reporting day-count convention used to annualize growth over `[anchor, maturity]`.
    pub day_count: DayCount,
    /// Reporting day-count context for `day_count`.
    pub day_count_context: DayCountContext<'a>,
    /// Output annual rate scale (`Decimal` or `BasisPoints`).
    pub scale: RateDiffScale,
}

impl<'a> InflationGrowthDiffQuery<'a> {
    /// Create a projected annual CPI growth comparison query.
    ///
    /// # Arguments
    /// - `curve_id` - Inflation curve identifier in `market_t0` and `market_t1`.
    /// - `intervals` - Non-empty unique ordered `[anchor, maturity]` intervals.
    /// - `day_count` - Reporting day-count convention for interval year fraction `α`.
    /// - `scale` - Output rate scale (`Decimal` or `BasisPoints`).
    #[must_use]
    pub fn new(
        curve_id: impl Into<CurveId>,
        intervals: &'a [DateInterval],
        day_count: DayCount,
        scale: RateDiffScale,
    ) -> Self {
        let id = curve_id.into();
        Self {
            source_t0: id.clone(),
            source_t1: id,
            intervals,
            day_count,
            day_count_context: DayCountContext::default(),
            scale,
        }
    }

    /// Override the earlier and later inflation curve identifiers explicitly.
    ///
    /// # Arguments
    /// - `source_t0` - Inflation curve identifier in `market_t0`.
    /// - `source_t1` - Inflation curve identifier in `market_t1`.
    #[must_use]
    pub fn with_source_ids(
        mut self,
        source_t0: impl Into<CurveId>,
        source_t1: impl Into<CurveId>,
    ) -> Self {
        self.source_t0 = source_t0.into();
        self.source_t1 = source_t1.into();
        self
    }

    /// Attach a [`DayCountContext`] for reporting day-count calculations.
    ///
    /// # Arguments
    /// - `context` - Reporting day-count context passed to [`DayCount::year_fraction`].
    #[must_use]
    pub fn with_day_count_context(mut self, context: DayCountContext<'a>) -> Self {
        self.day_count_context = context;
        self
    }
}

/// Measured change in projected annual CPI compound growth over `[anchor, maturity]`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InflationGrowthDiffRow {
    /// Source inflation curve identifier in `market_t0`.
    pub source_t0: CurveId,
    /// Source inflation curve identifier in `market_t1`.
    pub source_t1: CurveId,
    /// Common anchor date `V`.
    pub anchor: Date,
    /// Common maturity date `T`.
    pub maturity: Date,
    /// Reporting day count used to compute `year_fraction`.
    pub day_count: DayCount,
    /// Strictly positive reporting year fraction `α`.
    pub year_fraction: f64,
    /// Output unit scale (`Decimal` or `BasisPoints`).
    pub scale: RateDiffScale,
    /// Annualized compound growth change `π₁ - π₀` in `scale` units.
    pub diff: f64,
}

impl MeasuredDiffValue for InflationGrowthDiffRow {
    fn measured_diff(&self) -> f64 {
        self.diff
    }
}

/// Measure projected annual CPI compound growth differences `(I₁(T)/I₁(V))^(1/α) - (I₀(T)/I₀(V))^(1/α)`.
///
/// Requires compatible curve clocks (`c0.day_count() == c1.day_count()`) and computes the
/// continuous log cross-ratio via [`zero_rate_difference`] before converting cleanly through
/// `exp(r₀) * expm1(r₁ - r₀)` so small growth shifts retain full precision without subtraction
/// cancellation near `1.0`.
///
/// # Arguments
/// - `query` - Projected annual growth query naming curves, intervals, day count, and scale.
/// - `market_t0` - Earlier market snapshot.
/// - `market_t1` - Later market snapshot.
///
/// # Errors
/// Returns an error if either curve is missing, their `day_count()` clocks differ, `intervals`
/// is empty/duplicate/invalid, any date predates curve `base_date()`, reporting `α <= 0`, or any
/// CPI level or derived annual growth overflows/is non-finite.
pub fn measure_inflation_annual_growth_diff(
    query: &InflationGrowthDiffQuery<'_>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<DiffMeasurement<InflationGrowthDiffRow>> {
    if query.intervals.is_empty() {
        return Err(Error::Validation(
            "projected CPI annual growth diff requires at least one interval".to_string(),
        ));
    }
    let c0 = market_t0.get_inflation_curve(query.source_t0.as_str())?;
    let c1 = market_t1.get_inflation_curve(query.source_t1.as_str())?;
    require_matching_inflation_series(c0.get_series(), c1.get_series())?;
    if c0.day_count() != c1.day_count() {
        return Err(Error::Validation(format!(
            "inflation curves have incompatible day counts ({} vs {})",
            c0.day_count(),
            c1.day_count()
        )));
    }

    let mut seen = BTreeSet::new();
    let mut rows = Vec::with_capacity(query.intervals.len());
    for interval in query.intervals {
        if interval.maturity <= interval.anchor {
            return Err(Error::Validation(format!(
                "interval maturity ({}) must be strictly after anchor ({})",
                interval.maturity, interval.anchor
            )));
        }
        if !seen.insert((interval.anchor, interval.maturity)) {
            return Err(Error::Validation(format!(
                "duplicate comparison interval ({}, {})",
                interval.anchor, interval.maturity
            )));
        }
        if interval.anchor < c0.base_date() || interval.anchor < c1.base_date() {
            return Err(Error::Validation(format!(
                "interval anchor ({}) predates inflation curve base dates ({}, {})",
                interval.anchor,
                c0.base_date(),
                c1.base_date()
            )));
        }
        let alpha = query.day_count.year_fraction(
            interval.anchor,
            interval.maturity,
            query.day_count_context,
        )?;
        if !alpha.is_finite() || alpha <= 0.0 {
            return Err(Error::Validation(format!(
                "reporting day count {} produced invalid year fraction {alpha} for ({}, {})",
                query.day_count, interval.anchor, interval.maturity
            )));
        }

        let i0_a = positive_finite(c0.cpi_on_date(interval.anchor)?, "i0_anchor")?;
        let i0_m = positive_finite(c0.cpi_on_date(interval.maturity)?, "i0_maturity")?;
        let i1_a = positive_finite(c1.cpi_on_date(interval.anchor)?, "i1_anchor")?;
        let i1_m = positive_finite(c1.cpi_on_date(interval.maturity)?, "i1_maturity")?;

        // `zero_rate_difference` computes `(ell_0 - ell_1) / alpha`; for growth factor `I = exp(+r*t)`,
        // later minus earlier continuous growth rate `r_1 - r_0` is its negation.
        let delta_r = -zero_rate_difference(ZeroRateDiffParams {
            d0_anchor: i0_a,
            d0_maturity: i0_m,
            d1_anchor: i1_a,
            d1_maturity: i1_m,
            alpha,
            scale: RateDiffScale::Decimal,
        })?;
        let r0 =
            super::term_structures::common::checked_log_cross_ratio(i0_m, 1.0, i0_a, 1.0) / alpha;
        if !r0.is_finite() {
            return Err(Error::Validation(format!(
                "baseline annual CPI log-growth overflowed f64: r0={r0}"
            )));
        }
        let diff_dec = if exact_f64_eq(delta_r, 0.0) {
            0.0
        } else {
            r0.exp() * delta_r.exp_m1()
        };
        let diff = diff_dec * query.scale.scale_factor();
        if !diff.is_finite() || (!exact_f64_eq(delta_r, 0.0) && exact_f64_eq(diff, 0.0)) {
            return Err(Error::Validation(format!(
                "projected annual CPI growth diff overflowed or underflowed f64: diff_dec={diff_dec}"
            )));
        }
        rows.push(InflationGrowthDiffRow {
            source_t0: query.source_t0.clone(),
            source_t1: query.source_t1.clone(),
            anchor: interval.anchor,
            maturity: interval.maturity,
            day_count: query.day_count,
            year_fraction: alpha,
            scale: query.scale,
            diff: normalize_zero(diff),
        });
    }
    DiffMeasurement::from_validated_rows(rows)
}

/// Named monthly inflation reference period `(year, month)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct InflationPeriod {
    /// Calendar year of the inflation index print.
    pub year: i32,
    /// Calendar month of the inflation index print.
    pub month: Month,
}

impl InflationPeriod {
    /// Construct a monthly reference coordinate `(year, month)`.
    ///
    /// # Arguments
    /// - `year` - Calendar year of the monthly CPI print.
    /// - `month` - Calendar month of the monthly CPI print.
    #[must_use]
    pub const fn new(year: i32, month: Month) -> Self {
        Self { year, month }
    }
}

/// Pair of monthly inflation reference periods `(period_t0, period_t1)` to compare across snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct InflationPeriodPair {
    /// Reference `(year, month)` observed in `market_t0`.
    pub period_t0: InflationPeriod,
    /// Reference `(year, month)` observed in `market_t1`.
    pub period_t1: InflationPeriod,
}

impl InflationPeriodPair {
    /// Construct a same-period pair comparing the identical `(year, month)` print across snapshots.
    ///
    /// # Arguments
    /// - `year` - Calendar year of the monthly CPI print in both snapshots.
    /// - `month` - Calendar month of the monthly CPI print in both snapshots.
    #[must_use]
    pub const fn same(year: i32, month: Month) -> Self {
        let period = InflationPeriod::new(year, month);
        Self {
            period_t0: period,
            period_t1: period,
        }
    }

    /// Construct an explicit cross-period pair comparing `period_t0` in `market_t0` against `period_t1` in `market_t1`.
    ///
    /// # Arguments
    /// - `period_t0` - Reference `(year, month)` queried in `market_t0`.
    /// - `period_t1` - Reference `(year, month)` queried in `market_t1`.
    #[must_use]
    pub const fn cross(period_t0: InflationPeriod, period_t1: InflationPeriod) -> Self {
        Self {
            period_t0,
            period_t1,
        }
    }
}

/// Request parameters for comparing observed monthly CPI prints with explicit knowledge cutoffs.
#[derive(Debug, Clone)]
pub struct InflationObservationDiffQuery<'a> {
    /// Inflation index identifier in `market_t0`.
    pub source_t0: &'a str,
    /// Inflation index identifier in `market_t1`.
    pub source_t1: &'a str,
    /// Non-empty, duplicate-free ordered slice of reference `(period_t0, period_t1)` pairs.
    pub periods: &'a [InflationPeriodPair],
    /// Knowledge cutoff date `K₀` governing observation availability in `market_t0`.
    pub knowledge_date_t0: Date,
    /// Knowledge cutoff date `K₁` governing observation availability in `market_t1`.
    pub knowledge_date_t1: Date,
    /// Output scale (`NativePoints` or unannualized `Proportional`).
    pub scale: ObservationDiffScale,
}

impl<'a> InflationObservationDiffQuery<'a> {
    /// Create a query comparing monthly period pairs across two inflation index snapshots.
    ///
    /// # Arguments
    /// - `index_id` - Inflation index identifier in `market_t0` and `market_t1`.
    /// - `periods` - Non-empty ordered duplicate-free list of `(period_t0, period_t1)` pairs.
    /// - `knowledge_date_t0` - Information cutoff date for snapshot `t0`.
    /// - `knowledge_date_t1` - Information cutoff date for snapshot `t1`.
    /// - `scale` - Output difference scale (`NativePoints` or `Proportional`).
    #[must_use]
    pub const fn new(
        index_id: &'a str,
        periods: &'a [InflationPeriodPair],
        knowledge_date_t0: Date,
        knowledge_date_t1: Date,
        scale: ObservationDiffScale,
    ) -> Self {
        Self {
            source_t0: index_id,
            source_t1: index_id,
            periods,
            knowledge_date_t0,
            knowledge_date_t1,
            scale,
        }
    }

    /// Override the earlier and later index identifiers explicitly.
    ///
    /// # Arguments
    /// - `source_t0` - Inflation index identifier in `market_t0`.
    /// - `source_t1` - Inflation index identifier in `market_t1`.
    #[must_use]
    pub const fn with_source_ids(mut self, source_t0: &'a str, source_t1: &'a str) -> Self {
        self.source_t0 = source_t0;
        self.source_t1 = source_t1;
        self
    }
}

/// Measured difference for a single monthly inflation observation across two snapshots.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InflationObservationDiffRow {
    /// Source inflation index identifier in `market_t0`.
    pub source_t0: String,
    /// Source inflation index identifier in `market_t1`.
    pub source_t1: String,
    /// Reference `(year, month)` queried in `market_t0`.
    pub period_t0: InflationPeriod,
    /// Reference `(year, month)` queried in `market_t1`.
    pub period_t1: InflationPeriod,
    /// Knowledge cutoff date enforced on `market_t0`.
    pub knowledge_date_t0: Date,
    /// Knowledge cutoff date enforced on `market_t1`.
    pub knowledge_date_t1: Date,
    /// Qualified print in `market_t0`.
    pub value_t0: f64,
    /// Qualified print in `market_t1`.
    pub value_t1: f64,
    /// Scale of `diff`.
    pub scale: ObservationDiffScale,
    /// Classification of the comparison (`Unchanged`, `Revision`, or `CrossPeriod`).
    pub kind: ObservationChangeKind,
    /// Measured difference in `scale` units.
    pub diff: f64,
}

impl MeasuredDiffValue for InflationObservationDiffRow {
    fn measured_diff(&self) -> f64 {
        self.diff
    }
}

/// Measure observed monthly CPI changes for explicitly named periods and knowledge cutoffs.
///
/// Enforces matching index currency and seasonal-adjustment identity across both snapshots,
/// qualifies release eligibility using [`InflationIndex::qualified_monthly_observation`](crate::market_data::scalars::InflationIndex::qualified_monthly_observation),
/// labels same-month changes as [`ObservationChangeKind::Revision`] and distinct-month
/// comparisons as [`ObservationChangeKind::CrossPeriod`], and never falls back to
/// [`InflationCurve`](crate::market_data::term_structures::InflationCurve) projections.
///
/// # Arguments
/// - `query` - Observation diff query specifying index IDs, period pairs, cutoffs `K₀`/`K₁`, and scale.
/// - `market_t0` - Earlier market snapshot.
/// - `market_t1` - Later market snapshot.
///
/// # Errors
/// Returns an error if either index is missing, currencies differ,
/// `periods` is empty or contains duplicate `(period_t0, period_t1)` pairs, or any requested
/// monthly print is missing, non-positive, non-finite, or
/// unreleased as of its snapshot's `knowledge_date`.
pub fn measure_inflation_observation_diff(
    query: &InflationObservationDiffQuery<'_>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<DiffMeasurement<InflationObservationDiffRow>> {
    if query.periods.is_empty() {
        return Err(Error::Validation(
            "observed CPI diff requires at least one period".to_string(),
        ));
    }
    let idx0 = market_t0.get_inflation_index(query.source_t0)?;
    let idx1 = market_t1.get_inflation_index(query.source_t1)?;
    if idx0.currency() != idx1.currency() {
        return Err(Error::CurrencyMismatch {
            expected: idx0.currency(),
            actual: idx1.currency(),
        });
    }

    require_matching_inflation_series(idx0.get_series(), idx1.get_series())?;
    let mut seen = BTreeSet::new();
    let mut rows = Vec::with_capacity(query.periods.len());
    for &pair in query.periods {
        if !seen.insert((
            pair.period_t0.year,
            pair.period_t0.month as u8,
            pair.period_t1.year,
            pair.period_t1.month as u8,
        )) {
            return Err(Error::Validation(format!(
                "duplicate inflation period pair ({} {}, {} {})",
                pair.period_t0.year,
                pair.period_t0.month,
                pair.period_t1.year,
                pair.period_t1.month
            )));
        }
        let v0 = idx0.qualified_monthly_observation(
            pair.period_t0.year,
            pair.period_t0.month,
            query.knowledge_date_t0,
        )?;
        let v1 = idx1.qualified_monthly_observation(
            pair.period_t1.year,
            pair.period_t1.month,
            query.knowledge_date_t1,
        )?;
        let diff = match query.scale {
            ObservationDiffScale::NativePoints => finite_f64_diff(v0, v1, 1.0)?,
            ObservationDiffScale::Proportional(prop_scale) => {
                proportional_f64_change(v0, v1, prop_scale)?
            }
        };
        let kind = if pair.period_t0 != pair.period_t1 {
            ObservationChangeKind::CrossPeriod
        } else if exact_f64_eq(v1, v0) {
            ObservationChangeKind::Unchanged
        } else {
            ObservationChangeKind::Revision
        };
        rows.push(InflationObservationDiffRow {
            source_t0: query.source_t0.to_string(),
            source_t1: query.source_t1.to_string(),
            period_t0: pair.period_t0,
            period_t1: pair.period_t1,
            knowledge_date_t0: query.knowledge_date_t0,
            knowledge_date_t1: query.knowledge_date_t1,
            value_t0: v0,
            value_t1: v1,
            scale: query.scale,
            kind,
            diff,
        });
    }
    DiffMeasurement::from_validated_rows(rows)
}

/// Complete explicit FX quote query parameters for measuring ordered-pair proportional moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FxQuoteDiffParams {
    /// Base ("from") currency whose unit is priced in `quote` units.
    pub base: Currency,
    /// Quote ("to") currency.
    pub quote: Currency,
    /// Query date in the earlier snapshot `market_t0`.
    pub date_t0: Date,
    /// Required provider selection policy in `market_t0`.
    pub policy_t0: FxConversionPolicy,
    /// Query date in the later snapshot `market_t1`.
    pub date_t1: Date,
    /// Required provider selection policy in `market_t1`.
    pub policy_t1: FxConversionPolicy,
    /// Output proportional scale (`Fraction` or `Percent`).
    pub scale: ProportionalDiffScale,
}

/// Provenance-bearing measurement of an ordered-pair FX proportional quote change.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FxQuoteDiffMeasurement {
    /// Base ("from") currency whose unit is priced in `quote` units.
    pub base: Currency,
    /// Quote ("to") currency.
    pub quote: Currency,
    /// Query date in the earlier snapshot `market_t0`.
    pub date_t0: Date,
    /// Required provider selection policy in `market_t0`.
    pub policy_t0: FxConversionPolicy,
    /// Query date in the later snapshot `market_t1`.
    pub date_t1: Date,
    /// Required provider selection policy in `market_t1`.
    pub policy_t1: FxConversionPolicy,
    /// Resolved `base/quote` FX rate in `market_t0` (`1.0` when `base == quote`).
    pub rate_t0: f64,
    /// Whether `rate_t0` required pivot triangulation rather than a direct quote (`false` for identity).
    pub triangulated_t0: bool,
    /// Resolved `base/quote` FX rate in `market_t1` (`1.0` when `base == quote`).
    pub rate_t1: f64,
    /// Whether `rate_t1` required pivot triangulation rather than a direct quote (`false` for identity).
    pub triangulated_t1: bool,
    /// Output proportional scale (`Fraction` or `Percent`).
    pub scale: ProportionalDiffScale,
    /// Measured proportional difference `(rate_t1 - rate_t0) / rate_t0` in `scale` units.
    pub diff: f64,
}

/// Measure ordered-pair FX proportional change `(r₁ - r₀) / r₀` with explicit date, policy, and provenance.
///
/// Fast-paths `base == quote` to `diff = 0.0` (with unit resolved rates and `triangulated = false`)
/// without requiring FX matrices on either snapshot.
///
/// # Arguments
/// - `params` - Ordered currency pair, per-snapshot dates and conversion policies, and scale.
/// - `market_t0` - Earlier market snapshot.
/// - `market_t1` - Later market snapshot.
///
/// # Errors
/// Returns [`InputError::NotFound`] if either snapshot lacks an [`FxMatrix`](crate::money::fx::FxMatrix)
/// when `base != quote` or cannot quote `base/quote`, or [`Error::Validation`] if either quote is
/// non-positive/non-finite or the proportional ratio overflows or underflows `f64`.
pub fn measure_fx_quote_diff(
    params: &FxQuoteDiffParams,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<FxQuoteDiffMeasurement> {
    if params.base == params.quote {
        return Ok(FxQuoteDiffMeasurement {
            base: params.base,
            quote: params.quote,
            date_t0: params.date_t0,
            policy_t0: params.policy_t0,
            date_t1: params.date_t1,
            policy_t1: params.policy_t1,
            rate_t0: 1.0,
            triangulated_t0: false,
            rate_t1: 1.0,
            triangulated_t1: false,
            scale: params.scale,
            diff: 0.0,
        });
    }
    let fx0 = market_t0.fx().ok_or_else(|| InputError::NotFound {
        id: "fx_matrix (t0)".to_string(),
    })?;
    let fx1 = market_t1.fx().ok_or_else(|| InputError::NotFound {
        id: "fx_matrix (t1)".to_string(),
    })?;
    let res0 = fx0.rate(FxQuery::new(
        params.base,
        params.quote,
        params.date_t0,
        params.policy_t0,
    ))?;
    let res1 = fx1.rate(FxQuery::new(
        params.base,
        params.quote,
        params.date_t1,
        params.policy_t1,
    ))?;
    positive_finite(res0.rate, "fx_rate_t0")?;
    positive_finite(res1.rate, "fx_rate_t1")?;
    let diff = proportional_f64_change(res0.rate, res1.rate, params.scale)?;
    Ok(FxQuoteDiffMeasurement {
        base: params.base,
        quote: params.quote,
        date_t0: params.date_t0,
        policy_t0: params.policy_t0,
        date_t1: params.date_t1,
        policy_t1: params.policy_t1,
        rate_t0: res0.rate,
        triangulated_t0: res0.triangulated,
        rate_t1: res1.rate,
        triangulated_t1: res1.triangulated,
        scale: params.scale,
        diff,
    })
}

/// Measure exact native currency difference `m₁ - m₀` between two `MarketScalar::Price` entries.
///
/// Uses checked [`Money::checked_sub`] so exact `Decimal` precision and currency identity are
/// preserved without converting through `f64` or rounded accessors.
///
/// # Arguments
/// - `id_t0` - Price scalar identifier in `market_t0`.
/// - `id_t1` - Price scalar identifier in `market_t1`.
/// - `market_t0` - Earlier market snapshot.
/// - `market_t1` - Later market snapshot.
///
/// # Errors
/// Returns [`InputError::NotFound`] if either scalar is missing, [`Error::CurrencyMismatch`]
/// if their currencies differ, or [`Error::Validation`] if either entry is `Unitless` or the
/// `Decimal` difference overflows.
pub fn measure_price_scalar_diff(
    id_t0: &str,
    id_t1: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<Money> {
    let s0 = market_t0.get_price(id_t0)?;
    let s1 = market_t1.get_price(id_t1)?;
    match (s0.get_price(), s1.get_price()) {
        (Some(m0), Some(m1)) => m1.checked_sub(m0),
        _ => Err(Error::Validation(format!(
            "measure_price_scalar_diff requires MarketScalar::Price entries for '{id_t0}' and '{id_t1}'"
        ))),
    }
}

/// Measure exact native difference `x₁ - x₀` between two `MarketScalar::Unitless` entries.
///
/// # Arguments
/// - `id_t0` - Unitless scalar identifier in `market_t0`.
/// - `id_t1` - Unitless scalar identifier in `market_t1`.
/// - `market_t0` - Earlier market snapshot.
/// - `market_t1` - Later market snapshot.
///
/// # Errors
/// Returns [`InputError::NotFound`] if either scalar is missing, or [`Error::Validation`] if
/// either entry is `Price`, non-finite, or `x₁ - x₀` overflows `f64`.
pub fn measure_unitless_scalar_diff(
    id_t0: &str,
    id_t1: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    let s0 = market_t0.get_price(id_t0)?;
    let s1 = market_t1.get_price(id_t1)?;
    match (s0.get_unitless(), s1.get_unitless()) {
        (Some(x0), Some(x1)) => finite_f64_diff(x0, x1, 1.0),
        _ => Err(Error::Validation(format!(
            "measure_unitless_scalar_diff requires MarketScalar::Unitless entries for '{id_t0}' and '{id_t1}'"
        ))),
    }
}

/// Measure exact algebraic proportional change `(x₁ - x₀) / x₀` for two matched `MarketScalar` entries.
///
/// Dispatches strictly within matching scalar kinds (`Price` via [`Money::proportional_change`],
/// `Unitless` via `ldexp`-rescaled `f64` differences). Rejects zero baselines even when `x₁ == x₀`,
/// allows negative and arbitrarily small non-zero finite baselines, and errors on kind or currency
/// mismatches.
///
/// # Arguments
/// - `id_t0` - Scalar identifier in `market_t0`.
/// - `id_t1` - Scalar identifier in `market_t1`.
/// - `market_t0` - Earlier market snapshot.
/// - `market_t1` - Later market snapshot.
/// - `scale` - Output proportional scale (`Fraction` or `Percent`).
///
/// # Errors
/// Returns [`InputError::NotFound`] if either scalar is missing, [`Error::CurrencyMismatch`]
/// for differing `Price` currencies, or [`Error::Validation`] for kind mismatch (`Price` vs
/// `Unitless`), zero baseline, non-finite inputs, or `f64` overflow/underflow of non-zero moves.
pub fn measure_scalar_proportional_diff(
    id_t0: &str,
    id_t1: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    scale: ProportionalDiffScale,
) -> Result<f64> {
    let s0 = market_t0.get_price(id_t0)?;
    let s1 = market_t1.get_price(id_t1)?;
    if let (Some(m0), Some(m1)) = (s0.get_price(), s1.get_price()) {
        let frac = m1.proportional_change(m0)?;
        let out = frac * scale.scale_factor();
        if !out.is_finite() {
            return Err(Error::Validation(format!(
                "proportional price change scaled overflowed f64: {out}"
            )));
        }
        return Ok(normalize_zero(out));
    }
    if let (Some(x0), Some(x1)) = (s0.get_unitless(), s1.get_unitless()) {
        return proportional_f64_change(x0, x1, scale);
    }
    Err(Error::Validation(format!(
        "scalar kind mismatch between '{id_t0}' and '{id_t1}'"
    )))
}

#[inline]
fn exact_f64_eq(a: f64, b: f64) -> bool {
    a <= b && b <= a
}

#[inline]
fn normalize_zero(x: f64) -> f64 {
    if exact_f64_eq(x, 0.0) { 0.0 } else { x }
}

#[inline]
fn positive_finite(val: f64, label: &str) -> Result<f64> {
    if !val.is_finite() || val <= 0.0 {
        return Err(Error::Validation(format!(
            "'{label}' must be finite and strictly positive, got {val}"
        )));
    }
    Ok(val)
}

fn finite_f64_diff(x0: f64, x1: f64, scale: f64) -> Result<f64> {
    if !x0.is_finite() || !x1.is_finite() {
        return Err(Error::Validation(format!(
            "difference inputs must be finite, got ({x0}, {x1})"
        )));
    }
    if exact_f64_eq(x1, x0) {
        return Ok(0.0);
    }
    let diff = (x1 - x0) * scale;
    if !diff.is_finite() {
        return Err(Error::Validation(format!(
            "difference ({x1} - {x0}) * {scale} overflowed f64"
        )));
    }
    Ok(normalize_zero(diff))
}

fn proportional_f64_change(x0: f64, x1: f64, scale: ProportionalDiffScale) -> Result<f64> {
    if !x0.is_finite() || !x1.is_finite() {
        return Err(Error::Validation(format!(
            "proportional change inputs must be finite, got ({x0}, {x1})"
        )));
    }
    if exact_f64_eq(x0, 0.0) {
        return Err(Error::Validation(
            "proportional change baseline cannot be zero".to_string(),
        ));
    }
    if exact_f64_eq(x1, 0.0) {
        return Ok(-scale.scale_factor());
    }
    if exact_f64_eq(x1, x0) {
        return Ok(0.0);
    }

    let e0 = stats::frexp_exp(x0);
    let e1 = stats::frexp_exp(x1);
    let de = e1 - e0;
    let factor = scale.scale_factor();
    let m0 = stats::ldexp(x0, -e0);

    let out = if de <= 10 {
        let s1 = stats::ldexp(x1, -e0);
        ((s1 - m0) / m0) * factor
    } else {
        let m1 = stats::ldexp(x1, -e1);
        (stats::ldexp(m1 / m0, de) - 1.0) * factor
    };

    if !out.is_finite() || exact_f64_eq(out, 0.0) {
        return Err(Error::Validation(format!(
            "proportional change ({x1} - {x0}) / {x0} overflowed or underflowed f64"
        )));
    }
    Ok(normalize_zero(out))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::dates::DayCount;
    use crate::market_data::scalars::{InflationIndex, MarketScalar};
    use crate::market_data::term_structures::{
        DiscountCurve, ForwardCurve, HazardCurve, InflationCurve,
    };
    use crate::money::fx::{FxMatrix, FxProvider};
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn proportional_move_to_zero_is_a_complete_loss() {
        for baseline in [1.0, -0.01, f64::from_bits(1)] {
            assert_eq!(
                proportional_f64_change(baseline, -0.0, ProportionalDiffScale::Percent).unwrap(),
                -100.0
            );
        }
    }

    #[test]
    fn diff_enum_serde_uses_canonical_snake_case() {
        assert_eq!(
            serde_json::to_string(&RateDiffScale::BasisPoints).unwrap(),
            r#""basis_points""#
        );
        assert_eq!(
            serde_json::to_string(&HazardDiffScale::IntensityBp).unwrap(),
            r#""intensity_bp""#
        );
        assert_eq!(
            serde_json::to_string(&ProportionalDiffScale::Percent).unwrap(),
            r#""percent""#
        );
        assert_eq!(
            serde_json::to_string(&ObservationDiffScale::Proportional(
                ProportionalDiffScale::Percent
            ))
            .unwrap(),
            r#"{"proportional":"percent"}"#
        );
        assert_eq!(
            serde_json::to_string(&ObservationChangeKind::CrossPeriod).unwrap(),
            r#""cross_period""#
        );
    }

    #[derive(Debug)]
    struct StaticFx {
        rate: f64,
    }

    impl FxProvider for StaticFx {
        fn rate(
            &self,
            from: Currency,
            to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> Result<f64> {
            match (from, to) {
                (Currency::EUR, Currency::USD) => Ok(self.rate),
                (Currency::USD, Currency::EUR) => Ok(1.0 / self.rate),
                _ => Err(InputError::NotFound {
                    id: format!("{from}/{to}"),
                }
                .into()),
            }
        }
    }

    #[test]
    fn zero_rate_difference_validates_before_equal_shortcut() {
        assert!(
            zero_rate_difference(ZeroRateDiffParams {
                d0_anchor: 0.99,
                d0_maturity: 0.95,
                d1_anchor: 0.99,
                d1_maturity: 0.95,
                alpha: 0.0,
                scale: RateDiffScale::Decimal,
            })
            .is_err()
        );
        assert!(
            zero_rate_difference(ZeroRateDiffParams {
                d0_anchor: 0.0,
                d0_maturity: 0.95,
                d1_anchor: 0.0,
                d1_maturity: 0.95,
                alpha: 1.0,
                scale: RateDiffScale::Decimal,
            })
            .is_err()
        );
        assert!(
            zero_rate_difference(ZeroRateDiffParams {
                d0_anchor: 1.0,
                d0_maturity: f64::NAN,
                d1_anchor: 1.0,
                d1_maturity: 1.0,
                alpha: 1.0,
                scale: RateDiffScale::Decimal,
            })
            .is_err()
        );
    }

    #[test]
    fn zero_rate_difference_handles_extreme_scale_and_tiny_shift() {
        let scale_lo = 2.0_f64.powi(-800);
        let scale_hi = 2.0_f64.powi(800);
        let eq = zero_rate_difference(ZeroRateDiffParams {
            d0_anchor: scale_lo,
            d0_maturity: 0.95 * scale_lo,
            d1_anchor: scale_hi,
            d1_maturity: 0.95 * scale_hi,
            alpha: 1.0,
            scale: RateDiffScale::BasisPoints,
        })
        .expect("identical factor ratio across 2^1600 dynamic range");
        assert_eq!(eq, 0.0);

        let tiny_shift = 1e-12_f64;
        let alpha = 2.0_f64;
        let d0_m = (-0.04_f64 * alpha).exp();
        let d1_m = (-(0.04_f64 + tiny_shift) * alpha).exp();
        let diff = zero_rate_difference(ZeroRateDiffParams {
            d0_anchor: 1.0,
            d0_maturity: d0_m,
            d1_anchor: 1.0,
            d1_maturity: d1_m,
            alpha,
            scale: RateDiffScale::Decimal,
        })
        .expect("tiny shift");
        assert!((diff - tiny_shift).abs() < 1e-15);

        assert!(
            zero_rate_difference(ZeroRateDiffParams {
                d0_anchor: 1.0,
                d0_maturity: 1.0,
                d1_anchor: 1.0,
                d1_maturity: 1.0 - 0.5 * f64::EPSILON,
                alpha: 1e308,
                scale: RateDiffScale::Decimal,
            })
            .is_err()
        );
    }

    #[test]
    fn discount_diff_uses_retained_clock_and_rejects_cross_family_fallback() {
        let base0 = date!(2025 - 01 - 01);
        let cond_date = date!(2025 - 07 - 01);
        let v = date!(2025 - 07 - 01);
        let t = date!(2026 - 07 - 01);

        let c0 = DiscountCurve::flat("USD-OIS", base0, 0.04, DayCount::Act365F).expect("c0");
        let c1_raw = DiscountCurve::flat("USD-OIS", base0, 0.0425, DayCount::Act365F).expect("c1");
        let c1_cond = c1_raw.condition(cond_date).expect("conditioned c1");

        let m0 = MarketContext::new()
            .try_insert(c0)
            .expect("insert market curve");
        let m1 = MarketContext::new()
            .try_insert(c1_cond)
            .expect("insert market curve");

        let intervals = [DateInterval::new(v, t).expect("valid interval")];
        let query = FactorRateDiffQuery::new(
            "USD-OIS",
            &intervals,
            DayCount::Act365F,
            RateDiffScale::BasisPoints,
        );
        let measured = measure_discount_factor_rate_diff(&query, &m0, &m1).expect("discount diff");
        assert_eq!(measured.len(), 1);
        assert!((measured.rows()[0].diff - 25.0).abs() < 1e-10);
        assert!((measured.mean().expect("mean") - 25.0).abs() < 1e-10);

        // No fallback from ForwardCurve to DiscountCurve or vice versa.
        let fwd = ForwardCurve::builder("FWD-ONLY")
            .base_date(base0)
            .day_count(DayCount::Act365F)
            .knots([(1.0, 0.96), (5.0, 0.80)])
            .interp(crate::math::interp::InterpStyle::LogLinear)
            .extrapolation(crate::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("fwd");
        let mf = MarketContext::new()
            .try_insert(fwd)
            .expect("insert market curve");
        let fwd_query = FactorRateDiffQuery::new(
            "FWD-ONLY",
            &intervals,
            DayCount::Act365F,
            RateDiffScale::BasisPoints,
        );
        assert!(measure_discount_factor_rate_diff(&fwd_query, &mf, &mf).is_err());
        assert_eq!(
            measure_forward_factor_rate_diff(&fwd_query, &mf, &mf)
                .expect("identical forward")
                .rows()[0]
                .diff,
            0.0
        );
    }

    #[test]
    fn factor_diff_rejects_empty_duplicate_and_out_of_domain_intervals() {
        let base = date!(2025 - 01 - 01);
        let m0 = MarketContext::new()
            .try_insert(DiscountCurve::flat("C0", base, 0.04, DayCount::Act365F).unwrap())
            .expect("insert market curve");
        let empty: [DateInterval; 0] = [];
        let q_empty =
            FactorRateDiffQuery::new("C0", &empty, DayCount::Act365F, RateDiffScale::Decimal);
        assert!(measure_discount_factor_rate_diff(&q_empty, &m0, &m0).is_err());

        let i1 = DateInterval::new(date!(2025 - 07 - 01), date!(2026 - 07 - 01)).unwrap();
        let dup = [i1, i1];
        let q_dup = FactorRateDiffQuery::new("C0", &dup, DayCount::Act365F, RateDiffScale::Decimal);
        assert!(measure_discount_factor_rate_diff(&q_dup, &m0, &m0).is_err());

        let early = [DateInterval::new(date!(2024 - 12 - 01), date!(2026 - 01 - 01)).unwrap()];
        let q_early =
            FactorRateDiffQuery::new("C0", &early, DayCount::Act365F, RateDiffScale::Decimal);
        assert!(measure_discount_factor_rate_diff(&q_early, &m0, &m0).is_err());
    }

    #[test]
    fn hazard_intensity_diff_preserves_source_clock_and_no_recovery_coupling() {
        let base = date!(2025 - 01 - 01);
        let cond_date = date!(2026 - 01 - 01);
        let h0 = HazardCurve::builder("ACME")
            .base_date(base)
            .recovery_rate(0.40)
            .knots([(1.0, 0.01), (5.0, 0.02)])
            .day_count(crate::dates::DayCount::Act365F)
            .build()
            .unwrap();
        // Change recovery rate and bump intensity by +0.0020 (20 intensity bp):
        // native hazard diff must remain 0.0020 / 20.0 intensity bp regardless of recovery.
        let h1 = HazardCurve::builder("ACME")
            .base_date(base)
            .recovery_rate(0.20)
            .knots([(1.0, 0.012), (5.0, 0.022)])
            .day_count(crate::dates::DayCount::Act365F)
            .build()
            .unwrap()
            .condition(cond_date)
            .unwrap();

        let m0 = MarketContext::new()
            .try_insert(h0)
            .expect("insert market curve");
        let m1 = MarketContext::new()
            .try_insert(h1)
            .expect("insert market curve");
        let q = HazardDiffQuery::new("ACME", &[1.0, 5.0], HazardDiffScale::IntensityBp);
        let res = measure_hazard_intensity_diff(&q, &m0, &m1).expect("hazard diff");
        assert!((res.rows()[0].diff - 20.0).abs() < 1e-12);
        assert!((res.rows()[1].diff - 20.0).abs() < 1e-12);

        // Incompatible day count or unsorted times must error.
        let bad_order = HazardDiffQuery::new("ACME", &[5.0, 1.0], HazardDiffScale::Decimal);
        assert!(measure_hazard_intensity_diff(&bad_order, &m0, &m1).is_err());
    }

    fn cpi_series() -> crate::market_data::scalars::InflationSeries {
        crate::market_data::scalars::InflationSeries {
            series_id: "BLS:CUUR0000SA0".into(),
            reference_base: "1982-84=100".into(),
            seasonally_adjusted: false,
        }
    }

    #[test]
    fn monthly_revision_requires_common_series_base_and_seasonal_status() {
        use crate::market_data::scalars::InflationObservation;
        let cutoff = date!(2025 - 02 - 12);
        let market = |value, series| {
            let obs = InflationObservation {
                value,
                reference_month: date!(2025 - 01 - 01),
                available_on: cutoff,
            };
            let index = InflationIndex::new("USD-CPI", vec![obs], Currency::USD, cutoff)
                .unwrap()
                .with_series(series)
                .unwrap();
            MarketContext::new().insert_inflation_index(index)
        };
        let periods = [InflationPeriodPair::same(2025, Month::January)];
        let query = InflationObservationDiffQuery::new(
            "USD-CPI",
            &periods,
            cutoff,
            cutoff,
            ObservationDiffScale::NativePoints,
        );
        let opening = market(105.0, cpi_series());
        let compare = |value, series| {
            measure_inflation_observation_diff(&query, &opening, &market(value, series))
        };
        let mut rebased = cpi_series();
        rebased.reference_base = "2015=100".into();
        assert!(compare(210.0, rebased).is_err());
        let mut adjusted = cpi_series();
        adjusted.seasonally_adjusted = true;
        assert!(compare(105.0, adjusted).is_err());
        let mut other = cpi_series();
        other.series_id = "BLS:OTHER".into();
        assert!(compare(105.0, other).is_err());
        assert_eq!(
            compare(210.0 / 2.0, cpi_series()).unwrap().rows()[0].diff,
            0.0
        );
        assert_eq!(compare(105.5, cpi_series()).unwrap().rows()[0].diff, 0.5);
    }

    #[test]
    fn inflation_cpi_and_annual_growth_diffs() {
        use crate::math::interp::{ExtrapolationPolicy, InterpStyle};

        let base = date!(2025 - 01 - 01);
        let c0 = InflationCurve::builder("US-CPI")
            .series(Some(cpi_series()))
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 100.0), (1.0, 102.0), (5.0, 110.408_080_32)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .build()
            .unwrap();
        let c1 = InflationCurve::builder("US-CPI")
            .series(Some(cpi_series()))
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 100.0), (1.0, 103.0), (5.0, 115.927_407_43)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .build()
            .unwrap();
        let m0 = MarketContext::new()
            .try_insert(c0)
            .expect("insert market curve");
        let m1 = MarketContext::new()
            .try_insert(c1)
            .expect("insert market curve");

        let d1 = date!(2026 - 01 - 01);
        let dates = [d1];
        let q_lvl = InflationCpiDiffQuery::new("US-CPI", &dates);
        let lvl = measure_inflation_cpi_level_diff(&q_lvl, &m0, &m1).unwrap();
        assert!((lvl.rows()[0].diff - 1.0).abs() < 1e-2);

        let intervals = [DateInterval::new(base, d1).unwrap()];
        let q_gr = InflationGrowthDiffQuery::new(
            "US-CPI",
            &intervals,
            DayCount::Act365F,
            RateDiffScale::BasisPoints,
        );
        let gr = measure_inflation_annual_growth_diff(&q_gr, &m0, &m1).unwrap();
        assert!((gr.rows()[0].diff - 100.0).abs() < 1e-9);
    }

    #[test]
    fn inflation_observation_diff_enforces_knowledge_date_and_revisions() {
        use crate::market_data::scalars::InflationObservation;

        let pub_jan = date!(2025 - 02 - 12);
        let pub_feb = date!(2025 - 03 - 12);
        let obs_jan = InflationObservation {
            reference_month: date!(2025 - 01 - 01),
            value: 310.0,
            available_on: pub_jan,
        };
        let idx0 = InflationIndex::new("US-CPI-U", vec![obs_jan], Currency::USD, pub_jan).unwrap();

        let idx1 = InflationIndex::new(
            "US-CPI-U",
            vec![
                InflationObservation {
                    reference_month: date!(2025 - 01 - 01),
                    value: 310.5,
                    available_on: pub_jan,
                },
                InflationObservation {
                    reference_month: date!(2025 - 02 - 01),
                    value: 311.5,
                    available_on: pub_feb,
                },
            ],
            Currency::USD,
            pub_feb,
        )
        .unwrap();

        let m0 =
            MarketContext::new().insert_inflation_index(idx0.with_series(cpi_series()).unwrap());
        let m1 =
            MarketContext::new().insert_inflation_index(idx1.with_series(cpi_series()).unwrap());
        let periods = [InflationPeriodPair::same(2025, Month::January)];

        // Knowledge cutoff before publication date fails closed.
        let premature = InflationObservationDiffQuery::new(
            "US-CPI-U",
            &periods,
            date!(2025 - 02 - 10),
            date!(2025 - 02 - 15),
            ObservationDiffScale::NativePoints,
        );
        assert!(measure_inflation_observation_diff(&premature, &m0, &m1).is_err());

        // Knowledge cutoff on/after publication date succeeds and labels revision.
        let qualified = InflationObservationDiffQuery::new(
            "US-CPI-U",
            &periods,
            date!(2025 - 02 - 12),
            date!(2025 - 02 - 15),
            ObservationDiffScale::NativePoints,
        );
        let res = measure_inflation_observation_diff(&qualified, &m0, &m1).unwrap();
        assert_eq!(res.rows()[0].kind, ObservationChangeKind::Revision);
        assert_eq!(
            res.rows()[0].period_t0,
            InflationPeriod::new(2025, Month::January)
        );
        assert_eq!(
            res.rows()[0].period_t1,
            InflationPeriod::new(2025, Month::January)
        );
        assert!((res.rows()[0].diff - 0.5).abs() < 1e-12);

        // Explicit cross-period comparison across months labels CrossPeriod.
        let cross_periods = [InflationPeriodPair::cross(
            InflationPeriod::new(2025, Month::January),
            InflationPeriod::new(2025, Month::February),
        )];
        let cross_query = InflationObservationDiffQuery::new(
            "US-CPI-U",
            &cross_periods,
            date!(2025 - 02 - 12),
            date!(2025 - 03 - 12),
            ObservationDiffScale::NativePoints,
        );
        let cross_res = measure_inflation_observation_diff(&cross_query, &m0, &m1).unwrap();
        assert_eq!(cross_res.rows()[0].kind, ObservationChangeKind::CrossPeriod);
        assert!((cross_res.rows()[0].diff - 1.5).abs() < 1e-12);
    }

    #[test]
    fn fx_quote_diff_fast_paths_identity_without_matrices_and_uses_explicit_policy() {
        let empty = MarketContext::new();
        let d0 = date!(2025 - 01 - 01);
        let d1 = date!(2025 - 02 - 01);
        let same_currency_query = FxQuoteDiffParams {
            base: Currency::USD,
            quote: Currency::USD,
            date_t0: d0,
            policy_t0: FxConversionPolicy::CashflowDate,
            date_t1: d1,
            policy_t1: FxConversionPolicy::PeriodEnd,
            scale: ProportionalDiffScale::Percent,
        };
        let id_meas = measure_fx_quote_diff(&same_currency_query, &empty, &empty).unwrap();
        assert_eq!(id_meas.diff, 0.0);
        assert_eq!(id_meas.rate_t0, 1.0);
        assert_eq!(id_meas.rate_t1, 1.0);
        assert!(!id_meas.triangulated_t0);
        assert!(!id_meas.triangulated_t1);

        let eur_usd = FxQuoteDiffParams {
            base: Currency::EUR,
            quote: Currency::USD,
            ..same_currency_query
        };
        assert!(measure_fx_quote_diff(&eur_usd, &empty, &empty).is_err());

        let fx0 = FxMatrix::new(Arc::new(StaticFx { rate: 1.0 }), None);
        let fx1 = FxMatrix::new(Arc::new(StaticFx { rate: 1.1 }), None);
        let m0 = MarketContext::new().insert_fx(fx0);
        let m1 = MarketContext::new().insert_fx(fx1);
        let meas = measure_fx_quote_diff(&eur_usd, &m0, &m1).unwrap();
        assert!((meas.diff - 10.0).abs() < 1e-12);
        assert_eq!(meas.rate_t0, 1.0);
        assert_eq!(meas.rate_t1, 1.1);
        assert!(!meas.triangulated_t0);
        assert!(!meas.triangulated_t1);
    }

    #[test]
    fn scalar_diffs_preserve_money_exactness_and_algebraic_proportional_domain() {
        let m0 = MarketContext::new()
            .insert_price(
                "BIG_USD",
                MarketScalar::price(Money::from_decimal(
                    rust_decimal::Decimal::from_i128_with_scale(100_000_000_000_000_000, 0),
                    Currency::USD,
                )),
            )
            .insert_price("NEG", MarketScalar::unitless(-10.0).unwrap())
            .insert_price("TINY", MarketScalar::unitless(1e-200).unwrap())
            .insert_price("HUGE", MarketScalar::unitless(1e308).unwrap())
            .insert_price("ZERO_U", MarketScalar::unitless(0.0).unwrap())
            .insert_price("ZERO_M", MarketScalar::price(Money::zero(Currency::USD)));

        let m1 = MarketContext::new()
            .insert_price(
                "BIG_USD",
                MarketScalar::price(Money::from_decimal(
                    rust_decimal::Decimal::from_i128_with_scale(10_000_000_000_000_000_001, 2),
                    Currency::USD,
                )),
            )
            .insert_price("NEG", MarketScalar::unitless(-15.0).unwrap())
            .insert_price("TINY", MarketScalar::unitless(1.5e-200).unwrap())
            .insert_price("HUGE", MarketScalar::unitless(-1e308).unwrap())
            .insert_price("ZERO_U", MarketScalar::unitless(0.0).unwrap())
            .insert_price("ZERO_M", MarketScalar::price(Money::zero(Currency::USD)));

        // Exact Decimal subtraction preserves $0.01 at 1e17 USD without f64 cancellation.
        let price_delta = measure_price_scalar_diff("BIG_USD", "BIG_USD", &m0, &m1).unwrap();
        assert_eq!(
            price_delta.amount_decimal(),
            rust_decimal::Decimal::from_i128_with_scale(1, 2)
        );

        // Algebraic proportional delta admits negative and sub-1e-15 baselines.
        assert!(
            (measure_scalar_proportional_diff(
                "NEG",
                "NEG",
                &m0,
                &m1,
                ProportionalDiffScale::Fraction
            )
            .unwrap()
                - 0.5)
                .abs()
                < 1e-12
        );
        assert!(
            (measure_scalar_proportional_diff(
                "TINY",
                "TINY",
                &m0,
                &m1,
                ProportionalDiffScale::Fraction
            )
            .unwrap()
                - 0.5)
                .abs()
                < 1e-12
        );

        // Overflow-rescaled opposite-sign f64 ratio avoids spurious inf.
        assert_eq!(
            measure_scalar_proportional_diff(
                "HUGE",
                "HUGE",
                &m0,
                &m1,
                ProportionalDiffScale::Fraction
            )
            .unwrap(),
            -2.0
        );

        // Zero baseline fails even for 0 -> 0; Price vs Unitless fails closed.
        assert!(
            measure_scalar_proportional_diff(
                "ZERO_U",
                "ZERO_U",
                &m0,
                &m1,
                ProportionalDiffScale::Fraction
            )
            .is_err()
        );
        assert!(
            measure_scalar_proportional_diff(
                "ZERO_M",
                "ZERO_M",
                &m0,
                &m1,
                ProportionalDiffScale::Fraction
            )
            .is_err()
        );
        assert!(measure_unitless_scalar_diff("BIG_USD", "NEG", &m0, &m1).is_err());
    }
}
