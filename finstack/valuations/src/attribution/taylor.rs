//! Taylor-expansion P&L attribution.
//!
//! Decomposes P&L into risk-factor contributions using first-order sensitivities
//! computed via bump-and-reprice:
//!
//!   ΔP&L ≈ Σ DV01ᵢ × Δrateᵢ + Σ Fwd01ₖ × Δfwdₖ + Σ CS01ⱼ × Δspreadⱼ + vega × Δvol + theta
//!
//! Optionally includes second-order (gamma/convexity) terms:
//!
//!   + ½ Σ Gammaᵢ × Δrateᵢ² + ½ CsGamma × Δspread² + ½ Volga × Δvol²
//!
//! The FX-exposure factor is the exception: rather than a sensitivity × move
//! product it is isolated by repricing with the T₀ FX matrix restored (the same
//! restore-and-reprice technique the parallel methodology uses), so cross-
//! currency FX P&L is attributed instead of falling into the residual.
//!
//! Taylor does not compute market-scalar (spot/dividend/index) sensitivities;
//! any P&L from those factors remains in the residual.
//!
//! This is complementary to the waterfall (full-reval) approach: it produces a
//! factor-level explained/unexplained decomposition without sequential market
//! state construction.

use super::factors::{CurveRestoreFlags, MarketSnapshot};
use super::helpers::*;
use super::types::*;
use crate::instruments::common_impl::traits::Instrument;
use crate::metrics::{
    bump_discount_curve_parallel, bump_forward_curve_parallel, bump_surface_vol_absolute,
};
use finstack_core::dates::Date;
use finstack_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::diff::{
    measure_discount_curve_shift, measure_hazard_curve_shift, measure_vol_surface_shift,
    TenorSamplingMethod, STANDARD_TENORS,
};
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use finstack_core::Result;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Configuration for Taylor-based P&L attribution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaylorAttributionConfig {
    /// Include second-order (gamma/convexity) terms.
    #[serde(default)]
    pub include_gamma: bool,

    /// Rate bump size for DV01 computation (basis points).
    #[serde(default = "default_rate_bump_bp")]
    pub rate_bump_bp: f64,

    /// Credit spread bump size for CS01 computation (basis points).
    #[serde(default = "default_credit_bump_bp")]
    pub credit_bump_bp: f64,

    /// Vol bump size for vega computation (absolute vol points, e.g. 0.01 = 1%).
    #[serde(default = "default_vol_bump")]
    pub vol_bump: f64,
}

fn default_rate_bump_bp() -> f64 {
    1.0
}
fn default_credit_bump_bp() -> f64 {
    1.0
}
fn default_vol_bump() -> f64 {
    0.01
}

impl Default for TaylorAttributionConfig {
    fn default() -> Self {
        Self {
            include_gamma: false,
            rate_bump_bp: default_rate_bump_bp(),
            credit_bump_bp: default_credit_bump_bp(),
            vol_bump: default_vol_bump(),
        }
    }
}

fn record_taylor_factor_result(
    factor_kind: &str,
    factor_id: &CurveId,
    result: Result<TaylorFactorResult>,
    factors: &mut Vec<TaylorFactorResult>,
    total_explained: &mut f64,
    num_repricings: &mut usize,
) {
    match result {
        Ok(result) => {
            *total_explained += result.explained_pnl;
            if let Some(g) = result.gamma_pnl {
                *total_explained += g;
            }
            *num_repricings += 2;
            factors.push(result);
        }
        Err(e) => {
            tracing::warn!(
                factor_kind = factor_kind,
                curve_id = %factor_id,
                error = %e,
                "Taylor attribution: factor computation failed"
            );
        }
    }
}

/// Per-factor result from Taylor attribution.
///
/// # Unit conventions
///
/// | Factor kind | `sensitivity` unit      | `market_move` unit   |
/// |-------------|-------------------------|----------------------|
/// | Rates       | $ per basis point       | basis points         |
/// | Forward     | $ per basis point       | basis points         |
/// | Credit      | $ per basis point       | basis points         |
/// | Vol         | $ per vol point         | vol points (= 1 % of absolute vol) |
/// | FX          | $ (explained directly)  | 1.0 (dimensionless)  |
///
/// For vol factors `sensitivity` is $ per vol point and `market_move` is in
/// vol points (percentage points of absolute vol), matching the convention of
/// `measure_vol_surface_shift` which multiplies the absolute move by 100.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaylorFactorResult {
    /// Human-readable factor name (e.g. "Rates:USD-OIS").
    pub factor_name: String,
    /// First-order sensitivity (DV01, CS01, vega per vol point, etc.).
    pub sensitivity: f64,
    /// Observed market move between T0 and T1 (basis points for rates/credit,
    /// vol points for vol factors).
    pub market_move: f64,
    /// First-order explained P&L: sensitivity × move.
    pub explained_pnl: f64,
    /// Second-order (gamma) P&L if requested: ½ × gamma × move².
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gamma_pnl: Option<f64>,
}

/// Complete result of Taylor-based attribution.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaylorAttributionResult {
    /// Actual P&L (PV_T1 - PV_T0).
    pub actual_pnl: f64,
    /// Sum of all first-order (+ optional second-order) explained P&L.
    pub total_explained: f64,
    /// Unexplained residual: actual - explained.
    pub unexplained: f64,
    /// Unexplained as percentage of actual P&L.
    pub unexplained_pct: f64,
    /// Per-factor breakdown.
    pub factors: Vec<TaylorFactorResult>,
    /// Number of repricings performed (bump-and-reprice calls).
    pub num_repricings: usize,
    /// Present value at T0 (cached to avoid redundant repricing in compat layer).
    pub pv_t0: Money,
    /// Present value at T1 (cached to avoid redundant repricing in compat layer).
    pub pv_t1: Money,
}

/// Compute Taylor-based P&L attribution.
///
/// Uses bump-and-reprice at T0 to compute first-order sensitivities, then
/// multiplies by the observed market move between T0 and T1 to obtain
/// factor-level explained P&L.
///
/// # Arguments
///
/// * `instrument` - Instrument to attribute
/// * `market_t0` - Market context at T0
/// * `market_t1` - Market context at T1
/// * `as_of_t0` - Valuation date T0
/// * `as_of_t1` - Valuation date T1
/// * `config` - Taylor attribution configuration
///
/// # Returns
///
/// `TaylorAttributionResult` with per-factor decomposition and residual.
pub fn attribute_pnl_taylor(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    config: &TaylorAttributionConfig,
) -> Result<TaylorAttributionResult> {
    let pv_t0 = reprice_instrument(instrument, market_t0, as_of_t0)?;
    let pv_t1 = reprice_instrument(instrument, market_t1, as_of_t1)?;
    let actual_pnl = pv_t1.amount() - pv_t0.amount();

    let mut factors = Vec::new();
    let mut total_explained = 0.0;
    let mut num_repricings: usize = 2;

    // Rate sensitivities (parallel DV01 per discount curve)
    let market_deps = instrument.market_dependencies()?;
    let rate_results = market_deps
        .curve_dependencies()
        .discount_curves
        .par_iter()
        .map(|curve_id| {
            (
                curve_id.clone(),
                compute_rate_factor(
                    instrument, market_t0, market_t1, as_of_t0, pv_t0, curve_id, config,
                ),
            )
        })
        .collect::<Vec<_>>();
    for (curve_id, result) in rate_results {
        record_taylor_factor_result(
            "rate",
            &curve_id,
            result,
            &mut factors,
            &mut total_explained,
            &mut num_repricings,
        );
    }

    // Forward curve sensitivities (parallel bump per forward curve)
    let forward_results = market_deps
        .curve_dependencies()
        .forward_curves
        .par_iter()
        .map(|curve_id| {
            (
                curve_id.clone(),
                compute_forward_factor(
                    instrument, market_t0, market_t1, as_of_t0, pv_t0, curve_id, config,
                ),
            )
        })
        .collect::<Vec<_>>();
    for (curve_id, result) in forward_results {
        record_taylor_factor_result(
            "forward",
            &curve_id,
            result,
            &mut factors,
            &mut total_explained,
            &mut num_repricings,
        );
    }

    // Credit sensitivities (CS01 per hazard curve)
    let credit_results = market_deps
        .curve_dependencies()
        .credit_curves
        .par_iter()
        .map(|curve_id| {
            (
                curve_id.clone(),
                compute_credit_factor(
                    instrument, market_t0, market_t1, as_of_t0, pv_t0, curve_id, config,
                ),
            )
        })
        .collect::<Vec<_>>();
    for (curve_id, result) in credit_results {
        record_taylor_factor_result(
            "credit",
            &curve_id,
            result,
            &mut factors,
            &mut total_explained,
            &mut num_repricings,
        );
    }

    // Volatility sensitivity (vega)
    if let Some(ref surface_id_str) = market_deps.equity_dependencies().vol_surface_id {
        let surface_id = CurveId::new(surface_id_str.as_str());
        let result = compute_vol_factor(
            instrument,
            market_t0,
            market_t1,
            as_of_t0,
            pv_t0,
            &surface_id,
            config,
        );
        record_taylor_factor_result(
            "vol",
            &surface_id,
            result,
            &mut factors,
            &mut total_explained,
            &mut num_repricings,
        );
    }

    // FX-exposure factor: pricing impact of FX-rate changes on cross-currency
    // instruments. Only attempted when the T0 market actually carries an FX
    // matrix; otherwise there is nothing to restore and the factor is omitted
    // (single-currency instruments stay at zero FX P&L).
    if market_t0.fx().is_some() {
        match compute_fx_factor(instrument, market_t0, market_t1, as_of_t1, pv_t1) {
            Ok(result) => {
                total_explained += result.explained_pnl;
                num_repricings += 1;
                factors.push(result);
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Taylor attribution: FX factor computation failed"
                );
            }
        }
    }

    // Theta (time decay): reprice at T1 date with T0 market
    match compute_theta_factor(instrument, market_t0, as_of_t0, as_of_t1, pv_t0) {
        Ok(result) => {
            total_explained += result.explained_pnl;
            num_repricings += 1;
            factors.push(result);
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "Taylor attribution: theta factor computation failed"
            );
        }
    }

    let unexplained = actual_pnl - total_explained;
    let unexplained_pct = if actual_pnl.abs() > 1e-10 {
        (unexplained / actual_pnl) * 100.0
    } else {
        0.0
    };

    Ok(TaylorAttributionResult {
        actual_pnl,
        total_explained,
        unexplained,
        unexplained_pct,
        factors,
        num_repricings,
        pv_t0,
        pv_t1,
    })
}

/// Also produce a `PnlAttribution` compatible with the existing attribution framework.
///
/// This maps Taylor results into the standard `PnlAttribution` struct so that
/// Taylor output can be used interchangeably with parallel/waterfall results.
///
/// # Factor coverage
///
/// Taylor attribution covers **rates, credit, vol, FX-exposure and theta**.
/// [`attribute_pnl_taylor`] computes bump-and-reprice sensitivities for discount
/// curves, forward curves, hazard curves and vol surfaces, an FX-exposure factor
/// (T₀ FX matrix restored vs T₁ — mirroring the parallel methodology), and
/// theta. Each factor maps into its dedicated `PnlAttribution` bucket here, so
/// an FX-rate move on a cross-currency instrument lands in `fx_pnl` rather than
/// silently inflating `residual`.
///
/// Taylor does **not** compute market-scalar (spot / dividend / index)
/// sensitivities; for instruments whose pricing depends on those, the
/// corresponding P&L remains in `residual` (use the parallel methodology in
/// `attribution/parallel.rs` when scalar attribution is required). FX
/// *translation* into a non-native reporting currency is likewise out of scope
/// for this standalone path, which reports in the instrument's pricing currency.
pub fn attribute_pnl_taylor_standard(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    config: &TaylorAttributionConfig,
) -> Result<PnlAttribution> {
    let taylor =
        attribute_pnl_taylor(instrument, market_t0, market_t1, as_of_t0, as_of_t1, config)?;

    let total_pnl = compute_pnl_with_fx(
        taylor.pv_t0,
        taylor.pv_t1,
        taylor.pv_t1.currency(),
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
    )?;

    let ccy = total_pnl.currency();
    let mut attribution = init_attribution(
        total_pnl,
        instrument.id(),
        as_of_t0,
        as_of_t1,
        AttributionMethod::Taylor(config.clone()),
        None,
    );

    for factor in &taylor.factors {
        let pnl_amount = factor.explained_pnl + factor.gamma_pnl.unwrap_or(0.0);
        let factor_money = Money::new(pnl_amount, ccy);

        if factor.factor_name.starts_with("Rates:") || factor.factor_name.starts_with("Forward:") {
            attribution.rates_curves_pnl = Money::new(
                attribution.rates_curves_pnl.amount() + factor_money.amount(),
                ccy,
            );
        } else if factor.factor_name.starts_with("Credit:") {
            attribution.credit_curves_pnl = Money::new(
                attribution.credit_curves_pnl.amount() + factor_money.amount(),
                ccy,
            );
        } else if factor.factor_name.starts_with("Vol:") {
            attribution.vol_pnl =
                Money::new(attribution.vol_pnl.amount() + factor_money.amount(), ccy);
        } else if factor.factor_name == "Fx" {
            attribution.fx_pnl =
                Money::new(attribution.fx_pnl.amount() + factor_money.amount(), ccy);
            stamp_fx_policy(
                &mut attribution,
                ccy,
                "Taylor FX-exposure P&L (T0 FX matrix restored vs T1)",
            );
        } else if factor.factor_name == "Theta" {
            // Taylor theta already includes cashflows from compute_theta_factor.
            // Compute the PV-only portion for carry_detail.theta by subtracting
            // coupon income.
            let ci = {
                use crate::metrics::sensitivities::theta::collect_cashflows_in_period;
                let ci_val = collect_cashflows_in_period(
                    instrument.as_ref(),
                    market_t0,
                    as_of_t0,
                    as_of_t1,
                    ccy,
                )
                .unwrap_or(0.0);
                Money::new(ci_val, ccy)
            };
            let theta_only = Money::new(factor_money.amount() - ci.amount(), ccy);
            apply_total_return_carry(&mut attribution, theta_only, ci)?;
        }
    }

    finalize_attribution(
        &mut attribution,
        instrument.id(),
        "taylor",
        taylor.num_repricings,
        10.0,
        5.0,
    );
    // Report the residual consistent with the `PnlAttribution` total-return
    // total (coupon income + FX translation included), computed by
    // `finalize_attribution` above. The standalone `TaylorAttributionResult`
    // exposes a price-only `unexplained_pct` (PV₁−PV₀ basis); quoting that here
    // would disagree with `attribution.residual`, so we use the residual stats
    // that `compute_residual` just populated instead.
    attribution.meta.notes.push(format!(
        "Taylor attribution: {:.2}% residual ({} factors, {} repricings)",
        attribution.meta.residual_pct,
        taylor.factors.len(),
        taylor.num_repricings,
    ));
    attribution.meta.notes.push(
        "Taylor coverage: rates/credit/vol/FX-exposure/theta. Market-scalar \
         (spot/dividend/index) sensitivities are not computed; their P&L (if \
         any) remains in residual."
            .to_string(),
    );

    Ok(attribution)
}

// ─── Helper functions ──────────────────────────────────────────────────────

/// Measure average parallel forward rate shift between two markets (basis points).
fn measure_forward_curve_shift(
    curve_id: &CurveId,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    method: TenorSamplingMethod,
) -> Result<f64> {
    let curve_t0 = market_t0.get_forward(curve_id.as_str())?;
    let curve_t1 = market_t1.get_forward(curve_id.as_str())?;
    let tenors: &[f64] = match method {
        TenorSamplingMethod::Standard => STANDARD_TENORS,
        TenorSamplingMethod::Dynamic => {
            let knots = curve_t0.knots();
            if knots.is_empty() {
                STANDARD_TENORS
            } else {
                knots
            }
        }
        TenorSamplingMethod::Custom(ref tenors) => tenors.as_slice(),
    };
    Ok(measure_average_rate_shift(
        tenors,
        |t| curve_t0.rate(t),
        |t| curve_t1.rate(t),
    ))
}

fn measure_average_rate_shift(
    sample_points: &[f64],
    mut value_t0: impl FnMut(f64) -> f64,
    mut value_t1: impl FnMut(f64) -> f64,
) -> f64 {
    let mut total_shift = 0.0;
    let mut count = 0;

    for &t in sample_points {
        if t <= 0.0 {
            continue;
        }
        let v0 = value_t0(t);
        let v1 = value_t1(t);
        let shift = (v1 - v0) * 10_000.0;
        total_shift += shift;
        count += 1;
    }

    if count == 0 {
        return 0.0;
    }
    total_shift / count as f64
}

/// Compute rate (DV01) attribution for a single discount curve.
fn compute_rate_factor(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    pv_t0: Money,
    curve_id: &CurveId,
    config: &TaylorAttributionConfig,
) -> Result<TaylorFactorResult> {
    let bumped_up = bump_discount_curve_parallel(market_t0, curve_id, config.rate_bump_bp)?;
    let pv_up = reprice_instrument(instrument, &bumped_up, as_of_t0)?;

    let bumped_down = bump_discount_curve_parallel(market_t0, curve_id, -config.rate_bump_bp)?;
    let pv_down = reprice_instrument(instrument, &bumped_down, as_of_t0)?;

    // Central difference DV01: O(h²) accuracy
    let dv01 = (pv_up.amount() - pv_down.amount()) / (2.0 * config.rate_bump_bp);

    let rate_move_bp = measure_discount_curve_shift(
        curve_id.as_str(),
        market_t0,
        market_t1,
        TenorSamplingMethod::Standard,
    )?;

    let explained = dv01 * rate_move_bp;

    let gamma_pnl = if config.include_gamma {
        let gamma = (pv_up.amount() - 2.0 * pv_t0.amount() + pv_down.amount())
            / (config.rate_bump_bp * config.rate_bump_bp);
        Some(0.5 * gamma * rate_move_bp * rate_move_bp)
    } else {
        None
    };

    Ok(TaylorFactorResult {
        factor_name: format!("Rates:{}", curve_id),
        sensitivity: dv01,
        market_move: rate_move_bp,
        explained_pnl: explained,
        gamma_pnl,
    })
}

/// Compute forward-curve sensitivity attribution for a single forward curve.
///
/// Uses the same parallel bump convention as [`compute_rate_factor`], but applies
/// to the forward curve entry in [`MarketContext`] and measures the realized
/// move using forward rates (not discount zeros).
fn compute_forward_factor(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    pv_t0: Money,
    curve_id: &CurveId,
    config: &TaylorAttributionConfig,
) -> Result<TaylorFactorResult> {
    let bumped_up = bump_forward_curve_parallel(market_t0, curve_id, config.rate_bump_bp)?;
    let pv_up = reprice_instrument(instrument, &bumped_up, as_of_t0)?;

    let bumped_down = bump_forward_curve_parallel(market_t0, curve_id, -config.rate_bump_bp)?;
    let pv_down = reprice_instrument(instrument, &bumped_down, as_of_t0)?;

    let dv01 = (pv_up.amount() - pv_down.amount()) / (2.0 * config.rate_bump_bp);

    let rate_move_bp = measure_forward_curve_shift(
        curve_id,
        market_t0,
        market_t1,
        TenorSamplingMethod::Standard,
    )?;

    let explained = dv01 * rate_move_bp;

    let gamma_pnl = if config.include_gamma {
        let gamma = (pv_up.amount() - 2.0 * pv_t0.amount() + pv_down.amount())
            / (config.rate_bump_bp * config.rate_bump_bp);
        Some(0.5 * gamma * rate_move_bp * rate_move_bp)
    } else {
        None
    };

    Ok(TaylorFactorResult {
        factor_name: format!("Forward:{}", curve_id),
        sensitivity: dv01,
        market_move: rate_move_bp,
        explained_pnl: explained,
        gamma_pnl,
    })
}

/// Compute credit (CS01) attribution for a single hazard curve.
fn compute_credit_factor(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    pv_t0: Money,
    curve_id: &CurveId,
    config: &TaylorAttributionConfig,
) -> Result<TaylorFactorResult> {
    let bumped_up = market_t0.bump([MarketBump::Curve {
        id: curve_id.clone(),
        spec: BumpSpec::parallel_bp(config.credit_bump_bp),
    }])?;
    let pv_up = reprice_instrument(instrument, &bumped_up, as_of_t0)?;

    let bumped_down = market_t0.bump([MarketBump::Curve {
        id: curve_id.clone(),
        spec: BumpSpec::parallel_bp(-config.credit_bump_bp),
    }])?;
    let pv_down = reprice_instrument(instrument, &bumped_down, as_of_t0)?;

    // Central difference CS01: O(h²) accuracy
    let cs01 = (pv_up.amount() - pv_down.amount()) / (2.0 * config.credit_bump_bp);

    let spread_move_bp = measure_hazard_curve_shift(
        curve_id.as_str(),
        market_t0,
        market_t1,
        TenorSamplingMethod::Standard,
    )?;

    let explained = cs01 * spread_move_bp;

    let gamma_pnl = if config.include_gamma {
        let gamma = (pv_up.amount() - 2.0 * pv_t0.amount() + pv_down.amount())
            / (config.credit_bump_bp * config.credit_bump_bp);
        Some(0.5 * gamma * spread_move_bp * spread_move_bp)
    } else {
        None
    };

    Ok(TaylorFactorResult {
        factor_name: format!("Credit:{}", curve_id),
        sensitivity: cs01,
        market_move: spread_move_bp,
        explained_pnl: explained,
        gamma_pnl,
    })
}

/// Compute volatility (vega) attribution for a vol surface.
fn compute_vol_factor(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    pv_t0: Money,
    surface_id: &CurveId,
    config: &TaylorAttributionConfig,
) -> Result<TaylorFactorResult> {
    let bumped_up = bump_surface_vol_absolute(market_t0, surface_id.as_str(), config.vol_bump)?;
    let pv_up = reprice_instrument(instrument, &bumped_up, as_of_t0)?;

    let bumped_down = bump_surface_vol_absolute(market_t0, surface_id.as_str(), -config.vol_bump)?;
    let pv_down = reprice_instrument(instrument, &bumped_down, as_of_t0)?;

    // Central difference vega in $ per vol point.
    //
    // `measure_vol_surface_shift` returns the move in *percentage points* (absolute
    // move × 100). To keep units consistent we must express vega in the same
    // per-point basis: divide by `vol_bump_abs × 100` rather than `vol_bump_abs`.
    //
    //   vega_per_point [$/vol-point] = ΔPV / (2 × vol_bump_abs × 100)
    //   explained [$]               = vega_per_point × vol_move_points
    let vol_bump_points = config.vol_bump * 100.0; // convert bump to vol-point units
    let vega_per_point = (pv_up.amount() - pv_down.amount()) / (2.0 * vol_bump_points);

    // vol_move is in vol points (percentage points of absolute vol).
    let vol_move =
        measure_vol_surface_shift(surface_id.as_str(), market_t0, market_t1, None, None)?;

    let explained = vega_per_point * vol_move;

    let gamma_pnl = if config.include_gamma {
        // Volga in $ per vol-point²: use vol_bump_points consistently.
        //   volga [$/pt²] = ΔΔP / (vol_bump_points)²
        //   gamma_pnl [$] = 0.5 × volga × vol_move_points²
        let volga = (pv_up.amount() - 2.0 * pv_t0.amount() + pv_down.amount())
            / (vol_bump_points * vol_bump_points);
        Some(0.5 * volga * vol_move * vol_move)
    } else {
        None
    };

    Ok(TaylorFactorResult {
        factor_name: format!("Vol:{}", surface_id),
        sensitivity: vega_per_point,
        market_move: vol_move,
        explained_pnl: explained,
        gamma_pnl,
    })
}

/// Compute FX-exposure attribution by restoring the T0 FX matrix.
///
/// Unlike the curve/vol factors this is *not* a symmetric bump-and-reprice:
/// FX exposure is isolated the same way the parallel methodology does it
/// (see `attribution/parallel.rs`, Step 7) — reprice with the T1 market but the
/// T0 FX matrix restored, and take the differential against the T1 value. This
/// captures the pricing impact of FX-rate changes on cross-currency
/// instruments. For a single-currency instrument whose pricing does not read
/// the FX matrix this produces exactly zero.
///
/// `market_t1` is the full T1 market and `pv_t1` its repriced value.
fn compute_fx_factor(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t1: Date,
    pv_t1: Money,
) -> Result<TaylorFactorResult> {
    let fx_snapshot = MarketSnapshot::extract(market_t0, CurveRestoreFlags::FX);
    let market_with_t0_fx =
        MarketSnapshot::restore_market(market_t1, &fx_snapshot, CurveRestoreFlags::FX);
    let pv_with_t0_fx = reprice_instrument(instrument, &market_with_t0_fx, as_of_t1)?;

    // FX-exposure P&L: value with the actual T1 FX minus value with T0 FX
    // restored — i.e. the pricing impact attributable to the FX-rate move.
    let explained = pv_t1.amount() - pv_with_t0_fx.amount();

    Ok(TaylorFactorResult {
        factor_name: "Fx".to_string(),
        sensitivity: explained,
        market_move: 1.0,
        explained_pnl: explained,
        gamma_pnl: None,
    })
}

/// Compute theta (time decay + realized cashflows) by repricing at T1 date
/// with T0 market, then adding any coupon payments in the period.
fn compute_theta_factor(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    pv_t0: Money,
) -> Result<TaylorFactorResult> {
    use crate::metrics::sensitivities::theta::collect_cashflows_in_period;

    let pv_t0_at_t1 = reprice_instrument(instrument, market_t0, as_of_t1)?;
    let pv_diff = pv_t0_at_t1.amount() - pv_t0.amount();
    let days = (as_of_t1 - as_of_t0).whole_days() as f64;

    let coupon_income = collect_cashflows_in_period(
        instrument.as_ref(),
        market_t0,
        as_of_t0,
        as_of_t1,
        pv_t0.currency(),
    )
    .unwrap_or(0.0);

    let theta_pnl = pv_diff + coupon_income;
    let theta_per_day = if days.abs() > 0.0 {
        theta_pnl / days
    } else {
        // Same-day attribution: as_of_t0 == as_of_t1. Theta is undefined for
        // a zero time interval; we return 0 to avoid NaN, but warn loudly so
        // upstream date misalignment doesn't go unnoticed.
        tracing::warn!(
            ?as_of_t0,
            ?as_of_t1,
            "Same-day attribution: as_of_t0 == as_of_t1; theta is zeroed. \
             Check that the requested attribution period spans at least one day."
        );
        0.0
    };

    Ok(TaylorFactorResult {
        factor_name: "Theta".to_string(),
        sensitivity: theta_per_day,
        market_move: days,
        explained_pnl: theta_pnl,
        gamma_pnl: None,
    })
}

#[cfg(test)]
mod tests {
    #[allow(clippy::expect_used, dead_code, unused_imports)]
    mod test_utils {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/attribution_test_utils.rs"
        ));
    }

    use super::*;
    use finstack_core::currency::Currency;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::money::Money;
    use std::sync::Arc;
    use test_utils::TestInstrument;
    use time::macros::date;

    #[test]
    fn test_taylor_config_default() {
        let config = TaylorAttributionConfig::default();
        assert!(!config.include_gamma);
        assert_eq!(config.rate_bump_bp, 1.0);
        assert_eq!(config.credit_bump_bp, 1.0);
        assert_eq!(config.vol_bump, 0.01);
    }

    #[test]
    fn test_taylor_config_serde_roundtrip() {
        let config = TaylorAttributionConfig {
            include_gamma: true,
            rate_bump_bp: 0.5,
            credit_bump_bp: 2.0,
            vol_bump: 0.005,
        };

        let json = serde_json::to_string(&config).expect("serialize should succeed");
        let parsed: TaylorAttributionConfig =
            serde_json::from_str(&json).expect("deserialize should succeed");

        assert_eq!(parsed, config);
    }

    #[test]
    fn test_taylor_attribution_empty_market() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);

        let instrument: Arc<dyn Instrument> = Arc::new(TestInstrument::new(
            "TEST-001",
            Money::new(1000.0, Currency::USD),
        ));

        let market_t0 = MarketContext::new();
        let market_t1 = MarketContext::new();
        let config = TaylorAttributionConfig::default();

        let result = attribute_pnl_taylor(
            &instrument,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
            &config,
        )
        .expect("taylor attribution should succeed for simple instrument");

        // TestInstrument returns the same value regardless of market → actual_pnl ≈ 0
        assert!(result.actual_pnl.abs() < 1e-10);
    }

    #[test]
    fn test_taylor_compat_produces_pnl_attribution() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);

        let instrument: Arc<dyn Instrument> = Arc::new(TestInstrument::new(
            "TEST-001",
            Money::new(1000.0, Currency::USD),
        ));

        let market_t0 = MarketContext::new();
        let market_t1 = MarketContext::new();
        let config = TaylorAttributionConfig::default();

        let attribution = attribute_pnl_taylor_standard(
            &instrument,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
            &config,
        )
        .expect("taylor compat attribution should succeed");

        assert_eq!(attribution.meta.instrument_id, "TEST-001");
        assert!(matches!(
            attribution.meta.method,
            AttributionMethod::Taylor(_)
        ));
    }

    #[test]
    fn taylor_attribution_includes_forward_curve_factors() {
        use finstack_core::dates::DayCount;
        use finstack_core::market_data::term_structures::ForwardCurve;
        use finstack_core::types::CurveId;

        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);

        let fwd_t0 = ForwardCurve::builder(CurveId::new("TEST-FWD"), 0.25)
            .base_date(as_of_t0)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.03), (10.0, 0.03)])
            .build()
            .expect("forward curve");
        let fwd_t1 = ForwardCurve::builder(CurveId::new("TEST-FWD"), 0.25)
            .base_date(as_of_t0)
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.04), (10.0, 0.04)])
            .build()
            .expect("forward curve");

        let market_t0 = MarketContext::new().insert(fwd_t0);
        let market_t1 = MarketContext::new().insert(fwd_t1);

        let instrument: Arc<dyn Instrument> = Arc::new(
            TestInstrument::new("FWDI", Money::new(0.0, Currency::USD))
                .with_forward_curves(&["TEST-FWD"]),
        );

        let config = TaylorAttributionConfig::default();
        let result = attribute_pnl_taylor(
            &instrument,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
            &config,
        )
        .expect("taylor attribution should succeed");

        assert!(
            result
                .factors
                .iter()
                .any(|f| f.factor_name.starts_with("Forward:")),
            "expected forward curve factor, got {:?}",
            result.factors
        );
    }

    /// Cross-currency test instrument whose USD price reads the EUR/USD FX rate
    /// from the market's FX matrix. Used to verify Taylor buckets FX-exposure
    /// P&L into `fx_pnl` rather than `residual`.
    #[derive(Clone)]
    struct FxLinkedInstrument {
        id: String,
        /// EUR notional revalued in USD via the market FX rate.
        eur_notional: f64,
    }

    crate::impl_empty_cashflow_provider!(
        FxLinkedInstrument,
        crate::cashflow::builder::CashflowRepresentation::NoResidual
    );

    impl Instrument for FxLinkedInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> crate::pricer::InstrumentType {
            crate::pricer::InstrumentType::Bond
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn attributes(&self) -> &crate::instruments::common_impl::traits::Attributes {
            use std::sync::OnceLock;
            static ATTRS: OnceLock<crate::instruments::common_impl::traits::Attributes> =
                OnceLock::new();
            ATTRS.get_or_init(crate::instruments::common_impl::traits::Attributes::default)
        }

        fn attributes_mut(&mut self) -> &mut crate::instruments::common_impl::traits::Attributes {
            unreachable!("FxLinkedInstrument::attributes_mut should not be called")
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn market_dependencies(
            &self,
        ) -> finstack_core::Result<crate::instruments::common_impl::dependencies::MarketDependencies>
        {
            Ok(crate::instruments::common_impl::dependencies::MarketDependencies::new())
        }

        fn base_value(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
            // Price in USD as the EUR notional converted at the market FX rate.
            let usd = market.convert_money(
                Money::new(self.eur_notional, Currency::EUR),
                Currency::USD,
                as_of,
            )?;
            Ok(usd)
        }

        fn price_with_metrics(
            &self,
            market: &MarketContext,
            as_of: Date,
            _metrics: &[crate::metrics::MetricId],
            _options: crate::instruments::common_impl::traits::PricingOptions,
        ) -> Result<crate::results::ValuationResult> {
            Ok(crate::results::ValuationResult::stamped(
                self.id(),
                as_of,
                self.value(market, as_of)?,
            ))
        }
    }

    #[test]
    fn taylor_standard_buckets_fx_exposure_into_fx_pnl() {
        use finstack_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
        use finstack_core::Error;

        // FX provider with a deterministic EUR/USD rate.
        struct FixedFx(f64);
        impl FxProvider for FixedFx {
            fn rate(
                &self,
                from: Currency,
                to: Currency,
                _on: Date,
                _policy: FxConversionPolicy,
            ) -> Result<f64> {
                if from == to {
                    Ok(1.0)
                } else if from == Currency::EUR && to == Currency::USD {
                    Ok(self.0)
                } else if from == Currency::USD && to == Currency::EUR {
                    Ok(1.0 / self.0)
                } else {
                    Err(Error::Validation("FX rate not found".to_string()))
                }
            }
        }

        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);

        // USD-priced instrument whose value is a 1,000,000 EUR notional revalued
        // at the market EUR/USD rate. Only the FX rate moves between T0 and T1.
        let instrument: Arc<dyn Instrument> = Arc::new(FxLinkedInstrument {
            id: "FX-LINKED-001".to_string(),
            eur_notional: 1_000_000.0,
        });

        // T0: EUR/USD = 1.10, T1: EUR/USD = 1.20 (EUR appreciates).
        let market_t0 = MarketContext::new().insert_fx(FxMatrix::new(Arc::new(FixedFx(1.10))));
        let market_t1 = MarketContext::new().insert_fx(FxMatrix::new(Arc::new(FixedFx(1.20))));

        let config = TaylorAttributionConfig::default();
        let attribution = attribute_pnl_taylor_standard(
            &instrument,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
            &config,
        )
        .expect("taylor standard attribution should succeed");

        // USD P&L: 1_000_000 EUR * (1.20 - 1.10) = 100_000 USD, driven entirely
        // by the FX-rate move.
        assert_eq!(attribution.total_pnl.currency(), Currency::USD);
        assert!(
            (attribution.total_pnl.amount() - 100_000.0).abs() < 1e-6,
            "total_pnl = {}",
            attribution.total_pnl
        );

        // REGRESSION: the FX-driven P&L must land in `fx_pnl`, NOT `residual`.
        assert!(
            (attribution.fx_pnl.amount() - 100_000.0).abs() < 1e-6,
            "fx_pnl should capture the FX-exposure P&L, got {}",
            attribution.fx_pnl
        );
        assert!(
            attribution.residual.amount().abs() < 1e-6,
            "residual should be ~0 once FX P&L is bucketed, got {}",
            attribution.residual
        );

        // The standalone Taylor result should also expose an "Fx" factor.
        let taylor = attribute_pnl_taylor(
            &instrument,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
            &config,
        )
        .expect("taylor attribution should succeed");
        assert!(
            taylor.factors.iter().any(|f| f.factor_name == "Fx"),
            "expected an Fx factor, got {:?}",
            taylor.factors
        );
    }
}
