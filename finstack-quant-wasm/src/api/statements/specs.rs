//! Free-function twins of the Python spec-object constructors and methods.
//!
//! WASM specs are plain objects, so each Rust constructor or method on
//! `ForecastSpec`, `Adjustment`, `NormalizationConfig` and `CheckSuiteSpec`
//! is a function that returns (or takes) the plain object.

use crate::utils::input::{
    from_js_json, js_f64, js_f64_seq, js_opt_string, js_string, js_u64, js_uint,
};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::adjustments::engine::NormalizationEngine;
use finstack_quant_statements::adjustments::types::{Adjustment, NormalizationConfig};
use finstack_quant_statements::checks::BuiltinCheckSpec;
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::types::ForecastSpec;
use indexmap::IndexMap;
use wasm_bindgen::prelude::*;

/// Forecast that carries the last value forward unchanged.
///
/// Free-function twin of Python `ForecastSpec.forward_fill` (Rust
/// `ForecastSpec::forward_fill`).
/// @returns Plain `ForecastSpec` object (`{method: "forward_fill"}`).
///
/// # Errors
///
/// Throws only if the spec cannot be converted to a JavaScript object.
#[wasm_bindgen(js_name = forecastSpecForwardFill)]
pub fn forecast_spec_forward_fill() -> Result<JsValue, JsValue> {
    to_js_value(&ForecastSpec::forward_fill())
}

/// Forecast that compounds the last value at one rate per period.
///
/// Free-function twin of Python `ForecastSpec.growth` (Rust
/// `ForecastSpec::growth`): `v[t] = v[t-1] * (1 + rate)`.
/// @param rate - Growth per model period as a decimal (`0.05` = 5%), not annualised.
/// @returns Plain `ForecastSpec` object with method `growth_pct`.
///
/// # Errors
///
/// Throws with kind `invalid_type` if `rate` is not a number.
#[wasm_bindgen(js_name = forecastSpecGrowth)]
pub fn forecast_spec_growth(rate: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&ForecastSpec::growth(js_f64(&rate, "rate")?))
}

/// Forecast that applies a period-specific growth curve.
///
/// Free-function twin of Python `ForecastSpec.curve` (Rust
/// `ForecastSpec::curve`).
/// @param curve - One growth rate per forecast period, as decimals per period (`0.05` = 5%), in timeline order.
/// @returns Plain `ForecastSpec` object with method `curve_pct`.
///
/// # Errors
///
/// Throws with kind `invalid_type` if `curve` is not an array of numbers.
#[wasm_bindgen(js_name = forecastSpecCurve)]
pub fn forecast_spec_curve(curve: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&ForecastSpec::curve(js_f64_seq(&curve, "curve")?))
}

/// Forecast drawn from a seeded normal distribution.
///
/// Free-function twin of Python `ForecastSpec.normal` (Rust
/// `ForecastSpec::normal`).
/// @param mean - Mean of each period's draw, in the node's own units.
/// @param std_dev - Standard deviation of each draw, in the node's own units; must be non-negative when the model is evaluated.
/// @param seed - Random seed (non-negative integer or BigInt); the same seed reproduces the same draws.
/// @returns Plain `ForecastSpec` object with method `normal`.
///
/// # Errors
///
/// Throws with kind `invalid_type` if a number argument is not a number or
/// `seed` is not a non-negative integer.
#[wasm_bindgen(js_name = forecastSpecNormal)]
pub fn forecast_spec_normal(
    mean: JsValue,
    std_dev: JsValue,
    seed: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&ForecastSpec::normal(
        js_f64(&mean, "mean")?,
        js_f64(&std_dev, "stdDev")?,
        js_u64(&seed, "seed")?,
    ))
}

/// Forecast drawn from a seeded log-normal distribution.
///
/// Free-function twin of Python `ForecastSpec.log_normal` (Rust
/// `ForecastSpec::log_normal`).
/// @param mean - Mean of the underlying normal (log space).
/// @param std_dev - Standard deviation of the underlying normal (log space); must be non-negative when the model is evaluated.
/// @param seed - Random seed (non-negative integer or BigInt); the same seed reproduces the same draws.
/// @returns Plain `ForecastSpec` object with method `log_normal`.
///
/// # Errors
///
/// Throws with kind `invalid_type` if a number argument is not a number or
/// `seed` is not a non-negative integer.
#[wasm_bindgen(js_name = forecastSpecLogNormal)]
pub fn forecast_spec_log_normal(
    mean: JsValue,
    std_dev: JsValue,
    seed: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&ForecastSpec::log_normal(
        js_f64(&mean, "mean")?,
        js_f64(&std_dev, "stdDev")?,
        js_u64(&seed, "seed")?,
    ))
}

/// Forecast with explicit per-period override values.
///
/// Free-function twin of Python `ForecastSpec.override` (Rust
/// `ForecastSpec::overrides`). Periods without an override carry the previous
/// value forward.
/// @param overrides - Object mapping period id (e.g. `"2025Q3"`) to the value for that period, in the node's own units.
/// @returns Plain `ForecastSpec` object with method `override`.
///
/// # Errors
///
/// Throws with kind `validation` if `overrides` is not an object of numbers
/// keyed by valid period identifiers.
#[wasm_bindgen(js_name = forecastSpecOverride)]
pub fn forecast_spec_override(overrides: JsValue) -> Result<JsValue, JsValue> {
    let overrides: IndexMap<PeriodId, f64> = from_js_json(&overrides, "overrides")?;
    to_js_value(&ForecastSpec::overrides(overrides))
}

/// Forecast that repeats a seasonal pattern taken from history.
///
/// Free-function twin of Python `ForecastSpec.seasonal` (Rust
/// `ForecastSpec::seasonal`).
/// @param historical - Historical observations in timeline order, in the node's own units.
/// @param season_length - Number of periods in one season (e.g. `4` for quarterly data with an annual cycle).
/// @param mode - `"additive"` or `"multiplicative"`.
/// @returns Plain `ForecastSpec` object with method `seasonal`.
///
/// # Errors
///
/// Throws with kind `invalid_type` if an argument has the wrong JavaScript
/// type, and kind `validation` if `mode` is not a seasonal mode.
#[wasm_bindgen(js_name = forecastSpecSeasonal)]
pub fn forecast_spec_seasonal(
    historical: JsValue,
    season_length: JsValue,
    mode: JsValue,
) -> Result<JsValue, JsValue> {
    let mode =
        finstack_quant_core::wire::serde_parse(&js_string(&mode, "mode")?).map_err(to_js_err)?;
    to_js_value(&ForecastSpec::seasonal(
        js_f64_seq(&historical, "historical")?,
        js_uint(&season_length, "seasonLength")?,
        mode,
    ))
}

/// Forecast fitted to a historical time series.
///
/// Free-function twin of Python `ForecastSpec.time_series` (Rust
/// `ForecastSpec::time_series`).
/// @param historical - Historical observations in timeline order, in the node's own units.
/// @returns Plain `ForecastSpec` object with method `time_series`.
///
/// # Errors
///
/// Throws with kind `invalid_type` if `historical` is not an array of numbers.
#[wasm_bindgen(js_name = forecastSpecTimeSeries)]
pub fn forecast_spec_time_series(historical: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&ForecastSpec::time_series(js_f64_seq(
        &historical,
        "historical",
    )?))
}

/// Forecast that fades the last value toward a target level.
///
/// Free-function twin of Python `ForecastSpec.fade_to_target` (Rust
/// `ForecastSpec::fade_to_target`).
/// @param target - Level the node converges to, in the node's own units.
/// @returns Plain `ForecastSpec` object with method `fade_to_target`.
///
/// # Errors
///
/// Throws with kind `invalid_type` if `target` is not a number.
#[wasm_bindgen(js_name = forecastSpecFadeToTarget)]
pub fn forecast_spec_fade_to_target(target: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&ForecastSpec::fade_to_target(js_f64(&target, "target")?))
}

/// Forecast following a seeded mean-reverting process.
///
/// Free-function twin of Python `ForecastSpec.mean_reverting` (Rust
/// `ForecastSpec::mean_reverting`).
/// @param long_run_mean - Level the process reverts to, in the node's own units.
/// @param reversion_speed - Fraction of the gap to the mean closed each period, as a decimal in `[0, 1]`.
/// @param std_dev - Standard deviation of each period's shock, in the node's own units.
/// @param seed - Random seed (non-negative integer or BigInt); the same seed reproduces the same path.
/// @returns Plain `ForecastSpec` object with method `mean_reverting`.
///
/// # Errors
///
/// Throws with kind `invalid_type` if a number argument is not a number or
/// `seed` is not a non-negative integer.
#[wasm_bindgen(js_name = forecastSpecMeanReverting)]
pub fn forecast_spec_mean_reverting(
    long_run_mean: JsValue,
    reversion_speed: JsValue,
    std_dev: JsValue,
    seed: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&ForecastSpec::mean_reverting(
        js_f64(&long_run_mean, "longRunMean")?,
        js_f64(&reversion_speed, "reversionSpeed")?,
        js_f64(&std_dev, "stdDev")?,
        js_u64(&seed, "seed")?,
    ))
}

/// Forecast that resamples historical observations with a seed.
///
/// Free-function twin of Python `ForecastSpec.bootstrap` (Rust
/// `ForecastSpec::bootstrap`).
/// @param historical - Historical observations to resample, in the node's own units.
/// @param seed - Random seed (non-negative integer or BigInt); the same seed reproduces the same draws.
/// @returns Plain `ForecastSpec` object with method `bootstrap`.
///
/// # Errors
///
/// Throws with kind `invalid_type` if `historical` is not an array of numbers
/// or `seed` is not a non-negative integer.
#[wasm_bindgen(js_name = forecastSpecBootstrap)]
pub fn forecast_spec_bootstrap(historical: JsValue, seed: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&ForecastSpec::bootstrap(
        js_f64_seq(&historical, "historical")?,
        js_u64(&seed, "seed")?,
    ))
}

/// Adjustment with fixed amounts per period.
///
/// Free-function twin of Python `Adjustment.fixed` (Rust `Adjustment::fixed`).
/// @param id - Adjustment identifier, unique within a normalization configuration.
/// @param name - Human-readable name shown in reports.
/// @param amounts - Object mapping period id (e.g. `"2025Q1"`) to the signed amount for that period, in the target node's units (positive adds back, negative deducts).
/// @returns Plain `Adjustment` object.
///
/// # Errors
///
/// Throws with kind `validation` if `amounts` is not an object of numbers
/// keyed by valid period identifiers.
#[wasm_bindgen(js_name = adjustmentFixed)]
pub fn adjustment_fixed(id: JsValue, name: JsValue, amounts: JsValue) -> Result<JsValue, JsValue> {
    let amounts: IndexMap<PeriodId, f64> = from_js_json(&amounts, "amounts")?;
    to_js_value(&Adjustment::fixed(
        js_string(&id, "id")?,
        js_string(&name, "name")?,
        amounts,
    ))
}

/// Adjustment sized as a fraction of another node's value each period.
///
/// Free-function twin of Python `Adjustment.percentage` (Rust
/// `Adjustment::percentage`).
/// @param id - Adjustment identifier, unique within a normalization configuration.
/// @param name - Human-readable name shown in reports.
/// @param node_id - Reference node whose per-period value is scaled.
/// @param percentage - Fraction as a signed decimal (`0.05` = 5%; negative for a deduction).
/// @returns Plain `Adjustment` object.
///
/// # Errors
///
/// Throws with kind `invalid_type` if an argument has the wrong JavaScript type.
#[wasm_bindgen(js_name = adjustmentPercentage)]
pub fn adjustment_percentage(
    id: JsValue,
    name: JsValue,
    node_id: JsValue,
    percentage: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&Adjustment::percentage(
        js_string(&id, "id")?,
        js_string(&name, "name")?,
        js_string(&node_id, "nodeId")?,
        js_f64(&percentage, "percentage")?,
    ))
}

/// Copy of an adjustment with a cap measured on the reported base.
///
/// Free-function twin of Python `Adjustment.with_cap` (Rust
/// `Adjustment::with_cap`).
/// @param adjustment - `Adjustment` (object or JSON).
/// @param base_node - Node the cap is a fraction of; `null` makes `value` an absolute cap in the target node's units.
/// @param value - Cap as a decimal fraction of `baseNode` (`0.20` = 20%), or an absolute amount when `baseNode` is `null`.
/// @returns New plain `Adjustment` object with the cap set.
///
/// # Errors
///
/// Throws with kind `validation` if `adjustment` is malformed, and kind
/// `invalid_type` if another argument has the wrong JavaScript type.
#[wasm_bindgen(js_name = adjustmentWithCap)]
pub fn adjustment_with_cap(
    adjustment: JsValue,
    base_node: Option<JsValue>,
    value: JsValue,
) -> Result<JsValue, JsValue> {
    let adjustment: Adjustment = from_js_json(&adjustment, "adjustment")?;
    to_js_value(&adjustment.with_cap(
        js_opt_string(base_node.as_ref(), "baseNode")?,
        js_f64(&value, "value")?,
    ))
}

/// Copy of an adjustment with a cap and an explicit cap-base mode.
///
/// Free-function twin of Python `Adjustment.with_cap_mode` (Rust
/// `Adjustment::with_cap_mode`).
/// @param adjustment - `Adjustment` (object or JSON).
/// @param base_node - Node the cap is a fraction of; `null` makes `value` an absolute cap in the target node's units.
/// @param value - Cap as a decimal fraction of `baseNode` (`0.20` = 20%), or an absolute amount when `baseNode` is `null`.
/// @param base_mode - `"reported"` (cap measured on the reported base) or `"progressive"` (base grows with the adjustments already applied).
/// @returns New plain `Adjustment` object with the cap set.
///
/// # Errors
///
/// Throws with kind `validation` if `adjustment` is malformed or `baseMode`
/// is not a cap-base mode, and kind `invalid_type` if another argument has the
/// wrong JavaScript type.
#[wasm_bindgen(js_name = adjustmentWithCapMode)]
pub fn adjustment_with_cap_mode(
    adjustment: JsValue,
    base_node: Option<JsValue>,
    value: JsValue,
    base_mode: JsValue,
) -> Result<JsValue, JsValue> {
    let adjustment: Adjustment = from_js_json(&adjustment, "adjustment")?;
    let base_mode = finstack_quant_core::wire::serde_parse(&js_string(&base_mode, "baseMode")?)
        .map_err(to_js_err)?;
    to_js_value(&adjustment.with_cap_mode(
        js_opt_string(base_node.as_ref(), "baseNode")?,
        js_f64(&value, "value")?,
        base_mode,
    ))
}

/// Copy of an adjustment with a grouping category.
///
/// Free-function twin of Python `Adjustment.with_category` (Rust
/// `Adjustment::with_category`).
/// @param adjustment - `Adjustment` (object or JSON).
/// @param category - Grouping label used by reports, e.g. `"one_time"` or `"run_rate"`.
/// @returns New plain `Adjustment` object with the category set.
///
/// # Errors
///
/// Throws with kind `validation` if `adjustment` is malformed, and kind
/// `invalid_type` if `category` is not a string.
#[wasm_bindgen(js_name = adjustmentWithCategory)]
pub fn adjustment_with_category(
    adjustment: JsValue,
    category: JsValue,
) -> Result<JsValue, JsValue> {
    let adjustment: Adjustment = from_js_json(&adjustment, "adjustment")?;
    to_js_value(&adjustment.with_category(js_string(&category, "category")?))
}

/// Copy of a normalization configuration with one more adjustment.
///
/// Free-function twin of Python `NormalizationConfig.add_adjustment` (Rust
/// `NormalizationConfig::add_adjustment`), which rejects a duplicate
/// adjustment identifier.
/// @param config - `NormalizationConfig` (object or JSON).
/// @param adjustment - `Adjustment` to append (object or JSON).
/// @returns New plain `NormalizationConfig` object.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed or the adjustment
/// identifier already exists in the configuration.
#[wasm_bindgen(js_name = normalizationConfigAddAdjustment)]
pub fn normalization_config_add_adjustment(
    config: JsValue,
    adjustment: JsValue,
) -> Result<JsValue, JsValue> {
    let config: NormalizationConfig = from_js_json(&config, "config")?;
    let adjustment: Adjustment = from_js_json(&adjustment, "adjustment")?;
    to_js_value(&config.add_adjustment(adjustment).map_err(to_js_err)?)
}

/// Validate a `NormalizationConfig` and return its canonical JSON.
///
/// JSON wire twin of Python `NormalizationConfig.from_json` followed by
/// `validate` (Rust `NormalizationConfig::validate`).
/// @param json - `NormalizationConfig` (object or JSON).
/// @returns Canonical normalization-configuration JSON.
///
/// # Errors
///
/// Throws with kind `validation` if `json` is malformed or holds duplicate
/// adjustment identifiers.
#[wasm_bindgen(js_name = validateNormalizationConfigJson)]
pub fn validate_normalization_config_json(json: JsValue) -> Result<String, JsValue> {
    let config: NormalizationConfig = from_js_json(&json, "json")?;
    config.validate().map_err(to_js_err)?;
    serde_json::to_string(&config).map_err(to_js_err)
}

fn normalized(
    results: &JsValue,
    config: &JsValue,
) -> Result<Vec<finstack_quant_statements::adjustments::types::NormalizationResult>, JsValue> {
    let results: StatementResult = from_js_json(results, "results")?;
    let config: NormalizationConfig = from_js_json(config, "config")?;
    NormalizationEngine::normalize(&results, &config).map_err(to_js_err)
}

/// Normalize a target metric by applying an adjustment catalog period by period.
///
/// Twin of Python `normalize` (Rust `NormalizationEngine::normalize`).
/// @param results - Evaluated `StatementResult` holding the target node and any reference nodes (object or JSON).
/// @param config - `NormalizationConfig`: target node plus the adjustments to apply (object or JSON).
/// @returns One `NormalizationResult` per period of the target node: base value, each applied adjustment (raw and capped) and the final value.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed, the target node or
/// a referenced node is absent from the results, the configuration is
/// invalid, or the target and a reference node have incompatible units.
#[wasm_bindgen(js_name = normalize)]
pub fn normalize(results: JsValue, config: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&normalized(&results, &config)?)
}

/// Normalize a target metric and return the results as JSON text.
///
/// JSON wire twin of [`normalize`] (Python `normalize_json`).
/// @param results - Evaluated `StatementResult` holding the target node and any reference nodes (object or JSON).
/// @param config - `NormalizationConfig`: target node plus the adjustments to apply (object or JSON).
/// @returns JSON array of `NormalizationResult` objects, one per period.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed, the target node or
/// a referenced node is absent from the results, the configuration is
/// invalid, or the target and a reference node have incompatible units.
#[wasm_bindgen(js_name = normalizeJson)]
pub fn normalize_json(results: JsValue, config: JsValue) -> Result<String, JsValue> {
    serde_json::to_string(&normalized(&results, &config)?).map_err(to_js_err)
}

/// Serde `type` tags of every built-in check a suite spec accepts.
///
/// Free-function twin of Python `CheckSuiteSpec.builtin_check_names` (Rust
/// `BuiltinCheckSpec::names`).
/// @returns Tags accepted by `builtin_checks[].type`, in declaration order.
///
/// # Errors
///
/// Throws only if the list cannot be converted to a JavaScript array.
#[wasm_bindgen(js_name = checkSuiteSpecBuiltinCheckNames)]
pub fn check_suite_spec_builtin_check_names() -> Result<JsValue, JsValue> {
    to_js_value(&BuiltinCheckSpec::names())
}
