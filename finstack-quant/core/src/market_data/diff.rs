//! Market data comparison and shift measurement.
//!
//! Provides utilities for measuring market movements between two `MarketContext`
//! instances. Used primarily for metrics-based P&L attribution, risk reporting,
//! and scenario analysis.
//!
//! # Purpose
//!
//! This module is symmetric to `bumps.rs`:
//! - **bumps.rs**: Apply shifts to create scenarios
//! - **diff.rs**: Measure shifts between markets
//!
//! # Use Cases
//!
//! - **P&L Attribution**: Explain P&L changes via DV01 × Δrates, CS01 × Δspreads
//! - **Risk Reporting**: Measure daily market moves for VaR and stress testing
//! - **Calibration**: Compare calibrated curves vs market inputs
//! - **Market Analysis**: Track curve movements over time
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::market_data::diff::{measure_discount_curve_shift, TenorSamplingMethod};
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::types::CurveId;
//!
//! # fn example(market_yesterday: MarketContext, market_today: MarketContext) -> finstack_quant_core::Result<()> {
//! // Measure rate shift between two markets
//! let shift_bp = measure_discount_curve_shift(
//!     &CurveId::from("USD-OIS"),
//!     &market_yesterday,
//!     &market_today,
//!     TenorSamplingMethod::Standard,
//! )?;
//!
//! println!("USD-OIS moved {} basis points", shift_bp);
//! # Ok(())
//! # }
//! ```

use super::context::MarketContext;
use crate::currency::Currency;
use crate::dates::Date;
use crate::math::units::BASIS_POINTS_PER_UNIT;
use crate::Result;

use serde::{Deserialize, Serialize};

/// Standard market tenors for curve sampling (in years).
///
/// Based on liquid swap market points: 3M, 6M, 1Y, 2Y, 3Y, 5Y, 7Y, 10Y, 30Y.
/// These are the conventional points where curves are most actively traded
/// and quoted.
pub const STANDARD_TENORS: &[f64] = &[0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 30.0];

/// Method for selecting tenor points when measuring curve shifts.
///
/// Different sampling strategies trade off accuracy, performance, and
/// robustness to curve structure.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TenorSamplingMethod {
    /// Standard swap market tenors (3M, 6M, 1Y, 2Y, 3Y, 5Y, 7Y, 10Y, 30Y).
    ///
    /// Most robust for typical interest rate curves. Matches market liquidity
    /// points and works well for parallel shift detection.
    #[default]
    Standard,

    /// Use curve's own knot points dynamically.
    ///
    /// Adapts to curve structure but may miss shifts between pillars.
    /// Good for curves with non-standard pillar structure.
    Dynamic,

    /// Custom tenor list specified by caller.
    ///
    /// Allows fine-grained control for specific use cases (e.g., matching
    /// an instrument's cashflow schedule).
    Custom(Vec<f64>),
}

impl TenorSamplingMethod {
    /// Get the tenor points to sample based on the method.
    ///
    /// For `Dynamic`, uses the knot points from the provided curve.
    /// For `Standard`, uses `STANDARD_TENORS`.
    /// For `Custom`, uses the provided tenor list.
    fn tenors<'a>(&'a self, curve_knots: Option<&'a [f64]>) -> &'a [f64] {
        match self {
            Self::Standard => STANDARD_TENORS,
            Self::Dynamic => curve_knots.unwrap_or(STANDARD_TENORS),
            Self::Custom(tenors) => tenors.as_slice(),
        }
    }
}

/// Reject an empty tenor grid or a non-finite tenor.
fn validate_measurement_grid(sample_points: &[f64]) -> Result<()> {
    if sample_points.is_empty() || sample_points.iter().any(|t| !t.is_finite()) {
        return Err(crate::Error::Validation(
            "measurement tenor grid must be non-empty and finite".to_string(),
        ));
    }
    Ok(())
}

/// Generic internal measurement helper for curve-like objects.
fn measure_average_shift(
    sample_points: &[f64],
    scaling_factor: f64,
    mut value_t0: impl FnMut(f64) -> f64,
    mut value_t1: impl FnMut(f64) -> f64,
) -> Result<f64> {
    validate_measurement_grid(sample_points)?;
    if !scaling_factor.is_finite() {
        return Err(crate::Error::Validation(
            "shift scaling factor must be finite".to_string(),
        ));
    }
    let mut total_shift = 0.0;
    let mut count = 0;

    for &t in sample_points {
        if t <= 0.0 {
            continue;
        }
        let v0 = value_t0(t);
        let v1 = value_t1(t);
        if !v0.is_finite() || !v1.is_finite() {
            return Err(crate::Error::Validation(format!(
                "curve values at tenor {t} must be finite"
            )));
        }

        let shift = if values_are_effectively_equal(v0, v1) {
            0.0
        } else {
            (v1 - v0) * scaling_factor
        };
        total_shift += shift;
        if !total_shift.is_finite() {
            return Err(crate::Error::Validation(
                "accumulated curve shift is not finite".to_string(),
            ));
        }
        count += 1;
    }

    if count == 0 {
        return Err(crate::InputError::TooFewPoints.into());
    }
    let average = total_shift / count as f64;
    if !average.is_finite() {
        return Err(crate::Error::Validation(
            "average curve shift is not finite".to_string(),
        ));
    }
    Ok(average)
}

/// Map each tenor through `shift`, rejecting a non-finite result; `what`
/// names the measured quantity in the error.
fn per_tenor_shifts(tenors: &[f64], what: &str, shift: impl Fn(f64) -> f64) -> Result<Vec<f64>> {
    validate_measurement_grid(tenors)?;
    tenors
        .iter()
        .map(|&t| {
            let shift = shift(t);
            if shift.is_finite() {
                Ok(shift)
            } else {
                Err(crate::Error::Validation(format!(
                    "{what} shift at tenor {t} is not finite"
                )))
            }
        })
        .collect()
}

fn values_are_effectively_equal(a: f64, b: f64) -> bool {
    const RELATIVE_EQUALITY_TOLERANCE: f64 = 1.0e-14;

    if !(a.is_finite() && b.is_finite()) {
        return false;
    }
    let scale = a.abs().max(b.abs()).max(1.0);
    (a - b).abs() <= RELATIVE_EQUALITY_TOLERANCE * scale
}

/// Measure average parallel rate shift in discount curve (basis points).
///
/// # Arguments
///
/// * `curve_id` - Identifier of the discount curve to retrieve from both market
///   contexts. The same ID must resolve in each context.
/// * `market_t0` - Baseline market context representing the earlier state.
/// * `market_t1` - Comparison market context representing the later state.
/// * `method` - Tenor-sampling policy used to average zero-rate differences.
///   `Dynamic` samples the baseline curve's knots.
///
/// # Errors
///
/// Returns an error if `curve_id` is absent from either market context, the
/// selected grid is empty or non-finite, or a curve cannot provide a finite
/// sample value on that grid.
pub fn measure_discount_curve_shift(
    curve_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    method: TenorSamplingMethod,
) -> Result<f64> {
    let curve_t0 = market_t0.get_discount(&curve_id)?;
    let curve_t1 = market_t1.get_discount(&curve_id)?;

    let tenors = method.tenors(Some(curve_t0.knots()));
    measure_average_shift(
        tenors,
        BASIS_POINTS_PER_UNIT,
        |t| curve_t0.zero_rate(t, crate::math::Compounding::Continuous),
        |t| curve_t1.zero_rate(t, crate::math::Compounding::Continuous),
    )
}

/// Measure the average **par CDS spread** shift on a hazard curve (basis points).
///
/// Where a raw hazard-rate shift would sample `hazard_rate`, this samples
/// the par CDS spread (via `HazardCurve::cds_quote_bp`) rather than the raw
/// hazard rate. Credit sensitivities (`Cs01`, `BucketedCs01`) are defined per bp
/// of **par-spread** move, so attribution must pair them with this — pairing a
/// par-spread CS01 with a hazard-rate move overstates credit P&L by a factor of
/// `1 / (1 - recovery)`.
///
/// # Arguments
///
/// * `curve_id` - Identifier of the hazard curve to retrieve from both market
///   contexts. The same ID must resolve in each context.
/// * `market_t0` - Baseline market context representing the earlier state.
/// * `market_t1` - Comparison market context representing the later state.
/// * `method` - Tenor-sampling policy used to average par-CDS-spread changes.
///   `Dynamic` samples the baseline hazard curve's knots.
///
/// # Errors
///
/// Returns an error if `curve_id` is absent from either market context, the
/// selected grid is empty or non-finite, or a curve cannot provide a finite
/// sample value on that grid.
pub fn measure_par_spread_shift(
    curve_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    method: TenorSamplingMethod,
) -> Result<f64> {
    let curve_t0 = market_t0.get_hazard(&curve_id)?;
    let curve_t1 = market_t1.get_hazard(&curve_id)?;

    let t0_knots: Vec<f64> = curve_t0.knot_points().map(|(t, _)| t).collect();
    let tenors = match method {
        TenorSamplingMethod::Dynamic => t0_knots.as_slice(),
        _ => method.tenors(None),
    };

    let interp_t0 = curve_t0.par_interp();
    let interp_t1 = curve_t1.par_interp();
    // `cds_quote_bp` already returns basis points → scaling factor 1.0.
    measure_average_shift(
        tenors,
        1.0,
        |t| curve_t0.cds_quote_bp(t, interp_t0),
        |t| curve_t1.cds_quote_bp(t, interp_t1),
    )
}

/// Measure the **per-tenor** par CDS spread shift (basis points) at the given
/// tenors.
///
/// Unlike [`measure_par_spread_shift`] (which averages over a tenor grid), this
/// returns the shift at each requested tenor so callers can pair it with a
/// per-tenor (key-rate) `BucketedCs01`, attributing non-parallel (twisted)
/// credit-curve moves correctly. Returns one entry per input tenor, in order.
///
/// # Arguments
///
/// * `curve_id` - Identifier of the hazard curve to retrieve from both market
///   contexts. The same ID must resolve in each context.
/// * `market_t0` - Baseline market context representing the earlier state.
/// * `market_t1` - Comparison market context representing the later state.
/// * `tenors` - Year-fraction maturities at which to measure the par-CDS-spread
///   change. Output positions match this input order.
///
/// # Errors
///
/// Returns an error if `curve_id` is absent from either market context, the
/// selected grid is empty or non-finite, or a curve cannot provide a finite
/// sample value on that grid.
pub(crate) fn measure_per_tenor_par_spread_shift(
    curve_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    tenors: &[f64],
) -> Result<Vec<f64>> {
    let curve_t0 = market_t0.get_hazard(&curve_id)?;
    let curve_t1 = market_t1.get_hazard(&curve_id)?;
    let interp_t0 = curve_t0.par_interp();
    let interp_t1 = curve_t1.par_interp();
    per_tenor_shifts(tenors, "par spread", |t| {
        curve_t1.cds_quote_bp(t, interp_t1) - curve_t0.cds_quote_bp(t, interp_t0)
    })
}

/// Measure the average **credit-curve** shift (basis points), accepting either
/// curve representation a credit-risky instrument may use.
///
/// A credit curve declared in an instrument's `credit_curves` dependency can be
/// modelled two ways:
///
/// - a [`HazardCurve`](crate::market_data::term_structures::HazardCurve) — the
///   move is the **par CDS spread** shift ([`measure_par_spread_shift`]), the
///   basis a hazard-curve `Cs01` is defined on; or
/// - a [`DiscountCurve`](crate::market_data::term_structures::DiscountCurve)
///   serving a credit role — the move is the **zero-rate** shift
///   ([`measure_discount_curve_shift`]), the basis a discount-style `Cs01` is
///   bumped on.
///
/// The hazard interpretation is tried first, falling back to the discount
/// interpretation, so the returned move is always unit-consistent with the
/// instrument's own CS01. Used by P&L attribution so credit-spread P&L from a
/// discount-style credit curve is attributed to the credit factor rather than
/// leaking into the residual.
///
/// # Arguments
///
/// * `curve_id` - Identifier that resolves to a hazard curve or risky discount
///   curve in both market contexts.
/// * `market_t0` - Baseline market context representing the earlier state.
/// * `market_t1` - Comparison market context representing the later state.
/// * `method` - Tenor-sampling policy used for the averaged move. It is passed
///   to the selected hazard- or discount-curve measurement.
///
/// # Errors
///
/// Returns an error when `curve_id` is neither a hazard curve nor a discount
/// curve in both market contexts.
pub fn measure_credit_curve_shift(
    curve_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    method: TenorSamplingMethod,
) -> Result<f64> {
    let curve_id = curve_id.as_ref();
    if let Ok(shift) = measure_par_spread_shift(curve_id, market_t0, market_t1, method.clone()) {
        return Ok(shift);
    }
    measure_discount_curve_shift(curve_id, market_t0, market_t1, method)
}

/// Measure the **per-tenor** credit-curve shift (basis points) at the given
/// tenors, accepting either curve representation.
///
/// The per-tenor counterpart of [`measure_credit_curve_shift`] (see that
/// function for the hazard / discount duality): tries the par CDS spread
/// interpretation first (`measure_per_tenor_par_spread_shift`); on failure
/// falls back to the per-tenor discount zero-rate shift. Returns one entry per
/// input tenor, in order, so callers can pair it with a per-tenor (key-rate)
/// `BucketedCs01`.
///
/// # Arguments
///
/// * `curve_id` - Identifier that resolves to a hazard curve or risky discount
///   curve in both market contexts.
/// * `market_t0` - Baseline market context representing the earlier state.
/// * `market_t1` - Comparison market context representing the later state.
/// * `tenors` - Year-fraction maturities at which to measure the selected
///   credit-curve move. Output positions match this input order.
///
/// # Errors
///
/// Returns an error when `curve_id` is neither a hazard curve nor a discount
/// curve in both market contexts.
pub fn measure_per_tenor_credit_curve_shift(
    curve_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    tenors: &[f64],
) -> Result<Vec<f64>> {
    let curve_id = curve_id.as_ref();
    if let Ok(shifts) = measure_per_tenor_par_spread_shift(curve_id, market_t0, market_t1, tenors) {
        return Ok(shifts);
    }
    let curve_t0 = market_t0.get_discount(curve_id)?;
    let curve_t1 = market_t1.get_discount(curve_id)?;
    per_tenor_shifts(tenors, "zero-rate", |t| {
        (curve_t1.zero_rate(t, crate::math::Compounding::Continuous)
            - curve_t0.zero_rate(t, crate::math::Compounding::Continuous))
            * BASIS_POINTS_PER_UNIT
    })
}

/// Measure average inflation rate shift (basis points).
///
/// # Arguments
///
/// * `curve_id` - Identifier of the projected inflation curve to retrieve from
///   both market contexts.
/// * `market_t0` - Baseline market context representing the earlier state.
/// * `market_t1` - Comparison market context representing the later state.
///
/// # Errors
///
/// Returns an error if `curve_id` is absent from either market context.
pub fn measure_inflation_curve_shift(
    curve_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    let curve_t0 = market_t0.get_inflation_curve(&curve_id)?;
    let curve_t1 = market_t1.get_inflation_curve(&curve_id)?;
    // Annualized zero inflation to `t`; `measure_average_shift` skips `t <= 0`.
    let zero_inflation = |curve: &crate::market_data::term_structures::InflationCurve, t: f64| {
        (curve.cpi(t) / curve.base_cpi()).powf(1.0 / t) - 1.0
    };
    measure_average_shift(
        STANDARD_TENORS,
        BASIS_POINTS_PER_UNIT,
        |t| zero_inflation(&curve_t0, t),
        |t| zero_inflation(&curve_t1, t),
    )
}

/// Measure the change between the latest published index levels, in basis points.
/// This is a level move, not an annualized growth rate over the full history.
///
/// # Errors
///
/// Returns an error if `index_id` is absent, has no observations, or has a
/// non-finite or non-positive latest stored level. Contractual lag,
/// interpolation, and seasonal adjustments are not applied to published prints.
///
/// # Arguments
///
/// * `index_id` - Identifier of the published inflation index to retrieve from
///   both market contexts.
/// * `market_t0` - Baseline market context representing the earlier state.
/// * `market_t1` - Comparison market context representing the later state.
pub fn measure_inflation_index_shift(
    index_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    let index_id = index_id.as_ref();
    let index_t0 = market_t0.get_inflation_index(index_id)?;
    let index_t1 = market_t1.get_inflation_index(index_id)?;
    let (_, level_t0) = index_t0.get_latest_observation()?;
    let (_, level_t1) = index_t1.get_latest_observation()?;
    if !level_t0.is_finite() || !level_t1.is_finite() || level_t0 <= 0.0 || level_t1 <= 0.0 {
        return Err(crate::Error::Validation(
            "inflation index levels must be finite and positive".to_string(),
        ));
    }
    Ok((level_t1 / level_t0 - 1.0) * BASIS_POINTS_PER_UNIT)
}

/// Measure a declared inflation source using one canonical risk factor.
///
/// When a projected curve is available, use its parallel-rate move. Published
/// index levels are used only for index-only sources; adding both would apply a
/// single Inflation01 to two different risk factors.
///
/// # Errors
///
/// Returns an error when the selected curve or index source is unavailable or
/// when its latest values fail the validation performed by the delegated
/// measurement.
///
/// # Arguments
///
/// * `source_id` - Shared identifier of an inflation curve or published index.
///   A curve is preferred when both source forms are available.
/// * `market_t0` - Baseline market context representing the earlier state.
/// * `market_t1` - Comparison market context representing the later state.
pub fn measure_inflation_source_shift(
    source_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    let source_id = source_id.as_ref();
    let has_curve = market_t0.get_inflation_curve(source_id).is_ok()
        && market_t1.get_inflation_curve(source_id).is_ok();
    let has_index = market_t0.get_inflation_index(source_id).is_ok()
        && market_t1.get_inflation_index(source_id).is_ok();
    if has_curve || !has_index {
        measure_inflation_curve_shift(source_id, market_t0, market_t1)
    } else {
        measure_inflation_index_shift(source_id, market_t0, market_t1)
    }
}

/// Measure FX spot rate shift (percentage change).
///
/// Calculates the percentage change in FX rate between two markets.
///
/// # Arguments
///
/// * `base_currency` - Base currency of the FX quote; the measured rate is units
///   of `quote_currency` received for one unit of this currency.
/// * `quote_currency` - Quote currency of the FX quote whose change is measured.
/// * `market_t0` - Baseline market context supplying the earlier FX matrix.
/// * `market_t1` - Comparison market context supplying the later FX matrix.
/// * `as_of_t0` - Valuation date used to query the baseline FX matrix.
/// * `as_of_t1` - Valuation date used to query the comparison FX matrix.
///
/// # Returns
///
/// Percentage change in FX rate: (rate_t1 / rate_t0 - 1) * 100
///
/// # Errors
///
/// Returns error if FX matrix not available in either market or conversion fails.
///
/// # Examples
///
/// ```rust
/// # use finstack_quant_core::market_data::diff::measure_fx_shift;
/// # use finstack_quant_core::market_data::context::MarketContext;
/// # use finstack_quant_core::currency::Currency;
/// # use time::macros::date;
/// # fn example(market_t0: MarketContext, market_t1: MarketContext) -> finstack_quant_core::Result<()> {
/// let date_t0 = date!(2024 - 01 - 01);
/// let date_t1 = date!(2024 - 01 - 02);
/// let fx_shift = measure_fx_shift(
///     Currency::USD,
///     Currency::EUR,
///     &market_t0,
///     &market_t1,
///     date_t0,
///     date_t1,
/// )?;
///
/// println!("USD/EUR moved {}%", fx_shift);
/// # Ok(())
/// # }
/// ```
pub fn measure_fx_shift(
    base_currency: Currency,
    quote_currency: Currency,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
) -> Result<f64> {
    use crate::money::fx::FxQuery;

    // Get FX matrices via the shared helper on MarketContext.
    let fx_t0 = market_t0.fx_required()?;
    let fx_t1 = market_t1.fx_required()?;

    // Get rates using FxQuery with the provided valuation dates
    let query_t0 = FxQuery::new(base_currency, quote_currency, as_of_t0, Default::default());
    let query_t1 = FxQuery::new(base_currency, quote_currency, as_of_t1, Default::default());
    let rate_t0 = fx_t0.rate(query_t0)?.rate;
    let rate_t1 = fx_t1.rate(query_t1)?.rate;

    // `FxMatrix::rate` returns only finite, strictly positive rates.
    // Percentage change
    let pct_change = (rate_t1 / rate_t0 - 1.0) * 100.0;

    Ok(pct_change)
}

/// Measure market scalar shift (percentage change).
///
/// Measures the change in market scalar values (equity prices, commodities, etc.).
///
/// # Arguments
///
/// * `scalar_id` - Market scalar identifier
/// * `market_t0` - Market context at T₀
/// * `market_t1` - Market context at T₁
///
/// # Returns
///
/// Percentage change in scalar value.
///
/// # Errors
///
/// Returns an error if the scalar is absent, its kind differs between markets,
/// monetary prices use different currencies, the baseline is zero, or the
/// resulting shift is non-finite. Currency changes require explicit conversion
/// before comparison.
pub fn measure_scalar_shift(
    scalar_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    let scalar_t0 = market_t0.get_price(&scalar_id)?;
    let scalar_t1 = market_t1.get_price(&scalar_id)?;
    let (value_t0, value_t1) = comparable_scalar_values(scalar_id.as_ref(), scalar_t0, scalar_t1)?;

    // Guard against division by zero
    if value_t0.abs() < 1e-15 {
        return Err(crate::Error::Validation(format!(
            "Cannot compute percentage shift: baseline value is zero for scalar '{}'",
            scalar_id.as_ref()
        )));
    }

    // Percentage change
    let pct_change = (value_t1 / value_t0 - 1.0) * 100.0;

    if !pct_change.is_finite() {
        return Err(crate::Error::Validation(format!(
            "Non-finite percentage shift computed for scalar '{}': t0={}, t1={}",
            scalar_id.as_ref(),
            value_t0,
            value_t1
        )));
    }

    Ok(pct_change)
}

/// Compute the **absolute** change in a market scalar between two market states.
///
/// Unlike [`measure_scalar_shift`], which returns a percentage change, this
/// returns `value_t1 - value_t0` in the scalar's native units. Use this when
/// applying a sensitivity defined per unit of underlying move — e.g. an option
/// delta `dPV/dS` or a dividend sensitivity `dPV/d(dividend)` — where
/// multiplying by a percentage shift would introduce a `100 / level` scaling
/// error.
///
/// # Errors
///
/// Returns an error if either market lacks the scalar, scalar kinds differ,
/// monetary currencies differ, or the computed shift is non-finite. Prices
/// must be converted explicitly to the same currency before comparison.
///
/// # Arguments
///
/// * `scalar_id` - Identifier of the unitless scalar or money-denominated
///   price to compare in both market contexts.
/// * `market_t0` - Baseline market context representing the earlier state.
/// * `market_t1` - Comparison market context representing the later state.
pub fn measure_scalar_absolute_shift(
    scalar_id: impl AsRef<str>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<f64> {
    let scalar_t0 = market_t0.get_price(&scalar_id)?;
    let scalar_t1 = market_t1.get_price(&scalar_id)?;
    let (value_t0, value_t1) = comparable_scalar_values(scalar_id.as_ref(), scalar_t0, scalar_t1)?;

    let abs_change = value_t1 - value_t0;
    if !abs_change.is_finite() {
        return Err(crate::Error::Validation(format!(
            "Non-finite absolute shift computed for scalar '{}': t0={}, t1={}",
            scalar_id.as_ref(),
            value_t0,
            value_t1
        )));
    }
    Ok(abs_change)
}

fn comparable_scalar_values(
    scalar_id: &str,
    baseline: &super::scalars::MarketScalar,
    comparison: &super::scalars::MarketScalar,
) -> Result<(f64, f64)> {
    use super::scalars::MarketScalar;

    match (baseline, comparison) {
        (MarketScalar::Unitless(a), MarketScalar::Unitless(b)) => Ok((*a, *b)),
        (MarketScalar::Price(a), MarketScalar::Price(b)) => {
            if a.currency() != b.currency() {
                return Err(crate::Error::CurrencyMismatch {
                    expected: a.currency(),
                    actual: b.currency(),
                });
            }
            Ok((a.amount(), b.amount()))
        }
        _ => Err(crate::Error::Validation(format!(
            "Cannot compare scalar '{scalar_id}': monetary and unitless values have different units"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::currency::Currency;
    use crate::dates::Date;
    use crate::market_data::scalars::InflationIndex;
    use crate::market_data::term_structures::{DiscountCurve, HazardCurve, InflationCurve};
    use crate::math::interp::InterpStyle;
    use time::Month;

    fn sample_date() -> Date {
        Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date")
    }

    #[test]
    fn inflation_print_shift_ignores_lag_interpolation_and_seasonality() {
        use crate::market_data::scalars::{InflationLag, SeriesInterpolation};

        let observations = vec![
            (time::macros::date!(2025 - 01 - 01), 300.0),
            (time::macros::date!(2025 - 02 - 01), 301.0),
            (time::macros::date!(2025 - 03 - 01), 302.0),
            (time::macros::date!(2025 - 04 - 01), 303.0),
            (time::macros::date!(2025 - 05 - 01), 306.0),
        ];
        let mut seasonality = [1.0; 12];
        seasonality[0] = 1.1;
        seasonality[1] = 0.9;
        seasonality[3] = 1.2;
        seasonality[4] = 0.8;
        for lag in [InflationLag::None, InflationLag::Months(3)] {
            for interpolation in [SeriesInterpolation::Step, SeriesInterpolation::Linear] {
                let context = |observations| {
                    let index = InflationIndex::new("CPI", observations, Currency::USD)
                        .expect("index")
                        .with_lag(lag)
                        .with_interpolation(interpolation)
                        .with_seasonality(seasonality)
                        .expect("seasonality");
                    MarketContext::new().insert_inflation_index(index)
                };
                let t0 = context(observations[..4].to_vec());
                let t1 = context(observations.clone());
                let shift = measure_inflation_index_shift("CPI", &t0, &t1).expect("print shift");
                assert!((shift - (306.0 / 303.0 - 1.0) * 10_000.0).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn inflation_print_shift_does_not_require_lagged_history() {
        use crate::market_data::scalars::InflationLag;

        let context = |value| {
            let index = InflationIndex::new("CPI", vec![(sample_date(), value)], Currency::USD)
                .expect("single print")
                .with_lag(InflationLag::Months(3));
            MarketContext::new().insert_inflation_index(index)
        };
        let shift = measure_inflation_index_shift("CPI", &context(100.0), &context(101.0))
            .expect("raw print does not need lagged history");
        assert!((shift - 100.0).abs() < 1e-10);
    }

    #[test]
    fn scalar_shifts_reject_incompatible_units_and_currencies() {
        use crate::market_data::scalars::MarketScalar;
        use crate::money::Money;

        let price = |currency| MarketScalar::Price(Money::new(100.0, currency).expect("price"));
        for (left, right) in [
            (price(Currency::USD), price(Currency::EUR)),
            (price(Currency::USD), MarketScalar::Unitless(100.0)),
            (MarketScalar::Unitless(100.0), price(Currency::USD)),
        ] {
            let t0 = MarketContext::new().insert_price("SPOT", left);
            let t1 = MarketContext::new().insert_price("SPOT", right);
            assert!(measure_scalar_shift("SPOT", &t0, &t1).is_err());
            assert!(measure_scalar_absolute_shift("SPOT", &t0, &t1).is_err());
        }

        for (left, right) in [
            (MarketScalar::Unitless(100.0), MarketScalar::Unitless(120.0)),
            (
                price(Currency::USD),
                MarketScalar::Price(Money::new(120.0, Currency::USD).expect("price")),
            ),
        ] {
            let t0 = MarketContext::new().insert_price("SPOT", left);
            let t1 = MarketContext::new().insert_price("SPOT", right);
            assert!(
                (measure_scalar_shift("SPOT", &t0, &t1).expect("same units") - 20.0).abs() < 1e-12
            );
            assert_eq!(
                measure_scalar_absolute_shift("SPOT", &t0, &t1).expect("same units"),
                20.0
            );
        }
    }

    #[test]
    fn inflation_index_shift_measures_newly_published_print() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let prior_end = Date::from_calendar_date(2025, Month::December, 1).expect("date");
        let new_end = Date::from_calendar_date(2026, Month::January, 1).expect("date");
        let index_t0 = InflationIndex::new(
            "US-CPI",
            vec![(start, 100.0), (prior_end, 110.0)],
            Currency::USD,
        )
        .expect("t0 index");
        let index_t1 = InflationIndex::new(
            "US-CPI",
            vec![(start, 100.0), (prior_end, 110.0), (new_end, 112.0)],
            Currency::USD,
        )
        .expect("t1 index");
        let market_t0 = MarketContext::new().insert_inflation_index(index_t0);
        let market_t1 = MarketContext::new().insert_inflation_index(index_t1);

        let shift = measure_inflation_source_shift("US-CPI", &market_t0, &market_t1)
            .expect("published-print shift");
        assert!(
            shift > 0.0,
            "a higher newly published CPI print must be measured"
        );
    }

    #[test]
    fn hybrid_inflation_source_prefers_projected_curve_shift() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let prior_end = Date::from_calendar_date(2025, Month::December, 1).expect("date");
        let new_end = Date::from_calendar_date(2026, Month::January, 1).expect("date");
        let curve = || {
            InflationCurve::builder("US-CPI")
                .base_date(start)
                .base_cpi(100.0)
                .knots([(0.0, 100.0), (5.0, 110.0)])
                .build()
                .expect("inflation curve")
        };
        let index_t0 = InflationIndex::new(
            "US-CPI",
            vec![(start, 100.0), (prior_end, 110.0)],
            Currency::USD,
        )
        .expect("t0 index");
        let index_t1 = InflationIndex::new(
            "US-CPI",
            vec![(start, 100.0), (prior_end, 110.0), (new_end, 112.0)],
            Currency::USD,
        )
        .expect("t1 index");
        let market_t0 = MarketContext::new()
            .insert(curve())
            .insert_inflation_index(index_t0);
        let market_t1 = MarketContext::new()
            .insert(curve())
            .insert_inflation_index(index_t1);

        let source_shift =
            measure_inflation_source_shift("US-CPI", &market_t0, &market_t1).expect("hybrid shift");
        assert!(source_shift.abs() < 1e-12);
    }

    #[test]
    fn test_credit_curve_shift_falls_back_to_discount_zero_rate() {
        // A credit curve modelled as a `DiscountCurve` has no par CDS quote — `measure_credit_curve_shift`
        // must fall back to the zero-rate shift instead of erroring.
        let base_date = sample_date();
        let curve = |rate: f64| {
            DiscountCurve::builder("USD-CREDIT")
                .base_date(base_date)
                .knots([(0.0, 1.0), (10.0, (-rate * 10.0).exp())])
                .interp(InterpStyle::LogLinear)
                .build()
                .expect("discount curve should build")
        };
        // +40bp move in the risky discount curve's zero rate.
        let market_t0 = MarketContext::new().insert(curve(0.05));
        let market_t1 = MarketContext::new().insert(curve(0.054));

        let shift = measure_credit_curve_shift(
            "USD-CREDIT",
            &market_t0,
            &market_t1,
            TenorSamplingMethod::Standard,
        )
        .expect("credit-curve shift should fall back to discount measurement");
        assert!(
            (shift - 40.0).abs() < 1.0,
            "expected ~40bp zero-rate shift, got {shift}"
        );

        let per_tenor = measure_per_tenor_credit_curve_shift(
            "USD-CREDIT",
            &market_t0,
            &market_t1,
            &[1.0, 5.0, 10.0],
        )
        .expect("per-tenor credit-curve shift should fall back to discount measurement");
        assert_eq!(per_tenor.len(), 3, "one entry per requested tenor");
        for s in per_tenor {
            assert!(
                (s - 40.0).abs() < 1.0,
                "per-tenor zero-rate shift ~40bp, got {s}"
            );
        }
    }

    #[test]
    fn test_credit_curve_shift_uses_par_spread_for_hazard_curve() {
        // For a `HazardCurve`, `measure_credit_curve_shift` must use the par
        // CDS spread move (== hazard shift × LGD), matching `measure_par_spread_shift`.
        let base_date = sample_date();
        let curve_t0 = HazardCurve::builder("CORP-01")
            .base_date(base_date)
            .recovery_rate(0.4)
            .knots(vec![(1.0, 0.01), (5.0, 0.02), (10.0, 0.025)])
            .build()
            .expect("hazard curve t0");
        let curve_t1 = HazardCurve::builder("CORP-01")
            .base_date(base_date)
            .recovery_rate(0.4)
            .knots(vec![(1.0, 0.0125), (5.0, 0.0225), (10.0, 0.0275)])
            .build()
            .expect("hazard curve t1");
        let market_t0 = MarketContext::new().insert(curve_t0);
        let market_t1 = MarketContext::new().insert(curve_t1);

        let credit_shift = measure_credit_curve_shift(
            "CORP-01",
            &market_t0,
            &market_t1,
            TenorSamplingMethod::Standard,
        )
        .expect("credit shift");
        let par_shift = measure_par_spread_shift(
            "CORP-01",
            &market_t0,
            &market_t1,
            TenorSamplingMethod::Standard,
        )
        .expect("par-spread shift");
        assert_eq!(
            credit_shift, par_shift,
            "hazard-curve credit shift must equal the par-spread shift"
        );
    }

    #[test]
    fn average_shift_uses_relative_equality_for_large_magnitudes() {
        let shift = measure_average_shift(&[1.0], 10_000.0, |_| 1.0e9, |_| 1.0e9 + 1.0e-6)
            .expect("finite sample should measure");

        assert_eq!(shift, 0.0);
    }

    #[test]
    fn shift_measurement_rejects_empty_or_non_finite_grids_and_samples() {
        assert!(measure_average_shift(&[], 1.0, |_| 0.0, |_| 1.0).is_err());
        assert!(measure_average_shift(&[f64::NAN], 1.0, |_| 0.0, |_| 1.0).is_err());
        assert!(measure_average_shift(&[1.0], 1.0, |_| f64::INFINITY, |_| 1.0).is_err());
        assert!(validate_measurement_grid(&[]).is_err());
        assert!(validate_measurement_grid(&[f64::INFINITY]).is_err());
    }
}
