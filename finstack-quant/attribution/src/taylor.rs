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
//! Inflation curves, base-correlation curves, market scalars (spots, dividends,
//! published inflation indices, commodity price curves), and model-parameter
//! snapshots use the same restore-and-reprice isolation. The isolated P&L of
//! each family already includes that family's higher-order effects, so they
//! do not take a separate `include_gamma` term.
//!
//! This is complementary to the waterfall (full-reval) approach: it produces a
//! factor-level explained/unexplained decomposition without sequential market
//! state construction.

use super::factors::{MarketRestoreFlags, MarketSnapshot};
use super::helpers::*;
use super::metrics_based::extract_keyrate_per_curve;
use super::model_params;
use super::types::*;
use crate::policy_map::map_policy;
use crate::AttributionRequest;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::diff::{
    measure_credit_curve_shift, measure_per_tenor_credit_curve_shift, TenorSamplingMethod,
};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use finstack_quant_models::volatility::measure_vol_surface_shift;
use finstack_quant_valuations::instruments::model_params::ModelParamsSnapshot;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::bump_surface_vol_absolute;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Configuration for Taylor-based P&L attribution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
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

impl TaylorAttributionConfig {
    /// Validates configuration parameters.
    ///
    /// Rate and credit bumps are in basis points; `vol_bump` is an absolute
    /// volatility fraction (for example, `0.01` is one volatility point).
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] unless each rate and
    /// credit bump lies in `[0.01, 100]` bp and the volatility bump lies in
    /// `[1e-4, 0.20]`.
    ///
    /// The lower bounds guard the second-difference noise floor: a bump `h`
    /// far below the optimal finite-difference step makes the gamma estimate
    /// `(PV(+h) − 2·PV₀ + PV(−h))/h²` pure floating-point cancellation noise
    /// (Press et al., *Numerical Recipes*, §5.7) and can overflow the Decimal
    /// arithmetic used for repriced values.
    pub fn validate(&self) -> Result<()> {
        if self.rate_bump_bp < 0.01 || self.rate_bump_bp > 100.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Rate bump size must lie in [0.01, 100] bp (below 0.01bp the second-difference \
                 gamma is cancellation noise), got {:.6}",
                self.rate_bump_bp
            )));
        }
        if self.credit_bump_bp < 0.01 || self.credit_bump_bp > 100.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Credit bump size must lie in [0.01, 100] bp (below 0.01bp the second-difference \
                 gamma is cancellation noise), got {:.6}",
                self.credit_bump_bp
            )));
        }
        if self.vol_bump < 1e-4 || self.vol_bump > 0.20 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Volatility bump size must lie in [1e-4, 0.20] absolute vol (below 1e-4 the \
                 second-difference volga is cancellation noise), got {:.6}",
                self.vol_bump
            )));
        }
        Ok(())
    }
}

/// Record a successful Taylor factor result. `repricings` is the actual number
/// of bump-and-reprice calls the factor performed — a key-rate factor bumps
/// every bucket up and down, so it is far more than the 2 a single parallel
/// bump would cost.
///
/// On failure the factor is recorded in `notes` and `result_invalid` is set:
/// every factor routed through here is backed by a curve/surface that appears
/// in the instrument's market dependencies, so a failure means part of the
/// declared risk decomposition is silently missing and the result cannot be
/// trusted (previously the failure only reached the tracing log).
#[allow(clippy::too_many_arguments)]
fn record_taylor_factor_result(
    factor_kind: &str,
    factor_id: &CurveId,
    result: Result<TaylorFactorResult>,
    factors: &mut Vec<TaylorFactorResult>,
    total_explained: &mut finstack_quant_core::math::NeumaierAccumulator,
    num_repricings: &mut usize,
    repricings: usize,
    notes: &mut Vec<String>,
    result_invalid: &mut bool,
) {
    match result {
        Ok(result) => {
            total_explained.add(result.explained_pnl);
            if let Some(g) = result.gamma_pnl {
                total_explained.add(g);
            }
            *num_repricings += repricings;
            factors.push(result);
        }
        Err(e) => {
            tracing::warn!(
                factor_kind = factor_kind,
                curve_id = %factor_id,
                error = %e,
                "Taylor attribution: factor computation failed"
            );
            notes.push(format!(
                "Taylor {factor_kind} factor '{factor_id}' failed: {e}"
            ));
            *result_invalid = true;
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
/// | Inflation   | $ (explained directly)  | 1.0 (dimensionless)  |
/// | Correlations| $ (explained directly)  | 1.0 (dimensionless)  |
/// | Scalars     | $ (explained directly)  | 1.0 (dimensionless)  |
/// | Model params| $ (explained directly)  | 1.0 (dimensionless)  |
///
/// For vol factors `sensitivity` is $ per vol point and `market_move` is in
/// vol points (percentage points of absolute vol), matching the convention of
/// `measure_vol_surface_shift` which multiplies the absolute move by 100.
///
/// For key-rate-aware factors (rates / forward / key-rate credit) the
/// authoritative first-order number is `explained_pnl`, the per-bucket sum
/// `Σ sensitivityᵢ × moveᵢ`. The scalar `sensitivity` (total across buckets)
/// and `market_move` (average across buckets) are diagnostics: their product
/// does **not** equal `explained_pnl` for non-parallel curve moves.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub(crate) struct TaylorFactorResult {
    /// Human-readable factor name (e.g. "Rates:USD-OIS").
    pub factor_name: String,
    /// First-order sensitivity (DV01, CS01, vega per vol point, etc.). For
    /// key-rate factors this is the parallel-equivalent total across buckets —
    /// a diagnostic, not the multiplier that produced `explained_pnl`.
    pub sensitivity: f64,
    /// Observed market move between T0 and T1 (basis points for rates/credit,
    /// vol points for vol factors). For key-rate factors this is the average
    /// per-bucket move — a diagnostic, not the multiplier that produced
    /// `explained_pnl`.
    pub market_move: f64,
    /// First-order explained P&L. For key-rate factors this is the per-bucket
    /// sum `Σ sensitivityᵢ × moveᵢ` and is the authoritative number; for
    /// non-parallel moves it deliberately differs from
    /// `sensitivity × market_move`.
    pub explained_pnl: f64,
    /// Second-order (gamma) P&L if requested: ½ × γ_par × Δ̄², where γ_par is
    /// measured from a single parallel up/down reprice and Δ̄ is the
    /// sensitivity-weighted average bucket move.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gamma_pnl: Option<f64>,
}

/// Complete result of Taylor-based attribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub(crate) struct TaylorAttributionResult {
    /// Actual P&L on a total-return basis: `PV_T1 − PV_T0` plus the period
    /// coupon income captured by the theta factor. The explained factors
    /// include coupon income (theta is total-return), so `actual_pnl` uses the
    /// same basis — otherwise `unexplained` would be biased by exactly the
    /// period coupons. When theta computation fails the coupon component is
    /// unavailable and `actual_pnl` degrades to the price-only difference
    /// (recorded in `notes`).
    pub actual_pnl: f64,
    /// Sum of all first-order (+ optional second-order) explained P&L.
    pub total_explained: f64,
    /// Unexplained residual: actual - explained (both total-return basis).
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
    /// Coupon income for the theta period, captured by `compute_theta_factor`.
    /// `None` when theta computation failed; otherwise lets `attribute_pnl_taylor`
    /// split theta into PV-only and coupon components without re-collecting
    /// cashflows (fix).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theta_coupon_income: Option<f64>,
    /// Diagnostic notes accumulated during factor computation (failed factors,
    /// surface-averaged vol moves, missing T0 FX). Threaded into
    /// `PnlAttribution::meta.notes` by `attribute_pnl_taylor`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// True when a factor backed by a declared market dependency or the theta
    /// calculation failed. Part of the risk decomposition or period cash income
    /// is missing, so downstream consumers must not trust the residual split.
    #[serde(default)]
    pub result_invalid: bool,
}

#[derive(Clone, Copy)]
struct TaylorExecution {
    policy: ExecutionPolicy,
    prepared_endpoints: Option<(Money, Money)>,
}

/// Compute the detailed Taylor factor decomposition.
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
#[allow(clippy::too_many_arguments)]
fn compute_taylor_result(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    config: &TaylorAttributionConfig,
    execution: TaylorExecution,
    model_params_t0: Option<&ModelParamsSnapshot>,
) -> Result<TaylorAttributionResult> {
    config.validate()?;
    validate_attribution_period(as_of_t0, as_of_t1)?;
    let execution_policy = execution.policy;
    // Match parallel/waterfall: T₀ value uses the opening model-parameter
    // snapshot when the caller supplied one, so a param move is inside
    // `actual_pnl` rather than only the isolated factor.
    let instrument_t0 = if let Some(params) = model_params_t0 {
        model_params::with_model_params(instrument, params)?
    } else {
        Arc::clone(instrument)
    };
    let (pv_t0, pv_t1) = if let Some(endpoints) = execution.prepared_endpoints {
        endpoints
    } else {
        (
            reprice_instrument(&instrument_t0, market_t0, as_of_t0)?,
            reprice_instrument(instrument, market_t1, as_of_t1)?,
        )
    };
    // Decimal-exact difference: subtracting two large `.amount()` f64s loses
    // precision at high notionals, and `checked_sub` also rejects a currency
    // mismatch instead of silently differencing across currencies.
    let actual_pnl = pv_t1.checked_sub(pv_t0)?.amount();

    let mut factors = Vec::new();
    let mut total_explained = finstack_quant_core::math::NeumaierAccumulator::new();
    let mut num_repricings: usize = 2;
    let mut notes: Vec<String> = Vec::new();
    let mut result_invalid = false;
    // Extra parallel up/down reprice per curve factor when gamma is requested.
    let gamma_repricings = if config.include_gamma { 2 } else { 0 };

    // Rate sensitivities (parallel DV01 per discount curve)
    let market_deps = instrument.market_dependencies()?;
    let compute_rate = |curve_id: &CurveId| {
        (
            curve_id.clone(),
            compute_curve_factor(
                CurveKind::Discount,
                instrument,
                market_t0,
                market_t1,
                as_of_t0,
                pv_t0,
                curve_id,
                config,
            ),
        )
    };
    let rate_results = map_policy(
        execution_policy,
        &market_deps.curves.discount_curves,
        compute_rate,
    );
    for (curve_id, result) in rate_results {
        record_taylor_factor_result(
            "rate",
            &curve_id,
            result,
            &mut factors,
            &mut total_explained,
            &mut num_repricings,
            2 * KEY_RATE_BUCKETS_YEARS.len() + gamma_repricings,
            &mut notes,
            &mut result_invalid,
        );
    }

    // Forward curve sensitivities (parallel bump per forward curve)
    let compute_forward = |curve_id: &CurveId| {
        (
            curve_id.clone(),
            compute_curve_factor(
                CurveKind::Forward,
                instrument,
                market_t0,
                market_t1,
                as_of_t0,
                pv_t0,
                curve_id,
                config,
            ),
        )
    };
    let forward_results = map_policy(
        execution_policy,
        &market_deps.curves.forward_curves,
        compute_forward,
    );
    for (curve_id, result) in forward_results {
        record_taylor_factor_result(
            "forward",
            &curve_id,
            result,
            &mut factors,
            &mut total_explained,
            &mut num_repricings,
            2 * KEY_RATE_BUCKETS_YEARS.len() + gamma_repricings,
            &mut notes,
            &mut result_invalid,
        );
    }

    // Credit sensitivities — credit-curve move, key-rate aware.
    //
    // Hazard curves are measured in par CDS spread moves; discount-style credit
    // curves (for example convertible risky discount curves) are measured in zero
    // rate moves. `BucketedCs01` is requested once here; instruments without that
    // calculator yield no per-tenor keys and the per-curve `compute_credit_factor`
    // falls back to an aggregate CS01 times an average credit-curve move.
    let credit_curves = &market_deps.curves.credit_curves;
    let credit_keyrate = if credit_curves.is_empty() {
        None
    } else {
        instrument
            .price_with_metrics(
                market_t0,
                as_of_t0,
                &[finstack_quant_valuations::metrics::MetricId::BucketedCs01],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .ok()
            .map(|vr| extract_keyrate_per_curve(&vr.measures, credit_curves, "bucketed_cs01"))
    };
    let compute_credit = |curve_id: &CurveId| {
        let keyrate = credit_keyrate
            .as_ref()
            .and_then(|m| m.get(curve_id))
            .map(|v| v.as_slice());
        (
            curve_id.clone(),
            compute_credit_factor(CreditFactorInputs {
                instrument,
                market_t0,
                market_t1,
                as_of_t0,
                pv_t0,
                curve_id,
                config,
                keyrate,
            }),
        )
    };
    let credit_results = map_policy(execution_policy, credit_curves, compute_credit);
    for (curve_id, result) in credit_results {
        record_taylor_factor_result(
            "credit",
            &curve_id,
            result,
            &mut factors,
            &mut total_explained,
            &mut num_repricings,
            2 + gamma_repricings,
            &mut notes,
            &mut result_invalid,
        );
    }

    // Volatility sensitivities (vega) — one factor per vol-surface dependency,
    // iterated exactly like rates/credit (previously only the FIRST dependency
    // was priced and every other surface's move fell silently into residual).
    //
    // The realized vol move is measured at the instrument's own reference
    // point (expiry from `Instrument::expiry`, strike from the dependency)
    // when available; otherwise it falls back to the surface average and the
    // fallback is recorded in the notes, because an averaged move nets a
    // term-structure twist toward zero.
    let reference_expiry_years = instrument
        .expiry()
        .map(|expiry| (expiry - as_of_t0).whole_days() as f64 / 365.0)
        .filter(|t| *t > 0.0);
    for dependency in &market_deps.volatility_dependencies {
        if reference_expiry_years.is_none() || dependency.reference_strike.is_none() {
            notes.push(format!(
                "Taylor vol factor '{}': no reference expiry/strike available; \
                 vol move is surface-averaged",
                dependency.vol_surface_id
            ));
        }
    }
    let compute_vol =
        |dependency: &finstack_quant_valuations::instruments::VolatilityDependency| {
            (
                dependency.vol_surface_id.clone(),
                compute_vol_factor(
                    instrument,
                    market_t0,
                    market_t1,
                    as_of_t0,
                    pv_t0,
                    dependency,
                    reference_expiry_years,
                    config,
                ),
            )
        };
    let vol_results = map_policy(
        execution_policy,
        &market_deps.volatility_dependencies,
        compute_vol,
    );
    for (vol_surface_id, result) in vol_results {
        record_taylor_factor_result(
            "vol",
            &vol_surface_id,
            result,
            &mut factors,
            &mut total_explained,
            &mut num_repricings,
            2,
            &mut notes,
            &mut result_invalid,
        );
    }

    // FX-exposure factor: pricing impact of FX-rate changes on cross-currency
    // instruments. Attempted when EITHER market carries an FX matrix: FX
    // introduced at T1 is still an FX-rate move (restoring the T0 state clears
    // the matrix), so gating on T0 alone silently pushed that P&L into
    // residual. When neither market has FX there is nothing to restore and the
    // factor is omitted (single-currency instruments stay at zero FX P&L).
    if market_t0.fx().is_some() || market_t1.fx().is_some() {
        if market_t0.fx().is_none() {
            notes.push(
                "Taylor FX factor: T0 market has no FX matrix; FX-exposure P&L is measured \
                 against an FX-less T0 restore"
                    .to_string(),
            );
        }
        match compute_fx_factor(instrument, market_t0, market_t1, as_of_t1, pv_t1) {
            Ok(result) => {
                total_explained.add(result.explained_pnl);
                num_repricings += 1;
                factors.push(result);
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Taylor attribution: FX factor computation failed"
                );
                notes.push(format!("Taylor FX factor failed: {e}"));
            }
        }
    }

    // Restore-and-reprice families (same isolation as FX / parallel):
    // inflation curves, base correlations, and market scalars. The isolated
    // P&L already includes that family's higher-order effects.
    record_restored_family_factor(
        "inflation",
        "Inflation",
        MarketRestoreFlags::INFLATION,
        instrument,
        market_t0,
        market_t1,
        as_of_t1,
        pv_t1,
        &mut factors,
        &mut total_explained,
        &mut num_repricings,
        &mut notes,
        &mut result_invalid,
    );
    record_restored_family_factor(
        "correlations",
        "Correlations",
        MarketRestoreFlags::CORRELATION,
        instrument,
        market_t0,
        market_t1,
        as_of_t1,
        pv_t1,
        &mut factors,
        &mut total_explained,
        &mut num_repricings,
        &mut notes,
        &mut result_invalid,
    );
    record_restored_family_factor(
        "market-scalar",
        "MarketScalars",
        MarketRestoreFlags::SCALARS,
        instrument,
        market_t0,
        market_t1,
        as_of_t1,
        pv_t1,
        &mut factors,
        &mut total_explained,
        &mut num_repricings,
        &mut notes,
        &mut result_invalid,
    );

    // Model-parameter snapshot: reprice T1 market with T0 params restored.
    let params_t0 = model_params_t0
        .cloned()
        .unwrap_or_else(|| instrument.model_params_snapshot());
    if !matches!(params_t0, ModelParamsSnapshot::None) {
        record_taylor_factor_result(
            "model-parameter",
            &CurveId::new("ModelParameters"),
            compute_model_params_factor(instrument, market_t1, as_of_t1, pv_t1, &params_t0),
            &mut factors,
            &mut total_explained,
            &mut num_repricings,
            1,
            &mut notes,
            &mut result_invalid,
        );
    }

    // Theta (time decay): reprice at T1 date with T0 market. The outcome
    // also carries `coupon_income` so `attribute_pnl_taylor` can split theta
    // into PV-only and coupon components without re-collecting cashflows.
    let mut theta_coupon_income: Option<f64> = None;
    match compute_theta_factor(&instrument_t0, market_t0, as_of_t0, as_of_t1, pv_t0) {
        Ok(outcome) => {
            let ThetaFactorOutcome {
                factor: result,
                coupon_income,
            } = outcome;
            total_explained.add(result.explained_pnl);
            num_repricings += 1;
            theta_coupon_income = Some(coupon_income);
            factors.push(result);
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "Taylor attribution: theta factor computation failed"
            );
            notes.push(format!(
                "Taylor theta factor failed: {e}; actual_pnl excludes period coupon income"
            ));
            result_invalid = true;
        }
    }

    // Total-return basis: the theta factor's explained P&L includes period
    // coupon income, so the actual P&L it is reconciled against must include
    // it too — otherwise `unexplained` is biased by exactly the coupons paid.
    let actual_pnl = actual_pnl + theta_coupon_income.unwrap_or(0.0);

    let total_explained = total_explained.total();
    let unexplained = actual_pnl - total_explained;
    // Zero test via the RoundingContext money epsilon (consistent with
    // `PnlAttribution`'s zero checks) rather than a hardcoded 1e-10.
    let rounding = finstack_quant_core::config::RoundingContext::default();
    let unexplained_pct = if rounding.is_effectively_zero_money(actual_pnl, pv_t0.currency()) {
        0.0
    } else {
        (unexplained / actual_pnl) * 100.0
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
        theta_coupon_income,
        notes,
        result_invalid,
    })
}

/// Compute Taylor-based P&L attribution.
///
/// This maps Taylor factors into the standard `PnlAttribution` struct so Taylor
/// output can be used interchangeably with parallel/waterfall results.
///
/// # Factor coverage
///
/// Taylor attribution covers **rates, credit, vol, FX-exposure, inflation,
/// correlations, market scalars, model parameters, and theta**.
/// [`attribute_pnl_taylor`] computes bump-and-reprice sensitivities for discount
/// curves, forward curves, hazard curves and vol surfaces, restore-and-reprice
/// isolation for FX, inflation curves, base-correlation curves, and market
/// scalars (the same `MarketRestoreFlags` families the parallel methodology
/// uses), T₀-vs-T₁ model-parameter snapshot P&L via the internal
/// snapshot/restore helpers, and
/// theta. Each factor maps into its dedicated `PnlAttribution` bucket here, so
/// an FX-rate, spot, inflation, correlation, or model-parameter move lands in
/// its named field rather than silently inflating `residual`.
///
/// FX *translation* into a non-native reporting currency is out of scope
/// for this standalone path, which reports in the instrument's pricing currency.
///
/// The result uses bump-and-reprice first-order factor P&Ls and any configured
/// gamma term. The residual reconciles actual repriced P&L less explained
/// factor P&L; factor computations that fail internally are recorded through
/// the result metadata/residual rather than aborting the entire attribution
/// whenever the base repricing remains available. `execution_policy` is copied
/// into result metadata so callers can distinguish deterministic and parallel
/// runs.
///
/// # Arguments
///
/// * `request` - Instrument, market states, dates, execution policy,
///   optional opening model-parameter snapshot and prepared endpoints. The
///   request's `FinstackConfig` is not read; rounding is not applied.
/// * `config` - Taylor attribution policy, including factor selection, bump
///   sizes, and optional gamma treatment.
///
/// # Errors
///
/// Returns an error if the base instrument repricing, required market lookup,
/// FX conversion used to calculate total P&L, or result construction fails.
/// It can also return an error when factor accumulation detects an invalid
/// currency or non-finite monetary amount.
pub(crate) fn attribute_pnl_taylor(
    request: &AttributionRequest<'_>,
    config: &TaylorAttributionConfig,
) -> Result<PnlAttribution> {
    let AttributionRequest {
        instrument,
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        execution_policy,
        model_params_t0,
        prepared_endpoints,
        ..
    } = *request;
    let execution = TaylorExecution {
        policy: execution_policy,
        prepared_endpoints,
    };
    let taylor = compute_taylor_result(
        instrument,
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        config,
        execution,
        model_params_t0,
    )?;

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
    // Policy-visibility invariant: stamp the execution policy the
    // attribution ran under (workspace rule: results carry the parallel flag).
    attribution.meta.execution_policy = Some(execution.policy);

    // Surface the factor-level diagnostics collected during computation
    // (failed factors, surface-averaged vol moves, missing T0 FX) and
    // propagate the invalid flag when a dependency-backed factor or theta failed.
    attribution.meta.notes.extend(taylor.notes.iter().cloned());
    if taylor.result_invalid {
        attribution.result_invalid = true;
    }

    // Taylor factor P&Ls arrive as raw f64s; a degenerate curve or bump can
    // make one non-finite. Route every f64 → Money construction through
    // `factor_money_or_invalid` so a NaN/Inf flags the attribution invalid
    // instead of panicking inside `Money::new`.
    let mut non_finite_detected = false;

    for factor in &taylor.factors {
        let pnl_amount = factor.explained_pnl + factor.gamma_pnl.unwrap_or(0.0);
        let factor_money = factor_money_or_invalid(
            pnl_amount,
            ccy,
            &factor.factor_name,
            &mut attribution.meta.notes,
            &mut non_finite_detected,
        );

        // Route accumulation through Money::checked_add so a currency
        // mismatch surfaces as an error instead of being silently coerced into
        // `ccy`. Taylor factors are all produced in the instrument's native
        // currency in practice, but the safety net matches the rest of the
        // attribution code.
        if factor.factor_name.starts_with("Rates:") || factor.factor_name.starts_with("Forward:") {
            attribution.rates_curves_pnl =
                attribution.rates_curves_pnl.checked_add(factor_money)?;
        } else if factor.factor_name.starts_with("Credit:") {
            attribution.credit_curves_pnl =
                attribution.credit_curves_pnl.checked_add(factor_money)?;
        } else if factor.factor_name.starts_with("Vol:") {
            attribution.vol_pnl = attribution.vol_pnl.checked_add(factor_money)?;
        } else if factor.factor_name == "Fx" {
            attribution.fx_pnl = attribution.fx_pnl.checked_add(factor_money)?;
            stamp_fx_policy(
                &mut attribution,
                ccy,
                "Taylor FX-exposure P&L (T0 FX matrix restored vs T1)",
            );
        } else if factor.factor_name == "Inflation" {
            attribution.inflation_curves_pnl =
                attribution.inflation_curves_pnl.checked_add(factor_money)?;
        } else if factor.factor_name == "Correlations" {
            attribution.correlations_pnl =
                attribution.correlations_pnl.checked_add(factor_money)?;
        } else if factor.factor_name == "MarketScalars" {
            attribution.market_scalars_pnl =
                attribution.market_scalars_pnl.checked_add(factor_money)?;
        } else if factor.factor_name == "ModelParameters" {
            attribution.model_params_pnl =
                attribution.model_params_pnl.checked_add(factor_money)?;
        } else if factor.factor_name == "Theta" {
            // Taylor theta already includes cashflows from compute_theta_factor.
            // Re-use the coupon income that was captured during that compute
            // (previously we re-called collect_cashflows_in_period
            // here, which doubled cashflow traversal cost and risked silent
            // desync against the value `compute_theta_factor` consumed).
            let ci_val = taylor.theta_coupon_income.unwrap_or(0.0);
            let ci = factor_money_or_invalid(
                ci_val,
                ccy,
                "Theta coupon income",
                &mut attribution.meta.notes,
                &mut non_finite_detected,
            );
            let theta_only = Money::new(factor_money.amount() - ci.amount(), ccy)?;
            // Taylor path: delta_accrued and flat_window_diff are unavailable (no repricing).
            let carry_inputs = TotalReturnCarryInputs {
                cash_paid: ci,
                delta_accrued: None,
                flat_window_diff: None,
                funding_cost: None,
                warnings: Vec::new(),
                invalid: false,
            };
            apply_total_return_carry(&mut attribution, theta_only, carry_inputs)?;
        }
    }

    // Propagate the non-finite flag before `finalize_attribution` so the
    // residual / tolerance machinery treats the result as invalid.
    if non_finite_detected {
        attribution.result_invalid = true;
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
    // `finalize_attribution` above. The internal Taylor factor result keeps a
    // price-only `unexplained_pct` (PV₁−PV₀ basis); quoting that here would
    // disagree with `attribution.residual`, so we use the residual stats that
    // `compute_residual` just populated instead.
    attribution.meta.notes.push(format!(
        "Taylor attribution: {:.2}% residual ({} factors, {} repricings)",
        attribution.meta.residual_pct,
        taylor.factors.len(),
        taylor.num_repricings,
    ));
    attribution.meta.notes.push(
        "Taylor coverage: rates/credit/vol/FX-exposure/inflation/correlations/\
         market-scalars/model-parameters/theta. Restore-and-reprice families \
         (FX, inflation, correlations, scalars, model parameters) isolate the \
         full family P&L; rates/credit/vol remain sensitivity × observed move."
            .to_string(),
    );

    Ok(attribution)
}

// NOTE: the former `measure_forward_curve_shift` /
// `measure_average_rate_shift` helpers — an unweighted mean of per-tenor shifts
// — were removed. An unweighted average mis-attributes non-parallel curve
// moves (a steepener averages toward zero), so `compute_curve_factor`
// now measures the per-tenor move and pair it with a
// per-bucket (key-rate) DV01 instead.

/// Standard key-rate bucket grid (years) used for key-rate-aware rate / forward
/// curve attribution. Matches the DV01 calculator's standard bucket grid.
const KEY_RATE_BUCKETS_YEARS: [f64; 11] =
    [0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 15.0, 20.0, 30.0];

/// One key-rate bucket's contribution to a rate/forward factor: the per-bucket
/// DV01 (from a triangular bump) paired with the realized per-tenor curve move.
struct KeyRateBucket {
    /// Per-bucket DV01 (currency / bp) from a triangular key-rate bump.
    dv01: f64,
    /// Realized zero-rate move at this bucket's tenor (basis points).
    move_bp: f64,
}

/// Convexity (gamma) P&L from a single **parallel** up/down reprice of
/// `curve_id`.
///
/// The key-rate decomposition is first-order only. Summing per-bucket second
/// differences captures only the diagonal of the Hessian, and because
/// triangular bucket weights form a partition of unity (`Σ wᵢ(t) = 1`), an
/// exposure at a knot between two buckets picks up weight `w` from each bump
/// so its diagonal terms scale by `Σ wᵢ² < 1` — a 2×
/// convexity understatement for a knot midway between buckets (w = 0.5/0.5).
/// Cross-bucket Hessian terms would need O(n²) repricings, so instead the
/// second-order term comes from one parallel bump:
///
/// ```text
///   γ_par     = (PV(+h) − 2·PV₀ + PV(−h)) / h²         (h = bump_bp)
///   gamma_pnl = ½ · γ_par · Δ̄²
/// ```
///
/// where `Δ̄` is the sensitivity-weighted average bucket move
/// `Σ sᵢΔᵢ / Σ sᵢ` (the parallel-equivalent move for this exposure profile),
/// falling back to the simple mean of the bucket moves when `Σ sᵢ ≈ 0`.
///
/// See Press, Teukolsky, Vetterling & Flannery, *Numerical Recipes* (3rd ed.),
/// §5.7 for the finite-difference step-size / noise-floor considerations that
/// motivate the bump bounds in [`TaylorAttributionConfig::validate`].
#[allow(clippy::too_many_arguments)]
fn parallel_gamma_pnl(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    as_of_t0: Date,
    pv_t0: Money,
    curve_id: &CurveId,
    bump_bp: f64,
    sensitivities: &[f64],
    moves_bp: &[f64],
) -> Result<f64> {
    let up = market_t0.bump([MarketBump::Curve {
        id: curve_id.clone(),
        spec: BumpSpec::parallel_bp(bump_bp),
    }])?;
    let pv_up = reprice_instrument(instrument, &up, as_of_t0)?;

    let down = market_t0.bump([MarketBump::Curve {
        id: curve_id.clone(),
        spec: BumpSpec::parallel_bp(-bump_bp),
    }])?;
    let pv_down = reprice_instrument(instrument, &down, as_of_t0)?;

    let gamma_par =
        (pv_up.amount() - 2.0 * pv_t0.amount() + pv_down.amount()) / (bump_bp * bump_bp);

    let total_sens = finstack_quant_core::math::neumaier_sum(sensitivities.iter().copied());
    let avg_move_bp = if total_sens.abs() > 1e-12 {
        finstack_quant_core::math::neumaier_sum(
            sensitivities
                .iter()
                .zip(moves_bp.iter())
                .map(|(s, m)| s * m),
        ) / total_sens
    } else if moves_bp.is_empty() {
        0.0
    } else {
        finstack_quant_core::math::neumaier_sum(moves_bp.iter().copied()) / moves_bp.len() as f64
    };

    Ok(0.5 * gamma_par * avg_move_bp * avg_move_bp)
}

/// Triangular key-rate bump spec for bucket `i` of `KEY_RATE_BUCKETS_YEARS`.
///
/// The wing buckets use the dedicated half-triangle constructors so the
/// `Σ wᵢ(t) = 1.0` partition-of-unity invariant holds across the whole curve
/// (matching the canonical `BucketedDv01` calculator): the first bucket is
/// flat at 1.0 below its tenor (passing `prev = 0.0` instead would understate
/// sub-3M DV01), and the last bucket is flat at 1.0 beyond 30Y (passing
/// `next = ∞` instead produces a NaN weight for any knot past 30Y and aborts
/// the whole factor).
fn key_rate_bump_spec(i: usize, bump_bp: f64) -> BumpSpec {
    let target = KEY_RATE_BUCKETS_YEARS[i];
    if i == 0 {
        return BumpSpec::triangular_key_rate_first_bp(target, KEY_RATE_BUCKETS_YEARS[1], bump_bp);
    }
    let prev = KEY_RATE_BUCKETS_YEARS[i - 1];
    if i + 1 == KEY_RATE_BUCKETS_YEARS.len() {
        return BumpSpec::triangular_key_rate_last_bp(prev, target, bump_bp);
    }
    BumpSpec::triangular_key_rate_bp(prev, target, KEY_RATE_BUCKETS_YEARS[i + 1], bump_bp)
}

/// Which rates curve family a key-rate factor is measured on.
#[derive(Clone, Copy)]
enum CurveKind {
    /// Discount curve: realized move measured in zero rates, `Rates:` prefix.
    Discount,
    /// Forward/projection curve: realized move measured in forward rates,
    /// `Forward:` prefix.
    Forward,
}

/// Compute rates-curve sensitivity attribution for a single curve — KEY-RATE
/// AWARE.
///
/// Rather than a single parallel DV01 multiplied by an *average* curve shift
/// (which mis-attributes non-parallel moves — a steepener averages toward zero
/// and inflates the unexplained residual), this bumps each standard key-rate
/// bucket with a triangular weight, measures the DV01 of that bucket, and pairs
/// it with the realized move at that bucket's tenor (zero rate for a discount
/// curve, forward rate for a forward curve):
///
///   explained = Σ_bucket  DV01_bucket × Δr_bucket
///
/// The reported `sensitivity` is the parallel-equivalent DV01 (Σ bucket DV01s)
/// and `market_move` the average shift used by the internal factor result.
#[allow(clippy::too_many_arguments)]
fn compute_curve_factor(
    kind: CurveKind,
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    pv_t0: Money,
    curve_id: &CurveId,
    config: &TaylorAttributionConfig,
) -> Result<TaylorFactorResult> {
    // Realized per-tenor moves on the standard bucket grid (bp).
    let per_tenor_move_bp: Vec<f64> = match kind {
        CurveKind::Discount => {
            let (curve_t0, curve_t1) = (
                market_t0.get_discount(curve_id.as_str())?,
                market_t1.get_discount(curve_id.as_str())?,
            );
            KEY_RATE_BUCKETS_YEARS
                .iter()
                .map(|&t| (curve_t1.zero(t) - curve_t0.zero(t)) * 10_000.0)
                .collect()
        }
        CurveKind::Forward => {
            let (curve_t0, curve_t1) = (
                market_t0.get_forward(curve_id.as_str())?,
                market_t1.get_forward(curve_id.as_str())?,
            );
            KEY_RATE_BUCKETS_YEARS
                .iter()
                .map(|&t| (curve_t1.rate(t) - curve_t0.rate(t)) * 10_000.0)
                .collect()
        }
    };

    let mut buckets: Vec<KeyRateBucket> = Vec::with_capacity(KEY_RATE_BUCKETS_YEARS.len());
    for (i, &move_bp) in per_tenor_move_bp.iter().enumerate() {
        let up = market_t0.bump([MarketBump::Curve {
            id: curve_id.clone(),
            spec: key_rate_bump_spec(i, config.rate_bump_bp),
        }])?;
        let pv_up = reprice_instrument(instrument, &up, as_of_t0)?;

        let down = market_t0.bump([MarketBump::Curve {
            id: curve_id.clone(),
            spec: key_rate_bump_spec(i, -config.rate_bump_bp),
        }])?;
        let pv_down = reprice_instrument(instrument, &down, as_of_t0)?;

        // Central difference per bucket: O(h²) accuracy.
        let dv01 = (pv_up.amount() - pv_down.amount()) / (2.0 * config.rate_bump_bp);
        buckets.push(KeyRateBucket { dv01, move_bp });
    }

    // Key-rate-aware explained P&L: Σ DV01_bucket × Δr_bucket. Compensated
    // summation keeps the per-bucket accumulation stable.
    let explained =
        finstack_quant_core::math::neumaier_sum(buckets.iter().map(|b| b.dv01 * b.move_bp));
    let total_dv01 = finstack_quant_core::math::neumaier_sum(buckets.iter().map(|b| b.dv01));
    let avg_move_bp = if buckets.is_empty() {
        0.0
    } else {
        finstack_quant_core::math::neumaier_sum(buckets.iter().map(|b| b.move_bp))
            / buckets.len() as f64
    };

    // Second-order term from a single parallel reprice (see
    // `parallel_gamma_pnl`): the key-rate decomposition stays first-order
    // only, because a per-bucket gamma sum captures just the Hessian diagonal
    // and understates cross-bucket convexity.
    let gamma_pnl = if config.include_gamma {
        let dv01s: Vec<f64> = buckets.iter().map(|b| b.dv01).collect();
        let moves: Vec<f64> = buckets.iter().map(|b| b.move_bp).collect();
        Some(parallel_gamma_pnl(
            instrument,
            market_t0,
            as_of_t0,
            pv_t0,
            curve_id,
            config.rate_bump_bp,
            &dv01s,
            &moves,
        )?)
    } else {
        None
    };

    let prefix = match kind {
        CurveKind::Discount => "Rates",
        CurveKind::Forward => "Forward",
    };
    Ok(TaylorFactorResult {
        factor_name: format!("{prefix}:{curve_id}"),
        sensitivity: total_dv01,
        market_move: avg_move_bp,
        explained_pnl: explained,
        gamma_pnl,
    })
}

/// Compute credit (CS01) attribution for a single credit curve.
///
/// The credit curve may be a `HazardCurve` (CDS-family instruments) or a
/// `DiscountCurve` (the Tsiveriotis–Zhang risky discount curve a convertible
/// bond prices off). [`measure_credit_curve_shift`] /
/// [`measure_per_tenor_credit_curve_shift`] measure the move in whichever basis
/// the instrument's own CS01 is defined on — par CDS spread for a hazard curve,
/// zero rate for a discount-style credit curve — so the move always pairs
/// unit-correctly with the CS01 (pairing a par-spread CS01 with a hazard-rate
/// move would overstate by 1/(1−R)).
///
/// When per-tenor CS01 is available (`keyrate`, from `BucketedCs01`), the
/// explained P&L is the key-rate sum `Σ_tenor CS01_t × Δs_t` — correct for
/// non-parallel (steepener / twist) credit-curve moves. Otherwise it falls back
/// to a parallel bump: an aggregate CS01 times the average credit-curve move.
struct CreditFactorInputs<'a> {
    instrument: &'a Arc<dyn Instrument>,
    market_t0: &'a MarketContext,
    market_t1: &'a MarketContext,
    as_of_t0: Date,
    pv_t0: Money,
    curve_id: &'a CurveId,
    config: &'a TaylorAttributionConfig,
    keyrate: Option<&'a [(f64, f64)]>,
}

fn compute_credit_factor(inputs: CreditFactorInputs<'_>) -> Result<TaylorFactorResult> {
    let CreditFactorInputs {
        instrument,
        market_t0,
        market_t1,
        as_of_t0,
        pv_t0,
        curve_id,
        config,
        keyrate,
    } = inputs;

    // Key-rate path: per-tenor CS01 × per-tenor credit-curve move.
    if let Some(buckets) = keyrate.filter(|b| !b.is_empty()) {
        let tenors: Vec<f64> = buckets.iter().map(|(t, _)| *t).collect();
        let shifts =
            measure_per_tenor_credit_curve_shift(curve_id.as_str(), market_t0, market_t1, &tenors)?;
        let explained = finstack_quant_core::math::neumaier_sum(
            buckets
                .iter()
                .zip(shifts.iter())
                .map(|((_, cs01), shift)| cs01 * shift),
        );
        let total_cs01 = finstack_quant_core::math::neumaier_sum(buckets.iter().map(|(_, c)| *c));
        let avg_move = if shifts.is_empty() {
            0.0
        } else {
            finstack_quant_core::math::neumaier_sum(shifts.iter().copied()) / shifts.len() as f64
        };
        // Credit convexity from a single parallel reprice (see
        // `parallel_gamma_pnl`), so `include_gamma` yields a second-order term
        // on the key-rate path just like the parallel-bump fallback below
        // (previously this path silently dropped credit gamma).
        let gamma_pnl = if config.include_gamma {
            let cs01s: Vec<f64> = buckets.iter().map(|(_, c)| *c).collect();
            Some(parallel_gamma_pnl(
                instrument,
                market_t0,
                as_of_t0,
                pv_t0,
                curve_id,
                config.credit_bump_bp,
                &cs01s,
                &shifts,
            )?)
        } else {
            None
        };
        return Ok(TaylorFactorResult {
            factor_name: format!("Credit:{}", curve_id),
            sensitivity: total_cs01,
            market_move: avg_move,
            explained_pnl: explained,
            gamma_pnl,
        });
    }

    // Fallback: parallel bump of the credit curve. A `parallel_bp` bump is a
    // par-spread shock on a hazard curve and a zero-rate shock on a
    // discount-style credit curve; either way `cs01` and the move below share
    // that basis, so they pair unit-correctly.
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

    // Central difference CS01: O(h²) accuracy, $ per bp of credit-curve move.
    let cs01 = (pv_up.amount() - pv_down.amount()) / (2.0 * config.credit_bump_bp);

    let spread_move_bp = measure_credit_curve_shift(
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

/// Compute volatility (vega) attribution for a single vol-surface dependency.
///
/// The realized move is measured at the instrument's own reference point when
/// available — `reference_expiry_years` derived from [`Instrument::expiry`]
/// (Act/365) and the dependency's `reference_strike` — because the fallback
/// surface-averaged move (sampled across expiries at the middle strike) nets a
/// term-structure twist toward zero and mis-states the move the instrument
/// actually experienced. When either reference coordinate is missing,
/// `measure_vol_surface_shift` falls back to the surface average and the
/// caller records a metadata note.
#[allow(clippy::too_many_arguments)]
fn compute_vol_factor(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    pv_t0: Money,
    dependency: &finstack_quant_valuations::instruments::VolatilityDependency,
    reference_expiry_years: Option<f64>,
    config: &TaylorAttributionConfig,
) -> Result<TaylorFactorResult> {
    let vol_surface_id = &dependency.vol_surface_id;
    let bumped_up = bump_surface_vol_absolute(market_t0, vol_surface_id.as_str(), config.vol_bump)?;
    let pv_up = reprice_instrument(instrument, &bumped_up, as_of_t0)?;

    let bumped_down =
        bump_surface_vol_absolute(market_t0, vol_surface_id.as_str(), -config.vol_bump)?;
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

    // vol_move is in vol points (percentage points of absolute vol), measured
    // at the instrument's reference point when both coordinates are known
    // (surface-averaged otherwise — see the function docs).
    let vol_move = measure_vol_surface_shift(
        vol_surface_id.as_str(),
        market_t0,
        market_t1,
        reference_expiry_years,
        dependency.reference_strike,
    )?;

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
        factor_name: format!("Vol:{}", vol_surface_id),
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
    let fx_snapshot = MarketSnapshot::extract(market_t0, MarketRestoreFlags::FX);
    let market_with_t0_fx =
        MarketSnapshot::restore_market(market_t1, &fx_snapshot, MarketRestoreFlags::FX);
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

/// True when a snapshot extracted with `flags` actually holds that family.
fn snapshot_has_family(snapshot: &MarketSnapshot, flags: MarketRestoreFlags) -> bool {
    if flags.contains(MarketRestoreFlags::INFLATION) {
        return !snapshot.inflation_curves.is_empty();
    }
    if flags.contains(MarketRestoreFlags::CORRELATION) {
        return !snapshot.base_correlation_curves.is_empty();
    }
    if flags.contains(MarketRestoreFlags::SCALARS) {
        return !snapshot.prices.is_empty()
            || !snapshot.series.is_empty()
            || !snapshot.inflation_indices.is_empty()
            || !snapshot.dividends.is_empty()
            || !snapshot.price_curves.is_empty();
    }
    false
}

/// Isolate one restore-flag family by splicing the T₀ snapshot into the T₁
/// market, matching [`compute_fx_factor`] and the parallel methodology.
fn compute_restored_family_factor(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t1: Date,
    pv_t1: Money,
    flags: MarketRestoreFlags,
    factor_name: &str,
) -> Result<TaylorFactorResult> {
    let snapshot = MarketSnapshot::extract(market_t0, flags);
    let market_with_t0 = MarketSnapshot::restore_market(market_t1, &snapshot, flags);
    let pv_with_t0 = reprice_instrument(instrument, &market_with_t0, as_of_t1)?;
    let explained = pv_t1.amount() - pv_with_t0.amount();
    Ok(TaylorFactorResult {
        factor_name: factor_name.to_string(),
        sensitivity: explained,
        market_move: 1.0,
        explained_pnl: explained,
        gamma_pnl: None,
    })
}

/// Run a restore-and-reprice family when either market carries that family.
#[allow(clippy::too_many_arguments)]
fn record_restored_family_factor(
    factor_kind: &str,
    factor_name: &str,
    flags: MarketRestoreFlags,
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t1: Date,
    pv_t1: Money,
    factors: &mut Vec<TaylorFactorResult>,
    total_explained: &mut finstack_quant_core::math::NeumaierAccumulator,
    num_repricings: &mut usize,
    notes: &mut Vec<String>,
    result_invalid: &mut bool,
) {
    let t0_snap = MarketSnapshot::extract(market_t0, flags);
    let t1_snap = MarketSnapshot::extract(market_t1, flags);
    if !snapshot_has_family(&t0_snap, flags) && !snapshot_has_family(&t1_snap, flags) {
        return;
    }
    record_taylor_factor_result(
        factor_kind,
        &CurveId::new(factor_name),
        compute_restored_family_factor(
            instrument,
            market_t0,
            market_t1,
            as_of_t1,
            pv_t1,
            flags,
            factor_name,
        ),
        factors,
        total_explained,
        num_repricings,
        1,
        notes,
        result_invalid,
    );
}

/// Isolate T₀ vs T₁ model-parameter P&L by repricing the T₁ market with the
/// opening snapshot restored onto the instrument.
fn compute_model_params_factor(
    instrument: &Arc<dyn Instrument>,
    market_t1: &MarketContext,
    as_of_t1: Date,
    pv_t1: Money,
    params_t0: &ModelParamsSnapshot,
) -> Result<TaylorFactorResult> {
    let instrument_t0 = model_params::with_model_params(instrument, params_t0)?;
    let pv_with_t0 = reprice_instrument(&instrument_t0, market_t1, as_of_t1)?;
    let explained = pv_t1.amount() - pv_with_t0.amount();
    Ok(TaylorFactorResult {
        factor_name: "ModelParameters".to_string(),
        sensitivity: explained,
        market_move: 1.0,
        explained_pnl: explained,
        gamma_pnl: None,
    })
}

/// Coupon income for the theta period — surfaced separately so
/// `attribute_pnl_taylor` can re-use it when splitting `theta_pnl` into the
/// pure PV move and the realized cashflow component (instead of calling
/// `collect_cashflows_in_period` again, which would re-traverse the
/// instrument's cashflow schedule and risk silent desync if the schedule path
/// is non-deterministic).
struct ThetaFactorOutcome {
    factor: TaylorFactorResult,
    coupon_income: f64,
}

/// Compute theta (time decay + realized cashflows) by repricing at T1 date
/// with T0 market, then adding any coupon payments in the period.
fn compute_theta_factor(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    pv_t0: Money,
) -> Result<ThetaFactorOutcome> {
    use finstack_quant_valuations::metrics::collect_cashflows_in_period;

    let pv_t0_at_t1 = reprice_instrument(instrument, market_t0, as_of_t1)?;
    let pv_diff = pv_t0_at_t1.amount() - pv_t0.amount();
    let days = (as_of_t1 - as_of_t0).whole_days() as f64;

    let coupon_income = collect_cashflows_in_period(
        instrument.as_ref(),
        market_t0,
        as_of_t0,
        as_of_t1,
        pv_t0.currency(),
    )?;

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

    Ok(ThetaFactorOutcome {
        factor: TaylorFactorResult {
            factor_name: "Theta".to_string(),
            sensitivity: theta_per_day,
            market_move: days,
            explained_pnl: theta_pnl,
            gamma_pnl: None,
        },
        coupon_income,
    })
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
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
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
    fn test_taylor_config_validation() {
        let mut config = TaylorAttributionConfig::default();
        assert!(config.validate().is_ok());

        config.vol_bump = 0.20;
        assert!(config.validate().is_ok());

        config.vol_bump = 0.21;
        assert!(config.validate().is_err());

        config.vol_bump = 0.0;
        assert!(config.validate().is_err());

        config.vol_bump = -0.01;
        assert!(config.validate().is_err());
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
        assert!(serde_json::from_str::<TaylorAttributionConfig>(
            r#"{"include_gamma":false,"unexpected":true}"#
        )
        .is_err());

        let schema = serde_json::to_value(schemars::schema_for!(TaylorAttributionConfig))
            .expect("Taylor attribution config schema");
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn test_taylor_attribution_empty_market() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);

        let instrument: Arc<dyn Instrument> = Arc::new(TestInstrument::new(
            "TEST-001",
            Money::from((1000_i64, Currency::USD)),
        ));

        let market_t0 = MarketContext::new();
        let market_t1 = MarketContext::new();
        let config = TaylorAttributionConfig::default();

        let result = compute_taylor_result(
            &instrument,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
            &config,
            TaylorExecution {
                policy: ExecutionPolicy::Parallel,
                prepared_endpoints: None,
            },
            None,
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
            Money::from((1000_i64, Currency::USD)),
        ));

        let market_t0 = MarketContext::new();
        let market_t1 = MarketContext::new();
        let config = TaylorAttributionConfig::default();

        let attribution = crate::attribute_pnl(
            &AttributionMethod::Taylor(config),
            &crate::AttributionRequest {
                execution_policy: ExecutionPolicy::Parallel,
                ..crate::AttributionRequest::new(
                    &instrument,
                    &market_t0,
                    &market_t1,
                    as_of_t0,
                    as_of_t1,
                    &finstack_quant_core::config::FinstackConfig::default(),
                )
            },
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
        use finstack_quant_core::dates::DayCount;
        use finstack_quant_core::market_data::term_structures::ForwardCurve;
        use finstack_quant_core::types::CurveId;

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
            TestInstrument::new("FWDI", Money::from((0_i64, Currency::USD)))
                .with_forward_curves(&["TEST-FWD"]),
        );

        let config = TaylorAttributionConfig::default();
        let result = compute_taylor_result(
            &instrument,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
            &config,
            TaylorExecution {
                policy: ExecutionPolicy::Parallel,
                prepared_endpoints: None,
            },
            None,
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

    finstack_quant_valuations::impl_empty_cashflow_provider!(
        FxLinkedInstrument,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl Instrument for FxLinkedInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> finstack_quant_valuations::pricer::InstrumentType {
            finstack_quant_valuations::pricer::InstrumentType::Bond
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn attributes(&self) -> &finstack_quant_valuations::instruments::Attributes {
            use std::sync::OnceLock;
            static ATTRS: OnceLock<finstack_quant_valuations::instruments::Attributes> =
                OnceLock::new();
            ATTRS.get_or_init(finstack_quant_valuations::instruments::Attributes::default)
        }

        fn attributes_mut(&mut self) -> &mut finstack_quant_valuations::instruments::Attributes {
            unreachable!("FxLinkedInstrument::attributes_mut should not be called")
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn market_dependencies(
            &self,
        ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
        {
            Ok(finstack_quant_valuations::instruments::MarketDependencies::new())
        }

        fn base_value(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
            // Price in USD as the EUR notional converted at the market FX rate.
            let usd = market.convert_money(
                Money::new(self.eur_notional, Currency::EUR).expect("valid money fixture"),
                Currency::USD,
                as_of,
            )?;
            Ok(usd)
        }

        fn price_with_metrics(
            &self,
            market: &MarketContext,
            as_of: Date,
            _metrics: &[finstack_quant_valuations::metrics::MetricId],
            _options: finstack_quant_valuations::instruments::PricingOptions,
        ) -> finstack_quant_valuations::Result<finstack_quant_valuations::results::ValuationResult>
        {
            Ok(
                finstack_quant_valuations::results::ValuationResult::stamped(
                    self.id(),
                    as_of,
                    self.value(market, as_of)?,
                ),
            )
        }
    }

    #[test]
    fn taylor_buckets_fx_exposure_into_fx_pnl() {
        use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
        use finstack_quant_core::Error;

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
        let attribution = crate::attribute_pnl(
            &AttributionMethod::Taylor(config.clone()),
            &crate::AttributionRequest {
                execution_policy: ExecutionPolicy::Parallel,
                ..crate::AttributionRequest::new(
                    &instrument,
                    &market_t0,
                    &market_t1,
                    as_of_t0,
                    as_of_t1,
                    &finstack_quant_core::config::FinstackConfig::default(),
                )
            },
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

        // The internal Taylor factor decomposition should also expose an "Fx" factor.
        let taylor = compute_taylor_result(
            &instrument,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
            &config,
            TaylorExecution {
                policy: ExecutionPolicy::Parallel,
                prepared_endpoints: None,
            },
            None,
        )
        .expect("taylor attribution should succeed");
        assert!(
            taylor.factors.iter().any(|f| f.factor_name == "Fx"),
            "expected an Fx factor, got {:?}",
            taylor.factors
        );
    }

    /// Malformed config bumps (≤ 0 or > sane max) must be rejected at
    /// validation rather than producing a `result_invalid` flagged result.
    /// Previously the central-difference DV01 was a 0/0 NaN
    /// and the attribution flagged itself invalid; with the strengthened
    /// validation the caller now gets an immediate `Error::Validation`.
    #[test]
    fn taylor_rejects_non_positive_bump_at_validation() {
        use finstack_quant_core::market_data::term_structures::DiscountCurve;
        use finstack_quant_core::math::interp::InterpStyle;

        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);

        let instrument: Arc<dyn Instrument> = Arc::new(
            TestInstrument::new("NF-001", Money::from((1000_i64, Currency::USD)))
                .with_discount_curves(&["USD-OIS"]),
        );

        let curve = |base, df1| {
            DiscountCurve::builder("USD-OIS")
                .base_date(base)
                .knots(vec![(0.0, 1.0), (1.0, df1)])
                .interp(InterpStyle::Linear)
                .build()
                .expect("discount curve")
        };
        let market_t0 = MarketContext::new().insert(curve(as_of_t0, 0.98));
        let market_t1 = MarketContext::new().insert(curve(as_of_t1, 0.97));

        for bad in [
            TaylorAttributionConfig {
                rate_bump_bp: 0.0,
                ..TaylorAttributionConfig::default()
            },
            TaylorAttributionConfig {
                rate_bump_bp: -1.0,
                ..TaylorAttributionConfig::default()
            },
            TaylorAttributionConfig {
                credit_bump_bp: 0.0,
                ..TaylorAttributionConfig::default()
            },
            TaylorAttributionConfig {
                credit_bump_bp: 200.0,
                ..TaylorAttributionConfig::default()
            },
        ] {
            let err = crate::attribute_pnl(
                &AttributionMethod::Taylor(bad.clone()),
                &crate::AttributionRequest {
                    execution_policy: ExecutionPolicy::Parallel,
                    ..crate::AttributionRequest::new(
                        &instrument,
                        &market_t0,
                        &market_t1,
                        as_of_t0,
                        as_of_t1,
                        &finstack_quant_core::config::FinstackConfig::default(),
                    )
                },
            )
            .expect_err("malformed bump config must error at validation");
            let msg = format!("{err}");
            assert!(
                msg.to_lowercase().contains("bump"),
                "validation error must mention 'bump', got: {msg}"
            );
        }
    }

    // ── Audit-fix regression tests (2026-08) ───────────────────────────────
    //
    // Cover: B7 cross-bucket gamma, credit key-rate gamma, multi-surface vega,
    // vol reference-point measurement, factor-failure visibility, bump noise
    // floor, total-return unexplained basis, and FX presence on either side.

    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_valuations::instruments::VolatilityDependency;

    /// Payoff shapes for [`MockInstrument`].
    #[derive(Clone)]
    enum MockPayoff {
        /// `notional × df(tenor)` read from a discount curve.
        DiscountZero {
            curve: &'static str,
            tenor: f64,
            notional: f64,
        },
        /// `scale × exp(−rate(tenor)·tenor)` read from a forward curve.
        ForwardConvex {
            curve: &'static str,
            tenor: f64,
            scale: f64,
        },
        /// `scale × Σ finstack_quant_models::volatility::get_surface_vol_clamped(&surface, expiry, strike)` over `reads`.
        VolSum {
            reads: Vec<(&'static str, f64, f64)>,
            scale: f64,
        },
        /// EUR notional converted at the market FX rate when an FX matrix is
        /// present; face value in USD otherwise.
        FxOptional { eur_notional: f64 },
        /// `scale × price.amount()` from a market scalar price.
        Spot { id: &'static str, scale: f64 },
        /// `scale × cpi(tenor)` from an inflation curve.
        InflationCpi {
            curve: &'static str,
            tenor: f64,
            scale: f64,
        },
        /// `scale × correlation(detachment)` from a base-correlation curve.
        Correlation {
            curve: &'static str,
            detachment: f64,
            scale: f64,
        },
        /// `scale × model_cpr` from the instrument's model-parameter snapshot.
        ModelCpr { scale: f64 },
        /// Market-independent constant USD value.
        Constant(f64),
    }

    /// Flexible market-reading mock used by the audit-fix regression tests.
    #[derive(Clone)]
    struct MockInstrument {
        id: String,
        payoff: MockPayoff,
        discount_curves: Vec<CurveId>,
        forward_curves: Vec<CurveId>,
        credit_curves: Vec<CurveId>,
        vol_deps: Vec<VolatilityDependency>,
        expiry: Option<Date>,
        /// `(measure key, value)` pairs surfaced through `price_with_metrics`.
        keyrate_measures: Vec<(String, f64)>,
        /// Optional fixed coupon for the theta window.
        coupon: Option<(Date, Money)>,
        /// Optional schedule-construction failure independent of pricing.
        cashflow_error: Option<&'static str>,
        /// Optional CPR used by [`MockPayoff::ModelCpr`] and the model-param hooks.
        model_cpr: Option<f64>,
    }

    impl MockInstrument {
        fn new(id: &str, payoff: MockPayoff) -> Self {
            Self {
                id: id.to_string(),
                payoff,
                discount_curves: Vec::new(),
                forward_curves: Vec::new(),
                credit_curves: Vec::new(),
                vol_deps: Vec::new(),
                expiry: None,
                keyrate_measures: Vec::new(),
                coupon: None,
                cashflow_error: None,
                model_cpr: None,
            }
        }
    }

    impl finstack_quant_cashflows::traits::CashflowScheduleSource for MockInstrument {
        fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
            Ok(None)
        }

        fn raw_cashflow_schedule(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> Result<finstack_quant_cashflows::builder::CashFlowSchedule> {
            use finstack_quant_core::cashflow::{CFKind, CashFlow};
            if let Some(error) = self.cashflow_error {
                return Err(finstack_quant_core::Error::Validation(error.to_string()));
            }
            let flows: Vec<CashFlow> = self
                .coupon
                .iter()
                .map(|(date, amount)| CashFlow::new(*date, None, *amount, CFKind::Fixed, 0.0, None))
                .collect();
            Ok(
                finstack_quant_cashflows::traits::schedule_from_classified_flows(
                    flows,
                    DayCount::Act365F,
                    finstack_quant_cashflows::traits::ScheduleBuildOpts::default(),
                ),
            )
        }
    }

    impl Instrument for MockInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> finstack_quant_valuations::pricer::InstrumentType {
            finstack_quant_valuations::pricer::InstrumentType::Bond
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn attributes(&self) -> &finstack_quant_valuations::instruments::Attributes {
            use std::sync::OnceLock;
            static ATTRS: OnceLock<finstack_quant_valuations::instruments::Attributes> =
                OnceLock::new();
            ATTRS.get_or_init(finstack_quant_valuations::instruments::Attributes::default)
        }

        fn attributes_mut(&mut self) -> &mut finstack_quant_valuations::instruments::Attributes {
            unreachable!("MockInstrument::attributes_mut should not be called")
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn expiry(&self) -> Option<Date> {
            self.expiry
        }

        fn model_params_snapshot(
            &self,
        ) -> finstack_quant_valuations::instruments::model_params::ModelParamsSnapshot {
            match self.model_cpr {
                Some(cpr) => {
                    use finstack_quant_cashflows::builder::{
                        DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec,
                    };
                    ModelParamsSnapshot::StructuredCredit {
                        prepayment_spec: PrepaymentModelSpec::constant_cpr(cpr),
                        default_spec: DefaultModelSpec::constant_cdr(0.0),
                        recovery_spec: RecoveryModelSpec::with_lag(0.0, 0),
                    }
                }
                None => ModelParamsSnapshot::None,
            }
        }

        fn with_model_params(
            &self,
            params: &finstack_quant_valuations::instruments::model_params::ModelParamsSnapshot,
        ) -> Result<Box<dyn Instrument>> {
            match params {
                ModelParamsSnapshot::None => Ok(self.clone_box()),
                ModelParamsSnapshot::StructuredCredit {
                    prepayment_spec, ..
                } => {
                    let mut clone = self.clone();
                    clone.model_cpr = Some(prepayment_spec.cpr);
                    Ok(Box::new(clone))
                }
                ModelParamsSnapshot::Convertible { .. } => {
                    Err(finstack_quant_core::Error::Validation(
                        "MockInstrument does not accept convertible model parameters".to_string(),
                    ))
                }
            }
        }

        fn market_dependencies(
            &self,
        ) -> Result<finstack_quant_valuations::instruments::MarketDependencies> {
            let mut deps = finstack_quant_valuations::instruments::MarketDependencies::new();
            for id in &self.discount_curves {
                deps.add_discount_curve(id.clone());
            }
            for id in &self.forward_curves {
                deps.add_forward_curve(id.clone());
            }
            for id in &self.credit_curves {
                deps.add_credit_curve(id.clone());
            }
            for dep in &self.vol_deps {
                deps.add_volatility_dependency(dep.clone());
            }
            Ok(deps)
        }

        fn base_value(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
            match &self.payoff {
                MockPayoff::DiscountZero {
                    curve,
                    tenor,
                    notional,
                } => {
                    let df = market.get_discount(curve)?.df(*tenor);
                    Ok(Money::new(notional * df, Currency::USD).expect("valid money fixture"))
                }
                MockPayoff::ForwardConvex {
                    curve,
                    tenor,
                    scale,
                } => {
                    let rate = market.get_forward(curve)?.rate(*tenor);
                    Ok(Money::new(scale * (-rate * tenor).exp(), Currency::USD)
                        .expect("valid money fixture"))
                }
                MockPayoff::VolSum { reads, scale } => {
                    let mut total = 0.0;
                    for (surface_id, expiry, strike) in reads {
                        let surface = market.get_surface(surface_id)?;
                        total += finstack_quant_models::volatility::get_surface_vol_clamped(
                            &surface, *expiry, *strike,
                        );
                    }
                    Ok(Money::new(scale * total, Currency::USD).expect("valid money fixture"))
                }
                MockPayoff::FxOptional { eur_notional } => {
                    if market.fx().is_some() {
                        market.convert_money(
                            Money::new(*eur_notional, Currency::EUR).expect("valid money fixture"),
                            Currency::USD,
                            as_of,
                        )
                    } else {
                        Ok(Money::new(*eur_notional, Currency::USD).expect("valid money fixture"))
                    }
                }
                MockPayoff::Spot { id, scale } => {
                    let price = match market.get_price(*id)? {
                        finstack_quant_core::market_data::scalars::MarketScalar::Price(money) => {
                            money.amount()
                        }
                        finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
                    };
                    Ok(Money::new(scale * price, Currency::USD).expect("valid money fixture"))
                }
                MockPayoff::InflationCpi {
                    curve,
                    tenor,
                    scale,
                } => {
                    let cpi = market.get_inflation_curve(*curve)?.cpi(*tenor);
                    Ok(Money::new(scale * cpi, Currency::USD).expect("valid money fixture"))
                }
                MockPayoff::Correlation {
                    curve,
                    detachment,
                    scale,
                } => {
                    let corr = market
                        .get_base_correlation(*curve)?
                        .correlation(*detachment);
                    Ok(Money::new(scale * corr, Currency::USD).expect("valid money fixture"))
                }
                MockPayoff::ModelCpr { scale } => {
                    let cpr = self.model_cpr.unwrap_or(0.0);
                    Ok(Money::new(scale * cpr, Currency::USD).expect("valid money fixture"))
                }
                MockPayoff::Constant(value) => {
                    Ok(Money::new(*value, Currency::USD).expect("valid money fixture"))
                }
            }
        }

        fn price_with_metrics(
            &self,
            market: &MarketContext,
            as_of: Date,
            _metrics: &[finstack_quant_valuations::metrics::MetricId],
            _options: finstack_quant_valuations::instruments::PricingOptions,
        ) -> finstack_quant_valuations::Result<finstack_quant_valuations::results::ValuationResult>
        {
            let mut result = finstack_quant_valuations::results::ValuationResult::stamped(
                self.id(),
                as_of,
                self.value(market, as_of)?,
            );
            for (key, value) in &self.keyrate_measures {
                result.measures.insert(
                    finstack_quant_valuations::metrics::MetricId::custom(key.clone()),
                    *value,
                );
            }
            Ok(result)
        }
    }

    /// Discount curve with a flat continuously-compounded zero rate `z`,
    /// knotted densely across the key-rate bucket grid (plus 1.5y).
    fn flat_zero_curve(id: &'static str, base: Date, z: f64) -> DiscountCurve {
        let tenors = [
            0.25, 0.5, 1.0, 1.5, 2.0, 3.0, 5.0, 7.0, 10.0, 15.0, 20.0, 30.0,
        ];
        let mut knots = vec![(0.0, 1.0)];
        knots.extend(tenors.iter().map(|&t| (t, (-z * t).exp())));
        DiscountCurve::builder(id)
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots(knots)
            .build()
            .expect("discount curve")
    }

    /// Forward curve with a flat rate `r`, knotted densely across the bucket
    /// grid (plus 1.5y).
    fn flat_forward_curve(id: &'static str, base: Date, r: f64) -> ForwardCurve {
        let tenors = [
            0.0, 0.25, 0.5, 1.0, 1.5, 2.0, 3.0, 5.0, 7.0, 10.0, 15.0, 20.0, 30.0,
        ];
        ForwardCurve::builder(CurveId::new(id), 0.25)
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots(tenors.iter().map(|&t| (t, r)).collect::<Vec<_>>())
            .build()
            .expect("forward curve")
    }

    use finstack_quant_core::market_data::term_structures::ForwardCurve;

    /// Two-expiry (0.5y / 5y) surface with per-expiry flat vols.
    fn two_expiry_surface(id: &'static str, front: f64, back: f64) -> VolSurface {
        VolSurface::builder(id)
            .expiries(&[0.5, 5.0])
            .strikes(&[90.0, 110.0])
            .row(&[front, front])
            .row(&[back, back])
            .build()
            .expect("vol surface")
    }

    /// B7: key-rate gamma must capture cross-bucket convexity. A zero maturing
    /// between two buckets (t = 1.5y, triangular weights 0.5/0.5) under a
    /// +100bp parallel move has analytic convexity P&L ½·t²·PV₀·Δz²; the old
    /// diagonal-only per-bucket sum recovered only ~half of it.
    #[test]
    fn keyrate_gamma_captures_cross_bucket_convexity() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);
        let notional = 100_000_000.0;
        let tenor = 1.5;
        let (z0, z1) = (0.03, 0.04);

        let market_t0 = MarketContext::new().insert(flat_zero_curve("USD-OIS", as_of_t0, z0));
        let market_t1 = MarketContext::new().insert(flat_zero_curve("USD-OIS", as_of_t1, z1));

        let mut mock = MockInstrument::new(
            "ZC-1.5Y",
            MockPayoff::DiscountZero {
                curve: "USD-OIS",
                tenor,
                notional,
            },
        );
        mock.discount_curves = vec![CurveId::new("USD-OIS")];
        let instrument: Arc<dyn Instrument> = Arc::new(mock);

        let config = TaylorAttributionConfig {
            include_gamma: true,
            ..TaylorAttributionConfig::default()
        };
        let result = compute_taylor_result(
            &instrument,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
            &config,
            TaylorExecution {
                policy: ExecutionPolicy::Serial,
                prepared_endpoints: None,
            },
            None,
        )
        .expect("taylor attribution should succeed");

        let factor = result
            .factors
            .iter()
            .find(|f| f.factor_name == "Rates:USD-OIS")
            .expect("rates factor must be present");
        let pv0 = notional * (-z0 * tenor).exp();
        let dz = z1 - z0;
        let expected_gamma = 0.5 * tenor * tenor * pv0 * dz * dz;
        let gamma = factor.gamma_pnl.expect("gamma requested via include_gamma");
        assert!(
            ((gamma - expected_gamma) / expected_gamma).abs() < 0.01,
            "gamma_pnl {gamma:.2} must be within 1% of analytic {expected_gamma:.2} \
             (diagonal-only key-rate gamma understates it ~2x)"
        );
    }

    /// B7 (forward-curve copy): same cross-bucket convexity check for the
    /// forward-curve factor path.
    #[test]
    fn forward_keyrate_gamma_captures_cross_bucket_convexity() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);
        let scale = 100_000_000.0;
        let tenor = 1.5;
        let (r0, r1) = (0.03, 0.04);

        let market_t0 = MarketContext::new().insert(flat_forward_curve("TEST-FWD", as_of_t0, r0));
        let market_t1 = MarketContext::new().insert(flat_forward_curve("TEST-FWD", as_of_t1, r1));

        let mut mock = MockInstrument::new(
            "FWD-CONVEX",
            MockPayoff::ForwardConvex {
                curve: "TEST-FWD",
                tenor,
                scale,
            },
        );
        mock.forward_curves = vec![CurveId::new("TEST-FWD")];
        let instrument: Arc<dyn Instrument> = Arc::new(mock);

        let config = TaylorAttributionConfig {
            include_gamma: true,
            ..TaylorAttributionConfig::default()
        };
        let result = compute_taylor_result(
            &instrument,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
            &config,
            TaylorExecution {
                policy: ExecutionPolicy::Serial,
                prepared_endpoints: None,
            },
            None,
        )
        .expect("taylor attribution should succeed");

        let factor = result
            .factors
            .iter()
            .find(|f| f.factor_name == "Forward:TEST-FWD")
            .expect("forward factor must be present");
        let pv0 = scale * (-r0 * tenor).exp();
        let dr = r1 - r0;
        let expected_gamma = 0.5 * tenor * tenor * pv0 * dr * dr;
        let gamma = factor.gamma_pnl.expect("gamma requested via include_gamma");
        assert!(
            ((gamma - expected_gamma) / expected_gamma).abs() < 0.01,
            "forward gamma_pnl {gamma:.2} must be within 1% of analytic {expected_gamma:.2}"
        );
    }

    /// Credit convexity must not vanish on the BucketedCs01 (key-rate) path:
    /// with `include_gamma = true` a discount-style credit zero must report a
    /// second-order term matching the analytic ½·t²·PV₀·Δz².
    #[test]
    fn credit_keyrate_path_computes_parallel_gamma() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);
        let notional = 50_000_000.0;
        let tenor = 5.0;
        let (z0, z1) = (0.02, 0.03);

        let market_t0 = MarketContext::new().insert(flat_zero_curve("CR-DISC", as_of_t0, z0));
        let market_t1 = MarketContext::new().insert(flat_zero_curve("CR-DISC", as_of_t1, z1));

        let pv0 = notional * (-z0 * tenor).exp();
        // Analytic zero-rate CS01 ($/bp) for the 5y bucket.
        let cs01 = -notional * tenor * (-z0 * tenor).exp() / 10_000.0;

        let mut mock = MockInstrument::new(
            "RISKY-ZC",
            MockPayoff::DiscountZero {
                curve: "CR-DISC",
                tenor,
                notional,
            },
        );
        mock.credit_curves = vec![CurveId::new("CR-DISC")];
        mock.keyrate_measures = vec![("bucketed_cs01::CR-DISC::5y".to_string(), cs01)];
        let instrument: Arc<dyn Instrument> = Arc::new(mock);

        let config = TaylorAttributionConfig {
            include_gamma: true,
            ..TaylorAttributionConfig::default()
        };
        let result = compute_taylor_result(
            &instrument,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
            &config,
            TaylorExecution {
                policy: ExecutionPolicy::Serial,
                prepared_endpoints: None,
            },
            None,
        )
        .expect("taylor attribution should succeed");

        let factor = result
            .factors
            .iter()
            .find(|f| f.factor_name == "Credit:CR-DISC")
            .expect("credit factor must be present");
        // Key-rate first-order path must still drive explained P&L.
        assert!(
            (factor.explained_pnl - cs01 * 100.0).abs() < 1.0,
            "key-rate first-order explained P&L must be CS01 x 100bp, got {}",
            factor.explained_pnl
        );
        let dz = z1 - z0;
        let expected_gamma = 0.5 * tenor * tenor * pv0 * dz * dz;
        let gamma = factor
            .gamma_pnl
            .expect("credit gamma must be computed on the key-rate path too");
        assert!(
            ((gamma - expected_gamma) / expected_gamma).abs() < 0.01,
            "credit gamma_pnl {gamma:.2} must be within 1% of analytic {expected_gamma:.2}"
        );
    }

    /// Vega must cover every volatility dependency, not just the first: with
    /// two moved surfaces both must appear as factors with their own P&L.
    #[test]
    fn vega_covers_all_volatility_dependencies() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);

        let market_t0 = MarketContext::new()
            .insert_surface(two_expiry_surface("VOL-A", 0.20, 0.20))
            .insert_surface(two_expiry_surface("VOL-B", 0.20, 0.20));
        // A: +2 vol points, B: +4 vol points.
        let market_t1 = MarketContext::new()
            .insert_surface(two_expiry_surface("VOL-A", 0.22, 0.22))
            .insert_surface(two_expiry_surface("VOL-B", 0.24, 0.24));

        let mut mock = MockInstrument::new(
            "TWO-VOL",
            MockPayoff::VolSum {
                reads: vec![("VOL-A", 1.0, 100.0), ("VOL-B", 1.0, 100.0)],
                scale: 1_000_000.0,
            },
        );
        mock.vol_deps = vec![
            VolatilityDependency::new("VOL-A", None, None),
            VolatilityDependency::new("VOL-B", None, None),
        ];
        let instrument: Arc<dyn Instrument> = Arc::new(mock);

        let config = TaylorAttributionConfig::default();
        let result = compute_taylor_result(
            &instrument,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
            &config,
            TaylorExecution {
                policy: ExecutionPolicy::Serial,
                prepared_endpoints: None,
            },
            None,
        )
        .expect("taylor attribution should succeed");

        let factor_a = result
            .factors
            .iter()
            .find(|f| f.factor_name == "Vol:VOL-A")
            .expect("first vol surface factor must be present");
        let factor_b = result
            .factors
            .iter()
            .find(|f| f.factor_name == "Vol:VOL-B")
            .expect("second vol surface factor must not be silently dropped");

        // Linear payoff: vega = $10k/pt per surface; moves +2 / +4 points.
        assert!(
            (factor_a.explained_pnl - 20_000.0).abs() < 1.0,
            "VOL-A explained {}",
            factor_a.explained_pnl
        );
        assert!(
            (factor_b.explained_pnl - 40_000.0).abs() < 1.0,
            "VOL-B explained {}",
            factor_b.explained_pnl
        );
    }

    /// The vol move must be measured at the instrument's own reference
    /// expiry/strike when available: a front-up/back-down term-structure
    /// inversion averages to ~0 across the surface, but a front-expiry
    /// instrument experienced ≈ +4 vol points.
    #[test]
    fn vol_move_uses_instrument_reference_point() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);

        // Front +4 pts, back −4 pts: surface-average move ≈ 0.
        let market_t0 =
            MarketContext::new().insert_surface(two_expiry_surface("VOL-TS", 0.20, 0.28));
        let market_t1 =
            MarketContext::new().insert_surface(two_expiry_surface("VOL-TS", 0.24, 0.24));

        let mut mock = MockInstrument::new(
            "FRONT-VOL",
            MockPayoff::VolSum {
                reads: vec![("VOL-TS", 0.5, 100.0)],
                scale: 1_000_000.0,
            },
        );
        mock.vol_deps = vec![VolatilityDependency::new("VOL-TS", None, Some(100.0))];
        // 183 days ≈ 0.5y to expiry — the front of the surface.
        mock.expiry = Some(date!(2025 - 07 - 17));
        let instrument: Arc<dyn Instrument> = Arc::new(mock);

        let config = TaylorAttributionConfig::default();
        let result = compute_taylor_result(
            &instrument,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
            &config,
            TaylorExecution {
                policy: ExecutionPolicy::Serial,
                prepared_endpoints: None,
            },
            None,
        )
        .expect("taylor attribution should succeed");

        let factor = result
            .factors
            .iter()
            .find(|f| f.factor_name == "Vol:VOL-TS")
            .expect("vol factor must be present");
        assert!(
            (factor.market_move - 4.0).abs() < 0.1,
            "vol move must be measured at the instrument's front expiry (≈ +4 pts), got {}",
            factor.market_move
        );
    }

    /// Without a reference expiry/strike the vol move stays surface-averaged,
    /// and the attribution metadata must say so.
    #[test]
    fn vol_without_reference_point_notes_surface_average() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);

        let market_t0 =
            MarketContext::new().insert_surface(two_expiry_surface("VOL-X", 0.20, 0.20));
        let market_t1 =
            MarketContext::new().insert_surface(two_expiry_surface("VOL-X", 0.22, 0.22));

        let mut mock = MockInstrument::new(
            "NO-REF-VOL",
            MockPayoff::VolSum {
                reads: vec![("VOL-X", 1.0, 100.0)],
                scale: 1_000_000.0,
            },
        );
        mock.vol_deps = vec![VolatilityDependency::new("VOL-X", None, None)];
        let instrument: Arc<dyn Instrument> = Arc::new(mock);

        let attribution = crate::attribute_pnl(
            &AttributionMethod::Taylor(TaylorAttributionConfig::default()),
            &crate::AttributionRequest {
                execution_policy: ExecutionPolicy::Serial,
                ..crate::AttributionRequest::new(
                    &instrument,
                    &market_t0,
                    &market_t1,
                    as_of_t0,
                    as_of_t1,
                    &finstack_quant_core::config::FinstackConfig::default(),
                )
            },
        )
        .expect("taylor attribution should succeed");

        assert!(
            attribution
                .meta
                .notes
                .iter()
                .any(|n| n.contains("VOL-X") && n.contains("surface-averaged")),
            "metadata must note that the vol move is surface-averaged, got {:?}",
            attribution.meta.notes
        );
    }

    /// Factor failures must be visible: a curve present in market dependencies
    /// but missing from one market must be recorded in metadata notes and flag
    /// the result invalid — while the run still completes.
    #[test]
    fn failed_factor_is_recorded_in_notes_and_flags_invalid() {
        use finstack_quant_core::math::interp::InterpStyle;

        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);

        let instrument: Arc<dyn Instrument> = Arc::new(
            TestInstrument::new("FAIL-001", Money::from((1000_i64, Currency::USD)))
                .with_discount_curves(&["USD-OIS"]),
        );

        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of_t0)
            .knots(vec![(0.0, 1.0), (30.0, 0.5)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("discount curve");
        let market_t0 = MarketContext::new().insert(curve);
        // T1 lacks the curve entirely → the rates factor must fail.
        let market_t1 = MarketContext::new();

        let attribution = crate::attribute_pnl(
            &AttributionMethod::Taylor(TaylorAttributionConfig::default()),
            &crate::AttributionRequest {
                execution_policy: ExecutionPolicy::Serial,
                ..crate::AttributionRequest::new(
                    &instrument,
                    &market_t0,
                    &market_t1,
                    as_of_t0,
                    as_of_t1,
                    &finstack_quant_core::config::FinstackConfig::default(),
                )
            },
        )
        .expect("attribution must complete despite the failed factor");

        assert!(
            attribution
                .meta
                .notes
                .iter()
                .any(|n| n.contains("USD-OIS") && n.contains("failed")),
            "failed factor must be recorded in metadata notes, got {:?}",
            attribution.meta.notes
        );
        assert!(
            attribution.result_invalid,
            "a failed dependency-backed factor must flag the result invalid"
        );
    }

    /// Bumps below the second-difference noise floor must be rejected at
    /// validation (Press et al., Numerical Recipes §5.7).
    #[test]
    fn taylor_rejects_sub_noise_floor_bumps() {
        for bad in [
            TaylorAttributionConfig {
                rate_bump_bp: 1e-6,
                ..TaylorAttributionConfig::default()
            },
            TaylorAttributionConfig {
                rate_bump_bp: 0.009,
                ..TaylorAttributionConfig::default()
            },
            TaylorAttributionConfig {
                credit_bump_bp: 1e-6,
                ..TaylorAttributionConfig::default()
            },
            TaylorAttributionConfig {
                vol_bump: 1e-5,
                ..TaylorAttributionConfig::default()
            },
        ] {
            assert!(
                bad.validate().is_err(),
                "sub-noise-floor bump must fail validation: {bad:?}"
            );
        }

        // Boundary values remain valid.
        let ok = TaylorAttributionConfig {
            rate_bump_bp: 0.01,
            credit_bump_bp: 0.01,
            vol_bump: 1e-4,
            ..TaylorAttributionConfig::default()
        };
        assert!(ok.validate().is_ok());
    }

    /// `unexplained` must be computed on the same total-return basis as the
    /// explained factors: a period coupon flows into theta's explained P&L, so
    /// it must also be part of `actual_pnl`.
    #[test]
    fn unexplained_uses_total_return_actual_pnl() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 02 - 15);

        let mut mock = MockInstrument::new("COUPON-001", MockPayoff::Constant(1_000_000.0));
        mock.coupon = Some((
            date!(2025 - 02 - 01),
            Money::from((5_000_i64, Currency::USD)),
        ));
        let instrument: Arc<dyn Instrument> = Arc::new(mock);

        let result = compute_taylor_result(
            &instrument,
            &MarketContext::new(),
            &MarketContext::new(),
            as_of_t0,
            as_of_t1,
            &TaylorAttributionConfig::default(),
            TaylorExecution {
                policy: ExecutionPolicy::Serial,
                prepared_endpoints: None,
            },
            None,
        )
        .expect("taylor attribution should succeed");

        assert!(!result.result_invalid);
        assert_eq!(result.theta_coupon_income, Some(5_000.0));
        assert!(
            (result.actual_pnl - 5_000.0).abs() < 1e-6,
            "actual_pnl must include the period coupon (total-return basis), got {}",
            result.actual_pnl
        );
        assert!(
            result.unexplained.abs() < 1e-6,
            "coupon income must not bias unexplained, got {}",
            result.unexplained
        );
    }

    #[test]
    fn theta_cashflow_failures_are_recorded_without_a_successful_theta() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 02 - 15);
        let market = MarketContext::new();
        let config = TaylorAttributionConfig::default();
        let finstack_config = finstack_quant_core::config::FinstackConfig::default();

        for (cashflow_error, coupon_currency, expected_error) in [
            (
                Some("coupon schedule unavailable"),
                Currency::USD,
                "coupon schedule unavailable",
            ),
            (None, Currency::EUR, "Theta cashflow currency mismatch"),
        ] {
            let mut mock = MockInstrument::new("FAILED-COUPON", MockPayoff::Constant(1_000_000.0));
            mock.coupon = Some((
                date!(2025 - 02 - 01),
                Money::from((5_000_i64, coupon_currency)),
            ));
            mock.cashflow_error = cashflow_error;
            let instrument: Arc<dyn Instrument> = Arc::new(mock);

            let result = compute_taylor_result(
                &instrument,
                &market,
                &market,
                as_of_t0,
                as_of_t1,
                &config,
                TaylorExecution {
                    policy: ExecutionPolicy::Serial,
                    prepared_endpoints: None,
                },
                None,
            )
            .expect("failed period cashflow collection should retain diagnostic attribution");

            assert!(result.result_invalid);
            assert_eq!(result.theta_coupon_income, None);
            assert!(result
                .factors
                .iter()
                .all(|factor| factor.factor_name != "Theta"));
            assert!(result.notes.iter().any(|note| {
                note.contains(expected_error)
                    && note.contains("actual_pnl excludes period coupon income")
            }));

            let attribution = crate::attribute_pnl(
                &AttributionMethod::Taylor(config.clone()),
                &crate::AttributionRequest {
                    execution_policy: ExecutionPolicy::Serial,
                    ..crate::AttributionRequest::new(
                        &instrument,
                        &market,
                        &market,
                        as_of_t0,
                        as_of_t1,
                        &finstack_config,
                    )
                },
            )
            .expect("public attribution should preserve the failed-factor diagnostics");

            assert!(attribution.result_invalid);
            assert!(attribution.carry_detail.is_none());
            assert!(attribution
                .meta
                .notes
                .iter()
                .any(|note| note.contains(expected_error)));
        }
    }

    /// The FX-exposure factor must run when either side carries an FX matrix;
    /// previously a missing T0 matrix silently skipped it even though T1 had
    /// FX-driven P&L.
    #[test]
    fn fx_factor_runs_when_only_t1_has_fx() {
        use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
        use finstack_quant_core::Error;

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

        let instrument: Arc<dyn Instrument> = Arc::new(MockInstrument::new(
            "FX-T1-ONLY",
            MockPayoff::FxOptional {
                eur_notional: 1_000_000.0,
            },
        ));

        let market_t0 = MarketContext::new();
        let market_t1 = MarketContext::new().insert_fx(FxMatrix::new(Arc::new(FixedFx(1.20))));

        let result = compute_taylor_result(
            &instrument,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
            &TaylorAttributionConfig::default(),
            TaylorExecution {
                policy: ExecutionPolicy::Serial,
                prepared_endpoints: None,
            },
            None,
        )
        .expect("taylor attribution should succeed");

        let fx = result
            .factors
            .iter()
            .find(|f| f.factor_name == "Fx")
            .expect("Fx factor must run when only T1 carries an FX matrix");
        assert!(
            (fx.explained_pnl - 200_000.0).abs() < 1e-6,
            "Fx factor must capture the T1 FX-driven P&L, got {}",
            fx.explained_pnl
        );
    }

    #[test]
    fn taylor_buckets_spot_move_into_market_scalars_pnl() {
        use finstack_quant_core::market_data::scalars::MarketScalar;

        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);
        let instrument: Arc<dyn Instrument> = Arc::new(MockInstrument::new(
            "SPOT-001",
            MockPayoff::Spot {
                id: "AAPL",
                scale: 100.0,
            },
        ));
        let market_t0 = MarketContext::new().insert_price(
            "AAPL",
            MarketScalar::Price(Money::from((180_i64, Currency::USD))),
        );
        let market_t1 = MarketContext::new().insert_price(
            "AAPL",
            MarketScalar::Price(Money::from((185_i64, Currency::USD))),
        );

        let attribution = crate::attribute_pnl(
            &AttributionMethod::Taylor(TaylorAttributionConfig::default()),
            &crate::AttributionRequest {
                execution_policy: ExecutionPolicy::Serial,
                ..crate::AttributionRequest::new(
                    &instrument,
                    &market_t0,
                    &market_t1,
                    as_of_t0,
                    as_of_t1,
                    &finstack_quant_core::config::FinstackConfig::default(),
                )
            },
        )
        .expect("taylor attribution should succeed");

        // 100 shares × $5 spot move = $500, previously dumped into residual.
        assert!(
            (attribution.market_scalars_pnl.amount() - 500.0).abs() < 1e-6,
            "market_scalars_pnl = {}",
            attribution.market_scalars_pnl
        );
        assert!(
            attribution.residual.amount().abs() < 1e-6,
            "residual should shrink once the spot move is attributed, got {}",
            attribution.residual
        );
    }

    #[test]
    fn taylor_buckets_inflation_curve_move_into_inflation_pnl() {
        use finstack_quant_core::market_data::term_structures::InflationCurve;

        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);
        let infl = |cpi_5y| {
            InflationCurve::builder("US-CPI")
                .base_date(as_of_t0)
                .base_cpi(300.0)
                .knots([(0.0, 300.0), (5.0, cpi_5y)])
                .build()
                .expect("inflation curve")
        };
        let instrument: Arc<dyn Instrument> = Arc::new(MockInstrument::new(
            "INFL-001",
            MockPayoff::InflationCpi {
                curve: "US-CPI",
                tenor: 5.0,
                scale: 1_000.0,
            },
        ));
        let market_t0 = MarketContext::new().insert(infl(325.0));
        let market_t1 = MarketContext::new().insert(infl(350.0));

        let attribution = crate::attribute_pnl(
            &AttributionMethod::Taylor(TaylorAttributionConfig::default()),
            &crate::AttributionRequest {
                execution_policy: ExecutionPolicy::Serial,
                ..crate::AttributionRequest::new(
                    &instrument,
                    &market_t0,
                    &market_t1,
                    as_of_t0,
                    as_of_t1,
                    &finstack_quant_core::config::FinstackConfig::default(),
                )
            },
        )
        .expect("taylor attribution should succeed");

        assert!(
            (attribution.inflation_curves_pnl.amount() - 25_000.0).abs() < 1e-6,
            "inflation_curves_pnl = {}",
            attribution.inflation_curves_pnl
        );
        assert!(
            attribution.residual.amount().abs() < 1e-6,
            "residual should shrink once the inflation move is attributed, got {}",
            attribution.residual
        );
    }

    #[test]
    fn taylor_buckets_correlation_move_into_correlations_pnl() {
        use finstack_quant_core::market_data::term_structures::BaseCorrelationCurve;

        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);
        let corr = |mid| {
            BaseCorrelationCurve::builder("CDX")
                .knots(vec![(3.0, 0.20), (7.0, mid), (10.0, 0.60)])
                .build()
                .expect("base correlation curve")
        };
        let instrument: Arc<dyn Instrument> = Arc::new(MockInstrument::new(
            "CORR-001",
            MockPayoff::Correlation {
                curve: "CDX",
                detachment: 7.0,
                scale: 1_000_000.0,
            },
        ));
        let market_t0 = MarketContext::new().insert(corr(0.40));
        let market_t1 = MarketContext::new().insert(corr(0.50));

        let attribution = crate::attribute_pnl(
            &AttributionMethod::Taylor(TaylorAttributionConfig::default()),
            &crate::AttributionRequest {
                execution_policy: ExecutionPolicy::Serial,
                ..crate::AttributionRequest::new(
                    &instrument,
                    &market_t0,
                    &market_t1,
                    as_of_t0,
                    as_of_t1,
                    &finstack_quant_core::config::FinstackConfig::default(),
                )
            },
        )
        .expect("taylor attribution should succeed");

        assert!(
            (attribution.correlations_pnl.amount() - 100_000.0).abs() < 1e-6,
            "correlations_pnl = {}",
            attribution.correlations_pnl
        );
        assert!(
            attribution.residual.amount().abs() < 1e-6,
            "residual should shrink once the correlation move is attributed, got {}",
            attribution.residual
        );
    }

    #[test]
    fn taylor_buckets_model_param_move_into_model_params_pnl() {
        use finstack_quant_cashflows::builder::{
            DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec,
        };

        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);
        let mut instrument =
            MockInstrument::new("MODEL-001", MockPayoff::ModelCpr { scale: 1_000_000.0 });
        instrument.model_cpr = Some(0.08);
        let instrument: Arc<dyn Instrument> = Arc::new(instrument);
        let params_t0 = ModelParamsSnapshot::StructuredCredit {
            prepayment_spec: PrepaymentModelSpec::constant_cpr(0.06),
            default_spec: DefaultModelSpec::constant_cdr(0.0),
            recovery_spec: RecoveryModelSpec::with_lag(0.0, 0),
        };

        let attribution = crate::attribute_pnl(
            &AttributionMethod::Taylor(TaylorAttributionConfig::default()),
            &crate::AttributionRequest {
                model_params_t0: Some(&params_t0),
                ..crate::AttributionRequest::new(
                    &instrument,
                    &MarketContext::new(),
                    &MarketContext::new(),
                    as_of_t0,
                    as_of_t1,
                    &finstack_quant_core::config::FinstackConfig::default(),
                )
            },
        )
        .expect("taylor attribution should succeed");

        // scale × (0.08 − 0.06) = 20_000, previously residual.
        assert!(
            (attribution.model_params_pnl.amount() - 20_000.0).abs() < 1e-6,
            "model_params_pnl = {}",
            attribution.model_params_pnl
        );
        assert!(
            attribution.residual.amount().abs() < 1e-6,
            "residual should shrink once the model-param move is attributed, got {}",
            attribution.residual
        );
    }
}
