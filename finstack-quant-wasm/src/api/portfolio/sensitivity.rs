//! WASM bindings for factor-model sensitivities and risk decomposition.

use crate::api::valuations::market_handle::JsMarket;
use crate::utils::date::parse_iso_date;
use crate::utils::input::{js_opt_uint, js_string, json_text, opt_json_text};
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Compute first-order factor sensitivities and return the matrix.
///
/// Accepts a JSON array of positions, a JSON array of `FactorDefinition`,
/// a `MarketContext` JSON, an ISO 8601 date, and an optional `BumpSizeConfig`
/// JSON.  Returns the canonical sensitivity-matrix wire object
/// `{ base_currency, position_ids, factor_ids, data }` with `data` as nested
/// rows (`data[position][factor]`): the same shape `decomposeFactorRisk`
/// accepts and the Python `SensitivityMatrix.to_json` emits.
/// @param positions_json - Canonical portfolio-positions JSON to bump and revalue.
/// @param factors_json - Canonical factor-definition JSON identifying the market factors to shock.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param base_currency - ISO reporting currency for all returned monetary exposures; missing FX throws an error.
/// @param bump_config_json - Canonical bump-configuration JSON defining factor shock sizes and conventions.
///
/// # Errors
///
/// Throws a JavaScript exception if `asOf` is not a valid ISO date; any JSON
/// input is malformed; a factor definition or bump configuration is invalid or
/// unsupported; bumping or repricing fails; or the sensitivity matrix cannot be
/// converted to a JavaScript value.
#[wasm_bindgen(js_name = computeFactorSensitivities)]
pub fn compute_factor_sensitivities(
    positions_json: JsValue,
    factors_json: JsValue,
    market_json: JsValue,
    as_of: JsValue,
    base_currency: JsValue,
    bump_config_json: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let positions_json: &str = &json_text(&positions_json, "positionsJson")?;
    let factors_json: &str = &json_text(&factors_json, "factorsJson")?;
    let market_json: &str = &json_text(&market_json, "marketJson")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let base_currency: &str = &js_string(&base_currency, "baseCurrency")?;
    let bump_config_json = opt_json_text(bump_config_json.as_ref(), "bumpConfigJson")?;
    let as_of = parse_iso_date(as_of)?;
    let base_currency = base_currency
        .parse::<finstack_quant_core::currency::Currency>()
        .map_err(to_js_err)?;
    let market: finstack_quant_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;
    let matrix = finstack_quant_portfolio::sensitivity::compute_factor_sensitivities_from_json(
        positions_json,
        factors_json,
        &market,
        as_of,
        base_currency,
        bump_config_json.as_deref(),
    )
    .map_err(to_js_err)?;
    let output = finstack_quant_portfolio::sensitivity::SensitivityMatrixJson::from_matrix(
        &matrix,
        base_currency,
    );
    to_js_value(&output)
}

/// Compute first-order factor sensitivities using a pre-parsed [`JsMarket`].
///
/// Avoids reparsing market JSON for repeated factor analytics calls.
/// @param positions_json - Canonical portfolio-positions JSON to bump and revalue.
/// @param factors_json - Canonical factor-definition JSON identifying the market factors to shock.
/// @param market - Market context or JSON payload supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param base_currency - ISO reporting currency for all returned monetary exposures; missing FX throws an error.
/// @param bump_config_json - Canonical bump-configuration JSON defining factor shock sizes and conventions.
///
/// # Errors
///
/// Throws a JavaScript exception if `asOf` is not a valid ISO date; a position,
/// factor, or bump-config JSON input is malformed; a factor definition is
/// invalid or unsupported; bumping or repricing fails; or the sensitivity
/// matrix cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = computeFactorSensitivitiesWithMarket)]
pub fn compute_factor_sensitivities_with_market(
    positions_json: JsValue,
    factors_json: JsValue,
    market: &JsMarket,
    as_of: JsValue,
    base_currency: JsValue,
    bump_config_json: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let positions_json: &str = &json_text(&positions_json, "positionsJson")?;
    let factors_json: &str = &json_text(&factors_json, "factorsJson")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let base_currency: &str = &js_string(&base_currency, "baseCurrency")?;
    let bump_config_json = opt_json_text(bump_config_json.as_ref(), "bumpConfigJson")?;
    let as_of = parse_iso_date(as_of)?;
    let base_currency = base_currency
        .parse::<finstack_quant_core::currency::Currency>()
        .map_err(to_js_err)?;
    let matrix = finstack_quant_portfolio::sensitivity::compute_factor_sensitivities_from_json(
        positions_json,
        factors_json,
        market.inner(),
        as_of,
        base_currency,
        bump_config_json.as_deref(),
    )
    .map_err(to_js_err)?;
    let output = finstack_quant_portfolio::sensitivity::SensitivityMatrixJson::from_matrix(
        &matrix,
        base_currency,
    );
    to_js_value(&output)
}

/// Compute scenario P&L profiles via full repricing.
///
/// Same position/factor/market inputs as `computeFactorSensitivities`, plus
/// an optional `n_scenario_points` integer. Returns a structured array with one
/// `FactorPnlProfile` (`{ base_currency, factor_id, position_ids, shifts,
/// position_pnls }`, the Rust serde form) per shocked factor.
/// @param positions_json - Canonical portfolio-positions JSON to bump and revalue.
/// @param factors_json - Canonical factor-definition JSON identifying the market factors to shock.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param base_currency - ISO reporting currency for all returned monetary exposures; missing FX throws an error.
/// @param bump_config_json - Canonical bump-configuration JSON defining factor shock sizes and conventions.
/// @param n_scenario_points - Odd number of evenly spaced bump levels in each P-and-L profile, in `3..=1001`; omit for 5.
///
/// # Errors
///
/// Throws a JavaScript exception if `asOf` is not a valid ISO date; any JSON
/// input is malformed; a factor, bump configuration, or scenario-point count is
/// invalid or unsupported; bumping or repricing fails; or the profiles cannot
/// be converted to a JavaScript value.
#[wasm_bindgen(js_name = computePnlProfiles)]
pub fn compute_pnl_profiles(
    positions_json: JsValue,
    factors_json: JsValue,
    market_json: JsValue,
    as_of: JsValue,
    base_currency: JsValue,
    bump_config_json: Option<JsValue>,
    n_scenario_points: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let positions_json: &str = &json_text(&positions_json, "positionsJson")?;
    let factors_json: &str = &json_text(&factors_json, "factorsJson")?;
    let market_json: &str = &json_text(&market_json, "marketJson")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let base_currency: &str = &js_string(&base_currency, "baseCurrency")?;
    let bump_config_json = opt_json_text(bump_config_json.as_ref(), "bumpConfigJson")?;
    let n_scenario_points: Option<usize> =
        js_opt_uint(n_scenario_points.as_ref(), "nScenarioPoints")?;
    let as_of = parse_iso_date(as_of)?;
    let base_currency = base_currency
        .parse::<finstack_quant_core::currency::Currency>()
        .map_err(to_js_err)?;
    let market: finstack_quant_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;
    let profiles = finstack_quant_portfolio::sensitivity::compute_pnl_profiles_from_json(
        positions_json,
        factors_json,
        &market,
        as_of,
        base_currency,
        bump_config_json.as_deref(),
        n_scenario_points
            .unwrap_or(finstack_quant_portfolio::sensitivity::DEFAULT_PNL_SCENARIO_POINTS),
    )
    .map_err(to_js_err)?;
    to_js_value(&profiles)
}

/// Compute scenario P&L profiles using a pre-parsed [`JsMarket`].
/// @param positions_json - Canonical portfolio-positions JSON to bump and revalue.
/// @param factors_json - Canonical factor-definition JSON identifying the market factors to shock.
/// @param market - Market context or JSON payload supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param base_currency - ISO reporting currency for all returned monetary exposures; missing FX throws an error.
/// @param bump_config_json - Canonical bump-configuration JSON defining factor shock sizes and conventions.
/// @param n_scenario_points - Odd number of evenly spaced bump levels in each P-and-L profile, in `3..=1001`; omit for 5.
///
/// # Errors
///
/// Throws a JavaScript exception if `asOf` is not a valid ISO date; a position,
/// factor, or bump-config JSON input is malformed; a factor or scenario-point
/// count is invalid or unsupported; bumping or repricing fails; or the profiles
/// cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = computePnlProfilesWithMarket)]
pub fn compute_pnl_profiles_with_market(
    positions_json: JsValue,
    factors_json: JsValue,
    market: &JsMarket,
    as_of: JsValue,
    base_currency: JsValue,
    bump_config_json: Option<JsValue>,
    n_scenario_points: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let positions_json: &str = &json_text(&positions_json, "positionsJson")?;
    let factors_json: &str = &json_text(&factors_json, "factorsJson")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let base_currency: &str = &js_string(&base_currency, "baseCurrency")?;
    let bump_config_json = opt_json_text(bump_config_json.as_ref(), "bumpConfigJson")?;
    let n_scenario_points: Option<usize> =
        js_opt_uint(n_scenario_points.as_ref(), "nScenarioPoints")?;
    let as_of = parse_iso_date(as_of)?;
    let base_currency = base_currency
        .parse::<finstack_quant_core::currency::Currency>()
        .map_err(to_js_err)?;
    let profiles = finstack_quant_portfolio::sensitivity::compute_pnl_profiles_from_json(
        positions_json,
        factors_json,
        market.inner(),
        as_of,
        base_currency,
        bump_config_json.as_deref(),
        n_scenario_points
            .unwrap_or(finstack_quant_portfolio::sensitivity::DEFAULT_PNL_SCENARIO_POINTS),
    )
    .map_err(to_js_err)?;
    to_js_value(&profiles)
}

/// Decompose portfolio risk into factor and position contributions.
///
/// Uses the parametric (covariance-based) Euler decomposition.  Accepts
/// the canonical sensitivity-matrix wire object (the output of
/// `computeFactorSensitivities`, or the Python `SensitivityMatrix.to_json`),
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
/// `FactorRiskDecomposition.measure` getter returns the same value.
/// @param sensitivities_json - Canonical sensitivity-matrix JSON `{ base_currency, position_ids, factor_ids, data }` with one `data` row per position and one entry per factor; unknown keys are rejected.
/// @param covariance_json - Factor covariance-matrix JSON aligned with the supplied sensitivities.
/// @param risk_measure_json - Risk-measure JSON selecting the decomposition metric; omit for `"variance"`.
///
/// # Errors
///
/// Throws a JavaScript exception (`kind` `validation`) if any JSON input is
/// malformed or has unknown keys; `base_currency` is not an ISO-4217 code; the
/// sensitivity rows do not match `position_ids` / `factor_ids`; sensitivity and
/// covariance factor axes disagree; the covariance matrix or risk measure is
/// invalid; or decomposition produces invalid variance or another non-finite
/// value.
#[wasm_bindgen(js_name = decomposeFactorRisk)]
pub fn decompose_factor_risk(
    sensitivities_json: JsValue,
    covariance_json: JsValue,
    risk_measure_json: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let sensitivities_json: &str = &json_text(&sensitivities_json, "sensitivitiesJson")?;
    let covariance_json: &str = &json_text(&covariance_json, "covarianceJson")?;
    let risk_measure_json = opt_json_text(risk_measure_json.as_ref(), "riskMeasureJson")?;
    let wire: finstack_quant_portfolio::sensitivity::SensitivityMatrixJson =
        serde_json::from_str(sensitivities_json).map_err(to_js_err)?;
    let matrix = finstack_quant_portfolio::sensitivity::SensitivityMatrix::try_from(wire)
        .map_err(to_js_err)?;
    let covariance: finstack_quant_models::factor::FactorCovarianceMatrix =
        serde_json::from_str(covariance_json).map_err(to_js_err)?;
    let measure: finstack_quant_models::factor::RiskMeasure = risk_measure_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(to_js_err)?
        .unwrap_or_default();
    let result = finstack_quant_models::factor::risk::ParametricDecomposer
        .decompose(&matrix, &covariance, &measure)
        .map_err(to_js_err)?;
    to_js_value(&result)
}
