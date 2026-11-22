//! Inflation-rate derivative pricing benchmarks (inflation swaps and cap/floors).
//!
//! Inflation-linked bond (TIPS) benchmarks live in
//! `finstack-quant-valuations-fixed-income`'s `inflation_linked_bond_pricing` target.

#![allow(clippy::unwrap_used)]

#[path = "../../benches/support/mod.rs"]
mod support;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DateExt, DayCount, StubKind, Tenor, TenorUnit,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{
    InflationIndex, InflationLag, InflationObservation, SeriesInterpolation,
};
use finstack_quant_core::market_data::term_structures::InflationCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{Attributes, CurveId, InstrumentId};
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations_core::instruments::{Instrument, PayReceive};
use finstack_quant_valuations_core::pricer::ModelKey;
use finstack_quant_valuations_rates::cap_floor::RateOptionType;
use finstack_quant_valuations_rates::inflation_cap_floor::InflationCapFloor;
use finstack_quant_valuations_rates::inflation_swap::InflationSwap;
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
    InflationCurve::builder(id)
        .base_date(base)
        .day_count(DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
        .knots(knots)
        .build()
        .expect("inflation curve")
}

/// Historical CPI observations for ILB schedules (aligned with `inflation_linked_bond` tests).
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
        .try_insert(disc_ois)
        .expect("insert market curve")
        .try_insert(infl_curve)
        .expect("insert market curve")
        .insert_inflation_index(index)
        .insert_surface(vol)
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
        .lag(InflationLag::months(3))
        .interpolation(SeriesInterpolation::Linear)
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
        .lag(InflationLag::months(3))
        .interpolation(SeriesInterpolation::Linear)
        .attributes(Attributes::new())
        .build()
        .expect("inflation cap/floor")
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

criterion_group!(benches, inflation_swap_pv, inflation_cap_floor_pv);
criterion_main!(benches);
