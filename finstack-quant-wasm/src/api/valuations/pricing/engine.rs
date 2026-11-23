//! Pricing, cashflow, and metric-introspection WASM implementations.

use super::super::results::valuation_result_value;
use crate::api::core::market_context::JsMarketContext;
use crate::utils::input::{
    js_opt_string, js_opt_string_seq, js_string, js_string_seq, json_text, opt_from_js_json,
    opt_json_text,
};
use crate::utils::{contract_to_js_err, to_js_err, to_js_value};
use finstack_quant_core::contract::LoadLimits;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::pricer::PriceExtras;
use finstack_quant_valuations::results::ValuationResult;
use wasm_bindgen::prelude::*;

/// Parse a market-context argument (JSON string or plain object).
pub(crate) fn parse_market(market_json: &JsValue) -> Result<MarketContext, JsValue> {
    parse_market_text(&json_text(market_json, "marketJson")?)
}

pub(crate) fn parse_market_text(market_json: &str) -> Result<MarketContext, JsValue> {
    MarketContext::from_state_slice(market_json.as_bytes(), &LoadLimits::default())
        .map(|(market, _)| market)
        .map_err(contract_to_js_err)
}

pub(crate) fn parse_pricing_instrument_json(
    instrument_json: &str,
    metric_pricing_overrides: Option<&str>,
) -> Result<finstack_quant_valuations::pricer::ParsedInstrument, JsValue> {
    finstack_quant_valuations::pricer::parse_boxed_instrument_from_json(
        instrument_json,
        metric_pricing_overrides,
        None,
    )
    .map_err(to_js_err)
}

pub(crate) fn price_result_with_context(
    instrument: &finstack_quant_valuations::pricer::ParsedInstrument,
    market: &MarketContext,
    as_of: &str,
    model: &str,
    metrics: Vec<String>,
    extras: &PriceExtras,
) -> Result<ValuationResult, JsValue> {
    let options =
        extras.pricing_options(finstack_quant_calibration::recalibration::pricing_options());
    finstack_quant_valuations::pricer::price_instrument(
        instrument, market, as_of, model, &metrics, None, options,
    )
    .map_err(to_js_err)
}

/// Validate a canonical v1 instrument envelope after optional metric-pricing
/// overrides are merged by the canonical pricing path.
///
/// Deserializes the input against the known instrument schema and
/// returns the canonical (re-serialized) JSON.
///
/// # Arguments
///
/// * `json` - Required `finstack_quant.instrument/1` envelope.
/// * `metric_pricing_overrides` - Serialized JSON object patch applied before native
///   validation. Only supplied fields replace stored overrides; `{}` preserves them.
///   Supplied `bump_config` fields merge into the stored object. Explicit `null`
///   clears nullable fields to their Rust fallback. Omitting the argument or
///   passing JavaScript `null`/`undefined` retains the envelope configuration.
///
/// # Errors
///
/// Throws a JavaScript exception if the instrument or override JSON is
/// malformed, the merged payload is not a canonical v1 instrument envelope,
/// instrument validation fails, or the envelope cannot be canonically
/// serialized.
/// @param json - Required `finstack_quant.instrument/1` envelope.
/// @param metricPricingOverrides - Serialized JSON object patch applied before native validation. Only supplied fields replace stored overrides; `{}` preserves them. Supplied `bump_config` fields merge into the stored object. Explicit `null` clears nullable fields to their Rust fallback. Omitting the argument or passing JavaScript `null`/`undefined` retains the envelope configuration.
/// @returns Canonical instrument envelope JSON after schema validation.
#[wasm_bindgen(js_name = validateInstrumentJson)]
pub fn validate_instrument_json(
    #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
    #[wasm_bindgen(
        js_name = "metricPricingOverrides",
        unchecked_optional_param_type = "JsonInput | null"
    )]
    metric_pricing_overrides: Option<JsValue>,
) -> Result<String, JsValue> {
    let json: &str = &json_text(&json, "json")?;
    let metric_pricing_overrides =
        opt_json_text(metric_pricing_overrides.as_ref(), "metricPricingOverrides")?;
    finstack_quant_valuations::pricer::validate_instrument_json(
        json,
        metric_pricing_overrides.as_deref(),
    )
    .map_err(to_js_err)
}

/// Price an instrument from its canonical envelope and return a ValuationResult object.
///
/// Returns a plain JavaScript object (`instrument_id`, `as_of`, `value`,
/// `measures`, `meta`, …) — the same document Python callers reach through
/// `ValuationResult`.
///
/// Omit `model` (or pass `"default"`) to use the instrument-native default
/// model — matching the Python binding's `model="default"` default.
/// For bonds, `"discounting"` is non-callable rates-only PV,
/// `"hazard_rate"` is non-callable fractional recovery of par, `"tree"`
/// values rates-only exercise rights, and `"rates_credit"` values joint
/// rates-credit bonds including call, put, and return floors. When stochastic
/// factors are configured under `"rates_credit"`, `result.details` is tagged
/// `{ type: "monte_carlo", data: ... }` and reports the standard error, path
/// counts, random seed, simulation time grid, and variance-reduction flags.
/// That seed is a lossless JavaScript `BigInt`; serialize stochastic results
/// with `valuationResultToJson`, since `JSON.stringify` throws on `BigInt`.
/// @param instrumentJson - Required `finstack_quant.instrument/1` envelope.
/// @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param model - Optional pricing-model identifier; omit for the instrument-native model.
/// @param metrics - Optional canonical metric IDs such as `"ytm"`, `"dv01"`,
/// `"hvar"`, or `"expected_shortfall"`. Omit, `null`, or `undefined` for a
/// valuation-only result. Mortgage OAS and CMO Z-spread consume clean prices
/// per 100 current face and include settlement accrued interest. MBS DV01,
/// bucketed DV01 and duration share rate-dependent prepayment assumptions.
/// FI TRS duration DV01 requires `duration_id` and a finite signed scalar in years.
/// Roll specialness is in basis points versus `repo_curve_id` (a discount curve),
/// or the discount curve when omitted; implied financing is an ACT/360 decimal.
/// @param extras - Optional `PriceExtras` (plain object or JSON text); omit,
/// `null`, or `undefined` for none. Keys, each optional:
/// `metric_pricing_overrides` - a `MetricPricingOverrides` JSON object patch
/// applied before validation; only supplied fields replace stored overrides
/// (`{}` preserves them), supplied `bump_config` fields merge into the stored
/// object, and an explicit `null` clears a nullable field to its Rust fallback.
/// Omitted keeps the envelope configuration.
/// `instrument_pricing_overrides` - a patch for instrument-owned market quotes
/// and model parameters. Omitted fields keep their stored values, and an
/// explicit `null` clears a nullable parameter for this pricing request.
/// `market_history` - a `MarketHistory` object (base date, window and daily
/// risk-factor shifts) required by historical risk metrics such as `"hvar"` and
/// `"expected_shortfall"`. Omitted prices without history.
/// `explain` - boolean, default `false`; `true` attaches a step trace as
/// `result.explanation`. Under `"discounting"` and `"hazard_rate"` it holds one
/// `cashflow_pv` entry per projected cashflow (date, amount, discount factor,
/// curve, survival probability, present value) summing to the price, capped at
/// 1000 entries; other models get one `computation_step` entry stating that no
/// per-flow decomposition exists.
/// @returns Plain JavaScript `ValuationResult` (`instrument_id`, `as_of`,
/// `value`, `measures`, `meta`, `provenance`, …). `provenance` records the
/// model, requested date, declared market dependencies, scenario adjustment and,
/// when metrics are requested, the finite-difference bump sizes.
///
/// # Errors
///
/// Throws a JavaScript exception if an instrument, market or `extras` payload is
/// invalid (including an unknown `extras` key); `metrics` is not a string array; `asOf`,
/// `model`, or a metric identifier is invalid; required market data is missing;
/// pricing or a metric calculation fails; or the valuation cannot be converted
/// to a JavaScript value.
#[wasm_bindgen(js_name = priceInstrument, unchecked_return_type = "ValuationResult")]
pub fn price_instrument(
    #[wasm_bindgen(js_name = "instrumentJson", unchecked_param_type = "JsonInput")] instrument_json: JsValue,
    #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
    market_json: JsValue,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "string | null")] model: Option<JsValue>,
    #[wasm_bindgen(unchecked_optional_param_type = "string[] | null")] metrics: Option<JsValue>,
    #[wasm_bindgen(
        unchecked_optional_param_type = "generated.valuations.PriceExtras | JsonInput | null"
    )]
    extras: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let instrument_json = json_text(&instrument_json, "instrumentJson")?;
    let request = PriceRequest::from_js(&as_of, model.as_ref(), metrics.as_ref(), extras.as_ref())?;
    // Parse the instrument before the market so instrument errors surface first.
    let instrument = request.instrument(&instrument_json)?;
    request.price(&instrument, &parse_market(&market_json)?)
}

/// The converted arguments shared by `priceInstrument` and the typed
/// instrument classes' `price` methods.
pub(crate) struct PriceRequest {
    /// ISO-8601 valuation date.
    pub(super) as_of: String,
    /// Pricing-model identifier; `None` selects the instrument default.
    pub(super) model: Option<String>,
    /// Canonical metric identifiers to compute.
    pub(super) metrics: Vec<String>,
    /// Optional request configuration, market history, and override patches.
    pub(super) extras: PriceExtras,
}

impl PriceRequest {
    /// Convert the JavaScript pricing arguments, rejecting wrong types.
    ///
    /// `extras` is read once as `PriceExtras`: its override patch is kept for
    /// instrument parsing, its history and `explain` flag for the price call.
    pub(crate) fn from_js(
        as_of: &JsValue,
        model: Option<&JsValue>,
        metrics: Option<&JsValue>,
        extras: Option<&JsValue>,
    ) -> Result<Self, JsValue> {
        let as_of = js_string(as_of, "asOf")?;
        let model = js_opt_string(model, "model")?;
        let metrics = js_opt_string_seq(metrics, "metrics")?.unwrap_or_default();
        let extras: PriceExtras = opt_from_js_json(extras, "extras")?.unwrap_or_default();
        Ok(Self {
            as_of,
            model,
            metrics,
            extras,
        })
    }

    /// Parse `instrument_json` with this request's metric-pricing overrides.
    pub(crate) fn instrument(
        &self,
        instrument_json: &str,
    ) -> Result<finstack_quant_valuations::pricer::ParsedInstrument, JsValue> {
        finstack_quant_valuations::pricer::parse_boxed_instrument_from_json(
            instrument_json,
            self.extras.metric_pricing_overrides_json().as_deref(),
            self.extras.instrument_pricing_overrides.as_ref(),
        )
        .map_err(to_js_err)
    }

    /// Prepare a native Rust instrument with this request's metric-pricing overrides.
    pub(crate) fn typed_instrument(
        &self,
        instrument: Box<dyn finstack_quant_valuations::instruments::Instrument>,
    ) -> Result<finstack_quant_valuations::pricer::ParsedInstrument, JsValue> {
        finstack_quant_valuations::pricer::ParsedInstrument::from_instrument(
            instrument,
            self.extras.metric_pricing_overrides_json().as_deref(),
            self.extras.instrument_pricing_overrides.as_ref(),
        )
        .map_err(to_js_err)
    }

    /// Price a prepared instrument against `market`.
    pub(crate) fn price(
        self,
        instrument: &finstack_quant_valuations::pricer::ParsedInstrument,
        market: &MarketContext,
    ) -> Result<JsValue, JsValue> {
        let result = price_result_with_context(
            instrument,
            market,
            &self.as_of,
            self.model.as_deref().unwrap_or("default"),
            self.metrics,
            &self.extras,
        )?;
        valuation_result_value(&result)
    }
}

/// Per-flow cashflow envelope for an instrument, as a plain object.
///
/// Returns the Rust `InstrumentCashflowEnvelope` as a structured value.
/// Hazard-rate export rejects bonds with call, put, or
/// return-floor rights because static rows cannot represent
/// exercise-contingent value.
/// @param instrumentJson - Required `finstack_quant.instrument/1` envelope.
/// @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param model - Must be `"discounting"` or `"hazard_rate"`; `"default"` is not accepted.
/// @returns `InstrumentCashflowEnvelope` with one row per flow and the total PV.
///
/// # Errors
///
/// Throws a JavaScript exception if the instrument or market JSON or `asOf` is
/// invalid, `model` is unsupported or incompatible with the instrument, a
/// bond with embedded exercise rights is requested under a static cashflow
/// model, required curves are missing, the schedule mixes currencies, or
/// canonical pricing fails.
#[wasm_bindgen(js_name = instrumentCashflows, unchecked_return_type = "generated.valuations.InstrumentCashflowEnvelope")]
pub fn instrument_cashflows(
    #[wasm_bindgen(js_name = "instrumentJson", unchecked_param_type = "JsonInput")] instrument_json: JsValue,
    #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
    market_json: JsValue,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] model: JsValue,
) -> Result<JsValue, JsValue> {
    let instrument_json: &str = &json_text(&instrument_json, "instrumentJson")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let model: &str = &js_string(&model, "model")?;
    // Parse the instrument before the market so instrument errors surface first.
    let instrument = parse_pricing_instrument_json(instrument_json, None)?;
    let market = parse_market(&market_json)?;
    to_js_value(
        &finstack_quant_valuations::instruments::cashflow_export::instrument_cashflows(
            instrument.as_instrument(),
            &market,
            as_of,
            model,
        )
        .map_err(to_js_err)?,
    )
}

/// List all metric IDs in the standard metric registry.
///
/// # Errors
///
/// Throws a JavaScript exception if the metric identifier list cannot be
/// converted to a JavaScript value.
/// @returns Canonical metric identifiers, sorted alphabetically.
#[wasm_bindgen(js_name = listStandardMetrics, unchecked_return_type = "string[]")]
pub fn list_standard_metrics() -> Result<JsValue, JsValue> {
    let ids = finstack_quant_valuations::pricer::list_standard_metrics();
    to_js_value(&ids)
}

/// List all standard metrics organized by group.
///
/// Returns a JSON object `{ group_name: [metric_id, ...], ... }` where
/// each key is a human-readable group name (e.g. "Pricing", "Greeks",
/// "Sensitivity") and the value is a sorted array of metric ID strings.
///
/// # Errors
///
/// Throws a JavaScript exception if the grouped metric registry cannot be
/// converted to a JavaScript value.
/// @returns Metric identifiers grouped by human-readable group name.
#[wasm_bindgen(js_name = listStandardMetricsGrouped, unchecked_return_type = "Record<string, string[]>")]
pub fn list_standard_metrics_grouped() -> Result<JsValue, JsValue> {
    let map = finstack_quant_valuations::pricer::list_standard_metrics_grouped();
    to_js_value(&map)
}

/// List every metric the standard registry holds for one instrument type,
/// grouped by display category.
///
/// The per-instrument counterpart to `listStandardMetricsGrouped`: each group
/// keeps only the metrics the standard registry applies to `instrumentType`,
/// and groups left empty are omitted. Applicable metrics outside every display
/// group, such as `all_in_rate` or `parity`, are listed under
/// `"Instrument Specific"`. A listed metric is registered for the type; whether
/// it computes under a given pricing model or market still depends on that
/// model and market.
///
/// # Arguments
///
/// * `instrument_type` - Canonical instrument-type tag, matched exactly in snake case (for example `"bond"`, `"fx_option"` or `"structured_credit"`); the same tag an instrument envelope carries in its `type` field.
/// @returns Applicable metric identifiers keyed by human-readable group name, each array sorted ascending.
/// @throws Error - Throws a validation error when `instrumentType` is not a known instrument type, or a `TypeError` when it is not a string.
/// @param instrumentType - Canonical instrument-type tag, matched exactly in snake case (for example `"bond"`, `"fx_option"` or `"structured_credit"`); the same tag an instrument envelope carries in its `type` field.
///
/// @example
/// ```typescript
/// import init, { valuations } from "finstack-quant-wasm";
/// await init();
/// const grouped = valuations.instruments.metricsForInstrument('structured_credit');
/// const offersWal = Object.values(grouped).some((ids) => ids.includes('wal'));
/// console.log(offersWal); // true
/// ```
#[wasm_bindgen(js_name = metricsForInstrument, unchecked_return_type = "Record<string, string[]>")]
pub fn metrics_for_instrument(
    #[wasm_bindgen(js_name = "instrumentType", unchecked_param_type = "string")]
    instrument_type: JsValue,
) -> Result<JsValue, JsValue> {
    let instrument_type = js_string(&instrument_type, "instrumentType")?;
    let map = finstack_quant_valuations::pricer::metrics_for_instrument(&instrument_type)
        .map_err(to_js_err)?;
    to_js_value(&map)
}

/// Describe canonical metric identifiers using Rust-owned units and coordinates.
///
/// # Arguments
///
/// * `keys` - Canonical scalar or qualified wire keys in output order. A custom
///   key has a unit only when Rust declares one; one the standard registry holds gets the group
///   `"Instrument Specific"`, any other gets no group; malformed or obsolete
///   encodings are rejected.
/// @returns Ordered metadata records preserving original keys, decoded components, the coordinates that order them, native unit families, display groups, and bucket eligibility.
/// @throws Error - Throws a validation error for malformed canonical metric keys or a serialization error when conversion fails.
/// @param keys - Canonical scalar or qualified wire keys in output order. A custom key has a unit only when Rust declares one; one the standard registry holds gets the group `"Instrument Specific"`, any other gets no group; malformed or obsolete encodings are rejected.
///
/// @example
/// ```typescript
/// import init, { valuations } from "finstack-quant-wasm";
/// await init();
/// const [ytm, bucket] = valuations.instruments.metricMetadata([
///   'ytm',
///   'bucketed_dv01::USD-OIS::10y',
/// ]);
/// console.log(ytm.unit, bucket.components, bucket.bucketed);
/// ```
#[wasm_bindgen(js_name = metricMetadata, unchecked_return_type = "MetricMetadata[]")]
pub fn metric_metadata(
    #[wasm_bindgen(unchecked_param_type = "string[]")] keys: JsValue,
) -> Result<JsValue, JsValue> {
    let keys = js_string_seq(&keys, "keys")?;
    let metadata = finstack_quant_valuations::pricer::metric_metadata(&keys).map_err(to_js_err)?;
    to_js_value(&metadata)
}

/// Wire names of the mutually exclusive price-driving market quotes.
///
/// Validation rejects more than one of these on a
/// `instrument_pricing_overrides.market_quotes` object, so a host editor can
/// clear the others when one is entered. Other quotes, such as
/// `implied_volatility`, are not in the set.
/// @returns Field names as they appear on the wire, in precedence order.
/// @throws Error - Throws a `computation` error only if the names cannot cross the boundary.
#[wasm_bindgen(js_name = priceDriverFields, unchecked_return_type = "string[]")]
pub fn price_driver_fields() -> Result<JsValue, JsValue> {
    to_js_value(&finstack_quant_valuations::instruments::pricing_overrides::MarketQuoteOverrides::price_driver_fields())
}

/// Calendar-year ladders of a cashflow envelope, one per row currency.
///
/// Free-function twin of Python `InstrumentCashflowEnvelope.calendar_year_ladder_by_currency`
/// (Rust `InstrumentCashflowEnvelope::calendar_year_ladder_by_currency`). Currencies never mix;
/// each ladder reads `native_pv`, the row-currency PV before FX conversion and any scenario price shock.
/// @param envelope - `InstrumentCashflowEnvelope` returned by `instrumentCashflows`, as an object or JSON.
/// @returns One `CurrencyLadder` per row currency, in first-appearance order of `flows`; rows ascend by year.
/// @throws Error - Throws a `validation` error if `envelope` is malformed or an amount or native PV is non-finite.
#[wasm_bindgen(js_name = calendarYearLadderByCurrency, unchecked_return_type = "CurrencyLadder[]")]
pub fn calendar_year_ladder_by_currency(
    #[wasm_bindgen(
        unchecked_param_type = "generated.valuations.InstrumentCashflowEnvelope | string"
    )]
    envelope: JsValue,
) -> Result<JsValue, JsValue> {
    let envelope: finstack_quant_valuations::instruments::cashflow_export::InstrumentCashflowEnvelope =
        crate::utils::input::from_js_json(&envelope, "envelope")?;
    to_js_value(
        &envelope
            .calendar_year_ladder_by_currency()
            .map_err(to_js_err)?,
    )
}

/// Units of the tranche break-even CDR, discount margin and OAS results.
///
/// Those solvers document their units in prose and do not state them on the wire.
/// Values are never rescaled.
/// @returns `TrancheSolveFieldUnits`: `breakeven_cdr` and `discount_margin`, each one `MetricFieldUnit`, and `oas`, one `MetricFieldUnit` per `OasResult` field in struct order.
#[wasm_bindgen(js_name = trancheSolveFieldUnits, unchecked_return_type = "TrancheSolveFieldUnits")]
pub fn tranche_solve_field_units() -> Result<JsValue, JsValue> {
    to_js_value(&finstack_quant_valuations::results::tranche_solve_field_units())
}

/// Units of the scalar `TrancheMetrics` fields, in struct order.
///
/// `TrancheMetrics` documents these units in prose and does not state them on
/// the wire. Values are never rescaled.
/// @param currency - ISO-4217 code of the result's `TrancheMetrics.currency`, in any ASCII case; it names the unit of `pv`, `accrued` and `cs01`.
/// @returns One `MetricFieldUnit` per scalar field: the wire field name, its unit text and the Rust field that documents it. `tranche_id`, `currency` and the `metrics` map are not listed.
/// @throws Error - Throws a `validation` error if `currency` is not a recognized ISO-4217 code.
#[wasm_bindgen(js_name = trancheMetricFieldUnits, unchecked_return_type = "MetricFieldUnit[]")]
pub fn tranche_metric_field_units(
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
) -> Result<JsValue, JsValue> {
    let currency = crate::utils::input::js_currency(&currency, "currency")?;
    to_js_value(&finstack_quant_valuations::results::tranche_metric_field_units(currency))
}

/// Units of the `ScenarioCell` fields, the one place a host reads them from.
///
/// Rates are decimals, the price is a clean percent of the tranche's current balance and
/// the writedown is in currency units. Values are never rescaled.
/// @param currency - ISO-4217 code of the result's `ScenarioTable.currency`, in any ASCII case; it names the unit of `writedown`.
/// @returns `ScenarioTableFieldUnits`: `axes`, one `MetricFieldUnit` per swept input (`cpr`, `cdr`, `severity`), and `measures`, one per reported measure (`price`, `wal`, `writedown`), each in struct order.
/// @throws Error - Throws a `validation` error if `currency` is not a recognized ISO-4217 code.
#[wasm_bindgen(js_name = scenarioTableFieldUnits, unchecked_return_type = "ScenarioTableFieldUnits")]
pub fn scenario_table_field_units(
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
) -> Result<JsValue, JsValue> {
    let currency = crate::utils::input::js_currency(&currency, "currency")?;
    to_js_value(&finstack_quant_valuations::results::scenario_table_field_units(currency))
}

/// Units of a structured-credit simulation's charted series, the one place a host reads them from.
///
/// Coverage ratios, triggers and cushions are in times covered, credit enhancement
/// is a decimal share of the collateral and excess spread an annual decimal of the opening pool
/// balance. Values are never rescaled.
/// @param currency - ISO-4217 code of the simulation's deal currency, in any ASCII case; it names the unit of `diverted_cash`.
/// @returns `SimulationFieldUnits`: `coverage_tests`, one `MetricFieldUnit` per numeric coverage-test field (`ratio`, `trigger_level`, `cushion`) in struct order, and one each for `diverted_cash`, `credit_enhancement` and `excess_spread`.
/// @throws Error - Throws a `validation` error if `currency` is not a recognized ISO-4217 code.
#[wasm_bindgen(js_name = simulationFieldUnits, unchecked_return_type = "SimulationFieldUnits")]
pub fn simulation_field_units(
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
) -> Result<JsValue, JsValue> {
    let currency = crate::utils::input::js_currency(&currency, "currency")?;
    to_js_value(&finstack_quant_valuations::results::simulation_field_units(
        currency,
    ))
}

/// Deal families `StructuredCredit.newStandard` accepts, the one place a host reads them from.
///
/// @returns `StandardDealFamily[]` in `abs`, `clo`, `cmbs`, `rmbs` order: `deal_type`, the `newStandard` argument; `profile_id`, the registry profile supplying the family's frequency and credit assumptions; and `example`, complete `StandardDealParams` that `newStandard` accepts for the family, with its id, tranche ids and `pool.deal_type` naming the family.
/// @throws Error - Throws a `validation` error if an example cannot be constructed, which its fixed inputs do not trigger.
#[wasm_bindgen(js_name = standardDealFamilies, unchecked_return_type = "StandardDealFamily[]")]
pub fn standard_deal_families() -> Result<JsValue, JsValue> {
    to_js_value(&finstack_quant_valuations::instruments::fixed_income::structured_credit::standard_deal_families().map_err(crate::utils::to_js_err)?)
}

/// Units of the `EquityMetrics` fields, the one place a host reads them from.
///
/// The IRR is an annual decimal, the MOIC a multiple of the invested amount, the NAV a percent of the
/// equity's current balance and cash-on-cash a decimal share of the invested amount. Values are never rescaled.
/// @param currency - ISO-4217 code of the result's `currency`, in any ASCII case; it names the unit of `invested`.
/// @returns `EquityMetricsFieldUnits`: one `MetricFieldUnit` each for `invested`, `irr`, `moic`, `nav_pct` and `cash_on_cash`, and `purchase_price` for the `purchasePricePct` argument of `equityMetrics`: percent of current balance, omitted means par.
/// @throws Error - Throws a `validation` error if `currency` is not a recognized ISO-4217 code.
#[wasm_bindgen(js_name = equityMetricsFieldUnits, unchecked_return_type = "EquityMetricsFieldUnits")]
pub fn equity_metrics_field_units(
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
) -> Result<JsValue, JsValue> {
    let currency = crate::utils::input::js_currency(&currency, "currency")?;
    to_js_value(&finstack_quant_valuations::results::equity_metrics_field_units(currency))
}

/// Units of the `BondQuoteSet` fields, in struct order, the one place a host reads them from.
///
/// The set mixes currency prices, a percent-of-par price and decimal yields and spreads. Each field with a metric states that metric's unit, and `clean_price_pct`, which has none, states its quote convention. Values are never rescaled.
/// @param currency - ISO-4217 code of the bond whose set is being labelled, in any ASCII case; it names the unit of `clean_price_currency` and `dirty_price_currency`.
/// @returns One `MetricFieldUnit` per `BondQuoteSet` field in struct order: the wire field name, its unit text and the Rust field the unit is documented on.
/// @throws Error - Throws a `validation` error if `currency` is not a recognized ISO-4217 code.
#[wasm_bindgen(js_name = bondQuoteSetFieldUnits, unchecked_return_type = "MetricFieldUnit[]")]
pub fn bond_quote_set_field_units(
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
) -> Result<JsValue, JsValue> {
    let currency = crate::utils::input::js_currency(&currency, "currency")?;
    to_js_value(&finstack_quant_valuations::results::bond_quote_set_field_units(currency))
}

/// Units of the `PoolReport` fields, the one place a host reads them from.
///
/// `PoolStats` mixes a decimal coupon, a basis-point spread and a percent-point
/// defaulted share. Values are never rescaled.
/// @param currency - ISO-4217 code of the pool's base currency, in any ASCII case; it names the unit of `total_balance`, `undrawn_commitment` and each row's `balance`.
/// @returns `PoolReportFieldUnits`: `total_balance` and `warf`, each one `MetricFieldUnit`; `stats`, one per `PoolStats` field in struct order; and `concentration`, one per numeric `PoolConcentrationEntry` field in struct order.
/// @throws Error - Throws a `validation` error if `currency` is not a recognized ISO-4217 code.
#[wasm_bindgen(js_name = poolReportFieldUnits, unchecked_return_type = "PoolReportFieldUnits")]
pub fn pool_report_field_units(
    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
) -> Result<JsValue, JsValue> {
    let currency = crate::utils::input::js_currency(&currency, "currency")?;
    to_js_value(&finstack_quant_valuations::results::pool_report_field_units(currency))
}

/// List every pricing model key registered in the standard pricer registry.
///
/// The list is registry-derived rather than enum-derived, so it reflects real
/// dispatch coverage: a model with no registered pricer is omitted. Returns a
/// sorted array of canonical keys (`"discounting"`, `"rates_credit"`, …)
/// accepted by the `model` argument of `priceInstrument`.
///
/// # Errors
///
/// Throws a JavaScript exception if the model key list cannot be converted to
/// a JavaScript value.
/// @returns Returns the resulting `string[]` collection in ascending model-key order.
#[wasm_bindgen(js_name = listModels, unchecked_return_type = "string[]")]
pub fn list_models() -> Result<JsValue, JsValue> {
    let models = finstack_quant_valuations::pricer::list_models();
    to_js_value(&models)
}

/// List the standard registry's pricing models grouped by instrument type.
///
/// Returns a JSON object `{ instrument_type: [model_key, ...], ... }`. Only
/// instrument types with at least one registered pricer appear, and each entry
/// lists only the models that can actually price that instrument. The
/// `"bond"` entry includes `"discounting"`, `"hazard_rate"`, `"tree"`, and
/// `"rates_credit"`.
///
/// # Errors
///
/// Throws a JavaScript exception if the grouped model registry cannot be
/// converted to a JavaScript value.
/// @returns Returns the resulting `Record<string, string[]>` value keyed by instrument type.
#[wasm_bindgen(js_name = listModelsGrouped, unchecked_return_type = "Record<string, string[]>")]
pub fn list_models_grouped() -> Result<JsValue, JsValue> {
    let grouped = finstack_quant_valuations::pricer::list_models_grouped();
    to_js_value(&grouped)
}

// MarketContext-handle overloads — parse market once, reuse across pricing calls

/// Price an instrument using a pre-parsed `MarketContext` handle.
///
/// Avoids the per-call market-parse overhead of `priceInstrument`. Returns the
/// same plain JavaScript ValuationResult object. For bonds, `"discounting"`
/// is non-callable rates-only PV, `"hazard_rate"` is non-callable fractional
/// recovery of par, `"tree"` values rates-only exercise rights, and
/// `"rates_credit"` values joint rates-credit bonds including call, put, and
/// return floors. Stochastic `"rates_credit"` runs add tagged Monte Carlo diagnostics to
/// `result.details`. Their `seed` is a lossless JavaScript `BigInt`; serialize
/// such results with `valuationResultToJson`.
/// @param instrumentJson - Canonical instrument envelope JSON in the Finstack v1 schema.
/// @param market - Pre-parsed `core.MarketContext` handle supplying curves, quotes, and FX data for this call.
/// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param model - Optional pricing-model identifier; omit, `null`, or `"default"` for the instrument-native model.
/// @param metrics - Optional canonical metric IDs such as `"ytm"`, `"dv01"`,
/// `"hvar"`, or `"expected_shortfall"`. Omit, `null`, or `undefined` for a
/// valuation-only result.
/// @param extras - Optional `PriceExtras` (plain object or JSON text); omit,
/// `null`, or `undefined` for none. Keys, each optional:
/// `metric_pricing_overrides` - a `MetricPricingOverrides` JSON object patch
/// applied before validation; only supplied fields replace stored overrides
/// (`{}` preserves them), supplied `bump_config` fields merge into the stored
/// object, and an explicit `null` clears a nullable field to its Rust fallback.
/// Omitted keeps the envelope configuration.
/// `instrument_pricing_overrides` - a patch for instrument-owned market quotes
/// and model parameters. Omitted fields keep their stored values, and an
/// explicit `null` clears a nullable parameter for this pricing request.
/// `market_history` - a `MarketHistory` object (base date, window and daily
/// risk-factor shifts) required by historical risk metrics such as `"hvar"` and
/// `"expected_shortfall"`. Omitted prices without history.
/// `explain` - boolean, default `false`; `true` attaches a step trace as
/// `result.explanation`. Under `"discounting"` and `"hazard_rate"` it holds one
/// `cashflow_pv` entry per projected cashflow (date, amount, discount factor,
/// curve, survival probability, present value) summing to the price, capped at
/// 1000 entries; other models get one `computation_step` entry stating that no
/// per-flow decomposition exists.
/// @returns Plain JavaScript `ValuationResult` (`instrument_id`, `as_of`,
/// `value`, `measures`, `meta`, `provenance`, …). `provenance` records the
/// model, requested date, declared market dependencies, scenario adjustment and,
/// when metrics are requested, the finite-difference bump sizes.
///
/// # Errors
///
/// Throws a JavaScript exception if an instrument or `extras` payload is invalid
/// (including an unknown `extras` key); `metrics` is not a string array; `asOf`, `model`,
/// or a metric identifier is invalid; required market data is missing; pricing
/// or a metric calculation fails; or the valuation cannot be converted to a
/// JavaScript value.
#[wasm_bindgen(js_name = priceInstrumentWithMarket, unchecked_return_type = "ValuationResult")]
pub fn price_instrument_with_market(
    #[wasm_bindgen(js_name = "instrumentJson", unchecked_param_type = "JsonInput")] instrument_json: JsValue,
    market: &JsMarketContext,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "string | null")] model: Option<JsValue>,
    #[wasm_bindgen(unchecked_optional_param_type = "string[] | null")] metrics: Option<JsValue>,
    #[wasm_bindgen(
        unchecked_optional_param_type = "generated.valuations.PriceExtras | JsonInput | null"
    )]
    extras: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let instrument_json = json_text(&instrument_json, "instrumentJson")?;
    let request = PriceRequest::from_js(&as_of, model.as_ref(), metrics.as_ref(), extras.as_ref())?;
    let instrument = request.instrument(&instrument_json)?;
    request.price(&instrument, market.inner())
}

/// Per-flow cashflow envelope using a pre-parsed `MarketContext` handle, as a
/// plain object.
///
/// Hazard-rate export
/// rejects bonds with call, put, or return-floor rights because static rows
/// cannot represent exercise-contingent value.
/// @param instrumentJson - Canonical instrument envelope JSON in the Finstack v1 schema.
/// @param market - Pre-parsed `core.MarketContext` handle supplying curves, quotes, and FX data.
/// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param model - Must be `"discounting"` or `"hazard_rate"`; `"default"` is not accepted.
/// @returns `InstrumentCashflowEnvelope` with one row per flow and the total PV.
///
/// # Errors
///
/// Throws a JavaScript exception if `instrumentJson` or `asOf` is invalid,
/// `model` is unsupported or incompatible with the instrument, a bond with
/// embedded exercise rights is requested under a static cashflow model,
/// required curves are missing, the schedule mixes currencies, or canonical
/// pricing fails.
#[wasm_bindgen(js_name = instrumentCashflowsWithMarket, unchecked_return_type = "generated.valuations.InstrumentCashflowEnvelope")]
pub fn instrument_cashflows_with_market(
    #[wasm_bindgen(js_name = "instrumentJson", unchecked_param_type = "JsonInput")] instrument_json: JsValue,
    market: &JsMarketContext,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] model: JsValue,
) -> Result<JsValue, JsValue> {
    let instrument_json: &str = &json_text(&instrument_json, "instrumentJson")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let model: &str = &js_string(&model, "model")?;
    let instrument = parse_pricing_instrument_json(instrument_json, None)?;
    to_js_value(
        &finstack_quant_valuations::instruments::cashflow_export::instrument_cashflows(
            instrument.as_instrument(),
            market.inner(),
            as_of,
            model,
        )
        .map_err(to_js_err)?,
    )
}
