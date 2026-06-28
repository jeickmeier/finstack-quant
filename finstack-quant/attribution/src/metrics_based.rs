//! Metrics-based P&L attribution.
//!
//! Fast approximation using pre-computed risk metrics (Theta, DV01, CS01, Vega, etc.)
//! to estimate factor contributions without full repricing. Supports both first-order
//! (linear) and second-order (convexity) terms for improved accuracy.
//!
//! # Algorithm (Enhanced with Second-Order and Bucketed Metrics)
//!
//! 1. **Carry**: Theta × time_period
//! 2. **RatesCurves**:
//!    - Per-curve (if BucketedDv01 available): Σ(DV01_i × Δr_i) for each curve i
//!    - Fallback (aggregate DV01): DV01 × avg(Δr_i)
//!    - Second-order: ½ × Convexity × (Δr)² (if available)
//! 3. **CreditCurves**:
//!    - First-order: CS01 × Δs
//!    - Second-order: ½ × CS-Gamma × (Δs)² (if available)
//! 4. **Fx**: FX01 × Δfx
//! 5. **Volatility**:
//!    - First-order: Vega × Δσ
//!    - Second-order: ½ × Volga × (Δσ)²
//!    - Cross-term: CrossGammaSpotVol × Δspot_pct × Δσ_vol_pt
//!      (NOT Vanna — see the unit-contract note at the cross-factor site)
//! 6. **Market Scalars** (for options):
//!    - First-order: Delta × Δspot
//!    - Second-order: ½ × Gamma × (Δspot)²
//! 7. **Inflation**:
//!    - First-order: Inflation01 × Δi
//!    - Second-order: ½ × InflationConvexity × (Δi)²
//! 8. **ModelParameters**: Param01 metrics × param_shift
//! 9. **Residual**: Total P&L - sum(approximations)
//!
//! # Advantages (Enhanced)
//!
//! - Fast: Still no additional repricing required
//! - More accurate: Per-curve bucketed DV01 eliminates basis risk errors
//! - Second-order terms reduce residual from ~18% to <5%
//! - Graceful degradation: Works with or without bucketed/second-order metrics
//! - Convenient: Works with already-computed ValuationResults
//!
//! # Disadvantages
//!
//! - Still approximate (third-order+ effects ignored)
//! - Less accurate than parallel/waterfall methods for extreme moves
//! - Large market moves (>100bp rates, >5% vol) can exceed reliable approximation range
//!
//! # Metric Unit Contracts
//!
//! This module expects metrics to follow these unit conventions:
//!
//! | Metric              | Unit            | Definition                                                |
//! |---------------------|-----------------|-----------------------------------------------------------|
//! | DV01                | $ / bp          | Dollar change per 1bp parallel rate shift                 |
//! | Convexity           | per-100 (street)| Street convexity: (∂²P/∂y²) / P / 100 (Bloomberg YAS)     |
//! | IrConvexity         | $ / decimal²    | Raw dollar second derivative ∂²PV/∂r² (swaps)             |
//! | CS01                | $ / bp          | Dollar change per 1bp spread shift                        |
//! | CsGamma             | $ / decimal²    | Dollar second derivative ∂²V/∂s² (spread in decimal)      |
//! | Vega                | $ / vol point   | Dollar change per 1% absolute vol shift                   |
//! | Volga               | $ / vol point²  | Dollar second derivative per vol point²                   |
//! | Theta               | $ / day         | Dollar time decay per calendar day                        |
//! | Inflation01         | $ / bp          | Dollar change per 1bp inflation-curve shift               |
//! | InflationConvexity  | $ / decimal²    | Dollar second derivative ∂²V/∂i² (inflation in decimal)   |
//!
//! **Important**: `Convexity` and `IrConvexity` have DIFFERENT producer
//! conventions and are consumed with different formulas :
//! the bond producer emits *street convexity* (`d²P/dy² / P / 100`,
//! Bloomberg YAS), so `ΔP_convexity = ½ × P₀ × Convexity × 100 × (Δr_decimal)²`;
//! the IRS producer emits the raw dollar second derivative `d²PV/dr²`, so
//! `ΔP_convexity = ½ × IrConvexity × (Δr_decimal)²` with no P₀ factor (a
//! near-par swap has PV ≈ 0 but real gamma).
//!
//! `InflationConvexity` uses the `CsGamma`-style $/decimal² convention (no P₀
//! multiplier): `ΔP_inflation_convexity = ½ × InflationConvexity × (Δi_decimal)²`.
//! A pricer emitting `InflationConvexity` in the dimensionless-percentage
//! convention used by `Convexity` would mis-attribute by a factor of P₀
//! (e.g. 1,000,000× for a $1M bond).
//!
//! If your convexity metric uses different units, apply the appropriate scaling
//! factor before passing to attribution.

use super::helpers::*;
use super::types::*;
use finstack_quant_core::config::{RoundingContext, ZeroKind};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::diff::{
    measure_credit_curve_shift, measure_discount_curve_shift, measure_fx_shift,
    measure_inflation_curve_shift, measure_per_tenor_credit_curve_shift,
    measure_scalar_absolute_shift, measure_scalar_shift, measure_vol_surface_shift,
    TenorSamplingMethod,
};
#[cfg(test)]
use finstack_quant_core::market_data::term_structures::DiscountCurve;
#[cfg(test)]
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::math::NeumaierAccumulator;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::HashMap;
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::{collect_cashflows_in_period, MetricId};
use finstack_quant_valuations::results::ValuationResult;
use indexmap::IndexMap;
use std::sync::Arc;

// ═══════════════════════════════════════════════════════════════════════════════
// Large Move Warning Thresholds
// ═══════════════════════════════════════════════════════════════════════════════
//
// These thresholds define when market moves are large enough that second-order
// Taylor expansion may produce significant approximation errors (>5% relative).
//
// Beyond these thresholds, consider using parallel or waterfall attribution
// for more accurate results.

/// Maximum rate shift (in basis points) before warning about approximation accuracy.
/// Beyond ~100bp, third-order and higher terms become significant.
const LARGE_RATE_MOVE_THRESHOLD_BP: f64 = 100.0;

/// Maximum credit spread shift (in basis points) before warning.
/// Credit spread convexity is typically larger than rate convexity.
const LARGE_SPREAD_MOVE_THRESHOLD_BP: f64 = 50.0;

/// Maximum volatility shift (in percentage points) before warning.
/// Vol-of-vol effects become significant beyond ~5% absolute vol change.
const LARGE_VOL_MOVE_THRESHOLD_PCT: f64 = 5.0;

/// Extract per-curve bucketed DV01 sensitivities from ValuationResult measures.
///
/// Bucketed DV01 metrics are stored with composite keys like:
/// - `"bucketed_dv01::USD-OIS"` for per-curve total DV01
/// - `"bucketed_dv01"` for the primary curve (if single curve instrument)
///
/// This function parses these keys and returns a mapping of CurveId → DV01.
///
/// # Arguments
///
/// * `measures` - Measures from ValuationResult containing flattened bucketed metrics
/// * `curve_ids` - List of discount curves required by the instrument
///
/// # Returns
///
/// HashMap mapping each curve ID to its total DV01 sensitivity.
fn extract_bucketed_dv01_per_curve(
    measures: &indexmap::IndexMap<MetricId, f64>,
    curve_ids: &[CurveId],
) -> HashMap<CurveId, f64> {
    let mut result = HashMap::default();

    // Pattern 1: Explicit per-curve keys "bucketed_dv01::{curve_id}".
    // Reuse a single key buffer instead of a per-curve `format!` allocation.
    let mut key = String::new();
    for curve_id in curve_ids {
        key.clear();
        key.push_str("bucketed_dv01::");
        key.push_str(curve_id.as_str());
        if let Some(&dv01) = measures.get(key.as_str()) {
            result.insert(curve_id.clone(), dv01);
        }
    }

    // Pattern 2: For single-curve instruments, check the base key
    if result.is_empty() && curve_ids.len() == 1 {
        if let Some(&dv01) = measures.get("bucketed_dv01") {
            result.insert(curve_ids[0].clone(), dv01);
        }
    }

    // Diagnostic: warn when bucketed DV01 is unavailable for curves the caller
    // requested. Downstream attribution then falls back to coarser parallel
    // DV01 — silent without this warning.
    for curve_id in curve_ids {
        if !result.contains_key(curve_id) {
            tracing::warn!(
                curve_id = %curve_id.as_str(),
                "bucketed_dv01 unavailable for curve; attribution will fall back to aggregate \
                 parallel DV01 — results will be coarser",
            );
        }
    }

    result
}

/// Extract per-curve **key-rate** (per-tenor) DV01 sensitivities.
///
/// The `BucketedDv01` calculator flattens its per-tenor series into the
/// `measures` map under composite keys `bucketed_dv01::{curve}::{tenor_label}`
/// (e.g. `bucketed_dv01::USD-OIS::5y`). This walks the standard bucket grid and
/// collects, per curve, the `(tenor_years, dv01)` pairs that are present.
///
/// Returns a map `curve → Vec<(tenor_years, dv01)>`; a curve is absent from the
/// map when none of its per-tenor keys were found (caller then falls back to
/// the coarser per-curve-total or aggregate path).
fn extract_keyrate_dv01_per_curve(
    measures: &indexmap::IndexMap<MetricId, f64>,
    curve_ids: &[CurveId],
) -> HashMap<CurveId, Vec<(f64, f64)>> {
    use finstack_quant_valuations::metrics::{STANDARD_BUCKETS_YEARS, STANDARD_BUCKET_LABELS};

    let mut result: HashMap<CurveId, Vec<(f64, f64)>> = HashMap::default();
    // Reuse one key buffer across all curves/tenors: build the
    // `bucketed_dv01::{curve}::` prefix once per curve, then swap only the
    // trailing tenor label — no per-tenor `format!` allocation.
    let mut key = String::new();
    for curve_id in curve_ids {
        let mut buckets: Vec<(f64, f64)> = Vec::new();
        key.clear();
        key.push_str("bucketed_dv01::");
        key.push_str(curve_id.as_str());
        key.push_str("::");
        let prefix_len = key.len();
        for (&tenor_years, label) in STANDARD_BUCKETS_YEARS
            .iter()
            .zip(STANDARD_BUCKET_LABELS.iter())
        {
            key.truncate(prefix_len);
            key.push_str(label);
            if let Some(&dv01) = measures.get(key.as_str()) {
                buckets.push((tenor_years, dv01));
            }
        }
        if !buckets.is_empty() {
            result.insert(curve_id.clone(), buckets);
        }
    }
    result
}

/// Extract per-curve **key-rate** (per-tenor) par-spread CS01 sensitivities.
///
/// The `BucketedCs01` calculator (par-spread re-bootstrap) flattens its
/// per-tenor series into the `measures` map under composite keys
/// `bucketed_cs01::{curve}::{tenor_label}` (e.g. `bucketed_cs01::ACME-HAZ::5y`),
/// mirroring `bucketed_dv01`. This walks the standard bucket grid and collects,
/// per curve, the `(tenor_years, cs01)` pairs that are present.
///
/// Returns a map `curve → Vec<(tenor_years, cs01)>`; a curve is absent when none
/// of its per-tenor keys were found (caller then falls back to aggregate CS01).
pub(crate) fn extract_keyrate_cs01_per_curve(
    measures: &indexmap::IndexMap<MetricId, f64>,
    curve_ids: &[CurveId],
) -> HashMap<CurveId, Vec<(f64, f64)>> {
    use finstack_quant_valuations::metrics::{STANDARD_BUCKETS_YEARS, STANDARD_BUCKET_LABELS};

    let mut result: HashMap<CurveId, Vec<(f64, f64)>> = HashMap::default();
    // Reuse one key buffer across all curves/tenors (see
    // `extract_keyrate_dv01_per_curve`): build the prefix once per curve, then
    // swap only the trailing tenor label.
    let mut key = String::new();
    for curve_id in curve_ids {
        let mut buckets: Vec<(f64, f64)> = Vec::new();
        key.clear();
        key.push_str("bucketed_cs01::");
        key.push_str(curve_id.as_str());
        key.push_str("::");
        let prefix_len = key.len();
        for (&tenor_years, label) in STANDARD_BUCKETS_YEARS
            .iter()
            .zip(STANDARD_BUCKET_LABELS.iter())
        {
            key.truncate(prefix_len);
            key.push_str(label);
            if let Some(&cs01) = measures.get(key.as_str()) {
                buckets.push((tenor_years, cs01));
            }
        }
        if !buckets.is_empty() {
            result.insert(curve_id.clone(), buckets);
        }
    }
    result
}

/// Measure the per-tenor discount-curve zero-rate shift (in basis points) at
/// the supplied tenors.
///
/// Unlike [`measure_discount_curve_shift`], which averages the shift over a
/// fixed tenor grid (and so mis-attributes a non-parallel move), this returns
/// the shift at each requested tenor so the caller can pair it with the
/// per-tenor (key-rate) DV01.
fn measure_per_tenor_discount_shift(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    tenors: &[f64],
) -> Option<Vec<f64>> {
    let curve_t0 = market_t0.get_discount(curve_id).ok()?;
    let curve_t1 = market_t1.get_discount(curve_id).ok()?;
    Some(
        tenors
            .iter()
            .map(|&t| (curve_t1.zero(t) - curve_t0.zero(t)) * 10_000.0)
            .collect(),
    )
}

/// Per-tenor rate shift (bp) for a rates curve that may be a discount curve
/// (zero rates) **or** a forward/projection curve (forward rates).
///
/// the rates ladder must consume forward-curve DV01 too —
/// `BucketedDv01` emits per-tenor series for projection curves, and a basis
/// move (discount and forward moving differently) is mis-attributed when only
/// discount curves are measured.
fn measure_per_tenor_rate_shift(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    tenors: &[f64],
) -> Option<Vec<f64>> {
    if let Some(shifts) = measure_per_tenor_discount_shift(curve_id, market_t0, market_t1, tenors) {
        return Some(shifts);
    }
    let curve_t0 = market_t0.get_forward(curve_id).ok()?;
    let curve_t1 = market_t1.get_forward(curve_id).ok()?;
    Some(
        tenors
            .iter()
            .map(|&t| (curve_t1.rate(t) - curve_t0.rate(t)) * 10_000.0)
            .collect(),
    )
}

/// Signed mean forward-rate shift (bp) over the standard tenor grid —
/// forward-curve counterpart of `measure_discount_curve_shift`.
fn measure_forward_curve_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Option<f64> {
    use finstack_quant_core::market_data::diff::STANDARD_TENORS;
    let curve_t0 = market_t0.get_forward(curve_id).ok()?;
    let curve_t1 = market_t1.get_forward(curve_id).ok()?;
    let mut total = 0.0;
    let mut count = 0usize;
    for &t in STANDARD_TENORS {
        if t <= 0.0 {
            continue;
        }
        let r0 = curve_t0.rate(t);
        let r1 = curve_t1.rate(t);
        if r0.is_finite() && r1.is_finite() {
            total += (r1 - r0) * 10_000.0;
            count += 1;
        }
    }
    if count == 0 {
        None
    } else {
        Some(total / count as f64)
    }
}

/// Signed mean rate shift (bp) for a curve that may be a discount or a
/// forward/projection curve.
fn measure_rate_curve_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Option<f64> {
    measure_discount_curve_shift(
        curve_id,
        market_t0,
        market_t1,
        TenorSamplingMethod::Standard,
    )
    .ok()
    .or_else(|| measure_forward_curve_shift_bp(curve_id, market_t0, market_t1))
}

/// Mean of the per-tenor *absolute* discount-curve zero-rate shift (bp) on
/// the standard tenor grid.
///
/// Where [`measure_discount_curve_shift`] returns the signed mean (which
/// collapses toward zero for a twist), this returns the L1 mean so a
/// non-parallel move still registers a large magnitude. Used by the
/// rates-convexity block to detect "the average is small but the curve
/// genuinely moved" — see audit rec #6.
///
/// Returns `0.0` if either side's curve is missing.
fn discount_curve_abs_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> f64 {
    use finstack_quant_core::market_data::diff::STANDARD_TENORS;
    let (Ok(c0), Ok(c1)) = (
        market_t0.get_discount(curve_id),
        market_t1.get_discount(curve_id),
    ) else {
        return 0.0;
    };
    let mut total_abs = 0.0;
    let mut count = 0usize;
    for &t in STANDARD_TENORS {
        if t <= 0.0 {
            continue;
        }
        let z0 = c0.zero(t);
        let z1 = c1.zero(t);
        if z0.is_finite() && z1.is_finite() {
            total_abs += (z1 - z0).abs() * 10_000.0;
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        total_abs / count as f64
    }
}

/// L1-mean rate shift (bp) for a curve that may be a discount or a
/// forward/projection curve. Forward-aware counterpart of
/// [`discount_curve_abs_shift_bp`] for the twist-guard block.
fn rate_curve_abs_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> f64 {
    use finstack_quant_core::market_data::diff::STANDARD_TENORS;
    let v = discount_curve_abs_shift_bp(curve_id, market_t0, market_t1);
    if v > 0.0 {
        return v;
    }
    let (Ok(c0), Ok(c1)) = (
        market_t0.get_forward(curve_id),
        market_t1.get_forward(curve_id),
    ) else {
        return 0.0;
    };
    let mut total_abs = 0.0;
    let mut count = 0usize;
    for &t in STANDARD_TENORS {
        if t <= 0.0 {
            continue;
        }
        let r0 = c0.rate(t);
        let r1 = c1.rate(t);
        if r0.is_finite() && r1.is_finite() {
            total_abs += (r1 - r0).abs() * 10_000.0;
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        total_abs / count as f64
    }
}

/// Threshold below which a signed mean shift is considered twist-dominated
/// relative to its L1 magnitude. Below this level, signed-average convexity
/// understates the true quadratic contribution, so downstream consumers should
/// fall back to per-tenor convexity.
const TWIST_FRACTION_THRESHOLD: f64 = 1e-2;

/// Mean of the per-tenor *absolute* credit-curve shift (bp) on the standard
/// tenor grid. Counterpart of [`discount_curve_abs_shift_bp`] for credit.
///
/// For a hazard curve this is the L1 mean of the par CDS spread move; for a
/// discount-style credit curve (e.g. a convertible's risky discount curve) it
/// is the L1 mean of the zero-rate move. Either way it pairs with the signed
/// mean that the per-method credit attribution consumes.
///
/// Returns `0.0` if either side's curve is missing.
fn credit_curve_abs_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> f64 {
    use finstack_quant_core::market_data::diff::STANDARD_TENORS;
    let tenors: Vec<f64> = STANDARD_TENORS
        .iter()
        .copied()
        .filter(|t| *t > 0.0)
        .collect();
    let Ok(shifts) = finstack_quant_core::market_data::diff::measure_per_tenor_credit_curve_shift(
        curve_id, market_t0, market_t1, &tenors,
    ) else {
        return 0.0;
    };
    let (total_abs, count) = shifts
        .iter()
        .filter(|v| v.is_finite())
        .fold((0.0, 0usize), |(acc, n), v| (acc + v.abs(), n + 1));
    if count == 0 {
        0.0
    } else {
        total_abs / count as f64
    }
}

/// Mean of the per-tenor *absolute* inflation-curve shift (bp) on the standard
/// tenor grid. Counterpart of [`discount_curve_abs_shift_bp`] for inflation.
///
/// Returns `0.0` if either side's curve is missing.
fn inflation_curve_abs_shift_bp(
    curve_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> f64 {
    use finstack_quant_core::market_data::diff::STANDARD_TENORS;
    let (Ok(c0), Ok(c1)) = (
        market_t0.get_inflation_curve(curve_id),
        market_t1.get_inflation_curve(curve_id),
    ) else {
        return 0.0;
    };
    let mut total_abs = 0.0;
    let mut count = 0usize;
    for &t in STANDARD_TENORS {
        if t <= 0.0 {
            continue;
        }
        // Inflation rate at tenor t from the cpi ratio (mirrors the
        // measure_inflation_curve_shift formula in core::market_data::diff).
        let rate = |c: &finstack_quant_core::market_data::term_structures::InflationCurve| -> f64 {
            let ratio = c.cpi(t) / c.base_cpi();
            ratio.powf(1.0 / t) - 1.0
        };
        let r0 = rate(&c0);
        let r1 = rate(&c1);
        if r0.is_finite() && r1.is_finite() {
            total_abs += (r1 - r0).abs() * 10_000.0;
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        total_abs / count as f64
    }
}

/// Format a diagnostic note when a signed average shift is twist-dominated
/// — i.e. `|signed_avg| < TWIST_FRACTION_THRESHOLD × l1_avg`. In that regime,
/// scalar second-order terms `½·γ·avg²` collapse toward 0 even though the
/// true `½·Δxᵀ·H·Δx` contribution is non-trivial.
///
/// Returns `None` when not twist-dominated (signed average is the dominant
/// component) or when there is no L1 magnitude to compare against.
fn twist_diagnostic_note(factor_label: &str, signed_avg: f64, l1_avg: f64) -> Option<String> {
    if l1_avg <= 0.0 {
        return None;
    }
    if signed_avg.abs() >= TWIST_FRACTION_THRESHOLD * l1_avg {
        return None;
    }
    Some(format!(
        "{factor_label} second-order may be understated: curves twisted \
         (signed mean shift {signed_avg:.3}bp vs L1 mean shift {l1_avg:.3}bp); \
         the scalar `½·γ·avg²` term collapses for twist-dominated moves. \
         Consider per-tenor second-order or parallel/waterfall attribution \
         for an accurate second-order contribution."
    ))
}

fn add_cross_factor_term(
    by_pair: &mut IndexMap<String, Money>,
    total: &mut f64,
    label: &str,
    pnl: f64,
    currency: finstack_quant_core::currency::Currency,
    notes: &mut Vec<String>,
    result_invalid: &mut bool,
) {
    if pnl.is_finite() && pnl.abs() < 1e-12 {
        return;
    }
    let money = factor_money_or_invalid(pnl, currency, label, notes, result_invalid);
    // Only accumulate into total if finite; the sentinel zero already keeps the
    // sum well-behaved when result_invalid is set.
    if pnl.is_finite() {
        *total += pnl;
    }
    by_pair.insert(label.to_string(), money);
}

/// Perform metrics-based P&L attribution for an instrument.
///
/// Uses linear approximation with pre-computed risk metrics. Fast but less
/// accurate than full repricing for large market moves.
///
/// # Bucketed DV01 Support
///
/// This function now prioritizes bucketed DV01 (per-curve sensitivities) over
/// aggregate DV01 for rates attribution:
///
/// - **If BucketedDv01 is available**: Computes PnL = Σ(DV01_i × Δr_i) per curve,
///   eliminating basis risk approximation errors.
/// - **Fallback**: Uses aggregate DV01 × avg(Δr_i) with a warning note.
///
/// To get the most accurate rates attribution, include `MetricId::BucketedDv01`
/// in your metrics request when computing valuations.
///
/// # Arguments
///
/// * `instrument` - Instrument to attribute
/// * `market_t0` - Market context at T₀ (for measuring market shifts)
/// * `market_t1` - Market context at T₁ (for measuring market shifts)
/// * `val_t0` - Valuation result at T₀ (with metrics, ideally including BucketedDv01)
/// * `val_t1` - Valuation result at T₁ (with metrics)
/// * `as_of_t0` - Valuation date at T₀
/// * `as_of_t1` - Valuation date at T₁
///
/// # Returns
///
/// P&L attribution using linear approximation with per-curve bucketed metrics.
///
/// # Errors
///
/// Returns error if:
/// - Required metrics are missing
/// - Currency conversion fails
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_attribution::attribute_pnl_metrics_based;
/// use finstack_quant_valuations::instruments::Instrument;
/// use finstack_quant_valuations::instruments::rates::deposit::Deposit;
/// use finstack_quant_valuations::instruments::PricingOptions;
/// use finstack_quant_valuations::metrics::MetricId;
/// use std::sync::Arc;
/// use time::macros::date;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let as_of_t0 = date!(2025-01-15);
/// let as_of_t1 = date!(2025-01-16);
/// let market_t0 = MarketContext::new();
/// let market_t1 = MarketContext::new();
///
/// // Minimal instrument (for compilation); real attribution requires populated market context.
/// let instrument = Arc::new(
///     Deposit::builder()
///         .id("DEP-1D".into())
///         .notional(Money::new(1_000_000.0, Currency::USD))
///         .start_date(as_of_t0)
///         .maturity(as_of_t1)
///         .day_count(finstack_quant_core::dates::DayCount::Act360)
///         .discount_curve_id("USD-OIS".into())
///         .build()
///         .expect("deposit builder should succeed"),
/// ) as Arc<dyn Instrument>;
///
/// // Compute valuations with bucketed metrics for best accuracy
/// let metrics = vec![
///     MetricId::Theta,
///     MetricId::Dv01,
///     MetricId::BucketedDv01,  // ← Include for per-curve rates attribution
///     MetricId::Cs01,
///     MetricId::Vega
/// ];
/// let val_t0 = instrument.price_with_metrics(&market_t0, as_of_t0, &metrics, PricingOptions::default())?;
/// let val_t1 = instrument.price_with_metrics(&market_t1, as_of_t1, &metrics, PricingOptions::default())?;
///
/// let attribution = attribute_pnl_metrics_based(
///     &instrument,
///     &market_t0,
///     &market_t1,
///     &val_t0,
///     &val_t1,
///     as_of_t0,
///     as_of_t1,
/// )?;
/// # let _ = attribution;
/// # Ok(())
/// # }
/// ```
pub fn attribute_pnl_metrics_based(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    val_t0: &ValuationResult,
    val_t1: &ValuationResult,
    as_of_t0: Date,
    as_of_t1: Date,
) -> Result<PnlAttribution> {
    validate_attribution_period(as_of_t0, as_of_t1)?;

    // Total P&L — use date-specific FX to stay consistent with factor decomposition
    let total_pnl = compute_pnl_with_fx(
        val_t0.value,
        val_t1.value,
        val_t1.value.currency(),
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
    )?;

    let mut attribution = init_attribution(
        total_pnl,
        instrument.id(),
        as_of_t0,
        as_of_t1,
        AttributionMethod::MetricsBased,
        None,
    );

    // W56: track whether any non-finite factor P&L was encountered. When true
    // we set `attribution.result_invalid = true` before returning so that
    // `residual_within_tolerance` correctly refuses to report a clean result.
    let mut non_finite_detected = false;

    // Total-return basis : `PnlAttribution::new` captured the
    // raw MTM (`val_t1 − val_t0`) in `mark_to_market_pnl`; add cashflows paid
    // inside [T₀, T₁) so `total_pnl` matches the total-return convention the
    // carry metrics use (Theta / CarryTotal include period cashflows — see
    // `valuations::metrics::sensitivities::theta`). Without this, a coupon
    // payment date produced `residual ≈ −coupon` and a spurious tolerance
    // breach. Mirrors `apply_total_return_carry` on the reprice-based paths;
    // carry itself is NOT adjusted here because the metrics already carry the
    // cashflow component.
    match collect_cashflows_in_period(
        instrument.as_ref(),
        market_t0,
        as_of_t0,
        as_of_t1,
        val_t1.value.currency(),
    ) {
        Ok(coupon_income) if coupon_income.abs() > 0.0 && coupon_income.is_finite() => {
            attribution.total_pnl = attribution
                .total_pnl
                .checked_add(Money::new(coupon_income, val_t1.value.currency()))?;
        }
        Ok(_) => {}
        Err(e) => {
            attribution.meta.notes.push(format!(
                "Total-return adjustment unavailable (cashflow collection failed: {e}); \
                 total_pnl is MTM-only for this period"
            ));
        }
    }

    // Extract time period in days
    let time_period_days = (as_of_t1 - as_of_t0).whole_days() as f64;

    // ─── Preamble: compute market-shift averages ONCE ──────────────────────
    //
    // Each `measure_*_shift` helper is pure: same inputs → same output, so
    // computing the per-factor averages once up-front and threading them
    // through the per-factor blocks keeps the computation deterministic and
    // avoids the former pattern of redundant second loops over the same
    // curves.
    //
    // Iteration order is identical to the previous per-block loops:
    //   - discount_curves / credit_curves / spot_ids in the order returned
    //     by `market_deps.curve_dependencies()` / `market_deps.spot_ids`
    //     (preserve the existing HashMap/Vec iteration order — do NOT sort).
    //   - FX exposure / vol surface: single-valued, no ordering concern.
    //
    // A failed (Err) shift measurement skips that curve from the average,
    // matching the prior behavior.
    let market_deps = instrument.market_dependencies()?;

    // All rates curves the instrument depends on: discount AND
    // forward/projection (multi-curve swaps carry a joint
    // discount+forward DV01, and basis moves require measuring both
    // families). Order: discount first, then forward — deterministic.
    let rates_curve_ids: Vec<CurveId> = {
        let curves = market_deps.curve_dependencies();
        curves
            .discount_curves
            .iter()
            .chain(curves.forward_curves.iter())
            .cloned()
            .collect()
    };

    let (avg_rate_shift_bp, rate_curves_measured): (Option<f64>, usize) = {
        let mut total = 0.0;
        let mut count = 0usize;
        for curve_id in &rates_curve_ids {
            if let Some(shift) =
                measure_rate_curve_shift_bp(curve_id.as_str(), market_t0, market_t1)
            {
                total += shift;
                count += 1;
            }
        }
        if count > 0 {
            (Some(total / count as f64), count)
        } else {
            (None, 0)
        }
    };

    let (avg_credit_shift_bp, credit_curves_measured): (Option<f64>, usize) = {
        let mut total = 0.0;
        let mut count = 0usize;
        for curve_id in &market_deps.curve_dependencies().credit_curves {
            if let Ok(shift) = measure_credit_curve_shift(
                curve_id.as_str(),
                market_t0,
                market_t1,
                TenorSamplingMethod::Standard,
            ) {
                total += shift;
                count += 1;
            }
        }
        if count > 0 {
            (Some(total / count as f64), count)
        } else {
            (None, 0)
        }
    };

    let avg_vol_shift_abs: Option<f64> = market_deps
        .equity_dependencies()
        .vol_surface_id
        .as_ref()
        .and_then(|surface_id| {
            measure_vol_surface_shift(surface_id.as_str(), market_t0, market_t1, None, None).ok()
        });

    let fx_shift_pct: Option<f64> = instrument.fx_exposure().and_then(|(base_ccy, quote_ccy)| {
        measure_fx_shift(
            base_ccy, quote_ccy, market_t0, market_t1, as_of_t0, as_of_t1,
        )
        .ok()
    });

    let avg_spot_shift_pct: Option<f64> = {
        let mut total = 0.0;
        let mut count = 0usize;
        for spot_id in &market_deps.spot_ids {
            if let Ok(shift) = measure_scalar_shift(spot_id, market_t0, market_t1) {
                total += shift;
                count += 1;
            }
        }
        if count > 0 {
            Some(total / count as f64)
        } else {
            None
        }
    };

    // 1. Carry attribution (Theta / Carry decomposition)
    //
    // METRIC DEFINITION:
    // - Theta: Dollar P&L per day ($ / day)
    // - Formula: Theta × Δt (where Δt is time period in days)
    // - Carry decomposition metrics, when present, are scaled over the same horizon.
    let ccy = val_t1.value.currency();

    // Theta / CarryTotal / CouponIncome / PullToPar / RollDown / FundingCost
    // are PERIOD TOTALS over the producer's `theta_period` (default 1D),
    // capped at expiry. When the producer stamped the realized horizon
    // (`theta_period_days`), normalize by it before rescaling to the
    // attribution window; otherwise assume the 1D default (
    // multiplying a 1M carry total by the window's day count double-scales).
    let theta_horizon_days = val_t0
        .measures
        .get(MetricId::ThetaPeriodDays.as_str())
        .copied()
        .filter(|d| d.is_finite() && *d > 0.0);
    let carry_scale = match theta_horizon_days {
        Some(horizon) => time_period_days / horizon,
        None => time_period_days,
    };
    if let Some(horizon) = theta_horizon_days {
        if (horizon - 1.0).abs() > 1e-9 {
            attribution.meta.notes.push(format!(
                "Carry metrics normalized from a {horizon}-day producer horizon \
                 (theta_period override) to the {time_period_days}-day attribution window"
            ));
        }
    }

    if let Some(carry_total) = val_t0.measures.get(MetricId::CarryTotal.as_str()) {
        attribution.carry = factor_money_or_invalid(
            carry_total * carry_scale,
            ccy,
            "carry total",
            &mut attribution.meta.notes,
            &mut non_finite_detected,
        );

        let get_scaled =
            |id: MetricId, notes: &mut Vec<String>, flag: &mut bool| -> Option<Money> {
                val_t0.measures.get(id.as_str()).map(|value| {
                    factor_money_or_invalid(value * carry_scale, ccy, id.as_str(), notes, flag)
                })
            };

        attribution.carry_detail = Some(CarryDetail {
            total: attribution.carry,
            coupon_income: get_scaled(
                MetricId::CouponIncome,
                &mut attribution.meta.notes,
                &mut non_finite_detected,
            )
            .map(SourceLine::scalar),
            pull_to_par: get_scaled(
                MetricId::PullToPar,
                &mut attribution.meta.notes,
                &mut non_finite_detected,
            ),
            roll_down: get_scaled(
                MetricId::RollDown,
                &mut attribution.meta.notes,
                &mut non_finite_detected,
            )
            .map(SourceLine::scalar),
            funding_cost: get_scaled(
                MetricId::FundingCost,
                &mut attribution.meta.notes,
                &mut non_finite_detected,
            ),
        });
    } else if let Some(theta) = val_t0.measures.get(MetricId::Theta.as_str()) {
        let carry_amount = theta * carry_scale;
        attribution.carry = factor_money_or_invalid(
            carry_amount,
            ccy,
            "carry/theta",
            &mut attribution.meta.notes,
            &mut non_finite_detected,
        );
        attribution.carry_detail = Some(CarryDetail {
            total: attribution.carry,
            coupon_income: None,
            pull_to_par: None,
            roll_down: Some(SourceLine::scalar(attribution.carry)),
            funding_cost: None,
        });
    } else {
        note_warning(
            &mut attribution,
            "Metrics-based carry attribution skipped: neither CarryTotal nor Theta metric was present; carry P&L set to zero",
            instrument.id(),
            "carry",
        );
    }

    // 2. Rates curves attribution (DV01)
    //
    // METRIC DEFINITION:
    // - DV01: Dollar value of 1 basis point ($ / bp)
    // - BucketedDv01: Per-curve / per-tenor DV01 sensitivities
    // - Formula: PnL = Σ(DV01_i × Shift_i) for each curve/tenor i
    //
    // Accuracy ladder (best first):
    //   (a) key-rate aware: Σ_curve Σ_tenor DV01_{curve,tenor} × Δr_{curve,tenor}.
    //       Correct for non-parallel (steepener / twist) curve moves.
    //   (b) per-curve bucketed: Σ_curve DV01_curve × avg(Δr_curve). Correct for
    //       cross-curve basis but assumes each curve moved in parallel.
    //   (c) aggregate: DV01_total × avg(Δr). Coarsest.

    let curve_ids = &rates_curve_ids;
    // (a) per-tenor (key-rate) DV01 — the most accurate input.
    let keyrate_dv01 = extract_keyrate_dv01_per_curve(&val_t0.measures, curve_ids);
    // (b) per-curve total DV01 — fallback when no per-tenor series exist.
    let bucketed_dv01 = extract_bucketed_dv01_per_curve(&val_t0.measures, curve_ids);

    let has_keyrate = !keyrate_dv01.is_empty();
    let has_bucketed = !bucketed_dv01.is_empty();
    let mut rates_pnl = 0.0;
    // Average rate shift used for the rates convexity / large-move blocks.
    // - Key-rate / bucketed branches: average only over curves with data.
    // - Fallback branch: preamble average over all rates curves with a
    //   measurable shift.
    let mut convexity_avg_shift_bp: Option<f64> = None;

    if has_keyrate {
        // KEY-RATE AWARE: pair per-tenor DV01 with the per-tenor curve shift.
        // A steepener (+bp short / −bp long) is now attributed correctly
        // instead of collapsing to an average-shift × parallel-DV01 product.
        //
        // Note: curves WITHOUT per-tenor data fall down the ladder to their
        // per-curve bucketed DV01 (when present) instead of being silently
        // dropped — mixed-coverage books previously sent those curves' P&L to
        // residual with no note.
        let mut rates_acc = NeumaierAccumulator::new();
        let mut shift_acc = NeumaierAccumulator::new();
        let mut shift_terms = 0usize;
        let mut curves_with_data = 0usize;
        let mut curves_via_fallback: Vec<String> = Vec::new();
        for curve_id in curve_ids {
            let Some(buckets) = keyrate_dv01.get(curve_id) else {
                // Per-curve fallback for mixed coverage.
                if let Some(&dv01_for_curve) = bucketed_dv01.get(curve_id) {
                    if let Some(shift) =
                        measure_rate_curve_shift_bp(curve_id.as_str(), market_t0, market_t1)
                    {
                        rates_acc.add(dv01_for_curve * shift);
                        shift_acc.add(shift);
                        shift_terms += 1;
                        curves_via_fallback.push(curve_id.as_str().to_string());
                    }
                }
                continue;
            };
            let tenors: Vec<f64> = buckets.iter().map(|(t, _)| *t).collect();
            let Some(shifts) =
                measure_per_tenor_rate_shift(curve_id.as_str(), market_t0, market_t1, &tenors)
            else {
                continue;
            };
            for ((_, dv01), shift) in buckets.iter().zip(shifts.iter()) {
                rates_acc.add(dv01 * shift);
                shift_acc.add(*shift);
                shift_terms += 1;
            }
            curves_with_data += 1;
        }
        rates_pnl = rates_acc.total();
        attribution.rates_curves_pnl = factor_money_or_invalid(
            rates_pnl,
            val_t1.value.currency(),
            "rates curves P&L (key-rate)",
            &mut attribution.meta.notes,
            &mut non_finite_detected,
        );

        if shift_terms > 0 {
            // Mean per-tenor shift across all (curve, tenor) cells with data —
            // used only as the scalar input to the coarse convexity block.
            convexity_avg_shift_bp = Some(shift_acc.total() / shift_terms as f64);
        }
        if curves_with_data > 0 {
            attribution.meta.notes.push(format!(
                "Rates attribution computed using key-rate (per-tenor) DV01 across {} curve(s); \
                 non-parallel curve moves are attributed per tenor",
                curves_with_data
            ));
        }
        if !curves_via_fallback.is_empty() {
            attribution.meta.notes.push(format!(
                "Rates curves without per-tenor DV01 attributed via per-curve bucketed DV01 \
                 (parallel-move assumption): {}",
                curves_via_fallback.join(", ")
            ));
        }
    } else if has_bucketed {
        // PER-CURVE BUCKETED: sum per-curve contributions. Each curve is still
        // assumed to move in parallel (no per-tenor series available).
        let mut total_shift = 0.0;
        let mut curves_with_data = 0usize;
        for curve_id in curve_ids {
            if let Some(&dv01_for_curve) = bucketed_dv01.get(curve_id) {
                if let Some(shift) =
                    measure_rate_curve_shift_bp(curve_id.as_str(), market_t0, market_t1)
                {
                    rates_pnl += dv01_for_curve * shift;
                    total_shift += shift;
                    curves_with_data += 1;
                }
            }
        }

        attribution.rates_curves_pnl = factor_money_or_invalid(
            rates_pnl,
            val_t1.value.currency(),
            "rates curves P&L (bucketed)",
            &mut attribution.meta.notes,
            &mut non_finite_detected,
        );

        if curves_with_data > 0 {
            convexity_avg_shift_bp = Some(total_shift / curves_with_data as f64);
            attribution.meta.notes.push(format!(
                "Rates attribution computed using per-curve bucketed DV01 across {} curves \
                 (each curve assumed to move in parallel); provide per-tenor BucketedDv01 \
                 series for key-rate-aware attribution of non-parallel moves",
                curves_with_data
            ));
        }
    } else if let Some(dv01) = val_t0.measures.get(MetricId::Dv01.as_str()) {
        // Fallback: use aggregate DV01 with the preamble's average shift.
        let avg_shift = if let Some(avg_shift) = avg_rate_shift_bp {
            avg_shift
        } else {
            note_warning(
                &mut attribution,
                "Rates attribution has DV01 but no measurable discount-curve shift; rates P&L set to zero",
                instrument.id(),
                "rates_curves",
            );
            0.0
        };
        rates_pnl = dv01 * avg_shift;
        convexity_avg_shift_bp = avg_rate_shift_bp;

        attribution.rates_curves_pnl = factor_money_or_invalid(
            rates_pnl,
            val_t1.value.currency(),
            "rates curves P&L (aggregate dv01)",
            &mut attribution.meta.notes,
            &mut non_finite_detected,
        );

        // Add note about averaging limitation
        if rate_curves_measured > 1 {
            attribution.meta.notes.push(format!(
                "Rates attribution uses aggregate DV01 with average shift across {} curves; \
                 provide BucketedDv01 metric for more accurate per-curve attribution",
                rate_curves_measured
            ));
        }
    } else if avg_rate_shift_bp.is_some_and(|s| s.abs() > 0.0) {
        // No DV01 metric at all while the curves measurably moved: the rates
        // P&L stays zero and the move lands in the residual. Note it for
        // symmetric diagnosability with the carry block.
        note_warning(
            &mut attribution,
            "Rates attribution skipped: no Dv01/BucketedDv01 metric in the T0 valuation \
             while the rates curves moved; rates P&L set to zero (move flows to residual)",
            instrument.id(),
            "rates_curves",
        );
    }

    // 2b. Rates curves convexity (second-order)
    //
    // UNIT CONTRACT:
    // - `measure_discount_curve_shift` returns a shift in BASIS POINTS.
    // - `Convexity` / `IrConvexity` are percentage second-derivative metrics (dimensionless).
    // - P&L formula: ½ × P₀ × Convexity × (Δr_decimal)², where Δr_decimal = shift_bp / 10_000.
    //
    // LIMITATION: Assumes parallel/average shifts and small moves; for large or non-parallel
    // moves, use bump-and-reprice curve gamma when available.
    //
    // TWIST GUARD (audit rec #6): if the signed average is much smaller than
    // the L1 (absolute) average, the curves were twisted (e.g. short-end +50bp,
    // long-end −50bp averages to ~0). In that regime the scalar convexity term
    // `½·γ·avg²` collapses to ≈0 even though the true second-order
    // contribution `½·Δrᵀ·H·Δr` is non-trivial. Emit a note so the consumer
    // knows the convexity number is *not* a real upper bound.
    let avg_rate_abs_shift_bp: Option<f64> = {
        let mut total = 0.0;
        let mut count = 0usize;
        for curve_id in &rates_curve_ids {
            let v = rate_curve_abs_shift_bp(curve_id.as_str(), market_t0, market_t1);
            if v > 0.0 {
                total += v;
                count += 1;
            }
        }
        if count > 0 {
            Some(total / count as f64)
        } else {
            None
        }
    };
    if let Some(avg_shift) = convexity_avg_shift_bp {
        let rc = RoundingContext::default();
        // The two convexity MetricIds have DIFFERENT producer units and must
        // not be merged :
        //
        // - `Convexity` (bond producer) is *street convexity*:
        //   `(1/P)·d²P/dy² / 100` (Bloomberg YAS convention, golden-verified
        //   in valuations). P&L term: ½ × P₀ × Convexity × 100 × (Δy)².
        // - `IrConvexity` (IRS producer) is the *raw dollar second
        //   derivative* `d²PV/dr²` (no P normalization — a near-par swap has
        //   PV ≈ 0 but real gamma). P&L term: ½ × IrConvexity × (Δy)².
        let street_convexity = val_t0
            .measures
            .get(MetricId::Convexity.as_str())
            .filter(|&&v| !rc.is_effectively_zero(v, ZeroKind::Generic));
        let dollar_convexity = val_t0
            .measures
            .get(MetricId::IrConvexity.as_str())
            .filter(|&&v| !rc.is_effectively_zero(v, ZeroKind::Generic));

        let shift_decimal = avg_shift / 10_000.0;
        let convexity_pnl_opt = match (street_convexity, dollar_convexity) {
            (Some(&convexity), _) => {
                // Street convexity: ½ × P₀ × C × 100 × (Δy)².
                debug_assert!(
                    convexity.is_finite(),
                    "Convexity metric must be finite for P&L attribution, got {convexity}"
                );
                let p0 = val_t0.value.amount();
                Some(0.5 * p0 * convexity * 100.0 * shift_decimal * shift_decimal)
            }
            (None, Some(&ir_convexity)) => {
                // Dollar convexity: ½ × d²PV/dr² × (Δy)² — no P₀ factor.
                debug_assert!(
                    ir_convexity.is_finite(),
                    "IrConvexity metric must be finite for P&L attribution, got {ir_convexity}"
                );
                Some(0.5 * ir_convexity * shift_decimal * shift_decimal)
            }
            (None, None) => None,
        };

        if let Some(convexity_pnl) = convexity_pnl_opt {
            attribution.rates_curves_pnl = factor_money_or_invalid(
                attribution.rates_curves_pnl.amount() + convexity_pnl,
                val_t1.value.currency(),
                "rates convexity P&L",
                &mut attribution.meta.notes,
                &mut non_finite_detected,
            );
        }

        // Check for large rate moves that may exceed approximation accuracy
        if avg_shift.abs() > LARGE_RATE_MOVE_THRESHOLD_BP {
            attribution.meta.notes.push(format!(
                "Warning: Large rate move ({:.0}bp) exceeds {}bp threshold; \
                 third-order+ effects ignored, consider parallel/waterfall attribution \
                 for more accurate results",
                avg_shift.abs(),
                LARGE_RATE_MOVE_THRESHOLD_BP
            ));
        }

        // Twist-domination warning (audit rec #6).
        if let Some(abs_shift) = avg_rate_abs_shift_bp {
            if let Some(note) = twist_diagnostic_note("Rates convexity", avg_shift, abs_shift) {
                attribution.meta.notes.push(note);
                attribution
                    .meta
                    .notes
                    .push("Rates convexity: unreliable / bounds-exceeded".to_string());
            }
        }
    }

    // 3. Credit curves attribution (CS01)
    //
    // METRIC DEFINITION:
    // - Cs01 / BucketedCs01: $ per bp of credit-curve move ($ / bp).
    // - Formula: PnL = Σ_curve Σ_tenor BucketedCs01_{curve,tenor} × Δs_{curve,tenor}
    //   where Δs is the credit-curve move from `measure_credit_curve_shift` /
    //   `measure_per_tenor_credit_curve_shift`. Those measure the move in
    //   whichever basis the instrument's CS01 is defined on: a par CDS spread
    //   move for a hazard curve (CDS-family), or a zero-rate move for a
    //   discount-style credit curve (a convertible's Tsiveriotis–Zhang risky
    //   discount curve). Pairing a par-spread CS01 with a hazard-rate move would
    //   overstate credit P&L by 1/(1−R), so the move always matches the CS01.
    //
    // Accuracy ladder (best first):
    //   (a) key-rate: per-tenor BucketedCs01 × per-tenor credit-curve move —
    //       correct for non-parallel (steepener / twist) credit-curve moves.
    //   (b) aggregate: Cs01 × avg(credit-curve move). Coarser; assumes parallel.
    let credit_curve_ids = &market_deps.curve_dependencies().credit_curves;
    let keyrate_cs01 = extract_keyrate_cs01_per_curve(&val_t0.measures, credit_curve_ids);
    let mut credit_has_data = false;
    // Mean par-spread shift fed to the credit-convexity (second-order) block.
    let mut credit_convexity_avg_shift_bp: Option<f64> = None;

    if !keyrate_cs01.is_empty() {
        // KEY-RATE AWARE: pair per-tenor BucketedCs01 with the per-tenor
        // par-spread move. A credit-curve steepener is attributed per tenor
        // instead of collapsing to an average-shift × parallel-CS01 product —
        // so no twist guard / omit-on-twist workaround is needed.
        let mut credit_acc = NeumaierAccumulator::new();
        let mut shift_acc = NeumaierAccumulator::new();
        let mut shift_terms = 0usize;
        let mut curves_with_data = 0usize;
        for curve_id in credit_curve_ids {
            let Some(buckets) = keyrate_cs01.get(curve_id) else {
                continue;
            };
            let tenors: Vec<f64> = buckets.iter().map(|(t, _)| *t).collect();
            let Ok(shifts) = measure_per_tenor_credit_curve_shift(
                curve_id.as_str(),
                market_t0,
                market_t1,
                &tenors,
            ) else {
                continue;
            };
            for ((_, cs01), shift) in buckets.iter().zip(shifts.iter()) {
                credit_acc.add(cs01 * shift);
                shift_acc.add(*shift);
                shift_terms += 1;
            }
            curves_with_data += 1;
        }
        attribution.credit_curves_pnl = factor_money_or_invalid(
            credit_acc.total(),
            val_t1.value.currency(),
            "credit curves P&L (key-rate)",
            &mut attribution.meta.notes,
            &mut non_finite_detected,
        );
        credit_has_data = true;
        if shift_terms > 0 {
            credit_convexity_avg_shift_bp = Some(shift_acc.total() / shift_terms as f64);
        }
        if curves_with_data > 0 {
            attribution.meta.notes.push(format!(
                "Credit attribution computed using key-rate (per-tenor) BucketedCs01 across \
                 {} curve(s); non-parallel credit-curve moves are attributed per tenor",
                curves_with_data
            ));
        }
    } else if let Some(cs01) = val_t0.measures.get(MetricId::Cs01.as_str()) {
        // Aggregate fallback: parallel CS01 × average credit-curve move.
        let avg_shift = if let Some(avg_shift) = avg_credit_shift_bp {
            avg_shift
        } else {
            note_warning(
                &mut attribution,
                "Credit attribution has Cs01 but no measurable credit-curve shift; credit P&L set to zero",
                instrument.id(),
                "credit_curves",
            );
            0.0
        };
        attribution.credit_curves_pnl = factor_money_or_invalid(
            cs01 * avg_shift,
            val_t1.value.currency(),
            "credit curves P&L",
            &mut attribution.meta.notes,
            &mut non_finite_detected,
        );
        credit_has_data = true;
        credit_convexity_avg_shift_bp = avg_credit_shift_bp;
        if credit_curves_measured > 1 {
            attribution.meta.notes.push(format!(
                "Credit attribution uses aggregate Cs01 with average credit-curve shift across \
                 {} curves; provide BucketedCs01 for key-rate-aware attribution of \
                 non-parallel moves",
                credit_curves_measured
            ));
        }
    } else if avg_credit_shift_bp.is_some_and(|s| s.abs() > 0.0) {
        // No CS01 metric at all while the credit curves measurably moved —
        // note the silent zero (see the rates-ladder counterpart above).
        note_warning(
            &mut attribution,
            "Credit attribution skipped: no Cs01/BucketedCs01 metric in the T0 valuation \
             while the credit curves moved; credit P&L set to zero (move flows to residual)",
            instrument.id(),
            "credit_curves",
        );
    }

    // 3b. Credit curves gamma (second-order).
    //
    // UNIT CONTRACT: CsGamma is ∂²V/∂s² in $ per decimal² of *par spread*.
    //   ΔP_gamma = ½ × CsGamma × (Δs_decimal)², Δs = mean par-spread move.
    //
    // TWIST GUARD: like rates convexity, the scalar `½·γ·avg²` term collapses
    // when the credit curve is twisted (signed mean ≈ 0). Emit a note so the
    // consumer knows the gamma number is not a real upper bound. Average over
    // the same credit curves the metrics-based attribution consumed.
    let avg_credit_abs_shift_bp: Option<f64> = {
        let mut total = 0.0;
        let mut count = 0usize;
        for curve_id in &market_deps.curve_dependencies().credit_curves {
            let v = credit_curve_abs_shift_bp(curve_id.as_str(), market_t0, market_t1);
            if v > 0.0 {
                total += v;
                count += 1;
            }
        }
        if count > 0 {
            Some(total / count as f64)
        } else {
            None
        }
    };
    if credit_has_data {
        if let Some(avg_shift) = credit_convexity_avg_shift_bp {
            if let Some(cs_gamma) = val_t0.measures.get(MetricId::CsGamma.as_str()) {
                let shift_decimal = avg_shift / 10_000.0;
                let gamma_pnl = 0.5 * cs_gamma * shift_decimal * shift_decimal;
                attribution.credit_curves_pnl = factor_money_or_invalid(
                    attribution.credit_curves_pnl.amount() + gamma_pnl,
                    val_t1.value.currency(),
                    "credit gamma P&L",
                    &mut attribution.meta.notes,
                    &mut non_finite_detected,
                );
            }

            if avg_shift.abs() > LARGE_SPREAD_MOVE_THRESHOLD_BP {
                attribution.meta.notes.push(format!(
                    "Warning: Large credit spread move ({:.0}bp) exceeds {}bp threshold; \
                     consider parallel/waterfall attribution for more accurate results",
                    avg_shift.abs(),
                    LARGE_SPREAD_MOVE_THRESHOLD_BP
                ));
            }

            if let Some(abs_shift) = avg_credit_abs_shift_bp {
                if let Some(note) = twist_diagnostic_note("Credit gamma", avg_shift, abs_shift) {
                    attribution.meta.notes.push(note);
                    attribution
                        .meta
                        .notes
                        .push("Credit gamma: unreliable / bounds-exceeded".to_string());
                }
            }
        }
    }

    // 4. FX attribution (FX01 or FX Delta)
    //
    // METRIC DEFINITION:
    // - FX01: Dollar value of 1% FX rate change ($ / %)
    // - Formula: FX01 × Δfx (where Δfx is FX rate change in %)
    if let Some(fx01) = val_t0.measures.get(MetricId::Fx01.as_str()) {
        // FX01 × spot change (FX01 is typically per 1% move)
        if let Some(fx_shift) = fx_shift_pct {
            let fx_amount = fx01 * fx_shift;
            attribution.fx_pnl = factor_money_or_invalid(
                fx_amount,
                val_t1.value.currency(),
                "FX P&L",
                &mut attribution.meta.notes,
                &mut non_finite_detected,
            );
            // Fx01 is the JOINT sensitivity to a simultaneous move of all the
            // instrument's FX pairs, but the shift above is measured on the
            // single `fx_exposure()` pair — approximate when the instrument
            // declares more than one pair.
            if market_deps.fx_pairs.len() > 1 {
                attribution.meta.notes.push(format!(
                    "FX attribution pairs the joint Fx01 sensitivity with the primary \
                     FX pair's move only; the instrument declares {} FX pairs, so \
                     differential moves across pairs are approximated",
                    market_deps.fx_pairs.len()
                ));
            }
        } else {
            note_warning(
                &mut attribution,
                "FX attribution has FX01 but no measurable FX shift; FX P&L set to zero",
                instrument.id(),
                "fx",
            );
        }
    }

    // 5. Volatility attribution (Vega)
    //
    // METRIC DEFINITION:
    // - Vega: Dollar value of 1 percentage point volatility change ($ / vol point)
    // - Formula: Vega × Δσ (where Δσ is in percentage points, e.g., 1.0 for 1% vol change)
    if let Some(vega) = val_t0.measures.get(MetricId::Vega.as_str()) {
        // Vega × vol change (in percentage points). Preserves prior behavior:
        // vol PnL is only recorded when the instrument has a vol surface AND
        // the surface shift measurement succeeded (both conditions captured
        // by `avg_vol_shift_abs` being `Some`).
        if let Some(vol_shift) = avg_vol_shift_abs {
            // vol_shift is already in percentage points
            let vol_amount = vega * vol_shift;
            attribution.vol_pnl = factor_money_or_invalid(
                vol_amount,
                val_t1.value.currency(),
                "vol P&L",
                &mut attribution.meta.notes,
                &mut non_finite_detected,
            );

            // 5b. Volatility convexity (Volga - second-order)
            if let Some(volga) = val_t0.measures.get(MetricId::Volga.as_str()) {
                // Volga term: ½ × Volga × (Δσ)²
                let volga_pnl = 0.5 * volga * vol_shift * vol_shift;

                attribution.vol_pnl = factor_money_or_invalid(
                    attribution.vol_pnl.amount() + volga_pnl,
                    val_t1.value.currency(),
                    "volga P&L",
                    &mut attribution.meta.notes,
                    &mut non_finite_detected,
                );
            }

            // Check for large vol moves that may exceed approximation accuracy
            if vol_shift.abs() > LARGE_VOL_MOVE_THRESHOLD_PCT {
                attribution.meta.notes.push(format!(
                    "Warning: Large volatility move ({:.1}%) exceeds {:.1}% threshold; \
                     vol-of-vol effects ignored, consider parallel/waterfall attribution",
                    vol_shift.abs(),
                    LARGE_VOL_MOVE_THRESHOLD_PCT
                ));
            }
        } else {
            note_warning(
                &mut attribution,
                "Volatility attribution has Vega but no measurable volatility-surface shift; vol P&L set to zero",
                instrument.id(),
                "vol",
            );
        }
    }

    // 6. Market scalars: spot price Delta/Gamma attribution
    //
    // METRIC DEFINITION (see MetricId::Delta / MetricId::Gamma):
    // - Delta: dPV/dS — currency per UNIT of underlying move
    // - Gamma: d²PV/dS² — currency per (unit underlying)²
    // - Formula: PnL = Delta × ΔS + ½ × Gamma × (ΔS)², with ΔS the ABSOLUTE
    //   spot move. Multiplying by a percentage shift would mis-scale the P&L
    //   by 100/S₀ (resp. (100/S₀)²), exact only when S₀ = 100.
    //
    // Uses spot_ids from MarketDependencies to identify underlying spot prices.
    {
        let spot_ids = &market_deps.spot_ids;
        let delta_opt = val_t0.measures.get(MetricId::Delta.as_str());
        let gamma_opt = val_t0.measures.get(MetricId::Gamma.as_str());

        if let Some(&delta) = delta_opt {
            // Guard against a non-finite Delta silently corrupting attributed
            // P&L. `MetricId::Delta` is contractually dPV/dS (currency per unit
            // underlying move) — see the unit note above.
            debug_assert!(
                delta.is_finite(),
                "Delta metric must be finite for P&L attribution, got {delta}"
            );

            // Note: `Delta` / `Gamma` are sensitivities to the
            // instrument's PRIMARY spot driver, not a per-spot vector. The old
            // code multiplied the single Delta by EVERY spot's move and summed
            // (~N× overstatement for multi-spot instruments) while applying
            // Gamma once to the average. Both orders are now applied once, to
            // the first declared spot with a measurable move; additional spot
            // moves are unattributed (they flow to the residual) and noted.
            let mut primary_shift: Option<f64> = None;
            let mut extra_spots: Vec<&String> = Vec::new();
            for spot_id in spot_ids {
                if let Ok(spot_abs_shift) =
                    measure_scalar_absolute_shift(spot_id, market_t0, market_t1)
                {
                    if primary_shift.is_none() {
                        primary_shift = Some(spot_abs_shift);
                    } else {
                        extra_spots.push(spot_id);
                    }
                }
            }

            if let Some(spot_shift) = primary_shift {
                let mut total_spot_pnl = delta * spot_shift;

                // Second-order: Gamma applied to the same primary spot move.
                if let Some(&gamma) = gamma_opt {
                    debug_assert!(
                        gamma.is_finite(),
                        "Gamma metric must be finite for P&L attribution, got {gamma}"
                    );
                    total_spot_pnl += 0.5 * gamma * spot_shift * spot_shift;
                }

                attribution.market_scalars_pnl = factor_money_or_invalid(
                    total_spot_pnl,
                    val_t1.value.currency(),
                    "market scalars (delta/gamma) P&L",
                    &mut attribution.meta.notes,
                    &mut non_finite_detected,
                );
            }
            if !extra_spots.is_empty() {
                attribution.meta.notes.push(format!(
                    "Spot Delta/Gamma attribution applied to the primary spot driver only; \
                     moves on additional spot ids ({}) are unattributed and flow to the \
                     residual — provide per-spot sensitivities for multi-underlying books",
                    extra_spots
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }

        // Cross-factor terms (audit rec #5).
        //
        // Same six pairs as the parallel attribution (see
        // [`crate::parallel::attribute_pnl_parallel_with_credit_model`]
        // for the economic justification of the selection): Rates×Credit,
        // Rates×Vol, Spot×Vol, Spot×Credit, FX×Vol, FX×Rates. Each multiplies a
        // mixed second-partial (`CrossGamma_X_Y` metric) by the two observed
        // moves; the result enters `cross_factor_pnl` instead of either
        // factor's univariate P&L. Pairs not listed flow into the residual
        // bucket and may be material for books loaded on inflation /
        // structured / non-standard cross-effects — for those, prefer
        // parallel/waterfall attribution.
        //
        // UNIT CONTRACT for Spot cross-gamma metrics:
        // `CrossGammaSpotVol` and `CrossGammaSpotCredit` are produced by
        // `CrossFactorCalculator` using percentage-point–normalised finite
        // differences: the spot bump denominator is `spot_bump_pct × 100`
        // (e.g. 1.0 for a 1 % bump) and the vol/credit denominator is
        // similarly in percentage-point units.  Therefore the attribution
        // below must multiply by `avg_spot_shift_pct` (percentage-point spot
        // move), `avg_vol_shift_abs` (vol points) and `avg_credit_shift_bp`
        // (basis points) — each matching its cross-gamma metric's convention.
        //
        // WARNING: Do NOT substitute `MetricId::Vanna` here as a fallback for
        // `CrossGammaSpotVol`.  `Vanna` is defined as ∂²V/(∂S_abs × ∂σ_decimal)
        // — per unit spot, per decimal vol — and differs from
        // `CrossGammaSpotVol` by a factor of S₀ / 10_000.  Using `Vanna` with
        // percentage-point moves would mis-scale the cross P&L by 10_000/S₀.
        //
        // TWIST LIMITATION: the rate/credit cross terms below multiply the
        // mixed second-partial by the *signed average* shifts
        // (`avg_rate_shift_bp`, `avg_credit_shift_bp`). For a twisted curve
        // those averages collapse toward zero, so the cross P&L is understated
        // — the same caveat the rates/credit convexity blocks already emit a
        // twist note for. Prefer parallel/waterfall attribution when curves are
        // twisted and cross-gamma materiality matters.
        let mut cross_total = 0.0;
        let mut cross_by_pair = IndexMap::new();
        let currency = val_t1.value.currency();

        if let (Some(cross_gamma), Some(rate_shift), Some(credit_shift)) = (
            val_t0
                .measures
                .get(MetricId::CrossGammaRatesCredit.as_str())
                .copied(),
            avg_rate_shift_bp,
            avg_credit_shift_bp,
        ) {
            add_cross_factor_term(
                &mut cross_by_pair,
                &mut cross_total,
                "Rates×Credit",
                cross_gamma * rate_shift * credit_shift,
                currency,
                &mut attribution.meta.notes,
                &mut non_finite_detected,
            );
        }

        if let (Some(cross_gamma), Some(rate_shift), Some(vol_shift)) = (
            val_t0
                .measures
                .get(MetricId::CrossGammaRatesVol.as_str())
                .copied(),
            avg_rate_shift_bp,
            avg_vol_shift_abs,
        ) {
            add_cross_factor_term(
                &mut cross_by_pair,
                &mut cross_total,
                "Rates×Vol",
                cross_gamma * rate_shift * vol_shift,
                currency,
                &mut attribution.meta.notes,
                &mut non_finite_detected,
            );
        }

        if let (Some(cross_gamma), Some(spot_shift), Some(vol_shift)) = (
            val_t0
                .measures
                .get(MetricId::CrossGammaSpotVol.as_str())
                .copied(),
            avg_spot_shift_pct,
            avg_vol_shift_abs,
        ) {
            add_cross_factor_term(
                &mut cross_by_pair,
                &mut cross_total,
                "Spot×Vol",
                cross_gamma * spot_shift * vol_shift,
                currency,
                &mut attribution.meta.notes,
                &mut non_finite_detected,
            );
        }

        if let (Some(cross_gamma), Some(spot_shift), Some(credit_shift)) = (
            val_t0
                .measures
                .get(MetricId::CrossGammaSpotCredit.as_str())
                .copied(),
            avg_spot_shift_pct,
            avg_credit_shift_bp,
        ) {
            add_cross_factor_term(
                &mut cross_by_pair,
                &mut cross_total,
                "Spot×Credit",
                cross_gamma * spot_shift * credit_shift,
                currency,
                &mut attribution.meta.notes,
                &mut non_finite_detected,
            );
        }

        if let (Some(cross_gamma), Some(fx_shift), Some(vol_shift)) = (
            val_t0
                .measures
                .get(MetricId::CrossGammaFxVol.as_str())
                .copied(),
            fx_shift_pct,
            avg_vol_shift_abs,
        ) {
            add_cross_factor_term(
                &mut cross_by_pair,
                &mut cross_total,
                "FX×Vol",
                cross_gamma * fx_shift * vol_shift,
                currency,
                &mut attribution.meta.notes,
                &mut non_finite_detected,
            );
        }

        if let (Some(cross_gamma), Some(fx_shift), Some(rate_shift)) = (
            val_t0
                .measures
                .get(MetricId::CrossGammaFxRates.as_str())
                .copied(),
            fx_shift_pct,
            avg_rate_shift_bp,
        ) {
            add_cross_factor_term(
                &mut cross_by_pair,
                &mut cross_total,
                "FX×Rates",
                cross_gamma * fx_shift * rate_shift,
                currency,
                &mut attribution.meta.notes,
                &mut non_finite_detected,
            );
        }

        if !cross_by_pair.is_empty() {
            attribution.cross_factor_pnl = factor_money_or_invalid(
                cross_total,
                currency,
                "cross-factor P&L total",
                &mut attribution.meta.notes,
                &mut non_finite_detected,
            );
            attribution.cross_factor_detail = Some(CrossFactorDetail {
                total: attribution.cross_factor_pnl,
                by_pair: cross_by_pair,
            });
        }
    }

    // 8. Model parameters attribution
    // Requires measuring parameter shifts from instrument at T0 vs T1
    // This needs instrument-specific parameter extraction (prepayment, default, recovery)
    // (See model_params.rs for parameter extraction infrastructure)

    // 7. Dividend attribution (accumulates into market_scalars_pnl alongside spot Delta/Gamma)
    if let Some(dividend01) = val_t0.measures.get(MetricId::Dividend01.as_str()) {
        if let Some(scalar_id) = instrument.dividend_schedule_id() {
            // Note: the `Dividend01` producers emit **$ per
            // 1bp** of absolute dividend-yield move (the central difference is
            // rescaled by `DIVIDEND_BUMP_BP`, see equity_option/convertible
            // `dividend_risk.rs`). `measure_scalar_absolute_shift` returns the
            // DECIMAL Δq, so the move must be converted to bp before
            // multiplying — the former per-unit pairing understated dividend
            // P&L by 10,000×.
            if let Ok(div_abs_shift) =
                measure_scalar_absolute_shift(scalar_id.as_str(), market_t0, market_t1)
            {
                let div_shift_bp = div_abs_shift * 10_000.0;
                let div_amount = dividend01 * div_shift_bp;
                attribution.market_scalars_pnl = factor_money_or_invalid(
                    attribution.market_scalars_pnl.amount() + div_amount,
                    val_t1.value.currency(),
                    "dividend P&L",
                    &mut attribution.meta.notes,
                    &mut non_finite_detected,
                );
            }
        }
    }

    // 9. Inflation sensitivity
    if let Some(inflation01) = val_t0.measures.get(MetricId::Inflation01.as_str()) {
        // MarketDependencies does not (yet) declare inflation curves, so the
        // average is taken over every inflation curve in the market. Sort the
        // ids so the float summation order is deterministic (the context map
        // is hash-ordered), and surface multi-curve averaging in the notes —
        // in a shared multi-instrument market, unrelated inflation curves
        // contaminate this instrument's average Δi (prior fix).
        let mut curve_ids = Vec::new();
        for curve_id in market_t1.curve_ids() {
            if market_t1.get_inflation_curve(curve_id).is_ok() {
                curve_ids.push(curve_id.clone());
            }
        }
        curve_ids.sort_unstable();

        let mut total_shift = 0.0;
        let mut curve_count = 0;

        for curve_id in &curve_ids {
            if let Ok(shift_bp) =
                measure_inflation_curve_shift(curve_id.as_str(), market_t0, market_t1)
            {
                total_shift += shift_bp;
                curve_count += 1;
            }
        }

        let avg_shift = if curve_count > 0 {
            total_shift / curve_count as f64
        } else {
            0.0
        };
        if curve_count > 1 {
            attribution.meta.notes.push(format!(
                "Inflation attribution averaged the shift across {curve_count} inflation \
                 curves found in the market (instrument-level inflation-curve dependencies \
                 are not declared); unrelated curves may contaminate the average"
            ));
        }

        // First-order: Inflation01 × Δi (Δi in basis points)
        let inflation_amount = inflation01 * avg_shift;
        attribution.inflation_curves_pnl = factor_money_or_invalid(
            inflation_amount,
            val_t1.value.currency(),
            "inflation P&L",
            &mut attribution.meta.notes,
            &mut non_finite_detected,
        );

        // Second-order: Inflation convexity (if available).
        //
        // UNIT CONTRACT: `InflationConvexity` is ∂²V/∂i² in $ per decimal² of
        // inflation rate (the `CsGamma`-style convention, NOT the dimensionless
        // `Convexity` convention). The debug assertion guards against a
        // non-finite metric silently corrupting the attributed P&L (it cannot
        // enforce units — see the unit-contract table at the top of this file).
        if let Some(inflation_convexity) =
            val_t0.measures.get(MetricId::InflationConvexity.as_str())
        {
            debug_assert!(
                inflation_convexity.is_finite(),
                "InflationConvexity metric must be finite for P&L attribution, got {inflation_convexity}"
            );
            let shift_decimal = avg_shift / 10_000.0;
            let convexity_pnl = 0.5 * inflation_convexity * shift_decimal * shift_decimal;
            attribution.inflation_curves_pnl = factor_money_or_invalid(
                attribution.inflation_curves_pnl.amount() + convexity_pnl,
                val_t1.value.currency(),
                "inflation convexity P&L",
                &mut attribution.meta.notes,
                &mut non_finite_detected,
            );

            // TWIST GUARD: emit a diagnostic note when the inflation curve is
            // twisted (signed mean shift collapses toward 0 but L1 mean is
            // non-trivial). Same shape as the rates / credit twist guards.
            let mut total_abs = 0.0;
            let mut abs_count = 0usize;
            for curve_id in &curve_ids {
                let v = inflation_curve_abs_shift_bp(curve_id.as_str(), market_t0, market_t1);
                if v > 0.0 {
                    total_abs += v;
                    abs_count += 1;
                }
            }
            if abs_count > 0 {
                let abs_avg = total_abs / abs_count as f64;
                if let Some(note) = twist_diagnostic_note("Inflation convexity", avg_shift, abs_avg)
                {
                    attribution.meta.notes.push(note);
                    attribution
                        .meta
                        .notes
                        .push("Inflation convexity: unreliable / bounds-exceeded".to_string());
                }
            }
        }
    }

    // W56: propagate the non-finite flag BEFORE finalize_attribution so that
    // compute_residual sees result_invalid = true and doesn't attempt to
    // construct a residual from a (potentially sentinel) attributed sum.
    if non_finite_detected {
        attribution.result_invalid = true;
    }

    // Metadata - use reasonable tolerances for metrics-based attribution.
    // Note: Metrics-based attribution is inherently approximate, so larger residuals are expected.
    finalize_attribution(
        &mut attribution,
        instrument.id(),
        "metrics_based",
        0,    // Metrics-based doesn't reprice
        10.0, // $10 absolute tolerance
        1.0,  // 1% relative tolerance
    );

    // Note: For tighter tolerances, consider using waterfall or parallel attribution methods

    Ok(attribution)
}

#[cfg(test)]
mod tests {
    #[allow(dead_code, unused_imports)]
    mod test_utils {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/attribution_test_utils.rs"
        ));
    }

    use super::*;
    use finstack_quant_core::config::FinstackConfig;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::money::Money;
    use indexmap::IndexMap;
    use std::sync::{Arc, OnceLock};
    use test_utils::TestInstrument;
    use time::macros::date;

    #[derive(Clone)]
    struct SpotVolTestInstrument {
        id: String,
        value: Money,
    }

    finstack_quant_valuations::impl_empty_cashflow_provider!(
        SpotVolTestInstrument,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl SpotVolTestInstrument {
        fn new(id: &str, value: Money) -> Self {
            Self {
                id: id.to_string(),
                value,
            }
        }
    }

    impl Instrument for SpotVolTestInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> finstack_quant_valuations::pricer::InstrumentType {
            finstack_quant_valuations::pricer::InstrumentType::EquityOption
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn attributes(&self) -> &finstack_quant_valuations::instruments::Attributes {
            static ATTRS: OnceLock<finstack_quant_valuations::instruments::Attributes> =
                OnceLock::new();
            ATTRS.get_or_init(finstack_quant_valuations::instruments::Attributes::default)
        }

        fn attributes_mut(&mut self) -> &mut finstack_quant_valuations::instruments::Attributes {
            unreachable!("SpotVolTestInstrument::attributes_mut should not be called")
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn market_dependencies(
            &self,
        ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
        {
            let mut deps = finstack_quant_valuations::instruments::MarketDependencies::new();
            deps.add_spot_id("TEST-SPOT");
            deps.add_vol_surface_id("TEST-VOL");
            Ok(deps)
        }

        fn base_value(&self, _market: &MarketContext, _as_of: Date) -> Result<Money> {
            Ok(self.value)
        }

        fn price_with_metrics(
            &self,
            market: &MarketContext,
            as_of: Date,
            _metrics: &[MetricId],
            _options: finstack_quant_valuations::instruments::PricingOptions,
        ) -> Result<ValuationResult> {
            Ok(ValuationResult::stamped(
                self.id(),
                as_of,
                self.value(market, as_of)?,
            ))
        }
    }

    #[test]
    fn test_metrics_based_carry_matches_theta() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);
        let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

        let instrument: Arc<dyn Instrument> = Arc::new(TestInstrument::new(
            "TEST-THETA",
            Money::new(1_000.0, Currency::USD),
        ));

        let mut measures_t0 = IndexMap::new();
        measures_t0.insert(MetricId::Theta, -5.0);

        let val_t0 = ValuationResult::stamped_with_meta(
            "TEST-THETA",
            as_of_t0,
            Money::new(1_000.0, Currency::USD),
            meta.clone(),
        )
        .with_measures(measures_t0);
        let val_t1 = ValuationResult::stamped_with_meta(
            "TEST-THETA",
            as_of_t1,
            Money::new(995.0, Currency::USD),
            meta,
        );

        let attribution = attribute_pnl_metrics_based(
            &instrument,
            &MarketContext::new(),
            &MarketContext::new(),
            &val_t0,
            &val_t1,
            as_of_t0,
            as_of_t1,
        )
        .expect("metrics-based attribution should succeed");

        assert!((attribution.carry.amount() + 5.0).abs() < 1e-9);
        assert!((attribution.total_pnl.amount() + 5.0).abs() < 1e-9);
        assert!(attribution.residual_within_tolerance(0.01, 0.01));
    }

    #[test]
    fn metrics_based_missing_carry_metric_adds_note() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);
        let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

        let instrument: Arc<dyn Instrument> = Arc::new(TestInstrument::new(
            "TEST-MISSING-CARRY",
            Money::new(1_000.0, Currency::USD),
        ));
        let val_t0 = ValuationResult::stamped_with_meta(
            "TEST-MISSING-CARRY",
            as_of_t0,
            Money::new(1_000.0, Currency::USD),
            meta.clone(),
        );
        let val_t1 = ValuationResult::stamped_with_meta(
            "TEST-MISSING-CARRY",
            as_of_t1,
            Money::new(1_000.0, Currency::USD),
            meta,
        );

        let attribution = attribute_pnl_metrics_based(
            &instrument,
            &MarketContext::new(),
            &MarketContext::new(),
            &val_t0,
            &val_t1,
            as_of_t0,
            as_of_t1,
        )
        .expect("metrics-based attribution should succeed");

        assert_eq!(attribution.carry.amount(), 0.0);
        assert!(
            attribution
                .meta
                .notes
                .iter()
                .any(|note| note.contains("neither CarryTotal nor Theta")),
            "missing carry inputs should be visible in notes: {:?}",
            attribution.meta.notes
        );
    }

    #[test]
    fn test_metrics_based_carry_decomposition_populates_detail_fields() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);
        let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

        let instrument: Arc<dyn Instrument> = Arc::new(TestInstrument::new(
            "TEST-CARRY-DECOMP",
            Money::new(100_000.0, Currency::USD),
        ));

        let mut measures_t0 = IndexMap::new();
        measures_t0.insert(MetricId::Theta, -5.0);
        measures_t0.insert(MetricId::CarryTotal, -4.5);
        measures_t0.insert(MetricId::CouponIncome, 13.7);
        measures_t0.insert(MetricId::PullToPar, -8.2);
        measures_t0.insert(MetricId::RollDown, -10.0);
        measures_t0.insert(MetricId::FundingCost, 0.0);

        let val_t0 = ValuationResult::stamped_with_meta(
            "TEST-CARRY-DECOMP",
            as_of_t0,
            Money::new(100_000.0, Currency::USD),
            meta.clone(),
        )
        .with_measures(measures_t0);
        let val_t1 = ValuationResult::stamped_with_meta(
            "TEST-CARRY-DECOMP",
            as_of_t1,
            Money::new(99_995.5, Currency::USD),
            meta,
        );

        let attribution = attribute_pnl_metrics_based(
            &instrument,
            &MarketContext::new(),
            &MarketContext::new(),
            &val_t0,
            &val_t1,
            as_of_t0,
            as_of_t1,
        )
        .expect("metrics-based attribution should succeed");

        let detail = attribution
            .carry_detail
            .expect("carry_detail should be populated");
        assert_eq!(attribution.carry.amount(), -4.5);
        assert_eq!(
            detail
                .coupon_income
                .as_ref()
                .expect("coupon income")
                .total
                .amount(),
            13.7
        );
        assert_eq!(detail.pull_to_par.expect("pull to par").amount(), -8.2);
        assert_eq!(
            detail.roll_down.as_ref().expect("roll down").total.amount(),
            -10.0
        );
        assert_eq!(detail.funding_cost.expect("funding cost").amount(), 0.0);

        // Partition check: populated sub-lines should sum to total.
        let comp = detail
            .coupon_income
            .as_ref()
            .map(|l| l.total.amount())
            .unwrap_or(0.0)
            + detail.pull_to_par.map(|m| m.amount()).unwrap_or(0.0)
            + detail
                .roll_down
                .as_ref()
                .map(|l| l.total.amount())
                .unwrap_or(0.0)
            - detail.funding_cost.map(|m| m.amount()).unwrap_or(0.0);
        assert!(
            (comp - detail.total.amount()).abs() < 1e-6,
            "carry lines should partition total: {comp} vs {}",
            detail.total.amount()
        );
    }

    #[test]
    fn test_metrics_based_rates_bucketed_dv01() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);
        let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

        let instrument: Arc<dyn Instrument> = Arc::new(
            TestInstrument::new("TEST-RATES", Money::new(100_000.0, Currency::USD))
                .with_discount_curves(&["USD-OIS"]),
        );

        let market_t0 = MarketContext::new().insert(make_flat_curve("USD-OIS", as_of_t0, 0.02));
        let market_t1 = MarketContext::new().insert(make_flat_curve("USD-OIS", as_of_t1, 0.0201));

        let mut measures_t0 = IndexMap::new();
        measures_t0.insert(MetricId::custom("bucketed_dv01::USD-OIS"), -400.0);

        let val_t0 = ValuationResult::stamped_with_meta(
            "TEST-RATES",
            as_of_t0,
            Money::new(100_000.0, Currency::USD),
            meta.clone(),
        )
        .with_measures(measures_t0);
        let val_t1 = ValuationResult::stamped_with_meta(
            "TEST-RATES",
            as_of_t1,
            Money::new(99_600.0, Currency::USD),
            meta,
        );

        let attribution = attribute_pnl_metrics_based(
            &instrument,
            &market_t0,
            &market_t1,
            &val_t0,
            &val_t1,
            as_of_t0,
            as_of_t1,
        )
        .expect("metrics-based attribution should succeed");

        assert!((attribution.rates_curves_pnl.amount() + 400.0).abs() < 1e-6);
        assert!(attribution.residual_within_tolerance(0.1, 1.0));
    }

    #[test]
    fn test_metric_id_new_variants() {
        // Test that new MetricId variants exist and serialize correctly
        assert_eq!(MetricId::IrConvexity.as_str(), "ir_convexity");
        assert_eq!(MetricId::CsGamma.as_str(), "cs_gamma");
        assert_eq!(MetricId::InflationConvexity.as_str(), "inflation_convexity");

        // Test that they're distinct from existing metrics
        assert_ne!(MetricId::IrConvexity.as_str(), MetricId::Convexity.as_str());
        assert_ne!(MetricId::CsGamma.as_str(), MetricId::Gamma.as_str());
    }

    #[test]
    fn test_extract_bucketed_dv01_per_curve() {
        use finstack_quant_core::types::CurveId;

        // Test with explicit per-curve keys
        let mut measures = IndexMap::new();
        measures.insert(MetricId::custom("bucketed_dv01::USD-OIS"), -100.0);
        measures.insert(MetricId::custom("bucketed_dv01::USD-SOFR"), -50.0);
        measures.insert(MetricId::custom("bucketed_dv01::EUR-OIS"), -75.0);

        let curve_ids = vec![
            CurveId::new("USD-OIS"),
            CurveId::new("USD-SOFR"),
            CurveId::new("EUR-OIS"),
        ];

        let bucketed = extract_bucketed_dv01_per_curve(&measures, &curve_ids);

        assert_eq!(bucketed.len(), 3);
        assert_eq!(bucketed.get(&CurveId::new("USD-OIS")), Some(&-100.0));
        assert_eq!(bucketed.get(&CurveId::new("USD-SOFR")), Some(&-50.0));
        assert_eq!(bucketed.get(&CurveId::new("EUR-OIS")), Some(&-75.0));
    }

    #[test]
    fn test_extract_bucketed_dv01_single_curve() {
        use finstack_quant_core::types::CurveId;

        // Test with single curve using base key
        let mut measures = IndexMap::new();
        measures.insert(MetricId::custom("bucketed_dv01"), -250.0);

        let curve_ids = vec![CurveId::new("USD-OIS")];

        let bucketed = extract_bucketed_dv01_per_curve(&measures, &curve_ids);

        assert_eq!(bucketed.len(), 1);
        assert_eq!(bucketed.get(&CurveId::new("USD-OIS")), Some(&-250.0));
    }

    #[test]
    fn test_extract_bucketed_dv01_empty() {
        use finstack_quant_core::types::CurveId;

        // Test with no bucketed metrics
        let measures = IndexMap::new();
        let curve_ids = vec![CurveId::new("USD-OIS")];

        let bucketed = extract_bucketed_dv01_per_curve(&measures, &curve_ids);

        assert_eq!(bucketed.len(), 0);
    }

    #[test]
    fn test_extract_bucketed_dv01_partial_coverage() {
        use finstack_quant_core::types::CurveId;

        // Test with some curves having bucketed metrics and others not
        let mut measures = IndexMap::new();
        measures.insert(MetricId::custom("bucketed_dv01::USD-OIS"), -100.0);
        // USD-SOFR is missing

        let curve_ids = vec![CurveId::new("USD-OIS"), CurveId::new("USD-SOFR")];

        let bucketed = extract_bucketed_dv01_per_curve(&measures, &curve_ids);

        assert_eq!(bucketed.len(), 1);
        assert_eq!(bucketed.get(&CurveId::new("USD-OIS")), Some(&-100.0));
        assert_eq!(bucketed.get(&CurveId::new("USD-SOFR")), None);
    }

    /// `Vanna` (∂²V/∂S_abs∂σ_decimal) must NOT be used as a fallback for
    /// `CrossGammaSpotVol` in attribution because their unit conventions differ
    /// by a factor of S₀ / 10_000.  When only `Vanna` is present (no
    /// `CrossGammaSpotVol`), the Spot×Vol cross P&L must be zero (goes to
    /// residual) rather than silently mis-scaled.
    #[test]
    fn test_vanna_alone_does_not_produce_spot_vol_cross_pnl() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);
        let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

        let instrument: Arc<dyn Instrument> = Arc::new(SpotVolTestInstrument::new(
            "TEST-SPOT-VOL",
            Money::new(100.0, Currency::USD),
        ));

        let surface_t0 = VolSurface::builder("TEST-VOL")
            .expiries(&[1.0])
            .strikes(&[100.0])
            .row(&[0.20])
            .build()
            .expect("test vol surface should build");
        let surface_t1 = VolSurface::builder("TEST-VOL")
            .expiries(&[1.0])
            .strikes(&[100.0])
            .row(&[0.21])
            .build()
            .expect("test vol surface should build");

        let market_t0 = MarketContext::new()
            .insert_price("TEST-SPOT", MarketScalar::Unitless(100.0))
            .insert_surface(surface_t0);
        let market_t1 = MarketContext::new()
            .insert_price("TEST-SPOT", MarketScalar::Unitless(110.0))
            .insert_surface(surface_t1);

        // Only Vanna is present — NO CrossGammaSpotVol.
        let mut measures_t0 = IndexMap::new();
        measures_t0.insert(MetricId::Vega, 2.0);
        measures_t0.insert(MetricId::Vanna, 3.0);

        let val_t0 = ValuationResult::stamped_with_meta(
            "TEST-SPOT-VOL",
            as_of_t0,
            Money::new(100.0, Currency::USD),
            meta.clone(),
        )
        .with_measures(measures_t0);
        let val_t1 = ValuationResult::stamped_with_meta(
            "TEST-SPOT-VOL",
            as_of_t1,
            Money::new(132.0, Currency::USD),
            meta,
        );

        let attribution = attribute_pnl_metrics_based(
            &instrument,
            &market_t0,
            &market_t1,
            &val_t0,
            &val_t1,
            as_of_t0,
            as_of_t1,
        )
        .expect("metrics-based attribution should succeed");

        // Vol P&L: Vega × Δσ_pct_pt = 2.0 × 1.0 = 2.0
        assert!((attribution.vol_pnl.amount() - 2.0).abs() < 1e-9);
        // Spot×Vol cross P&L must be zero: Vanna is not a valid substitute for
        // CrossGammaSpotVol (wrong unit convention).
        assert!(
            attribution.cross_factor_pnl.amount().abs() < 1e-9,
            "cross_factor_pnl should be zero when only Vanna is available (not CrossGammaSpotVol); \
             got {}",
            attribution.cross_factor_pnl.amount()
        );
        // cross_factor_detail should be None (no cross terms found).
        assert!(
            attribution.cross_factor_detail.is_none(),
            "cross_factor_detail should be None when no CrossGamma metrics are present"
        );
    }

    #[test]
    fn hw1f_cap_surface_shock_produces_metrics_based_vol_pnl() {
        use finstack_quant_core::dates::{DayCount, Tenor};
        use finstack_quant_core::market_data::bumps::{
            BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
        };
        use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
        use finstack_quant_core::types::CurveId;
        use finstack_quant_valuations::instruments::rates::cap_floor::{CapFloor, CapFloorVolType};
        use finstack_quant_valuations::instruments::PricingOptions;
        use finstack_quant_valuations::pricer::ModelKey;

        let as_of_t0 = date!(2024 - 01 - 01);
        let as_of_t1 = date!(2024 - 01 - 02);
        let mut cap = CapFloor::new_cap(
            "HW-SURFACE-CAP",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            date!(2024 - 04 - 01),
            date!(2029 - 04 - 01),
            Tenor::quarterly(),
            DayCount::Act365F,
            "USD-OIS",
            "USD-LIBOR-3M",
            "USD-CAP-VOL",
        )
        .expect("cap");
        cap.vol_type = CapFloorVolType::Normal;

        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of_t0)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (10.0, (-0.05_f64 * 10.0).exp())])
            .build()
            .expect("discount");
        let forward = ForwardCurve::builder("USD-LIBOR-3M", 0.25)
            .base_date(as_of_t0)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 0.05), (10.0, 0.05)])
            .build()
            .expect("forward");
        let surface = VolSurface::builder("USD-CAP-VOL")
            .expiries(&[0.25, 1.0, 5.0, 10.0])
            .strikes(&[0.05])
            .row(&[0.010])
            .row(&[0.010])
            .row(&[0.010])
            .row(&[0.010])
            .build()
            .expect("surface");
        let market_t0 = MarketContext::new()
            .insert(discount)
            .insert(forward)
            .insert_surface(surface);
        let market_t1 = market_t0
            .bump([MarketBump::Curve {
                id: CurveId::from("USD-CAP-VOL"),
                spec: BumpSpec {
                    mode: BumpMode::Multiplicative,
                    units: BumpUnits::Factor,
                    value: 1.10,
                    bump_type: BumpType::Parallel,
                },
            }])
            .expect("vol shock");
        let options = PricingOptions::default().with_model(ModelKey::HullWhite1F);
        let metrics = [MetricId::Vega];

        let val_t0 = cap
            .price_with_metrics(&market_t0, as_of_t0, &metrics, options.clone())
            .expect("t0 price");
        let val_t1 = cap
            .price_with_metrics(&market_t1, as_of_t1, &metrics, options)
            .expect("t1 price");
        let instrument: Arc<dyn Instrument> = Arc::new(cap);

        let attribution = attribute_pnl_metrics_based(
            &instrument,
            &market_t0,
            &market_t1,
            &val_t0,
            &val_t1,
            as_of_t0,
            as_of_t1,
        )
        .expect("attribution");

        assert!(val_t0.measures.get("vega").copied().unwrap_or(0.0) > 0.0);
        assert!(
            attribution.vol_pnl.amount().abs() > 1e-6,
            "surface-driven HW cap must produce non-zero vol P&L"
        );
    }

    /// Regression test: `CrossGammaSpotVol` (in pct-spot × vol-point units,
    /// produced by `CrossFactorCalculator`) multiplied by `avg_spot_shift_pct`
    /// and `avg_vol_shift_abs` must give the correct cross P&L.
    ///
    /// Setup:
    ///   S₀ = 100, S₁ = 110  → avg_spot_shift_pct = 10.0 (pct-pt)
    ///   σ₀ = 0.20, σ₁ = 0.21 → avg_vol_shift_abs = 1.0 (vol-pt)
    ///   CrossGammaSpotVol = 0.005 ($ per pct-pt spot per vol-pt)
    ///
    /// Expected cross P&L = 0.005 × 10.0 × 1.0 = 0.05
    #[test]
    fn test_cross_gamma_spot_vol_uses_pct_spot_move() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);
        let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

        let instrument: Arc<dyn Instrument> = Arc::new(SpotVolTestInstrument::new(
            "TEST-SPOT-VOL-CGAMMA",
            Money::new(100.0, Currency::USD),
        ));

        let surface_t0 = VolSurface::builder("TEST-VOL")
            .expiries(&[1.0])
            .strikes(&[100.0])
            .row(&[0.20])
            .build()
            .expect("test vol surface should build");
        let surface_t1 = VolSurface::builder("TEST-VOL")
            .expiries(&[1.0])
            .strikes(&[100.0])
            .row(&[0.21])
            .build()
            .expect("test vol surface should build");

        let market_t0 = MarketContext::new()
            .insert_price("TEST-SPOT", MarketScalar::Unitless(100.0))
            .insert_surface(surface_t0);
        let market_t1 = MarketContext::new()
            .insert_price("TEST-SPOT", MarketScalar::Unitless(110.0))
            .insert_surface(surface_t1);

        // CrossGammaSpotVol is explicitly present (pct-spot × vol-point units).
        // Vanna is also set to a different value to confirm it is NOT used.
        let cross_gamma_spot_vol = 0.005_f64; // $ per pct-pt spot per vol-pt
        let mut measures_t0 = IndexMap::new();
        measures_t0.insert(MetricId::Vega, 2.0);
        measures_t0.insert(MetricId::Vanna, 999.0); // must be ignored
        measures_t0.insert(MetricId::CrossGammaSpotVol, cross_gamma_spot_vol);

        let val_t0 = ValuationResult::stamped_with_meta(
            "TEST-SPOT-VOL-CGAMMA",
            as_of_t0,
            Money::new(100.0, Currency::USD),
            meta.clone(),
        )
        .with_measures(measures_t0);
        let val_t1 = ValuationResult::stamped_with_meta(
            "TEST-SPOT-VOL-CGAMMA",
            as_of_t1,
            Money::new(102.07, Currency::USD), // arbitrary end value
            meta,
        );

        let attribution = attribute_pnl_metrics_based(
            &instrument,
            &market_t0,
            &market_t1,
            &val_t0,
            &val_t1,
            as_of_t0,
            as_of_t1,
        )
        .expect("metrics-based attribution should succeed");

        // avg_spot_shift_pct = (110/100 - 1) × 100 = 10.0
        // avg_vol_shift_abs  = (0.21 - 0.20) × 100 = 1.0
        // expected cross P&L = 0.005 × 10.0 × 1.0 = 0.05
        let expected_cross_pnl = cross_gamma_spot_vol * 10.0 * 1.0;
        assert!(
            (attribution.cross_factor_pnl.amount() - expected_cross_pnl).abs() < 1e-9,
            "cross P&L should be {expected_cross_pnl} (pct-spot units); got {}",
            attribution.cross_factor_pnl.amount()
        );
        let detail = attribution
            .cross_factor_detail
            .expect("cross factor detail should be populated");
        let spot_vol_entry = detail
            .by_pair
            .get("Spot×Vol")
            .expect("Spot×Vol entry should be present");
        assert!(
            (spot_vol_entry.amount() - expected_cross_pnl).abs() < 1e-9,
            "Spot×Vol detail should be {expected_cross_pnl}; got {}",
            spot_vol_entry.amount()
        );
    }

    fn make_flat_curve(id: &str, base_date: Date, rate: f64) -> DiscountCurve {
        let mut knots = Vec::new();
        knots.push((0.0, 1.0));
        for tenor in finstack_quant_core::market_data::diff::STANDARD_TENORS {
            let discount = (-rate * tenor).exp();
            knots.push((*tenor, discount));
        }

        DiscountCurve::builder(id)
            .base_date(base_date)
            .knots(knots)
            .interp(InterpStyle::Linear)
            .build()
            .expect("flat curve construction should succeed")
    }

    /// Build a discount curve whose zero rate at each standard tenor is taken
    /// from `rates_by_tenor` (parallel to `STANDARD_TENORS`).
    fn make_curve_from_zero_rates(
        id: &str,
        base_date: Date,
        rates_by_tenor: &[f64],
    ) -> DiscountCurve {
        let mut knots = vec![(0.0, 1.0)];
        for (tenor, &rate) in finstack_quant_core::market_data::diff::STANDARD_TENORS
            .iter()
            .zip(rates_by_tenor.iter())
        {
            knots.push((*tenor, (-rate * tenor).exp()));
        }
        DiscountCurve::builder(id)
            .base_date(base_date)
            .knots(knots)
            .interp(InterpStyle::Linear)
            .build()
            .expect("per-tenor curve construction should succeed")
    }

    /// Audit item #3: when per-tenor (key-rate) `bucketed_dv01` is available the
    /// rates attribution must pair each tenor's DV01 with that tenor's realized
    /// shift. For a steepener (short tenors down, long tenors up) the signed
    /// average shift is ~0; an average-shift × parallel-DV01 product would
    /// report ~0 rates P&L, but the key-rate-aware sum is materially non-zero.
    #[test]
    fn test_metrics_based_rates_keyrate_aware_for_steepener() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);
        let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

        let instrument: Arc<dyn Instrument> = Arc::new(
            TestInstrument::new("TEST-KEYRATE", Money::new(100_000.0, Currency::USD))
                .with_discount_curves(&["USD-OIS"]),
        );

        // T0 flat at 3%. T1 steepener: short tenors −10bp, long tenors +10bp,
        // arranged so the average over the 9 standard tenors is ~0.
        let t0_rates = [0.03_f64; 9];
        let t1_rates = [
            0.029, 0.029, 0.029, 0.0295, 0.030, 0.0305, 0.031, 0.031, 0.031,
        ];
        let market_t0 =
            MarketContext::new().insert(make_curve_from_zero_rates("USD-OIS", as_of_t0, &t0_rates));
        let market_t1 =
            MarketContext::new().insert(make_curve_from_zero_rates("USD-OIS", as_of_t1, &t1_rates));

        // Per-tenor key-rate DV01: concentrated at the LONG end (10y/30y),
        // so the steepener's long-end rise dominates the attributed P&L.
        let mut measures_t0 = IndexMap::new();
        for (label, dv01) in [
            ("3m", -1.0),
            ("6m", -1.0),
            ("1y", -2.0),
            ("2y", -3.0),
            ("3y", -4.0),
            ("5y", -6.0),
            ("7y", -8.0),
            ("10y", -40.0),
            ("30y", -120.0),
        ] {
            measures_t0.insert(
                MetricId::custom(format!("bucketed_dv01::USD-OIS::{label}")),
                dv01,
            );
        }

        let val_t0 = ValuationResult::stamped_with_meta(
            "TEST-KEYRATE",
            as_of_t0,
            Money::new(100_000.0, Currency::USD),
            meta.clone(),
        )
        .with_measures(measures_t0);
        let val_t1 = ValuationResult::stamped_with_meta(
            "TEST-KEYRATE",
            as_of_t1,
            Money::new(99_000.0, Currency::USD),
            meta,
        );

        let attribution = attribute_pnl_metrics_based(
            &instrument,
            &market_t0,
            &market_t1,
            &val_t0,
            &val_t1,
            as_of_t0,
            as_of_t1,
        )
        .expect("metrics-based attribution should succeed");

        // Long-end DV01 (−40, −120) paired with the long-end +10bp rise gives a
        // large negative rates P&L; the short-end −10bp moves partly offset it.
        // The key-rate-aware total is materially non-zero — NOT the ~0 an
        // average-shift attribution would have produced.
        let rates_pnl = attribution.rates_curves_pnl.amount();
        assert!(
            rates_pnl.abs() > 100.0,
            "key-rate-aware steepener attribution must be materially non-zero, got {rates_pnl}"
        );
        // A note must record that key-rate (per-tenor) DV01 was used.
        assert!(
            attribution
                .meta
                .notes
                .iter()
                .any(|n| n.contains("key-rate")),
            "a note must record key-rate attribution; notes: {:?}",
            attribution.meta.notes
        );
    }

    /// W56: a NaN/Inf factor sensitivity must produce `result_invalid = true`
    /// instead of panicking inside `Money::new`.
    ///
    /// Injects `f64::NAN` as the aggregate `Dv01` metric (the fallback path
    /// that reads `val_t0.measures["dv01"]` and computes `dv01 * avg_shift`)
    /// then asserts the attribution returns without panic and sets
    /// `result_invalid = true`.
    #[test]
    fn nan_factor_sensitivity_sets_result_invalid_instead_of_panicking() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);
        let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

        // A TestInstrument with one discount curve so a measurable rate shift
        // exists — that keeps us in the `dv01 * avg_shift` branch where a NaN
        // DV01 will flow into `Money::new`.
        let instrument: Arc<dyn Instrument> = Arc::new(
            TestInstrument::new("NAN-DV01", Money::new(100_000.0, Currency::USD))
                .with_discount_curves(&["USD-OIS"]),
        );

        let market_t0 = MarketContext::new().insert(make_flat_curve("USD-OIS", as_of_t0, 0.02));
        let market_t1 = MarketContext::new().insert(make_flat_curve("USD-OIS", as_of_t1, 0.0201));

        // Inject NaN as the Dv01 sensitivity — simulates an overflowed or
        // corrupt Greek value reaching the attribution engine.
        let mut measures_t0 = IndexMap::new();
        measures_t0.insert(MetricId::Dv01, f64::NAN);

        let val_t0 = ValuationResult::stamped_with_meta(
            "NAN-DV01",
            as_of_t0,
            Money::new(100_000.0, Currency::USD),
            meta.clone(),
        )
        .with_measures(measures_t0);
        let val_t1 = ValuationResult::stamped_with_meta(
            "NAN-DV01",
            as_of_t1,
            Money::new(99_600.0, Currency::USD),
            meta,
        );

        // Must NOT panic; must return Ok(_) with result_invalid = true.
        let attribution = attribute_pnl_metrics_based(
            &instrument,
            &market_t0,
            &market_t1,
            &val_t0,
            &val_t1,
            as_of_t0,
            as_of_t1,
        )
        .expect("attribution must not return Err on NaN sensitivity");

        assert!(
            attribution.result_invalid,
            "result_invalid must be true when a NaN factor sensitivity is detected; \
             got result_invalid = false"
        );

        // Residual should be a finite sentinel (zero), not NaN/Inf.
        assert!(
            attribution.residual.amount().is_finite(),
            "residual must be finite (sentinel zero) when result_invalid; got {}",
            attribution.residual.amount()
        );
    }

    /// W56 (Inf variant): same contract with +Inf sensitivity.
    #[test]
    fn inf_factor_sensitivity_sets_result_invalid_instead_of_panicking() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);
        let meta = finstack_quant_core::config::results_meta(&FinstackConfig::default());

        let instrument: Arc<dyn Instrument> = Arc::new(
            TestInstrument::new("INF-DV01", Money::new(100_000.0, Currency::USD))
                .with_discount_curves(&["USD-OIS"]),
        );
        let market_t0 = MarketContext::new().insert(make_flat_curve("USD-OIS", as_of_t0, 0.02));
        let market_t1 = MarketContext::new().insert(make_flat_curve("USD-OIS", as_of_t1, 0.0201));

        let mut measures_t0 = IndexMap::new();
        measures_t0.insert(MetricId::Dv01, f64::INFINITY);

        let val_t0 = ValuationResult::stamped_with_meta(
            "INF-DV01",
            as_of_t0,
            Money::new(100_000.0, Currency::USD),
            meta.clone(),
        )
        .with_measures(measures_t0);
        let val_t1 = ValuationResult::stamped_with_meta(
            "INF-DV01",
            as_of_t1,
            Money::new(99_600.0, Currency::USD),
            meta,
        );

        let attribution = attribute_pnl_metrics_based(
            &instrument,
            &market_t0,
            &market_t1,
            &val_t0,
            &val_t1,
            as_of_t0,
            as_of_t1,
        )
        .expect("attribution must not return Err on Inf sensitivity");

        assert!(
            attribution.result_invalid,
            "result_invalid must be true for Inf factor sensitivity"
        );
        assert!(
            attribution.residual.amount().is_finite(),
            "residual must be finite sentinel when result_invalid"
        );
    }
}
