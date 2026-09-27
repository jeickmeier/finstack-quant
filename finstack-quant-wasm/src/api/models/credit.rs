//! WASM bindings for structural credit model specifications.
//!
//! Mirrors the layout of `finstack-quant-py/src/bindings/models/credit.rs` so the
//! Rust-canonical → PyO3 → wasm-bindgen triplet keeps file parity. The exported
//! JS surface is nested under `models.credit`; wasm-bindgen exports remain flat
//! and the hand-written facade establishes the public namespace.

use crate::utils::{check_js_safe_count, parse_iso_date, to_js_err};
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::math::random::Pcg64Rng;
use finstack_quant_models::credit::{
    AssetDynamics, CreditState, CreditStateVariable, DynamicRecoverySpec, EndogenousHazardSpec,
    MertonBarrierType, MertonModel, OptimalToggle, ThresholdDirection, ToggleExerciseModel,
};
use js_sys::Float64Array;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

fn parse_f64_tenors(value: JsValue) -> Result<Vec<f64>, JsValue> {
    if value.is_instance_of::<Float64Array>() {
        Ok(Float64Array::new(&value).to_vec())
    } else {
        serde_wasm_bindgen::from_value(value).map_err(to_js_err)
    }
}

/// Build a structural Merton model JSON payload.
///
/// # Errors
///
/// Throws a JavaScript exception if `asset_value`, `asset_vol`, or
/// `debt_barrier` is non-positive, or if the model cannot be serialized to
/// JSON.
/// @param asset_value - Current fair value of the firm's assets in monetary units.
/// @param asset_vol - Annualized volatility of firm-asset returns, expressed as a decimal.
/// @param debt_barrier - Positive debt face value defining the structural-model default barrier.
/// @param risk_free_rate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
#[wasm_bindgen(js_name = mertonModelJson)]
pub fn merton_model_json(
    asset_value: f64,
    asset_vol: f64,
    debt_barrier: f64,
    risk_free_rate: f64,
) -> Result<String, JsValue> {
    let model = MertonModel::new(asset_value, asset_vol, debt_barrier, risk_free_rate)
        .map_err(to_js_err)?;
    serde_json::to_string(&model).map_err(to_js_err)
}

/// Build a CreditGrades structural model JSON payload.
///
/// # Errors
///
/// Throws a JavaScript exception if CreditGrades or Merton model validation
/// rejects the supplied equity, volatility, debt, barrier-uncertainty, or
/// recovery inputs, or if the model cannot be serialized to JSON.
/// @param equity_value - Current market value of equity in the firm's monetary units.
/// @param equity_vol - Annualized equity-return volatility expressed as a decimal.
/// @param total_debt - Total debt face value in the firm's monetary units.
/// @param risk_free_rate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
/// @param barrier_uncertainty - Lognormal dispersion of the CreditGrades default barrier, not a generic uncertainty score.
/// @param mean_recovery - Mean recovery rate at default expressed as a fraction from 0 through 1.
#[wasm_bindgen(js_name = creditGradesModelJson)]
pub fn credit_grades_model_json(
    equity_value: f64,
    equity_vol: f64,
    total_debt: f64,
    risk_free_rate: f64,
    barrier_uncertainty: f64,
    mean_recovery: f64,
) -> Result<String, JsValue> {
    let model = MertonModel::credit_grades(
        equity_value,
        equity_vol,
        total_debt,
        risk_free_rate,
        barrier_uncertainty,
        mean_recovery,
    )
    .map_err(to_js_err)?;
    serde_json::to_string(&model).map_err(to_js_err)
}

/// Compute structural default probability from model JSON.
///
/// # Errors
///
/// Throws a JavaScript exception if `model_json` is malformed or does not
/// deserialize as a Merton model.
/// @param model_json - Serialized Merton structural-credit model produced by this API's model builder.
/// @param horizon - Forward-looking model horizon measured in years.
#[wasm_bindgen(js_name = mertonDefaultProbability)]
pub fn merton_default_probability(model_json: &str, horizon: f64) -> Result<f64, JsValue> {
    let model: MertonModel = serde_json::from_str(model_json).map_err(to_js_err)?;
    Ok(model.default_probability(horizon))
}

/// Compute the physical-measure (Moody's KMV) default probability, the
/// theoretical EDF, from a Merton model JSON payload.
///
/// # Errors
///
/// Throws a JavaScript exception if `model_json` is malformed, if
/// `asset_drift` is not finite, or if the model uses driftless CreditGrades
/// dynamics.
/// @param model_json - Serialized Merton structural-credit model produced by this API's model builder.
/// @param asset_drift - Expected physical total return on firm assets as a continuously compounded decimal, replacing the risk-free rate.
/// @param horizon - Forward-looking model horizon measured in years.
#[wasm_bindgen(js_name = mertonDefaultProbabilityWithDrift)]
pub fn merton_default_probability_with_drift(
    model_json: &str,
    asset_drift: f64,
    horizon: f64,
) -> Result<f64, JsValue> {
    let model: MertonModel = serde_json::from_str(model_json).map_err(to_js_err)?;
    model
        .default_probability_with_drift(asset_drift, horizon)
        .map_err(to_js_err)
}

/// Compute distance-to-default from a Merton model JSON payload.
///
/// Distance-to-default is `ln(V/B)/(sigma*sqrt(T))` plus drift adjustments.
/// Lower values indicate higher default risk. This is the risk-neutral `d2`,
/// not the Moody's KMV distance-to-default.
///
/// # Errors
///
/// Throws a JavaScript exception if `model_json` is malformed or does not
/// deserialize as a Merton model.
/// @param model_json - Serialized Merton structural-credit model produced by this API's model builder.
/// @param horizon - Forward-looking model horizon measured in years.
#[wasm_bindgen(js_name = mertonDistanceToDefault)]
pub fn merton_distance_to_default(model_json: &str, horizon: f64) -> Result<f64, JsValue> {
    let model: MertonModel = serde_json::from_str(model_json).map_err(to_js_err)?;
    Ok(model.distance_to_default(horizon))
}

/// Compute the physical-measure (Moody's KMV) distance-to-default from a
/// Merton model JSON payload.
///
/// # Errors
///
/// Throws a JavaScript exception if `model_json` is malformed, if
/// `asset_drift` is not finite, or if the model uses driftless CreditGrades
/// dynamics.
/// @param model_json - Serialized Merton structural-credit model produced by this API's model builder.
/// @param asset_drift - Expected physical total return on firm assets as a continuously compounded decimal, replacing the risk-free rate.
/// @param horizon - Forward-looking model horizon measured in years.
#[wasm_bindgen(js_name = mertonDistanceToDefaultWithDrift)]
pub fn merton_distance_to_default_with_drift(
    model_json: &str,
    asset_drift: f64,
    horizon: f64,
) -> Result<f64, JsValue> {
    let model: MertonModel = serde_json::from_str(model_json).map_err(to_js_err)?;
    model
        .distance_to_default_with_drift(asset_drift, horizon)
        .map_err(to_js_err)
}

/// Compute the Moody's KMV default point, short-term debt plus half of
/// long-term debt, for use as a structural default barrier.
///
/// # Errors
///
/// Throws a JavaScript exception if either input is negative or non-finite,
/// or if the resulting default point is zero.
/// @param short_term_debt - Liabilities due within one year, in the firm's monetary units.
/// @param long_term_debt - Liabilities maturing beyond one year, in the same units; half of it enters the default point.
#[wasm_bindgen(js_name = mertonKmvDefaultPoint)]
pub fn merton_kmv_default_point(short_term_debt: f64, long_term_debt: f64) -> Result<f64, JsValue> {
    MertonModel::kmv_default_point(short_term_debt, long_term_debt).map_err(to_js_err)
}

/// Compute the zero-coupon bond credit spread (per year) from a Merton model
/// JSON payload, given an exogenous recovery rate paid at maturity.
///
/// # Errors
///
/// Throws a JavaScript exception if `model_json` is malformed or does not
/// deserialize as a Merton model, `horizon` is non-finite or non-positive, or
/// `recovery` is outside `[0, 1]`.
/// @param model_json - Serialized Merton structural-credit model produced by this API's model builder.
/// @param horizon - Forward-looking model horizon measured in years.
/// @param recovery - Recovery rate at default expressed as a fraction of par from 0 through 1.
#[wasm_bindgen(js_name = mertonImpliedSpread)]
pub fn merton_implied_spread(
    model_json: &str,
    horizon: f64,
    recovery: f64,
) -> Result<f64, JsValue> {
    let model: MertonModel = serde_json::from_str(model_json).map_err(to_js_err)?;
    model.implied_spread(horizon, recovery).map_err(to_js_err)
}

/// Compute the Merton (1974) endogenous debt spread (per year) from a Merton
/// model JSON payload, where recovery is the firm's own terminal asset value.
///
/// # Errors
///
/// Throws a JavaScript exception if `model_json` is malformed, if `horizon` is
/// non-positive, if the barrier type is not terminal, or if the implied debt
/// value is non-positive.
/// @param model_json - Serialized Merton structural-credit model produced by this API's model builder.
/// @param horizon - Maturity of the firm's debt measured in years.
#[wasm_bindgen(js_name = mertonDebtSpread)]
pub fn merton_debt_spread(model_json: &str, horizon: f64) -> Result<f64, JsValue> {
    let model: MertonModel = serde_json::from_str(model_json).map_err(to_js_err)?;
    model.debt_spread(horizon).map_err(to_js_err)
}

/// Compute the ISDA-style CDS par spread (per year, as a decimal) implied by a
/// Merton model's survival curve.
///
/// # Errors
///
/// Throws a JavaScript exception if `model_json` is malformed, if `maturity`
/// is non-positive, if `recovery` is outside `[0, 1]` or contradicts the
/// model's CreditGrades `mean_recovery`, or if the implied survival curve
/// cannot be bootstrapped.
/// @param model_json - Serialized Merton structural-credit model produced by this API's model builder.
/// @param maturity - CDS maturity in years; must be positive and finite.
/// @param recovery - Recovery rate at default expressed as a fraction of par from 0 through 1.
#[wasm_bindgen(js_name = mertonCdsParSpread)]
pub fn merton_cds_par_spread(
    model_json: &str,
    maturity: f64,
    recovery: f64,
) -> Result<f64, JsValue> {
    let model: MertonModel = serde_json::from_str(model_json).map_err(to_js_err)?;
    model.cds_par_spread(maturity, recovery).map_err(to_js_err)
}

/// Build a Merton model JSON payload from observable equity inputs (KMV
/// calibration).
///
/// # Errors
///
/// Throws a JavaScript exception if equity, volatility, debt, rate, or maturity
/// inputs are invalid, or if the model cannot be serialized to JSON.
/// @param equity_value - Current market value of equity in the firm's monetary units.
/// @param equity_vol - Annualized equity-return volatility expressed as a decimal.
/// @param total_debt - Total debt face value used as the structural default barrier.
/// @param risk_free_rate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
/// @param payout_rate - Continuous dividend or payout yield on assets, expressed as a decimal.
/// @param maturity - Calibration horizon in years; must be positive and finite.
#[wasm_bindgen(js_name = mertonFromEquityJson)]
pub fn merton_from_equity_json(
    equity_value: f64,
    equity_vol: f64,
    total_debt: f64,
    risk_free_rate: f64,
    payout_rate: f64,
    maturity: f64,
) -> Result<String, JsValue> {
    let model = MertonModel::from_equity(
        equity_value,
        equity_vol,
        total_debt,
        risk_free_rate,
        payout_rate,
        maturity,
    )
    .map_err(to_js_err)?;
    serde_json::to_string(&model).map_err(to_js_err)
}

/// Build a Merton model JSON payload from a target CDS par spread.
///
/// The objective is a full ISDA-style par spread built from the model's
/// survival curve. A quote that no volatility in `[0.01, 2.0]` reproduces, or
/// one consistent with several volatilities, is rejected rather than resolved
/// arbitrarily.
///
/// # Errors
///
/// Throws a JavaScript exception if spread, recovery, debt, rate, maturity,
/// asset value, or payout inputs are invalid, if the quote is unattainable or
/// ambiguous, or if the model cannot be serialized to JSON.
/// @param cds_spread_bp - Target CDS par spread in basis points.
/// @param recovery - Recovery rate at default expressed as a fraction from 0 through 1.
/// @param total_debt - Total debt face value in the firm's monetary units.
/// @param risk_free_rate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
/// @param maturity - Calibration horizon in years; must be positive and finite.
/// @param asset_value - Assumed initial firm asset value in monetary units.
/// @param payout_rate - Continuous payout rate on assets, expressed as a decimal.
#[wasm_bindgen(js_name = mertonFromCdsSpreadJson)]
pub fn merton_from_cds_spread_json(
    cds_spread_bp: f64,
    recovery: f64,
    total_debt: f64,
    risk_free_rate: f64,
    maturity: f64,
    asset_value: f64,
    payout_rate: f64,
) -> Result<String, JsValue> {
    let model = MertonModel::from_cds_spread(
        cds_spread_bp,
        recovery,
        total_debt,
        risk_free_rate,
        maturity,
        asset_value,
        payout_rate,
    )
    .map_err(to_js_err)?;
    serde_json::to_string(&model).map_err(to_js_err)
}

/// Build a Merton model JSON payload calibrated to a target cumulative default
/// probability.
///
/// # Errors
///
/// Throws a JavaScript exception if asset value, volatility, rate, target PD,
/// or maturity inputs are invalid, or if the model cannot be serialized to JSON.
/// @param asset_value - Current fair value of the firm's assets in monetary units.
/// @param asset_vol - Annualized volatility of firm-asset returns, expressed as a decimal; must be positive.
/// @param risk_free_rate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%. Pass the expected physical asset return to calibrate against a real-world default rate.
/// @param payout_rate - Continuous payout rate on assets, expressed as a decimal; it enters the calibration drift and is carried on the returned model.
/// @param target_pd - Target cumulative default probability in `(0, 1)`.
/// @param maturity - Calibration horizon in years; must be positive and finite.
#[wasm_bindgen(js_name = mertonFromTargetPdJson)]
pub fn merton_from_target_pd_json(
    asset_value: f64,
    asset_vol: f64,
    risk_free_rate: f64,
    payout_rate: f64,
    target_pd: f64,
    maturity: f64,
) -> Result<String, JsValue> {
    let model = MertonModel::from_target_pd(
        asset_value,
        asset_vol,
        risk_free_rate,
        payout_rate,
        target_pd,
        maturity,
    )
    .map_err(to_js_err)?;
    serde_json::to_string(&model).map_err(to_js_err)
}

/// Build a Merton model JSON payload with explicit barrier and asset-dynamics
/// specifications.
///
/// # Errors
///
/// Throws a JavaScript exception if model inputs are invalid, if
/// `barrier_type_json` or `dynamics_json` does not deserialize, or if the model
/// cannot be serialized to JSON.
/// @param asset_value - Current fair value of the firm's assets in monetary units.
/// @param asset_vol - Annualized volatility of firm-asset returns, expressed as a decimal.
/// @param debt_barrier - Positive debt face value defining the structural-model default barrier.
/// @param risk_free_rate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
/// @param payout_rate - Continuous payout rate on assets, expressed as a decimal.
/// @param barrier_type_json - Serialized `MertonBarrierType` JSON (terminal or first-passage).
/// @param dynamics_json - Serialized `AssetDynamics` JSON (GBM, jump-diffusion, or CreditGrades).
#[wasm_bindgen(js_name = mertonModelWithDynamicsJson)]
pub fn merton_model_with_dynamics_json(
    asset_value: f64,
    asset_vol: f64,
    debt_barrier: f64,
    risk_free_rate: f64,
    payout_rate: f64,
    barrier_type_json: &str,
    dynamics_json: &str,
) -> Result<String, JsValue> {
    let barrier_type: MertonBarrierType =
        serde_json::from_str(barrier_type_json).map_err(to_js_err)?;
    let dynamics: AssetDynamics = serde_json::from_str(dynamics_json).map_err(to_js_err)?;
    let model = MertonModel::new_with_dynamics(
        asset_value,
        asset_vol,
        debt_barrier,
        risk_free_rate,
        payout_rate,
        barrier_type,
        dynamics,
    )
    .map_err(to_js_err)?;
    serde_json::to_string(&model).map_err(to_js_err)
}

/// Compute implied equity value and equity volatility from a Merton model JSON
/// payload.
///
/// # Errors
///
/// Throws a JavaScript exception if `model_json` is malformed, if `horizon` is
/// non-positive or non-finite, or if the inversion is numerically ill-conditioned.
/// @param model_json - Serialized Merton structural-credit model produced by this API's model builder.
/// @param horizon - Forward-looking model horizon measured in years.
/// @returns A `Float64Array` of length 2: `[equityValue, equityVolatility]`.
#[wasm_bindgen(js_name = mertonTryImpliedEquity)]
pub fn merton_try_implied_equity(model_json: &str, horizon: f64) -> Result<Float64Array, JsValue> {
    let (equity, equity_vol) = merton_try_implied_equity_pair(model_json, horizon)?;
    let arr = Float64Array::new_with_length(2);
    arr.set_index(0, equity);
    arr.set_index(1, equity_vol);
    Ok(arr)
}

fn merton_try_implied_equity_pair(model_json: &str, horizon: f64) -> Result<(f64, f64), JsValue> {
    let model: MertonModel = serde_json::from_str(model_json).map_err(to_js_err)?;
    model.try_implied_equity(horizon).map_err(to_js_err)
}

/// Bootstrap a hazard-curve JSON payload from structural default probabilities.
///
/// # Errors
///
/// Throws a JavaScript exception if `model_json` is malformed, if `base_date`
/// is not a valid ISO-8601 calendar date (`YYYY-MM-DD`), if `tenors` is empty
/// or contains non-positive values, if `recovery` is out of range or
/// contradicts the model's CreditGrades `mean_recovery`, if `day_count` is not
/// a recognized convention, if the implied survival curve is non-monotonic, or
/// if the hazard curve cannot be serialized to JSON.
/// @param model_json - Serialized Merton structural-credit model produced by this API's model builder.
/// @param id - Hazard-curve identifier string.
/// @param base_date - Valuation date in ISO-8601 form, such as `"2025-01-15"`.
/// @param tenors - Tenor grid in years as a `number[]` or `Float64Array`; entries must be positive and distinct.
/// @param recovery - Recovery rate at default expressed as a fraction from 0 through 1.
/// @param day_count - Day-count convention the curve uses to turn dates into year fractions, such as `"act_365f"` or `"act_360"`.
#[wasm_bindgen(js_name = mertonToHazardCurveJson)]
pub fn merton_to_hazard_curve_json(
    model_json: &str,
    id: &str,
    base_date: &str,
    tenors: JsValue,
    recovery: f64,
    day_count: &str,
) -> Result<String, JsValue> {
    let model: MertonModel = serde_json::from_str(model_json).map_err(to_js_err)?;
    let base = parse_iso_date(base_date)?;
    let tenor_vec = parse_f64_tenors(tenors)?;
    let day_count: DayCount = day_count
        .parse()
        .map_err(|e| to_js_err(format!("Invalid day_count {day_count:?}: {e}")))?;
    let curve = model
        .to_hazard_curve(id, base, &tenor_vec, recovery, day_count)
        .map_err(to_js_err)?;
    serde_json::to_string(&curve).map_err(to_js_err)
}

/// Simulate firm-asset paths and return a JSON payload with the time grid and
/// row-major asset values.
///
/// `num_paths` and `num_steps` must fit JavaScript's safe integer range
/// (`Number.MAX_SAFE_INTEGER`, `2^53 - 1`): counts marshal across the wasm
/// boundary as IEEE-754 doubles, so a larger value would round silently rather
/// than fail loudly.
///
/// # Errors
///
/// Throws a JavaScript exception if `model_json` is malformed, if path or step
/// counts exceed the safe-integer range, if `num_steps` is zero, if `horizon`
/// is non-positive or non-finite, or if the result cannot be serialized to JSON.
/// @param model_json - Serialized Merton structural-credit model produced by this API's model builder.
/// @param num_paths - Number of Monte Carlo paths to simulate.
/// @param num_steps - Number of time steps per path; must be at least 1.
/// @param horizon - Simulation horizon in years; must be positive and finite.
/// @param seed - RNG seed for reproducible draws (`Pcg64Rng`).
/// @param antithetic - When `true`, use antithetic variates for variance reduction.
#[wasm_bindgen(js_name = mertonSimulatePathsJson)]
pub fn merton_simulate_paths_json(
    model_json: &str,
    num_paths: usize,
    num_steps: usize,
    horizon: f64,
    seed: u64,
    antithetic: bool,
) -> Result<String, JsValue> {
    check_js_safe_count(num_paths, "num_paths")?;
    check_js_safe_count(num_steps, "num_steps")?;
    let model: MertonModel = serde_json::from_str(model_json).map_err(to_js_err)?;
    let mut rng = Pcg64Rng::new(seed);
    let paths = model
        .simulate_paths(num_paths, num_steps, horizon, &mut rng, antithetic)
        .map_err(to_js_err)?;
    serde_json::to_string(&paths).map_err(to_js_err)
}

/// Evaluate a `DynamicRecoverySpec` JSON payload at a given accreted
/// notional, returning the implied recovery rate. Result is clamped to
/// `[0, base_recovery]`.
///
/// # Errors
///
/// Throws a JavaScript exception if `spec_json` is malformed or does not
/// deserialize as a dynamic-recovery specification.
/// @param spec_json - Serialized DynamicRecoverySpec JSON defining the notional-to-recovery mapping.
/// @param notional - Signed trade notional in the instrument's native currency units.
#[wasm_bindgen(js_name = dynamicRecoveryAtNotional)]
pub fn dynamic_recovery_at_notional(spec_json: &str, notional: f64) -> Result<f64, JsValue> {
    let spec: DynamicRecoverySpec = serde_json::from_str(spec_json).map_err(to_js_err)?;
    Ok(spec.recovery_at_notional(notional))
}

/// Evaluate an `EndogenousHazardSpec` JSON payload at a given leverage
/// level, returning the implied hazard rate. Floored at 0.
///
/// # Errors
///
/// Throws a JavaScript exception if `spec_json` is malformed or does not
/// deserialize as an endogenous-hazard specification.
/// @param spec_json - Serialized EndogenousHazardSpec JSON defining the leverage-to-hazard mapping.
/// @param leverage - Debt-to-assets leverage ratio used by the structural credit model.
#[wasm_bindgen(js_name = endogenousHazardAtLeverage)]
pub fn endogenous_hazard_at_leverage(spec_json: &str, leverage: f64) -> Result<f64, JsValue> {
    let spec: EndogenousHazardSpec = serde_json::from_str(spec_json).map_err(to_js_err)?;
    Ok(spec.hazard_at_leverage(leverage))
}

/// Convenience evaluator: hazard rate after a PIK accrual updates the
/// outstanding notional. Computes leverage = `accreted_notional / asset_value`
/// then evaluates the hazard mapping.
///
/// # Errors
///
/// Throws a JavaScript exception if `spec_json` is malformed or does not
/// deserialize as an endogenous-hazard specification.
/// @param spec_json - Serialized EndogenousHazardSpec JSON defining the leverage-to-hazard mapping.
/// @param accreted_notional - Outstanding notional after PIK accrual, in the debt's monetary units.
/// @param asset_value - Current fair value of the firm's assets in monetary units.
#[wasm_bindgen(js_name = endogenousHazardAfterPikAccrual)]
pub fn endogenous_hazard_after_pik_accrual(
    spec_json: &str,
    accreted_notional: f64,
    asset_value: f64,
) -> Result<f64, JsValue> {
    let spec: EndogenousHazardSpec = serde_json::from_str(spec_json).map_err(to_js_err)?;
    Ok(spec.hazard_after_pik_accrual(accreted_notional, asset_value))
}

/// Build a constant dynamic-recovery spec JSON payload.
///
/// # Errors
///
/// Throws a JavaScript exception if `recovery` is outside `[0, 1]` or the
/// specification cannot be serialized to JSON.
/// @param recovery - Recovery rate at default expressed as a fraction of par from 0 through 1.
#[wasm_bindgen(js_name = dynamicRecoveryConstantJson)]
pub fn dynamic_recovery_constant_json(recovery: f64) -> Result<String, JsValue> {
    let spec = DynamicRecoverySpec::constant(recovery).map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Build an endogenous hazard power-law spec JSON payload.
///
/// # Errors
///
/// Throws a JavaScript exception if `base_hazard` is negative,
/// `base_leverage` is non-positive, or the specification cannot be serialized
/// to JSON.
/// @param base_hazard - Reference annual default intensity used by the leverage-to-hazard mapping.
/// @param base_leverage - Positive reference debt-to-assets leverage ratio for the hazard mapping.
/// @param exponent - Power-law exponent in `lambda(L) = baseHazard * (L / baseLeverage)^exponent`.
#[wasm_bindgen(js_name = endogenousHazardPowerLawJson)]
pub fn endogenous_hazard_power_law_json(
    base_hazard: f64,
    base_leverage: f64,
    exponent: f64,
) -> Result<String, JsValue> {
    let spec =
        EndogenousHazardSpec::power_law(base_hazard, base_leverage, exponent).map_err(to_js_err)?;
    serde_json::to_string(&spec).map_err(to_js_err)
}

/// Build a credit-state JSON payload for toggle-exercise decisions.
///
/// Parameter order follows the canonical Rust `CreditState` field order
/// (and the Python binding): `hazardRate`, `distanceToDefault`, `leverage`,
/// `accretedNotional`, `couponDue`, `assetValue`.
///
/// # Errors
///
/// Throws a JavaScript exception if the credit state cannot be serialized to
/// JSON.
/// @param hazard_rate - Annualized instantaneous default intensity, expressed as a decimal.
/// @param distance_to_default - Optional distance to default, measured as standard deviations from the default point.
/// @param leverage - Debt-to-assets leverage ratio used by the structural credit model.
/// @param accreted_notional - Outstanding notional after PIK accrual, in the debt's monetary units.
/// @param coupon_due - Cash coupon amount due at the toggle decision date, in debt monetary units.
/// @param asset_value - Current fair value of the firm's assets in monetary units.
#[wasm_bindgen(js_name = creditStateJson)]
pub fn credit_state_json(
    hazard_rate: f64,
    distance_to_default: Option<f64>,
    leverage: f64,
    accreted_notional: f64,
    coupon_due: f64,
    asset_value: Option<f64>,
) -> Result<String, JsValue> {
    let state = CreditState {
        hazard_rate,
        distance_to_default,
        leverage,
        accreted_notional,
        coupon_due,
        asset_value,
    };
    serde_json::to_string(&state).map_err(to_js_err)
}

/// Build a threshold toggle-exercise model JSON payload.
///
/// # Errors
///
/// Throws a JavaScript exception if `variable` or `direction` is not a
/// supported value, or if the model cannot be serialized to JSON.
/// @param variable - Credit-state variable: `"hazard_rate"`, `"distance_to_default"`, or `"leverage"`.
/// @param threshold - Threshold value in the units of the selected credit-state variable.
/// @param direction - Threshold comparison: `"above"` selects PIK above the level and `"below"` below it.
#[wasm_bindgen(js_name = toggleExerciseThresholdJson)]
pub fn toggle_exercise_threshold_json(
    variable: &str,
    threshold: f64,
    direction: &str,
) -> Result<String, JsValue> {
    let variable = variable.parse::<CreditStateVariable>().map_err(to_js_err)?;
    let direction = direction.parse::<ThresholdDirection>().map_err(to_js_err)?;
    let model = ToggleExerciseModel::threshold(variable, threshold, direction);
    serde_json::to_string(&model).map_err(to_js_err)
}

/// Build an optimal toggle-exercise model JSON payload.
///
/// `nested_paths` is the Monte-Carlo path count for the nested optimal-exercise
/// simulation. It is rejected if it exceeds `Number.MAX_SAFE_INTEGER` (`2^53-1`):
/// `usize` counts marshal across the wasm boundary as IEEE-754 doubles, so a
/// larger value would round silently rather than fail loudly.
///
/// # Errors
///
/// Throws a JavaScript exception if `nested_paths` exceeds JavaScript's safe
/// integer range or the model cannot be serialized to JSON.
/// @param nested_paths - Number of nested Monte Carlo paths for continuation-value estimation; must fit JavaScript's safe integer range.
/// @param equity_discount_rate - Annual equity-holder discount rate used in the nested toggle decision.
/// @param asset_vol - Annualized volatility of firm-asset returns, expressed as a decimal.
/// @param risk_free_rate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
/// @param horizon - Forward-looking model horizon measured in years.
#[wasm_bindgen(js_name = toggleExerciseOptimalJson)]
pub fn toggle_exercise_optimal_json(
    nested_paths: usize,
    equity_discount_rate: f64,
    asset_vol: f64,
    risk_free_rate: f64,
    horizon: f64,
) -> Result<String, JsValue> {
    check_js_safe_count(nested_paths, "nested_paths")?;
    let model = ToggleExerciseModel::OptimalExercise(OptimalToggle {
        nested_paths,
        equity_discount_rate,
        asset_vol,
        risk_free_rate,
        horizon,
    });
    serde_json::to_string(&model).map_err(to_js_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Credit-model evaluator parity (mirrors finstack-quant-py PyMertonModel etc.).

    #[test]
    fn merton_distance_to_default_matches_native() {
        let json = merton_model_json(100.0, 0.20, 80.0, 0.05).expect("merton json");
        let dd_wasm = merton_distance_to_default(&json, 1.0).expect("dd");
        let model = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("merton");
        let dd_native = model.distance_to_default(1.0);
        assert!(
            (dd_wasm - dd_native).abs() < 1e-12,
            "WASM dd ({dd_wasm}) must match native ({dd_native})"
        );
    }

    #[test]
    fn merton_implied_spread_matches_native() {
        let json = merton_model_json(100.0, 0.20, 80.0, 0.05).expect("merton json");
        let spread_wasm = merton_implied_spread(&json, 5.0, 0.40).expect("spread");
        let model = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("merton");
        let spread_native = model.implied_spread(5.0, 0.40).expect("spread");
        assert!(
            (spread_wasm - spread_native).abs() < 1e-12,
            "WASM spread ({spread_wasm}) must match native ({spread_native})"
        );
    }

    #[test]
    fn merton_from_equity_roundtrips() {
        let m_known = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("merton");
        let (equity, equity_vol) = m_known.try_implied_equity(1.0).expect("equity");
        let json = merton_from_equity_json(equity, equity_vol, 80.0, 0.05, 0.0, 1.0).expect("json");
        let m_cal: MertonModel = serde_json::from_str(&json).expect("deserialize");
        assert!(
            (m_cal.asset_value() - m_known.asset_value()).abs() < 1e-6,
            "asset value roundtrip"
        );
        assert!(
            (m_cal.asset_vol() - m_known.asset_vol()).abs() < 1e-6,
            "asset vol roundtrip"
        );
    }

    #[test]
    fn merton_try_implied_equity_matches_native() {
        let json = merton_model_json(100.0, 0.20, 80.0, 0.05).expect("merton json");
        let (equity_wasm, vol_wasm) = merton_try_implied_equity_pair(&json, 1.0).expect("equity");
        let model = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("merton");
        let (equity_native, vol_native) = model.try_implied_equity(1.0).expect("native");
        assert!(
            (equity_wasm - equity_native).abs() < 1e-12,
            "WASM equity ({equity_wasm}) must match native ({equity_native})"
        );
        assert!(
            (vol_wasm - vol_native).abs() < 1e-12,
            "WASM equity vol ({vol_wasm}) must match native ({vol_native})"
        );
    }

    #[test]
    fn dynamic_recovery_at_notional_matches_native() {
        let json = dynamic_recovery_constant_json(0.40).expect("spec json");
        let r_wasm = dynamic_recovery_at_notional(&json, 100.0).expect("r");
        let spec = DynamicRecoverySpec::constant(0.40).expect("spec");
        let r_native = spec.recovery_at_notional(100.0);
        assert!((r_wasm - r_native).abs() < 1e-12);
    }

    #[test]
    fn endogenous_hazard_at_leverage_matches_native() {
        let json = endogenous_hazard_power_law_json(0.10, 1.5, 2.5).expect("spec json");
        let h_wasm = endogenous_hazard_at_leverage(&json, 2.0).expect("h");
        let spec = EndogenousHazardSpec::power_law(0.10, 1.5, 2.5).expect("spec");
        let h_native = spec.hazard_at_leverage(2.0);
        assert!((h_wasm - h_native).abs() < 1e-12);
    }

    #[test]
    fn endogenous_hazard_after_pik_accrual_matches_native() {
        let json = endogenous_hazard_power_law_json(0.10, 1.5, 2.5).expect("spec json");
        let h_wasm = endogenous_hazard_after_pik_accrual(&json, 120.0, 66.67).expect("h");
        let spec = EndogenousHazardSpec::power_law(0.10, 1.5, 2.5).expect("spec");
        let h_native = spec.hazard_after_pik_accrual(120.0, 66.67);
        assert!((h_wasm - h_native).abs() < 1e-12);
    }

    #[test]
    fn toggle_exercise_optimal_json_accepts_reasonable_path_count() {
        // A normal nested-path count round-trips into a valid model payload.
        let json = toggle_exercise_optimal_json(10_000, 0.10, 0.25, 0.04, 5.0).expect("model json");
        let model: ToggleExerciseModel =
            serde_json::from_str(&json).expect("payload must deserialize");
        match model {
            ToggleExerciseModel::OptimalExercise(o) => assert_eq!(o.nested_paths, 10_000),
            other => panic!("expected OptimalExercise, got {other:?}"),
        }
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn toggle_exercise_optimal_json_rejects_unsafe_path_count() {
        // A `nested_paths` above Number.MAX_SAFE_INTEGER would round silently
        // when marshaled as an f64; the binding must reject it instead.
        let unsafe_count = crate::utils::MAX_SAFE_JS_INTEGER as usize + 1;
        let result = toggle_exercise_optimal_json(unsafe_count, 0.10, 0.25, 0.04, 5.0);
        assert!(
            result.is_err(),
            "nested_paths above 2^53-1 must be rejected, not silently rounded"
        );
    }
}
