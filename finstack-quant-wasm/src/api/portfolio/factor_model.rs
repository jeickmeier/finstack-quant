//! WASM bindings for the portfolio factor-model workflows: the stateful
//! `FactorModel` handle, strategy weight allocation, factor stress, position
//! what-if and the credit volatility report.

use super::JsPortfolio;
use crate::api::core::market_context::JsMarketContext;
use crate::api::models::factor::JsCreditFactorModel;
use crate::utils::date::parse_iso_date;
use crate::utils::input::{from_js_json, js_bool, js_string, json_text};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_models::factor::{FactorId, FactorModelConfig};
use finstack_quant_portfolio::factor_model::{
    self as fm, FactorModel, PositionChange, WeightAllocationSpec,
};
use finstack_quant_portfolio::sensitivity::SensitivityMatrixJson;
use wasm_bindgen::prelude::*;

/// Convert `[factorId, shift]` pairs into Rust factor stresses.
fn parse_stresses(stresses: &JsValue) -> Result<Vec<(FactorId, f64)>, JsValue> {
    let stresses: Vec<(String, f64)> = from_js_json(stresses, "stresses")?;
    Ok(stresses
        .into_iter()
        .map(|(factor_id, shift)| (FactorId::new(factor_id), shift))
        .collect())
}

/// Portfolio factor-risk model built once from a `FactorModelConfig`.
///
/// Holds the factor definitions, covariance matrix, dependency matcher and
/// sensitivity engine so repeated analyses reuse one validated model. Build
/// it with `FactorModel.fromConfig`; call `free()` when done.
#[wasm_bindgen(js_name = FactorModel)]
pub struct JsFactorModel {
    pub(crate) inner: FactorModel,
}

#[wasm_bindgen(js_class = FactorModel)]
impl JsFactorModel {
    /// Build a factor model from a declarative configuration.
    ///
    /// @param config - `FactorModelConfig` object or JSON: factor definitions, covariance matrix (axes in factor order), matching rules, pricing mode, bump sizes and risk measure.
    /// @returns A validated `FactorModel` handle.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` (kind `invalid_type`) if `config` is not a JSON
    /// string or plain object, and a `FinstackError` (kind `validation`) if it
    /// does not match the `FactorModelConfig` schema, a matching rule names an
    /// undeclared factor, or the covariance axes do not align with the factors.
    #[wasm_bindgen(js_name = fromConfig)]
    pub fn from_config(config: JsValue) -> Result<JsFactorModel, JsValue> {
        let config: FactorModelConfig = from_js_json(&config, "config")?;
        FactorModel::from_config(config)
            .map(|inner| JsFactorModel { inner })
            .map_err(to_js_err)
    }

    /// Match every position's market dependencies to the configured factors.
    ///
    /// Returns the `FactorAssignmentReport`: per-position `assignments` and
    /// the `unmatched` dependencies.
    /// @param portfolio - Built portfolio whose instrument dependencies are mapped.
    /// @param market - `core.MarketContext` handle used to resolve credit-index aggregates.
    /// @returns The `FactorAssignmentReport`.
    ///
    /// # Errors
    ///
    /// Throws a `FinstackError` (kind `not_found`) if a referenced credit
    /// index is missing from `market`, and kind `validation` if the unmatched
    /// policy is strict and a dependency cannot be mapped.
    #[wasm_bindgen(js_name = assignFactors)]
    pub fn assign_factors(
        &self,
        portfolio: &JsPortfolio,
        market: &JsMarketContext,
    ) -> Result<JsValue, JsValue> {
        let report = self
            .inner
            .assign_factors(&portfolio.inner, market.inner())
            .map_err(to_js_err)?;
        to_js_value(&report)
    }

    /// Compute the weighted position-by-factor sensitivity matrix.
    ///
    /// Returns the sensitivity-matrix wire object (`base_currency`,
    /// `position_ids`, `factor_ids`, `data` rows), the same shape
    /// `computeFactorSensitivities` returns and `decomposeFactorRisk` accepts.
    /// @param portfolio - Built portfolio; its base currency is the reporting currency of every sensitivity.
    /// @param market - `core.MarketContext` handle bumped by the sensitivity engine, including FX for cross-currency positions.
    /// @param as_of - ISO-8601 valuation date for sensitivities and spot FX.
    /// @returns The `SensitivityMatrixJson` object.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` (kind `invalid_type`) if `asOf` is not a string,
    /// and a `FinstackError` if it is not an ISO date or factor assignment,
    /// sensitivity generation or FX conversion fails.
    #[wasm_bindgen(js_name = computeSensitivities)]
    pub fn compute_sensitivities(
        &self,
        portfolio: &JsPortfolio,
        market: &JsMarketContext,
        as_of: JsValue,
    ) -> Result<JsValue, JsValue> {
        let as_of = parse_iso_date(&js_string(&as_of, "asOf")?)?;
        let matrix = self
            .inner
            .compute_sensitivities(&portfolio.inner, market.inner(), as_of)
            .map_err(to_js_err)?;
        to_js_value(&SensitivityMatrixJson::from_matrix(
            &matrix,
            portfolio.inner.base_currency,
        ))
    }

    /// Decompose portfolio risk into factor and residual contributions.
    ///
    /// Returns the `RiskDecomposition` in the configured risk measure's units.
    /// @param portfolio - Built portfolio to analyze.
    /// @param market - `core.MarketContext` handle used for sensitivity generation.
    /// @param as_of - ISO-8601 valuation date of the analysis.
    /// @returns The `RiskDecomposition`.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` (kind `invalid_type`) if `asOf` is not a string,
    /// and a `FinstackError` if it is not an ISO date or factor assignment,
    /// sensitivity or decomposition fails.
    #[wasm_bindgen(js_name = analyze)]
    pub fn analyze(
        &self,
        portfolio: &JsPortfolio,
        market: &JsMarketContext,
        as_of: JsValue,
    ) -> Result<JsValue, JsValue> {
        let as_of = parse_iso_date(&js_string(&as_of, "asOf")?)?;
        let decomposition = self
            .inner
            .analyze(&portfolio.inner, market.inner(), as_of)
            .map_err(to_js_err)?;
        to_js_value(&decomposition)
    }

    /// Run a position remove/resize what-if against the model's baseline.
    ///
    /// Returns the `WhatIfResult`: the `before` and `after` risk
    /// decompositions and the per-factor `delta`.
    /// @param portfolio - Built portfolio the changes are applied to.
    /// @param market - `core.MarketContext` handle holding the curves, quotes and FX data.
    /// @param as_of - ISO-8601 valuation date.
    /// @param changes - Array of `PositionChange` objects: `{ kind: "remove", position_id }` or `{ kind: "resize", position_id, new_quantity }`.
    /// @returns The `WhatIfResult`.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` (kind `invalid_type`) for a mistyped argument, and a
    /// `FinstackError` if the changes are malformed, `asOf` is not an ISO
    /// date, a change names an unknown position, or the analysis fails.
    #[wasm_bindgen(js_name = positionWhatIf)]
    pub fn position_what_if(
        &self,
        portfolio: &JsPortfolio,
        market: &JsMarketContext,
        as_of: JsValue,
        changes: JsValue,
    ) -> Result<JsValue, JsValue> {
        let as_of = parse_iso_date(&js_string(&as_of, "asOf")?)?;
        let changes: Vec<PositionChange> = from_js_json(&changes, "changes")?;
        let result = self
            .inner
            .position_what_if(&portfolio.inner, market.inner(), as_of, &changes)
            .map_err(to_js_err)?;
        to_js_value(&result)
    }

    /// Shock factors, reprice, and decompose risk under the stressed market.
    ///
    /// Returns the `StressResult`: base-currency `total_pnl`, per-position
    /// `position_pnl`, and the `stressed_decomposition`.
    /// @param portfolio - Built portfolio whose positions are revalued.
    /// @param market - `core.MarketContext` handle holding the unstressed market.
    /// @param as_of - ISO-8601 valuation date for both endpoints.
    /// @param stresses - Array of `[factorId, shift]` pairs; each shift is in the factor's configured market-mapping units.
    /// @returns The `StressResult`.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` (kind `invalid_type`) for a mistyped argument, and a
    /// `FinstackError` if the stresses are malformed, `asOf` is not an ISO
    /// date, a stress names an unknown factor, or repricing or decomposition
    /// fails.
    #[wasm_bindgen(js_name = factorStress)]
    pub fn factor_stress(
        &self,
        portfolio: &JsPortfolio,
        market: &JsMarketContext,
        as_of: JsValue,
        stresses: JsValue,
    ) -> Result<JsValue, JsValue> {
        let as_of = parse_iso_date(&js_string(&as_of, "asOf")?)?;
        let stresses = parse_stresses(&stresses)?;
        let result = self
            .inner
            .factor_stress(&portfolio.inner, market.inner(), as_of, &stresses)
            .map_err(to_js_err)?;
        to_js_value(&result)
    }

    /// Shock factors and reprice without decomposing stressed risk.
    ///
    /// Returns the `StressPnl`: base-currency `total_pnl` and per-position
    /// `position_pnl` (loss negative).
    /// @param portfolio - Built portfolio whose positions are revalued.
    /// @param market - `core.MarketContext` handle holding the unstressed market.
    /// @param as_of - ISO-8601 valuation date for both endpoints.
    /// @param stresses - Array of `[factorId, shift]` pairs; each shift is in the factor's configured market-mapping units.
    /// @returns The `StressPnl`.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` (kind `invalid_type`) for a mistyped argument, and a
    /// `FinstackError` if the stresses are malformed, `asOf` is not an ISO
    /// date, a stress names an unknown factor, or repricing fails.
    #[wasm_bindgen(js_name = factorStressPnl)]
    pub fn factor_stress_pnl(
        &self,
        portfolio: &JsPortfolio,
        market: &JsMarketContext,
        as_of: JsValue,
        stresses: JsValue,
    ) -> Result<JsValue, JsValue> {
        let as_of = parse_iso_date(&js_string(&as_of, "asOf")?)?;
        let stresses = parse_stresses(&stresses)?;
        let result = self
            .inner
            .factor_stress_pnl(&portfolio.inner, market.inner(), as_of, &stresses)
            .map_err(to_js_err)?;
        to_js_value(&result)
    }
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
