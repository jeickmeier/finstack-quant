//! WASM bindings for product-independent factor risk: factor-level
//! (`decomposeFactors`) and position-level VaR / ES decomposition, risk
//! budgeting and stress attribution.
//!
//! Exposed through the `models.factor.risk` facade namespace.

use crate::utils::input::{
    from_js_json, js_f64, js_f64_matrix, js_f64_seq, js_opt_f64, js_string, js_string_seq,
};
use crate::utils::{to_js_err, to_js_value};

use finstack_quant_factor::risk::PositionRiskDecomposition;
use wasm_bindgen::prelude::*;

// Position-level VaR / ES decomposition and risk budgeting

/// Decompose position VaR/ES from a parametric or historical source, with an
/// optional risk budget and, for scenario P&L, the tail stress attribution.
///
/// Returns the canonical `PositionRiskReport` (the object Python's
/// `position_risk_report` returns): `decomposition` (portfolio and
/// per-position VaR/ES, losses negative), `budget` (`null` without a budget)
/// and `stress` (`null` for a parametric source).
///
/// # Arguments
///
/// * `request` - `PositionRiskRequest` object or JSON text: `position_ids`, an
///   optional `confidence` (decimal in `(0.5, 1)`, default 0.95), a `source`
///   tagged by `kind` (`{kind: "parametric", weights, covariance,
///   compute_incremental}` with exposures in portfolio currency, an `n x n`
///   covariance in position order and incremental VaR off by default, or
///   `{kind: "historical", pnls}` with one row of scenario P&L per position,
///   losses negative) and an optional `budget` (`{targets,
///   utilization_threshold}`, target VaR shares in position order summing to
///   ~1).
///
/// # Errors
///
/// Throws a `validation` error for a malformed request, an unknown field or
/// kind, dimensions that disagree with the position ids, an invalid
/// covariance, too few scenarios for the tail, a confidence outside
/// `(0.5, 1)` or a malformed budget; `invalid_type` for a non-finite number.
#[wasm_bindgen(js_name = positionRiskReport)]
pub fn position_risk_report(request: JsValue) -> Result<JsValue, JsValue> {
    let request: finstack_quant_factor::risk::PositionRiskRequest =
        from_js_json(&request, "request")?;
    let report = finstack_quant_factor::risk::position_risk_report(&request).map_err(to_js_err)?;
    crate::utils::to_js_value(&report)
}

/// Evaluate a per-position risk budget against actual component VaRs.
///
/// Returns the canonical `RiskBudgetResult` (the object Python's
/// `evaluate_risk_budget` returns): per-position `positions` rows,
/// `total_overbudget` and `has_breach`. Validation (array-length agreement,
/// duplicate position-id rejection) and the default `utilizationThreshold`
/// live in the canonical Rust `evaluate_risk_budget` /
/// `DEFAULT_UTILIZATION_THRESHOLD` path shared with the Python binding.
/// @param position_ids - Position identifiers, one per budget row.
/// @param actual_var - Actual component VaR per position, in portfolio currency (loss-signed as the engine reports it).
/// @param target_var_pct - Target share of portfolio VaR per position; non-empty targets must sum to one.
/// @param portfolio_var - Total portfolio VaR used to convert risk-budget shares into absolute amounts.
/// @param utilization_threshold - Optional actual-to-target risk ratio that
///   flags a budget breach; omit for the Rust default of 1.2.
///
/// # Errors
///
/// Throws a `TypeError` if an argument has the wrong JavaScript type, and a
/// `validation` error if actual or target arrays do not match the identifier
/// count, a position id is duplicated, target shares are non-finite or outside
/// [0, 1], non-empty target shares do not sum to one within 0.05, risk inputs
/// are non-finite, the utilization threshold is non-finite or non-positive, or
/// nonzero component risk is paired with zero `portfolioVar`.
#[wasm_bindgen(js_name = evaluateRiskBudget)]
pub fn evaluate_risk_budget(
    position_ids: JsValue,
    actual_var: JsValue,
    target_var_pct: JsValue,
    portfolio_var: JsValue,
    utilization_threshold: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let portfolio_var = js_f64(&portfolio_var, "portfolioVar")?;
    let utilization_threshold = js_opt_f64(utilization_threshold.as_ref(), "utilizationThreshold")?;
    use finstack_quant_factor::risk::{
        DEFAULT_UTILIZATION_THRESHOLD, evaluate_risk_budget as eval_risk_budget,
    };
    let ids = js_string_seq(&position_ids, "positionIds")?;
    let actual_var = js_f64_seq(&actual_var, "actualVar")?;
    let target_var_pct = js_f64_seq(&target_var_pct, "targetVarPct")?;
    let threshold = utilization_threshold.unwrap_or(DEFAULT_UTILIZATION_THRESHOLD);

    let result = eval_risk_budget(ids, &actual_var, &target_var_pct, portfolio_var, threshold)
        .map_err(to_js_err)?;
    let js = crate::utils::to_js_value(&result)?;
    let positions = js_sys::Reflect::get(&js, &JsValue::from("positions"))?;
    crate::utils::restore_non_finite_rows::<finstack_quant_factor::risk::PositionBudgetEntry>(
        &positions,
    )?;
    Ok(js)
}

/// Evaluate a risk budget against a position risk decomposition.
///
/// Twin of Python `evaluate_risk_budget_from_decomposition` (Rust
/// `evaluate_risk_budget_from_decomposition`): reads each position's component
/// VaR and the portfolio VaR from `decomp`. Rows keep the decomposition's order.
/// @param decomp - `PositionRiskDecomposition` object or JSON, such as a `positionRiskReport` result's `decomposition`.
/// @param target_var_pct - Target fraction of portfolio VaR in `[0, 1]` per position, aligned with `decomp.var_contributions`; non-empty targets must sum to one within 0.05.
/// @param utilization_threshold - Optional actual-to-target ratio that flags a budget breach; omit for the Rust default of 1.2.
/// @returns `RiskBudgetResult`: per-position `positions` rows, `total_overbudget` and `has_breach`.
///
/// # Errors
///
/// Throws a `TypeError` if an argument has the wrong JavaScript type, and a
/// `validation` error if `decomp` is malformed, `targetVarPct` does not have one
/// entry per position, targets are non-finite, outside [0, 1] or do not sum to
/// one, the threshold is non-positive, or nonzero component VaR is paired with
/// zero portfolio VaR.
#[wasm_bindgen(js_name = evaluateRiskBudgetFromDecomposition)]
pub fn evaluate_risk_budget_from_decomposition(
    decomp: JsValue,
    target_var_pct: JsValue,
    utilization_threshold: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let decomp: PositionRiskDecomposition = from_js_json(&decomp, "decomp")?;
    let target_var_pct = js_f64_seq(&target_var_pct, "targetVarPct")?;
    let utilization_threshold = js_opt_f64(utilization_threshold.as_ref(), "utilizationThreshold")?;
    let result = finstack_quant_factor::risk::evaluate_risk_budget_from_decomposition(
        &decomp,
        &target_var_pct,
        utilization_threshold,
    )
    .map_err(to_js_err)?;
    let js = crate::utils::to_js_value(&result)?;
    let positions = js_sys::Reflect::get(&js, &JsValue::from("positions"))?;
    crate::utils::restore_non_finite_rows::<finstack_quant_factor::risk::PositionBudgetEntry>(
        &positions,
    )?;
    Ok(js)
}

// Stress attribution and lookups

/// Default utilization threshold of `evaluateRiskBudget`. Twin of the Rust and
/// Python constant `DEFAULT_UTILIZATION_THRESHOLD`.
/// @returns The threshold as a fraction of the risk budget.
#[wasm_bindgen(js_name = defaultUtilizationThreshold)]
pub fn default_utilization_threshold() -> f64 {
    finstack_quant_factor::risk::DEFAULT_UTILIZATION_THRESHOLD
}

/// Attribute the portfolio loss in tail scenarios to positions.
///
/// Returns the canonical `StressAttribution` (the object Python's
/// `build_stress_attribution` returns): the VaR threshold, the tail scenarios
/// and each position's average tail P&L and share of the tail loss.
/// @param position_ids - Position identifiers, one per row of `positionPnls`.
/// @param position_pnls - Position-major P&L matrix as nested rows: one row per position, one column per scenario, in reporting-currency amounts.
/// @param confidence - Optional tail confidence in `(0.5, 1)`; omitted or `null` defaults to 0.95.
/// @returns The `StressAttribution` object.
///
/// # Errors
///
/// Throws a `TypeError` if an argument has the wrong JavaScript type, and a
/// `validation` error if the dimensions disagree, a P&L is non-finite,
/// `confidence` is outside `(0.5, 1)`, or the tail holds no scenario.
#[wasm_bindgen(js_name = buildStressAttribution)]
pub fn build_stress_attribution(
    position_ids: JsValue,
    position_pnls: JsValue,
    confidence: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    use finstack_quant_factor::risk::{build_stress_attribution, flatten_position_pnls};
    let ids = js_string_seq(&position_ids, "positionIds")?;
    let position_pnls = js_f64_matrix(&position_pnls, "positionPnls")?;
    let confidence = js_opt_f64(confidence.as_ref(), "confidence")?;
    let (flat, n_scenarios) = flatten_position_pnls(position_pnls, ids.len()).map_err(to_js_err)?;
    let result =
        build_stress_attribution(&ids, &flat, n_scenarios, confidence).map_err(to_js_err)?;
    to_js_value(&result)
}

/// One position's component VaR from a position risk decomposition.
/// @param decomp - `PositionRiskDecomposition` object or JSON, such as a `positionRiskReport` result's `decomposition`.
/// @param position_id - Position identifier exactly as it appears in the decomposition.
/// @returns The position's Euler-allocated component VaR (losses negative).
///
/// # Errors
///
/// Throws a `validation` error if `decomp` is malformed, and a `not_found`
/// error if the position is not in the decomposition.
#[wasm_bindgen(js_name = positionComponentVar)]
pub fn position_component_var(decomp: JsValue, position_id: JsValue) -> Result<f64, JsValue> {
    let decomp: PositionRiskDecomposition = from_js_json(&decomp, "decomp")?;
    let position_id = js_string(&position_id, "positionId")?;
    decomp.component_var(&position_id).map_err(to_js_err)
}

// Factor-level risk decomposition

/// Decompose portfolio risk into factor and position contributions.
///
/// Uses the parametric (covariance-based) Euler decomposition.  Accepts
/// the canonical sensitivity-matrix wire object (the output of
/// `portfolio.computeFactorSensitivities`, or the Python `SensitivityMatrix.to_json`),
/// a `FactorCovarianceMatrix` JSON, and an optional `RiskMeasure` JSON.
///
/// Returns a structured object with `total_risk`, `measure`, `residual_risk`,
/// `factor_contributions` (array), `position_factor_contributions` (array),
/// and `position_residual_contributions` (array; empty for the parametric
/// decomposer — populated only by credit-aware position decomposers).
///
/// `measure` is the risk measure in its canonical serde form, the same shape
/// as the `riskMeasureJson` input: `"variance"` or `"volatility"`, or an
/// object carrying `confidence` for `var` / `expected_shortfall`. The Python
/// `RiskDecomposition.measure` getter returns the same value.
/// @param sensitivities_json - Canonical factor-sensitivity JSON with required `base_currency`, ordered `position_ids`/`factor_ids`, and nested `data[position][factor]` rows; `JSON.stringify` the result of `computeFactorSensitivities`. Python `SensitivityMatrix.to_json()` uses the same contract. Unknown fields, inconsistent dimensions, and non-finite entries are rejected.
/// @param covariance_json - Factor covariance-matrix JSON aligned with the supplied sensitivities.
/// @param risk_measure_json - `RiskMeasure` wire value selecting the decomposition metric: `"variance"`, `"volatility"`, an object such as `{ var: { confidence: 0.99 } }`, or the same value as JSON text; omit for `"variance"`.
///
/// # Errors
///
/// Throws a JavaScript exception (`kind` `validation`) if any JSON input is
/// malformed or has unknown keys; `base_currency` is not an ISO-4217 code; the
/// sensitivity rows do not match `position_ids` / `factor_ids`; sensitivity and
/// covariance factor axes disagree; the covariance matrix or risk measure is
/// invalid; or decomposition produces invalid variance or another non-finite
/// value.
#[wasm_bindgen(js_name = decomposeFactors)]
pub fn decompose_factors(
    sensitivities_json: JsValue,
    covariance_json: JsValue,
    risk_measure_json: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let wire: finstack_quant_portfolio::sensitivity::SensitivityMatrixJson =
        from_js_json(&sensitivities_json, "sensitivitiesJson")?;
    let matrix = finstack_quant_factor::SensitivityMatrix::try_from(wire).map_err(to_js_err)?;
    let covariance: finstack_quant_factor::FactorCovarianceMatrix =
        from_js_json(&covariance_json, "covarianceJson")?;
    // `js_opt_wire`: the unit variants are bare wire strings (`"variance"`),
    // which are not JSON text.
    let measure: finstack_quant_factor::RiskMeasure =
        crate::utils::wire::js_opt_wire(risk_measure_json.as_ref(), "riskMeasureJson")?
            .unwrap_or_default();
    let result = finstack_quant_factor::risk::decompose_factors(&matrix, &covariance, &measure)
        .map_err(to_js_err)?;
    to_js_value(&result)
}
