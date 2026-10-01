//! WASM bindings for factor-model sensitivities and risk decomposition.

use crate::api::valuations::market_handle::JsMarket;
use crate::utils::date::parse_iso_date;
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Compute first-order factor sensitivities and return the matrix.
///
/// Accepts a JSON array of positions, a JSON array of `FactorDefinition`,
/// a `MarketContext` JSON, an ISO 8601 date, and an optional `BumpSizeConfig`
/// JSON.  Returns a structured object with `position_ids`, `factor_ids`, and a
/// row-major `data` matrix; `JSON.stringify` it to chain into
/// `decomposeFactorRisk`.
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
    positions_json: &str,
    factors_json: &str,
    market_json: &str,
    as_of: &str,
    base_currency: &str,
    bump_config_json: Option<String>,
) -> Result<JsValue, JsValue> {
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
    positions_json: &str,
    factors_json: &str,
    market: &JsMarket,
    as_of: &str,
    base_currency: &str,
    bump_config_json: Option<String>,
) -> Result<JsValue, JsValue> {
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
/// `{ factor_id, shifts, position_pnls }` entry per shocked factor.
/// @param positions_json - Canonical portfolio-positions JSON to bump and revalue.
/// @param factors_json - Canonical factor-definition JSON identifying the market factors to shock.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param base_currency - ISO reporting currency for all returned monetary exposures; missing FX throws an error.
/// @param bump_config_json - Canonical bump-configuration JSON defining factor shock sizes and conventions.
/// @param n_scenario_points - Positive number of evenly spaced bump levels in each P-and-L profile.
///
/// # Errors
///
/// Throws a JavaScript exception if `asOf` is not a valid ISO date; any JSON
/// input is malformed; a factor, bump configuration, or scenario-point count is
/// invalid or unsupported; bumping or repricing fails; or the profiles cannot
/// be converted to a JavaScript value.
#[wasm_bindgen(js_name = computePnlProfiles)]
pub fn compute_pnl_profiles(
    positions_json: &str,
    factors_json: &str,
    market_json: &str,
    as_of: &str,
    base_currency: &str,
    bump_config_json: Option<String>,
    n_scenario_points: Option<usize>,
) -> Result<JsValue, JsValue> {
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
    let output: Vec<finstack_quant_portfolio::sensitivity::FactorPnlProfileJson> = profiles
        .iter()
        .map(finstack_quant_portfolio::sensitivity::FactorPnlProfileJson::from)
        .collect();
    to_js_value(&output)
}

/// Compute scenario P&L profiles using a pre-parsed [`JsMarket`].
/// @param positions_json - Canonical portfolio-positions JSON to bump and revalue.
/// @param factors_json - Canonical factor-definition JSON identifying the market factors to shock.
/// @param market - Market context or JSON payload supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param base_currency - ISO reporting currency for all returned monetary exposures; missing FX throws an error.
/// @param bump_config_json - Canonical bump-configuration JSON defining factor shock sizes and conventions.
/// @param n_scenario_points - Positive number of evenly spaced bump levels in each P-and-L profile.
///
/// # Errors
///
/// Throws a JavaScript exception if `asOf` is not a valid ISO date; a position,
/// factor, or bump-config JSON input is malformed; a factor or scenario-point
/// count is invalid or unsupported; bumping or repricing fails; or the profiles
/// cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = computePnlProfilesWithMarket)]
pub fn compute_pnl_profiles_with_market(
    positions_json: &str,
    factors_json: &str,
    market: &JsMarket,
    as_of: &str,
    base_currency: &str,
    bump_config_json: Option<String>,
    n_scenario_points: Option<usize>,
) -> Result<JsValue, JsValue> {
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
    let output: Vec<finstack_quant_portfolio::sensitivity::FactorPnlProfileJson> = profiles
        .iter()
        .map(finstack_quant_portfolio::sensitivity::FactorPnlProfileJson::from)
        .collect();
    to_js_value(&output)
}

/// Decompose portfolio risk into factor and position contributions.
///
/// Uses the parametric (covariance-based) Euler decomposition.  Accepts
/// a JSON sensitivity matrix (same schema as the output of
/// `computeFactorSensitivities`), a `FactorCovarianceMatrix` JSON, and an
/// optional `RiskMeasure` JSON.
///
/// Returns a structured object with `total_risk`, `measure`, `residual_risk`,
/// `factor_contributions` (array), `position_factor_contributions` (array),
/// and `position_residual_contributions` (array; empty for the parametric
/// decomposer — populated only by credit-aware position decomposers).
///
/// `measure` uses the canonical serde form (`"variance"`, `"volatility"`, or
/// an object for `var` / `expected_shortfall`); the Python binding's
/// `measure` getter reports the same snake_case tag.
/// @param sensitivities_json - Canonical factor-sensitivity JSON with required `base_currency`, ordered `position_ids`/`factor_ids`, and nested `data[position][factor]` rows; `JSON.stringify` the result of `computeFactorSensitivities`. Python `SensitivityMatrix.to_json()` uses the same contract. Unknown fields, inconsistent dimensions, and non-finite entries are rejected.
/// @param covariance_json - Factor covariance-matrix JSON aligned with the supplied sensitivities.
/// @param risk_measure_json - Risk-measure configuration JSON selecting the decomposition metric.
///
/// # Errors
///
/// Throws a JavaScript exception if any JSON input is malformed; sensitivity
/// dimensions or factor axes disagree; the covariance matrix or risk measure is
/// invalid; decomposition produces invalid variance or another non-finite
/// value; or the result cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = decomposeFactorRisk)]
pub fn decompose_factor_risk(
    sensitivities_json: &str,
    covariance_json: &str,
    risk_measure_json: Option<String>,
) -> Result<JsValue, JsValue> {
    let input: finstack_quant_portfolio::sensitivity::SensitivityMatrixJson =
        serde_json::from_str(sensitivities_json).map_err(to_js_err)?;

    let covariance: finstack_quant_models::factor::FactorCovarianceMatrix =
        serde_json::from_str(covariance_json).map_err(to_js_err)?;

    let measure: finstack_quant_models::factor::RiskMeasure = match risk_measure_json {
        Some(ref json) => serde_json::from_str(json).map_err(to_js_err)?,
        None => finstack_quant_models::factor::RiskMeasure::Variance,
    };

    let decomposer = finstack_quant_models::factor::risk::ParametricDecomposer;
    let result = decomposer
        .decompose(&input.matrix, &covariance, &measure)
        .map_err(to_js_err)?;

    to_js_value(&result)
}
