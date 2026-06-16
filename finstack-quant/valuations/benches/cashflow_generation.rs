//! Cashflow generation benchmarks.
//!
//! Measures performance of:
//! - Schedule generation (fixed, floating, amortizing)
//! - Cashflow building for various bond types
//! - Period aggregation
//! - Kahan summation for long legs
//!
//! Market Standards Review (Week 5)

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_cashflows::aggregation::aggregate_cashflows_checked;
use finstack_quant_cashflows::builder::{CashFlowSchedule, CouponType, FixedCouponSpec};
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
#[allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]
mod test_utils {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/test_utils.rs"
    ));
}
use rust_decimal_macros::dec;
use std::hint::black_box;
use time::Month;

fn create_market() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (5.0, 0.88),
            (10.0, 0.70),
            (30.0, 0.40),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .knots([
            (0.0, 0.04),
            (1.0, 0.042),
            (5.0, 0.045),
            (10.0, 0.050),
            (30.0, 0.055),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    MarketContext::new().insert(disc).insert(fwd)
}

fn bench_bond_cashflow_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_cashflow_generation");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [2, 5, 10, 30].iter() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let maturity = Date::from_calendar_date(2025 + tenor, Month::January, 1).unwrap();

        let bond = Bond::fixed(
            format!("BOND-{}Y", tenor),
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            issue,
            maturity,
            "USD-OIS",
        )
        .expect("Bond::fixed should succeed with valid parameters");

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| bond.dated_cashflows(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

fn bench_swap_cashflow_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("swap_cashflow_generation");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [2, 5, 10, 30].iter() {
        let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let end = Date::from_calendar_date(2025 + tenor, Month::January, 1).unwrap();

        let swap = test_utils::usd_irs_swap(
            InstrumentId::new(format!("IRS-{}Y", tenor)),
            Money::new(10_000_000.0, Currency::USD),
            0.04,
            start,
            end,
            finstack_quant_valuations::instruments::rates::irs::PayReceive::Pay,
        )
        .expect("Failed to create swap for benchmark");

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| swap.dated_cashflows(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

fn bench_schedule_builder_fixed(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_builder_fixed");

    for tenor in [2, 5, 10, 30].iter() {
        let issue = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let maturity = Date::from_calendar_date(2025 + tenor, Month::January, 1).unwrap();

        let fixed_spec = FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: dec!(0.05),
            freq: Tenor::semi_annual(),
            dc: DayCount::Thirty360,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
        };

        let init = Money::new(1_000_000.0, Currency::USD);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| {
                    let mut builder = CashFlowSchedule::builder();
                    let _ = builder
                        .principal(black_box(init), black_box(issue), black_box(maturity))
                        .fixed_cf(black_box(fixed_spec.clone()));
                    builder.build_with_curves(None)
                });
            },
        );
    }
    group.finish();
}

fn bench_kahan_summation(c: &mut Criterion) {
    let mut group = c.benchmark_group("kahan_summation");

    // Generate cashflow legs of varying lengths
    for num_flows in [10, 20, 50, 100, 200].iter() {
        let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let flows: Vec<(Date, Money)> = (0..*num_flows)
            .map(|i| {
                let months = i * 6; // Semi-annual
                let date = base + time::Duration::days(months * 30);
                (date, Money::new(1000.0, Currency::USD))
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}flows", num_flows)),
            num_flows,
            |b, _| {
                b.iter(|| {
                    let _ = aggregate_cashflows_checked(black_box(&flows), Currency::USD).unwrap();
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_bond_cashflow_generation,
    bench_swap_cashflow_generation,
    bench_schedule_builder_fixed,
    bench_kahan_summation
);
criterion_main!(benches);
