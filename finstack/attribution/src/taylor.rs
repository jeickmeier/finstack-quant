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

use super::factors::{MarketRestoreFlags, MarketSnapshot};
use super::helpers::*;
use super::metrics_based::extract_keyrate_cs01_per_curve;
use super::types::*;
use finstack_core::dates::Date;
use finstack_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::diff::{
    measure_credit_curve_shift, measure_per_tenor_credit_curve_shift, measure_vol_surface_shift,
    TenorSamplingMethod,
};
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use finstack_core::Result;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::bump_surface_vol_absolute;
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

impl TaylorAttributionConfig {
    /// Validates configuration parameters.
    pub fn validate(&self) -> Result<()> {
        if self.rate_bump_bp <= 0.0 || self.rate_bump_bp > 100.0 {
            return Err(finstack_core::Error::Validation(format!(
                "Rate bump size must be strictly positive and no greater than 100bp, got {:.4}",
                self.rate_bump_bp
            )));
        }
        if self.credit_bump_bp <= 0.0 || self.credit_bump_bp > 100.0 {
            return Err(finstack_core::Error::Validation(format!(
                "Credit bump size must be strictly positive and no greater than 100bp, got {:.4}",
                self.credit_bump_bp
            )));
        }
        if self.vol_bump <= 0.0 || self.vol_bump > 0.20 {
            return Err(finstack_core::Error::Validation(format!(
                "Volatility bump size must be strictly positive and no greater than 20% (0.20), got {:.4}",
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
fn record_taylor_factor_result(
    factor_kind: &str,
    factor_id: &CurveId,
    result: Result<TaylorFactorResult>,
    factors: &mut Vec<TaylorFactorResult>,
    total_explained: &mut f64,
    num_repricings: &mut usize,
    repricings: usize,
) {
    match result {
        Ok(result) => {
            *total_explained += result.explained_pnl;
            if let Some(g) = result.gamma_pnl {
                *total_explained += g;
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
pub(crate) struct TaylorFactorResult {
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
pub(crate) struct TaylorAttributionResult {
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
    /// Coupon income for the theta period, captured by `compute_theta_factor`.
    /// `None` when theta computation failed; otherwise lets `attribute_pnl_taylor`
    /// split theta into PV-only and coupon components without re-collecting
    /// cashflows (audit MO3 fix).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theta_coupon_income: Option<f64>,
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
fn compute_taylor_result(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    config: &TaylorAttributionConfig,
) -> Result<TaylorAttributionResult> {
    config.validate()?;
    validate_attribution_period(as_of_t0, as_of_t1)?;
    let pv_t0 = reprice_instrument(instrument, market_t0, as_of_t0)?;
    let pv_t1 = reprice_instrument(instrument, market_t1, as_of_t1)?;
    // Decimal-exact difference: subtracting two large `.amount()` f64s loses
    // precision at high notionals, and `checked_sub` also rejects a currency
    // mismatch instead of silently differencing across currencies.
    let actual_pnl = pv_t1.checked_sub(pv_t0)?.amount();

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
            2 * KEY_RATE_BUCKETS_YEARS.len(),
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
            2 * KEY_RATE_BUCKETS_YEARS.len(),
        );
    }

    // Credit sensitivities — credit-curve move, key-rate aware.
    //
    // Hazard curves are measured in par CDS spread moves; discount-style credit
    // curves (for example convertible risky discount curves) are measured in zero
    // rate moves. `BucketedCs01` is requested once here; instruments without that
    // calculator yield no per-tenor keys and the per-curve `compute_credit_factor`
    // falls back to an aggregate CS01 times an average credit-curve move.
    let credit_curves = &market_deps.curve_dependencies().credit_curves;
    let credit_keyrate = if credit_curves.is_empty() {
        None
    } else {
        instrument
            .price_with_metrics(
                market_t0,
                as_of_t0,
                &[finstack_valuations::metrics::MetricId::BucketedCs01],
                finstack_valuations::instruments::PricingOptions::default(),
            )
            .ok()
            .map(|vr| extract_keyrate_cs01_per_curve(&vr.measures, credit_curves))
    };
    let credit_results = credit_curves
        .par_iter()
        .map(|curve_id| {
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
            2,
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
            2,
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

    // Theta (time decay): reprice at T1 date with T0 market. The outcome
    // also carries `coupon_income` so `attribute_pnl_taylor` can split theta
    // into PV-only and coupon components without re-collecting cashflows.
    let mut theta_coupon_income: Option<f64> = None;
    match compute_theta_factor(instrument, market_t0, as_of_t0, as_of_t1, pv_t0) {
        Ok(outcome) => {
            let ThetaFactorOutcome {
                factor: result,
                coupon_income,
            } = outcome;
            total_explained += result.explained_pnl;
            num_repricings += 1;
            theta_coupon_income = Some(coupon_income);
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
        theta_coupon_income,
    })
}

/// Compute Taylor-based P&L attribution.
///
/// This maps Taylor factors into the standard `PnlAttribution` struct so Taylor
/// output can be used interchangeably with parallel/waterfall results.
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
pub fn attribute_pnl_taylor(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    config: &TaylorAttributionConfig,
) -> Result<PnlAttribution> {
    let taylor =
        compute_taylor_result(instrument, market_t0, market_t1, as_of_t0, as_of_t1, config)?;

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

        // MO5: route accumulation through Money::checked_add so a currency
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
        } else if factor.factor_name == "Theta" {
            // Taylor theta already includes cashflows from compute_theta_factor.
            // Re-use the coupon income that was captured during that compute
            // (audit MO3: previously we re-called collect_cashflows_in_period
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
            let theta_only = Money::new(factor_money.amount() - ci.amount(), ccy);
            apply_total_return_carry(&mut attribution, theta_only, ci, None)?;
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
        "Taylor coverage: rates/credit/vol/FX-exposure/theta. Market-scalar \
         (spot/dividend/index) sensitivities are not computed; their P&L (if \
         any) remains in residual."
            .to_string(),
    );

    Ok(attribution)
}

// ─── Helper functions ──────────────────────────────────────────────────────

// NOTE (audit item #3): the former `measure_forward_curve_shift` /
// `measure_average_rate_shift` helpers — an unweighted mean of per-tenor shifts
// — were removed. An unweighted average mis-attributes non-parallel curve
// moves (a steepener averages toward zero), so `compute_rate_factor` and
// `compute_forward_factor` now measure the per-tenor move and pair it with a
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
    /// Per-bucket DV01 gamma (currency / bp²), only when `include_gamma`.
    gamma: f64,
    /// Realized zero-rate move at this bucket's tenor (basis points).
    move_bp: f64,
}

/// Triangular key-rate bump spec for bucket `i` of `KEY_RATE_BUCKETS_YEARS`.
fn key_rate_bump_spec(i: usize, bump_bp: f64) -> BumpSpec {
    let prev = if i == 0 {
        0.0
    } else {
        KEY_RATE_BUCKETS_YEARS[i - 1]
    };
    let target = KEY_RATE_BUCKETS_YEARS[i];
    let next = if i + 1 == KEY_RATE_BUCKETS_YEARS.len() {
        f64::INFINITY
    } else {
        KEY_RATE_BUCKETS_YEARS[i + 1]
    };
    BumpSpec::triangular_key_rate_bp(prev, target, next, bump_bp)
}

/// Compute rate (DV01) attribution for a single discount curve — KEY-RATE
/// AWARE.
///
/// Rather than a single parallel DV01 multiplied by an *average* curve shift
/// (which mis-attributes non-parallel moves — a steepener averages toward zero
/// and inflates the unexplained residual), this bumps each standard key-rate
/// bucket with a triangular weight, measures the DV01 of that bucket, and pairs
/// it with the realized zero-rate move at that bucket's tenor:
///
///   explained = Σ_bucket  DV01_bucket × Δr_bucket
///
/// The reported `sensitivity` is the parallel-equivalent DV01 (Σ bucket DV01s)
/// and `market_move` the average shift used by the internal factor result.
fn compute_rate_factor(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    pv_t0: Money,
    curve_id: &CurveId,
    config: &TaylorAttributionConfig,
) -> Result<TaylorFactorResult> {
    // Realized per-tenor zero-rate moves on the standard bucket grid (bp).
    let (curve_t0, curve_t1) = (
        market_t0.get_discount(curve_id.as_str())?,
        market_t1.get_discount(curve_id.as_str())?,
    );
    let per_tenor_move_bp: Vec<f64> = KEY_RATE_BUCKETS_YEARS
        .iter()
        .map(|&t| (curve_t1.zero(t) - curve_t0.zero(t)) * 10_000.0)
        .collect();

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
        let gamma = if config.include_gamma {
            (pv_up.amount() - 2.0 * pv_t0.amount() + pv_down.amount())
                / (config.rate_bump_bp * config.rate_bump_bp)
        } else {
            0.0
        };
        buckets.push(KeyRateBucket {
            dv01,
            gamma,
            move_bp,
        });
    }

    // Key-rate-aware explained P&L: Σ DV01_bucket × Δr_bucket. Compensated
    // summation keeps the per-bucket accumulation stable.
    let explained = finstack_core::math::neumaier_sum(buckets.iter().map(|b| b.dv01 * b.move_bp));
    let total_dv01 = finstack_core::math::neumaier_sum(buckets.iter().map(|b| b.dv01));
    let avg_move_bp = if buckets.is_empty() {
        0.0
    } else {
        finstack_core::math::neumaier_sum(buckets.iter().map(|b| b.move_bp)) / buckets.len() as f64
    };

    // Second-order term, also key-rate aware: Σ ½ γ_bucket × Δr_bucket².
    let gamma_pnl = if config.include_gamma {
        Some(finstack_core::math::neumaier_sum(
            buckets
                .iter()
                .map(|b| 0.5 * b.gamma * b.move_bp * b.move_bp),
        ))
    } else {
        None
    };

    Ok(TaylorFactorResult {
        factor_name: format!("Rates:{}", curve_id),
        sensitivity: total_dv01,
        market_move: avg_move_bp,
        explained_pnl: explained,
        gamma_pnl,
    })
}

/// Compute forward-curve sensitivity attribution for a single forward curve —
/// KEY-RATE AWARE.
///
/// Mirrors [`compute_rate_factor`] but applies triangular key-rate bumps to the
/// forward curve and measures the realized move using forward rates (not
/// discount zeros). A non-parallel forward-curve move is attributed per bucket
/// rather than collapsing to an average shift × parallel DV01.
fn compute_forward_factor(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    pv_t0: Money,
    curve_id: &CurveId,
    config: &TaylorAttributionConfig,
) -> Result<TaylorFactorResult> {
    let (curve_t0, curve_t1) = (
        market_t0.get_forward(curve_id.as_str())?,
        market_t1.get_forward(curve_id.as_str())?,
    );
    let per_tenor_move_bp: Vec<f64> = KEY_RATE_BUCKETS_YEARS
        .iter()
        .map(|&t| (curve_t1.rate(t) - curve_t0.rate(t)) * 10_000.0)
        .collect();

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

        let dv01 = (pv_up.amount() - pv_down.amount()) / (2.0 * config.rate_bump_bp);
        let gamma = if config.include_gamma {
            (pv_up.amount() - 2.0 * pv_t0.amount() + pv_down.amount())
                / (config.rate_bump_bp * config.rate_bump_bp)
        } else {
            0.0
        };
        buckets.push(KeyRateBucket {
            dv01,
            gamma,
            move_bp,
        });
    }

    let explained = finstack_core::math::neumaier_sum(buckets.iter().map(|b| b.dv01 * b.move_bp));
    let total_dv01 = finstack_core::math::neumaier_sum(buckets.iter().map(|b| b.dv01));
    let avg_move_bp = if buckets.is_empty() {
        0.0
    } else {
        finstack_core::math::neumaier_sum(buckets.iter().map(|b| b.move_bp)) / buckets.len() as f64
    };

    let gamma_pnl = if config.include_gamma {
        Some(finstack_core::math::neumaier_sum(
            buckets
                .iter()
                .map(|b| 0.5 * b.gamma * b.move_bp * b.move_bp),
        ))
    } else {
        None
    };

    Ok(TaylorFactorResult {
        factor_name: format!("Forward:{}", curve_id),
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
        let explained = finstack_core::math::neumaier_sum(
            buckets
                .iter()
                .zip(shifts.iter())
                .map(|((_, cs01), shift)| cs01 * shift),
        );
        let total_cs01 = finstack_core::math::neumaier_sum(buckets.iter().map(|(_, c)| *c));
        let avg_move = if shifts.is_empty() {
            0.0
        } else {
            finstack_core::math::neumaier_sum(shifts.iter().copied()) / shifts.len() as f64
        };
        return Ok(TaylorFactorResult {
            factor_name: format!("Credit:{}", curve_id),
            sensitivity: total_cs01,
            market_move: avg_move,
            explained_pnl: explained,
            // Key-rate path is first-order only; per-tenor CS-gamma is not
            // modelled. `include_gamma` credit convexity is available via the
            // parallel-bump fallback below.
            gamma_pnl: None,
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
    use finstack_valuations::metrics::collect_cashflows_in_period;

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

        let result = compute_taylor_result(
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

        let attribution = attribute_pnl_taylor(
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
        let result = compute_taylor_result(
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

    finstack_valuations::impl_empty_cashflow_provider!(
        FxLinkedInstrument,
        finstack_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl Instrument for FxLinkedInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> finstack_valuations::pricer::InstrumentType {
            finstack_valuations::pricer::InstrumentType::Bond
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn attributes(&self) -> &finstack_valuations::instruments::Attributes {
            use std::sync::OnceLock;
            static ATTRS: OnceLock<finstack_valuations::instruments::Attributes> = OnceLock::new();
            ATTRS.get_or_init(finstack_valuations::instruments::Attributes::default)
        }

        fn attributes_mut(&mut self) -> &mut finstack_valuations::instruments::Attributes {
            unreachable!("FxLinkedInstrument::attributes_mut should not be called")
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn market_dependencies(
            &self,
        ) -> finstack_core::Result<finstack_valuations::instruments::MarketDependencies> {
            Ok(finstack_valuations::instruments::MarketDependencies::new())
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
            _metrics: &[finstack_valuations::metrics::MetricId],
            _options: finstack_valuations::instruments::PricingOptions,
        ) -> Result<finstack_valuations::results::ValuationResult> {
            Ok(finstack_valuations::results::ValuationResult::stamped(
                self.id(),
                as_of,
                self.value(market, as_of)?,
            ))
        }
    }

    #[test]
    fn taylor_buckets_fx_exposure_into_fx_pnl() {
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
        let attribution = attribute_pnl_taylor(
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

        // The internal Taylor factor decomposition should also expose an "Fx" factor.
        let taylor = compute_taylor_result(
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

    /// MO4 regression: malformed config bumps (≤ 0 or > sane max) must be
    /// rejected at validation rather than producing a `result_invalid`
    /// flagged result. Before MO4 the central-difference DV01 was a 0/0 NaN
    /// and the attribution flagged itself invalid; with the strengthened
    /// validation the caller now gets an immediate `Error::Validation`.
    #[test]
    fn taylor_rejects_non_positive_bump_at_validation() {
        use finstack_core::market_data::term_structures::DiscountCurve;
        use finstack_core::math::interp::InterpStyle;

        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);

        let instrument: Arc<dyn Instrument> = Arc::new(
            TestInstrument::new("NF-001", Money::new(1000.0, Currency::USD))
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
            let err = attribute_pnl_taylor(
                &instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &bad,
            )
            .expect_err("malformed bump config must error at validation");
            let msg = format!("{err}");
            assert!(
                msg.to_lowercase().contains("bump"),
                "validation error must mention 'bump', got: {msg}"
            );
        }
    }

    // Audit N2 (originally `taylor_flags_non_finite_factor_instead_of_panicking`):
    // covered redundantly by the MO4 validation test above
    // (`taylor_rejects_non_positive_bump_at_validation`) and by the
    // metrics-based NaN-flagging test
    // (`metrics_based::tests::nan_factor_sensitivity_sets_result_invalid_instead_of_panicking`).
    // The MO4 change closed the original trigger (zero bump → 0/0 NaN) at the
    // boundary; constructing an alternative NaN-producing pricer would
    // duplicate the metrics-based coverage without exercising any new code.
}
