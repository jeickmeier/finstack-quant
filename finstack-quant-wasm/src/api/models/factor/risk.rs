//! WASM bindings for product-independent factor risk: position-level VaR /
//! ES decomposition, risk budgeting and stress attribution.
//!
//! Exposed through the `models.factor.risk` facade namespace.

use crate::utils::input::{
    from_js_json, js_f64, js_f64_matrix, js_f64_seq, js_opt_bool, js_opt_f64, js_string,
    js_string_seq,
};
use crate::utils::{to_js_err, to_js_value};

use finstack_quant_models::factor::risk::{DecompositionConfig, PositionRiskDecomposition};
use wasm_bindgen::prelude::*;

// Position-level VaR / ES decomposition and risk budgeting

/// Decompose portfolio VaR and ES into position contributions via parametric
/// Euler allocation.
///
/// Returns the canonical `PositionRiskDecomposition` (the object Python's
/// `parametric_var_decomposition` returns): portfolio VaR/ES (losses
/// negative), `method`, and per-position `var_contributions` and
/// `es_contributions` rows.
/// @param position_ids - Position identifiers, one per weight.
/// @param weights - Position weights or exposures in portfolio currency.
/// @param covariance - Square position-return covariance matrix as nested rows (`n x n`, row-major).
/// @param confidence - Optional tail confidence as a decimal probability in `(0.5, 1)`; omitted or `null` uses the Rust `DecompositionConfig::parametric_95()` preset (0.95).
/// @param compute_incremental - Optional; when `true`, also computes
///   incremental VaR (one full repricing per position). Defaults to `false`.
///
/// # Errors
///
/// Throws a `TypeError` if an argument has the wrong JavaScript type, and a
/// `validation` error if identifier, weight, or covariance dimensions
/// disagree; the covariance matrix is not finite, symmetric, and positive
/// semidefinite; or `confidence` is not finite and in `(0.5, 1)`.
#[wasm_bindgen(js_name = parametricVarDecomposition)]
pub fn parametric_var_decomposition(
    position_ids: JsValue,
    weights: JsValue,
    covariance: JsValue,
    confidence: Option<JsValue>,
    compute_incremental: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let confidence = js_opt_f64(confidence.as_ref(), "confidence")?;
    use finstack_quant_models::factor::risk::{
        flatten_square_matrix, parametric_var_decomposition,
    };
    let ids = js_string_seq(&position_ids, "positionIds")?;
    let weights = js_f64_seq(&weights, "weights")?;
    let covariance = js_f64_matrix(&covariance, "covariance")?;
    let compute_incremental = js_opt_bool(compute_incremental.as_ref(), "computeIncremental")?;

    let cov_flat =
        flatten_square_matrix(covariance, weights.len(), "covariance").map_err(to_js_err)?;
    let result = parametric_var_decomposition(
        &ids,
        &weights,
        &cov_flat,
        confidence,
        compute_incremental == Some(true),
    )
    .map_err(to_js_err)?;
    crate::utils::to_js_value(&result)
}

/// Decompose portfolio Expected Shortfall into position contributions via
/// parametric Euler allocation.
///
/// Returns the `ParametricEsDecompositionView` reporting view (the object
/// Python's `parametric_es_decomposition` returns): a top-level
/// `{portfolio_var, portfolio_es, confidence, n_positions, contributions}`
/// object whose `contributions` entries are
/// `{position_id, component_es, marginal_es, pct_contribution}`.
/// @param position_ids - Position identifiers, one per weight.
/// @param weights - Position weights or exposures in portfolio currency.
/// @param covariance - Square position-return covariance matrix as nested rows (`n x n`, row-major).
/// @param confidence - Optional tail confidence as a decimal probability in `(0.5, 1)`; omitted or `null` uses the Rust `DecompositionConfig::parametric_95()` preset (0.95).
///
/// # Errors
///
/// Throws a `TypeError` if an argument has the wrong JavaScript type, and a
/// `validation` error if identifier, weight, or covariance dimensions
/// disagree; the covariance matrix is not finite, symmetric, and positive
/// semidefinite; or `confidence` is not finite and in `(0.5, 1)`.
#[wasm_bindgen(js_name = parametricEsDecomposition)]
pub fn parametric_es_decomposition(
    position_ids: JsValue,
    weights: JsValue,
    covariance: JsValue,
    confidence: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let confidence = js_opt_f64(confidence.as_ref(), "confidence")?;
    use finstack_quant_models::factor::risk::{flatten_square_matrix, parametric_es_decomposition};
    let ids = js_string_seq(&position_ids, "positionIds")?;
    let weights = js_f64_seq(&weights, "weights")?;
    let covariance = js_f64_matrix(&covariance, "covariance")?;

    let cov_flat =
        flatten_square_matrix(covariance, weights.len(), "covariance").map_err(to_js_err)?;
    let view =
        parametric_es_decomposition(&ids, &weights, &cov_flat, confidence).map_err(to_js_err)?;
    crate::utils::to_js_value(&view)
}

/// Decompose portfolio VaR and ES from per-position scenario P&Ls via
/// historical simulation.
///
/// Returns the canonical `PositionRiskDecomposition`, including the
/// historical `es_contributions` rows (marginal and incremental VaR are
/// `null`).
/// @param position_ids - Position identifiers, one per P&L row.
/// @param position_pnls - Position-major P&L matrix: one row per position, one column per scenario (losses negative).
/// @param confidence - Optional tail confidence as a decimal probability in `(0.5, 1)`; omitted or `null` uses the Rust `DecompositionConfig::historical_95()` preset (0.95).
///
/// # Errors
///
/// Throws a `TypeError` if an argument has the wrong JavaScript type, and a
/// `validation` error if the matrix does not have one row per position or its
/// rows have different scenario counts, `confidence` is not finite and in
/// `(0.5, 1)`, too few scenarios resolve the requested tail, or a P&L value is
/// non-finite.
#[wasm_bindgen(js_name = historicalVarDecomposition)]
pub fn historical_var_decomposition(
    position_ids: JsValue,
    position_pnls: JsValue,
    confidence: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let confidence = js_opt_f64(confidence.as_ref(), "confidence")?;
    use finstack_quant_models::factor::risk::{
        flatten_position_pnls, historical_var_decomposition,
    };
    let ids = js_string_seq(&position_ids, "positionIds")?;
    let position_pnls = js_f64_matrix(&position_pnls, "positionPnls")?;

    let (flat, n_scenarios) = flatten_position_pnls(position_pnls, ids.len()).map_err(to_js_err)?;
    let result =
        historical_var_decomposition(&ids, &flat, n_scenarios, confidence).map_err(to_js_err)?;
    crate::utils::to_js_value(&result)
}

/// Evaluate a per-position risk budget against actual component VaRs.
///
/// Returns the canonical `RiskBudgetResult` (the object Python's
/// `evaluate_risk_budget` returns): per-position `positions` rows,
/// `total_overbudget` and `has_breach`. Validation (array-length agreement,
/// duplicate position-id rejection) and the default `utilizationThreshold`
/// live in the canonical Rust `evaluate_risk_budget_arrays` /
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
    use finstack_quant_models::factor::risk::{
        evaluate_risk_budget_arrays, DEFAULT_UTILIZATION_THRESHOLD,
    };
    let ids = js_string_seq(&position_ids, "positionIds")?;
    let actual_var = js_f64_seq(&actual_var, "actualVar")?;
    let target_var_pct = js_f64_seq(&target_var_pct, "targetVarPct")?;
    let threshold = utilization_threshold.unwrap_or(DEFAULT_UTILIZATION_THRESHOLD);

    let result =
        evaluate_risk_budget_arrays(ids, &actual_var, &target_var_pct, portfolio_var, threshold)
            .map_err(to_js_err)?;
    let js = crate::utils::to_js_value(&result)?;
    let positions = js_sys::Reflect::get(&js, &JsValue::from("positions"))?;
    crate::utils::restore_non_finite_rows::<
        finstack_quant_models::factor::risk::PositionBudgetEntry,
    >(&positions)?;
    Ok(js)
}

// Position-risk configuration, stress attribution and lookups

/// Configuration of position-level VaR / ES decomposition.
#[wasm_bindgen(js_name = DecompositionConfig)]
pub struct JsDecompositionConfig {
    pub(crate) inner: DecompositionConfig,
}

json_round_trip!(JsDecompositionConfig, DecompositionConfig);

#[wasm_bindgen(js_class = DecompositionConfig)]
impl JsDecompositionConfig {
    /// Parametric (delta-normal) decomposition at a confidence level.
    /// @param confidence - Tail confidence as a decimal probability in `(0.5, 1)`, such as 0.99.
    /// @returns The parametric configuration.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `confidence` is not a number; the range is
    /// checked when the configuration is used.
    pub fn parametric(confidence: JsValue) -> Result<JsDecompositionConfig, JsValue> {
        Ok(Self {
            inner: DecompositionConfig::parametric(js_f64(&confidence, "confidence")?),
        })
    }

    /// Parametric decomposition at 95% confidence.
    /// @returns The parametric 95% preset.
    #[wasm_bindgen(js_name = parametric95)]
    pub fn parametric_95() -> JsDecompositionConfig {
        Self {
            inner: DecompositionConfig::parametric_95(),
        }
    }

    /// Parametric decomposition at 99% confidence.
    /// @returns The parametric 99% preset.
    #[wasm_bindgen(js_name = parametric99)]
    pub fn parametric_99() -> JsDecompositionConfig {
        Self {
            inner: DecompositionConfig::parametric_99(),
        }
    }

    /// Historical-simulation decomposition at a confidence level.
    /// @param confidence - Tail confidence as a decimal probability in `(0.5, 1)`, such as 0.99.
    /// @returns The historical configuration.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `confidence` is not a number; the range is
    /// checked when the configuration is used.
    pub fn historical(confidence: JsValue) -> Result<JsDecompositionConfig, JsValue> {
        Ok(Self {
            inner: DecompositionConfig::historical(js_f64(&confidence, "confidence")?),
        })
    }

    /// Historical-simulation decomposition at 95% confidence.
    /// @returns The historical 95% preset.
    #[wasm_bindgen(js_name = historical95)]
    pub fn historical_95() -> JsDecompositionConfig {
        Self {
            inner: DecompositionConfig::historical_95(),
        }
    }

    /// Copy of this configuration that also computes incremental VaR (one full repricing per position).
    /// @returns The configuration with `computeIncremental` set.
    #[wasm_bindgen(js_name = withIncremental)]
    pub fn with_incremental(&self) -> JsDecompositionConfig {
        Self {
            inner: self.inner.clone().with_incremental(),
        }
    }

    /// Tail confidence as a decimal probability.
    #[wasm_bindgen(getter)]
    pub fn confidence(&self) -> f64 {
        self.inner.confidence
    }

    /// Decomposition method: `"parametric"` or `"historical"`.
    #[wasm_bindgen(getter)]
    pub fn method(&self) -> Result<String, JsValue> {
        finstack_quant_core::wire::serde_label(&self.inner.method).map_err(to_js_err)
    }

    /// Whether leave-one-out incremental VaR is computed.
    #[wasm_bindgen(getter, js_name = computeIncremental)]
    pub fn compute_incremental(&self) -> bool {
        self.inner.compute_incremental
    }
}

/// Default utilization threshold of `evaluateRiskBudget`. Twin of the Rust and
/// Python constant `DEFAULT_UTILIZATION_THRESHOLD`.
/// @returns The threshold as a fraction of the risk budget.
#[wasm_bindgen(js_name = defaultUtilizationThreshold)]
pub fn default_utilization_threshold() -> f64 {
    finstack_quant_models::factor::risk::DEFAULT_UTILIZATION_THRESHOLD
}

/// Attribute the portfolio loss in tail scenarios to positions.
///
/// Returns the canonical `StressAttribution` (the object Python's
/// `build_stress_attribution` returns): the VaR threshold, the tail scenarios
/// and each position's average tail P&L and share of the tail loss.
/// @param position_ids - Position identifiers, one per row of `positionPnls`.
/// @param position_pnls - Position-major P&L matrix as nested rows: one row per position, one column per scenario, in reporting-currency amounts.
/// @param confidence - Optional tail confidence in `(0.5, 1)`; omitted or `null` uses the Rust `DecompositionConfig::historical_95()` preset (0.95).
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
    use finstack_quant_models::factor::risk::{build_stress_attribution, flatten_position_pnls};
    let ids = js_string_seq(&position_ids, "positionIds")?;
    let position_pnls = js_f64_matrix(&position_pnls, "positionPnls")?;
    let confidence = js_opt_f64(confidence.as_ref(), "confidence")?;
    let (flat, n_scenarios) = flatten_position_pnls(position_pnls, ids.len()).map_err(to_js_err)?;
    let result =
        build_stress_attribution(&ids, &flat, n_scenarios, confidence).map_err(to_js_err)?;
    to_js_value(&result)
}

/// One position's component VaR from a position risk decomposition.
/// @param decomp - `PositionRiskDecomposition` object or JSON, as returned by `parametricVarDecomposition` or `historicalVarDecomposition`.
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
