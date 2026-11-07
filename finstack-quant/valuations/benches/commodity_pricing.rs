//! Commodity instrument pricing benchmarks.
//!
//! Covers:
//! - [`CommodityForward`]: outright forward PV vs tenor.
//! - [`CommoditySwap`]: fixed-float swap with monthly resets vs tenor.
//! - [`CommoditySpotOption`]: Black-76 vanilla option vs tenor.
//! - [`CommodityAsianOption`]: arithmetic/geometric averaging (observation count hot path).
//! - [`CommoditySpreadOption`]: Kirk's approximation for two-leg spreads.
//! - [`CommoditySwaption`]: option on commodity swap via Black-76 swaption model.

#![allow(clippy::unwrap_used)]

#[path = "support/record.rs"]
mod record;
mod support;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor, TenorUnit};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod;
use finstack_quant_models::types::{ExerciseStyle, OptionType};
use finstack_quant_valuations::instruments::CommodityUnderlyingParams;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::Position;
use finstack_quant_valuations::instruments::commodity::commodity_asian_option::CommodityAsianOption;
use finstack_quant_valuations::instruments::commodity::commodity_forward::CommodityForward;
use finstack_quant_valuations::instruments::commodity::commodity_spot_option::CommoditySpotOption;
use finstack_quant_valuations::instruments::commodity::commodity_spread_option::CommoditySpreadOption;
use finstack_quant_valuations::instruments::commodity::commodity_swap::CommoditySwap;
use finstack_quant_valuations::instruments::commodity::commodity_swaption::CommoditySwaption;
use finstack_quant_valuations::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations::instruments::{PayReceive, PricingOptions, SettlementType};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::ModelKey;
use finstack_quant_valuations_test_support::commodity_curves as commodity_curve_support;
use finstack_quant_valuations_test_support::volatility as volatility_support;
use std::hint::black_box;
use time::Month;

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

/// USD-OIS discount, WTI/CL/NG/RBOB forward curves, and flat vol surfaces.
///
/// Extended to support all commodity bench scenarios including spread and swaption.
fn create_commodity_market() -> MarketContext {
    let as_of = base_date();

    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([
            (0.0, 1.0),
            (0.5, 0.975),
            (1.0, 0.95),
            (2.0, 0.90),
            (5.0, 0.78),
            (10.0, 0.60),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let wti_curve =
        commodity_curve_support::contango_price_curve("WTI-FWD", as_of, 75.0, 0.05, 5.0);
    let cl_curve =
        commodity_curve_support::contango_price_curve("CL-FORWARD", as_of, 75.0, 0.05, 5.0);
    let ng_curve =
        commodity_curve_support::contango_price_curve("NG-FORWARD", as_of, 3.5, 0.02, 5.0);
    let rbob_curve =
        commodity_curve_support::contango_price_curve("RBOB-FORWARD", as_of, 90.0, 0.04, 5.0);

    let wti_vol = volatility_support::flat_vol_surface(
        "WTI-VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[50.0, 60.0, 70.0, 80.0, 90.0],
        0.30,
    );
    let cl_vol = volatility_support::flat_vol_surface(
        "CL-VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[50.0, 60.0, 70.0, 80.0, 90.0],
        0.30,
    );
    let ng_vol = volatility_support::flat_vol_surface(
        "NG-VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[2.0, 3.0, 4.0, 5.0, 6.0],
        0.35,
    );
    let rbob_vol = volatility_support::flat_vol_surface(
        "RBOB-VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[60.0, 75.0, 90.0, 105.0, 120.0],
        0.32,
    );

    MarketContext::new()
        .insert(discount_curve)
        .insert(wti_curve)
        .insert(cl_curve)
        .insert(ng_curve)
        .insert(rbob_curve)
        .insert_surface(wti_vol)
        .insert_surface(cl_vol)
        .insert_surface(ng_vol)
        .insert_surface(rbob_vol)
}

fn wti_underlying() -> CommodityUnderlyingParams {
    CommodityUnderlyingParams::new("Energy", "CL", "BBL", Currency::USD)
}

fn forward_contract(label: &str, maturity: Date) -> CommodityForward {
    CommodityForward::builder()
        .id(InstrumentId::new(label))
        .underlying(wti_underlying())
        .quantity(1000.0)
        .multiplier(1.0)
        .maturity(maturity)
        .position(Position::Long)
        .contract_price_opt(Some(72.0))
        .forward_curve_id(CurveId::new("WTI-FWD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::default())
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .build()
        .expect("commodity forward for benchmark")
}

fn commodity_swap_contract(label: &str, end: Date) -> CommoditySwap {
    let start = base_date();
    CommoditySwap::builder()
        .id(InstrumentId::new(label))
        .underlying(wti_underlying())
        .quantity(10_000.0)
        .fixed_price(75.0)
        .forward_curve_id(CurveId::new("WTI-FWD"))
        .side(PayReceive::Pay)
        .start_date(start)
        .maturity(end)
        .frequency(Tenor::new(1, TenorUnit::Months).expect("valid tenor fixture"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::default())
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .build()
        .expect("commodity swap for benchmark")
}

fn commodity_call_option(label: &str, expiry: Date) -> CommoditySpotOption {
    CommoditySpotOption::builder()
        .id(InstrumentId::new(label))
        .underlying(wti_underlying())
        .strike(75.0)
        .option_type(OptionType::Call)
        .exercise_style(ExerciseStyle::European)
        .expiry(expiry)
        .quantity(1000.0)
        .multiplier(1.0)
        .settlement(SettlementType::Cash)
        .forward_curve_id(CurveId::new("WTI-FWD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("WTI-VOL"))
        .day_count(DayCount::Act365F)
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::default())
        .build()
        .expect("commodity option for benchmark")
}

fn bench_commodity_forward_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("commodity_forward_pv");
    group.bench_function("6M", |b| {
        let market = create_commodity_market();
        let as_of = base_date();
        let fwd = forward_contract(
            "WTI-FWD-6M",
            Date::from_calendar_date(2025, Month::July, 1).unwrap(),
        );
        assert_eq!(fwd.default_model(), ModelKey::Discounting);
        assert!(fwd.maturity > as_of);
        support::bench(
            b,
            || {
                fwd.value(black_box(&market), black_box(as_of))
                    .expect("commodity forward pricing must succeed")
            },
            |value| {
                assert!(value.amount().is_finite());
                assert_eq!(value.currency(), Currency::USD);
                record::capture("commodity_forward_pv/6M", || {
                    serde_json::json!({
                        "fixture": {
                            "instrument": &fwd,
                            "market": &market,
                            "as_of": as_of.to_string(),
                            "model": fwd.default_model(),
                            "requested_metrics": [],
                            "dimensions": {
                                "tenor_months": 6,
                                "days_to_maturity": (fwd.maturity - as_of).whole_days()
                            }
                        },
                        "output": { "value": value.amount(), "currency": value.currency() }
                    })
                });
            },
        );
    });
    group.finish();
}

fn bench_commodity_swap_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("commodity_swap_pv");
    group.bench_function("2Y", |b| {
        let market = create_commodity_market();
        let as_of = base_date();
        let swap = commodity_swap_contract(
            "WTI-SWAP-2Y",
            Date::from_calendar_date(2027, Month::January, 1).unwrap(),
        );
        assert_eq!(swap.default_model(), ModelKey::Discounting);
        assert_eq!(swap.frequency, Tenor::monthly());
        assert!(swap.maturity > as_of);
        support::bench(
            b,
            || {
                swap.value(black_box(&market), black_box(as_of))
                    .expect("commodity swap pricing must succeed")
            },
            |value| {
                assert!(value.amount().is_finite());
                assert_eq!(value.currency(), Currency::USD);
                record::capture("commodity_swap_pv/2Y", || {
                    serde_json::json!({
                        "fixture": {
                            "instrument": &swap,
                            "market": &market,
                            "as_of": as_of.to_string(),
                            "model": swap.default_model(),
                            "requested_metrics": [],
                            "dimensions": {
                                "tenor_years": 2,
                                "days_to_maturity": (swap.maturity - as_of).whole_days(),
                                "payment_frequency_months": 1
                            }
                        },
                        "output": { "value": value.amount(), "currency": value.currency() }
                    })
                });
            },
        );
    });
    group.finish();
}

fn bench_commodity_spot_option_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("commodity_spot_option_pv");
    group.bench_function("6M", |b| {
        let market = create_commodity_market();
        let as_of = base_date();
        let opt = commodity_call_option(
            "WTI-OPT-6M",
            Date::from_calendar_date(2025, Month::July, 1).unwrap(),
        );
        assert_eq!(opt.default_model(), ModelKey::Black76);
        assert_eq!(opt.exercise_style, ExerciseStyle::European);
        assert!(opt.expiry > as_of);
        support::bench(
            b,
            || {
                opt.value(black_box(&market), black_box(as_of))
                    .expect("commodity spot option pricing must succeed")
            },
            |value| {
                assert!(value.amount().is_finite());
                assert_eq!(value.currency(), Currency::USD);
                record::capture("commodity_spot_option_pv/6M", || {
                    serde_json::json!({
                        "fixture": {
                            "instrument": &opt,
                            "market": &market,
                            "as_of": as_of.to_string(),
                            "model": opt.default_model(),
                            "requested_metrics": [],
                            "dimensions": {
                                "tenor_months": 6,
                                "days_to_expiry": (opt.expiry - as_of).whole_days()
                            }
                        },
                        "output": { "value": value.amount(), "currency": value.currency() }
                    })
                });
            },
        );
    });
    group.finish();
}

fn bench_commodity_spot_option_greeks(c: &mut Criterion) {
    let mut group = c.benchmark_group("commodity_spot_option_greeks");
    group.bench_function("6M", |b| {
        let market = create_commodity_market();
        let as_of = base_date();
        let opt_6m = commodity_call_option(
            "WTI-OPT-6M-GREEKS",
            Date::from_calendar_date(2025, Month::July, 1).unwrap(),
        );
        let greeks = vec![
            MetricId::Delta,
            MetricId::Gamma,
            MetricId::Vega,
            MetricId::Theta,
        ];
        assert_eq!(opt_6m.default_model(), ModelKey::Black76);
        assert_eq!(opt_6m.exercise_style, ExerciseStyle::European);
        assert!(opt_6m.expiry > as_of);
        support::bench(
            b,
            || {
                opt_6m
                    .price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&greeks),
                        PricingOptions::default(),
                    )
                    .expect("commodity spot option Greeks must succeed")
            },
            |result| {
                assert!(result.value.amount().is_finite());
                assert_eq!(result.value.currency(), Currency::USD);
                for greek in &greeks {
                    assert!(
                        result
                            .measures
                            .get(greek)
                            .is_some_and(|value| value.is_finite()),
                        "missing or non-finite {greek}"
                    );
                }
                record::capture("commodity_spot_option_greeks/6M", || {
                    serde_json::json!({
                        "fixture": {
                            "instrument": &opt_6m,
                            "market": &market,
                            "as_of": as_of.to_string(),
                            "model": opt_6m.default_model(),
                            "requested_metrics": &greeks,
                            "dimensions": {
                                "tenor_months": 6,
                                "days_to_expiry": (opt_6m.expiry - as_of).whole_days()
                            }
                        },
                        "output": {
                            "value": result.value.amount(),
                            "currency": result.value.currency(),
                            "measures": &result.measures
                        }
                    })
                });
            },
        );
    });
    group.finish();
}

// Asian option benchmarks

/// Benchmark commodity Asian pricing with twelve monthly fixing dates.
///
/// Measures Turnbull-Wakeman moment-matching for arithmetic averaging:
/// each additional fixing requires a forward price lookup and running moment update.
fn bench_commodity_asian_option_obs_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("commodity_asian_option/obs_count");
    let n_obs = 12_usize;
    group.bench_with_input(
        BenchmarkId::from_parameter(format!("{n_obs}obs")),
        &n_obs,
        |b, &n_obs| {
            let market = create_commodity_market();
            let as_of = base_date();
            // Generate `n_obs` monthly fixing dates starting 1 month from as_of
            let fixing_dates: Vec<Date> = (1..=n_obs)
                .map(|i| {
                    let months_ahead = i as i64;
                    as_of + time::Duration::days(months_ahead * 30)
                })
                .collect();
            let expiry = *fixing_dates.last().unwrap() + time::Duration::days(2);

            let asian = CommodityAsianOption::builder()
                .id(InstrumentId::new("CL-ASIAN-BENCH"))
                .underlying(CommodityUnderlyingParams::new(
                    "Energy",
                    "CL",
                    "BBL",
                    Currency::USD,
                ))
                .strike(75.0)
                .option_type(OptionType::Call)
                .averaging_method(AveragingMethod::Arithmetic)
                .fixing_dates(fixing_dates)
                .quantity(1000.0)
                .expiry(expiry)
                .forward_curve_id(CurveId::new("CL-FORWARD"))
                .discount_curve_id(CurveId::new("USD-OIS"))
                .vol_surface_id(CurveId::new("CL-VOL"))
                .day_count(DayCount::Act365F)
                .instrument_pricing_overrides(InstrumentPricingOverrides::default())
                .attributes(Attributes::default())
                .build()
                .unwrap();

            assert_eq!(asian.default_model(), ModelKey::AsianTurnbullWakeman);
            assert_eq!(asian.averaging_method, AveragingMethod::Arithmetic);
            assert_eq!(asian.fixing_dates.len(), n_obs);
            assert!(asian.fixing_dates.iter().all(|date| *date > as_of));
            support::bench(
                b,
                || {
                    asian
                        .value(black_box(&market), black_box(as_of))
                        .expect("commodity arithmetic Asian pricing must succeed")
                },
                |value| {
                    assert!(value.amount().is_finite());
                    assert_eq!(value.currency(), Currency::USD);
                    record::capture("commodity_asian_option/obs_count/12obs", || serde_json::json!({
                        "fixture": {
                            "instrument": &asian,
                            "market": &market,
                            "as_of": as_of.to_string(),
                            "model": asian.default_model(),
                            "requested_metrics": [],
                            "dimensions": {
                                "total_fixings": asian.fixing_dates.len(),
                                "realized_fixing_count": asian.fixing_dates.iter().filter(|date| **date <= as_of).count(),
                                "future_fixings": asian.fixing_dates.iter().filter(|date| **date > as_of).count(),
                                "averaging_method": asian.averaging_method,
                                "volatility": 0.30,
                                "fixing_horizon_days": 360,
                                "payment_lag_days": 2
                            }
                        },
                        "output": { "value": value.amount(), "currency": value.currency() }
                    }));
                },
            );
        },
    );
    group.finish();
}

/// Compare arithmetic vs geometric Asian option pricing.
fn bench_commodity_asian_option_method(c: &mut Criterion) {
    let mut group = c.benchmark_group("commodity_asian_option/method");
    for (name, method) in [
        ("arithmetic", AveragingMethod::Arithmetic),
        ("geometric", AveragingMethod::Geometric),
    ] {
        group.bench_with_input(BenchmarkId::from_parameter(name), &method, |b, &method| {
            let market = create_commodity_market();
            let as_of = base_date();
            let fixing_dates: Vec<Date> = (1..=12_i64)
                .map(|i| as_of + time::Duration::days(i * 30))
                .collect();
            let expiry = *fixing_dates.last().unwrap() + time::Duration::days(2);
            let asian = CommodityAsianOption::builder()
                .id(InstrumentId::new("CL-ASIAN-BENCH"))
                .underlying(CommodityUnderlyingParams::new(
                    "Energy",
                    "CL",
                    "BBL",
                    Currency::USD,
                ))
                .strike(75.0)
                .option_type(OptionType::Call)
                .averaging_method(method)
                .fixing_dates(fixing_dates)
                .quantity(1000.0)
                .expiry(expiry)
                .forward_curve_id(CurveId::new("CL-FORWARD"))
                .discount_curve_id(CurveId::new("USD-OIS"))
                .vol_surface_id(CurveId::new("CL-VOL"))
                .day_count(DayCount::Act365F)
                .instrument_pricing_overrides(InstrumentPricingOverrides::default())
                .attributes(Attributes::default())
                .build()
                .unwrap();

            assert_eq!(asian.default_model(), ModelKey::AsianTurnbullWakeman);
            assert_eq!(asian.averaging_method, method);
            assert_eq!(asian.fixing_dates.len(), 12);
            assert!(asian.fixing_dates.iter().all(|date| *date > as_of));
            support::bench(
                b,
                || {
                    asian
                        .value(black_box(&market), black_box(as_of))
                        .expect("commodity Asian pricing must succeed")
                },
                |value| {
                    assert!(value.amount().is_finite());
                    assert_eq!(value.currency(), Currency::USD);
                    record::capture(&format!("commodity_asian_option/method/{name}"), || serde_json::json!({
                        "fixture": {
                            "instrument": &asian,
                            "market": &market,
                            "as_of": as_of.to_string(),
                            "model": asian.default_model(),
                            "requested_metrics": [],
                            "dimensions": {
                                "total_fixings": asian.fixing_dates.len(),
                                "realized_fixing_count": asian.fixing_dates.iter().filter(|date| **date <= as_of).count(),
                                "future_fixings": asian.fixing_dates.iter().filter(|date| **date > as_of).count(),
                                "averaging_method": asian.averaging_method,
                                "volatility": 0.30,
                                "fixing_horizon_days": 360,
                                "payment_lag_days": 2
                            }
                        },
                        "output": { "value": value.amount(), "currency": value.currency() }
                    }));
                },
            );
        });
    }
    group.finish();
}

// Spread option benchmarks

/// Benchmark crack spread option (WTI vs RBOB) using Kirk's approximation.
///
/// Measures Kirk's approximation: two forward lookups, vol surface interpolation
/// per leg, and correlation-adjusted effective volatility computation.
fn bench_commodity_spread_option_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("commodity_spread_option/tenor");
    group.bench_function("6M", |b| {
        let market = create_commodity_market();
        let as_of = base_date();
        let months = 6_i64;
        let expiry = as_of + time::Duration::days(months * 30);
        let spread_opt = CommoditySpreadOption::builder()
            .id(InstrumentId::new("CRACK-SPREAD-BENCH"))
            .currency(Currency::USD)
            .option_type(OptionType::Call)
            .expiry(expiry)
            .strike(10.0)
            .quantity(1000.0)
            .leg1_forward_curve_id(CurveId::new("RBOB-FORWARD"))
            .leg2_forward_curve_id(CurveId::new("WTI-FWD"))
            .leg1_vol_surface_id(CurveId::new("RBOB-VOL"))
            .leg2_vol_surface_id(CurveId::new("WTI-VOL"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .correlation(0.85)
            .day_count(DayCount::Act365F)
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
            .attributes(Attributes::default())
            .build()
            .unwrap();

        assert_eq!(spread_opt.default_model(), ModelKey::Black76);
        assert!(spread_opt.expiry > as_of);
        support::bench(
            b,
            || {
                spread_opt
                    .value(black_box(&market), black_box(as_of))
                    .expect("commodity spread option pricing must succeed")
            },
            |value| {
                assert!(value.amount().is_finite());
                assert_eq!(value.currency(), Currency::USD);
                record::capture("commodity_spread_option/tenor/6M", || {
                    serde_json::json!({
                        "fixture": {
                            "instrument": &spread_opt,
                            "market": &market,
                            "as_of": as_of.to_string(),
                            "model": spread_opt.default_model(),
                            "requested_metrics": [],
                            "dimensions": {
                                "tenor_months": months,
                                "days_to_expiry": (spread_opt.expiry - as_of).whole_days()
                            }
                        },
                        "output": { "value": value.amount(), "currency": value.currency() }
                    })
                });
            },
        );
    });
    group.finish();
}

// Swaption benchmarks

/// Benchmark commodity swaption pricing with a two-year underlying swap.
///
/// Measures Black-76 swaption model: swap payment schedule generation,
/// weighted-average forward rate computation, and discount factor chain.
fn bench_commodity_swaption_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("commodity_swaption/swap_tenor");
    group.bench_function("2Y", |b| {
        let market = create_commodity_market();
        let as_of = base_date();
        let swap_years = 2_i32;
        let expiry = Date::from_calendar_date(as_of.year() + 1, as_of.month(), 1).unwrap();
        let swap_start = expiry + time::Duration::days(2);
        let swap_end =
            Date::from_calendar_date(as_of.year() + 1 + swap_years, as_of.month(), 1).unwrap();

        let swaption = CommoditySwaption::builder()
            .id(InstrumentId::new("NG-SWAPTION-BENCH"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "NG",
                "MMBTU",
                Currency::USD,
            ))
            .option_type(OptionType::Call)
            .expiry(expiry)
            .underlying_start_date(swap_start)
            .underlying_maturity(swap_end)
            .swap_frequency(Tenor::new(1, TenorUnit::Months).expect("valid tenor fixture"))
            .fixed_price(3.50)
            .quantity(10_000.0)
            .forward_curve_id(CurveId::new("NG-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("NG-VOL"))
            .day_count(DayCount::Act365F)
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
            .attributes(Attributes::default())
            .build()
            .unwrap();

        assert_eq!(swaption.default_model(), ModelKey::Black76);
        assert!(swaption.expiry > as_of);
        assert!(swaption.underlying_start_date > swaption.expiry);
        support::bench(
            b,
            || {
                swaption
                    .value(black_box(&market), black_box(as_of))
                    .expect("commodity swaption pricing must succeed")
            },
            |value| {
                assert!(value.amount().is_finite());
                assert_eq!(value.currency(), Currency::USD);
                record::capture("commodity_swaption/swap_tenor/2Y", || {
                    serde_json::json!({
                        "fixture": {
                            "instrument": &swaption,
                            "market": &market,
                            "as_of": as_of.to_string(),
                            "model": swaption.default_model(),
                            "requested_metrics": [],
                            "dimensions": {
                                "underlying_tenor_years": swap_years,
                                "days_to_expiry": (swaption.expiry - as_of).whole_days(),
                                "underlying_tenor_days": (swap_end - swap_start).whole_days(),
                                "payment_frequency_months": 1
                            }
                        },
                        "output": { "value": value.amount(), "currency": value.currency() }
                    })
                });
            },
        );
    });
    group.finish();
}

/// Keep the new fixing-count axis on one horizon with distinct calendar dates.
fn matrix_asian(method: AveragingMethod, n_obs: usize) -> CommodityAsianOption {
    assert!(matches!(n_obs, 12 | 60 | 252 | 1260));
    let base = base_date();
    let spacing_days = 1260 / n_obs;
    let fixing_dates = (1..=n_obs)
        .map(|index| base + time::Duration::days((spacing_days * index) as i64))
        .collect();
    CommodityAsianOption::builder()
        .id(InstrumentId::new("CL-ASIAN-MATRIX"))
        .underlying(wti_underlying())
        .strike(75.0)
        .option_type(OptionType::Call)
        .averaging_method(method)
        .fixing_dates(fixing_dates)
        .quantity(1000.0)
        .expiry(base + time::Duration::days(1262))
        .forward_curve_id(CurveId::new("CL-FORWARD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("CL-VOL"))
        .day_count(DayCount::Act365F)
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::default())
        .build()
        .expect("commodity Asian matrix fixture")
}

/// Cover the fixed matrix expiry without changing the original market fixtures.
fn matrix_market(volatility: f64, flat_forward: bool) -> MarketContext {
    let surface = volatility_support::flat_vol_surface(
        "CL-VOL",
        &[0.25, 0.5, 1.0, 2.0, 5.0],
        &[50.0, 60.0, 70.0, 80.0, 90.0],
        volatility,
    );
    let market = create_commodity_market().insert_surface(surface);
    if flat_forward {
        market.insert(commodity_curve_support::flat_price_curve(
            "CL-FORWARD",
            base_date(),
            75.0,
            5.0,
        ))
    } else {
        market
    }
}

/// Isolate observation count at a fixed 1,260-calendar-day fixing horizon.
fn bench_commodity_asian_fixing_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("commodity_asian_option/fixing_count");
    for (name, method) in [
        ("arithmetic", AveragingMethod::Arithmetic),
        ("geometric", AveragingMethod::Geometric),
    ] {
        for n_obs in [12_usize, 60, 252, 1260] {
            group.bench_with_input(BenchmarkId::new(name, n_obs), &n_obs, |b, &n_obs| {
                let as_of = base_date();
                let market = matrix_market(0.30, false);
                let asian = matrix_asian(method, n_obs);
                assert_eq!(asian.default_model(), ModelKey::AsianTurnbullWakeman);
                assert_eq!(asian.averaging_method, method);
                assert_eq!(asian.fixing_dates.len(), n_obs);
                assert!(asian.past_fixings.is_empty());
                assert!(asian.fixing_dates.iter().all(|date| *date > as_of));
                assert_eq!(asian.fixing_dates.last().copied(), Some(as_of + time::Duration::days(1260)));
                assert!(asian.instrument_pricing_overrides.market_quotes.implied_volatility.is_none());
                support::bench(
                    b,
                    || {
                        asian
                            .value(black_box(&market), black_box(as_of))
                            .expect("commodity Asian fixing-count pricing must succeed")
                    },
                    |value| {
                        assert!(value.amount().is_finite() && value.amount() >= 0.0);
                        assert_eq!(value.currency(), Currency::USD);
                        record::capture(
                            &format!("commodity_asian_option/fixing_count/{name}/{n_obs}"),
                            || serde_json::json!({
                                "fixture": {
                                    "instrument": &asian,
                                    "market": &market,
                                    "as_of": as_of.to_string(),
                                    "model": asian.default_model(),
                                    "requested_metrics": [],
                                    "dimensions": {
                                        "total_fixings": n_obs,
                                        "realized_fixing_count": 0,
                                        "future_fixings": n_obs,
                                        "averaging_method": method,
                                        "volatility": 0.30,
                                        "fixing_horizon_days": 1260,
                                        "fixing_spacing_days": 1260 / n_obs,
                                        "payment_lag_days": 2,
                                        "forward_shape": "linear_contango"
                                    }
                                },
                                "output": { "value": value.amount(), "currency": value.currency() }
                            }),
                        );
                    },
                );
            });
        }
    }
    group.finish();
}

/// Include history validation and accumulation with half the observations realized.
fn bench_commodity_asian_seasoned(c: &mut Criterion) {
    let mut group = c.benchmark_group("commodity_asian_option/seasoned");
    for (name, method) in [
        ("arithmetic", AveragingMethod::Arithmetic),
        ("geometric", AveragingMethod::Geometric),
    ] {
        let label = format!("{name}_252obs_126realized");
        group.bench_function(label.as_str(), |b| {
            let as_of = base_date() + time::Duration::days(630);
            let market = matrix_market(0.30, false);
            let mut asian = matrix_asian(method, 252);
            asian.past_fixings = asian
                .fixing_dates
                .iter()
                .take(126)
                .enumerate()
                .map(|(index, &date)| (date, 75.0 + 0.02 * (index + 1) as f64))
                .collect();
            assert_eq!(asian.default_model(), ModelKey::AsianTurnbullWakeman);
            assert_eq!(asian.averaging_method, method);
            assert_eq!(asian.fixing_dates.len(), 252);
            assert_eq!(asian.past_fixings.len(), 126);
            assert_eq!(
                asian.past_fixings.last().map(|(date, _)| *date),
                Some(as_of)
            );
            assert_eq!(
                asian
                    .fixing_dates
                    .iter()
                    .filter(|date| **date > as_of)
                    .count(),
                126
            );
            assert!(asian.past_fixings.iter().all(|(_, value)| *value > 0.0));
            let historical_sum: f64 = asian.past_fixings.iter().map(|(_, value)| value).sum();
            assert!(252.0 * asian.strike - historical_sum > 0.0);
            assert!(
                asian
                    .instrument_pricing_overrides
                    .market_quotes
                    .implied_volatility
                    .is_none()
            );
            support::bench(
                b,
                || {
                    asian
                        .value(black_box(&market), black_box(as_of))
                        .expect("seasoned commodity Asian pricing must succeed")
                },
                |value| {
                    assert!(value.amount().is_finite() && value.amount() >= 0.0);
                    assert_eq!(value.currency(), Currency::USD);
                    record::capture(&format!("commodity_asian_option/seasoned/{label}"), || {
                        serde_json::json!({
                            "fixture": {
                                "instrument": &asian,
                                "market": &market,
                                "as_of": as_of.to_string(),
                                "model": asian.default_model(),
                                "requested_metrics": [],
                                "dimensions": {
                                    "total_fixings": 252,
                                    "realized_fixing_count": 126,
                                    "future_fixings": 126,
                                    "averaging_method": method,
                                    "volatility": 0.30,
                                    "fixing_horizon_days": 1260,
                                    "remaining_fixing_horizon_days": 630,
                                    "fixing_spacing_days": 5,
                                    "payment_lag_days": 2,
                                    "forward_shape": "linear_contango"
                                }
                            },
                            "output": { "value": value.amount(), "currency": value.currency() }
                        })
                    });
                },
            );
        });
    }
    group.finish();
}

/// Keep zero/small-volatility controls off ATM while retaining normal market lookup.
fn bench_commodity_asian_volatility(c: &mut Criterion) {
    let mut group = c.benchmark_group("commodity_asian_option/volatility");
    for (name, method) in [
        ("arithmetic", AveragingMethod::Arithmetic),
        ("geometric", AveragingMethod::Geometric),
    ] {
        for (vol_name, volatility) in [("zero", 0.0), ("small", 1.0e-5)] {
            let label = format!("{name}_252obs_{vol_name}");
            group.bench_function(label.as_str(), |b| {
                let as_of = base_date();
                let market = matrix_market(volatility, true);
                let mut asian = matrix_asian(method, 252);
                asian.strike = 70.0;
                assert_eq!(asian.default_model(), ModelKey::AsianTurnbullWakeman);
                assert_eq!(asian.averaging_method, method);
                assert_eq!(asian.fixing_dates.len(), 252);
                assert!(asian.past_fixings.is_empty());
                assert!(asian.fixing_dates.iter().all(|date| *date > as_of));
                assert!(asian.instrument_pricing_overrides.market_quotes.implied_volatility.is_none());
                support::bench(
                    b,
                    || {
                        asian
                            .value(black_box(&market), black_box(as_of))
                            .expect("zero/small-volatility commodity Asian pricing must succeed")
                    },
                    |value| {
                        assert!(value.amount().is_finite() && value.amount() >= 0.0);
                        assert_eq!(value.currency(), Currency::USD);
                        record::capture(
                            &format!("commodity_asian_option/volatility/{label}"),
                            || serde_json::json!({
                                "fixture": {
                                    "instrument": &asian,
                                    "market": &market,
                                    "as_of": as_of.to_string(),
                                    "model": asian.default_model(),
                                    "requested_metrics": [],
                                    "dimensions": {
                                        "total_fixings": 252,
                                        "realized_fixing_count": 0,
                                        "future_fixings": 252,
                                        "averaging_method": method,
                                        "volatility": volatility,
                                        "fixing_horizon_days": 1260,
                                        "fixing_spacing_days": 5,
                                        "payment_lag_days": 2,
                                        "forward_shape": "flat"
                                    }
                                },
                                "output": { "value": value.amount(), "currency": value.currency() }
                            }),
                        );
                    },
                );
            });
        }
    }
    group.finish();
}

criterion_group!(
    commodity_benches,
    bench_commodity_forward_pv,
    bench_commodity_swap_pv,
    bench_commodity_spot_option_pv,
    bench_commodity_spot_option_greeks,
    bench_commodity_asian_option_obs_count,
    bench_commodity_asian_option_method,
    bench_commodity_spread_option_pv,
    bench_commodity_swaption_pv,
    bench_commodity_asian_fixing_count,
    bench_commodity_asian_seasoned,
    bench_commodity_asian_volatility,
);
criterion_main!(commodity_benches);
