//! Monte Carlo and barrier PDE exotic instrument pricing benchmarks.
//!
//! `AsianOption`'s `Instrument::value` uses Turnbull–Wakeman for arithmetic averages; the Asian group
//! calls `AsianOption::npv_mc` so timings reflect Monte Carlo paths controlled by `InstrumentPricingOverrides::with_mc_paths`.
//!
//! `CliquetOption` uses an internal GBM MC engine with step count driven by the number of reset dates.
//! Barrier cases price total at-hit rebates under continuous and quarterly monitoring.

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{BarrierType, CurveId, InstrumentId, PriceId};
use finstack_quant_models::closed_form::barrier::RebateTiming;
use finstack_quant_valuations::instruments::equity::autocallable::{Autocallable, FinalPayoffType};
use finstack_quant_valuations::instruments::equity::{CliquetOption, EquityPathModel};
use finstack_quant_valuations::instruments::exotics::lookback_option::{
    LookbackOption, LookbackType,
};
use finstack_quant_valuations::instruments::Attributes;
use finstack_quant_valuations::instruments::{
    AsianOption, AveragingMethod, BarrierOption, Instrument, InstrumentPricingOverrides,
    Monitoring, OptionType,
};
use finstack_quant_valuations::pricer::{
    standard_pricer_registry, InstrumentType, ModelKey, PricerKey,
};
use std::hint::black_box;
use time::Month;

#[allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]
#[path = "../tests/support/volatility.rs"]
mod volatility_support;

fn as_of() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

/// Equity MC context: USD-OIS discount, flat vol surface, spot scalar (see `tests/metrics/determinism.rs`).
fn create_mc_market(as_of: Date, spot: f64, vol: f64, rate: f64) -> MarketContext {
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0_f64, 1.0_f64),
            (1.0_f64, (-rate).exp()),
            (2.0_f64, (-rate * 2.0_f64).exp()),
        ])
        .build()
        .unwrap();

    let vol_surface = volatility_support::flat_vol_surface(
        "SPOT_VOL",
        &[0.5_f64, 1.0, 2.0],
        &[80.0_f64, 100.0, 120.0],
        vol,
    );

    MarketContext::new()
        .insert(disc_curve)
        .insert_surface(vol_surface)
        .insert_price("SPOT", MarketScalar::Unitless(spot))
        .insert_price("SPOT_DIV", MarketScalar::Unitless(0.0))
}

fn asian_option(mc_paths: usize) -> AsianOption {
    let expiry = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    AsianOption {
        id: "ASIAN-BENCH".into(),
        underlying_ticker: "SPOT".to_string(),
        spot_id: "SPOT".into(),
        strike: 100.0,
        option_type: OptionType::Call,
        expiry,
        notional: Money::new(1.0, Currency::USD).expect("valid money fixture"),
        averaging_method: AveragingMethod::Arithmetic,
        fixing_dates: vec![
            Date::from_calendar_date(2025, Month::July, 1).unwrap(),
            expiry,
        ],
        day_count: DayCount::Act365F,
        discount_curve_id: "USD-OIS".into(),
        vol_surface_id: "SPOT_VOL".into(),
        div_yield_id: None,
        instrument_pricing_overrides: InstrumentPricingOverrides::default().with_mc_paths(mc_paths),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
        past_fixings: vec![],
    }
}

fn lookback_option(mc_paths: usize) -> LookbackOption {
    let expiry = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    LookbackOption::builder()
        .id("LOOKBACK-BENCH".into())
        .underlying_ticker("SPOT".to_string())
        .strike_opt(Some(100.0))
        .option_type(OptionType::Call)
        .lookback_type(LookbackType::FixedStrike)
        .expiry(expiry)
        .notional(Money::new(1.0, Currency::USD).expect("valid money fixture"))
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id("SPOT".into())
        .vol_surface_id(CurveId::new("SPOT_VOL"))
        .div_yield_id_opt(Some(PriceId::new("SPOT_DIV")))
        .use_gobet_miri(true)
        .instrument_pricing_overrides(InstrumentPricingOverrides::default().with_mc_paths(mc_paths))
        .attributes(Attributes::new())
        .observed_max_opt(None)
        .build()
        .unwrap()
}

fn autocallable_note(mc_paths: usize) -> Autocallable {
    let observation_dates = vec![
        Date::from_calendar_date(2025, Month::April, 1).unwrap(),
        Date::from_calendar_date(2025, Month::July, 1).unwrap(),
        Date::from_calendar_date(2025, Month::October, 1).unwrap(),
        Date::from_calendar_date(2026, Month::January, 1).unwrap(),
    ];
    let n = observation_dates.len();
    Autocallable {
        id: "AUTO-BENCH".into(),
        underlying_ticker: "SPOT".into(),
        payment_dates: observation_dates.clone(),
        expiry: *observation_dates.last().unwrap(),
        observation_dates,
        autocall_barriers: vec![1.0; n],
        coupons: vec![0.02; n],
        coupon_barriers: vec![0.70; n],
        memory_coupons: false,
        final_barrier: 0.6,
        final_payoff_type: FinalPayoffType::Participation { rate: 1.0 },
        participation_rate: 1.0,
        cap_level: 1.5,
        notional: Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
        day_count: DayCount::Act365F,
        discount_curve_id: CurveId::new("USD-OIS"),
        spot_id: "SPOT".into(),
        vol_surface_id: CurveId::new("SPOT_VOL"),
        path_model: finstack_quant_valuations::instruments::equity::EquityPathModel::AtmTermGbm,
        div_yield_id: Some(finstack_quant_core::types::PriceId::new("SPOT_DIV")),
        initial_level: None,
        past_fixings: vec![],
        instrument_pricing_overrides: InstrumentPricingOverrides::default().with_mc_paths(mc_paths),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
    }
}

fn bench_asian_option_mc(c: &mut Criterion) {
    let mut group = c.benchmark_group("asian_option_mc");
    let as_of = as_of();
    let market = create_mc_market(as_of, 100.0, 0.25, 0.05);

    let paths = 2_500_u64;
    group.throughput(Throughput::Elements(paths));
    let option = asian_option(paths as usize);
    group.bench_with_input(BenchmarkId::from_parameter(paths), &paths, |b, &_paths| {
        b.iter(|| black_box(option.npv_mc(black_box(&market), black_box(as_of)).unwrap()));
    });
    group.finish();
}

fn bench_lookback_option_mc(c: &mut Criterion) {
    let mut group = c.benchmark_group("lookback_option_mc");
    let as_of = as_of();
    let market = create_mc_market(as_of, 100.0, 0.25, 0.05);

    let paths = 2_500_u64;
    group.throughput(Throughput::Elements(paths));
    let option = lookback_option(paths as usize);
    group.bench_with_input(BenchmarkId::from_parameter(paths), &paths, |b, &_paths| {
        b.iter(|| option.value(black_box(&market), black_box(as_of)).unwrap());
    });
    group.finish();
}

fn bench_autocallable_mc(c: &mut Criterion) {
    let mut group = c.benchmark_group("autocallable_mc");
    let as_of = as_of();
    let market = create_mc_market(as_of, 100.0, 0.25, 0.05);

    let paths = 2_500_u64;
    group.throughput(Throughput::Elements(paths));
    let note = autocallable_note(paths as usize);
    group.bench_with_input(BenchmarkId::from_parameter(paths), &paths, |b, &_paths| {
        b.iter(|| note.value(black_box(&market), black_box(as_of)).unwrap());
    });
    group.finish();
}

fn make_cliquet(as_of: Date, n_resets: usize) -> CliquetOption {
    // Space resets evenly over 1 year
    let reset_dates: Vec<Date> = (1..=n_resets)
        .map(|i| {
            let days = (365 * i / n_resets) as i64;
            as_of + time::Duration::days(days)
        })
        .collect();
    let expiry = *reset_dates.last().unwrap();

    CliquetOption::builder()
        .id(InstrumentId::new("CLIQ-BENCH"))
        .underlying_ticker("SPOT".to_string())
        .reset_dates(reset_dates)
        .expiry(expiry)
        .local_cap(0.05)
        .local_floor(0.0)
        .global_cap(0.20)
        .global_floor(0.0)
        .notional(Money::new(100_000.0, Currency::USD).expect("valid money fixture"))
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id("SPOT".into())
        .vol_surface_id(CurveId::new("SPOT_VOL"))
        .path_model(EquityPathModel::AtmTermGbm)
        .div_yield_id_opt(Some(PriceId::new("SPOT_DIV")))
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .unwrap()
}

/// Representative cliquet option MC cost with 12 reset periods.
fn bench_cliquet_option_mc(c: &mut Criterion) {
    let mut group = c.benchmark_group("cliquet_option_mc");
    let as_of = as_of();
    let market = create_mc_market(as_of, 100.0, 0.25, 0.05);

    let n_resets = 12;
    let option = make_cliquet(as_of, n_resets);
    group.bench_with_input(
        BenchmarkId::from_parameter(format!("{n_resets}resets")),
        &n_resets,
        |b, _| {
            b.iter(|| {
                black_box(finstack_quant_valuations::instruments::Instrument::value(
                    &option,
                    black_box(&market),
                    black_box(as_of),
                ))
                .unwrap()
            })
        },
    );
    group.finish();
}

fn bench_barrier_monitoring(c: &mut Criterion) {
    let as_of = as_of();
    let market = create_mc_market(as_of, 100.0, 0.25, 0.05)
        .insert_price("HESTON_KAPPA", MarketScalar::Unitless(2.0))
        .insert_price("HESTON_THETA", MarketScalar::Unitless(0.0625))
        .insert_price("HESTON_SIGMA_V", MarketScalar::Unitless(0.3))
        .insert_price("HESTON_RHO", MarketScalar::Unitless(-0.5))
        .insert_price("HESTON_V0", MarketScalar::Unitless(0.0625));
    let mut option = BarrierOption::example().unwrap();
    option.expiry = as_of + time::Duration::days(365);
    option.strike = 100.0;
    option.barrier = Money::from((120_i64, Currency::USD));
    option.barrier_type = BarrierType::UpAndOut;
    option.notional = Money::from((1_000_i64, Currency::USD));
    option.rebate = Some(Money::from((25_i64, Currency::USD)));
    option.rebate_timing = RebateTiming::AtHit;
    option.spot_id = "SPOT".into();
    option.vol_surface_id = "SPOT_VOL".into();
    option.div_yield_id = Some("SPOT_DIV".into());
    option.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_mc_paths(2_500);
    let registry = standard_pricer_registry();
    let mut group = c.benchmark_group("barrier_monitoring");
    for (monitoring_name, monitoring) in [
        ("continuous", Monitoring::Continuous),
        (
            "quarterly",
            Monitoring::Discrete {
                observation_dates: [91, 182, 273, 365]
                    .map(|days| as_of + time::Duration::days(days))
                    .to_vec(),
            },
        ),
    ] {
        option.monitoring = monitoring;
        for (model_name, model) in [
            ("gbm_2500_paths", ModelKey::MonteCarloGBM),
            ("heston_2500_paths", ModelKey::MonteCarloHeston),
            ("pde", ModelKey::PdeCrankNicolson1D),
        ] {
            let pricer = registry
                .get_pricer(PricerKey::new(InstrumentType::BarrierOption, model))
                .unwrap();
            group.bench_function(format!("{monitoring_name}/{model_name}"), |b| {
                b.iter(|| {
                    pricer
                        .price_dyn(black_box(&option), black_box(&market), black_box(as_of))
                        .unwrap()
                });
            });
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_asian_option_mc,
    bench_lookback_option_mc,
    bench_autocallable_mc,
    bench_cliquet_option_mc,
    bench_barrier_monitoring,
);
criterion_main!(benches);
