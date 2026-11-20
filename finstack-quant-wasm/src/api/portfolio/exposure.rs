//! Dated portfolio and instrument exposure simulation.

use super::JsPortfolio;
use crate::api::core::config::JsFinstackConfig;
use crate::api::core::market_context::JsMarketContext;
use crate::utils::input::{from_js_json, js_currency, json_text};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_portfolio::margin;
use finstack_quant_valuations::pricer::parse_boxed_instrument_from_json;
use wasm_bindgen::prelude::*;

/// Reprice an admitted netting set on complete dated market paths.
/// @param portfolio - One legally nettable book and its signed positions and reporting currency.
/// @param market - Immutable opening market with all required prices, curves, fixings and FX.
/// @param pricing - Shared valuation and FX conversion settings.
/// @param spec - Dated path simulation, exact risk-factor bindings and netting scope.
/// @returns Calendar observations, expected exposures, basis and completeness evidence.
/// @throws FinstackError for an invalid path grid, ambiguous netting or risk source, missing market data or failed repricing.
#[wasm_bindgen(js_name = portfolioExposureProfile, unchecked_return_type = "generated.margin.ExposureProfile")]
pub fn portfolio_exposure_profile(
    portfolio: &JsPortfolio,
    market: &JsMarketContext,
    pricing: &JsFinstackConfig,
    #[wasm_bindgen(unchecked_param_type = "generated.portfolio.ExposureSimulationConfig | string")]
    spec: JsValue,
) -> Result<JsValue, JsValue> {
    let spec = from_js_json(&spec, "spec")?;
    let profile =
        margin::portfolio_exposure_profile(&portfolio.inner, market.inner(), &pricing.inner, &spec)
            .map_err(to_js_err)?;
    to_js_value(&profile)
}

/// Simulate complete dated exposure at one instrument's contractual size.
/// @param instrument - Validated instrument envelope, as a typed object or canonical JSON.
/// @param currency - ISO 4217 reporting currency for marks and exposures.
/// @param market - Immutable opening market with its required pricing and factor observations.
/// @param pricing - Valuation and FX conversion settings shared across the paths.
/// @param spec - Dated factor simulation with the instrument ID as the single scoped position.
/// @returns Complete dated exposure moments with the selected basis and diagnostics.
/// @throws TypeError for mistyped host values, or FinstackError for invalid scope, missing market data or failed repricing.
#[wasm_bindgen(js_name = instrumentExposureProfile, unchecked_return_type = "generated.margin.ExposureProfile")]
pub fn instrument_exposure_profile(
    #[wasm_bindgen(unchecked_param_type = "generated.valuations.InstrumentEnvelope | string")]
    instrument: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
    market: &JsMarketContext,
    pricing: &JsFinstackConfig,
    #[wasm_bindgen(unchecked_param_type = "generated.portfolio.ExposureSimulationConfig | string")]
    spec: JsValue,
) -> Result<JsValue, JsValue> {
    let instrument =
        parse_boxed_instrument_from_json(&json_text(&instrument, "instrument")?, None, None)
            .map_err(to_js_err)?;
    let currency = js_currency(&currency, "currency")?;
    let spec = from_js_json(&spec, "spec")?;
    let profile = margin::instrument_exposure_profile(
        instrument.as_instrument(),
        currency,
        market.inner(),
        &pricing.inner,
        &spec,
    )
    .map_err(to_js_err)?;
    to_js_value(&profile)
}
