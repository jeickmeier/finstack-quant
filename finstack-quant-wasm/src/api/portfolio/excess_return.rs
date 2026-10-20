//! WASM bindings for duration-cell credit excess returns.

use crate::utils::input::{js_f64, js_string, json_text};

use crate::api::core::market_data::JsDiscountCurve;
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Build a duration-cell base-return table from a reference universe.
///
/// Binds Rust `cell_returns_from_reference` (Dynkin, Hyman & Vankudre 1998,
/// Appendix B): buckets `referenceJson` into fixed-width duration cells and
/// averages each cell's member total returns, interpolating interior gaps
/// and flat-extrapolating leading/trailing gaps. Returns a structured
/// `DurationCellTable` object; `JSON.stringify` it to chain into
/// `excessReturns`.
/// @param referenceJson - Canonical JSON array of `ReferenceReturn` objects (`duration`, `total_return`, both decimals with duration in years); must be non-empty.
/// @param baseLabel - Label identifying the resulting curve (e.g. `"UST"`), carried through to the output's `base_label` for policy visibility.
/// @param configJson - Canonical JSON `CellConfig`; `width` is its only field (cell width in years, finite and positive) and is required, with no default.
///
/// # Errors
///
/// Throws a JavaScript exception if either JSON input is malformed, the
/// reference universe is empty or contains an invalid duration or return, the
/// cell width is not finite and positive, labels collide, the grid exceeds its
/// safety bound, or the result cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = cellReturnsFromReference)]
pub fn cell_returns_from_reference(
    reference_json: JsValue,
    base_label: JsValue,
    config_json: JsValue,
) -> Result<JsValue, JsValue> {
    let reference_json: &str = &json_text(&reference_json, "referenceJson")?;
    let base_label: &str = &js_string(&base_label, "baseLabel")?;
    let config_json: &str = &json_text(&config_json, "configJson")?;
    let reference: Vec<finstack_quant_portfolio::excess_return::ReferenceReturn> =
        serde_json::from_str(reference_json).map_err(to_js_err)?;
    let config: finstack_quant_portfolio::excess_return::CellConfig =
        serde_json::from_str(config_json).map_err(to_js_err)?;
    let table = finstack_quant_portfolio::excess_return::cell_returns_from_reference(
        &reference, base_label, &config,
    )
    .map_err(to_js_err)?;
    to_js_value(&table)
}

/// Build a duration-cell base-return table from start/end discount curves.
///
/// Binds Rust `cell_returns_from_curves`: each cell's base return is the
/// holding-period return of a hypothetical zero-coupon position bought at
/// the cell midpoint off `start` and revalued off `end` after
/// `horizonYears` have elapsed. Every resulting cell is observed, unlike the
/// reference-universe path in `cellReturnsFromReference`. Returns a structured
/// `DurationCellTable` object; `JSON.stringify` it to chain into
/// `excessReturns`.
/// @param start - Discount curve observed at the start of the holding period.
/// @param end - Discount curve observed `horizonYears` later, at period end.
/// @param horizonYears - Length of the holding period, in years; must be finite and positive.
/// @param maxDuration - Upper bound of the duration grid, in years; must be finite and strictly greater than `horizonYears`.
/// @param baseLabel - Label identifying the base curve (e.g. `"UST"`, `"USD-SOFR"`), stamped into the result purely for policy visibility.
/// @param configJson - Canonical JSON `CellConfig`; `width` is its only field and is required, with no default.
///
/// # Errors
///
/// Throws a JavaScript exception if `configJson` is malformed; the width,
/// horizon, or maximum duration is invalid; a cell matures within the holding
/// period; the grid is too large or has duplicate labels; a required discount
/// factor is not finite and positive; or the result cannot be converted to a
/// JavaScript value.
#[wasm_bindgen(js_name = cellReturnsFromCurves)]
pub fn cell_returns_from_curves(
    start: &JsDiscountCurve,
    end: &JsDiscountCurve,
    horizon_years: JsValue,
    max_duration: JsValue,
    base_label: JsValue,
    config_json: JsValue,
) -> Result<JsValue, JsValue> {
    let horizon_years = js_f64(&horizon_years, "horizonYears")?;
    let max_duration = js_f64(&max_duration, "maxDuration")?;
    let base_label: &str = &js_string(&base_label, "baseLabel")?;
    let config_json: &str = &json_text(&config_json, "configJson")?;
    let config: finstack_quant_portfolio::excess_return::CellConfig =
        serde_json::from_str(config_json).map_err(to_js_err)?;
    let table = finstack_quant_portfolio::excess_return::cell_returns_from_curves(
        &start.inner,
        &end.inner,
        horizon_years,
        max_duration,
        base_label,
        &config,
    )
    .map_err(to_js_err)?;
    to_js_value(&table)
}

/// Compute duration-matched credit excess returns against a base-return table.
///
/// Binds Rust `excess_returns` (Dynkin, Hyman & Vankudre 1998, Appendix B):
/// each position's `duration` is matched to its duration cell in
/// `tableJson` and the position's excess return is `total_return -
/// cell.base_return`, the credit-specific component of performance isolated
/// from the general level/shape move of the base curve. Returns a structured
/// `ExcessReturnResult` object with per-position and portfolio-level totals.
///
/// @param positionsJson - Canonical JSON array of `ExcessReturnPosition` objects (`id`, `weight`, `duration`, `total_return`); weights must sum to 1.
/// @param tableJson - Canonical JSON `DurationCellTable`; `JSON.stringify` the structured table returned by `cellReturnsFromReference` or `cellReturnsFromCurves`.
///
/// # Errors
///
/// Throws a JavaScript exception if either JSON input is malformed, the cell
/// table is invalid, a position is invalid or falls in no cell, position
/// weights do not sum to one, or the result cannot be converted to a JavaScript
/// value.
#[wasm_bindgen(js_name = excessReturns)]
pub fn excess_returns(positions_json: JsValue, table_json: JsValue) -> Result<JsValue, JsValue> {
    let positions_json: &str = &json_text(&positions_json, "positionsJson")?;
    let table_json: &str = &json_text(&table_json, "tableJson")?;
    let positions: Vec<finstack_quant_portfolio::excess_return::ExcessReturnPosition> =
        serde_json::from_str(positions_json).map_err(to_js_err)?;
    let table: finstack_quant_portfolio::excess_return::DurationCellTable =
        serde_json::from_str(table_json).map_err(to_js_err)?;
    let result = finstack_quant_portfolio::excess_return::excess_returns(&positions, &table)
        .map_err(to_js_err)?;
    to_js_value(&result)
}
