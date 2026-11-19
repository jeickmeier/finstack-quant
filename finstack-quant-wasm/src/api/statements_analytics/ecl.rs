//! IFRS 9 staging and expected-credit-loss bindings.

use crate::utils::input::{from_js_json, js_f64, js_opt_f64};
use crate::utils::wire::js_wire;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_statements_analytics::analysis as ecl;
use wasm_bindgen::prelude::*;

/// Classify an exposure into an IFRS 9 stage from directly supplied lifetime PDs.
///
/// Twin of Python and Rust `classify_stage`. Runs the full
/// staging waterfall: days-past-due backstops, the SICR test on the two PDs,
/// the rating-downgrade notch test, qualitative flags and curing. The Python
/// `Exposure` carries the two PDs itself; here they are arguments.
/// @param exposure - `Exposure` whose days past due, qualitative flags, rating labels and cure state drive the waterfall (object or JSON).
/// @param currentPd - Current lifetime probability of default as a decimal in `[0, 1]`.
/// @param originationPd - Lifetime probability of default at initial recognition as a decimal in `[0, 1]`.
/// @param config - Optional `StagingConfig` thresholds, backstops and curing rules; omitted uses the Rust defaults (object or JSON).
/// @returns `StageResult`: the assigned `stage`, the ordered `triggers` audit trail and whether the exposure `cured`.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed or a PD, maturity
/// or threshold is non-finite or out of range, and kind `invalid_type` if a
/// PD is not a number.
#[wasm_bindgen(js_name = classifyStage, unchecked_return_type = "generated.statements_analytics.StageResult")]
pub fn classify_stage(
    #[wasm_bindgen(unchecked_param_type = "generated.statements_analytics.Exposure | string")]
    exposure: JsValue,
    #[wasm_bindgen(js_name = "currentPd", unchecked_param_type = "number")] current_pd: JsValue,
    #[wasm_bindgen(js_name = "originationPd", unchecked_param_type = "number")]
    origination_pd: JsValue,
    #[wasm_bindgen(
        unchecked_optional_param_type = "generated.statements_analytics.StagingConfig | string | null"
    )]
    config: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let exposure: ecl::Exposure = from_js_json(&exposure, "exposure")?;
    let config: ecl::StagingConfig = match config.filter(|c| !c.is_null() && !c.is_undefined()) {
        Some(config) => from_js_json(&config, "config")?,
        None => ecl::StagingConfig::default(),
    };
    let result = ecl::classify_stage(
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
/// Twin of Python and Rust `compute_ecl` (one scenario of weight `1.0`). The
/// priced exposure at default is `ead + undrawn * ccf`; the schedule is
/// anchored at `(0, 0)` when that knot is absent. Classify with
/// `classifyStage` first and pass its `stage`.
/// @param exposure - `Exposure` supplying EAD, undrawn amount and CCF, LGD, EIR, remaining maturity and any EAD schedule (object or JSON).
/// @param pdSchedule - Cumulative PD knots as `[timeYears, cumulativePd]` pairs, ascending in time and non-decreasing in PD (decimals).
/// @param stage - `"stage1"` (12-month horizon), `"stage2"` (lifetime) or `"stage3"` (credit-impaired).
/// @param bucketWidthYears - Integration bucket width in years (`0.25` = quarterly); omitted uses the Rust policy default.
/// @param stage3TimeToRecoveryYears - Stage 3 discounting horizon to expected recovery in years; omitted uses the Rust policy default.
/// @returns `WeightedEclResult`: the ECL in the exposure's currency units, the stage and the per-scenario bucket audit trail.
///
/// # Errors
///
/// Throws with kind `validation` if an input is malformed, `stage` is not a
/// stage name, the schedule violates the cumulative-PD invariants, or the
/// exposure fails validation.
#[wasm_bindgen(js_name = computeEcl, unchecked_return_type = "generated.statements_analytics.WeightedEclResult")]
pub fn compute_ecl(
    #[wasm_bindgen(unchecked_param_type = "generated.statements_analytics.Exposure | string")]
    exposure: JsValue,
    #[wasm_bindgen(
        js_name = "pdSchedule",
        unchecked_param_type = "[number, number][] | string"
    )]
    pd_schedule: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.statements_analytics.Stage")] stage: JsValue,
    #[wasm_bindgen(
        js_name = "bucketWidthYears",
        unchecked_optional_param_type = "number | null"
    )]
    bucket_width_years: Option<JsValue>,
    #[wasm_bindgen(
        js_name = "stage3TimeToRecoveryYears",
        unchecked_optional_param_type = "number | null"
    )]
    stage3_time_to_recovery_years: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let exposure: ecl::Exposure = from_js_json(&exposure, "exposure")?;
    let pd_schedule: Vec<(f64, f64)> = from_js_json(&pd_schedule, "pdSchedule")?;
    let result = ecl::compute_ecl(
        &exposure,
        js_wire::<ecl::Stage>(&stage, "stage")?,
        pd_schedule,
        js_opt_f64(bucket_width_years.as_ref(), "bucketWidthYears")?,
        js_opt_f64(
            stage3_time_to_recovery_years.as_ref(),
            "stage3TimeToRecoveryYears",
        )?,
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Measure ECL under named, jointly weighted macroeconomic scenarios.
/// @param exposure - Credit exposure whose allowance will be calculated.
/// @param scenarios - Unique scenario identifiers, exact decimal weights summing to one,
///   optional cumulative-PD schedules anchored at (0,0), and optional LGD fractions.
/// @param stage - Already resolved IFRS 9 impairment stage.
/// @param bucketWidthYears - Integration width in years, defaulting to the Rust policy.
/// @param stage3TimeToRecoveryYears - Elected discount horizon for Stage 3 recovery.
/// @returns Scenario evidence and total allowance. The total is null if required evidence is unavailable.
/// @throws Validation error for invalid exposure, stage, scenario weights, or supplied PD or LGD data.
#[wasm_bindgen(js_name = computeEclWeighted, unchecked_return_type = "generated.statements_analytics.WeightedEclResult")]
pub fn compute_ecl_weighted(
    #[wasm_bindgen(unchecked_param_type = "generated.statements_analytics.Exposure | string")]
    exposure: JsValue,
    #[wasm_bindgen(
        unchecked_param_type = "readonly generated.statements_analytics.EclScenario[] | string"
    )]
    scenarios: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.statements_analytics.Stage")] stage: JsValue,
    #[wasm_bindgen(
        js_name = "bucketWidthYears",
        unchecked_optional_param_type = "number | null"
    )]
    bucket_width_years: Option<JsValue>,
    #[wasm_bindgen(
        js_name = "stage3TimeToRecoveryYears",
        unchecked_optional_param_type = "number | null"
    )]
    stage3_time_to_recovery_years: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let exposure: ecl::Exposure = from_js_json(&exposure, "exposure")?;
    let scenarios: Vec<ecl::EclScenario> = from_js_json(&scenarios, "scenarios")?;
    let result = ecl::compute_ecl_weighted(
        &exposure,
        js_wire::<ecl::Stage>(&stage, "stage")?,
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
