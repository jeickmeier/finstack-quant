//! Equity option pricing benchmarks.
//!
//! Measures performance of Black-Scholes pricing and Greeks:
//! - Present value (call and put)
//! - Full Greeks calculation (delta, gamma, vega, theta, rho)
//! - Individual Greeks
//!
//! Market Standards Review (Week 5)

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::BarrierType;
use finstack_quant_models::types::{ExerciseStyle, OptionType};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
use finstack_quant_valuations::instruments::exotics::barrier_option::BarrierOption;
use finstack_quant_valuations::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::ModelKey;

#[allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]
#[path = "../tests/support/equity_fx_options.rs"]
mod option_support;
mod support;
use std::hint::black_box;
use time::Month;

fn create_call_option(expiry_months: i64) -> EquityOption {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let expiry = base + time::Duration::days(expiry_months * 30);

    option_support::equity_option_european_call(
        format!("CALL-{}M", expiry_months),
        "AAPL",
        100.0, // ATM strike
        expiry,
        100.0, // 100 shares
    )
    .expect("equity option should build in benchmarks")
}

fn create_market() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (0.5, 0.98),
            (1.0, 0.96),
            (2.0, 0.92),
            (5.0, 0.80),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let vol_surface = VolSurface::from_grid(
        "EQUITY-VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[80.0, 90.0, 100.0, 110.0, 120.0],
        &[0.25; 20], // Flat 25% vol
        Default::default(),
        None,
    )
    .unwrap();

    MarketContext::new()
        .insert(disc)
        .insert_surface(vol_surface)
        .insert_price(
            "EQUITY-SPOT",
            MarketScalar::Price(Money::new(100.0, Currency::USD).expect("valid money fixture")),
        )
        .insert_price("EQUITY-DIVYIELD", MarketScalar::Unitless(0.02))
}

fn create_barrier_option() -> BarrierOption {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    BarrierOption {
        expiry_fixing: None,
        id: "BARRIER-12M".into(),
        underlying_ticker: "SPX".into(),
        strike: 100.0,
        barrier: 80.0,
        rebate: None,
        rebate_timing: Default::default(),
        option_type: OptionType::Call,
        barrier_type: BarrierType::DownAndOut,
        expiry: as_of + time::Duration::days(365),
        observed_barrier_breached: None,
        quantity: 100.0,
        currency: Currency::USD,
        day_count: finstack_quant_core::dates::DayCount::Act365F,
        monitoring: finstack_quant_valuations::instruments::Monitoring::Continuous,
        discount_curve_id: "USD_DISC".into(),
        spot_id: "SPX".into(),
        vol_surface_id: "SPX_VOL".into(),
        div_yield_id: Some("SPX_DIV".into()),
        instrument_pricing_overrides: InstrumentPricingOverrides::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

fn create_barrier_market() -> MarketContext {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let disc = DiscountCurve::builder("USD_DISC")
        .base_date(as_of)
        .knots([
            (0.0, 1.0),
            (0.5, 0.98),
            (1.0, 0.96),
            (2.0, 0.92),
            (5.0, 0.80),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let vol_surface = VolSurface::from_grid(
        "SPX_VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[80.0, 90.0, 100.0, 110.0, 120.0],
        &[0.25; 20],
        Default::default(),
        None,
    )
    .unwrap();

    MarketContext::new()
        .insert(disc)
        .insert_surface(vol_surface)
        .insert_price(
            "SPX",
            MarketScalar::Price(Money::new(100.0, Currency::USD).expect("valid money fixture")),
        )
        .insert_price("SPX_DIV", MarketScalar::Unitless(0.02))
}

fn bench_option_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("option_pv");
    for months in [3, 6, 12, 24].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}M", months)),
            months,
            |b, months| {
                let market = create_market();
                let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
                let option = create_call_option(*months);
                assert_eq!(option.exercise_style, ExerciseStyle::European);
                assert_eq!(option.default_model(), ModelKey::Black76);
                assert!(option.expiry > as_of);
                support::bench(
                    b,
                    || {
                        option
                            .value(black_box(&market), black_box(as_of))
                            .expect("equity option PV")
                    },
                    |value| {
                        assert_eq!(value.currency(), Currency::USD);
                        assert!(value.amount().is_finite());
                    },
                );
            },
        );
    }
    group.finish();
}

fn bench_option_greeks(c: &mut Criterion) {
    let mut group = c.benchmark_group("option_greeks");
    for months in [3, 6, 12].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}M", months)),
            months,
            |b, months| {
                let market = create_market();
                let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
                let option = create_call_option(*months);
                let greeks = [
                    MetricId::Delta,
                    MetricId::Gamma,
                    MetricId::Vega,
                    MetricId::Theta,
                    MetricId::Rho,
                ];
                assert_eq!(option.exercise_style, ExerciseStyle::European);
                assert_eq!(option.default_model(), ModelKey::Black76);
                assert!(option.expiry > as_of);
                support::bench(
                    b,
                    || {
                        option
                            .price_with_metrics(
                                black_box(&market),
                                black_box(as_of),
                                black_box(&greeks),
                                finstack_quant_valuations::instruments::PricingOptions::default(),
                            )
                            .expect("equity option Greeks")
                    },
                    |result| {
                        assert_eq!(result.value.currency(), Currency::USD);
                        assert!(result.value.amount().is_finite());
                        for metric in &greeks {
                            assert!(
                                result
                                    .measures
                                    .get(metric)
                                    .expect("requested Greek")
                                    .is_finite()
                            );
                        }
                    },
                );
            },
        );
    }
    group.finish();
}

fn bench_barrier_option_fd_greeks(c: &mut Criterion) {
    c.bench_function("barrier_option_fd_greeks_5_metrics", |b| {
        let market = create_barrier_market();
        let option = create_barrier_option();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let greeks = [
            MetricId::Delta,
            MetricId::Gamma,
            MetricId::Vega,
            MetricId::Vanna,
            MetricId::Volga,
        ];
        assert_eq!(option.default_model(), ModelKey::BarrierBSContinuous);
        assert!(option.expiry > as_of);
        assert!(option.barrier < 100.0);
        support::bench(
            b,
            || {
                option
                    .price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&greeks),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                    .expect("barrier option finite-difference Greeks")
            },
            |result| {
                assert_eq!(result.value.currency(), Currency::USD);
                assert!(result.value.amount().is_finite());
                for metric in &greeks {
                    assert!(
                        result
                            .measures
                            .get(metric)
                            .expect("requested Greek")
                            .is_finite()
                    );
                }
            },
        );
    });
}

fn bench_option_bucketed_vega(c: &mut Criterion) {
    c.bench_function("equity_option_bucketed_vega_4x5", |b| {
        let market = create_market();
        let option = create_call_option(12);
        let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        assert_eq!(option.exercise_style, ExerciseStyle::European);
        assert_eq!(option.default_model(), ModelKey::Black76);
        assert_eq!(
            market.get_surface("EQUITY-VOL").unwrap().grid_shape(),
            (4, 5)
        );
        support::bench(
            b,
            || {
                option
                    .price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&[MetricId::BucketedVega]),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                    .expect("equity option bucketed vega")
            },
            |result| {
                assert_eq!(result.value.currency(), Currency::USD);
                assert!(result.value.amount().is_finite());
                let total = result
                    .measures
                    .get(&MetricId::BucketedVega)
                    .expect("requested bucketed vega");
                assert!(total.is_finite());
                let buckets: Vec<_> = result
                    .measures
                    .iter()
                    .filter(|(key, _)| key.decode_components(&MetricId::BucketedVega).is_some())
                    .collect();
                assert_eq!(buckets.len(), 20, "one bucket per source surface node");
                assert!(buckets.iter().all(|(_, value)| value.is_finite()));
                let bucket_sum: f64 = buckets.iter().map(|(_, value)| **value).sum();
                assert!((bucket_sum - total).abs() < 1e-8);
            },
        );
    });
}

criterion_group!(
    benches,
    bench_option_pv,
    bench_option_greeks,
    bench_barrier_option_fd_greeks,
    bench_option_bucketed_vega
);
criterion_main!(benches);
