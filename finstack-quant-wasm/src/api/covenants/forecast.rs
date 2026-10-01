//! Covenant forecasting over dated metric rows.

use super::engine::JsCovenantEngine;
use crate::utils::wire::{js_opt_wire, js_wire};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_covenants::{
    CovenantForecastConfig, CovenantScope, CovenantSpec, DatedMetricSeries, DatedMetrics,
};
use wasm_bindgen::prelude::*;

/// Read `DatedMetrics` rows as a Rust dated metric series.
fn metric_series(metrics: &JsValue) -> Result<DatedMetricSeries, JsValue> {
    DatedMetricSeries::new(js_wire::<Vec<DatedMetrics>>(metrics, "metrics")?).map_err(to_js_err)
}

/// Forecast configuration, or the Rust default (deterministic, maintenance scope).
fn forecast_config(config: Option<&JsValue>) -> Result<CovenantForecastConfig, JsValue> {
    Ok(js_opt_wire::<CovenantForecastConfig>(config, "config")?.unwrap_or_default())
}

/// Forecast one numeric covenant over dated metric projections.
///
/// Deterministic by default: breach probability is `0` for a pass and `1` for
/// a breach. With `config.stochastic` a lognormal overlay on the metric gives
/// probabilities, analytic when `num_paths` is `0` and Monte Carlo otherwise.
///
/// @param spec - `CovenantSpec` wire object of a numeric covenant.
/// @param metrics - `DatedMetrics` rows (`{ date, metrics }`): projected metric values per ISO-8601 test date, in any order with unique dates. (Python takes a date-indexed DataFrame.)
/// @param config - Optional `CovenantForecastConfig` wire object; omitted means the deterministic Rust default.
/// @returns `CovenantForecast` wire object: per-date projected values, thresholds, headroom and breach probabilities, with the first breach date and minimum headroom.
/// @throws If `metrics` is empty or repeats a date, the covenant is not numeric, the configuration is invalid or an observation is non-finite (kind `validation`), or the covenant's metric is missing on a date (kind `not_found`).
#[wasm_bindgen(js_name = forecastCovenant)]
pub fn forecast_covenant(
    spec: JsValue,
    metrics: JsValue,
    config: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let spec = js_wire::<CovenantSpec>(&spec, "spec")?;
    let series = metric_series(&metrics)?;
    let config = forecast_config(config.as_ref())?;
    to_js_value(
        &finstack_quant_covenants::forecast_covenant(&spec, &series, config).map_err(to_js_err)?,
    )
}

/// Forecast every breach of an engine's covenants over dated metric projections.
///
/// Effective windows, waivers and threshold schedules are honored;
/// non-numeric covenants are skipped. Deterministic breaches are always
/// listed; in stochastic mode dates whose breach probability reaches
/// `config.breach_probability_threshold` are listed too.
///
/// @param engine - `CovenantEngine` handle.
/// @param metrics - `DatedMetrics` rows (`{ date, metrics }`): projected metric values per ISO-8601 test date, in any order with unique dates. (Python takes a date-indexed DataFrame.)
/// @param config - Optional `CovenantForecastConfig` wire object; omitted means deterministic maintenance-scope forecasting.
/// @returns `FutureBreach` wire objects sorted by breach date, then covenant id.
/// @throws If `metrics` is empty or repeats a date, the engine or configuration is invalid or an observation is non-finite (kind `validation`), or a required metric is missing on a date (kind `not_found`).
#[wasm_bindgen(js_name = forecastBreaches)]
pub fn forecast_breaches(
    engine: &JsCovenantEngine,
    metrics: JsValue,
    config: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let series = metric_series(&metrics)?;
    let config = forecast_config(config.as_ref())?;
    to_js_value(
        &finstack_quant_covenants::forecast_breaches(&engine.inner, &series, config)
            .map_err(to_js_err)?,
    )
}

/// Copy of a forecast configuration with a different covenant scope.
///
/// @param config - `CovenantForecastConfig` wire object.
/// @param scope - `"maintenance"` (scheduled tests) or `"incurrence"` (hypothetical capacity for a contemplated action).
/// @returns The updated `CovenantForecastConfig`.
/// @throws If `config` is not a `CovenantForecastConfig` or `scope` is not one of the listed strings (kind `validation`).
#[wasm_bindgen(js_name = covenantForecastConfigWithScope)]
pub fn covenant_forecast_config_with_scope(
    config: JsValue,
    scope: JsValue,
) -> Result<JsValue, JsValue> {
    let config = js_wire::<CovenantForecastConfig>(&config, "config")?;
    to_js_value(&config.with_scope(js_wire::<CovenantScope>(&scope, "scope")?))
}
