//! Covenant evaluation and breach forecasting over evaluated statement results.

use crate::api::covenants::engine::JsCovenantEngine;
use crate::utils::input::from_js_json;
use crate::utils::wire::js_opt_wire;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_covenants::CovenantForecastConfig;
use finstack_quant_statements::evaluator::StatementResult;
use wasm_bindgen::prelude::*;

/// Evaluate covenant compliance at the end of every statement result period.
///
/// Each period of `results.periods` is tested on its last calendar day with
/// the covenants applicable on that date: a covenant window covering the date replaces the engine's
/// top-level specifications, and waivers and threshold schedules apply. A
/// node is read only when an applicable covenant needs it, so a node that is
/// missing or non-finite in a period where no applicable covenant reads it
/// (for example a trailing ratio before four quarters of history, outside the
/// window that tests it) does not throw. The engine's breach history is not
/// updated.
///
/// @param results - Evaluated `StatementResult` (object or JSON). Node ids are the covenant metric ids (`metric_id`, or each covenant type's conventional name); ratios in turns (`3.5` means 3.5x), amounts in the model currency.
/// @param covenants - `CovenantEngine` handle whose specifications, windows, waivers and threshold schedules are evaluated.
/// @returns `DatedCovenantReports` rows (`{ as_of, reports }`), one per result period in timeline order, with the reports of the covenants applicable on that date keyed by covenant instance key in engine order. (Python returns the same reports as a long DataFrame.)
/// @throws If `results` is malformed, the engine is invalid or a metric an applicable covenant reads is non-finite (kind `validation`), or such a metric has no value in the period (kind `not_found`).
#[wasm_bindgen(js_name = evaluateCovenants, unchecked_return_type = "generated.covenants.DatedCovenantReports[]")]
pub fn evaluate_covenants(
    #[wasm_bindgen(unchecked_param_type = "StatementResult | string")] results: JsValue,
    covenants: &JsCovenantEngine,
) -> Result<JsValue, JsValue> {
    let results: StatementResult = from_js_json(&results, "results")?;
    to_js_value(
        &finstack_quant_statements_analytics::analysis::evaluate_covenants(
            &results,
            &covenants.inner,
        )
        .map_err(to_js_err)?,
    )
}

/// Forecast covenant breaches over every statement result period.
///
/// Published as `statements_analytics.forecastBreaches` (raw export
/// `statementsAnalyticsForecastBreaches`; `covenants.forecastBreaches` over
/// dated metric rows owns the bare raw name).
///
/// Every period of `results.periods` is a test date (its last calendar day). Effective windows, waivers and threshold schedules are
/// honored and non-numeric covenants are skipped. Deterministic breaches are
/// always listed; in stochastic mode dates whose breach probability reaches
/// `config.breach_probability_threshold` are listed too. Without
/// `config.reference_date` the horizon starts the day before the first result
/// period begins.
///
/// @param results - Evaluated `StatementResult` (object or JSON) holding every metric the active numeric covenants read on each test date (ratios in turns).
/// @param covenants - `CovenantEngine` handle whose effective specifications are forecast.
/// @param config - Optional `CovenantForecastConfig` object or JSON text (JSON text keeps a uint64 `random_seed` exact); omitted means deterministic maintenance-scope forecasting.
/// @returns `FutureBreach` wire objects sorted by breach date, then covenant id.
/// @throws If `results` or `config` is malformed, the engine or configuration is invalid or a metric an active covenant reads is non-finite (kind `validation`), or such a metric has no value in a period (kind `not_found`).
#[wasm_bindgen(js_name = statementsAnalyticsForecastBreaches, unchecked_return_type = "generated.covenants.FutureBreach[]")]
pub fn forecast_breaches(
    #[wasm_bindgen(unchecked_param_type = "StatementResult | string")] results: JsValue,
    covenants: &JsCovenantEngine,
    #[wasm_bindgen(
        unchecked_optional_param_type = "generated.covenants.CovenantForecastConfig | string | null"
    )]
    config: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let results: StatementResult = from_js_json(&results, "results")?;
    let config =
        js_opt_wire::<CovenantForecastConfig>(config.as_ref(), "config")?.unwrap_or_default();
    to_js_value(
        &finstack_quant_statements_analytics::analysis::forecast_breaches(
            &results,
            &covenants.inner,
            config,
        )
        .map_err(to_js_err)?,
    )
}
