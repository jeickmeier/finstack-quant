//! WASM façade for composite initialization, rebalancing, decomposition, and history.
//!
//! Pricing uses frozen quantities. Only `initialize` / `rebalance` calculate a
//! new state. Period return is `pnl / capital`; `return_index` starts at `100`.
//! Close-effective rebalances report pre-trade P&L, then open the next interval
//! at the post-trade financed value. There is no separate `initializeFixed`
//! export; `initialize` resolves `fixed_quantity` without history.

use crate::utils::input::{from_js_json, js_string, json_text, opt_json_text};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::instruments::composite::{
    history, history_from_spec, CompositeInstrument, CompositeMarketObservation, CompositeSpec,
};
use finstack_quant_valuations::metrics::MetricId;
use wasm_bindgen::prelude::*;

fn parse_spec(json: &str) -> Result<CompositeSpec, JsValue> {
    serde_json::from_str(json).map_err(to_js_err)
}

fn parse_composite(json: &str) -> Result<CompositeInstrument, JsValue> {
    finstack_quant_valuations::pricer::parse_typed_instrument_json(json).map_err(to_js_err)
}

fn parse_observations(json: Option<&str>) -> Result<Vec<CompositeMarketObservation>, JsValue> {
    serde_json::from_str(json.unwrap_or("[]")).map_err(to_js_err)
}

fn parse_metrics(metrics: Option<JsValue>) -> Result<Vec<MetricId>, JsValue> {
    match metrics {
        None => Ok(Vec::new()),
        Some(value) if value.is_null() || value.is_undefined() => Ok(Vec::new()),
        Some(value) => from_js_json::<Vec<MetricId>>(&value, "metrics"),
    }
}

/// Resolve an unresolved composite specification into a priceable instrument.
///
/// Fixed-quantity specs resolve without historical observations. Volatility
/// weighting requires `history_json` to be strictly increasing and to end on
/// `as_of`. There is no separate `initializeFixed` export.
///
/// @param spec_json - Bare canonical `CompositeSpec` JSON.
/// @param market_json - Complete canonical market-context JSON at the effective date.
/// @param as_of - ISO-8601 effective date for the resolved holdings state.
/// @param history_json - Optional chronological `CompositeMarketObservation[]` JSON available through `asOf`.
/// @returns Object containing a canonical composite instrument envelope and primitive establishment trades.
///
/// # Arguments
///
/// * `spec_json` - Bare serialized composite definition with embedded instruments.
/// * `market_json` - Complete market used for unit values, additive metrics,
///   notionals, and reporting-currency FX conversion.
/// * `as_of` - ISO-8601 state effective date; no later history is permitted.
/// * `history_json` - Optional strictly increasing dated observations. Required
///   for volatility weighting and must end on `as_of`; unused for `fixed_quantity`.
///
/// # Errors
///
/// Throws for malformed JSON, invalid specifications, missing market/history
/// inputs, unsupported metrics/notionals, or non-finite resolved quantities.
#[wasm_bindgen(js_name = initializeComposite)]
pub fn initialize_composite(
    spec_json: JsValue,
    market_json: JsValue,
    as_of: JsValue,
    history_json: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let spec_json: &str = &json_text(&spec_json, "specJson")?;
    let market_json: &str = &json_text(&market_json, "marketJson")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let history_json = opt_json_text(history_json.as_ref(), "historyJson")?;
    let spec = parse_spec(spec_json)?;
    let market = super::pricing::parse_market_json(market_json)?;
    let date = finstack_quant_core::dates::parse_iso_date(as_of).map_err(to_js_err)?;
    let history = parse_observations(history_json.as_deref())?;
    let result = spec
        .initialize(&market, date, &history)
        .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Explicitly rebalance a resolved composite without mutating the prior state.
///
/// Trades are net primitive quantity deltas from the supplied envelope to the
/// newly resolved state. Volatility weighting requires history ending on `as_of`.
///
/// @param instrument_json - Canonical resolved composite instrument envelope.
/// @param market_json - Complete canonical market-context JSON at the rebalance date.
/// @param as_of - ISO-8601 effective date for the new state.
/// @param history_json - Optional chronological observation JSON available through `asOf`.
/// @returns Object containing the new envelope and net primitive quantity deltas.
///
/// # Arguments
///
/// * `instrument_json` - Existing resolved composite envelope used as the trade baseline.
/// * `market_json` - Complete market used to resolve new quantities and convert
///   notionals, metrics, and FX into the reporting currency.
/// * `as_of` - ISO-8601 effective date for the distinct returned state.
/// * `history_json` - Optional strictly increasing dated observations. Required
///   for volatility weighting and must end on `as_of`.
///
/// # Errors
///
/// Throws for malformed inputs, a non-composite envelope, invalid history,
/// missing market data, or quantity-resolution failures.
#[wasm_bindgen(js_name = rebalanceComposite)]
pub fn rebalance_composite(
    instrument_json: JsValue,
    market_json: JsValue,
    as_of: JsValue,
    history_json: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let instrument_json: &str = &json_text(&instrument_json, "instrumentJson")?;
    let market_json: &str = &json_text(&market_json, "marketJson")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let history_json = opt_json_text(history_json.as_ref(), "historyJson")?;
    let instrument = parse_composite(instrument_json)?;
    let market = super::pricing::parse_market_json(market_json)?;
    let date = finstack_quant_core::dates::parse_iso_date(as_of).map_err(to_js_err)?;
    let history = parse_observations(history_json.as_deref())?;
    let result = instrument
        .rebalance(&market, date, &history)
        .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Return path-level plus net/gross primitive value and additive risk.
///
/// Only additive metrics can be requested. Yield, duration, implied
/// volatility, and other non-linear measures are rejected. Amounts are
/// converted to the composite reporting currency on `as_of`.
///
/// @param instrument_json - Canonical resolved composite instrument envelope.
/// @param market_json - Complete canonical market-context JSON used for primitive pricing and FX.
/// @param as_of - ISO-8601 valuation date.
/// @param metrics - Optional additive metric identifier array.
/// @returns Plain object containing primitive paths and net/gross aggregates.
///
/// # Arguments
///
/// * `instrument_json` - Resolved composite whose frozen quantities are decomposed.
/// * `market_json` - Complete valuation and FX context.
/// * `as_of` - ISO-8601 valuation date used for prices, metrics, and FX.
/// * `metrics` - Optional JavaScript array of additive metric identifiers;
///   omit or pass `[]` to report value only.
///
/// # Errors
///
/// Throws for invalid input, non-additive metrics, missing market data, or
/// primitive valuation failures.
#[wasm_bindgen(js_name = compositePrimitiveExposures)]
pub fn composite_primitive_exposures(
    instrument_json: JsValue,
    market_json: JsValue,
    as_of: JsValue,
    metrics: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let instrument_json: &str = &json_text(&instrument_json, "instrumentJson")?;
    let market_json: &str = &json_text(&market_json, "marketJson")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let instrument = parse_composite(instrument_json)?;
    let market = super::pricing::parse_market_json(market_json)?;
    let date = finstack_quant_core::dates::parse_iso_date(as_of).map_err(to_js_err)?;
    let metrics = parse_metrics(metrics)?;
    let report = instrument
        .primitive_exposures(&market, date, &metrics)
        .map_err(to_js_err)?;
    to_js_value(&report)
}

/// Flatten current holdings or a state transition into executable primitive deltas.
///
/// @param instrument_json - Canonical target composite instrument envelope.
/// @param previous_instrument_json - Optional canonical prior composite envelope; omit for establishment trades.
/// @returns Primitive trade array with signed quantity deltas.
///
/// # Arguments
///
/// * `instrument_json` - Target resolved composite state.
/// * `previous_instrument_json` - Optional prior resolved state used to calculate net deltas.
///
/// # Errors
///
/// Throws for malformed or non-composite envelopes, conflicting primitive
/// definitions, or invalid frozen states.
#[wasm_bindgen(js_name = compositeExecutionTrades)]
pub fn composite_execution_trades(
    instrument_json: JsValue,
    previous_instrument_json: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let instrument_json: &str = &json_text(&instrument_json, "instrumentJson")?;
    let previous_instrument_json =
        opt_json_text(previous_instrument_json.as_ref(), "previousInstrumentJson")?;
    let instrument = parse_composite(instrument_json)?;
    let previous = previous_instrument_json
        .as_deref()
        .map(parse_composite)
        .transpose()?;
    let trades = instrument
        .execution_trades(previous.as_ref())
        .map_err(to_js_err)?;
    to_js_value(&trades)
}

/// Initialize a specification at the first snapshot and calculate dated history.
///
/// Warmup snapshots feed dynamic weighting only. Each row values the state
/// held into that close. A scheduled rebalance is close-effective: the row
/// reports pre-trade P&L, then the next interval opens at the post-trade
/// financed value. Period return is `pnl / capital`. The first row has
/// `return_index = 100` and zero P&L, cashflows, and period return.
///
/// @param spec_json - Bare canonical `CompositeSpec` JSON.
/// @param observations_json - Strictly increasing output observation array JSON.
/// @param warmup_json - Optional strictly earlier observation array used only for weighting inputs.
/// @param metrics - Optional additive primitive metric identifier array.
/// @returns Chronological rows containing value, cashflows, P&L, return, index, exposures, state dates, and trades.
///
/// # Arguments
///
/// * `spec_json` - Unresolved composite specification initialized from warmup
///   plus the first output observation.
/// * `observations_json` - Non-empty strictly increasing complete snapshots
///   that become output rows.
/// * `warmup_json` - Optional complete snapshots whose last date precedes the
///   first output observation; used only for weighting inputs.
/// * `metrics` - Optional additive primitive metrics reported on every row;
///   omit or pass `[]` to report value only.
///
/// # Errors
///
/// Throws for empty, duplicate, or unordered observations; overlapping warmup;
/// noncanonical metric keys; initialization/rebalance failures; or missing market/history inputs.
#[wasm_bindgen(js_name = compositeHistoryFromSpec)]
pub fn composite_history_from_spec(
    spec_json: JsValue,
    observations_json: JsValue,
    warmup_json: Option<JsValue>,
    metrics: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let spec_json: &str = &json_text(&spec_json, "specJson")?;
    let observations_json: &str = &json_text(&observations_json, "observationsJson")?;
    let warmup_json = opt_json_text(warmup_json.as_ref(), "warmupJson")?;
    let spec = parse_spec(spec_json)?;
    let observations = parse_observations(Some(observations_json))?;
    let warmup = parse_observations(warmup_json.as_deref())?;
    let metrics = parse_metrics(metrics)?;
    let rows = history_from_spec(&spec, &warmup, &observations, &metrics).map_err(to_js_err)?;
    to_js_value(&rows)
}

/// Calculate dated history from an already-resolved composite state.
///
/// The initial state's effective date must be on or before the first
/// observation. Scheduled rebalances after that date are close-effective.
/// Period return is `pnl / capital`; `return_index` starts at `100`.
///
/// @param instrument_json - Canonical resolved composite instrument envelope.
/// @param observations_json - Strictly increasing complete market observation array JSON.
/// @param metrics - Optional additive primitive metric identifier array.
/// @returns Chronological composite history rows.
///
/// # Arguments
///
/// * `instrument_json` - Resolved immutable holdings held from the first
///   observation until a later scheduled rebalance.
/// * `observations_json` - Non-empty strictly increasing complete snapshots
///   that become output rows.
/// * `metrics` - Optional additive primitive metrics reported on every row;
///   omit or pass `[]` to report value only.
///
/// # Errors
///
/// Throws for invalid states, empty/duplicate/unordered observations, missing
/// market data, or valuation and rebalance failures.
#[wasm_bindgen(js_name = compositeHistory)]
pub fn composite_history(
    instrument_json: JsValue,
    observations_json: JsValue,
    metrics: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let instrument_json: &str = &json_text(&instrument_json, "instrumentJson")?;
    let observations_json: &str = &json_text(&observations_json, "observationsJson")?;
    let instrument = parse_composite(instrument_json)?;
    let observations = parse_observations(Some(observations_json))?;
    let metrics = parse_metrics(metrics)?;
    let rows = history(&instrument, &observations, &metrics).map_err(to_js_err)?;
    to_js_value(&rows)
}

// ---------------------------------------------------------------------------
// Constructors and methods of the composite data types
// ---------------------------------------------------------------------------
//
// `WeightingMethod`, `RebalanceRule`, `CompositeLegSpec` and the history rows
// cross the boundary as plain objects; their Rust constructors and methods
// are free functions that take and return the plain object.

use super::typed::arg;
use finstack_quant_valuations::instruments::composite::{
    CompositeHistoryRow, CompositeLegSpec, RebalanceRule, WeightingMethod,
};
use finstack_quant_valuations::instruments::InstrumentEnvelope;

/// `WeightingMethod::FixedQuantity`: leg quantities are the leg scores.
/// @returns The `WeightingMethod` plain object.
/// @throws Error - Throws if the value cannot be converted to JavaScript.
#[wasm_bindgen(js_name = weightingMethodFixedQuantity)]
pub fn weighting_method_fixed_quantity() -> Result<JsValue, JsValue> {
    to_js_value(&WeightingMethod::FixedQuantity)
}

/// `WeightingMethod::NotionalWeighted`: scale the legs to a gross notional.
/// @param gross_notional - Target gross notional as a `Money` plain object in the reporting currency.
/// @returns The `WeightingMethod` plain object.
/// @throws Error - Throws with kind `validation` if `grossNotional` is not a `Money` plain object.
#[wasm_bindgen(js_name = weightingMethodNotionalWeighted)]
pub fn weighting_method_notional_weighted(gross_notional: JsValue) -> Result<JsValue, JsValue> {
    to_js_value(&WeightingMethod::NotionalWeighted {
        gross_notional: arg::json(&gross_notional, "grossNotional")?,
    })
}

/// `WeightingMethod::MetricWeighted`: size the legs by an additive metric.
/// @param metric - Canonical additive metric identifier, e.g. `"dv01"` or `"delta"`.
/// @param anchor_leg_id - Leg whose quantity is fixed at `anchorQuantity`.
/// @param anchor_quantity - Quantity of the anchor leg.
/// @param neutralize - Optional; `true` sizes the other legs so the net metric is zero, `false` (the default) scales them by score.
/// @returns The `WeightingMethod` plain object.
/// @throws Error - Throws with kind `validation` if `metric` is not a canonical metric identifier, and kind `invalid_type` for a wrong argument type.
#[wasm_bindgen(js_name = weightingMethodMetricWeighted)]
pub fn weighting_method_metric_weighted(
    metric: JsValue,
    anchor_leg_id: JsValue,
    anchor_quantity: JsValue,
    neutralize: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    to_js_value(&WeightingMethod::MetricWeighted {
        metric: js_string(&metric, "metric")?.parse().map_err(to_js_err)?,
        anchor_leg_id: arg::id(&anchor_leg_id, "anchorLegId")?,
        anchor_quantity: arg::num(&anchor_quantity, "anchorQuantity")?,
        neutralize: crate::utils::input::js_opt_bool(neutralize.as_ref(), "neutralize")?
            .unwrap_or(false),
    })
}

/// One anchored weighting preset `(anchorLegId, anchorQuantity)`.
macro_rules! anchored_weighting {
    ($(#[$doc:meta])* $name:ident as $js_name:ident => $method:ident) => {
        $(#[$doc])*
        /// @param anchor_leg_id - Leg whose quantity is fixed at `anchorQuantity`.
        /// @param anchor_quantity - Quantity of the anchor leg.
        /// @returns The `WeightingMethod` plain object.
        /// @throws Error - Throws with kind `invalid_type` if `anchorLegId` is not a string or `anchorQuantity` is not a number.
        #[wasm_bindgen(js_name = $js_name)]
        pub fn $name(anchor_leg_id: JsValue, anchor_quantity: JsValue) -> Result<JsValue, JsValue> {
            to_js_value(&WeightingMethod::$method(
                js_string(&anchor_leg_id, "anchorLegId")?,
                arg::num(&anchor_quantity, "anchorQuantity")?,
            ))
        }
    };
}

anchored_weighting!(
    /// DV01-neutral weighting (mirrors Rust `WeightingMethod::dv01_neutral`).
    weighting_method_dv01_neutral as weightingMethodDv01Neutral => dv01_neutral
);
anchored_weighting!(
    /// Delta-neutral weighting (mirrors Rust `WeightingMethod::delta_neutral`).
    weighting_method_delta_neutral as weightingMethodDeltaNeutral => delta_neutral
);
anchored_weighting!(
    /// Duration-weighted sizing (mirrors Rust `WeightingMethod::duration_weighted`).
    weighting_method_duration_weighted as weightingMethodDurationWeighted => duration_weighted
);

/// Inverse-volatility weighting (mirrors Rust `WeightingMethod::volatility_weighted`).
/// @param anchor_leg_id - Leg whose quantity is fixed at `anchorQuantity`.
/// @param anchor_quantity - Quantity of the anchor leg.
/// @param lookback - Number of return observations in the volatility window.
/// @param min_observations - Minimum observations required to estimate a volatility.
/// @param annualization_factor - Periods per year used to annualize volatility (252 for daily data).
/// @returns The `WeightingMethod` plain object.
/// @throws Error - Throws with kind `invalid_type` if a count is not a non-negative whole number or another argument has the wrong type.
#[wasm_bindgen(js_name = weightingMethodVolatilityWeighted)]
pub fn weighting_method_volatility_weighted(
    anchor_leg_id: JsValue,
    anchor_quantity: JsValue,
    lookback: JsValue,
    min_observations: JsValue,
    annualization_factor: JsValue,
) -> Result<JsValue, JsValue> {
    to_js_value(&WeightingMethod::volatility_weighted(
        js_string(&anchor_leg_id, "anchorLegId")?,
        arg::num(&anchor_quantity, "anchorQuantity")?,
        arg::uint(&lookback, "lookback")?,
        arg::uint(&min_observations, "minObservations")?,
        arg::num(&annualization_factor, "annualizationFactor")?,
    ))
}

/// Validate a rebalance rule and return its plain object.
fn rebalance_rule(rule: RebalanceRule) -> Result<JsValue, JsValue> {
    rule.validate().map_err(to_js_err)?;
    to_js_value(&rule)
}

/// `RebalanceRule::Manual`: holdings change only on explicit `rebalance` calls.
/// @returns The `RebalanceRule` plain object.
/// @throws Error - Throws if the value cannot be converted to JavaScript.
#[wasm_bindgen(js_name = rebalanceRuleManual)]
pub fn rebalance_rule_manual() -> Result<JsValue, JsValue> {
    rebalance_rule(RebalanceRule::Manual)
}

/// `RebalanceRule::Dates`: rebalance on explicit dates.
/// @param dates - Strictly increasing ISO-8601 rebalance dates.
/// @returns The `RebalanceRule` plain object.
/// @throws Error - Throws with kind `validation` if a date is malformed or the dates are duplicated or unordered, and kind `invalid_type` if `dates` is not an array of strings.
#[wasm_bindgen(js_name = rebalanceRuleDates)]
pub fn rebalance_rule_dates(dates: JsValue) -> Result<JsValue, JsValue> {
    rebalance_rule(RebalanceRule::Dates {
        dates: arg::dates(&dates, "dates")?,
    })
}

/// `RebalanceRule::Calendar`: rebalance on a business-day adjusted schedule.
/// @param start - First rebalance date as an ISO-8601 string.
/// @param frequency - Rebalance cadence as a `Tenor` plain object, e.g. `{ count: 1, unit: "months" }`.
/// @param calendar_id - Registered holiday-calendar identifier, e.g. `"nyse"`.
/// @param business_day_convention - Business-day adjustment serde name, e.g. `"modified_following"`.
/// @param end - Optional last rebalance date as an ISO-8601 string; omit for an open-ended schedule.
/// @returns The `RebalanceRule` plain object.
/// @throws Error - Throws with kind `validation` if a date, the tenor or the convention is malformed, `end` precedes `start`, or the calendar is unknown; and kind `invalid_type` for a wrong argument type.
#[wasm_bindgen(js_name = rebalanceRuleCalendar)]
pub fn rebalance_rule_calendar(
    start: JsValue,
    frequency: JsValue,
    calendar_id: JsValue,
    business_day_convention: JsValue,
    end: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    rebalance_rule(RebalanceRule::Calendar {
        start: arg::date(&start, "start")?,
        end: crate::utils::input::js_opt_string(end.as_ref(), "end")?
            .map(|text| crate::utils::parse_iso_date(&text))
            .transpose()?,
        frequency: arg::json(&frequency, "frequency")?,
        calendar_id: js_string(&calendar_id, "calendarId")?,
        business_day_convention: arg::en(&business_day_convention, "businessDayConvention")?,
    })
}

/// The instrument of a composite leg as its canonical envelope.
///
/// Twin of Python `CompositeLegSpec.instrument_dict`.
/// @param leg - `CompositeLegSpec` plain object or JSON string.
/// @returns The leg's `finstack_quant.instrument/1` envelope as a plain object.
/// @throws Error - Throws with kind `validation` if `leg` does not match the `CompositeLegSpec` schema.
#[wasm_bindgen(js_name = compositeLegSpecInstrumentDict)]
pub fn composite_leg_spec_instrument_dict(leg: JsValue) -> Result<JsValue, JsValue> {
    let leg: CompositeLegSpec = from_js_json(&leg, "leg")?;
    to_js_value(&InstrumentEnvelope::new(*leg.instrument))
}

/// One row of a composite history as canonical JSON.
///
/// Twin of Python `CompositeHistoryResult.row_json`.
/// @param rows - `CompositeHistoryRow[]` returned by `history` or `historyFromSpec` (or its JSON).
/// @param index - Zero-based row index.
/// @returns Canonical `CompositeHistoryRow` JSON text.
/// @throws Error - Throws with kind `validation` if `rows` does not match the `CompositeHistoryRow` schema or `index` is out of range, and kind `invalid_type` if `index` is not a non-negative whole number.
#[wasm_bindgen(js_name = compositeHistoryResultRowJson)]
pub fn composite_history_result_row_json(rows: JsValue, index: JsValue) -> Result<String, JsValue> {
    let rows: Vec<CompositeHistoryRow> = from_js_json(&rows, "rows")?;
    let index: usize = arg::uint(&index, "index")?;
    let row = rows.get(index).ok_or_else(|| {
        to_js_err(finstack_quant_core::Error::Validation(format!(
            "history row index {index} is out of range"
        )))
    })?;
    serde_json::to_string(row).map_err(to_js_err)
}
