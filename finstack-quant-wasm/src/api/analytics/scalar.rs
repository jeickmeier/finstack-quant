//! Standalone return-risk metrics with explicit financial inputs.

use crate::utils::input::{js_f64, js_f64_seq};
use crate::utils::to_js_err;
use wasm_bindgen::prelude::*;

/// Arithmetic cash-excess mean divided by sample risk, scaled by square-root frequency.
/// @param returns - Ordered finite simple investment returns at least -1; a terminal -1 is allowed only at the end.
/// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; geometrically converted to an observation-period rate.
/// @param periodsPerYear - Required finite positive observation frequency; dates are not inferred.
/// @returns Annualized Sharpe ratio; NaN for fewer than two observations or zero over zero, and signed infinity for genuine zero risk with nonzero excess.
/// @throws Error - Rejects invalid return/cash/frequency domains and unrepresentable numerical results.
/// @example
/// ```typescript
/// import init, { analytics } from "finstack-quant-wasm";
/// await init();
/// console.log(analytics.sharpe([0.02, 0, -0.01], 0, 12));
/// ```
#[wasm_bindgen]
pub fn sharpe(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] returns: JsValue,
    #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
    risk_free_rate: JsValue,
    #[wasm_bindgen(js_name = "periodsPerYear", unchecked_param_type = "number")]
    periods_per_year: JsValue,
) -> Result<f64, JsValue> {
    let returns = js_f64_seq(&returns, "returns")?;
    let risk_free_rate = js_f64(&risk_free_rate, "riskFreeRate")?;
    let periods_per_year = js_f64(&periods_per_year, "periodsPerYear")?;
    finstack_quant_analytics::sharpe(&returns, risk_free_rate, periods_per_year).map_err(to_js_err)
}

/// All-observation downside Sortino ratio using an explicit per-observation hurdle.
/// @param returns - Ordered finite simple investment returns at least -1; a terminal -1 is allowed only at the end.
/// @param mar - Required finite per-observation decimal minimum acceptable return; no annual cash reinterpretation.
/// @param periodsPerYear - Required finite positive observation frequency used for square-root scaling.
/// @returns Annualized Sortino; NaN with fewer than two observations or zero over zero, and positive infinity with nonzero excess and genuine zero downside.
/// @throws Error - Rejects invalid return/frequency/MAR domains and unrepresentable numerical results.
/// @example
/// ```typescript
/// import init, { analytics } from "finstack-quant-wasm";
/// await init();
/// console.log(analytics.sortino([0.02, 0, -0.01], 0, 12)); // 2
/// ```
#[wasm_bindgen]
pub fn sortino(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] returns: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] mar: JsValue,
    #[wasm_bindgen(js_name = "periodsPerYear", unchecked_param_type = "number")]
    periods_per_year: JsValue,
) -> Result<f64, JsValue> {
    let returns = js_f64_seq(&returns, "returns")?;
    let mar = js_f64(&mar, "mar")?;
    let periods_per_year = js_f64(&periods_per_year, "periodsPerYear")?;
    finstack_quant_analytics::sortino(&returns, mar, periods_per_year).map_err(to_js_err)
}

/// Sample standard deviation with an n-1 denominator and square-root frequency scaling.
/// @param returns - Ordered finite simple investment returns at least -1; a terminal -1 is allowed only at the end.
/// @param periodsPerYear - Required finite positive number of observations per year.
/// @returns Annualized decimal sample volatility; NaN with fewer than two observations and zero for a sufficiently observed constant series.
/// @throws Error - Rejects invalid return/frequency domains and unrepresentable numerical results.
/// @example
/// ```typescript
/// import init, { analytics } from "finstack-quant-wasm";
/// await init();
/// console.log(analytics.volatility([0.01, -0.01], 12));
/// ```
#[wasm_bindgen]
pub fn volatility(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] returns: JsValue,
    #[wasm_bindgen(js_name = "periodsPerYear", unchecked_param_type = "number")]
    periods_per_year: JsValue,
) -> Result<f64, JsValue> {
    let returns = js_f64_seq(&returns, "returns")?;
    let periods_per_year = js_f64(&periods_per_year, "periodsPerYear")?;
    finstack_quant_analytics::volatility(&returns, periods_per_year).map_err(to_js_err)
}

/// Maximum geometric peak-to-trough percentage decline from initial unit wealth.
/// @param returns - Ordered finite simple investment returns at least -1; exact terminal -1 is allowed only at the end and yields complete drawdown.
/// @returns Nonpositive decimal depth, such as -0.25 for a 25% loss; NaN for empty history and zero for an observed path without decline.
/// @throws Error - Rejects malformed investment-return histories and unrepresentable numerical results.
/// @example
/// ```typescript
/// import init, { analytics } from "finstack-quant-wasm";
/// await init();
/// console.log(analytics.maxDrawdown([0.1, -1])); // -1
/// ```
#[wasm_bindgen(js_name = maxDrawdown)]
pub fn max_drawdown(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] returns: JsValue,
) -> Result<f64, JsValue> {
    let returns = js_f64_seq(&returns, "returns")?;
    finstack_quant_analytics::max_drawdown(&returns).map_err(to_js_err)
}
