//! WASM bindings for the `finstack-covenants` crate.

use crate::utils::to_js_err;
use wasm_bindgen::prelude::*;

/// Validate and canonicalize a covenant spec JSON string.
#[wasm_bindgen(js_name = validateCovenantSpec)]
pub fn validate_covenant_spec(spec_json: &str) -> Result<String, JsValue> {
    finstack_covenants::validate_covenant_spec_json(spec_json).map_err(to_js_err)
}

/// Validate and canonicalize a covenant report JSON string.
#[wasm_bindgen(js_name = validateCovenantReport)]
pub fn validate_covenant_report(report_json: &str) -> Result<String, JsValue> {
    finstack_covenants::validate_covenant_report_json(report_json).map_err(to_js_err)
}

/// Validate and canonicalize a covenant engine JSON string.
#[wasm_bindgen(js_name = validateCovenantEngine)]
pub fn validate_covenant_engine(engine_json: &str) -> Result<String, JsValue> {
    finstack_covenants::validate_covenant_engine_json(engine_json).map_err(to_js_err)
}

/// Evaluate a covenant engine JSON string against a JSON metric map.
#[wasm_bindgen(js_name = evaluateEngine)]
pub fn evaluate_engine(
    engine_json: &str,
    metrics_json: &str,
    as_of: &str,
) -> Result<String, JsValue> {
    finstack_covenants::evaluate_engine_json(engine_json, metrics_json, as_of).map_err(to_js_err)
}

/// Standard leveraged-buyout covenant package as JSON.
#[wasm_bindgen(js_name = lboStandard)]
pub fn lbo_standard(
    initial_leverage: f64,
    interest_coverage: f64,
    fixed_charge_coverage: f64,
    max_capex: f64,
) -> Result<String, JsValue> {
    finstack_covenants::lbo_standard_json(
        initial_leverage,
        interest_coverage,
        fixed_charge_coverage,
        max_capex,
    )
    .map_err(to_js_err)
}

/// Covenant-lite package as JSON.
#[wasm_bindgen(js_name = covLite)]
pub fn cov_lite(max_leverage: f64, max_senior_leverage: f64) -> Result<String, JsValue> {
    finstack_covenants::cov_lite_json(max_leverage, max_senior_leverage).map_err(to_js_err)
}

/// Real-estate covenant package as JSON.
#[wasm_bindgen(js_name = realEstate)]
pub fn real_estate(min_dscr: f64, min_debt_yield: f64, max_ltv: f64) -> Result<String, JsValue> {
    finstack_covenants::real_estate_json(min_dscr, min_debt_yield, max_ltv).map_err(to_js_err)
}

/// Project-finance covenant package as JSON.
#[wasm_bindgen(js_name = projectFinance)]
pub fn project_finance(
    min_dscr: f64,
    distribution_lockup_dscr: f64,
    min_liquidity: f64,
    max_net_leverage: f64,
) -> Result<String, JsValue> {
    finstack_covenants::project_finance_json(
        min_dscr,
        distribution_lockup_dscr,
        min_liquidity,
        max_net_leverage,
    )
    .map_err(to_js_err)
}
