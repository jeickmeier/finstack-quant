//! Covenant evaluation and breach forecasting over evaluated statement results.

use super::{parse_model, parse_results};
use crate::api::covenants::engine::JsCovenantEngine;
use crate::utils::wire::js_opt_wire;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_covenants::CovenantForecastConfig;
use wasm_bindgen::prelude::*;

/// Evaluate covenant compliance at the end of every statement result period.
///
/// Each period that carries a value in `results` is tested on its last
/// calendar day (from the model calendar) with the covenants applicable on
/// that date: a covenant window covering the date replaces the engine's
/// top-level specifications, and waivers and threshold schedules apply. A
/// node is read only when an applicable covenant needs it, so a node that is
/// missing or non-finite in a period where no applicable covenant reads it
/// (for example a trailing ratio before four quarters of history, outside the
/// window that tests it) does not throw. The engine's breach history is not
/// updated.
///
/// @param results - Evaluated `StatementResult` (object or JSON). Node ids are the covenant metric ids (`metric_id`, or each covenant type's conventional name); ratios in turns (`3.5` means 3.5x), amounts in the model currency.
/// @param covenants - `CovenantEngine` handle whose specifications, windows, waivers and threshold schedules are evaluated.
/// @param model - `FinancialModelSpec` (object or JSON) whose period calendar gives each result period's test date.
/// @returns `DatedCovenantReports` rows (`{ as_of, reports }`), one per result period in ascending period order, with the reports of the covenants applicable on that date keyed by covenant instance key in engine order. (Python returns the same reports as a long DataFrame.)
/// @throws If `results` or `model` is malformed, a result period is missing from the model calendar, the engine is invalid or a metric an applicable covenant reads is non-finite (kind `validation`), or such a metric has no value in the period (kind `not_found`).
#[wasm_bindgen(js_name = evaluateCovenants)]
pub fn evaluate_covenants(
    results: JsValue,
    covenants: &JsCovenantEngine,
    model: JsValue,
) -> Result<JsValue, JsValue> {
    let results = parse_results(&results)?;
    let model = parse_model(&model)?;
    to_js_value(
        &finstack_quant_statements_analytics::analysis::evaluate_covenants(
            &results,
            &covenants.inner,
            &model,
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
/// Every period that carries a value in `results` is a test date (its last
/// calendar day). Effective windows, waivers and threshold schedules are
/// honored and non-numeric covenants are skipped. Deterministic breaches are
/// always listed; in stochastic mode dates whose breach probability reaches
/// `config.breach_probability_threshold` are listed too. Without
/// `config.reference_date` the horizon starts the day before the first result
/// period begins.
///
/// @param results - Evaluated `StatementResult` (object or JSON) holding every metric the active numeric covenants read on each test date (ratios in turns).
/// @param covenants - `CovenantEngine` handle whose effective specifications are forecast.
/// @param model - `FinancialModelSpec` (object or JSON) whose period calendar gives the test dates and the default horizon start.
/// @param config - Optional `CovenantForecastConfig` wire object; omitted means deterministic maintenance-scope forecasting.
/// @returns `FutureBreach` wire objects sorted by breach date, then covenant id.
/// @throws If `results`, `model` or `config` is malformed, a result period is missing from the model calendar, the engine or configuration is invalid or a metric an active covenant reads is non-finite (kind `validation`), or such a metric has no value in a period (kind `not_found`).
#[wasm_bindgen(js_name = statementsAnalyticsForecastBreaches)]
pub fn forecast_breaches(
    results: JsValue,
    covenants: &JsCovenantEngine,
    model: JsValue,
    config: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let results = parse_results(&results)?;
    let model = parse_model(&model)?;
    let config =
        js_opt_wire::<CovenantForecastConfig>(config.as_ref(), "config")?.unwrap_or_default();
    to_js_value(
        &finstack_quant_statements_analytics::analysis::forecast_breaches(
            &results,
            &covenants.inner,
            &model,
            config,
        )
        .map_err(to_js_err)?,
    )
}
