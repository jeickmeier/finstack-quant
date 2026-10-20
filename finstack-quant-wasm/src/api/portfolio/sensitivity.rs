//! WASM bindings for factor-model sensitivities and P&L profiles.

use crate::api::core::market_context::JsMarketContext;
use crate::utils::date::parse_iso_date;
use crate::utils::input::{js_opt_uint, js_string, json_text, opt_json_text};
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;
/// Compute first-order factor sensitivities using a pre-parsed `MarketContext` handle.
///
/// Avoids reparsing market JSON for repeated factor analytics calls.
/// @param positions_json - Canonical portfolio-positions JSON to bump and revalue.
/// @param factors_json - Canonical factor-definition JSON identifying the market factors to shock.
/// @param market - Pre-parsed `core.MarketContext` handle supplying curves, quotes, and FX data.
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
#[wasm_bindgen(js_name = computeFactorSensitivities)]
pub fn compute_factor_sensitivities(
    positions_json: JsValue,
    factors_json: JsValue,
    market: &JsMarketContext,
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
/// Compute scenario P&L profiles using a pre-parsed `MarketContext` handle.
/// @param positions_json - Canonical portfolio-positions JSON to bump and revalue.
/// @param factors_json - Canonical factor-definition JSON identifying the market factors to shock.
/// @param market - Pre-parsed `core.MarketContext` handle supplying curves, quotes, and FX data.
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
#[wasm_bindgen(js_name = computePnlProfiles)]
pub fn compute_pnl_profiles(
    positions_json: JsValue,
    factors_json: JsValue,
    market: &JsMarketContext,
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
