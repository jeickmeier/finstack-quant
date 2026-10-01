//! WASM bindings for the portfolio factor-model workflows: strategy weight
//! allocation, factor stress, position what-if and the credit volatility
//! report.

use super::JsPortfolio;
use crate::api::core::market_context::JsMarketContext;
use crate::api::models::factor::JsCreditFactorModel;
use crate::utils::date::parse_iso_date;
use crate::utils::input::{from_js_json, js_bool, js_string, json_text};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_models::factor::{FactorId, FactorModelConfig};
use finstack_quant_portfolio::factor_model::{
    self as fm, FactorModel, FactorModelBuilder, PositionChange, WeightAllocationSpec,
};
use wasm_bindgen::prelude::*;

/// Build the portfolio factor model a `FactorModelConfig` describes.
fn build_model(factor_model_config: &JsValue) -> Result<FactorModel, JsValue> {
    let config: FactorModelConfig = from_js_json(factor_model_config, "factorModelConfig")?;
    FactorModelBuilder::new()
        .config(config)
        .build()
        .map_err(to_js_err)
}

/// Allocate strategy weights from a `WeightAllocationSpec`.
///
/// Returns the `WeightAllocationResult` object (`scheme`, per-strategy
/// `allocations` and portfolio `diagnostics`); `allocateWeightsJson` returns
/// the same result as a JSON string.
/// @param spec - `WeightAllocationSpec` object or JSON selecting the allocation scheme, the strategy inputs and any covariance data the scheme needs.
/// @returns The `WeightAllocationResult`.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `spec` is not a JSON string
/// or plain object, and a `FinstackError` (kind `validation`) if it does not
/// match the `WeightAllocationSpec` schema or violates a scheme invariant.
#[wasm_bindgen(js_name = allocateWeights)]
pub fn allocate_weights(spec: JsValue) -> Result<JsValue, JsValue> {
    let spec: WeightAllocationSpec = from_js_json(&spec, "spec")?;
    let result = fm::allocate_weights(&spec).map_err(to_js_err)?;
    to_js_value(&result)
}

/// Allocate strategy weights and return the result as wire JSON.
///
/// Wire twin of `allocateWeights` (Rust `allocate_weights_json`).
/// @param spec_json - `WeightAllocationSpec` object or JSON.
/// @returns Compact JSON of the `WeightAllocationResult`.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `specJson` is not a JSON
/// string or plain object, and a `FinstackError` (kind `validation`) if it
/// does not match the `WeightAllocationSpec` schema or violates a scheme
/// invariant.
#[wasm_bindgen(js_name = allocateWeightsJson)]
pub fn allocate_weights_json(spec_json: JsValue) -> Result<String, JsValue> {
    let spec_json = json_text(&spec_json, "specJson")?;
    fm::allocate_weights_json(&spec_json).map_err(to_js_err)
}

/// Validate a strategy allocation specification and return its canonical JSON.
///
/// The spec is parsed and executed solely for validation; the allocation
/// itself is discarded (Rust `validate_allocation_json`).
/// @param spec_json - `WeightAllocationSpec` object or JSON.
/// @returns The canonical compact JSON of the accepted spec.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `specJson` is not a JSON
/// string or plain object, and a `FinstackError` (kind `validation`) if it
/// does not match the `WeightAllocationSpec` schema or violates a scheme
/// invariant.
#[wasm_bindgen(js_name = validateAllocationJson)]
pub fn validate_allocation_json(spec_json: JsValue) -> Result<String, JsValue> {
    let spec_json = json_text(&spec_json, "specJson")?;
    fm::validate_allocation_json(&spec_json).map_err(to_js_err)
}

/// Run a factor-stress scenario and revalue the portfolio under the stressed market.
///
/// Each stress shifts one configured factor by an absolute amount in that
/// factor's own bump units. Returns the `StressResult`: base-currency
/// `total_pnl`, per-position `position_pnl`, and the `stressed_decomposition`
/// of risk under the stressed market.
/// @param portfolio - Built portfolio whose positions are revalued.
/// @param market - `core.MarketContext` handle holding the unstressed curves, quotes and FX data.
/// @param factor_model_config - `FactorModelConfig` object or JSON: factor definitions, matching rules, covariance and risk measure.
/// @param as_of - ISO-8601 valuation date.
/// @param stresses - Array of `[factorId, shift]` pairs; every `factorId` must be a factor of the configured model.
/// @returns The `StressResult`.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) for a mistyped argument, and a
/// `FinstackError` if the config or stresses are malformed, `asOf` is not an
/// ISO date, a stress names an unknown factor, or the market bump, valuation,
/// sensitivity or decomposition step fails.
#[wasm_bindgen(js_name = factorStress)]
pub fn factor_stress(
    portfolio: &JsPortfolio,
    market: &JsMarketContext,
    factor_model_config: JsValue,
    as_of: JsValue,
    stresses: JsValue,
) -> Result<JsValue, JsValue> {
    let model = build_model(&factor_model_config)?;
    let as_of = parse_iso_date(&js_string(&as_of, "asOf")?)?;
    let stresses: Vec<(String, f64)> = from_js_json(&stresses, "stresses")?;
    let stresses: Vec<(FactorId, f64)> = stresses
        .into_iter()
        .map(|(factor_id, shift)| (FactorId::new(factor_id), shift))
        .collect();
    let result = model
        .factor_stress(&portfolio.inner, market.inner(), as_of, &stresses)
        .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Run a position remove/resize what-if analysis against a factor model.
///
/// Decomposes the portfolio's baseline risk, applies the changes and
/// decomposes again. Returns the `WhatIfResult`: the `before` and `after`
/// risk decompositions and the per-factor `delta`.
/// @param portfolio - Built portfolio the changes are applied to.
/// @param market - `core.MarketContext` handle holding the curves, quotes and FX data.
/// @param factor_model_config - `FactorModelConfig` object or JSON: factor definitions, matching rules, covariance and risk measure.
/// @param as_of - ISO-8601 valuation date.
/// @param changes - Array of `PositionChange` objects: `{ kind: "remove", position_id }` or `{ kind: "resize", position_id, new_quantity }`.
/// @returns The `WhatIfResult`.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) for a mistyped argument, and a
/// `FinstackError` if the config or changes are malformed, `asOf` is not an
/// ISO date, a change names an unknown position, or factor assignment,
/// sensitivity or decomposition fails.
#[wasm_bindgen(js_name = positionWhatIf)]
pub fn position_what_if(
    portfolio: &JsPortfolio,
    market: &JsMarketContext,
    factor_model_config: JsValue,
    as_of: JsValue,
    changes: JsValue,
) -> Result<JsValue, JsValue> {
    let model = build_model(&factor_model_config)?;
    let as_of = parse_iso_date(&js_string(&as_of, "asOf")?)?;
    let changes: Vec<PositionChange> = from_js_json(&changes, "changes")?;
    let result = model
        .position_what_if(&portfolio.inner, market.inner(), as_of, &changes)
        .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Build a credit volatility report from a risk decomposition and a credit factor model.
///
/// Rolls the decomposition's factor contributions up by credit-hierarchy
/// level. Returns the `CreditVolReport`: `total`, `measure`, the `generic`
/// credit-factor contribution, `by_level` rollups, `idiosyncratic_total` and,
/// when requested, `by_position_optional` rows.
/// @param decomposition - `RiskDecomposition` object or JSON, e.g. the output of `decomposeFactorRisk`.
/// @param model - `models.factor.credit.CreditFactorModel` handle whose hierarchy names the levels.
/// @param by_position - `true` to include the per-position breakdown; `false` leaves `by_position_optional` `null`.
/// @returns The `CreditVolReport`.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `decomposition` is not a JSON
/// string or plain object or `byPosition` is not a boolean, and a
/// `FinstackError` (kind `validation`) if `decomposition` does not match the
/// `RiskDecomposition` schema.
#[wasm_bindgen(js_name = buildCreditVolReport)]
pub fn build_credit_vol_report(
    decomposition: JsValue,
    model: &JsCreditFactorModel,
    by_position: JsValue,
) -> Result<JsValue, JsValue> {
    let decomposition: finstack_quant_models::factor::risk::RiskDecomposition =
        from_js_json(&decomposition, "decomposition")?;
    let by_position = js_bool(&by_position, "byPosition")?;
    let report = fm::build_credit_vol_report(&decomposition, &model.inner, by_position);
    to_js_value(&report)
}
