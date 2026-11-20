//! Market change measurement primitives for curves, FX, and scalars.
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
//!     HazardDiffQuery, HazardDiffScale, measure_hazard_intensity_diff,
//! };
//! use finstack_quant_core::market_data::term_structures::HazardCurve;
//! use time::macros::date;
//!
//! # fn main() -> finstack_quant_core::Result<()> {
//! let base = date!(2025 - 01 - 01);
//! let h0 = HazardCurve::builder("ACME")
//!     .base_date(base)
//!     .recovery_rate(0.40)
//!     .knots([(1.0, 0.01), (5.0, 0.02)])
//!     .day_count(DayCount::Act365F)
//!     .build()?;
//! let h1 = HazardCurve::builder("ACME")
//!     .base_date(base)
//!     .recovery_rate(0.40)
//!     .knots([(1.0, 0.012), (5.0, 0.022)])
//!     .day_count(DayCount::Act365F)
//!     .build()?;
//! let m0 = MarketContext::new().try_insert(h0)?;
//! let m1 = MarketContext::new().try_insert(h1)?;
//!
//! let query = HazardDiffQuery::new("ACME", &[1.0, 5.0], HazardDiffScale::IntensityBp);
//! let result = measure_hazard_intensity_diff(&query, &m0, &m1)?;
//! assert!((result.rows()[0].diff - 20.0).abs() < 1e-12);
//! # Ok(())
//! # }
//! ```

use std::fmt;

use crate::currency::Currency;
use crate::dates::{Date, DayCount};
use crate::error::InputError;
use crate::market_data::context::MarketContext;
use crate::math::interp::exact_f64_eq;
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

/// Ordered, non-empty measurement vector returned by term-structure diffs.
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
    use crate::market_data::scalars::MarketScalar;
    use crate::market_data::term_structures::HazardCurve;
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
