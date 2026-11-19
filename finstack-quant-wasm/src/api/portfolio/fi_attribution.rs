//! WASM bindings for Campisi fixed-income attribution.

use crate::utils::input::js_f64;

use crate::utils::input::from_js_json;
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Attribute dated fixed-income returns to allocation, coupon carry, Treasury,
/// settlement Z-spread and residual.
///
/// @param portfolio - Position snapshots with exact signed opening weights and realized returns on the declared basis.
/// @param benchmark - Benchmark snapshots on the same valuation, return, FX and fee basis.
/// @param config - Reporting currency, positive calendar window, conventions and ordered sector axis.
/// @returns Active return contributions and observed side returns as decimals. An absent side has a null sector return.
/// @throws FinstackError if the inputs fail validation, exact weights do not sum to one or an output is not representable.
/// @example
/// ```typescript
/// import init, { portfolio } from "finstack-quant-wasm";
/// await init();
/// const snapshot = {
///   id: "BOND", sector: "Credit", weight: "1", total_return: 0.02,
///   opening_market_value: { amount: "80", currency: "USD" },
///   annual_coupon: { amount: "5", currency: "USD" },
///   treasury: null, spread: null,
/// } as const;
/// const report = portfolio.campisiAttribution([snapshot], [snapshot], {
///   start: "2025-01-01", end: "2025-03-15", currency: "USD",
///   basis: { portfolio_series_id: "portfolio", benchmark_series_id: "benchmark", return_policy: "reinvested", valuation: "dirty", fx: "native", fees: "net" },
///   sectors: ["Credit"],
/// });
/// console.log(report.active_return); // 0
/// ```
#[wasm_bindgen(js_name = campisiAttribution, unchecked_return_type = "FiAttributionResult")]
pub fn campisi_attribution(
    #[wasm_bindgen(unchecked_param_type = "ReadonlyArray<generated.portfolio.FiPositionSnapshot>")]
    portfolio: JsValue,
    #[wasm_bindgen(unchecked_param_type = "ReadonlyArray<generated.portfolio.FiPositionSnapshot>")]
    benchmark: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.portfolio.FiAttributionConfig")]
    config: JsValue,
) -> Result<JsValue, JsValue> {
    let portfolio: Vec<finstack_quant_portfolio::fi_attribution::FiPositionSnapshot> =
        from_js_json(&portfolio, "portfolio")?;
    let benchmark: Vec<finstack_quant_portfolio::fi_attribution::FiPositionSnapshot> =
        from_js_json(&benchmark, "benchmark")?;
    let config: finstack_quant_portfolio::fi_attribution::FiAttributionConfig =
        from_js_json(&config, "config")?;
    let result = finstack_quant_portfolio::fi_attribution::campisi_attribution(
        &portfolio, &benchmark, &config,
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Link contiguous checked Campisi periods with Carino smoothing.
/// @param periods - Verified period results in chronological order on one series, currency, reporting basis and sector taxonomy.
/// @returns Linked decimal effects and the independently compounded active return, with source periods retained.
/// @throws FinstackError if the source observations do not reproduce the period, dates are not contiguous, growth is nonpositive, or a required result is unrepresentable.
#[wasm_bindgen(js_name = campisiCarinoLink, unchecked_return_type = "FiCarinoLinkedResult")]
pub fn campisi_carino_link(
    #[wasm_bindgen(unchecked_param_type = "readonly FiAttributionResult[]")] periods: JsValue,
) -> Result<JsValue, JsValue> {
    let periods: Vec<finstack_quant_portfolio::fi_attribution::FiAttributionResult> =
        from_js_json(&periods, "periods")?;
    let result = finstack_quant_portfolio::fi_attribution::campisi_carino_link(&periods)
        .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Compare displayed Campisi effects against the source-verified active return.
/// @param result - Canonical Campisi result, including the dated position observations needed for native replay.
/// @param tolerance - Finite nonnegative absolute tolerance in decimal return units.
/// @returns The numerical residual, requested tolerance and pass indicator.
/// @throws FinstackError if the result disagrees with its source, the tolerance is invalid or serialization fails.
#[wasm_bindgen(js_name = fiAttributionResultReconciliationCheck, unchecked_return_type = "ReconciliationReport")]
pub fn fi_attribution_result_reconciliation_check(
    #[wasm_bindgen(unchecked_param_type = "FiAttributionResult | string")] result: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] tolerance: JsValue,
) -> Result<JsValue, JsValue> {
    let tolerance = js_f64(&tolerance, "tolerance")?;
    let result: finstack_quant_portfolio::fi_attribution::FiAttributionResult =
        from_js_json(&result, "result")?;
    to_js_value(&result.reconciliation_check(tolerance).map_err(to_js_err)?)
}
