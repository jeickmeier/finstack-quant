//! Inflation-linked bond (TIPS) pricing and analytics benchmarks.

#![allow(clippy::unwrap_used)]

#[path = "../../benches/support/mod.rs"]
mod support;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DateExt, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{
    InflationIndex, InflationLag, InflationObservation,
};
use finstack_quant_core::market_data::surfaces::{
    VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
};
use finstack_quant_core::market_data::term_structures::InflationCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{Attributes, CurveId, InstrumentId};
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations_core::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_core::pricer::ModelKey;
use finstack_quant_valuations_fixed_income::inflation_linked_bond::{
    DeflationProtection, IndexationMethod, InflationLinkedBond,
};
use finstack_quant_valuations_test_support::discount_forward_curves as discount_forward_curve_support;
use rust_decimal::Decimal;
use std::hint::black_box;
use time::Month;

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

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
    InflationCurve::builder(id)
        .base_date(base)
        .day_count(DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
        .knots(knots)
        .build()
        .expect("inflation curve")
}

fn bench_inflation_index() -> InflationIndex {
    let start = Date::from_calendar_date(2019, Month::October, 1).unwrap();
    let start_value = 250.0_f64;
    let monthly_rate = 0.002_f64;
    let observations = (0..64)
        .map(|i| {
            let date = start.add_months(i).expect("valid date shift");
            let value = start_value * (1.0 + monthly_rate).powi(i);
            InflationObservation {
                reference_month: date,
                value,
                available_on: base_date(),
            }
        })
        .collect();
    InflationIndex::new("US-CPI-U", observations, Currency::USD, base_date())
        .expect("inflation index")
        .with_expected_releases(
            finstack_quant_valuations_test_support::inflation::simple_index(
                "US-CPI-U",
                base_date(),
                300.0,
                Currency::USD,
            )
            .get_expected_releases()
            .to_vec(),
        )
        .expect("explicit releases")
}

fn create_inflation_linked_bond_market() -> MarketContext {
    let as_of = base_date();
    let disc_ois = discount_forward_curve_support::flat_discount("USD-OIS", as_of, 0.04);
    let infl_curve = bench_flat_inflation_curve("US-CPI-U", as_of, 300.0, 0.02);
    let index = bench_inflation_index();
    let floor_vol = VolSurface::builder("US-CPI-RATIO-FLOOR-VOL")
        .expiries(&[0.01, 1.0, 5.0, 10.0])
        .secondary_values(&[0.5, 1.0, 1.5, 2.0])
        .row(&[0.10; 4])
        .row(&[0.10; 4])
        .row(&[0.10; 4])
        .row(&[0.10; 4])
        .quote_type(VolQuoteType::BlackLognormal)
        .secondary_axis(VolSurfaceAxis::Strike)
        .interpolation_mode(VolInterpolationMode::Vol)
        .build()
        .expect("CPI-ratio deflation-floor volatility");

    MarketContext::new()
        .try_insert(disc_ois)
        .expect("insert market curve")
        .try_insert(infl_curve)
        .expect("insert market curve")
        .insert_inflation_index(index)
        .insert_surface(floor_vol)
}

fn standard_notional() -> Money {
    Money::new(1_000_000.0, Currency::USD).expect("valid money fixture")
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
        .lag(InflationLag::months(3))
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
                        PricingOptions::default(),
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
    inflation_linked_bond_pv,
    inflation_linked_bond_analytics
);
criterion_main!(benches);
