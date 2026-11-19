//! Standalone SIMM calculation for instrument envelopes.

use crate::api::core::market_context::JsMarketContext;
use crate::utils::input::{js_opt_currency, js_opt_string, js_opt_uint, json_text};
use crate::utils::wire::js_date;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_margin::SimmVersion;
use finstack_quant_valuations::pricer::parse_boxed_instrument_from_json;
use wasm_bindgen::prelude::*;

/// Calculate model SIMM for a single selected set of trades.
/// @param instruments - Canonical instrument envelopes from one netting set. Each trades at its own contractual amount and direction.
/// @param market - MarketContext supplying the trades' SIMM risk factors.
/// @param asOf - ISO-8601 valuation date used for sensitivities and the result.
/// @param version - Embedded SIMM label, omitted for the registered default.
/// @param mporDays - Positive margin period of risk in business days, omitted for the version's default.
/// @param currency - USD calculation currency, omitted for USD. Sensitivities must already be USD.
/// @returns Initial margin in USD, with breakdown, methodology, valuation date and approximation flag.
/// @throws TypeError for a mistyped input, or FinstackError for an invalid instrument, empty set, unsupported currency, invalid version or margin period, missing market data or failed repricing.
#[wasm_bindgen(js_name = calculateSimm, unchecked_return_type = "generated.margin.ImResult")]
pub fn calculate_simm(
    #[wasm_bindgen(
        unchecked_param_type = "readonly (generated.valuations.InstrumentEnvelope | string)[]"
    )]
    instruments: JsValue,
    market: &JsMarketContext,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "string | null")] version: Option<JsValue>,
    #[wasm_bindgen(js_name = "mporDays", unchecked_optional_param_type = "number | null")]
    mpor_days: Option<JsValue>,
    #[wasm_bindgen(unchecked_optional_param_type = "string | null")] currency: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    if !js_sys::Array::is_array(&instruments) {
        return Err(crate::utils::input::invalid_type(
            "instruments",
            "expected an array",
        ));
    }
    let instruments = js_sys::Array::from(&instruments)
        .iter()
        .map(|item| {
            parse_boxed_instrument_from_json(&json_text(&item, "instrument")?, None, None)
                .map_err(to_js_err)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let instruments = instruments
        .iter()
        .map(|item| item.as_instrument())
        .collect::<Vec<_>>();
    let as_of = js_date(&as_of, "asOf")?;
    let version = js_opt_string(version.as_ref(), "version")?
        .map(|value| value.parse::<SimmVersion>())
        .transpose()
        .map_err(to_js_err)?;
    let mpor_days = js_opt_uint(mpor_days.as_ref(), "mporDays")?;
    let currency = js_opt_currency(currency.as_ref(), "currency")?;
    let result = finstack_quant_valuations::margin::calculate_simm(
        &instruments,
        market.inner(),
        as_of,
        version,
        mpor_days,
        currency,
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}
