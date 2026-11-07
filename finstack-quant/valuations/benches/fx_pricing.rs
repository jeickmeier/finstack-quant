#![allow(clippy::unwrap_used)]

//! FX instrument pricing benchmarks.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fx::fx_forward::FxForward;
use finstack_quant_valuations::instruments::fx::fx_spot::FxSpot;
use finstack_quant_valuations::instruments::fx::fx_swap::FxSwap;
use finstack_quant_valuations::instruments::fx::ndf::Ndf;
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::ModelKey;

mod support;
use std::hint::black_box;
use std::sync::Arc;
use time::Month;

#[allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]
#[path = "../tests/support/equity_fx_options.rs"]
mod option_support;

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn create_market() -> MarketContext {
    let base = base_date();
    let usd_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (10.0, 0.9)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let eur_curve = DiscountCurve::builder("EUR-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (10.0, 0.95)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let fx_provider = Arc::new(SimpleFxProvider::new());
    fx_provider
        .set_quotes(&[(Currency::EUR, Currency::USD, 1.10)])
        .unwrap();
    fx_provider
        .set_quotes(&[(Currency::CNY, Currency::USD, 7.25)])
        .unwrap();
    let fx_matrix = FxMatrix::new(fx_provider);

    let vol_surface = VolSurface::from_grid(
        "EURUSD-VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[0.8, 0.9, 1.0, 1.1, 1.2],
        &[0.10; 20],
        Default::default(),
        None,
    )
    .unwrap();

    MarketContext::new()
        .insert(usd_curve)
        .insert(eur_curve)
        .insert_fx(fx_matrix)
        .insert_surface(vol_surface)
}

fn bench_fx_spot_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("fx_spot_pv");
    group.bench_function("eurusd", |b| {
        let market = create_market();
        let as_of = base_date();
        let spot = FxSpot::builder()
            .id(InstrumentId::new("EURUSD"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .notional(Money::from((1_i64, Currency::EUR)))
            .build()
            .expect("valid FX spot");
        assert_eq!(spot.default_model(), ModelKey::Discounting);
        support::bench(
            b,
            || {
                spot.value(black_box(&market), black_box(as_of))
                    .expect("FX spot PV")
            },
            |value| {
                assert_eq!(value.currency(), Currency::USD);
                assert!(value.amount().is_finite());
            },
        );
    });
    group.finish();
}

fn bench_fx_swap_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("fx_swap_pv");
    let swaps = [
        (
            "1M",
            Date::from_calendar_date(2025, Month::February, 3).unwrap(),
        ),
        (
            "3M",
            Date::from_calendar_date(2025, Month::April, 3).unwrap(),
        ),
        (
            "1Y",
            Date::from_calendar_date(2026, Month::January, 3).unwrap(),
        ),
    ];
    for (label, far_date) in swaps {
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &far_date,
            |b, far_date| {
                let market = create_market();
                let as_of = base_date();
                let near_date = Date::from_calendar_date(2025, Month::January, 3).unwrap();
                let swap = FxSwap::builder()
                    .id(InstrumentId::new(format!("EURUSD-SWAP-{label}")))
                    .base_currency(Currency::EUR)
                    .quote_currency(Currency::USD)
                    .near_date(near_date)
                    .far_date(*far_date)
                    .notional(Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"))
                    .domestic_discount_curve_id("USD-OIS".into())
                    .foreign_discount_curve_id("EUR-OIS".into())
                    .build()
                    .unwrap();
                assert_eq!(swap.default_model(), ModelKey::Discounting);
                assert!(swap.near_date > as_of && swap.far_date > swap.near_date);
                support::bench(
                    b,
                    || {
                        swap.value(black_box(&market), black_box(as_of))
                            .expect("FX swap PV")
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

fn bench_fx_forward_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("fx_forward_pv");
    let tenors: [(&str, i64); 4] = [("1M", 30), ("3M", 90), ("6M", 180), ("1Y", 365)];
    for (label, days) in tenors {
        group.bench_with_input(BenchmarkId::from_parameter(label), &days, |b, days| {
            let market = create_market();
            let as_of = base_date();
            let maturity = as_of + time::Duration::days(*days);
            let forward = FxForward::builder()
                .id(InstrumentId::new(format!("EURUSD-FWD-{label}")))
                .base_currency(Currency::EUR)
                .quote_currency(Currency::USD)
                .maturity(maturity)
                .notional(Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"))
                .contract_rate_opt(Some(1.05))
                .domestic_discount_curve_id(CurveId::new("USD-OIS"))
                .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
                .attributes(Attributes::new())
                .build()
                .unwrap();
            assert_eq!(forward.default_model(), ModelKey::Discounting);
            assert!(forward.maturity > as_of);
            support::bench(
                b,
                || {
                    forward
                        .value(black_box(&market), black_box(as_of))
                        .expect("FX forward PV")
                },
                |value| {
                    assert_eq!(value.currency(), Currency::USD);
                    assert!(value.amount().is_finite());
                },
            );
        });
    }
    group.finish();
}

fn bench_fx_option_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("fx_option_pv");
    let expiries: [(&str, i64); 3] = [("3M", 90), ("6M", 180), ("12M", 365)];
    for (label, days) in expiries {
        group.bench_with_input(BenchmarkId::from_parameter(label), &days, |b, days| {
            let market = create_market();
            let as_of = base_date();
            let expiry = as_of + time::Duration::days(*days);
            let option = option_support::fx_option_european_call(
                InstrumentId::new(format!("FX-CALL-{label}")),
                Currency::EUR,
                Currency::USD,
                1.10,
                expiry,
                Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"),
                "EURUSD-VOL",
            )
            .unwrap();
            assert_eq!(option.default_model(), ModelKey::Black76);
            assert!(option.expiry > as_of);
            support::bench(
                b,
                || {
                    option
                        .value(black_box(&market), black_box(as_of))
                        .expect("FX option PV")
                },
                |value| {
                    assert_eq!(value.currency(), Currency::USD);
                    assert!(value.amount().is_finite());
                },
            );
        });
    }
    group.finish();
}

fn bench_fx_option_greeks(c: &mut Criterion) {
    let mut group = c.benchmark_group("fx_option_greeks");
    group.bench_function("6M_delta_gamma_vega_theta_rho", |b| {
        let market = create_market();
        let as_of = base_date();
        let expiry = as_of + time::Duration::days(180);
        let option = option_support::fx_option_european_call(
            "FX-CALL-6M",
            Currency::EUR,
            Currency::USD,
            1.10,
            expiry,
            Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"),
            "EURUSD-VOL",
        )
        .unwrap();
        let greeks = [
            MetricId::Delta,
            MetricId::Gamma,
            MetricId::Vega,
            MetricId::Theta,
            MetricId::Rho,
        ];
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
                        PricingOptions::default(),
                    )
                    .expect("FX option Greeks")
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
    group.finish();
}

fn bench_ndf_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("ndf_pv");
    group.bench_function("cnyusd", |b| {
        let market = create_market();
        let as_of = base_date();
        let ndf = Ndf::example().expect("example");
        assert_eq!(ndf.default_model(), ModelKey::Discounting);
        assert!(ndf.fixing_date > as_of);
        assert_eq!(ndf.quoted_forward, Some(7.25));
        support::bench(
            b,
            || {
                ndf.value(black_box(&market), black_box(as_of))
                    .expect("NDF PV")
            },
            |value| {
                assert_eq!(value.currency(), Currency::USD);
                assert!(value.amount().is_finite());
            },
        );
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_fx_spot_pv,
    bench_fx_swap_pv,
    bench_fx_forward_pv,
    bench_fx_option_pv,
    bench_fx_option_greeks,
    bench_ndf_pv
);
criterion_main!(benches);
