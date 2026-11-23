use super::super::results::valuation_result_json;
use super::engine::*;
use crate::utils::to_js_err;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::Compounding;
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_quant_valuations::metrics::risk::MarketHistory;
use finstack_quant_valuations::pricer::PriceExtras;
use finstack_quant_valuations::results::ValuationResult;
use wasm_bindgen::JsValue;

fn price_instrument_with_context(
    instrument_json: &str,
    market: &MarketContext,
    as_of: &str,
    model: &str,
    metrics: Vec<String>,
    metric_pricing_overrides: Option<&str>,
    market_history: Option<&MarketHistory>,
) -> Result<String, JsValue> {
    let instrument = parse_pricing_instrument_json(instrument_json, metric_pricing_overrides)?;
    valuation_result_json(price_result_with_context(
        &instrument,
        market,
        as_of,
        model,
        metrics,
        &PriceExtras {
            market_history: market_history.cloned(),
            ..Default::default()
        },
    )?)
}

/// Native-testable core of [`price_instrument`].
///
/// Kept separate from the `#[wasm_bindgen]` wrapper because `JsValue`
/// construction aborts on non-wasm32 targets, so the unit tests below drive
/// this function and assert on the `ValuationResult` directly.
fn price_instrument_result(
    instrument_json: &str,
    market_json: &str,
    as_of: &str,
    model: Option<&str>,
) -> Result<ValuationResult, JsValue> {
    let instrument = parse_pricing_instrument_json(instrument_json, None)?;
    let market: MarketContext = serde_json::from_str(market_json).map_err(to_js_err)?;
    price_result_with_context(
        &instrument,
        &market,
        as_of,
        model.unwrap_or("default"),
        Vec::new(),
        &PriceExtras::default(),
    )
}

/// A request whose market, date and model are all invalid.
pub(crate) fn not_a_market_request() -> PriceRequest {
    PriceRequest {
        as_of: "not-a-date".to_string(),
        model: Some("not-a-model".to_string()),
        metrics: Vec::new(),
        extras: PriceExtras::default(),
    }
}

fn envelope_json(
    instrument: finstack_quant_valuations::instruments::json_loader::InstrumentJson,
) -> String {
    serde_json::to_string(
        &finstack_quant_valuations::instruments::json_loader::InstrumentEnvelope::new(instrument),
    )
    .expect("serialize instrument envelope")
}

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
        finstack_quant_valuations::pricer::parse_model_key("rates_credit").expect("ok"),
        finstack_quant_valuations::pricer::ModelKey::RatesCredit
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

#[test]
fn public_json_routes_validate_instrument_before_market_json() {
    assert!(not_a_market_request().instrument("{}").is_err());
    assert!(parse_pricing_instrument_json("{}", None).is_err());
}

pub(crate) fn bond_instrument_json() -> String {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::Rate;
    use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
    use finstack_quant_valuations::instruments::json_loader::InstrumentJson;

    let bond = Bond::fixed(
        "TEST-BOND",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        Rate::from_decimal(0.05).expect("valid rate fixture"),
        finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
            .expect("date"),
        finstack_quant_core::dates::Date::from_calendar_date(2034, time::Month::January, 1)
            .expect("date"),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    envelope_json(InstrumentJson::Bond(bond))
}

fn revolving_credit_json(invalid_gearing: bool, with_credit: bool) -> String {
    use finstack_quant_cashflows::builder::RateSpec;
    use finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCredit;
    use finstack_quant_valuations::instruments::json_loader::InstrumentJson;

    let mut facility = RevolvingCredit::example().expect("facility");
    if invalid_gearing {
        let RateSpec::Floating(spec) = &mut facility.rate else {
            unreachable!("example is floating");
        };
        spec.gearing = Default::default();
    } else {
        facility.rate = RateSpec::Fixed { rate: 0.05 };
    }
    if with_credit {
        facility.credit_curve_id = Some("USD-HZ".into());
        facility.recovery_rate = 0.4;
    }
    envelope_json(InstrumentJson::RevolvingCredit(facility))
}

fn revolving_credit_market(with_credit: bool) -> MarketContext {
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use time::macros::date;

    let mut market = MarketContext::new()
        .try_insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(date!(2024 - 01 - 01))
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.85)])
                .interp(InterpStyle::MonotoneConvex)
                .extrapolation(ExtrapolationPolicy::Extend)
                .build()
                .expect("discount curve"),
        )
        .expect("insert market curve");
    if with_credit {
        market = market
            .try_insert(
                HazardCurve::builder("USD-HZ")
                    .base_date(date!(2024 - 01 - 01))
                    .day_count(DayCount::Act365F)
                    .recovery_rate(0.4)
                    .knots([(1.0, 0.02), (5.0, 0.02)])
                    .build()
                    .expect("hazard curve"),
            )
            .expect("insert market curve");
    }
    market
}

pub(crate) fn bermudan_swaption_json() -> String {
    use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
    use finstack_quant_valuations::instruments::rates::swaption::BermudanSwaption;

    envelope_json(InstrumentJson::BermudanSwaption(
        BermudanSwaption::example().expect("example"),
    ))
}

pub(crate) fn tarn_json() -> String {
    use finstack_quant_core::dates::{Date, DayCount, Tenor};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_valuations::instruments::exotics::tarn::Tarn;
    use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
    use finstack_quant_valuations::instruments::pricing_overrides::InstrumentPricingOverrides;
    use time::Month;

    let mut instrument_pricing_overrides = InstrumentPricingOverrides::default();
    instrument_pricing_overrides.model_config.mc_paths = Some(32);
    instrument_pricing_overrides
        .model_config
        .hw1f_mean_reversion = Some(0.05);
    instrument_pricing_overrides.model_config.hw1f_sigma = Some(1e-12);

    let tarn = Tarn {
        id: InstrumentId::new("TARN-WASM-E2E"),
        fixed_rate: "0.06".parse().expect("rate"),
        coupon_floor: 0.0,
        target_coupon: 1.0,
        notional: Money::new(1_000_000.0, finstack_quant_core::currency::Currency::USD)
            .expect("valid money fixture"),
        start_date: Date::from_calendar_date(2025, Month::January, 1).expect("date"),
        payment_dates: vec![
            Date::from_calendar_date(2025, Month::July, 1).expect("date"),
            Date::from_calendar_date(2026, Month::January, 1).expect("date"),
            Date::from_calendar_date(2026, Month::July, 1).expect("date"),
        ],
        index_tenor: Tenor::semi_annual(),
        forward_curve_id: CurveId::new("USD-SOFR-6M"),
        discount_curve_id: CurveId::new("USD-OIS"),
        vol_surface_id: Some(CurveId::new("USD-SOFR-HW-VOL")),
        day_count: DayCount::Act365F,
        instrument_pricing_overrides,
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };
    envelope_json(InstrumentJson::Tarn(tarn))
}

pub(crate) fn snowball_json() -> String {
    use finstack_quant_core::dates::{Date, DayCount, Tenor};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_valuations::instruments::exotics::snowball::{Snowball, SnowballVariant};
    use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
    use finstack_quant_valuations::instruments::pricing_overrides::InstrumentPricingOverrides;
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
        fixed_rate: "0.05".parse().expect("rate"),
        gearing: 1.0,
        coupon_floor: 0.0,
        coupon_cap: None,
        notional: Money::new(1_000_000.0, finstack_quant_core::currency::Currency::USD)
            .expect("valid money fixture"),
        start_date: Date::from_calendar_date(2025, Month::January, 1).expect("date"),
        payment_dates: vec![
            Date::from_calendar_date(2025, Month::July, 1).expect("date"),
            Date::from_calendar_date(2026, Month::January, 1).expect("date"),
            Date::from_calendar_date(2026, Month::July, 1).expect("date"),
        ],
        forward_curve_id: CurveId::new("USD-SOFR-6M"),
        index_tenor: Tenor::semi_annual(),
        discount_curve_id: CurveId::new("USD-OIS"),
        vol_surface_id: Some(CurveId::new("USD-SOFR-HW-VOL")),
        call_provision: None,
        day_count: DayCount::Act365F,
        instrument_pricing_overrides,
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };
    envelope_json(InstrumentJson::Snowball(snowball))
}

pub(crate) fn inverse_floater_json() -> String {
    use finstack_quant_core::dates::{Date, DayCount, Tenor};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_valuations::instruments::exotics::snowball::{Snowball, SnowballVariant};
    use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
    use finstack_quant_valuations::instruments::pricing_overrides::InstrumentPricingOverrides;
    use time::Month;

    let inverse_floater = Snowball {
        id: InstrumentId::new("INV-FLOATER-WASM-E2E"),
        variant: SnowballVariant::InverseFloater,
        initial_coupon: 0.0,
        fixed_rate: "0.08".parse().expect("rate"),
        gearing: 1.5,
        coupon_floor: 0.0,
        coupon_cap: Some(0.10),
        notional: Money::new(500_000.0, finstack_quant_core::currency::Currency::USD)
            .expect("valid money fixture"),
        start_date: Date::from_calendar_date(2025, Month::January, 1).expect("date"),
        payment_dates: vec![
            Date::from_calendar_date(2025, Month::July, 1).expect("date"),
            Date::from_calendar_date(2026, Month::January, 1).expect("date"),
            Date::from_calendar_date(2026, Month::July, 1).expect("date"),
        ],
        forward_curve_id: CurveId::new("USD-SOFR-6M"),
        index_tenor: Tenor::semi_annual(),
        discount_curve_id: CurveId::new("USD-OIS"),
        vol_surface_id: Some(CurveId::new("USD-SOFR-HW-VOL")),
        call_provision: None,
        day_count: DayCount::Act365F,
        instrument_pricing_overrides: InstrumentPricingOverrides::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };
    envelope_json(InstrumentJson::Snowball(inverse_floater))
}

pub(crate) fn callable_range_accrual_json() -> String {
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_valuations::instruments::exotics::BermudanCallProvision;
    use finstack_quant_valuations::instruments::exotics::callable_range_accrual::CallableRangeAccrual;
    use finstack_quant_valuations::instruments::exotics::range_accrual::{
        BoundsType, RangeAccrualTerms,
    };
    use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
    use finstack_quant_valuations::instruments::pricing_overrides::InstrumentPricingOverrides;
    use time::Month;

    let mut instrument_pricing_overrides = InstrumentPricingOverrides::default();
    instrument_pricing_overrides.model_config.mc_paths = Some(8);
    instrument_pricing_overrides
        .model_config
        .hw1f_mean_reversion = Some(0.05);
    instrument_pricing_overrides.model_config.hw1f_sigma = Some(1e-12);

    let range_accrual = RangeAccrualTerms::builder()
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
        .notional(
            Money::new(1_000_000.0, finstack_quant_core::currency::Currency::USD)
                .expect("valid money fixture"),
        )
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .start_date(Date::from_calendar_date(2025, Month::January, 1).expect("date"))
        .index_id_opt(Some("SOFR".into()))
        .forward_curve_id_opt(Some(CurveId::new("USD-OIS")))
        .index_tenor_opt(Some(
            finstack_quant_core::dates::Tenor::new(
                6,
                finstack_quant_core::dates::TenorUnit::Months,
            )
            .expect("valid tenor fixture"),
        ))
        .spot_id("SOFR-RATE".into())
        .vol_surface_id(CurveId::new("SOFR-VOL"))
        .div_yield_id_opt(None)
        .payment_date_opt(None)
        .past_observations_in_range_opt(None)
        .total_past_observations_opt(None)
        .build()
        .expect("range accrual");

    let callable = CallableRangeAccrual {
        id: InstrumentId::new("CALLABLE-RA-WASM-E2E"),
        range_accrual,
        call_provision: BermudanCallProvision::new(
            vec![Date::from_calendar_date(2025, Month::July, 1).expect("date")],
            100.0,
            None,
        ),
        instrument_pricing_overrides,
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };
    envelope_json(InstrumentJson::CallableRangeAccrual(Box::new(callable)))
}

pub(crate) fn cms_spread_option_json() -> String {
    use finstack_quant_core::dates::{Date, DayCount, Tenor, TenorUnit};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_models::types::OptionType;
    use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
    use finstack_quant_valuations::instruments::pricing_overrides::InstrumentPricingOverrides;
    use finstack_quant_valuations::instruments::rates::cms_spread_option::CmsSpreadOption;
    use time::Month;

    let option = CmsSpreadOption {
        id: InstrumentId::new("CMS-SPREAD-WASM-E2E"),
        long_cms_tenor: Tenor::new(10, TenorUnit::Years).expect("valid tenor fixture"),
        short_cms_tenor: Tenor::new(2, TenorUnit::Years).expect("valid tenor fixture"),
        strike: "0.005".parse().expect("strike"),
        option_type: OptionType::Call,
        notional: Money::new(10_000_000.0, finstack_quant_core::currency::Currency::USD)
            .expect("valid money fixture"),
        expiry: Date::from_calendar_date(2026, Month::January, 1).expect("date"),
        payment_date: Date::from_calendar_date(2026, Month::January, 5).expect("date"),
        long_vol_surface_id: CurveId::new("USD-SWAPTION-VOL-10Y"),
        short_vol_surface_id: CurveId::new("USD-SWAPTION-VOL-2Y"),
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        correlation: 0.5,
        day_count: DayCount::Act365F,
        index_id: Some(finstack_quant_core::types::IndexId::new("USD-SOFR-OIS")),
        long_cms_fixing_index_id: None,
        short_cms_fixing_index_id: None,
        swap_fixed_frequency: None,
        swap_float_frequency: None,
        swap_fixed_day_count: None,
        swap_float_day_count: None,
        instrument_pricing_overrides: InstrumentPricingOverrides::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };
    envelope_json(InstrumentJson::CmsSpreadOption(option))
}

pub(crate) fn market_context_json() -> String {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    let base = finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
        .expect("date");
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([(0.5, 0.99), (1.0, 0.98), (5.0, 0.90), (10.0, 0.80)])
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .build()
        .expect("curve");
    let ctx = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve");
    serde_json::to_string(&ctx).expect("serialize")
}

pub(crate) fn tarn_market_context_json() -> String {
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    let base = finstack_quant_core::dates::Date::from_calendar_date(2025, time::Month::January, 1)
        .expect("date");
    let disc =
        DiscountCurve::flat("USD-OIS", base, 0.02, DayCount::Act365F).expect("discount curve");
    let fwd =
        ForwardCurve::flat("USD-SOFR-6M", base, 0.03, DayCount::Act365F).expect("forward curve");
    let ctx = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(fwd)
        .expect("insert market curve")
        .insert_price(
            "SOFR-RATE",
            MarketScalar::unitless(0.03).expect("unitless scalar"),
        );
    serde_json::to_string(&ctx).expect("serialize")
}

pub(crate) fn cms_spread_market_context_json() -> String {
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};

    fn flat_surface(id: &str, vol: f64) -> VolSurface {
        use finstack_quant_core::market_data::surfaces::{
            VolInterpolationMode, VolQuoteType, VolSurfaceAxis,
        };
        VolSurface::builder(id)
            .expiries(&[0.25, 1.0, 5.0])
            .secondary_values(&[0.01, 0.10])
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .row(&[vol, vol])
            .row(&[vol, vol])
            .row(&[vol, vol])
            .build()
            .expect("flat Black surface")
    }

    let base = finstack_quant_core::dates::Date::from_calendar_date(2025, time::Month::January, 1)
        .expect("date");
    let disc =
        DiscountCurve::flat("USD-OIS", base, 0.035, DayCount::Act365F).expect("discount curve");
    let fwd = ForwardCurve::from_zero_rates(
        "USD-SOFR-3M",
        base,
        &[(2.0, 0.030), (10.0, 0.045), (30.0, 0.055)],
        Compounding::Continuous,
        DayCount::Act365F,
        InterpStyle::LogLinear,
        ExtrapolationPolicy::Extend,
    )
    .expect("forward curve");
    let ctx = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(fwd)
        .expect("insert market curve")
        .insert_surface(flat_surface("USD-SWAPTION-VOL-10Y", 0.25))
        .insert_surface(flat_surface("USD-SWAPTION-VOL-2Y", 0.20));
    serde_json::to_string(&ctx).expect("serialize")
}

/// Price through the native-testable core and expose the canonical
/// serde document the `priceInstrument` binding hands to JavaScript.
///
/// The `#[wasm_bindgen]` wrappers return `JsValue`, whose construction
/// aborts on non-wasm32 targets, so unit tests drive the core instead. The
/// JS-object shape itself is covered by `tests/wasm_valuations.rs` and
/// `tests/facade/return_floor.test.mjs`.
fn priced_document(
    instrument_json: &str,
    market_json: &str,
    as_of: &str,
    model: &str,
) -> serde_json::Value {
    let result =
        price_instrument_result(instrument_json, market_json, as_of, Some(model)).expect("price");
    serde_json::to_value(&result).expect("serialize valuation result")
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
    let canonical =
        finstack_quant_valuations::pricer::validate_instrument_json(&json, None).expect("validate");
    assert!(!canonical.is_empty());
}

#[test]
fn validate_instrument_json_bermudan_swaption() {
    let json = bermudan_swaption_json();
    let canonical =
        finstack_quant_valuations::pricer::validate_instrument_json(&json, None).expect("validate");
    let parsed: serde_json::Value = serde_json::from_str(&canonical).expect("json");
    assert_eq!(parsed["instrument"]["type"], "bermudan_swaption");
}

#[test]
fn validate_revolving_credit_rejects_invalid_floating_rate_spec() {
    assert!(
        finstack_quant_valuations::pricer::validate_instrument_json(
            &revolving_credit_json(true, false),
            None
        )
        .is_err()
    );
}

#[test]
fn revolving_credit_metrics_and_cashflow_fail_closed_cross_wasm_context() {
    // `drawn` (10M of 50M) is the balance at the valuation date;
    // the example's 5M draw on 2024-03-01 lies in the future of
    // 2024-02-01 and must not enter the as-of metrics.
    let instrument = revolving_credit_json(false, false);
    let market = revolving_credit_market(false);
    let result = price_instrument_with_context(
        &instrument,
        &market,
        "2024-02-01",
        "discounting",
        vec![
            "utilization_rate".to_string(),
            "available_capacity".to_string(),
        ],
        None,
        None,
    )
    .expect("metrics");
    let parsed: serde_json::Value = serde_json::from_str(&result).expect("result");
    assert_eq!(parsed["measures"]["utilization_rate"], 0.20);
    assert_eq!(parsed["measures"]["available_capacity"], 40_000_000.0);

    let credit_instrument =
        parse_pricing_instrument_json(&revolving_credit_json(false, true), None)
            .expect("credit instrument");
    assert!(
        finstack_quant_valuations::instruments::cashflow_export::instrument_cashflows(
            credit_instrument.as_instrument(),
            &revolving_credit_market(true),
            "2024-01-01",
            "discounting",
        )
        .is_err()
    );
}

#[test]
fn price_instrument_bond() {
    let parsed = priced_document(
        &bond_instrument_json(),
        &market_context_json(),
        "2024-01-01",
        "discounting",
    );
    assert!(parsed.is_object());
    assert_eq!(parsed["instrument_id"], "TEST-BOND");
}

#[test]
fn parsed_market_prices_and_exports_cashflows() {
    let inst = bond_instrument_json();
    let market = serde_json::from_str::<MarketContext>(&market_context_json()).expect("market");
    let instrument = parse_pricing_instrument_json(&inst, None).expect("parsed instrument");

    let priced = price_result_with_context(
        &instrument,
        &market,
        "2024-01-01",
        "discounting",
        Vec::new(),
        &PriceExtras::default(),
    )
    .expect("price");
    let parsed = serde_json::to_value(&priced).expect("price json");
    assert!(parsed.is_object());

    let cashflows = finstack_quant_valuations::instruments::cashflow_export::instrument_cashflows(
        instrument.as_instrument(),
        &market,
        "2024-01-01",
        "discounting",
    )
    .expect("cashflows");
    let parsed_cashflows = serde_json::to_value(&cashflows).expect("cashflow json");
    assert!(parsed_cashflows.is_object());
}

#[test]
fn price_instrument_tarn_hull_white_mc() {
    let parsed = priced_document(
        &tarn_json(),
        &tarn_market_context_json(),
        "2025-01-01",
        "monte_carlo_hull_white_1f",
    );
    let amount = amount_from_result(&parsed);
    assert!(amount > 0.0);
    assert_eq!(parsed["measures"]["mc_num_paths"], 32.0);
    assert_eq!(parsed["measures"]["mc_num_simulated_paths"], 64.0);
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
    let first_parsed = priced_document(&inst, &mkt, "2025-01-01", "monte_carlo_hull_white_1f");
    let second_parsed = priced_document(&inst, &mkt, "2025-01-01", "monte_carlo_hull_white_1f");
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
    let parsed = priced_document(
        &snowball_json(),
        &tarn_market_context_json(),
        "2025-01-01",
        "monte_carlo_hull_white_1f",
    );
    assert!(amount_from_result(&parsed) > 0.0);
    assert_eq!(parsed["measures"]["mc_num_paths"], 32.0);
    assert_eq!(parsed["measures"]["mc_num_simulated_paths"], 64.0);
}

#[test]
fn price_instrument_inverse_floater_discounting() {
    let parsed = priced_document(
        &inverse_floater_json(),
        &tarn_market_context_json(),
        "2025-01-01",
        "discounting",
    );
    assert!(amount_from_result(&parsed) > 0.0);
}

#[test]
fn price_instrument_callable_range_accrual_hull_white_mc() {
    let parsed = priced_document(
        &callable_range_accrual_json(),
        &tarn_market_context_json(),
        "2025-01-01",
        "monte_carlo_hull_white_1f",
    );
    assert!(amount_from_result(&parsed) > 0.0);
    assert_eq!(parsed["measures"]["mc_num_paths"], 8.0);
    assert_eq!(parsed["measures"]["mc_num_simulated_paths"], 16.0);
}

#[test]
fn price_instrument_cms_spread_option_static_replication() {
    let parsed = priced_document(
        &cms_spread_option_json(),
        &cms_spread_market_context_json(),
        "2025-01-01",
        "static_replication",
    );
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
    // JS callers reach this by stringifying the object `priceInstrument`
    // returns; the wire document is unchanged by that conversion.
    let result = price_instrument_result(
        &bond_instrument_json(),
        &market_context_json(),
        "2024-01-01",
        Some("discounting"),
    )
    .expect("price");
    let result_json = valuation_result_json(result).expect("serialize");
    let reparsed: ValuationResult = serde_json::from_str(&result_json).expect("parse");
    let canonical = valuation_result_json(reparsed).expect("validate");
    assert!(!canonical.is_empty());
    let parsed: serde_json::Value = serde_json::from_str(&canonical).expect("json");
    assert!(parsed.is_object());
}

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
            "fixed": {
                "rate": "0.10",
                "frequency": { "count": 12, "unit": "months" },
                "day_count": "30_360",
                "business_day_convention": "following",
                "calendar_id": "weekends_only"
            }
        },
        "quote_compounding": {"periodic": 1},
        "discount_curve_id": "USD-OIS",
        "settlement_days": 0,
        "ex_coupon_days": 0,
        "attributes": {},
        // The rates tree needs explicit HW1F inputs; sigma = 0 selects
        // deterministic rates for this smoke test.
        "instrument_pricing_overrides": {
            "model_config": { "hw1f_mean_reversion": 0.03, "hw1f_sigma": 0.0 }
        },
        "return_floor": return_floor
    });
    let bond = serde_json::from_value(spec).expect("valid return-floor bond fixture");
    envelope_json(finstack_quant_valuations::instruments::json_loader::InstrumentJson::Bond(bond))
}

/// Minimal 5-year flat discount market for the return-floor tests.
fn return_floor_market_json() -> String {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(time::macros::date!(2024 - 01 - 01))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([(0.0, 1.0), (5.0, 0.85)])
        .interp(InterpStyle::MonotoneConvex)
        .extrapolation(ExtrapolationPolicy::Extend)
        .build()
        .expect("valid discount curve");
    serde_json::to_string(
        &MarketContext::new()
            .try_insert(curve)
            .expect("insert market curve"),
    )
    .expect("serialize market")
}

#[test]
fn return_floor_bond_moic_floor_validates_and_prices() {
    // Smoke test: a bond with a 1.25× MOIC return-floor spec round-trips
    // through the JSON validator and prices successfully via the rates tree
    // model — no new Rust binding code is required; return_floor is already a
    // serde field on the core Bond type.
    let floor_spec = serde_json::json!({
        "kind": { "moic": 1.25 },
        "issue_price": "par",
        "window": "full"
    });
    let inst = return_floor_bond_instrument_json(floor_spec);

    // Validate round-trips through the binding
    let canonical =
        finstack_quant_valuations::pricer::validate_instrument_json(&inst, None).expect("validate");
    assert!(
        canonical.contains("return_floor"),
        "return_floor survived round-trip"
    );

    // Price and check the four return metrics via the internal helper
    // (avoids serde_wasm_bindgen which requires a wasm32 target).
    let mkt = return_floor_market_json();
    let market = serde_json::from_str::<MarketContext>(&mkt).expect("market");
    let metrics = vec![
        "moic".to_string(),
        "moic_to_worst".to_string(),
        "xirr".to_string(),
        "xirr_to_worst".to_string(),
    ];
    let result_json =
        price_instrument_with_context(&inst, &market, "2024-01-01", "tree", metrics, None, None)
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
        "kind": { "xirr": 0.12 },
        "issue_price": "par",
        "window": "full"
    });
    let inst = return_floor_bond_instrument_json(floor_spec);
    let mkt = return_floor_market_json();
    let market = serde_json::from_str::<MarketContext>(&mkt).expect("market");
    let metrics = vec!["xirr".to_string(), "xirr_to_worst".to_string()];
    let result_json =
        price_instrument_with_context(&inst, &market, "2024-01-01", "tree", metrics, None, None)
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
