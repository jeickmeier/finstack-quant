//! [`TaylorAttributionConfig`] and top-level Taylor-expansion attribution orchestration.

use super::factors::*;
use crate::AttributionRequest;
use crate::factors::{MarketRestoreFlags, MarketSnapshot};
use crate::metrics_based::shifts::extract_credit_keyrates;
use crate::model_params::{ModelParamsSnapshot, extract_model_params};
use crate::pnl_support::*;
use crate::{AttributionFactor, AttributionMethod, PnlAttribution};
use finstack_quant_calibration::recalibration::CachedRecalibrationProvider;
use finstack_quant_core::Result;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use serde::{Deserialize, Serialize};

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
    #[serde(default = "default_credit_spread_bump_bp")]
    pub credit_spread_bump_bp: f64,

    /// Vol bump size for vega computation (absolute vol points, e.g. 0.01 = 1%).
    #[serde(default = "default_vol_bump")]
    pub vol_bump: f64,
}

pub(super) fn default_rate_bump_bp() -> f64 {
    1.0
}
pub(super) fn default_credit_spread_bump_bp() -> f64 {
    1.0
}
pub(super) fn default_vol_bump() -> f64 {
    0.01
}

impl Default for TaylorAttributionConfig {
    fn default() -> Self {
        Self {
            include_gamma: false,
            rate_bump_bp: default_rate_bump_bp(),
            credit_spread_bump_bp: default_credit_spread_bump_bp(),
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
        if !self.rate_bump_bp.is_finite() || !(0.01..=100.0).contains(&self.rate_bump_bp) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Rate bump size must lie in [0.01, 100] bp (below 0.01bp the second-difference \
                 gamma is cancellation noise), got {:.6}",
                self.rate_bump_bp
            )));
        }
        if !self.credit_spread_bump_bp.is_finite()
            || !(0.01..=100.0).contains(&self.credit_spread_bump_bp)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Credit bump size must lie in [0.01, 100] bp (below 0.01bp the second-difference \
                 gamma is cancellation noise), got {:.6}",
                self.credit_spread_bump_bp
            )));
        }
        if !self.vol_bump.is_finite() || !(1e-4..=0.20).contains(&self.vol_bump) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Volatility bump size must lie in [1e-4, 0.20] absolute vol (below 1e-4 the \
                 second-difference volga is cancellation noise), got {:.6}",
                self.vol_bump
            )));
        }
        Ok(())
    }
}

impl TaylorAttributionResult {
    /// Record a Taylor factor result and its attempted pricing calls, including
    /// calls that returned an error before the factor completed.
    ///
    /// On failure the factor is recorded in `notes` and `result_invalid` is set:
    /// every factor routed through here is backed by a curve/surface that appears
    /// in the instrument's market dependencies, so a failure means part of the
    /// declared risk decomposition is silently missing and the result cannot be
    /// trusted.
    pub(super) fn record(
        &mut self,
        factor_kind: &str,
        factor_id: &str,
        result: Result<TaylorFactorResult>,
        repricings: usize,
    ) {
        self.num_repricings += repricings;
        match result {
            Ok(result) => self.factors.push(result),
            Err(e) => {
                tracing::warn!(
                    factor_kind = factor_kind,
                    factor_id,
                    error = %e,
                    "Taylor attribution: factor computation failed"
                );
                self.notes.push(format!(
                    "Taylor {factor_kind} factor '{factor_id}' failed: {e}"
                ));
                self.result_invalid = true;
            }
        }
    }
}

/// One Taylor factor's first-order and optional second-order P&L.
#[derive(Debug, Clone)]
pub(super) struct TaylorFactorResult {
    /// Attribution bucket the factor's P&L lands in (`Carry` for theta).
    pub factor: AttributionFactor,
    /// Diagnostic factor label, including its curve or volatility input ID.
    pub factor_name: String,
    /// First-order explained P&L. For key-rate factors this is the per-bucket
    /// sum `Σ sensitivityᵢ × moveᵢ` and is the authoritative number; for
    /// non-parallel moves it deliberately differs from
    /// `sensitivity × market_move`.
    pub explained_pnl: f64,
    /// Second-order P&L if requested. Key-rate curve factors measure curvature
    /// along the observed bucket moves, including cross-bucket effects.
    pub gamma_pnl: Option<f64>,
    /// Sensitivities and moves (or the repriced value) behind `explained_pnl`.
    pub working: FactorWorking,
}

/// Complete result of Taylor-based attribution.
#[derive(Debug, Clone)]
pub(super) struct TaylorAttributionResult {
    /// Per-factor breakdown.
    pub factors: Vec<TaylorFactorResult>,
    /// Number of repricings performed (bump-and-reprice calls).
    pub num_repricings: usize,
    /// Present value at T0.
    pub pv_t0: Money,
    /// Present value at T1.
    pub pv_t1: Money,
    /// Coupon income for the theta period, captured by `compute_theta_factor`.
    /// `None` when theta computation failed; otherwise lets `attribute_pnl_taylor`
    /// split theta into PV-only and coupon components without re-collecting
    /// cashflows.
    pub theta_coupon_income: Option<Money>,
    /// All realized period receipts, including returned principal.
    pub theta_cash_paid: Option<Money>,
    /// Diagnostic notes accumulated during factor computation (failed factors,
    /// missing volatility references, missing T0 FX). Threaded into
    /// `PnlAttribution::meta.notes` by `attribute_pnl_taylor`.
    pub notes: Vec<String>,
    /// True when a factor backed by a declared market dependency or the theta
    /// calculation failed. Part of the risk decomposition or period cash income
    /// is missing, so downstream consumers must not trust the residual split.
    pub result_invalid: bool,
}

/// Compute the detailed Taylor factor decomposition.
///
/// Uses bump-and-reprice at T0 to compute first-order sensitivities, then
/// multiplies by the observed market move between T0 and T1 to obtain
/// factor-level explained P&L.
///
/// # Arguments
///
/// * `request` - Instrument, markets, dates, execution policy, optional
///   opening model parameters and prepared endpoint values.
/// * `config` - Taylor bump sizes and gamma treatment.
///
/// # Returns
///
/// `TaylorAttributionResult` with factors, endpoints and pricing diagnostics.
pub(super) fn compute_taylor_result(
    request: &AttributionRequest<'_>,
    config: &TaylorAttributionConfig,
) -> Result<TaylorAttributionResult> {
    config.validate()?;
    let AttributionRequest {
        instrument,
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        execution_policy,
        model_params_t0,
        ..
    } = *request;
    let (instrument_t0, pv_t0, pv_t1) = endpoint_values(request)?;
    // Decimal-exact difference: subtracting two large `.amount()` f64s loses
    // precision at high notionals, and `checked_sub` also rejects a currency
    // mismatch instead of silently differencing across currencies.
    pv_t1.checked_sub(pv_t0)?;

    let mut acc = TaylorAttributionResult {
        factors: Vec::new(),
        num_repricings: 2,
        pv_t0,
        pv_t1,
        theta_coupon_income: None,
        theta_cash_paid: None,
        notes: Vec::new(),
        result_invalid: false,
    };

    // Rate sensitivities (key-rate DV01 per discount curve)
    let market_deps = instrument_t0.market_dependencies()?;
    // A risky discount curve belongs to the credit factor even though its
    // storage family also makes it a discount dependency.
    let rate_curve_ids: Vec<CurveId> = market_deps
        .curves
        .discount_curves
        .iter()
        .filter(|id| !market_deps.curves.credit_curves.contains(id))
        .cloned()
        .collect();
    let recalibration_provider = CachedRecalibrationProvider::default();
    // Key-rate DV01 per discount curve, then per projection curve.
    for (kind, factor_kind, curve_ids) in [
        (CurveKind::Discount, "rate", rate_curve_ids.as_slice()),
        (
            CurveKind::Forward,
            "forward",
            market_deps.curves.forward_curves.as_slice(),
        ),
    ] {
        let compute = |curve_id: &CurveId| {
            let mut repricings = 0;
            let result = compute_curve_factor(CurveFactorInputs {
                kind,
                instrument: &instrument_t0,
                market_t0,
                market_t1,
                as_of_t0,
                pv_t0,
                curve_id,
                config,
                repricings: &mut repricings,
            });
            (curve_id.clone(), result, repricings)
        };
        for (curve_id, result, repricings) in map_policy(execution_policy, curve_ids, compute) {
            acc.record(factor_kind, curve_id.as_str(), result, repricings);
        }
    }

    // Credit sensitivities — credit-curve move, key-rate aware.
    //
    // Hazard curves are measured in par CDS spread moves; discount-style credit
    // curves are measured in zero rate moves. `BucketedCs01` is requested once here; instruments without that
    // calculator yield no per-tenor keys and the per-curve `compute_credit_factor`
    // falls back to an aggregate CS01 times an average credit-curve move.
    let credit_curves = &market_deps.curves.credit_curves;
    let credit_keyrate = if credit_curves.is_empty() {
        None
    } else {
        acc.num_repricings += 1;
        instrument_t0
            .price_with_metrics(
                market_t0,
                as_of_t0,
                &[finstack_quant_valuations::metrics::MetricId::BucketedCs01],
                finstack_quant_calibration::recalibration::pricing_options(),
            )
            .and_then(|vr| {
                extract_credit_keyrates(
                    &vr.measures,
                    credit_curves,
                    market_t0,
                    market_t1,
                    &recalibration_provider,
                )
                .map_err(Into::into)
            })
            .map_err(|error| {
                acc.result_invalid = true;
                acc.notes.push(format!(
                    "Taylor bucketed credit sensitivities failed: {error}"
                ));
                error
            })
            .ok()
    };
    let compute_credit = |curve_id: &CurveId| {
        if credit_keyrate.is_none() {
            return (
                curve_id.clone(),
                Err(finstack_quant_core::Error::Validation(
                    "credit risk unavailable after bucket validation failed".into(),
                )),
                0,
            );
        }
        let keyrate = credit_keyrate
            .as_ref()
            .and_then(|m| m.get(curve_id))
            .map(|v| v.as_slice());
        let mut repricings = 0;
        let result = compute_credit_factor(CreditFactorInputs {
            instrument: &instrument_t0,
            market_t0,
            market_t1,
            as_of_t0,
            pv_t0,
            curve_id,
            config,
            keyrate,
            recalibration_provider: &recalibration_provider,
            repricings: &mut repricings,
        });
        (curve_id.clone(), result, repricings)
    };
    let credit_results = map_policy(execution_policy, credit_curves, compute_credit);
    for (curve_id, result, repricings) in credit_results {
        acc.record("credit", curve_id.as_str(), result, repricings);
    }

    // Measure each declared surface at its contractual expiry and strike.
    let surface_dependencies: Vec<_> = market_deps
        .volatility_dependencies
        .iter()
        .filter(|dep| {
            market_t0.get_surface(dep.vol_surface_id.as_str()).is_ok()
                || market_t1.get_surface(dep.vol_surface_id.as_str()).is_ok()
        })
        .cloned()
        .collect();
    let compute_vol =
        |dependency: &finstack_quant_valuations::instruments::VolatilityDependency| {
            let mut repricings = 0;
            let result = compute_vol_factor(VolFactorInputs {
                instrument: &instrument_t0,
                market_t0,
                market_t1,
                as_of_t0,
                pv_t0,
                dependency,
                config,
                repricings: &mut repricings,
            });
            (dependency.vol_surface_id.clone(), result, repricings)
        };
    let vol_results = map_policy(execution_policy, &surface_dependencies, compute_vol);
    for (vol_surface_id, result, repricings) in vol_results {
        acc.record("vol", vol_surface_id.as_str(), result, repricings);
    }

    let scalar_vol_t0 =
        MarketSnapshot::extract_with_dependencies(market_t0, MarketRestoreFlags::VOL, &market_deps);
    let mut scalar_vol_restore =
        MarketSnapshot::extract_with_dependencies(market_t1, MarketRestoreFlags::VOL, &market_deps);
    if !scalar_vol_t0.volatility_scalars.is_empty()
        || !scalar_vol_restore.volatility_scalars.is_empty()
    {
        // T1 surfaces/cubes remain in the snapshot; only scalar vol quotes move.
        scalar_vol_restore.volatility_scalars = scalar_vol_t0.volatility_scalars;
        let mut repricings = 0;
        let result = restored_family_factor(
            request,
            &scalar_vol_restore,
            pv_t1,
            AttributionFactor::Volatility,
            MarketRestoreFlags::VOL,
            "Vol:scalars",
            &mut repricings,
        );
        acc.record("vol", "Vol:scalars", result, repricings);
    }

    // FX-exposure factor: pricing impact of FX-rate changes on cross-currency
    // instruments. Attempted when EITHER market carries an FX matrix: FX
    // introduced at T1 is still an FX-rate move (restoring the T0 state clears
    // the matrix), so gating on T0 alone silently pushed that P&L into
    // residual. When neither market has FX there is nothing to restore and the
    // factor is omitted (single-currency instruments stay at zero FX P&L).
    if market_t0.fx().is_some() || market_t1.fx().is_some() {
        if market_t0.fx().is_none() {
            acc.notes.push(
                "Taylor FX factor: T0 market has no FX matrix; FX-exposure P&L is measured \
                 against an FX-less T0 restore"
                    .to_string(),
            );
        }
        // Both values are in the instrument's native currency. A
        // single-currency instrument that does not read the FX matrix
        // produces exactly zero.
        let fx_snapshot = MarketSnapshot::extract(market_t0, MarketRestoreFlags::FX);
        match restored_family_factor(
            request,
            &fx_snapshot,
            pv_t1,
            AttributionFactor::Fx,
            MarketRestoreFlags::FX,
            "Fx",
            &mut acc.num_repricings,
        ) {
            Ok(result) => {
                acc.factors.push(result);
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Taylor attribution: FX factor computation failed"
                );
                acc.notes.push(format!("Taylor FX factor failed: {e}"));
            }
        }
    }

    // Restore-and-reprice families (same isolation as FX / parallel):
    // inflation curves, base correlations, and market scalars. The isolated
    // P&L already includes that family's higher-order effects.
    for (factor, factor_kind, factor_name) in [
        (AttributionFactor::InflationCurves, "inflation", "Inflation"),
        (
            AttributionFactor::Correlations,
            "correlations",
            "Correlations",
        ),
        (
            AttributionFactor::MarketScalars,
            "market-scalar",
            "MarketScalars",
        ),
    ] {
        if let Some(flags) = MarketRestoreFlags::for_factor(&factor) {
            record_restored_family_factor(
                &mut acc,
                request,
                &market_deps,
                pv_t1,
                (factor, flags),
                factor_kind,
                factor_name,
            );
        }
    }

    // Model-parameter snapshot: reprice T1 market with T0 params restored.
    let params_t0 = model_params_t0
        .cloned()
        .unwrap_or_else(|| extract_model_params(instrument.as_ref()));
    if !matches!(params_t0, ModelParamsSnapshot::None) {
        let result = compute_model_params_factor(
            instrument,
            market_t1,
            as_of_t1,
            pv_t1,
            &params_t0,
            &mut acc.num_repricings,
        );
        acc.record("model-parameter", "ModelParameters", result, 0);
    }

    // Theta (time decay): reprice at T1 date with T0 market. The outcome
    // also carries `coupon_income` so `attribute_pnl_taylor` can split theta
    // into PV-only and coupon components without re-collecting cashflows.
    match compute_theta_factor(
        &instrument_t0,
        market_t0,
        as_of_t0,
        as_of_t1,
        pv_t0,
        &mut acc.num_repricings,
    ) {
        Ok(outcome) => {
            let ThetaFactorOutcome {
                factor: result,
                coupon_income,
                cash_paid,
            } = outcome;

            acc.theta_coupon_income = Some(coupon_income);
            acc.theta_cash_paid = Some(cash_paid);
            acc.factors.push(result);
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "Taylor attribution: theta factor computation failed"
            );
            acc.notes.push(format!(
                "Taylor theta factor failed: {e}; total P&L excludes period cash receipts"
            ));
            acc.result_invalid = true;
        }
    }

    Ok(acc)
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
/// snapshot/restore pnl_support, and
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
///   request's `FinstackConfig` supplies the result rounding context.
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
    let instrument = request.instrument;
    let taylor = compute_taylor_result(request, config)?;
    let mut attribution = seed_attribution(
        request,
        taylor.pv_t0,
        taylor.pv_t1,
        AttributionMethod::Taylor(config.clone()),
        request.execution_policy,
    )?;
    let ccy = attribution.total_pnl.currency();

    // Surface the factor-level diagnostics collected during computation
    // (failed factors, missing volatility references, missing T0 FX) and
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
        // Expose the working behind this factor. A non-finite component is
        // already flagged through `factor_money` above.
        let step = sensitivity_step(
            &factor.factor_name,
            factor.working.clone(),
            money_or_zero(factor.explained_pnl, ccy),
            factor.gamma_pnl.map(|gamma| money_or_zero(gamma, ccy)),
            &mut attribution.meta.notes,
        );
        attribution.sensitivity_steps.push(step);

        // Route accumulation through Money::checked_add so a currency
        // mismatch surfaces as an error instead of being silently coerced into
        // `ccy`.
        if factor.factor == AttributionFactor::Carry {
            // Taylor theta already includes cashflows from compute_theta_factor.
            let zero = Money::from((0_i64, ccy));
            let ci = taylor.theta_coupon_income.unwrap_or(zero);
            let cash_paid = taylor.theta_cash_paid.unwrap_or(zero);
            let theta_only = Money::new(factor_money.amount() - cash_paid.amount(), ccy)?;
            // Taylor path: delta_accrued and flat_window_diff are unavailable (no repricing).
            let carry_inputs = TotalReturnCarryInputs {
                cash_paid,
                income_cash_paid: ci,
                delta_accrued: None,
                flat_window_diff: None,
                funding_cost: None,
                num_repricings: 0,
                warnings: Vec::new(),
            };
            apply_total_return_carry(&mut attribution, theta_only, carry_inputs)?;
            continue;
        }
        let bucket = attribution.factor_pnl_mut(&factor.factor);
        *bucket = bucket.checked_add(factor_money)?;
        if factor.factor == AttributionFactor::Fx {
            stamp_fx_policy(
                &mut attribution,
                ccy,
                "Taylor FX-exposure P&L (T0 FX matrix restored vs T1)",
            );
        }
    }

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
