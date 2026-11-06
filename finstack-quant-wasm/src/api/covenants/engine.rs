//! `CovenantEngine` handle and the covenant package templates.

use crate::utils::input::{from_js_json, json_text};
use crate::utils::wire::{js_date, js_wire};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_covenants::{
    CovenantEngine, CovenantScope, CovenantSpec, CovenantWaiver, DatedMetrics, HashMapMetricSource,
    templates,
};
use wasm_bindgen::prelude::*;

/// Parse a metric map (plain object or JSON) with the Rust metric-map parser.
fn metric_source(metrics: &JsValue) -> Result<HashMapMetricSource, JsValue> {
    HashMapMetricSource::from_json(&json_text(metrics, "metrics")?).map_err(to_js_err)
}

/// Covenant package: specifications, waivers and the breach history.
///
/// Add specifications and waivers, then evaluate metric values on a test
/// date. `evaluate` is read-only; `evaluateAndTrack` also records breaches and
/// cures.
#[wasm_bindgen(js_name = CovenantEngine)]
#[derive(Clone, Debug, Default)]
pub struct JsCovenantEngine {
    pub(crate) inner: CovenantEngine,
}

#[wasm_bindgen(js_class = CovenantEngine)]
impl JsCovenantEngine {
    /// Create an empty engine.
    ///
    /// @returns An engine with no specifications, waivers or breach history.
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsCovenantEngine {
        JsCovenantEngine::default()
    }

    /// Build an engine from covenant specifications.
    ///
    /// @param specs - `CovenantSpec` wire objects, for example the result of `covLite` or `lboStandard`; they are added in order.
    /// @returns A `CovenantEngine` holding the specifications; it is validated when evaluated.
    /// @throws If `specs` is not an array of `CovenantSpec` (kind `validation`).
    #[wasm_bindgen(js_name = fromSpecs)]
    pub fn from_specs(specs: JsValue) -> Result<JsCovenantEngine, JsValue> {
        let mut inner = CovenantEngine::new();
        for spec in js_wire::<Vec<CovenantSpec>>(&specs, "specs")? {
            inner.add_spec(spec);
        }
        Ok(JsCovenantEngine { inner })
    }

    /// Parse an engine from its canonical JSON.
    ///
    /// @param json - `CovenantEngine` JSON string or plain object; only `specs` is required and unknown fields are rejected.
    /// @returns The parsed `CovenantEngine`; it is validated when evaluated.
    /// @throws If `json` is neither a string nor a plain object (kind `invalid_type`) or does not match the engine schema (kind `validation`).
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsCovenantEngine, JsValue> {
        serde_json::from_str(&json_text(&json, "json")?)
            .map(|inner| JsCovenantEngine { inner })
            .map_err(to_js_err)
    }

    /// Serialize the engine, including its breach history, to canonical JSON.
    ///
    /// @returns `CovenantEngine` JSON accepted by `fromJson` and by `evaluateEngine`.
    /// @throws If the engine cannot be serialized.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Add a covenant specification in place.
    ///
    /// @param spec - `CovenantSpec` wire object. Its covenant label is the report key and must be unique among specifications applicable on a test date.
    /// @throws If `spec` is not a `CovenantSpec` (kind `validation`).
    #[wasm_bindgen(js_name = addSpec)]
    pub fn add_spec(&mut self, spec: JsValue) -> Result<(), JsValue> {
        self.inner.add_spec(js_wire::<CovenantSpec>(&spec, "spec")?);
        Ok(())
    }

    /// Add a waiver or threshold amendment in place.
    ///
    /// @param waiver - `CovenantWaiver` wire object: covenant id, effective and optional expiry dates, and an optional amended threshold (omitted means a full waiver).
    /// @throws If `waiver` is not a `CovenantWaiver` (kind `validation`).
    #[wasm_bindgen(js_name = addWaiver)]
    pub fn add_waiver(&mut self, waiver: JsValue) -> Result<(), JsValue> {
        self.inner
            .add_waiver(js_wire::<CovenantWaiver>(&waiver, "waiver")?);
        Ok(())
    }

    /// Check the engine configuration without evaluating.
    ///
    /// @throws If a specification, window, waiver or breach record is invalid (kind `validation`).
    #[wasm_bindgen]
    pub fn validate(&self) -> Result<(), JsValue> {
        self.inner.validate().map_err(to_js_err)
    }

    /// Evaluate every applicable covenant on a test date.
    ///
    /// Read-only: the breach history is not updated.
    ///
    /// @param metrics - Metric values keyed by covenant metric identifier (plain object or JSON), in the units the tests expect: ratios in turns (`4.5` means 4.5x), amounts in the reporting currency.
    /// @param as_of - ISO-8601 test date; it selects the active window, waivers, threshold schedule and cure-period state.
    /// @returns Covenant reports keyed by stable covenant instance key, in engine order.
    /// @throws If the engine is invalid, two applicable specifications share an instance key, a metric value is not a number (kind `validation`), or a required metric is missing (kind `not_found`).
    #[wasm_bindgen]
    pub fn evaluate(&self, metrics: JsValue, as_of: JsValue) -> Result<JsValue, JsValue> {
        let source = metric_source(&metrics)?;
        let reports = self
            .inner
            .evaluate(&source, js_date(&as_of, "asOf")?)
            .map_err(to_js_err)?;
        to_js_value(&reports)
    }

    /// Evaluate like `evaluate` and update the breach history.
    ///
    /// A failing covenant without an active breach gains a breach record with
    /// its cure deadline; a later pass inside the cure period marks it cured.
    /// On error the history is left untouched.
    ///
    /// @param metrics - Metric values keyed by covenant metric identifier (plain object or JSON), in the units the tests expect.
    /// @param as_of - ISO-8601 test date; it also stamps new breach records.
    /// @param scope - `"maintenance"` for scheduled testing, or `"incurrence"` only for a completed action assessed with its pro forma metrics.
    /// @returns Covenant reports keyed by stable covenant instance key, in engine order.
    /// @throws If `scope` is not one of the listed strings, the engine is invalid, a metric value is not a number (kind `validation`), or a required metric is missing (kind `not_found`).
    #[wasm_bindgen(js_name = evaluateAndTrack)]
    pub fn evaluate_and_track(
        &mut self,
        metrics: JsValue,
        as_of: JsValue,
        scope: JsValue,
    ) -> Result<JsValue, JsValue> {
        let scope = js_wire::<CovenantScope>(&scope, "scope")?;
        let source = metric_source(&metrics)?;
        let reports = self
            .inner
            .evaluate_and_track(&source, js_date(&as_of, "asOf")?, scope)
            .map_err(to_js_err)?;
        to_js_value(&reports)
    }

    /// Evaluate the engine on each dated row of metric values.
    ///
    /// Each row is evaluated independently; the breach history is not updated.
    ///
    /// @param metrics - `DatedMetrics` rows (`{ date, metrics }`): metric values per ISO-8601 test date, evaluated in the order given. (Python takes a date-indexed DataFrame.)
    /// @returns `DatedCovenantReports` rows (`{ as_of, reports }`), one per input row in input order. (Python returns the same reports as a long DataFrame.)
    /// @throws If `metrics` is not an array of `DatedMetrics`, the engine is invalid (kind `validation`), or a required metric is missing on a date (kind `not_found`).
    #[wasm_bindgen(js_name = evaluateSeries)]
    pub fn evaluate_series(&self, metrics: JsValue) -> Result<JsValue, JsValue> {
        let rows = js_wire::<Vec<DatedMetrics>>(&metrics, "metrics")?;
        to_js_value(&self.inner.evaluate_series(&rows).map_err(to_js_err)?)
    }

    /// Covenant specifications, in the order they were added.
    ///
    /// @returns `CovenantSpec` wire objects.
    /// @throws If the specifications cannot be serialized.
    #[wasm_bindgen(getter)]
    pub fn specs(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.specs)
    }

    /// Waivers and threshold amendments.
    ///
    /// @returns `CovenantWaiver` wire objects.
    /// @throws If the waivers cannot be serialized.
    #[wasm_bindgen(getter)]
    pub fn waivers(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.waivers)
    }

    /// Breach records written by `evaluateAndTrack`.
    ///
    /// @returns `CovenantBreach` wire objects, oldest first; each keeps its original consequences and cure state.
    /// @throws If the breach history cannot be serialized.
    #[wasm_bindgen(getter, js_name = breachHistory)]
    pub fn breach_history(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.breach_history)
    }
}

/// Build the covenant specs of one standard package.
///
/// @param template - `CovenantTemplate` object or JSON text: `kind` (`"lbo_standard"`, `"cov_lite"`, `"real_estate"` or `"project_finance"`) and that package's thresholds by Rust field name (ratios in turns, `min_debt_yield` and `max_ltv` as decimals, `max_capex` and `min_liquidity` in the reporting currency).
/// @returns `CovenantSpec` wire objects in the package's documented order, ready for `CovenantEngine.fromSpecs`.
/// @throws If the template is malformed, has an unknown kind or field, or a threshold is NaN, infinite or negative (kind `validation`).
#[wasm_bindgen(js_name = covenantTemplate)]
pub fn covenant_template(template: JsValue) -> Result<JsValue, JsValue> {
    let template: templates::CovenantTemplate = from_js_json(&template, "template")?;
    to_js_value(&templates::covenant_template(&template).map_err(to_js_err)?)
}
