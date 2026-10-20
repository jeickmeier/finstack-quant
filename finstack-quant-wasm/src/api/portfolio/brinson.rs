//! WASM bindings for Brinson-Fachler attribution and Carino linking.

use crate::utils::input::from_js_json;
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Compute a single-period Brinson-Fachler attribution from sector JSON.
///
/// Accepts a JSON array of `SectorPeriod` objects and returns a structured
/// `BrinsonPeriodResult` object.
/// @param sectors_json - Sector-classification JSON.
///
/// # Errors
///
/// Throws a JavaScript exception if `sectorsJson` is malformed, contains no
/// sectors or a non-finite weight or return, portfolio or benchmark weights do
/// not sum to one, or the result cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = brinsonFachler)]
pub fn brinson_fachler(sectors_json: JsValue) -> Result<JsValue, JsValue> {
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
/// reconstruct the geometrically compounded active return. Use
/// `carinoLinkFromSectorPeriods` to link raw sector inputs instead.
/// @param periods_json - Chronological JSON array of `BrinsonPeriodResult` objects with identical sector ordering in every period.
///
/// # Errors
///
/// Throws a JavaScript exception if `periodsJson` is malformed, the sequence is
/// empty or changes sector ordering, or a period return is non-finite or at
/// most `-1`.
#[wasm_bindgen(js_name = carinoLink)]
pub fn carino_link(periods_json: JsValue) -> Result<JsValue, JsValue> {
    let periods: Vec<finstack_quant_portfolio::brinson::BrinsonPeriodResult> =
        from_js_json(&periods_json, "periodsJson")?;
    let result = finstack_quant_portfolio::brinson::carino_link(&periods).map_err(to_js_err)?;
    to_js_value(&result)
}

/// Compute Carino-linked multi-period Brinson attribution from raw sector
/// periods.
///
/// Binds Rust `carino_link_from_sector_periods`: runs `brinsonFachler` on each
/// period, then Carino-links the results. Returns a structured
/// `CarinoLinkedAttribution` object.
/// @param periods_json - Chronological JSON array of periods, each an array of `SectorPeriod` objects (`sector`, `portfolio_weight`, `benchmark_weight`, `portfolio_return`, `benchmark_return`).
///
/// # Errors
///
/// Throws a JavaScript exception if `periodsJson` is malformed, any period fails
/// Brinson validation, the sequence is empty or changes sector ordering, or a
/// period return is non-finite or at most `-1`.
#[wasm_bindgen(js_name = carinoLinkFromSectorPeriods)]
pub fn carino_link_from_sector_periods(periods_json: JsValue) -> Result<JsValue, JsValue> {
    let periods: Vec<Vec<finstack_quant_portfolio::brinson::SectorPeriod>> =
        from_js_json(&periods_json, "periodsJson")?;
    let result = finstack_quant_portfolio::brinson::carino_link_from_sector_periods(&periods)
        .map_err(to_js_err)?;
    to_js_value(&result)
}
