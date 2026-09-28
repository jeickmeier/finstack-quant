//! Exotic payoff terms share one vocabulary on the wire.
//!
//! - Barrier and fixing levels are bare `f64` quotes in the underlying's price
//!   units (`barrier`, `expiry_fixing`, `observed_min` / `observed_max`).
//! - A rebate is a total `Money` amount; for FX it is paid in the quote currency.
//! - Historical fixings are `past_fixings`; a single realized fixing is
//!   `observed_fixing`; a recorded barrier hit is `observed_barrier_breached`.
//! - Lookback monitoring is the contractual `monitoring` convention.
//! - The autocallable participation rate lives once, inside its payoff variant.
//! - The inter-leg correlation of a spread option is `correlation`.
//!
//! Each retired spelling is rejected by `deny_unknown_fields`.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
use finstack_quant_valuations::instruments::exotics::lookback_option::{
    LookbackOption, LookbackType,
};
use finstack_quant_valuations::instruments::{
    Attributes, Autocallable, BarrierOption, CmsSpreadOption, CommodityAsianOption,
    CommodityFuture, CommoditySwap, FxBarrierOption, FxTouchOption, Instrument,
    InstrumentPricingOverrides, Monitoring, Ndf, OptionType, PricingOptions, RangeAccrual,
};
use finstack_quant_valuations::pricer::ModelKey;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use time::Month;

/// Serialize `instrument`, insert the retired top-level spelling with `value`,
/// and require rejection naming that key.
fn assert_rejects<T: Serialize + DeserializeOwned>(instrument: &T, retired: &str, value: Value) {
    let mut json = serde_json::to_value(instrument).expect("serialize");
    json.as_object_mut()
        .expect("instrument serializes as an object")
        .insert(retired.to_string(), value);
    let err = serde_json::from_value::<T>(json)
        .err()
        .unwrap_or_else(|| panic!("retired key {retired} must be rejected"));
    assert!(err.to_string().contains(retired), "{retired}: {err}");
}

/// Replace `field` with `value` and require the instrument to fail to deserialize.
fn assert_shape_rejected<T: Serialize + DeserializeOwned>(
    instrument: &T,
    field: &str,
    value: Value,
) {
    let mut json = serde_json::to_value(instrument).expect("serialize");
    json[field] = value;
    assert!(
        serde_json::from_value::<T>(json).is_err(),
        "retired shape of `{field}` must be rejected"
    );
}

#[test]
// schema-rejection-test: BarrierOption `barrier` / `expiry_fixing` as `{amount, currency}` Money objects
fn barrier_option_money_levels_are_rejected() {
    let option = BarrierOption::example().expect("example");
    let money = serde_json::json!({"amount": "5000", "currency": "USD"});
    assert_shape_rejected(&option, "barrier", money.clone());
    assert_shape_rejected(&option, "expiry_fixing", money);
}

#[test]
// schema-rejection-test: LookbackOption `use_gobet_miri`; `observed_min`/`observed_max`/`expiry_fixing` as Money objects
fn lookback_option_retired_keys_and_money_levels_are_rejected() {
    let option = LookbackOption::example().expect("example");
    assert_rejects(&option, "use_gobet_miri", serde_json::json!(true));
    let money = serde_json::json!({"amount": "100", "currency": "USD"});
    assert_shape_rejected(&option, "observed_min", money.clone());
    assert_shape_rejected(&option, "observed_max", money.clone());
    assert_shape_rejected(&option, "expiry_fixing", money);
}

#[test]
// schema-rejection-test: FxTouchOption `barrier_level`, `observed_touch`
fn fx_touch_option_retired_keys_are_rejected() {
    let option = FxTouchOption::example().expect("example");
    assert_rejects(&option, "barrier_level", serde_json::json!(1.05));
    assert_rejects(&option, "observed_touch", serde_json::json!(false));
}

#[test]
// schema-rejection-test: FxBarrierOption `rebate` as a per-unit number
fn fx_barrier_option_per_unit_rebate_is_rejected() {
    let option = FxBarrierOption::example().expect("example");
    assert_shape_rejected(&option, "rebate", serde_json::json!(0.02));
}

#[test]
// schema-rejection-test: Ndf `fixing_rate`
fn ndf_retired_fixing_rate_is_rejected() {
    assert_rejects(
        &Ndf::example().expect("example"),
        "fixing_rate",
        serde_json::json!(7.3),
    );
}

#[test]
// schema-rejection-test: CmsSpreadOption `spread_correlation`
fn cms_spread_option_retired_spread_correlation_is_rejected() {
    let option = CmsSpreadOption::example().expect("example");
    assert_rejects(&option, "spread_correlation", serde_json::json!(0.85));
    // `correlation` is a bounded correlation on the wire.
    assert_shape_rejected(&option, "correlation", serde_json::json!(1.5));
}

#[test]
// schema-rejection-test: CommodityAsianOption, CommoditySwap and CommodityFuture `realized_fixings`
fn commodity_retired_realized_fixings_are_rejected() {
    assert_rejects(
        &CommodityAsianOption::example().expect("example"),
        "realized_fixings",
        serde_json::json!([]),
    );
    assert_rejects(
        &CommoditySwap::example().expect("example"),
        "realized_fixings",
        serde_json::json!([]),
    );

    let future = CommodityFuture::example().expect("example");
    let mut json = serde_json::to_value(&future).expect("serialize");
    json["fixing"]["realized_fixings"] = serde_json::json!([]);
    let err =
        serde_json::from_value::<CommodityFuture>(json).expect_err("realized_fixings retired");
    assert!(err.to_string().contains("realized_fixings"), "{err}");
}

#[test]
// schema-rejection-test: RangeAccrual `past_fixings_in_range`
fn range_accrual_retired_past_fixings_in_range_is_rejected() {
    assert_rejects(
        &RangeAccrual::example().expect("example"),
        "past_fixings_in_range",
        serde_json::json!(1),
    );
}

#[test]
// schema-rejection-test: Autocallable root `participation_rate` and `final_payoff_type.participation.rate`
fn autocallable_retired_participation_keys_are_rejected() {
    let note = Autocallable::example().expect("example");
    assert_rejects(&note, "participation_rate", serde_json::json!(1.0));

    let mut json = serde_json::to_value(&note).expect("serialize");
    json["final_payoff_type"] = serde_json::json!({"participation": {"rate": 1.0}});
    let err = serde_json::from_value::<Autocallable>(json).expect_err("rate retired");
    assert!(err.to_string().contains("rate"), "{err}");
}

#[test]
fn fx_barrier_rebate_is_a_quote_currency_total() {
    let mut option = FxBarrierOption::example().expect("example");
    option.rebate = Some(Money::from((20_000_i64, Currency::USD)));
    option.validate().expect("quote-currency rebate is valid");

    option.rebate = Some(Money::from((20_000_i64, Currency::EUR)));
    let err = option.validate().expect_err("base-currency rebate");
    assert!(err.to_string().contains("Currency mismatch"), "{err}");

    option.rebate = Some(Money::from((-1_i64, Currency::USD)));
    option
        .validate()
        .expect_err("negative rebate must be rejected");
}

fn lookback_market(as_of: Date) -> MarketContext {
    let rate: f64 = 0.05;
    let vol = 0.20;
    MarketContext::new()
        .insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (10.0, (-rate * 10.0).exp())])
                .build()
                .expect("discount curve"),
        )
        .insert_price(
            "SPX-SPOT",
            MarketScalar::Price(Money::new(100.0, Currency::USD).expect("spot")),
        )
        .insert_price("SPX-DIV", MarketScalar::Unitless(0.0))
        .insert_surface(
            VolSurface::builder("SPX-VOL")
                .expiries(&[0.5, 1.0, 2.0])
                .strikes(&[80.0, 100.0, 120.0])
                .row(&[vol, vol, vol])
                .row(&[vol, vol, vol])
                .row(&[vol, vol, vol])
                .build()
                .expect("vol surface"),
        )
}

fn lookback(lookback_type: LookbackType, expiry: Date, monitoring: Monitoring) -> LookbackOption {
    LookbackOption::builder()
        .id(InstrumentId::new("LOOKBACK-MONITORING"))
        .underlying_ticker("SPX".to_string())
        .strike_opt(match lookback_type {
            LookbackType::FixedStrike => Some(100.0),
            LookbackType::FloatingStrike => None,
        })
        .option_type(OptionType::Call)
        .lookback_type(lookback_type)
        .expiry(expiry)
        .quantity(1.0)
        .currency(Currency::USD)
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id("SPX-SPOT".into())
        .vol_surface_id(CurveId::new("SPX-VOL"))
        .div_yield_id_opt(Some(PriceId::new("SPX-DIV")))
        .monitoring(monitoring)
        .instrument_pricing_overrides(InstrumentPricingOverrides::default().with_mc_paths(20_000))
        .attributes(Attributes::new())
        .build()
        .expect("lookback option")
}

#[test]
fn discrete_lookback_observes_only_contractual_dates() {
    let as_of = Date::from_calendar_date(2024, Month::January, 2).expect("as_of");
    let expiry = Date::from_calendar_date(2025, Month::January, 2).expect("expiry");
    let market = lookback_market(as_of);
    let expiry_only = Monitoring::Discrete {
        observation_dates: vec![expiry],
    };

    // With the only observation at expiry, a floating-strike call pays
    // S_T - min(S_T) = 0 on every path: the value is exactly zero.
    let floating = lookback(LookbackType::FloatingStrike, expiry, expiry_only.clone());
    assert_eq!(floating.default_model(), ModelKey::MonteCarloGBM);
    assert_eq!(floating.value(&market, as_of).expect("pv").amount(), 0.0);

    // ... and a fixed-strike call pays max(S_T - K, 0): a European call.
    // Black-Scholes reference at S=K=100, r=5%, q=0, sigma=20%. The payoff
    // standard deviation is below 15, so with 20,000 paths the standard error
    // is below 0.11; 0.45 is a 4-sigma bound.
    let t = DayCount::Act365F
        .year_fraction(as_of, expiry, DayCountContext::default())
        .expect("year fraction");
    let black_scholes = finstack_quant_models::closed_form::vanilla::bs_price_unchecked(
        100.0,
        100.0,
        0.05,
        0.0,
        0.20,
        t,
        OptionType::Call,
    );
    let fixed = lookback(LookbackType::FixedStrike, expiry, expiry_only);
    let pv = fixed.value(&market, as_of).expect("pv").amount();
    assert!(
        (pv - black_scholes).abs() < 0.45,
        "expiry-only fixed lookback {pv} vs Black-Scholes {black_scholes}"
    );

    // Monthly observations sit between the European call and the
    // continuously monitored lookback.
    let monthly = Monitoring::Discrete {
        observation_dates: (2..=12)
            .map(|month| {
                Date::from_calendar_date(2024, Month::try_from(month).expect("month"), 2)
                    .expect("date")
            })
            .chain(std::iter::once(expiry))
            .collect(),
    };
    let monthly_pv = lookback(LookbackType::FixedStrike, expiry, monthly)
        .value(&market, as_of)
        .expect("pv")
        .amount();
    let continuous = lookback(LookbackType::FixedStrike, expiry, Monitoring::Continuous);
    assert_eq!(continuous.default_model(), ModelKey::LookbackBSContinuous);
    let continuous_pv = continuous.value(&market, as_of).expect("pv").amount();
    assert!(
        black_scholes + 1.0 < monthly_pv && monthly_pv + 1.0 < continuous_pv,
        "European {black_scholes} < monthly {monthly_pv} < continuous {continuous_pv}"
    );
}

#[test]
fn discrete_lookback_is_rejected_by_the_continuous_analytical_engine() {
    let as_of = Date::from_calendar_date(2024, Month::January, 2).expect("as_of");
    let expiry = Date::from_calendar_date(2025, Month::January, 2).expect("expiry");
    let option = lookback(
        LookbackType::FixedStrike,
        expiry,
        Monitoring::Discrete {
            observation_dates: vec![expiry],
        },
    );
    let err = option
        .price_with_metrics(
            &lookback_market(as_of),
            as_of,
            &[],
            PricingOptions::default().with_model(ModelKey::LookbackBSContinuous),
        )
        .expect_err("analytical engine must reject discrete monitoring");
    assert!(err.to_string().contains("continuous monitoring"), "{err}");
}
