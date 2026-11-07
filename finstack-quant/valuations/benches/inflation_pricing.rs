//! Inflation instrument pricing benchmarks.

#![allow(clippy::unwrap_used)]

mod support;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DateExt, DayCount, StubKind, Tenor, TenorUnit,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{
    InflationIndex, InflationLag, SeriesInterpolation,
};
use finstack_quant_core::market_data::surfaces::{VolQuoteType, VolSurface, VolSurfaceAxis};
use finstack_quant_core::market_data::term_structures::InflationCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::PayReceive;
use finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::{
    DeflationProtection, IndexationMethod, InflationLinkedBond,
};
use finstack_quant_valuations::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations::instruments::rates::cap_floor::RateOptionType;
use finstack_quant_valuations::instruments::rates::inflation_cap_floor::InflationCapFloor;
use finstack_quant_valuations::instruments::rates::inflation_swap::InflationSwap;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::ModelKey;
use rust_decimal::Decimal;
use std::hint::black_box;
use time::Month;

use finstack_quant_valuations_test_support::discount_forward_curves as discount_forward_curve_support;
use finstack_quant_valuations_test_support::volatility as volatility_support;

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

/// Flat CPI curve (constant annualized inflation) — mirrors `inflation_swap` test fixtures.
fn bench_flat_inflation_curve(
    id: &str,
    base: Date,
    base_cpi: f64,
    annual_inflation_rate: f64,
) -> InflationCurve {
    let knots = vec![
        (0.0, base_cpi),
        (1.0, base_cpi * (1.0 + annual_inflation_rate)),
        (5.0, base_cpi * (1.0 + annual_inflation_rate).powf(5.0)),
        (10.0, base_cpi * (1.0 + annual_inflation_rate).powf(10.0)),
        (30.0, base_cpi * (1.0 + annual_inflation_rate).powf(30.0)),
    ];
    let knots: Vec<(f64, f64)> = knots
        .into_iter()
        .map(|(t, cpi)| (t, cpi.max(1e-10)))
        .collect();
    InflationCurve::builder(id)
        .base_date(base)
        .base_cpi(base_cpi.max(1e-10))
        .knots(knots)
        .build()
        .expect("inflation curve")
}

/// Historical CPI observations for ILB schedules (aligned with `inflation_linked_bond` tests).
fn bench_inflation_index() -> InflationIndex {
    let start = Date::from_calendar_date(2019, Month::October, 1).unwrap();
    let start_value = 250.0_f64;
    let monthly_rate = 0.002_f64;
    let observations: Vec<(Date, f64)> = (0..364)
        .map(|i| {
            let date = start.add_months(i).expect("valid date shift");
            let value = start_value * (1.0 + monthly_rate).powi(i);
            (date, value)
        })
        .collect();
    InflationIndex::new("US-CPI-U", observations, Currency::USD)
        .expect("inflation index")
        .with_interpolation(SeriesInterpolation::Linear)
        .with_lag(InflationLag::Months(3))
}

/// Nominal discount (USD-OIS), CPI curve, CPI index, and inflation vol surface.
fn create_inflation_market() -> MarketContext {
    let as_of = base_date();
    let disc_ois = discount_forward_curve_support::flat_discount("USD-OIS", as_of, 0.04);
    let infl_curve = bench_flat_inflation_curve("US-CPI-U", as_of, 300.0, 0.02);
    let index = bench_inflation_index();
    let vol = volatility_support::flat_vol_surface(
        "US-CPI-VOL",
        &[0.25, 0.5, 1.0, 2.0, 5.0, 10.0],
        &[0.0, 0.01, 0.02, 0.05],
        0.20,
    );

    MarketContext::new()
        .insert(disc_ois)
        .insert(infl_curve)
        .insert_inflation_index(index)
        .insert_surface(vol)
}

/// ILBs apply their own publication lag, so their market index must be lag-free.
fn create_inflation_linked_bond_market() -> MarketContext {
    // The protected principal is an option on reference CPI / base CPI,
    // rather than on the annual inflation rate quoted by US-CPI-VOL.
    let floor_vol = VolSurface::builder("US-CPI-RATIO-FLOOR-VOL")
        .expiries(&[0.01, 1.0, 5.0, 10.0])
        .strikes(&[0.5, 1.0, 1.5, 2.0])
        .row(&[0.10; 4])
        .row(&[0.10; 4])
        .row(&[0.10; 4])
        .row(&[0.10; 4])
        .quote_type(VolQuoteType::BlackLognormal)
        .secondary_axis(VolSurfaceAxis::Strike)
        .build()
        .expect("CPI-ratio deflation-floor volatility");
    create_inflation_market()
        .insert_inflation_index(bench_inflation_index().with_lag(InflationLag::None))
        .insert_surface(floor_vol)
}

fn standard_notional() -> Money {
    Money::new(1_000_000.0, Currency::USD).expect("valid money fixture")
}

fn inflation_swap(as_of: Date, maturity: Date, id: &str) -> InflationSwap {
    InflationSwap::builder()
        .id(id.into())
        .notional(standard_notional())
        .start_date(as_of)
        .maturity(maturity)
        .fixed_rate(Decimal::try_from(0.02).expect("valid decimal"))
        .inflation_index_id("US-CPI-U".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(DayCount::Act365F)
        .side(PayReceive::Pay)
        .attributes(Attributes::new())
        .build()
        .expect("inflation swap")
}

fn inflation_cap_floor(
    id: &str,
    rate_option_type: RateOptionType,
    as_of: Date,
    maturity: Date,
) -> InflationCapFloor {
    InflationCapFloor::builder()
        .id(InstrumentId::new(id))
        .rate_option_type(rate_option_type)
        .notional(standard_notional())
        .strike(Decimal::try_from(0.02).expect("valid decimal"))
        .start_date(as_of)
        .maturity(maturity)
        .frequency(Tenor::new(1, TenorUnit::Years).expect("valid tenor fixture"))
        .day_count(DayCount::Act365F)
        .stub(StubKind::None)
        .business_day_convention(BusinessDayConvention::Following)
        .calendar_id_opt(None)
        .inflation_index_id(CurveId::new("US-CPI-U"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("US-CPI-VOL"))
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .lag_opt(None)
        .attributes(Attributes::new())
        .build()
        .expect("inflation cap/floor")
}

fn tips_benchmark(id: &str, maturity: Date) -> InflationLinkedBond {
    InflationLinkedBond::builder()
        .id(InstrumentId::new(id))
        .notional(standard_notional())
        .real_coupon(Decimal::try_from(0.0125).expect("valid decimal"))
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::ActAct)
        .issue_date(Date::from_calendar_date(2020, Month::January, 15).unwrap())
        .maturity(maturity)
        .base_cpi(250.0)
        .base_date(Date::from_calendar_date(2019, Month::October, 1).unwrap())
        .indexation_method(IndexationMethod::Tips)
        .lag(InflationLag::Months(3))
        .deflation_protection(DeflationProtection::MaturityOnly)
        .floor_vol_surface_id(CurveId::new("US-CPI-RATIO-FLOOR-VOL"))
        .business_day_convention(BusinessDayConvention::Following)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .inflation_index_id(CurveId::new("US-CPI-U"))
        .instrument_pricing_overrides(
            InstrumentPricingOverrides::default().with_quoted_clean_price_pct(100.0),
        )
        .attributes(Attributes::new())
        .build()
        .expect("TIPS")
}

fn assert_tips_floor(tips: &InflationLinkedBond, market: &MarketContext) {
    assert_eq!(tips.default_model(), ModelKey::Discounting);
    assert_eq!(tips.deflation_protection, DeflationProtection::MaturityOnly);
    let surface = market
        .get_surface(
            tips.floor_vol_surface_id
                .as_ref()
                .expect("TIPS floor surface")
                .as_str(),
        )
        .expect("TIPS floor surface in market");
    assert_eq!(surface.quote_type(), VolQuoteType::BlackLognormal);
    assert_eq!(surface.secondary_axis(), VolSurfaceAxis::Strike);
}

fn inflation_swap_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("inflation_swap_pv");
    let (label, years) = ("5Y", 5_i32);
    group.bench_with_input(BenchmarkId::new("pv", label), &years, |b, &years| {
        let market = create_inflation_market();
        let as_of = base_date();
        let maturity = Date::from_calendar_date(2025 + years, Month::January, 1).unwrap();
        let swap = inflation_swap(as_of, maturity, &format!("ZCINF-BENCH-{label}"));
        support::bench(
            b,
            || {
                swap.value(black_box(&market), black_box(as_of))
                    .expect("inflation swap PV should calculate successfully")
            },
            |value| {
                assert!(value.amount().is_finite());
                assert_eq!(value.currency(), Currency::USD);
                assert_eq!(swap.start_date, as_of);
                assert_eq!(swap.maturity, maturity);
            },
        );
    });
    group.finish();
}

fn inflation_cap_floor_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("inflation_cap_floor_pv");
    for (label, id, rate_option_type) in [
        ("cap_5y_pv", "INF-CAP-5Y", RateOptionType::Cap),
        ("floor_5y_pv", "INF-FLOOR-5Y", RateOptionType::Floor),
    ] {
        group.bench_function(label, |b| {
            let market = create_inflation_market();
            let as_of = base_date();
            let option = inflation_cap_floor(
                id,
                rate_option_type,
                as_of,
                Date::from_calendar_date(2030, Month::January, 1).unwrap(),
            );
            support::bench(
                b,
                || {
                    option
                        .value(black_box(&market), black_box(as_of))
                        .expect("inflation cap/floor PV should calculate successfully")
                },
                |value| {
                    assert!(value.amount().is_finite() && value.amount() > 0.0);
                    assert_eq!(value.currency(), Currency::USD);
                    assert_eq!(option.default_model(), ModelKey::Black76);
                    assert_eq!(option.rate_option_type, rate_option_type);
                    assert_eq!(option.start_date, as_of);
                    assert_eq!(option.frequency, Tenor::annual());
                },
            );
        });
    }
    group.finish();
}

fn inflation_linked_bond_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("inflation_linked_bond_pv");
    group.bench_function("tips_5y_pv", |b| {
        let market = create_inflation_linked_bond_market();
        let as_of = base_date();
        let tips_5y = tips_benchmark(
            "TIPS-5Y-BENCH",
            Date::from_calendar_date(2030, Month::January, 15).unwrap(),
        );
        assert_tips_floor(&tips_5y, &market);
        support::bench(
            b,
            || {
                tips_5y
                    .value(black_box(&market), black_box(as_of))
                    .expect("TIPS PV should calculate successfully")
            },
            |value| {
                assert!(value.amount().is_finite() && value.amount() > 0.0);
                assert_eq!(value.currency(), Currency::USD);
                assert_eq!(tips_5y.indexation_method, IndexationMethod::Tips);
                assert!(tips_5y.maturity > as_of);
            },
        );
    });

    group.finish();
}

/// Benchmark the yield, duration, and combined analytics displaced from tests.
fn inflation_linked_bond_analytics(c: &mut Criterion) {
    let mut group = c.benchmark_group("inflation_linked_bond_analytics");
    group.bench_function(BenchmarkId::new("real_yield", "5Y"), |b| {
        let as_of = base_date();
        let tips_5y = tips_benchmark(
            "TIPS-5Y-ANALYTICS-BENCH",
            Date::from_calendar_date(2030, Month::January, 15).unwrap(),
        );
        support::bench(
            b,
            || {
                tips_5y
                    .real_yield(black_box(100.0), black_box(as_of))
                    .expect("TIPS real yield should calculate successfully")
            },
            |value| assert!(value.is_finite()),
        );
    });

    group.bench_function(BenchmarkId::new("real_duration", "5Y"), |b| {
        let as_of = base_date();
        let tips_5y = tips_benchmark(
            "TIPS-5Y-ANALYTICS-BENCH",
            Date::from_calendar_date(2030, Month::January, 15).unwrap(),
        );
        support::bench(
            b,
            || {
                tips_5y
                    .real_duration(black_box(as_of), 1.0)
                    .expect("TIPS real duration should calculate successfully")
            },
            |value| assert!(value.is_finite() && *value > 0.0),
        );
    });

    let metrics = [
        MetricId::RealYield,
        MetricId::IndexRatio,
        MetricId::RealDuration,
        MetricId::BreakevenInflation,
        MetricId::Dv01,
    ];
    group.bench_function(BenchmarkId::new("five_metrics", "5Y"), |b| {
        let market = create_inflation_linked_bond_market();
        let as_of = base_date();
        let tips_5y = tips_benchmark(
            "TIPS-5Y-ANALYTICS-BENCH",
            Date::from_calendar_date(2030, Month::January, 15).unwrap(),
        );
        assert_tips_floor(&tips_5y, &market);
        support::bench(
            b,
            || {
                tips_5y
                    .price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&metrics),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                    .expect("TIPS analytics should calculate successfully")
            },
            |result| {
                assert!(result.value.amount().is_finite() && result.value.amount() > 0.0);
                assert_eq!(result.value.currency(), Currency::USD);
                assert_eq!(result.as_of, as_of);
                for metric in &metrics {
                    let value = result
                        .measures
                        .get(metric)
                        .unwrap_or_else(|| panic!("missing TIPS metric {metric}"));
                    assert!(value.is_finite(), "non-finite TIPS metric {metric}");
                }
            },
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    inflation_swap_pv,
    inflation_cap_floor_pv,
    inflation_linked_bond_pv,
    inflation_linked_bond_analytics
);
criterion_main!(benches);
