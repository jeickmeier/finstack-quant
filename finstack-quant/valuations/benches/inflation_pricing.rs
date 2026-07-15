//! Inflation instrument pricing benchmarks.

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DateExt, DayCount, StubKind, Tenor, TenorUnit,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{
    InflationIndex, InflationInterpolation, InflationLag,
};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, InflationCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::{
    DeflationProtection, IndexationMethod, InflationLinkedBond,
};
use finstack_quant_valuations::instruments::rates::inflation_cap_floor::{
    InflationCapFloor, InflationCapFloorType,
};
use finstack_quant_valuations::instruments::rates::inflation_swap::{
    InflationSwap, InflationSwapBuilder,
};
use finstack_quant_valuations::instruments::Attributes;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::InstrumentPricingOverrides;
use finstack_quant_valuations::instruments::PayReceive;
use rust_decimal::Decimal;
use std::hint::black_box;
use time::Month;

#[allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]
mod test_utils {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/test_utils.rs"
    ));
}

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
            let date = start.add_months(i);
            let value = start_value * (1.0 + monthly_rate).powi(i);
            (date, value)
        })
        .collect();
    InflationIndex::new("US-CPI-U", observations, Currency::USD)
        .expect("inflation index")
        .with_interpolation(InflationInterpolation::Linear)
        .with_lag(InflationLag::Months(3))
}

/// Discount (USD-OIS), real discount (USD-REAL), CPI curve, CPI index, and inflation vol surface.
fn create_inflation_market() -> MarketContext {
    let as_of = base_date();
    let disc_ois = test_utils::flat_discount("USD-OIS", as_of, 0.04);
    let disc_real = DiscountCurve::builder("USD-REAL")
        .base_date(as_of)
        .knots([
            (0.0, 1.0),
            (0.5, 0.99),
            (1.0, 0.98),
            (2.0, 0.96),
            (5.0, 0.90),
            (10.0, 0.82),
        ])
        .build()
        .expect("USD-REAL");
    let infl_curve = bench_flat_inflation_curve("US-CPI-U", as_of, 300.0, 0.02);
    let index = bench_inflation_index();
    let vol = test_utils::flat_vol_surface(
        "US-CPI-VOL",
        &[0.25, 0.5, 1.0, 2.0, 5.0, 10.0],
        &[0.0, 0.01, 0.02, 0.05],
        0.20,
    );

    MarketContext::new()
        .insert(disc_ois)
        .insert(disc_real)
        .insert(infl_curve)
        .insert_inflation_index("US-CPI-U", index)
        .insert_surface(vol)
}

fn standard_notional() -> Money {
    Money::new(1_000_000.0, Currency::USD)
}

fn inflation_swap(as_of: Date, maturity: Date, id: &str) -> InflationSwap {
    InflationSwapBuilder::new()
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
    option_type: InflationCapFloorType,
    as_of: Date,
    maturity: Date,
) -> InflationCapFloor {
    InflationCapFloor::builder()
        .id(InstrumentId::new(id))
        .option_type(option_type)
        .notional(standard_notional())
        .strike(Decimal::try_from(0.02).expect("valid decimal"))
        .start_date(as_of)
        .maturity(maturity)
        .frequency(Tenor::new(1, TenorUnit::Years))
        .day_count(DayCount::Act365F)
        .stub(StubKind::None)
        .bdc(BusinessDayConvention::Following)
        .calendar_id_opt(None)
        .inflation_index_id(CurveId::new("US-CPI-U"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("US-CPI-VOL"))
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .lag_override_opt(None)
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
        .base_index(250.0)
        .base_date(Date::from_calendar_date(2019, Month::October, 1).unwrap())
        .indexation_method(IndexationMethod::TIPS)
        .lag(InflationLag::Months(3))
        .deflation_protection(DeflationProtection::MaturityOnly)
        .bdc(BusinessDayConvention::Following)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::new("USD-REAL"))
        .inflation_index_id(CurveId::new("US-CPI-U"))
        .attributes(Attributes::new())
        .build()
        .expect("TIPS")
}

fn inflation_swap_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("inflation_swap_pv");
    let market = create_inflation_market();
    let as_of = base_date();

    {
        let (label, years) = ("5Y", 5_i32);
        let maturity = Date::from_calendar_date(2025 + years, Month::January, 1).unwrap();
        let swap = inflation_swap(as_of, maturity, &format!("ZCINF-BENCH-{label}"));
        group.bench_with_input(BenchmarkId::new("pv", label), &years, |b, _| {
            b.iter(|| swap.value(black_box(&market), black_box(as_of)));
        });
    }
    group.finish();
}

fn inflation_cap_floor_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("inflation_cap_floor_pv");
    let market = create_inflation_market();
    let as_of = base_date();

    let cap_5y = inflation_cap_floor(
        "INF-CAP-5Y",
        InflationCapFloorType::Cap,
        as_of,
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
    );
    group.bench_function("cap_5y_pv", |b| {
        b.iter(|| cap_5y.value(black_box(&market), black_box(as_of)));
    });

    let floor_5y = inflation_cap_floor(
        "INF-FLOOR-5Y",
        InflationCapFloorType::Floor,
        as_of,
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
    );
    group.bench_function("floor_5y_pv", |b| {
        b.iter(|| floor_5y.value(black_box(&market), black_box(as_of)));
    });

    group.finish();
}

fn inflation_linked_bond_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("inflation_linked_bond_pv");
    let market = create_inflation_market();
    let as_of = base_date();

    let tips_5y = tips_benchmark(
        "TIPS-5Y-BENCH",
        Date::from_calendar_date(2030, Month::January, 15).unwrap(),
    );
    group.bench_function("tips_5y_pv", |b| {
        b.iter(|| tips_5y.value(black_box(&market), black_box(as_of)));
    });

    group.finish();
}

criterion_group!(
    benches,
    inflation_swap_pv,
    inflation_cap_floor_pv,
    inflation_linked_bond_pv
);
criterion_main!(benches);
