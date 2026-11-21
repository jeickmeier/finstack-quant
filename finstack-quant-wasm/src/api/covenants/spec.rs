//! Covenant definitions: constructors and modifiers over wire values.
//!
//! `CovenantType`, `CovenantConsequence`, `Covenant`, `CovenantSpec` and
//! `ThresholdSchedule` cross the boundary as the plain objects their serde
//! contracts describe, typed by the generated `covenants` TypeScript
//! declarations. The Rust constructors and methods are free functions here.

use crate::utils::input::js_string;
use crate::utils::to_js_value;
use crate::utils::wire::{js_date, js_wire};
use finstack_quant_core::dates::Tenor;
use finstack_quant_covenants::{Covenant, CovenantSpec, CovenantType, ThresholdSchedule};
use wasm_bindgen::prelude::*;

// --- Covenant ----------------------------------------------------------------

/// Create a covenant with the Rust defaults.
///
/// Mirrors Rust `Covenant::new` (the Python `Covenant(...)` constructor): an
/// active maintenance covenant with no cure period, consequences or
/// springing condition.
///
/// @param covenantType - `CovenantType` wire value, for example `{ max_debt_to_ebitda: { threshold: 4.5 } }`.
/// @param testFrequency - `Tenor` wire object (`{ count, unit }`) recording how often the covenant is tested; it is metadata, the caller chooses each test date.
/// @param label - Instance label; it is the report key and must be unique within an engine on a test date.
/// @returns `Covenant` wire object.
/// @throws If an argument does not match its wire type (kind `invalid_type` or `validation`).
#[wasm_bindgen(js_name = covenantNew, unchecked_return_type = "generated.covenants.Covenant")]
pub fn covenant_new(
    #[wasm_bindgen(
        js_name = "covenantType",
        unchecked_param_type = "generated.covenants.CovenantType"
    )]
    covenant_type: JsValue,
    #[wasm_bindgen(
        js_name = "testFrequency",
        unchecked_param_type = "generated.covenants.Tenor"
    )]
    test_frequency: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] label: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&Covenant::new(
        js_wire::<CovenantType>(&covenant_type, "covenantType")?,
        js_wire::<Tenor>(&test_frequency, "testFrequency")?,
        js_string(&label, "label")?,
    ))
}

// --- CovenantSpec / ThresholdSchedule ----------------------------------------------

/// Covenant specification that reads the covenant type's default metric.
///
/// Mirrors Rust `CovenantSpec::new` (the Python `CovenantSpec(covenant)`
/// constructor without a metric id): a net debt / EBITDA covenant also gets
/// the default `"ebitda"` earnings denominator.
///
/// @param covenant - `Covenant` wire object.
/// @returns `CovenantSpec` wire object with no `metric_id`; evaluation reads the covenant type's conventional metric (for example `"debt_to_ebitda"`) or the `metric` / `name` of a custom or basket covenant.
/// @throws If `covenant` is not a `Covenant` (kind `validation`).
#[wasm_bindgen(js_name = covenantSpecNew, unchecked_return_type = "generated.covenants.CovenantSpec")]
pub fn covenant_spec_new(
    #[wasm_bindgen(unchecked_param_type = "generated.covenants.Covenant")] covenant: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantSpec::new(js_wire::<Covenant>(
        &covenant, "covenant",
    )?))
}

/// Covenant specification that reads its test value from a named metric.
///
/// Mirrors Rust `CovenantSpec::with_metric` (the Python `CovenantSpec(...)`
/// constructor with a metric id): a net debt / EBITDA covenant also gets the
/// default `"ebitda"` earnings denominator.
///
/// @param covenant - `Covenant` wire object.
/// @param metricId - Identifier of the metric the covenant tests, as supplied in the metric map, for example `"debt_to_ebitda"`.
/// @returns `CovenantSpec` wire object.
/// @throws If `covenant` is not a `Covenant` (kind `validation`) or `metricId` is not a string (kind `invalid_type`).
#[wasm_bindgen(js_name = covenantSpecWithMetric, unchecked_return_type = "generated.covenants.CovenantSpec")]
pub fn covenant_spec_with_metric(
    #[wasm_bindgen(unchecked_param_type = "generated.covenants.Covenant")] covenant: JsValue,
    #[wasm_bindgen(js_name = "metricId", unchecked_param_type = "string")] metric_id: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&CovenantSpec::with_metric(
        js_wire::<Covenant>(&covenant, "covenant")?,
        js_string(&metric_id, "metricId")?,
    ))
}

/// Threshold in force on a test date.
///
/// @param schedule - `ThresholdSchedule` wire value: `[isoDate, threshold]` pairs.
/// @param testDate - ISO-8601 covenant test date.
/// @returns The threshold of the latest entry effective on or before `testDate`, or `undefined` when the schedule starts later.
/// @throws If `schedule` has a non-finite threshold or repeats a date (kind `validation`), or `testDate` is not an ISO-8601 string (kind `invalid_type` or `validation`).
#[wasm_bindgen(js_name = thresholdScheduleThresholdFor)]
pub fn threshold_schedule_threshold_for(
    #[wasm_bindgen(unchecked_param_type = "generated.covenants.ThresholdSchedule")]
    schedule: JsValue,
    #[wasm_bindgen(js_name = "testDate", unchecked_param_type = "string")] test_date: JsValue,
) -> Result<Option<f64>, JsValue> {
    Ok(js_wire::<ThresholdSchedule>(&schedule, "schedule")?
        .threshold_for(js_date(&test_date, "testDate")?))
}
