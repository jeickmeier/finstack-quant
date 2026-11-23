//! Scenario application and horizon-return WASM entry points.

use super::spec::{parse_config, scenario_spec};
use crate::utils::input::{js_opt_string, js_string, json_text, opt_json_text};
use crate::utils::{parse_iso_date, to_js_err};
use wasm_bindgen::prelude::*;

fn extract_instruments(
    json: Option<String>,
) -> Result<Option<Vec<Box<dyn finstack_quant_valuations::instruments::Instrument>>>, JsValue> {
    json.as_deref()
        .map(finstack_quant_valuations::pricer::parse_boxed_instruments_from_json)
        .transpose()
        .map_err(to_js_err)
}

/// Apply a scenario to copied market data and an optional financial model.
///
/// Returns a JavaScript object with `market` and `model` (the mutated
/// contexts as objects, not JSON strings), and a canonical `report` with `operations_applied`,
/// `user_operations`, `expanded_operations`, `changes` (a
/// `ScenarioChangeManifest`), `warnings`, `meta` (a `ResultsMeta` audit stamp
/// carrying the numeric mode, rounding context, and FX policy; omitted when
/// absent), and `time_roll` (a `RollForwardReport`, only present when the
/// scenario contained a `time_roll_forward` operation).
///
/// Optional instrument envelopes are copied and returned in `instruments`, in
/// input order. The Rust engine rejects instrument-scoped operations without
/// an inventory. No holiday calendar is supplied; business-day rolls adjust
/// without holiday information.
///
/// # Errors
///
/// Rejects a malformed or invalid scenario (checked before the market is
/// parsed), malformed market, model, instrument, or configuration JSON,
/// instrument-scoped operations without `instruments_json`, a model that fails
/// semantic validation (the same `FinancialModelSpec::from_json` check the
/// statements exports and Python `apply_scenario` apply), an invalid ISO
/// `as_of` date, an invalid scenario operation, missing market objects or
/// hierarchy context, statement-model execution failures, failure to encode
/// the mutated contexts, or failure to serialize the application envelope to
/// JavaScript.
/// @param scenarioJson - JSON-serialized ScenarioSpec to validate and apply.
/// @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param modelJson - Optional FinancialModelSpec JSON; omit for market-only scenarios.
/// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param instrumentsJson - Optional JSON array of canonical instrument envelopes; required for instrument shocks and returned as shocked copies in input order.
/// @param configJson - Optional FinstackConfig JSON; its rounding policy is stamped into `meta`. Omit for the library default.
/// @returns Mutated market and optional model after applying the scenario.
#[wasm_bindgen(js_name = applyScenario, unchecked_return_type = "ApplicationEnvelope")]
pub fn apply_scenario(
    #[wasm_bindgen(js_name = "scenarioJson", unchecked_param_type = "JsonInput")]
    scenario_json: JsValue,
    #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
    market_json: JsValue,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    #[wasm_bindgen(js_name = "modelJson", unchecked_optional_param_type = "JsonInput")] model_json: Option<JsValue>,
    #[wasm_bindgen(
        js_name = "instrumentsJson",
        unchecked_optional_param_type = "JsonInput"
    )]
    instruments_json: Option<JsValue>,
    #[wasm_bindgen(js_name = "configJson", unchecked_optional_param_type = "JsonInput")]
    config_json: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let spec = scenario_spec(&scenario_json, "scenarioJson")?;
    let market_json: &str = &json_text(&market_json, "marketJson")?;
    let model_json = opt_json_text(model_json.as_ref(), "modelJson")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let instruments_json = opt_json_text(instruments_json.as_ref(), "instrumentsJson")?;
    let config = parse_config(config_json.as_ref())?;
    let mut market: finstack_quant_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;
    let mut model = model_json
        .as_deref()
        .map(finstack_quant_statements::types::FinancialModelSpec::from_json)
        .transpose()
        .map_err(crate::utils::contract_to_js_err)?;
    let date = parse_iso_date(as_of)?;
    let mut instruments = extract_instruments(instruments_json)?;
    let report = {
        let mut ctx = finstack_quant_scenarios::ExecutionContext {
            market: &mut market,
            model: model.as_mut(),
            instruments: instruments.as_mut(),
            rate_bindings: None,
            calendar: None,
            as_of: date,
        };
        finstack_quant_scenarios::ScenarioEngine::with_config(config)
            .apply(&spec, &mut ctx)
            .map_err(to_js_err)?
    };
    let out = finstack_quant_scenarios::ApplicationEnvelope::from_contexts(
        report,
        &market,
        model.as_ref(),
        instruments.as_deref(),
    )
    .map_err(to_js_err)?;
    crate::utils::to_js_value(&out)
}

/// Compute horizon total return under a scenario.
///
/// Applies a scenario specification to project an instrument forward, then
/// decomposes the resulting P&L using factor-based attribution.
///
/// # Arguments
///
/// * `instrument_json` - Canonical `finstack_quant.instrument/1` envelope.
/// * `market_json` - JSON-serialized `MarketContext`.
/// * `as_of` - Valuation date (ISO 8601).
/// * `scenario_json` - JSON-serialized `ScenarioSpec`.
/// * `method` - `AttributionMethod` wire value: a unit variant name such as
///   `"parallel"` or `"metrics_based"`, an object such as
///   `{ waterfall: ["carry", "rates_curves"] }` or `{ taylor: {} }`, or the
///   same value as JSON text. Omit for the Rust default
///   (`AttributionMethod::default()`, currently "parallel"). "metrics_based"
///   calculates the canonical registry's
///   applicable subset of default attribution metrics at the opening snapshot;
///   metrics unsupported by the instrument type are omitted, and when a
///   hazard curve of the instrument has no calibration recipe and that menu
///   fails, the quote-space credit metrics are dropped with a note and
///   credit-curve P&L flows to residual; any other failure calculating
///   selected metrics throws.
/// * `config_json` - Optional FinstackConfig JSON for horizon analysis; omit to use defaults.
/// * `calendar_id` - Optional built-in holiday calendar (e.g. "nyse", "target") used to
///   business-day adjust `time_roll_forward` targets under `business_days` mode.
///   Omit for a weekends-only calendar; unknown identifiers throw.
///
/// # Returns
///
/// The serde `HorizonResult` (`attribution`, `initial_value` and
/// `terminal_value` as exact-decimal Money objects, `horizon_days`, null
/// without a time roll, and `scenario_report`) plus `summary`: the Rust-computed
/// `total_return`, `annualized_return`, `currency` and `factor_contributions`
/// that Python exposes as `HorizonResult` accessors. Rust computes every
/// derived value. Undefined derived values are null here and NaN in Python:
/// total return and factor contributions require positive initial value and
/// matching P&L currency; annualization also requires a positive horizon and
/// total return at or above -100%.
///
/// # Errors
///
/// Rejects a malformed or invalid scenario; malformed instrument, market, or
/// configuration JSON; an
/// invalid ISO `as_of` date; an unsupported attribution `method`; an unknown
/// `calendar_id`; invalid, unsupported, or unresolved scenario operations;
/// missing market data; pricing or attribution failures; or failure to
/// serialize the horizon result to JavaScript.
/// @param instrumentJson - Canonical `finstack_quant.instrument/1` envelope.
/// @param marketJson - JSON-serialized `MarketContext`.
/// @param asOf - Valuation date (ISO 8601).
/// @param scenarioJson - JSON-serialized `ScenarioSpec`.
/// @param method - `AttributionMethod` wire value: a unit variant name such as `"parallel"` or `"metrics_based"`, an object such as `{ waterfall: ["carry", "rates_curves"] }` or `{ taylor: {} }`, or the same value as JSON text. Omit for the Rust default (`AttributionMethod::default()`, currently "parallel"). "metrics_based" calculates the canonical registry's applicable subset of default attribution metrics at the opening snapshot; metrics unsupported by the instrument type are omitted, and when a hazard curve of the instrument has no calibration recipe and that menu fails, the quote-space credit metrics are dropped with a note and credit-curve P&L flows to residual; any other failure calculating selected metrics throws.
/// @param configJson - Optional FinstackConfig JSON for horizon analysis; omit to use defaults.
/// @param calendarId - Optional built-in holiday calendar (e.g. "nyse", "target") used to business-day adjust `time_roll_forward` targets under `business_days` mode. Omit for a weekends-only calendar; unknown identifiers throw.
/// @returns The serde `HorizonResult` (`attribution`, `initial_value` and `terminal_value` as exact-decimal Money objects, `horizon_days`, null without a time roll, and `scenario_report`) plus `summary`: the Rust-computed `total_return`, `annualized_return`, `currency` and `factor_contributions` that Python exposes as `HorizonResult` accessors. Rust computes every derived value. Undefined derived values are null here and NaN in Python: total return and factor contributions require positive initial value and matching P&L currency; annualization also requires a positive horizon and total return at or above -100%.
#[wasm_bindgen(js_name = computeHorizonReturn, unchecked_return_type = "HorizonReport")]
pub fn compute_horizon_return(
    #[wasm_bindgen(js_name = "instrumentJson", unchecked_param_type = "JsonInput")] instrument_json: JsValue,
    #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
    market_json: JsValue,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    #[wasm_bindgen(js_name = "scenarioJson", unchecked_param_type = "JsonInput")]
    scenario_json: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "JsonInput")] method: Option<JsValue>,
    #[wasm_bindgen(js_name = "configJson", unchecked_optional_param_type = "JsonInput")]
    config_json: Option<JsValue>,
    #[wasm_bindgen(js_name = "calendarId", unchecked_optional_param_type = "string")]
    calendar_id: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let instrument_json: &str = &json_text(&instrument_json, "instrumentJson")?;
    let market_json: &str = &json_text(&market_json, "marketJson")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let scenario = scenario_spec(&scenario_json, "scenarioJson")?;
    let attribution_method: finstack_quant_attribution::AttributionMethod =
        crate::utils::wire::js_opt_wire(method.as_ref(), "method")?.unwrap_or_default();
    let config = parse_config(config_json.as_ref())?;
    let calendar_id = js_opt_string(calendar_id.as_ref(), "calendarId")?;
    use std::sync::Arc;

    let boxed = finstack_quant_valuations::pricer::parse_boxed_instrument_from_json(
        instrument_json,
        None,
        None,
    )
    .map_err(to_js_err)?;
    let instrument: Arc<dyn finstack_quant_valuations::instruments::Instrument> =
        Arc::from(boxed.into_boxed());

    let market: finstack_quant_core::market_data::context::MarketContext =
        serde_json::from_str(market_json).map_err(to_js_err)?;

    let date = parse_iso_date(as_of)?;

    let mut analyzer = finstack_quant_scenarios::HorizonAnalysis::new(attribution_method, config);
    if let Some(id) = calendar_id.as_deref() {
        analyzer = analyzer.with_calendar_id(id);
    }
    let result = analyzer
        .compute(&instrument, &market, date, &scenario)
        .map_err(to_js_err)?;

    crate::utils::to_js_value(&result.report())
}
