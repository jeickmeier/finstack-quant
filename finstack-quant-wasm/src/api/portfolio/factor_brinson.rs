//! WASM bindings for factor-Brinson attribution.

use crate::utils::input::{js_f64_seq, json_text};

use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Compute Jeet-Partani (2023) factor-Brinson unified attribution.
///
/// Binds Rust `factor_brinson_attribution`: generalizes classical
/// Brinson-Fachler allocation/selection to continuous factor exposures by
/// replacing the sector partition with a factor-exposure matrix and a
/// caller-supplied benchmark factor-return vector. Returns a structured
/// `FactorBrinsonResult` object with `allocation`, `selection`, and their
/// per-factor / per-asset breakdowns.
/// @param inputJson - Canonical JSON `FactorBrinsonInput` with `asset_ids`, `asset_returns`, `exposures` (row-major n_assets x n_factors), `factor_names`, `portfolio_weights` and `benchmark_weights`; each weight vector must sum to 1.
/// @param factorReturns - Caller-supplied benchmark factor returns `f_b` as a `number[]` or `Float64Array`, length `input.factor_names`; the `Float64Array` returned by `analytics.constrainedLeastSquares` can be passed directly.
///
/// # Errors
///
/// Throws a JavaScript exception if `inputJson` is malformed; the asset or
/// factor sets are empty; dimensions disagree; a value is non-finite; either
/// weight vector does not sum to one; benchmark factor completeness is outside
/// tolerance; or the result cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = factorBrinsonAttribution)]
pub fn factor_brinson_attribution(
    input_json: JsValue,
    factor_returns: JsValue,
) -> Result<JsValue, JsValue> {
    let factor_returns = js_f64_seq(&factor_returns, "factorReturns")?;
    let input_json: &str = &json_text(&input_json, "inputJson")?;
    let input: finstack_quant_portfolio::factor_brinson::FactorBrinsonInput =
        serde_json::from_str(input_json).map_err(to_js_err)?;
    let result = finstack_quant_portfolio::factor_brinson::factor_brinson_attribution(
        &input,
        &factor_returns,
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}
