//! WASM bindings for the `finstack-quant-covenants` crate.

use crate::utils::to_js_err;
use wasm_bindgen::prelude::*;

/// Validate and canonicalize a covenant spec JSON string.
/// @param spec_json - JSON-serialized covenant specification to validate.
///
/// # Errors
///
/// Throws a JavaScript exception if `specJson` is malformed, does not match the
/// covenant-spec schema, violates covenant threshold or frequency invariants, or
/// cannot be serialized to canonical JSON.
#[wasm_bindgen(js_name = validateCovenantSpecJson)]
pub fn validate_covenant_spec_json(spec_json: &str) -> Result<String, JsValue> {
    finstack_quant_covenants::validate_covenant_spec_json(spec_json).map_err(to_js_err)
}

/// Validate and canonicalize a covenant report JSON string.
/// @param report_json - JSON-serialized covenant evaluation report to validate.
///
/// # Errors
///
/// Throws a JavaScript exception if `reportJson` is malformed, does not match the
/// covenant-report schema, or cannot be serialized to canonical JSON.
#[wasm_bindgen(js_name = validateCovenantReportJson)]
pub fn validate_covenant_report_json(report_json: &str) -> Result<String, JsValue> {
    finstack_quant_covenants::validate_covenant_report_json(report_json).map_err(to_js_err)
}

/// Validate and canonicalize a covenant engine JSON string.
/// @param engine_json - JSON-serialized covenant engine and its covenant definitions.
///
/// # Errors
///
/// Throws a JavaScript exception if `engineJson` is malformed, does not match the
/// covenant-engine schema, contains an invalid covenant package, violates engine
/// invariants, or cannot be serialized to canonical JSON.
#[wasm_bindgen(js_name = validateCovenantEngineJson)]
pub fn validate_covenant_engine_json(engine_json: &str) -> Result<String, JsValue> {
    finstack_quant_covenants::validate_covenant_engine_json(engine_json).map_err(to_js_err)
}

/// Evaluate a covenant engine JSON string against a JSON metric map.
///
/// Returns a plain JavaScript object keyed by the engine's stable covenant
/// instance key, each value a covenant report carrying `covenant_type`,
/// `covenant_id`, `passed`, `actual_value`, `threshold`, `details`,
/// `headroom`, and `meta`. This mirrors the Python binding, which returns
/// `dict[str, CovenantReport]` from the same Rust entry point.
///
/// @param engine_json - JSON-serialized covenant engine and its covenant definitions.
/// @param metrics_json - JSON object of financial metrics referenced by the covenant engine.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
///
/// # Errors
///
/// Throws a JavaScript exception if either JSON input is malformed or has the
/// wrong schema, a metric is non-numeric, `asOf` is not a valid ISO date, the
/// engine or required metrics fail validation, or the reports cannot be
/// serialized to JavaScript.
#[wasm_bindgen(js_name = evaluateEngine)]
pub fn evaluate_engine(
    engine_json: &str,
    metrics_json: &str,
    as_of: &str,
) -> Result<JsValue, JsValue> {
    let reports = finstack_quant_covenants::evaluate_engine_map(engine_json, metrics_json, as_of)
        .map_err(to_js_err)?;
    crate::utils::to_js_value(&reports)
}

/// Standard leveraged-buyout covenant package as JSON.
/// @param initial_leverage - Maximum leverage ratio permitted at the initial test date.
/// @param interest_coverage - Minimum EBIT-to-interest coverage ratio in turns.
/// @param fixed_charge_coverage - Minimum EBITDA-to-fixed-charges coverage ratio.
/// @param max_capex - Maximum annual capital expenditure amount in the caller's reporting currency.
///
/// # Errors
///
/// Throws a JavaScript exception if any threshold is `NaN`, infinite or
/// negative, or if the generated covenant package cannot be serialized to
/// JSON.
#[wasm_bindgen(js_name = lboStandardJson)]
pub fn lbo_standard_json(
    initial_leverage: f64,
    interest_coverage: f64,
    fixed_charge_coverage: f64,
    max_capex: f64,
) -> Result<String, JsValue> {
    finstack_quant_covenants::lbo_standard_json(
        initial_leverage,
        interest_coverage,
        fixed_charge_coverage,
        max_capex,
    )
    .map_err(to_js_err)
}

/// Covenant-lite package as JSON.
/// @param max_leverage - Maximum total debt-to-EBITDA leverage ratio.
/// @param max_senior_leverage - Maximum senior-debt-to-EBITDA leverage ratio.
///
/// # Errors
///
/// Throws a JavaScript exception if any threshold is `NaN`, infinite or
/// negative, or if the generated covenant package cannot be serialized to
/// JSON.
#[wasm_bindgen(js_name = covLiteJson)]
pub fn cov_lite_json(max_leverage: f64, max_senior_leverage: f64) -> Result<String, JsValue> {
    finstack_quant_covenants::cov_lite_json(max_leverage, max_senior_leverage).map_err(to_js_err)
}

/// Real-estate covenant package as JSON.
/// @param min_dscr - Minimum debt-service coverage ratio.
/// @param min_debt_yield - Minimum net-operating-income debt yield expressed as a decimal.
/// @param max_ltv - Maximum loan-to-value ratio expressed as a decimal.
///
/// # Errors
///
/// Throws a JavaScript exception if any threshold is `NaN`, infinite or
/// negative, or if the generated covenant package cannot be serialized to
/// JSON.
#[wasm_bindgen(js_name = realEstateJson)]
pub fn real_estate_json(
    min_dscr: f64,
    min_debt_yield: f64,
    max_ltv: f64,
) -> Result<String, JsValue> {
    finstack_quant_covenants::real_estate_json(min_dscr, min_debt_yield, max_ltv).map_err(to_js_err)
}

/// Project-finance covenant package as JSON.
/// @param min_dscr - Minimum debt-service coverage ratio.
/// @param distribution_lockup_dscr - DSCR threshold below which borrower distributions are locked up.
/// @param min_liquidity - Minimum required liquidity reserve in the model's monetary units.
/// @param max_net_leverage - Maximum net-debt-to-EBITDA leverage ratio.
///
/// # Errors
///
/// Throws a JavaScript exception if any threshold is `NaN`, infinite or
/// negative, or if the generated covenant package cannot be serialized to
/// JSON.
#[wasm_bindgen(js_name = projectFinanceJson)]
pub fn project_finance_json(
    min_dscr: f64,
    distribution_lockup_dscr: f64,
    min_liquidity: f64,
    max_net_leverage: f64,
) -> Result<String, JsValue> {
    finstack_quant_covenants::project_finance_json(
        min_dscr,
        distribution_lockup_dscr,
        min_liquidity,
        max_net_leverage,
    )
    .map_err(to_js_err)
}
