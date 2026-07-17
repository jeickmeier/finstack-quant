//! WASM bindings for instrument pricing and metric introspection.
//!
//! Structural credit-model factories (Merton, CreditGrades, dynamic recovery,
//! endogenous hazard, toggle exercise) live in [`super::credit`]. CDS-family
//! example payloads live in [`super::credit_derivatives`]. Both mirror the
//! Python binding layout; the exported JS surface is unchanged.
//!
//! # Monte-Carlo determinism
//!
//! `priceInstrument` / `priceInstrumentWithMetrics` (and their `Market`
//! variants) accept Monte-Carlo models (e.g. `monte_carlo_gbm`,
//! `monte_carlo_hull_white_1f`). These bindings deliberately expose **no**
//! explicit RNG-seed parameter: the seed is part of the *instrument*
//! contract, not the pricing call.
//!
//! The determinism guarantee is provided by the Rust core, not by these
//! wrappers: when an instrument's `metric_pricing_overrides.mc_seed_scenario`
//! is `None`, the core MC pricers derive a **stable** seed deterministically
//! from the instrument ID (see
//! `finstack_quant_valuations::instruments::InstrumentPricingOverrides`). Repricing the same
//! instrument JSON therefore yields bit-identical results without the caller
//! supplying a seed. Callers who need a distinct deterministic stream set
//! `mc_seed_scenario` inside the instrument JSON. This contract is verified by
//! `tests::price_instrument_mc_is_deterministic_without_explicit_seed`.

use super::market_handle::Market;
use crate::utils::{to_js_err, to_js_error, to_js_value};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::results::ValuationResult;
use wasm_bindgen::prelude::*;

pub(super) fn parse_market_json(market_json: &str) -> Result<MarketContext, JsValue> {
    serde_json::from_str(market_json).map_err(to_js_err)
}

pub(super) fn validate_pricing_instrument_json(
    instrument_json: &str,
    pricing_options: Option<&str>,
) -> Result<(), JsValue> {
    finstack_quant_valuations::pricer::parse_boxed_instrument_json(instrument_json, pricing_options)
        .map(drop)
        .map_err(|e| to_js_error(&e))
}

pub(super) fn valuation_result_json(result: ValuationResult) -> Result<String, JsValue> {
    serde_json::to_string(&result).map_err(to_js_err)
}

pub(super) fn price_instrument_with_context(
    instrument_json: &str,
    market: &MarketContext,
    as_of: &str,
    model: &str,
) -> Result<String, JsValue> {
    let result = finstack_quant_valuations::pricer::price_instrument_json(
        instrument_json,
        market,
        as_of,
        model,
    )
    .map_err(|e| to_js_error(&e))?;
    valuation_result_json(result)
}

pub(super) fn price_instrument_with_metrics_context(
    instrument_json: &str,
    market: &MarketContext,
    as_of: &str,
    model: &str,
    metrics: Vec<String>,
    pricing_options: Option<&str>,
    market_history_json: Option<&str>,
) -> Result<String, JsValue> {
    let result = finstack_quant_valuations::pricer::price_instrument_json_with_metrics_and_history(
        instrument_json,
        market,
        as_of,
        model,
        &metrics,
        pricing_options,
        market_history_json,
    )
    .map_err(|e| to_js_error(&e))?;
    valuation_result_json(result)
}

pub(super) fn metric_value_with_context(
    instrument_json: &str,
    market: &MarketContext,
    as_of: &str,
    model: &str,
    metric: &str,
) -> Result<f64, JsValue> {
    finstack_quant_valuations::pricer::metric_value_from_instrument_json(
        instrument_json,
        market,
        as_of,
        model,
        metric,
    )
    .map_err(to_js_err)
}

pub(super) fn standard_option_greeks_with_context(
    instrument_json: &str,
    market: &MarketContext,
    as_of: &str,
    model: &str,
) -> Result<Vec<(&'static str, f64)>, JsValue> {
    finstack_quant_valuations::pricer::present_standard_option_greeks_from_instrument_json(
        instrument_json,
        market,
        as_of,
        model,
    )
    .map_err(to_js_err)
}

/// Deserialize a `ValuationResult` from JSON and return the canonical JSON.
///
/// Validates the input conforms to the `ValuationResult` schema.
/// @param json - Canonical JSON string defining the object to deserialize or normalize.
#[wasm_bindgen(js_name = validateValuationResultJson)]
pub fn validate_valuation_result_json(json: &str) -> Result<String, JsValue> {
    let result: ValuationResult = serde_json::from_str(json).map_err(to_js_err)?;
    valuation_result_json(result)
}

/// Validate a tagged instrument JSON string.
///
/// Deserializes the input against the known instrument schema and
/// returns the canonical (re-serialized) JSON.
/// @param json - Canonical JSON string defining the object to deserialize or normalize.
#[wasm_bindgen(js_name = validateInstrumentJson)]
pub fn validate_instrument_json(json: &str) -> Result<String, JsValue> {
    finstack_quant_valuations::pricer::validate_instrument_json(json).map_err(|e| to_js_error(&e))
}

/// Construct tagged bond instrument JSON from a cashflow schedule.
/// @param instrument_id - Stable instrument identifier used for pricing and metric keys.
/// @param schedule_json - Canonical cashflow-schedule JSON used to construct the fixed-income instrument.
/// @param discount_curve_id - Market-context discount-curve identifier for the instrument currency.
/// @param quoted_clean - Optional observed clean bond price in the schedule's documented price quotation convention.
#[wasm_bindgen(js_name = bondFromCashflowsJson)]
pub fn bond_from_cashflows_json(
    instrument_id: &str,
    schedule_json: &str,
    discount_curve_id: &str,
    quoted_clean: Option<f64>,
) -> Result<String, JsValue> {
    finstack_quant_valuations::instruments::fixed_income::bond::bond_from_cashflows_json(
        instrument_id,
        schedule_json,
        discount_curve_id,
        quoted_clean,
    )
    .map_err(to_js_err)
}

/// Price an instrument from its tagged JSON and return a ValuationResult JSON.
///
/// Pass `model = "default"` to use the instrument-native default model.
/// @param instrument_json - Canonical JSON payload representing the instrument consumed by this API.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param model - Pricing-model identifier; use `"default"` for the instrument-native model when supported.
#[wasm_bindgen(js_name = priceInstrument)]
pub fn price_instrument(
    instrument_json: &str,
    market_json: &str,
    as_of: &str,
    model: &str,
) -> Result<String, JsValue> {
    validate_pricing_instrument_json(instrument_json, None)?;
    let market = parse_market_json(market_json)?;
    price_instrument_with_context(instrument_json, &market, as_of, model)
}

/// Price an instrument with explicit metric requests.
///
/// Pass `model = "default"` to use the instrument-native default model.
/// @param instrument_json - Canonical JSON payload representing the instrument consumed by this API.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param model - Pricing-model identifier; use `"default"` for the instrument-native model when supported.
/// @param metrics - Array of canonical metric identifiers to calculate with the instrument price.
/// @param pricing_options - Optional JSON pricing overrides accepted by the canonical instrument validator.
/// @param market_history - Optional serialized historical market snapshots required by historical pricing models.
#[wasm_bindgen(js_name = priceInstrumentWithMetrics)]
pub fn price_instrument_with_metrics(
    instrument_json: &str,
    market_json: &str,
    as_of: &str,
    model: &str,
    metrics: JsValue,
    pricing_options: Option<String>,
    market_history: Option<String>,
) -> Result<String, JsValue> {
    validate_pricing_instrument_json(instrument_json, pricing_options.as_deref())?;
    let market = parse_market_json(market_json)?;
    let metric_strs: Vec<String> = serde_wasm_bindgen::from_value(metrics).map_err(to_js_err)?;
    price_instrument_with_metrics_context(
        instrument_json,
        &market,
        as_of,
        model,
        metric_strs,
        pricing_options.as_deref(),
        market_history.as_deref(),
    )
}

/// Per-flow cashflow envelope (DF / survival / PV) for a discountable instrument.
///
/// `model` must be `"discounting"` or `"hazard_rate"`. Unsupported models or
/// incompatible instrument types throw. For supported pairs, the envelope's
/// `total_pv` matches the instrument's `base_value` within rounding.
/// @param instrument_json - Canonical JSON payload representing the instrument consumed by this API.
/// @param market_json - Canonical market-context JSON supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param model - Pricing-model identifier; use `"default"` for the instrument-native model when supported.
#[wasm_bindgen(js_name = instrumentCashflowsJson)]
pub fn instrument_cashflows_json(
    instrument_json: &str,
    market_json: &str,
    as_of: &str,
    model: &str,
) -> Result<String, JsValue> {
    validate_pricing_instrument_json(instrument_json, None)?;
    let market = parse_market_json(market_json)?;
    finstack_quant_valuations::pricer::instrument_cashflows_json(
        instrument_json,
        &market,
        as_of,
        model,
    )
    .map_err(|e| to_js_error(&e))
}

/// List all metric IDs in the standard metric registry.
#[wasm_bindgen(js_name = listStandardMetrics)]
pub fn list_standard_metrics() -> Result<JsValue, JsValue> {
    let ids = finstack_quant_valuations::pricer::list_standard_metrics();
    serde_wasm_bindgen::to_value(&ids).map_err(to_js_err)
}

/// List all standard metrics organized by group.
///
/// Returns a JSON object `{ group_name: [metric_id, ...], ... }` where
/// each key is a human-readable group name (e.g. "Pricing", "Greeks",
/// "Sensitivity") and the value is a sorted array of metric ID strings.
#[wasm_bindgen(js_name = listStandardMetricsGrouped)]
pub fn list_standard_metrics_grouped() -> Result<JsValue, JsValue> {
    let map = finstack_quant_valuations::pricer::list_standard_metrics_grouped();
    to_js_value(&map)
}

// ---------------------------------------------------------------------------
// Market overloads — parse market once, reuse across pricing calls
// ---------------------------------------------------------------------------

/// Price an instrument using a pre-parsed [`Market`].
///
/// Avoids the per-call market-parse overhead of `priceInstrument`.
/// @param instrument_json - Canonical JSON payload representing the instrument consumed by this API.
/// @param market - Market context or JSON payload supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param model - Pricing-model identifier; use `"default"` for the instrument-native model when supported.
#[wasm_bindgen(js_name = priceInstrumentWithMarket)]
pub fn price_instrument_with_market(
    instrument_json: &str,
    market: &Market,
    as_of: &str,
    model: &str,
) -> Result<String, JsValue> {
    validate_pricing_instrument_json(instrument_json, None)?;
    price_instrument_with_context(instrument_json, market.inner(), as_of, model)
}

/// Price an instrument with explicit metric requests using a pre-parsed [`Market`].
/// @param instrument_json - Canonical JSON payload representing the instrument consumed by this API.
/// @param market - Market context or JSON payload supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param model - Pricing-model identifier; use `"default"` for the instrument-native model when supported.
/// @param metrics - Array of canonical metric identifiers to calculate with the instrument price.
/// @param pricing_options - Optional JSON pricing overrides accepted by the canonical instrument validator.
/// @param market_history - Optional serialized historical market snapshots required by historical pricing models.
#[wasm_bindgen(js_name = priceInstrumentWithMetricsAndMarket)]
pub fn price_instrument_with_metrics_and_market(
    instrument_json: &str,
    market: &Market,
    as_of: &str,
    model: &str,
    metrics: JsValue,
    pricing_options: Option<String>,
    market_history: Option<String>,
) -> Result<String, JsValue> {
    validate_pricing_instrument_json(instrument_json, pricing_options.as_deref())?;
    let metric_strs: Vec<String> = serde_wasm_bindgen::from_value(metrics).map_err(to_js_err)?;
    price_instrument_with_metrics_context(
        instrument_json,
        market.inner(),
        as_of,
        model,
        metric_strs,
        pricing_options.as_deref(),
        market_history.as_deref(),
    )
}

/// Per-flow cashflow envelope using a pre-parsed [`Market`].
/// @param instrument_json - Canonical JSON payload representing the instrument consumed by this API.
/// @param market - Market context or JSON payload supplying curves, quotes, and FX data.
/// @param as_of - ISO-8601 valuation date used to resolve date-dependent market data.
/// @param model - Pricing-model identifier; use `"default"` for the instrument-native model when supported.
#[wasm_bindgen(js_name = instrumentCashflowsWithMarket)]
pub fn instrument_cashflows_with_market(
    instrument_json: &str,
    market: &Market,
    as_of: &str,
    model: &str,
) -> Result<String, JsValue> {
    validate_pricing_instrument_json(instrument_json, None)?;
    finstack_quant_valuations::pricer::instrument_cashflows_json(
        instrument_json,
        market.inner(),
        as_of,
        model,
    )
    .map_err(|e| to_js_error(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_model_key_recognizes_standard_keys() {
        assert_eq!(
            finstack_quant_valuations::pricer::parse_model_key("discounting").expect("ok"),
            finstack_quant_valuations::pricer::ModelKey::Discounting
        );
        assert_eq!(
            finstack_quant_valuations::pricer::parse_model_key("tree").expect("ok"),
            finstack_quant_valuations::pricer::ModelKey::Tree
        );
        assert_eq!(
            finstack_quant_valuations::pricer::parse_model_key("black76").expect("ok"),
            finstack_quant_valuations::pricer::ModelKey::Black76
        );
        assert_eq!(
            finstack_quant_valuations::pricer::parse_model_key("hull_white_1f").expect("ok"),
            finstack_quant_valuations::pricer::ModelKey::HullWhite1F
        );
        assert_eq!(
            finstack_quant_valuations::pricer::parse_model_key("hazard_rate").expect("ok"),
            finstack_quant_valuations::pricer::ModelKey::HazardRate
        );
        assert_eq!(
            finstack_quant_valuations::pricer::parse_model_key("normal").expect("ok"),
            finstack_quant_valuations::pricer::ModelKey::Normal
        );
        assert_eq!(
            finstack_quant_valuations::pricer::parse_model_key("monte_carlo_gbm").expect("ok"),
            finstack_quant_valuations::pricer::ModelKey::MonteCarloGBM
        );
    }

    pub(crate) fn bond_instrument_json() -> String {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::money::Money;
        use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
        use finstack_quant_valuations::instruments::InstrumentJson;

        let bond = Bond::fixed(
            "TEST-BOND",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            time::Date::from_calendar_date(2024, time::Month::January, 1).expect("date"),
            time::Date::from_calendar_date(2034, time::Month::January, 1).expect("date"),
            "USD-OIS",
        )
        .expect("bond");
        serde_json::to_string(&InstrumentJson::Bond(bond)).expect("serialize")
    }

    pub(crate) fn bermudan_swaption_json() -> String {
        use finstack_quant_valuations::instruments::rates::swaption::BermudanSwaption;
        use finstack_quant_valuations::instruments::InstrumentJson;

        serde_json::to_string(&InstrumentJson::BermudanSwaption(
            BermudanSwaption::example(),
        ))
        .expect("serialize")
    }

    pub(crate) fn tarn_json() -> String {
        use finstack_quant_core::dates::{Date, DayCount, Tenor};
        use finstack_quant_core::money::Money;
        use finstack_quant_core::types::{CurveId, InstrumentId};
        use finstack_quant_valuations::instruments::rates::tarn::Tarn;
        use finstack_quant_valuations::instruments::{InstrumentJson, InstrumentPricingOverrides};
        use time::Month;

        let mut instrument_pricing_overrides = InstrumentPricingOverrides::default();
        instrument_pricing_overrides.model_config.mc_paths = Some(32);
        instrument_pricing_overrides
            .model_config
            .hw1f_mean_reversion = Some(0.05);
        instrument_pricing_overrides.model_config.hw1f_sigma = Some(1e-12);

        let tarn = Tarn {
            id: InstrumentId::new("TARN-WASM-E2E"),
            fixed_rate: 0.06,
            coupon_floor: 0.0,
            target_coupon: 1.0,
            notional: Money::new(1_000_000.0, finstack_quant_core::currency::Currency::USD),
            coupon_dates: vec![
                Date::from_calendar_date(2025, Month::January, 1).expect("date"),
                Date::from_calendar_date(2025, Month::July, 1).expect("date"),
                Date::from_calendar_date(2026, Month::January, 1).expect("date"),
                Date::from_calendar_date(2026, Month::July, 1).expect("date"),
            ],
            floating_tenor: Tenor::semi_annual(),
            floating_index_id: CurveId::new("USD-SOFR-6M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            vol_surface_id: Some(CurveId::new("USD-SOFR-HW-VOL")),
            day_count: DayCount::Act365F,
            instrument_pricing_overrides,
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
        };
        serde_json::to_string(&InstrumentJson::Tarn(tarn)).expect("serialize")
    }

    pub(crate) fn snowball_json() -> String {
        use finstack_quant_core::dates::{Date, DayCount, Tenor};
        use finstack_quant_core::money::Money;
        use finstack_quant_core::types::{CurveId, InstrumentId};
        use finstack_quant_valuations::instruments::rates::snowball::{Snowball, SnowballVariant};
        use finstack_quant_valuations::instruments::{InstrumentJson, InstrumentPricingOverrides};
        use time::Month;

        let mut instrument_pricing_overrides = InstrumentPricingOverrides::default();
        instrument_pricing_overrides.model_config.mc_paths = Some(32);
        instrument_pricing_overrides
            .model_config
            .hw1f_mean_reversion = Some(0.05);
        instrument_pricing_overrides.model_config.hw1f_sigma = Some(1e-12);

        let snowball = Snowball {
            id: InstrumentId::new("SNOWBALL-WASM-E2E"),
            variant: SnowballVariant::Snowball,
            initial_coupon: 0.03,
            fixed_rate: 0.05,
            leverage: 1.0,
            coupon_floor: 0.0,
            coupon_cap: None,
            notional: Money::new(1_000_000.0, finstack_quant_core::currency::Currency::USD),
            coupon_dates: vec![
                Date::from_calendar_date(2025, Month::January, 1).expect("date"),
                Date::from_calendar_date(2025, Month::July, 1).expect("date"),
                Date::from_calendar_date(2026, Month::January, 1).expect("date"),
                Date::from_calendar_date(2026, Month::July, 1).expect("date"),
            ],
            floating_index_id: CurveId::new("USD-SOFR-6M"),
            floating_tenor: Tenor::semi_annual(),
            discount_curve_id: CurveId::new("USD-OIS"),
            vol_surface_id: Some(CurveId::new("USD-SOFR-HW-VOL")),
            callable: None,
            day_count: DayCount::Act365F,
            instrument_pricing_overrides,
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
        };
        serde_json::to_string(&InstrumentJson::Snowball(snowball)).expect("serialize")
    }

    pub(crate) fn inverse_floater_json() -> String {
        use finstack_quant_core::dates::{Date, DayCount, Tenor};
        use finstack_quant_core::money::Money;
        use finstack_quant_core::types::{CurveId, InstrumentId};
        use finstack_quant_valuations::instruments::rates::snowball::{Snowball, SnowballVariant};
        use finstack_quant_valuations::instruments::{InstrumentJson, InstrumentPricingOverrides};
        use time::Month;

        let inverse_floater = Snowball {
            id: InstrumentId::new("INV-FLOATER-WASM-E2E"),
            variant: SnowballVariant::InverseFloater,
            initial_coupon: 0.0,
            fixed_rate: 0.08,
            leverage: 1.5,
            coupon_floor: 0.0,
            coupon_cap: Some(0.10),
            notional: Money::new(500_000.0, finstack_quant_core::currency::Currency::USD),
            coupon_dates: vec![
                Date::from_calendar_date(2025, Month::January, 1).expect("date"),
                Date::from_calendar_date(2025, Month::July, 1).expect("date"),
                Date::from_calendar_date(2026, Month::January, 1).expect("date"),
                Date::from_calendar_date(2026, Month::July, 1).expect("date"),
            ],
            floating_index_id: CurveId::new("USD-SOFR-6M"),
            floating_tenor: Tenor::semi_annual(),
            discount_curve_id: CurveId::new("USD-OIS"),
            vol_surface_id: Some(CurveId::new("USD-SOFR-HW-VOL")),
            callable: None,
            day_count: DayCount::Act365F,
            instrument_pricing_overrides: InstrumentPricingOverrides::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
        };
        serde_json::to_string(&InstrumentJson::Snowball(inverse_floater)).expect("serialize")
    }

    pub(crate) fn callable_range_accrual_json() -> String {
        use finstack_quant_core::dates::{Date, DayCount};
        use finstack_quant_core::money::Money;
        use finstack_quant_core::types::{CurveId, InstrumentId};
        use finstack_quant_valuations::instruments::rates::callable_range_accrual::CallableRangeAccrual;
        use finstack_quant_valuations::instruments::rates::exotics_shared::bermudan_call::BermudanCallProvision;
        use finstack_quant_valuations::instruments::rates::range_accrual::{
            BoundsType, RangeAccrual,
        };
        use finstack_quant_valuations::instruments::{InstrumentJson, InstrumentPricingOverrides};
        use time::Month;

        let mut instrument_pricing_overrides = InstrumentPricingOverrides::default();
        instrument_pricing_overrides.model_config.mc_paths = Some(8);
        instrument_pricing_overrides
            .model_config
            .hw1f_mean_reversion = Some(0.05);
        instrument_pricing_overrides.model_config.hw1f_sigma = Some(1e-12);

        let range_accrual = RangeAccrual::builder()
            .id(InstrumentId::new("RA-WASM-E2E"))
            .underlying_ticker("SOFR".to_string())
            .observation_dates(vec![
                Date::from_calendar_date(2025, Month::July, 1).expect("date"),
                Date::from_calendar_date(2026, Month::January, 1).expect("date"),
                Date::from_calendar_date(2026, Month::July, 1).expect("date"),
            ])
            .lower_bound(0.02)
            .upper_bound(0.04)
            .bounds_type(BoundsType::Absolute)
            .coupon_rate(0.06)
            .notional(Money::new(
                1_000_000.0,
                finstack_quant_core::currency::Currency::USD,
            ))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .accrual_start_date_opt(Some(
                Date::from_calendar_date(2025, Month::January, 1).expect("date"),
            ))
            .rate_index_id_opt(Some("SOFR".into()))
            .projection_curve_id_opt(Some(CurveId::new("USD-OIS")))
            .reference_tenor_opt(Some(finstack_quant_core::dates::Tenor::new(
                6,
                finstack_quant_core::dates::TenorUnit::Months,
            )))
            .spot_id("SOFR-RATE".into())
            .vol_surface_id(CurveId::new("SOFR-VOL"))
            .div_yield_id_opt(None)
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
            .attributes(Default::default())
            .payment_date_opt(None)
            .past_fixings_in_range_opt(None)
            .total_past_observations_opt(None)
            .build()
            .expect("range accrual");

        let callable = CallableRangeAccrual {
            id: InstrumentId::new("CALLABLE-RA-WASM-E2E"),
            range_accrual,
            call_provision: BermudanCallProvision::new(
                vec![Date::from_calendar_date(2025, Month::July, 1).expect("date")],
                1.0,
                0,
            ),
            instrument_pricing_overrides,
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
        };
        serde_json::to_string(&InstrumentJson::CallableRangeAccrual(Box::new(callable)))
            .expect("serialize")
    }

    pub(crate) fn cms_spread_option_json() -> String {
        use finstack_quant_core::dates::{Date, DayCount, Tenor, TenorUnit};
        use finstack_quant_core::money::Money;
        use finstack_quant_core::types::{CurveId, InstrumentId};
        use finstack_quant_valuations::instruments::rates::cms_spread_option::{
            CmsSpreadOption, CmsSpreadOptionType,
        };
        use finstack_quant_valuations::instruments::{InstrumentJson, InstrumentPricingOverrides};
        use time::Month;

        let option = CmsSpreadOption {
            id: InstrumentId::new("CMS-SPREAD-WASM-E2E"),
            long_cms_tenor: Tenor::new(10, TenorUnit::Years),
            short_cms_tenor: Tenor::new(2, TenorUnit::Years),
            strike: 0.005,
            option_type: CmsSpreadOptionType::Call,
            notional: Money::new(10_000_000.0, finstack_quant_core::currency::Currency::USD),
            expiry_date: Date::from_calendar_date(2026, Month::January, 1).expect("date"),
            payment_date: Date::from_calendar_date(2026, Month::January, 5).expect("date"),
            long_vol_surface_id: CurveId::new("USD-SWAPTION-VOL-10Y"),
            short_vol_surface_id: CurveId::new("USD-SWAPTION-VOL-2Y"),
            discount_curve_id: CurveId::new("USD-OIS"),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            spread_correlation: 0.5,
            day_count: DayCount::Act365F,
            swap_convention: None,
            swap_fixed_freq: None,
            swap_float_freq: None,
            swap_day_count: None,
            swap_float_day_count: None,
            instrument_pricing_overrides: InstrumentPricingOverrides::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
        };
        serde_json::to_string(&InstrumentJson::CmsSpreadOption(option)).expect("serialize")
    }

    pub(crate) fn market_context_json() -> String {
        use finstack_quant_core::market_data::context::MarketContext;
        use finstack_quant_core::market_data::term_structures::DiscountCurve;
        let base = time::Date::from_calendar_date(2024, time::Month::January, 1).expect("date");
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots([(0.5, 0.99), (1.0, 0.98), (5.0, 0.90), (10.0, 0.80)])
            .build()
            .expect("curve");
        let ctx = MarketContext::new().insert(disc);
        serde_json::to_string(&ctx).expect("serialize")
    }

    pub(crate) fn tarn_market_context_json() -> String {
        use finstack_quant_core::dates::DayCount;
        use finstack_quant_core::market_data::context::MarketContext;
        use finstack_quant_core::market_data::scalars::MarketScalar;
        use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
        let base = time::Date::from_calendar_date(2025, time::Month::January, 1).expect("date");
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (6.0, (-0.02_f64 * 6.0).exp())])
            .build()
            .expect("discount curve");
        let fwd = ForwardCurve::builder("USD-SOFR-6M", 0.5)
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 0.03), (6.0, 0.03)])
            .build()
            .expect("forward curve");
        let ctx = MarketContext::new()
            .insert(disc)
            .insert(fwd)
            .insert_price("SOFR-RATE", MarketScalar::Unitless(0.03));
        serde_json::to_string(&ctx).expect("serialize")
    }

    pub(crate) fn cms_spread_market_context_json() -> String {
        use finstack_quant_core::dates::DayCount;
        use finstack_quant_core::market_data::context::MarketContext;
        use finstack_quant_core::market_data::surfaces::VolCube;
        use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
        use finstack_quant_core::math::volatility::sabr::SabrParams;

        fn sabr_cube(id: &str, alpha: f64, forward: f64) -> VolCube {
            let params = SabrParams::new(alpha, 0.5, -0.20, 0.40).expect("valid SABR params");
            VolCube::builder(id)
                .expiries(&[0.25, 1.0, 5.0])
                .tenors(&[2.0, 10.0])
                .node(params, forward)
                .node(params, forward)
                .node(params, forward)
                .node(params, forward)
                .node(params, forward)
                .node(params, forward)
                .build()
                .expect("vol cube")
        }

        let base = time::Date::from_calendar_date(2025, time::Month::January, 1).expect("date");
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (30.0, (-0.035_f64 * 30.0).exp())])
            .build()
            .expect("discount curve");
        let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 0.025), (2.0, 0.030), (10.0, 0.045), (30.0, 0.055)])
            .build()
            .expect("forward curve");
        let ctx = MarketContext::new()
            .insert(disc)
            .insert(fwd)
            .insert_vol_cube(sabr_cube("USD-SWAPTION-VOL-10Y", 0.035, 0.045))
            .insert_vol_cube(sabr_cube("USD-SWAPTION-VOL-2Y", 0.035, 0.030));
        serde_json::to_string(&ctx).expect("serialize")
    }

    fn amount_from_result(parsed: &serde_json::Value) -> f64 {
        parsed["value"]["amount"]
            .as_f64()
            .or_else(|| {
                parsed["value"]["amount"]
                    .as_str()
                    .and_then(|s| s.parse::<f64>().ok())
            })
            .expect("amount")
    }

    #[test]
    fn validate_instrument_json_bond() {
        let json = bond_instrument_json();
        let canonical = validate_instrument_json(&json).expect("validate");
        assert!(!canonical.is_empty());
    }

    #[test]
    fn validate_instrument_json_bermudan_swaption() {
        let json = bermudan_swaption_json();
        let canonical = validate_instrument_json(&json).expect("validate");
        let parsed: serde_json::Value = serde_json::from_str(&canonical).expect("json");
        assert_eq!(parsed["type"], "bermudan_swaption");
    }

    #[test]
    fn price_instrument_bond() {
        let inst = bond_instrument_json();
        let mkt = market_context_json();
        let result = price_instrument(&inst, &mkt, "2024-01-01", "discounting").expect("price");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("json");
        assert!(parsed.is_object());
    }

    #[test]
    fn wasm_market_reuses_parsed_market_for_pricing_and_cashflows() {
        let inst = bond_instrument_json();
        let market = Market::new(&market_context_json()).expect("market handle");

        let priced = price_instrument_with_market(&inst, &market, "2024-01-01", "discounting")
            .expect("price");
        let parsed: serde_json::Value = serde_json::from_str(&priced).expect("price json");
        assert!(parsed.is_object());

        let cashflows =
            instrument_cashflows_with_market(&inst, &market, "2024-01-01", "discounting")
                .expect("cashflows");
        let parsed_cashflows: serde_json::Value =
            serde_json::from_str(&cashflows).expect("cashflow json");
        assert!(parsed_cashflows.is_object());
    }

    #[test]
    fn price_instrument_tarn_hull_white_mc() {
        let inst = tarn_json();
        let mkt = tarn_market_context_json();
        let result = price_instrument(&inst, &mkt, "2025-01-01", "monte_carlo_hull_white_1f")
            .expect("price");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("json");
        let amount = amount_from_result(&parsed);
        assert!(amount > 0.0);
        assert_eq!(parsed["measures"]["mc_num_paths"], 32.0);
    }

    #[test]
    fn price_instrument_mc_is_deterministic_without_explicit_seed() {
        // The MC pricing bindings expose no explicit seed parameter. The
        // determinism contract (documented at the module level) is that the
        // Rust core derives a stable seed from the instrument ID, so repricing
        // the *same* instrument JSON yields bit-identical priced values and MC
        // diagnostics. Only the wall-clock `meta.timestamp` differs between
        // calls, so the assertion targets `value` and `measures` rather than
        // the whole serialized envelope.
        let inst = tarn_json();
        let mkt = tarn_market_context_json();
        let first = price_instrument(&inst, &mkt, "2025-01-01", "monte_carlo_hull_white_1f")
            .expect("first MC price");
        let second = price_instrument(&inst, &mkt, "2025-01-01", "monte_carlo_hull_white_1f")
            .expect("second MC price");
        let first_parsed: serde_json::Value =
            serde_json::from_str(&first).expect("first result json");
        let second_parsed: serde_json::Value =
            serde_json::from_str(&second).expect("second result json");
        assert_eq!(
            first_parsed["value"], second_parsed["value"],
            "MC priced value must be deterministic across repeated calls with no explicit seed"
        );
        assert_eq!(
            first_parsed["measures"], second_parsed["measures"],
            "MC diagnostics (paths, stderr, CI) must be deterministic across repeated calls"
        );
    }

    #[test]
    fn price_instrument_snowball_hull_white_mc() {
        let inst = snowball_json();
        let mkt = tarn_market_context_json();
        let result = price_instrument(&inst, &mkt, "2025-01-01", "monte_carlo_hull_white_1f")
            .expect("price");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("json");
        assert!(amount_from_result(&parsed) > 0.0);
        assert_eq!(parsed["measures"]["mc_num_paths"], 32.0);
    }

    #[test]
    fn price_instrument_inverse_floater_discounting() {
        let inst = inverse_floater_json();
        let mkt = tarn_market_context_json();
        let result = price_instrument(&inst, &mkt, "2025-01-01", "discounting").expect("price");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("json");
        assert!(amount_from_result(&parsed) > 0.0);
    }

    #[test]
    fn price_instrument_callable_range_accrual_hull_white_mc() {
        let inst = callable_range_accrual_json();
        let mkt = tarn_market_context_json();
        let result = price_instrument(&inst, &mkt, "2025-01-01", "monte_carlo_hull_white_1f")
            .expect("price");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("json");
        assert!(amount_from_result(&parsed) > 0.0);
        assert_eq!(parsed["measures"]["mc_num_paths"], 8.0);
    }

    #[test]
    fn price_instrument_cms_spread_option_static_replication() {
        let inst = cms_spread_option_json();
        let mkt = cms_spread_market_context_json();
        let result =
            price_instrument(&inst, &mkt, "2025-01-01", "static_replication").expect("price");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("json");
        assert!(amount_from_result(&parsed) > 0.0);
        assert!(
            parsed["measures"]["cms_spread_forward"]
                .as_f64()
                .expect("cms spread forward")
                > 0.0
        );
    }

    #[test]
    fn validate_valuation_result_json_roundtrip() {
        let inst = bond_instrument_json();
        let mkt = market_context_json();
        let result_json =
            price_instrument(&inst, &mkt, "2024-01-01", "discounting").expect("price");
        let canonical = validate_valuation_result_json(&result_json).expect("validate");
        assert!(!canonical.is_empty());
        let parsed: serde_json::Value = serde_json::from_str(&canonical).expect("json");
        assert!(parsed.is_object());
    }

    // (Credit-model evaluator parity tests live in `super::credit::tests`,
    // co-located with the functions they exercise.)

    /// Build a floored-bond InstrumentJson string from a raw JSON spec.
    ///
    /// Uses the same 5-year 10% annual bullet as the Python `test_return_floor.py`
    /// fixture so the two surfaces stay directly comparable.
    fn return_floor_bond_instrument_json(return_floor: serde_json::Value) -> String {
        let spec = serde_json::json!({
            "id": "WASM-RETURN-FLOOR-BOND",
            "notional": { "amount": "1000000", "currency": "USD" },
            "issue_date": "2024-01-01",
            "maturity": "2029-01-01",
            "cashflow_spec": {
                "Fixed": {
                    "rate": "0.10",
                    "freq": { "count": 12, "unit": "months" },
                    "dc": "Thirty360",
                    "bdc": "following",
                    "calendar_id": "weekends_only"
                }
            },
            "discount_curve_id": "USD-OIS",
            "settlement_days": 0,
            "ex_coupon_days": 0,
            "attributes": {},
            "return_floor": return_floor
        });
        serde_json::json!({ "type": "bond", "spec": spec }).to_string()
    }

    /// Minimal 5-year flat discount market for the return-floor tests.
    fn return_floor_market_json() -> String {
        serde_json::json!({
            "version": 2,
            "curves": [{
                "type": "discount",
                "id": "USD-OIS",
                "base": "2024-01-01",
                "day_count": "Act365F",
                "knot_points": [[0.0, 1.0], [5.0, 0.85]],
                "interp_style": "monotone_convex",
                "extrapolation": "flat_forward",
                "min_forward_rate": null,
                "allow_non_monotonic": false,
                "min_forward_tenor": 1e-6
            }],
            "fx": null,
            "surfaces": [],
            "prices": {},
            "series": [],
            "inflation_indices": [],
            "dividends": [],
            "credit_indices": [],
            "fx_delta_vol_surfaces": [],
            "vol_cubes": [],
            "collateral": {}
        })
        .to_string()
    }

    #[test]
    fn return_floor_bond_moic_floor_validates_and_prices() {
        // Smoke test: a bond with a 1.25× MOIC return-floor spec round-trips
        // through the JSON validator and prices successfully via the discounting
        // model — no new Rust binding code is required; return_floor is already a
        // serde field on the core Bond type.
        let floor_spec = serde_json::json!({
            "kind": { "Moic": 1.25 },
            "issue_price": "Par",
            "window": "Full"
        });
        let inst = return_floor_bond_instrument_json(floor_spec);

        // Validate round-trips through the binding
        let canonical = validate_instrument_json(&inst).expect("validate");
        assert!(
            canonical.contains("return_floor"),
            "return_floor survived round-trip"
        );

        // Price and check the four return metrics via the internal helper
        // (avoids serde_wasm_bindgen which requires a wasm32 target).
        let mkt = return_floor_market_json();
        let market = parse_market_json(&mkt).expect("market");
        let metrics = vec![
            "moic".to_string(),
            "moic_to_worst".to_string(),
            "xirr".to_string(),
            "xirr_to_worst".to_string(),
        ];
        let result_json = price_instrument_with_metrics_context(
            &inst,
            &market,
            "2024-01-01",
            "discounting",
            metrics,
            None,
            None,
        )
        .expect("price_with_metrics");
        let parsed: serde_json::Value = serde_json::from_str(&result_json).expect("parse");

        // 10% annual 5Y par bullet: MOIC ≈ 1.50 (5 × 0.10 + 1.0 principal)
        let moic = parsed["measures"]["moic"].as_f64().expect("moic");
        assert!(moic > 1.0, "MOIC must be > 1.0 (coupon income)");
        assert!((moic - 1.50).abs() < 0.02, "MOIC ≈ 1.50, got {moic}");

        // XIRR ≈ 10% for a par bullet
        let xirr = parsed["measures"]["xirr"].as_f64().expect("xirr");
        assert!((xirr - 0.10).abs() < 0.005, "XIRR ≈ 0.10, got {xirr}");

        // moic_to_worst ≤ moic: the floored bond has synthetic call options injected
        // by the return-floor machinery, so the worst-exit path can only be equal
        // to or worse than the held-to-maturity multiple.
        let moic_tw = parsed["measures"]["moic_to_worst"]
            .as_f64()
            .expect("moic_to_worst");
        assert!(
            moic_tw <= moic + 1e-9,
            "moic_to_worst must be ≤ moic, got {moic_tw} vs {moic}"
        );
    }

    #[test]
    fn return_floor_bond_xirr_floor_prices_without_error() {
        let floor_spec = serde_json::json!({
            "kind": { "Xirr": 0.12 },
            "issue_price": "Par",
            "window": "Full"
        });
        let inst = return_floor_bond_instrument_json(floor_spec);
        let mkt = return_floor_market_json();
        let market = parse_market_json(&mkt).expect("market");
        let metrics = vec!["xirr".to_string(), "xirr_to_worst".to_string()];
        let result_json = price_instrument_with_metrics_context(
            &inst,
            &market,
            "2024-01-01",
            "discounting",
            metrics,
            None,
            None,
        )
        .expect("xirr floor bond prices");
        let parsed: serde_json::Value = serde_json::from_str(&result_json).expect("parse");
        // Price > 0 and xirr metric present
        let amount = parsed["value"]["amount"]
            .as_f64()
            .or_else(|| {
                parsed["value"]["amount"]
                    .as_str()
                    .and_then(|s| s.parse::<f64>().ok())
            })
            .expect("value.amount");
        assert!(amount > 0.0, "floored bond price must be positive");
        let xirr = parsed["measures"]["xirr"].as_f64().expect("xirr");
        assert!(xirr > 0.0, "xirr must be positive");
    }

    #[test]
    fn return_floor_metrics_in_standard_metrics_list() {
        // The four return-floor metric IDs must be present in the global registry.
        // Uses the underlying Rust pricer function directly (no JsValue).
        let ids = finstack_quant_valuations::pricer::list_standard_metrics();
        for metric in ["moic", "moic_to_worst", "xirr", "xirr_to_worst"] {
            assert!(
                ids.iter().any(|id| id == metric),
                "'{metric}' missing from standard metrics"
            );
        }
    }
}
