//! WASM bindings for Brinson-Fachler attribution and Carino linking.

use crate::utils::input::from_js_json;
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Compute a single-period Brinson-Fachler attribution from sector JSON.
///
/// Accepts a JSON array of `SectorPeriod` objects and returns a structured
/// `BrinsonPeriodResult` object.
/// @param sectorsJson - Sector-classification JSON.
///
/// # Errors
///
/// Throws a JavaScript exception if `sectorsJson` is malformed, contains no
/// sectors or a non-finite weight or return, portfolio or benchmark weights do
/// not sum to one, or the result cannot be converted to a JavaScript value.
/// @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
#[wasm_bindgen(js_name = brinsonFachler, unchecked_return_type = "BrinsonPeriodResult")]
pub fn brinson_fachler(
    #[wasm_bindgen(js_name = "sectorsJson", unchecked_param_type = "JsonInput")]
    sectors_json: JsValue,
) -> Result<JsValue, JsValue> {
    let sectors: Vec<finstack_quant_portfolio::brinson::SectorPeriod> =
        from_js_json(&sectors_json, "sectorsJson")?;
    let result = finstack_quant_portfolio::brinson::brinson_fachler(&sectors).map_err(to_js_err)?;
    to_js_value(&result)
}

/// Carino-link already-computed single-period Brinson-Fachler results.
///
/// Binds Rust `carino_link`: accepts a chronological JSON array of
/// `BrinsonPeriodResult` objects (for example `brinsonFachler` outputs) and
/// returns a structured `CarinoLinkedAttribution` object whose linked effects
/// reconstruct the geometrically compounded active return. To link raw sector
/// inputs, attribute each period with `brinsonFachler` first.
/// @param periodsJson - Chronological JSON array of `BrinsonPeriodResult` objects with identical sector ordering in every period.
///
/// # Errors
///
/// Throws a JavaScript exception if `periodsJson` is malformed, the sequence is
/// empty or changes sector ordering, or a period return is non-finite or at
/// most `-1`.
/// @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
#[wasm_bindgen(js_name = carinoLink, unchecked_return_type = "CarinoLinkedAttribution")]
pub fn carino_link(
    #[wasm_bindgen(js_name = "periodsJson", unchecked_param_type = "JsonInput")]
    periods_json: JsValue,
) -> Result<JsValue, JsValue> {
    let periods: Vec<finstack_quant_portfolio::brinson::BrinsonPeriodResult> =
        from_js_json(&periods_json, "periodsJson")?;
    let result = finstack_quant_portfolio::brinson::carino_link(&periods).map_err(to_js_err)?;
    to_js_value(&result)
}

/// Attribute each period with Brinson-Fachler and Carino-link the results.
///
/// Binds Rust `brinson_linked`: the multi-period Brinson-Fachler report in one
/// call, identical to `carinoLink(periods.map(brinsonFachler))`. Returns a
/// structured `CarinoLinkedAttribution` object with every period's sector
/// allocation, selection and interaction (`periods`), the linked effects by
/// sector (`linked_sectors`) and by effect (`linked_allocation`,
/// `linked_selection`, `linked_interaction`), and the compounded portfolio and
/// benchmark returns; the linked effects sum to the compounded active return.
/// @param periods - Chronological array of periods (or its JSON text), each an array of `SectorPeriod` objects (`sector`, `portfolio_weight`, `benchmark_weight`, `portfolio_return`, `benchmark_return`). Weights are decimal fractions summing to one on each side; returns are decimal period returns (`0.015` is +1.5 %). Every period lists the same sectors in the same order; a sector absent on one side keeps a zero weight.
///
/// # Errors
///
/// Throws a JavaScript exception if `periods` is malformed or a row has an
/// unknown field, there are no periods, a period fails Brinson-Fachler (no
/// sectors, a non-finite input, or weights not summing to one; the message
/// names the zero-based period), the sector ordering changes between periods,
/// or a period return is at most `-1` (kind `validation`).
/// @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
#[wasm_bindgen(js_name = brinsonLinked, unchecked_return_type = "CarinoLinkedAttribution")]
pub fn brinson_linked(
    #[wasm_bindgen(unchecked_param_type = "generated.portfolio.SectorPeriod[][] | string")] periods: JsValue,
) -> Result<JsValue, JsValue> {
    let periods: Vec<Vec<finstack_quant_portfolio::brinson::SectorPeriod>> =
        from_js_json(&periods, "periods")?;
    let result = finstack_quant_portfolio::brinson::brinson_linked(&periods).map_err(to_js_err)?;
    to_js_value(&result)
}
