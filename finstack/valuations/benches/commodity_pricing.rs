//! Commodity instrument pricing benchmarks.
//!
//! Covers:
//! - [`CommodityForward`]: outright forward PV vs tenor.
//! - [`CommoditySwap`]: fixed-float swap with monthly resets vs tenor.
//! - [`CommodityOption`]: Black-76 vanilla option vs tenor.
//! - [`CommodityAsianOption`]: arithmetic/geometric averaging (observation count hot path).
//! - [`CommoditySpreadOption`]: Kirk's approximation for two-leg spreads.
//! - [`CommoditySwaption`]: option on commodity swap via Black-76 swaption model.

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, Tenor, TenorUnit};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::interp::InterpStyle;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_valuations::instruments::commodity::commodity_asian_option::CommodityAsianOption;
use finstack_valuations::instruments::commodity::commodity_forward::{CommodityForward, Position};
use finstack_valuations::instruments::commodity::commodity_option::CommodityOption;
use finstack_valuations::instruments::commodity::commodity_spread_option::CommoditySpreadOption;
use finstack_valuations::instruments::commodity::commodity_swap::CommoditySwap;
use finstack_valuations::instruments::commodity::commodity_swaption::CommoditySwaption;
use finstack_valuations::instruments::exotics::asian_option::AveragingMethod;
use finstack_valuations::instruments::CommodityUnderlyingParams;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::instruments::{
    Attributes, ExerciseStyle, OptionType, PayReceive, PricingOptions, PricingOverrides,
    SettlementType,
};
use finstack_valuations::metrics::MetricId;
#[allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]
mod test_utils {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/test_utils.rs"
    ));
}
use rust_decimal::Decimal;
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

    let wti_curve = test_utils::contango_price_curve("WTI-FWD", as_of, 75.0, 0.05, 5.0);
    let cl_curve = test_utils::contango_price_curve("CL-FORWARD", as_of, 75.0, 0.05, 5.0);
    let ng_curve = test_utils::contango_price_curve("NG-FORWARD", as_of, 3.5, 0.02, 5.0);
    let rbob_curve = test_utils::contango_price_curve("RBOB-FORWARD", as_of, 90.0, 0.04, 5.0);

    let wti_vol = test_utils::flat_vol_surface(
        "WTI-VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[50.0, 60.0, 70.0, 80.0, 90.0],
        0.30,
    );
    let cl_vol = test_utils::flat_vol_surface(
        "CL-VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[50.0, 60.0, 70.0, 80.0, 90.0],
        0.30,
    );
    let ng_vol = test_utils::flat_vol_surface(
        "NG-VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[2.0, 3.0, 4.0, 5.0, 6.0],
        0.35,
    );
    let rbob_vol = test_utils::flat_vol_surface(
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
        .pricing_overrides(PricingOverrides::default())
        .build()
        .expect("commodity forward for benchmark")
}

fn commodity_swap_contract(label: &str, end: Date) -> CommoditySwap {
    let start = base_date();
    CommoditySwap::builder()
        .id(InstrumentId::new(label))
        .underlying(wti_underlying())
        .quantity(10_000.0)
        .fixed_price(Decimal::try_from(75.0).expect("fixed price"))
        .floating_index_id(CurveId::new("WTI-FWD"))
        .side(PayReceive::Pay)
        .start_date(start)
        .maturity(end)
        .frequency(Tenor::new(1, TenorUnit::Months))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::default())
        .pricing_overrides(PricingOverrides::default())
        .build()
        .expect("commodity swap for benchmark")
}

fn commodity_call_option(label: &str, expiry: Date) -> CommodityOption {
    CommodityOption::builder()
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
        .pricing_overrides(PricingOverrides::default())
        .attributes(Attributes::default())
        .build()
        .expect("commodity option for benchmark")
}

fn bench_commodity_forward_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("commodity_forward_pv");
    let market = create_commodity_market();
    let as_of = base_date();

    let fwd_3m = forward_contract(
        "WTI-FWD-3M",
        Date::from_calendar_date(2025, Month::April, 1).unwrap(),
    );
    let fwd_6m = forward_contract(
        "WTI-FWD-6M",
        Date::from_calendar_date(2025, Month::July, 1).unwrap(),
    );
    let fwd_1y = forward_contract(
        "WTI-FWD-1Y",
        Date::from_calendar_date(2026, Month::January, 1).unwrap(),
    );

    for (label, fwd) in [("3M", &fwd_3m), ("6M", &fwd_6m), ("1Y", &fwd_1y)] {
        group.bench_with_input(BenchmarkId::from_parameter(label), fwd, |b, fwd| {
            b.iter(|| fwd.value(black_box(&market), black_box(as_of)));
        });
    }
    group.finish();
}

fn bench_commodity_swap_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("commodity_swap_pv");
    let market = create_commodity_market();
    let as_of = base_date();

    let swap_1y = commodity_swap_contract(
        "WTI-SWAP-1Y",
        Date::from_calendar_date(2026, Month::January, 1).unwrap(),
    );
    let swap_2y = commodity_swap_contract(
        "WTI-SWAP-2Y",
        Date::from_calendar_date(2027, Month::January, 1).unwrap(),
    );
    let swap_5y = commodity_swap_contract(
        "WTI-SWAP-5Y",
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
    );

    for (label, swap) in [("1Y", &swap_1y), ("2Y", &swap_2y), ("5Y", &swap_5y)] {
        group.bench_with_input(BenchmarkId::from_parameter(label), swap, |b, swap| {
            b.iter(|| swap.value(black_box(&market), black_box(as_of)));
        });
    }
    group.finish();
}

fn bench_commodity_option_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("commodity_option_pv");
    let market = create_commodity_market();
    let as_of = base_date();

    let opt_3m = commodity_call_option(
        "WTI-OPT-3M",
        Date::from_calendar_date(2025, Month::April, 1).unwrap(),
    );
    let opt_6m = commodity_call_option(
        "WTI-OPT-6M",
        Date::from_calendar_date(2025, Month::July, 1).unwrap(),
    );
    let opt_12m = commodity_call_option(
        "WTI-OPT-12M",
        Date::from_calendar_date(2026, Month::January, 1).unwrap(),
    );

    for (label, opt) in [("3M", &opt_3m), ("6M", &opt_6m), ("12M", &opt_12m)] {
        group.bench_with_input(BenchmarkId::from_parameter(label), opt, |b, opt| {
            b.iter(|| opt.value(black_box(&market), black_box(as_of)));
        });
    }
    group.finish();
}

fn bench_commodity_option_greeks(c: &mut Criterion) {
    let mut group = c.benchmark_group("commodity_option_greeks");
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

    group.bench_function("6M", |b| {
        b.iter(|| {
            opt_6m.price_with_metrics(
                black_box(&market),
                black_box(as_of),
                black_box(&greeks),
                PricingOptions::default(),
            )
        });
    });
    group.finish();
}

// ================================================================================================
// Asian option benchmarks
// ================================================================================================

/// Scale commodity Asian option pricing vs number of fixing dates (3, 6, 12, 24 obs).
///
/// Measures Turnbull-Wakeman moment-matching for arithmetic averaging:
/// each additional fixing requires a forward price lookup and running moment update.
fn bench_commodity_asian_option_obs_count(c: &mut Criterion) {
    let market = create_commodity_market();
    let as_of = base_date();

    let mut group = c.benchmark_group("commodity_asian_option/obs_count");
    for n_obs in [3_usize, 6, 12, 24] {
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
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::default())
            .build()
            .unwrap();

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{n_obs}obs")),
            &n_obs,
            |b, _| b.iter(|| black_box(asian.value(black_box(&market), black_box(as_of))).unwrap()),
        );
    }
    group.finish();
}

/// Compare arithmetic vs geometric Asian option pricing.
fn bench_commodity_asian_option_method(c: &mut Criterion) {
    let market = create_commodity_market();
    let as_of = base_date();

    let fixing_dates: Vec<Date> = (1..=12_i64)
        .map(|i| as_of + time::Duration::days(i * 30))
        .collect();
    let expiry = *fixing_dates.last().unwrap() + time::Duration::days(2);

    let mut group = c.benchmark_group("commodity_asian_option/method");
    for (name, method) in [
        ("arithmetic", AveragingMethod::Arithmetic),
        ("geometric", AveragingMethod::Geometric),
    ] {
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
            .fixing_dates(fixing_dates.clone())
            .quantity(1000.0)
            .expiry(expiry)
            .forward_curve_id(CurveId::new("CL-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("CL-VOL"))
            .day_count(DayCount::Act365F)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::default())
            .build()
            .unwrap();

        group.bench_with_input(BenchmarkId::from_parameter(name), &name, |b, _| {
            b.iter(|| black_box(asian.value(black_box(&market), black_box(as_of))).unwrap())
        });
    }
    group.finish();
}

// ================================================================================================
// Spread option benchmarks
// ================================================================================================

/// Benchmark crack spread option (WTI vs RBOB) using Kirk's approximation.
///
/// Measures Kirk's approximation: two forward lookups, vol surface interpolation
/// per leg, and correlation-adjusted effective volatility computation.
fn bench_commodity_spread_option_pv(c: &mut Criterion) {
    let market = create_commodity_market();
    let as_of = base_date();

    let mut group = c.benchmark_group("commodity_spread_option/tenor");
    for (label, months) in [("3M", 3_i64), ("6M", 6), ("12M", 12)] {
        let expiry = as_of + time::Duration::days(months * 30);
        let spread_opt = CommoditySpreadOption::builder()
            .id(InstrumentId::new("CRACK-SPREAD-BENCH"))
            .currency(Currency::USD)
            .option_type(OptionType::Call)
            .expiry(expiry)
            .strike(10.0)
            .notional(1000.0)
            .leg1_forward_curve_id(CurveId::new("RBOB-FORWARD"))
            .leg2_forward_curve_id(CurveId::new("WTI-FWD"))
            .leg1_vol_surface_id(CurveId::new("RBOB-VOL"))
            .leg2_vol_surface_id(CurveId::new("WTI-VOL"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .correlation(0.85)
            .day_count(DayCount::Act365F)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::default())
            .build()
            .unwrap();

        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            b.iter(|| black_box(spread_opt.value(black_box(&market), black_box(as_of))).unwrap())
        });
    }
    group.finish();
}

// ================================================================================================
// Swaption benchmarks
// ================================================================================================

/// Scale commodity swaption pricing vs underlying swap tenor (1Y, 2Y, 3Y).
///
/// Measures Black-76 swaption model: swap payment schedule generation,
/// weighted-average forward rate computation, and discount factor chain.
fn bench_commodity_swaption_pv(c: &mut Criterion) {
    let market = create_commodity_market();
    let as_of = base_date();

    let mut group = c.benchmark_group("commodity_swaption/swap_tenor");
    for (label, swap_years) in [("1Y", 1_i32), ("2Y", 2), ("3Y", 3)] {
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
            .swap_start(swap_start)
            .swap_end(swap_end)
            .swap_frequency(Tenor::new(1, TenorUnit::Months))
            .fixed_price(3.50)
            .notional(10_000.0)
            .forward_curve_id(CurveId::new("NG-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("NG-VOL"))
            .day_count(DayCount::Act365F)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::default())
            .build()
            .unwrap();

        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            b.iter(|| black_box(swaption.value(black_box(&market), black_box(as_of))).unwrap())
        });
    }
    group.finish();
}

criterion_group!(
    commodity_benches,
    bench_commodity_forward_pv,
    bench_commodity_swap_pv,
    bench_commodity_option_pv,
    bench_commodity_option_greeks,
    bench_commodity_asian_option_obs_count,
    bench_commodity_asian_option_method,
    bench_commodity_spread_option_pv,
    bench_commodity_swaption_pv,
);
criterion_main!(commodity_benches);
