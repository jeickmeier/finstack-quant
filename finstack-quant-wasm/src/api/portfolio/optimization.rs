//! WASM bindings for portfolio optimization.

use super::JsPortfolio;
use crate::utils::input::json_text;

use crate::utils::input::from_js_json;
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Optimize portfolio weights using the LP-based optimizer.
///
/// Accepts a `PortfolioOptimizationSpec` JSON (portfolio + objective +
/// constraints + options) and a `MarketContext` JSON, and returns a structured
/// `PortfolioOptimizationResult` object.
/// @param specJson - `PortfolioOptimizationSpec` JSON: `portfolio` (a `PortfolioSpec`) plus `objective`, and optional `constraints`, `weighting`, `missing_metric_policy`, `label` and `trade_universe`.
/// @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
///
/// # Errors
///
/// Throws a JavaScript exception if either JSON input is malformed, the
/// portfolio, objective, constraints, weighting, or missing-metric policy is
/// invalid, a required market-dependent valuation fails, the solver cannot
/// produce a result, or the result cannot be converted to a JavaScript value.
/// @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a JSON string (integer-like map keys enumerate in numeric order, so it is not byte-identical to the Rust wire JSON).
#[wasm_bindgen(js_name = optimizePortfolio, unchecked_return_type = "PortfolioOptimizationResult")]
pub fn optimize_portfolio(
    #[wasm_bindgen(js_name = "specJson", unchecked_param_type = "JsonInput")] spec_json: JsValue,
    #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
    market_json: JsValue,
) -> Result<JsValue, JsValue> {
    let spec: finstack_quant_portfolio::optimization::PortfolioOptimizationSpec =
        from_js_json(&spec_json, "specJson")?;
    let market: finstack_quant_core::market_data::context::MarketContext =
        from_js_json(&market_json, "marketJson")?;
    let config = finstack_quant_core::config::FinstackConfig::default();
    let result =
        finstack_quant_portfolio::optimization::optimize_portfolio(&spec, &market, &config)
            .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Rebalance a spec's portfolio to an optimization result.
///
/// Wire twin of Python and Rust `optimization::rebalance_from_spec`: held positions take the
/// result's implied quantities and trade-universe candidates with a
/// non-negligible target weight and quantity are added as new positions.
/// @param specJson - The `PortfolioOptimizationSpec` JSON passed to `optimizePortfolio`.
/// @param resultJson - The `PortfolioOptimizationResult` that `optimizePortfolio` returned for that spec (object or JSON).
/// @returns The rebalanced, validated `Portfolio` handle.
///
/// # Errors
///
/// Throws with kind `validation` if either input is malformed, the solution is
/// infeasible, or the result names a position that is neither in the spec
/// portfolio nor a trade-universe candidate (a result paired with the wrong
/// spec), and propagates portfolio validation failures.
#[wasm_bindgen(js_name = rebalanceFromSpec)]
pub fn rebalance_from_spec(
    #[wasm_bindgen(js_name = "specJson", unchecked_param_type = "JsonInput")] spec_json: JsValue,
    #[wasm_bindgen(
        js_name = "resultJson",
        unchecked_param_type = "PortfolioOptimizationResult | string"
    )]
    result_json: JsValue,
) -> Result<JsPortfolio, JsValue> {
    let spec: finstack_quant_portfolio::optimization::PortfolioOptimizationSpec =
        serde_json::from_str(&json_text(&spec_json, "specJson")?).map_err(to_js_err)?;
    let result: finstack_quant_portfolio::optimization::PortfolioOptimizationResult =
        serde_json::from_str(&json_text(&result_json, "resultJson")?).map_err(to_js_err)?;
    let portfolio = finstack_quant_portfolio::optimization::rebalance_from_spec(&spec, &result)
        .map_err(to_js_err)?;
    Ok(JsPortfolio { inner: portfolio })
}
