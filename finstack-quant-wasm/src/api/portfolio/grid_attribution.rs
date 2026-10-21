//! WASM bindings for duration-cell x sector grid attribution.

use crate::utils::input::from_js_json;
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Compute a single-period hierarchical duration-cell x sector grid attribution.
///
/// Binds Rust `grid_attribution` (Dynkin, Hyman & Vankudre 1998, Appendix A):
/// decomposes active return into a per-cell curve (positioning) effect, a
/// within-cell sector allocation effect, and a security-selection residual
/// per (cell, sector). Returns a structured `GridAttributionResult` object
/// (`JSON.stringify` it to chain into `gridCarinoLink`) whose
/// `total_curve`, `total_sector` and `total_selection` sum to
/// `active_return` to floating-point precision for well-conditioned inputs;
/// among accepted inputs, the reconciliation residual grows the closer any
/// bucket's net weight sits to the near-zero-net-weight rejection boundary
/// (see the Rust module docs for measured magnitudes).
/// @param portfolioJson - Canonical JSON array of `GridPosition` objects (`cell`, `sector`, `weight`, `total_return`) for the portfolio side; weights must sum to 1.
/// @param benchmarkJson - Canonical JSON array of `GridPosition` objects for the benchmark side; same weight-sum requirement.
///
/// # Errors
///
/// Throws a JavaScript exception if either JSON input is malformed, a weight or
/// return is non-finite, either side's weights do not sum to one, a cell or
/// cell-sector bucket has a zero or near-zero net weight relative to gross
/// weight, or the result cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = gridAttribution)]
pub fn grid_attribution(
    portfolio_json: JsValue,
    benchmark_json: JsValue,
) -> Result<JsValue, JsValue> {
    let portfolio: Vec<finstack_quant_portfolio::grid_attribution::GridPosition> =
        from_js_json(&portfolio_json, "portfolioJson")?;
    let benchmark: Vec<finstack_quant_portfolio::grid_attribution::GridPosition> =
        from_js_json(&benchmark_json, "benchmarkJson")?;
    let result =
        finstack_quant_portfolio::grid_attribution::grid_attribution(&portfolio, &benchmark)
            .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Carino-link multi-period hierarchical grid attribution results.
///
/// Binds Rust `grid_carino_link` (Carino 1999): applies Carino smoothing to
/// a chronological sequence of single-period `gridAttribution` results so
/// the three top-level effects (`linked_curve`, `linked_sector`,
/// `linked_selection`) sum exactly to the geometrically compounded active
/// return. Only the three top-level effects are linked; per-cell /
/// per-(cell, sector) multi-period linking is out of scope. Returns a structured
/// `GridCarinoLinkedResult` object.
/// @param periodsJson - Canonical JSON array of `GridAttributionResult` objects, in chronological order; `JSON.stringify` the structured results returned by `gridAttribution`.
///
/// # Errors
///
/// Throws a JavaScript exception if `periodsJson` is malformed, the sequence is
/// empty, a consumed value is non-finite or inconsistent, a return is at most
/// `-1`, or the linked result cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = gridCarinoLink)]
pub fn grid_carino_link(periods_json: JsValue) -> Result<JsValue, JsValue> {
    let periods: Vec<finstack_quant_portfolio::grid_attribution::GridAttributionResult> =
        from_js_json(&periods_json, "periodsJson")?;
    let result = finstack_quant_portfolio::grid_attribution::grid_carino_link(&periods)
        .map_err(to_js_err)?;
    to_js_value(&result)
}
