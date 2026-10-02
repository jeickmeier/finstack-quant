//! IFRS 9 staging and expected-credit-loss bindings.

use crate::utils::input::{from_js_json, js_f64, js_opt_f64, js_string};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_statements_analytics::analysis as ecl;
use wasm_bindgen::prelude::*;

fn parse_stage(value: &JsValue) -> Result<ecl::Stage, JsValue> {
    finstack_quant_core::wire::serde_parse(&js_string(value, "stage")?).map_err(to_js_err)
}

/// Classify an exposure into an IFRS 9 stage from directly supplied lifetime PDs.
///
/// Twin of Python `classify_stage` (Rust `classify_exposure`). Runs the full
/// staging waterfall: days-past-due backstops, the SICR test on the two PDs,
/// the rating-downgrade notch test, qualitative flags and curing. The Python
/// `Exposure` carries the two PDs itself; here they are arguments.
/// @param exposure - `Exposure` whose days past due, qualitative flags, rating labels and cure state drive the waterfall (object or JSON).
/// @param current_pd - Current lifetime probability of default as a decimal in `[0, 1]`.
/// @param origination_pd - Lifetime probability of default at initial recognition as a decimal in `[0, 1]`.
/// @param config - Optional `StagingConfig` thresholds, backstops and curing rules; omitted uses the Rust defaults (object or JSON).
/// @returns `StageResult`: the assigned `stage`, the ordered `triggers` audit trail and whether the exposure `cured`.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed or a PD, maturity
/// or threshold is non-finite or out of range, and kind `invalid_type` if a
/// PD is not a number.
#[wasm_bindgen(js_name = classifyStage)]
pub fn classify_stage(
    exposure: JsValue,
    current_pd: JsValue,
    origination_pd: JsValue,
    config: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let exposure: ecl::Exposure = from_js_json(&exposure, "exposure")?;
    let config: ecl::StagingConfig = match config.filter(|c| !c.is_null() && !c.is_undefined()) {
        Some(config) => from_js_json(&config, "config")?,
        None => ecl::StagingConfig::default(),
    };
    let result = ecl::classify_exposure(
        &exposure,
        js_f64(&current_pd, "currentPd")?,
        js_f64(&origination_pd, "originationPd")?,
        &config,
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Expected credit loss of one exposure under a single cumulative-PD schedule.
///
/// Twin of Python `compute_ecl` (Rust `compute_ecl_for_exposure` with one
/// scenario of weight `1.0`). The priced exposure at default is
/// `ead + undrawn * ccf`; the schedule is anchored at `(0, 0)` when that knot
/// is absent. Unlike Python, `stage` is required (classify with
/// `classifyStage` first).
/// @param exposure - `Exposure` supplying EAD, undrawn amount and CCF, LGD, EIR, remaining maturity and any EAD schedule (object or JSON).
/// @param pd_schedule - Cumulative PD knots as `[timeYears, cumulativePd]` pairs, ascending in time and non-decreasing in PD (decimals).
/// @param stage - `"stage1"` (12-month horizon), `"stage2"` (lifetime) or `"stage3"` (credit-impaired).
/// @param bucket_width_years - Integration bucket width in years (`0.25` = quarterly); omitted uses the Rust policy default.
/// @param stage3_time_to_recovery_years - Stage 3 discounting horizon to expected recovery in years; omitted uses the Rust policy default.
/// @returns `WeightedEclResult`: the ECL in the exposure's currency units, the stage and the per-scenario bucket audit trail.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed, `stage` is not a
/// stage name, the schedule violates the cumulative-PD invariants, or the
/// exposure fails validation.
#[wasm_bindgen(js_name = computeEcl)]
pub fn compute_ecl(
    exposure: JsValue,
    pd_schedule: JsValue,
    stage: JsValue,
    bucket_width_years: Option<JsValue>,
    stage3_time_to_recovery_years: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let exposure: ecl::Exposure = from_js_json(&exposure, "exposure")?;
    let pd_schedule: Vec<(f64, f64)> = from_js_json(&pd_schedule, "pdSchedule")?;
    let result = ecl::compute_ecl_for_exposure(
        &exposure,
        parse_stage(&stage)?,
        &[(1.0, pd_schedule)],
        js_opt_f64(bucket_width_years.as_ref(), "bucketWidthYears")?,
        js_opt_f64(
            stage3_time_to_recovery_years.as_ref(),
            "stage3TimeToRecoveryYears",
        )?,
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Probability-weighted expected credit loss across macro scenarios.
///
/// Twin of Python `compute_ecl_weighted` (Rust `compute_ecl_for_exposure`).
/// Unlike Python, `stage` is required (classify with `classifyStage` first).
/// @param exposure - `Exposure` supplying EAD, undrawn amount and CCF, LGD, EIR, remaining maturity and any EAD schedule (object or JSON).
/// @param scenarios - `[weight, schedule]` pairs whose weights sum to `1.0`; each schedule holds `[timeYears, cumulativePd]` knots ascending in time and non-decreasing in PD.
/// @param stage - `"stage1"` (12-month horizon), `"stage2"` (lifetime) or `"stage3"` (credit-impaired).
/// @param bucket_width_years - Integration bucket width in years (`0.25` = quarterly); omitted uses the Rust policy default.
/// @param stage3_time_to_recovery_years - Stage 3 discounting horizon to expected recovery in years; omitted uses the Rust policy default.
/// @returns `WeightedEclResult`: the probability-weighted ECL in the exposure's currency units and one `scenario_breakdown` entry per scenario.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed, `scenarios` is
/// empty, the weights do not sum to `1.0`, `stage` is not a stage name, a
/// schedule violates the cumulative-PD invariants, or the exposure fails
/// validation.
#[wasm_bindgen(js_name = computeEclWeighted)]
pub fn compute_ecl_weighted(
    exposure: JsValue,
    scenarios: JsValue,
    stage: JsValue,
    bucket_width_years: Option<JsValue>,
    stage3_time_to_recovery_years: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let exposure: ecl::Exposure = from_js_json(&exposure, "exposure")?;
    let scenarios: Vec<(f64, Vec<(f64, f64)>)> = from_js_json(&scenarios, "scenarios")?;
    let result = ecl::compute_ecl_for_exposure(
        &exposure,
        parse_stage(&stage)?,
        &scenarios,
        js_opt_f64(bucket_width_years.as_ref(), "bucketWidthYears")?,
        js_opt_f64(
            stage3_time_to_recovery_years.as_ref(),
            "stage3TimeToRecoveryYears",
        )?,
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}
