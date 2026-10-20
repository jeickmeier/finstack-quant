//! WASM bindings for portfolio factor sensitivities and P&L profiles.

use super::JsPortfolio;
use crate::api::core::market_context::JsMarketContext;
use crate::utils::date::parse_iso_date;
use crate::utils::input::{from_js_json, js_opt_uint, js_string, opt_json_text};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_factor::{BumpSizeConfig, FactorDefinition};
use wasm_bindgen::prelude::*;

/// Deserialize an optional `BumpSizeConfig` (JSON text or plain object).
fn parse_bump_config(value: Option<&JsValue>) -> Result<Option<BumpSizeConfig>, JsValue> {
    opt_json_text(value, "bumpConfigJson")?
        .map(|text| {
            serde_json::from_str(&text).map_err(|error| {
                to_js_err(finstack_quant_core::Error::Validation(format!(
                    "bumpConfigJson: {error}"
                )))
            })
        })
        .transpose()
}

/// Compute the position-by-factor sensitivity matrix of a portfolio.
///
/// Each cell is the central difference of the position's base-currency PV
/// under an up and a down bump of the factor, weighted by the position's scale
/// factor. Rows follow portfolio position order, columns follow the factors,
/// and every value is in the portfolio's base currency. Returns the
/// sensitivity-matrix wire object (`base_currency`, `position_ids`,
/// `factor_ids`, `data` rows).
/// @param portfolio - Built portfolio whose positions are the matrix rows; its base currency is the reporting currency.
/// @param factors_json - `FactorDefinition` array (object or JSON text); every factor must map to market data, so credit hierarchy factors need `FactorModel.computeSensitivities`.
/// @param market - `core.MarketContext` handle bumped per factor, including FX for cross-currency positions.
/// @param as_of - ISO-8601 valuation date for pricing and spot FX.
/// @param bump_config_json - Optional `BumpSizeConfig` (object or JSON text); omit for 1 bp / 1 % per factor type.
/// @returns The `SensitivityMatrixJson` object.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `asOf` is not a string or a
/// JSON argument is neither text nor a plain object/array, and a
/// `FinstackError` if `asOf` is not an ISO date, the factors or bump config do
/// not match their schema, a factor is a credit hierarchy factor (kind
/// `validation`), or bumping, repricing or FX conversion fails.
#[wasm_bindgen(js_name = computeFactorSensitivities)]
pub fn compute_factor_sensitivities(
    portfolio: &JsPortfolio,
    factors_json: JsValue,
    market: &JsMarketContext,
    as_of: JsValue,
    bump_config_json: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let factors: Vec<FactorDefinition> = from_js_json(&factors_json, "factorsJson")?;
    let as_of = parse_iso_date(&js_string(&as_of, "asOf")?)?;
    let bump_config = parse_bump_config(bump_config_json.as_ref())?;
    let matrix = finstack_quant_portfolio::sensitivity::compute_factor_sensitivities(
        &portfolio.inner,
        &factors,
        market.inner(),
        as_of,
        bump_config,
    )
    .map_err(to_js_err)?;
    to_js_value(
        &finstack_quant_portfolio::sensitivity::SensitivityMatrixJson::from_matrix(
            &matrix,
            portfolio.inner.base_currency,
        ),
    )
}

/// Compute full-repricing scenario P&L profiles of a portfolio, one per factor.
///
/// Each profile holds the scale-factor-weighted P&L of every position at each
/// grid shift, in the portfolio's base currency.
/// @param portfolio - Built portfolio whose positions are the profile columns; its base currency is the reporting currency.
/// @param factors_json - `FactorDefinition` array (object or JSON text) to shock, in returned profile order.
/// @param market - `core.MarketContext` handle reused by every scenario.
/// @param as_of - ISO-8601 valuation date for pricing and spot FX.
/// @param bump_config_json - Optional `BumpSizeConfig` (object or JSON text); omit for 1 bp / 1 % per factor type.
/// @param n_points - Odd number of grid points in `3..=1001`; omit for 5 (shifts `[-2, -1, 0, 1, 2]`).
/// @returns The `FactorPnlProfile` array.
///
/// # Errors
///
/// Throws a `TypeError` (kind `invalid_type`) if `asOf` is not a string,
/// `nPoints` is not a whole number, or a JSON argument is neither text nor a
/// plain object/array, and a `FinstackError` if `asOf` is not an ISO date, the
/// factors or bump config do not match their schema, the grid size is invalid
/// or a factor is a credit hierarchy factor (kind `validation`), or bumping,
/// repricing or FX conversion fails.
#[wasm_bindgen(js_name = computePnlProfiles)]
pub fn compute_pnl_profiles(
    portfolio: &JsPortfolio,
    factors_json: JsValue,
    market: &JsMarketContext,
    as_of: JsValue,
    bump_config_json: Option<JsValue>,
    n_points: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let factors: Vec<FactorDefinition> = from_js_json(&factors_json, "factorsJson")?;
    let as_of = parse_iso_date(&js_string(&as_of, "asOf")?)?;
    let bump_config = parse_bump_config(bump_config_json.as_ref())?;
    let n_points: Option<usize> = js_opt_uint(n_points.as_ref(), "nPoints")?;
    let profiles = finstack_quant_portfolio::sensitivity::compute_pnl_profiles(
        &portfolio.inner,
        &factors,
        market.inner(),
        as_of,
        bump_config,
        n_points,
    )
    .map_err(to_js_err)?;
    to_js_value(&profiles)
}
