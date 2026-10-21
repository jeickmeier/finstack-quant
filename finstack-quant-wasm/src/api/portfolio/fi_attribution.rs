//! WASM bindings for Campisi fixed-income attribution.

use crate::utils::input::js_f64;

use crate::utils::input::from_js_json;
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Compute a single-period Campisi fixed-income attribution from JSON.
///
/// Decomposes both sides into carry / treasury / spread / selection and
/// splits the active return into allocation plus four active component
/// effects (Campisi 2000). Returns a structured `FiAttributionResult` object
/// to chain into `campisiCarinoLink` or
/// `fiAttributionResultReconciliationCheck`.
///
/// Every snapshot must use the quote-reproducing `z_spread` basis:
/// `spread_duration` is the canonical Z-spread duration, and `spread` plus
/// `delta_spread` are the matching Z-spread level and move. OAS, G-spread, or
/// discount-margin values are incompatible. The direct JSON shape contains
/// numeric values but no metric IDs, so this binding cannot detect mislabeled
/// spread provenance.
///
/// Throws when JSON is malformed or canonical Rust validation rejects empty
/// sides, non-finite values, invalid weights or period length, or a sector
/// present on either side has `|net sector weight| <= 1e-6 * gross absolute
/// sector weight`. Spread-basis provenance cannot be validated from numeric
/// JSON alone.
/// @param portfolio_json - Canonical JSON array of `FiPositionSnapshot` objects describing the portfolio side on the quote-reproducing Z-spread basis; weights must sum to 1.
/// @param benchmark_json - Canonical JSON array of `FiPositionSnapshot` objects describing the benchmark side on the quote-reproducing Z-spread basis; weights must sum to 1.
/// @param config_json - Canonical JSON `FiAttributionConfig`; `period_years` is its only field, is required (no default), and unknown keys are rejected.
///
/// # Errors
///
/// Throws a JavaScript exception if any JSON input is malformed; either side is
/// empty; a value is non-finite; weights do not sum to one; `periodYears` is not
/// finite and positive; a sector has a zero or near-zero net weight relative to
/// gross weight; or the result cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = campisiAttribution)]
pub fn campisi_attribution(
    portfolio_json: JsValue,
    benchmark_json: JsValue,
    config_json: JsValue,
) -> Result<JsValue, JsValue> {
    let portfolio: Vec<finstack_quant_portfolio::fi_attribution::FiPositionSnapshot> =
        from_js_json(&portfolio_json, "portfolioJson")?;
    let benchmark: Vec<finstack_quant_portfolio::fi_attribution::FiPositionSnapshot> =
        from_js_json(&benchmark_json, "benchmarkJson")?;
    let config: finstack_quant_portfolio::fi_attribution::FiAttributionConfig =
        from_js_json(&config_json, "configJson")?;
    let result = finstack_quant_portfolio::fi_attribution::campisi_attribution(
        &portfolio, &benchmark, &config,
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Carino-link already-computed single-period Campisi results.
///
/// Binds Rust `campisi_carino_link`. Attribute each period with
/// `campisiAttribution` first; each period carries its own already-applied
/// `period_years`, so periods of *different* lengths (e.g. act/365 calendar
/// months) link correctly. Returns a structured `FiCarinoLinkedResult` object.
///
/// Throws if no periods are supplied, sector ordering differs, a consumed
/// top-level return/effect, per-sector linked effect, or sector `total_active`
/// is non-finite, `active_return` disagrees with the portfolio-minus-benchmark
/// return, a sector `total_active` disagrees with its five effects, sector
/// effects do not reconcile to their declared top-level totals, the five
/// totals do not reconcile to `active_return` within the overflow-safe
/// scaled-L1 tolerance, a reconciliation residual is non-finite, or a return
/// is outside the Carino domain.
/// @param periods_json - Canonical JSON array of `FiAttributionResult` objects in chronological order, as returned by `campisiAttribution`.
///
/// # Errors
///
/// Throws a JavaScript exception if `periodsJson` is malformed, the sequence is
/// empty or changes sector ordering, a consumed value or reconciliation is
/// non-finite or inconsistent, a return is at most `-1`, or the linked result
/// cannot be converted to a JavaScript value.
#[wasm_bindgen(js_name = campisiCarinoLink)]
pub fn campisi_carino_link(periods_json: JsValue) -> Result<JsValue, JsValue> {
    let periods: Vec<finstack_quant_portfolio::fi_attribution::FiAttributionResult> =
        from_js_json(&periods_json, "periodsJson")?;
    let result = finstack_quant_portfolio::fi_attribution::campisi_carino_link(&periods)
        .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Reconcile the five Campisi effect totals against the active return.
///
/// Binds the Rust method `FiAttributionResult::reconciliation_check` (Python
/// `FiAttributionResult.reconciliation_check`); WASM results are plain
/// objects, so the method is a free function taking the object. The
/// decomposition reconciles by construction (selection is the residual), so
/// this is a floating-point sanity gate rather than a model check. Returns a
/// structured `ReconciliationReport` object with `total_residual`,
/// `is_reconciled` and `tolerance`.
/// @param result - `FiAttributionResult` object or JSON as returned by `campisiAttribution`; unknown fields are rejected.
/// @param tolerance - Absolute reconciliation tolerance in return units; `1e-10` suits return-space values.
///
/// # Errors
///
/// Throws a JavaScript exception if `result` is malformed or does not match
/// `FiAttributionResult`, or if the reconciliation report cannot be converted to
/// a JavaScript value.
#[wasm_bindgen(js_name = fiAttributionResultReconciliationCheck)]
pub fn fi_attribution_result_reconciliation_check(
    result: JsValue,
    tolerance: JsValue,
) -> Result<JsValue, JsValue> {
    let tolerance = js_f64(&tolerance, "tolerance")?;
    let result: finstack_quant_portfolio::fi_attribution::FiAttributionResult =
        from_js_json(&result, "result")?;
    to_js_value(&result.reconciliation_check(tolerance))
}
