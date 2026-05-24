//! Interest rate swap pricing benchmarks.
//!
//! Measures performance of IRS operations:
//! - Present value calculation (with Kahan summation)
//! - DV01 (bump-and-revalue)
//! - Annuity factor calculation
//! - Par rate calculation
//! - OIS compounding (daily accrual)
//!
//! # Numerical Stability Focus
//!
//! Long-dated swaps (30Y+) with quarterly/monthly frequencies can have 120-360+
//! cashflows. The IRS pricer uses **Kahan compensated summation** to prevent
//! floating-point drift. These benchmarks verify that the numerical stability
//! overhead is acceptable.
//!
//! Market Standards Review (Week 5)

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, Tenor, TenorUnit};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_valuations::instruments::rates::irs::{
    FloatingLegCompounding, InterestRateSwap, PayReceive,
};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::instruments::{FixedLegSpec, FloatLegSpec};
use finstack_valuations::metrics::MetricId;
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

fn create_swap(tenor_years: i32) -> InterestRateSwap {
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2025 + tenor_years, Month::January, 1).unwrap();

    test_utils::usd_irs_swap(
        InstrumentId::new(format!("IRS-{}Y", tenor_years)),
        Money::new(10_000_000.0, Currency::USD),
        0.04, // 4% fixed rate
        start,
        end,
        PayReceive::Pay,
    )
    .expect("Failed to create swap for benchmark")
}

/// Create a swap with monthly frequency for stress-testing Kahan summation.
///
/// A 50Y monthly swap has 600 periods, which is where numerical precision
/// really matters.
fn create_monthly_swap(tenor_years: i32) -> InterestRateSwap {
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2025 + tenor_years, Month::January, 1).unwrap();

    let disc_id = CurveId::new("USD-OIS");
    let fwd_id = CurveId::new("USD-SOFR-1M");

    InterestRateSwap::builder()
        .id(format!("IRS-{}Y-Monthly", tenor_years).into())
        .notional(Money::new(10_000_000.0, Currency::USD))
        .side(PayReceive::Pay)
        .fixed(FixedLegSpec {
            discount_curve_id: disc_id.clone(),
            rate: dec!(0.04),
            frequency: Tenor::new(1, TenorUnit::Months), // Monthly fixed
            day_count: finstack_core::dates::DayCount::Act360,
            bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: finstack_core::dates::StubKind::None,
            start,
            end,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .float(FloatLegSpec {
            discount_curve_id: disc_id,
            forward_curve_id: fwd_id,
            spread_bp: dec!(0.0),
            frequency: Tenor::new(1, TenorUnit::Months), // Monthly float
            day_count: finstack_core::dates::DayCount::Act360,
            bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: finstack_core::dates::StubKind::None,
            reset_lag_days: 2,
            start,
            end,
            compounding: FloatingLegCompounding::Simple,
            fixing_calendar_id: None,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .build()
        .expect("Failed to create monthly swap for benchmark")
}

/// Create an OIS swap with daily compounding for benchmarking the complex
/// accrual logic.
fn create_ois_swap(tenor_years: i32) -> InterestRateSwap {
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2025 + tenor_years, Month::January, 1).unwrap();

    let disc_id = CurveId::new("USD-OIS");

    InterestRateSwap::builder()
        .id(format!("OIS-{}Y", tenor_years).into())
        .notional(Money::new(10_000_000.0, Currency::USD))
        .side(PayReceive::Pay)
        .fixed(FixedLegSpec {
            discount_curve_id: disc_id.clone(),
            rate: dec!(0.04),
            frequency: Tenor::new(1, TenorUnit::Years), // Annual fixed
            day_count: finstack_core::dates::DayCount::Act360,
            bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: finstack_core::dates::StubKind::None,
            start,
            end,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 2,
            end_of_month: false,
        })
        .float(FloatLegSpec {
            discount_curve_id: disc_id.clone(),
            forward_curve_id: disc_id, // Single-curve OIS
            spread_bp: dec!(0.0),
            frequency: Tenor::new(1, TenorUnit::Years), // Annual payment with daily compounding
            day_count: finstack_core::dates::DayCount::Act360,
            bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: finstack_core::dates::StubKind::None,
            reset_lag_days: 0,
            start,
            end,
            compounding: FloatingLegCompounding::sofr(), // Compounded in arrears
            fixing_calendar_id: None,
            payment_lag_days: 2,
            end_of_month: false,
        })
        .build()
        .expect("Failed to create OIS swap for benchmark")
}

fn create_market() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (2.0, 0.96),
            (5.0, 0.88),
            (10.0, 0.70),
            (30.0, 0.40),
            (50.0, 0.20),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .knots([
            (0.0, 0.035),
            (1.0, 0.038),
            (2.0, 0.040),
            (5.0, 0.045),
            (10.0, 0.050),
            (30.0, 0.055),
            (50.0, 0.055),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // Monthly forward curve for monthly swaps
    let fwd_1m = ForwardCurve::builder("USD-SOFR-1M", 1.0 / 12.0)
        .base_date(base)
        .knots([
            (0.0, 0.034),
            (1.0, 0.037),
            (2.0, 0.039),
            (5.0, 0.044),
            (10.0, 0.049),
            (30.0, 0.054),
            (50.0, 0.054),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    MarketContext::new().insert(disc).insert(fwd).insert(fwd_1m)
}

fn bench_swap_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("swap_pv");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [2, 5, 10, 30].iter() {
        let swap = create_swap(*tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| swap.value(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

/// Benchmark long-dated swaps with many periods to stress-test Kahan summation.
///
/// A 50Y monthly swap has 600 fixed + 600 float = 1200 cashflows.
/// This is where numerical precision really matters.
fn bench_swap_pv_kahan_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("swap_pv_kahan_stress");
    group.sample_size(50); // Fewer samples for expensive benchmarks
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Long-dated monthly swaps: tests Kahan summation with many periods
    for tenor in [10, 30, 50].iter() {
        let swap = create_monthly_swap(*tenor);
        let periods = tenor * 12 * 2; // fixed + float periods
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y_monthly_{}periods", tenor, periods)),
            tenor,
            |b, _| {
                b.iter(|| swap.value(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

/// Benchmark OIS swaps with daily compounding.
///
/// OIS pricing involves daily rate accumulation which is computationally
/// more expensive than simple term-rate swaps.
fn bench_ois_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("ois_pv");
    group.sample_size(30); // OIS is expensive
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [1, 2, 5, 10].iter() {
        let swap = create_ois_swap(*tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| swap.value(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

fn bench_swap_dv01(c: &mut Criterion) {
    let mut group = c.benchmark_group("swap_dv01");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [2, 5, 10, 30].iter() {
        let swap = create_swap(*tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| {
                    swap.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&[MetricId::Dv01]),
                        finstack_valuations::instruments::PricingOptions::default(),
                    )
                });
            },
        );
    }
    group.finish();
}

fn bench_swap_par_rate(c: &mut Criterion) {
    let mut group = c.benchmark_group("swap_par_rate");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [2, 5, 10, 30].iter() {
        let swap = create_swap(*tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| {
                    swap.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&[MetricId::ParRate, MetricId::Annuity]),
                        finstack_valuations::instruments::PricingOptions::default(),
                    )
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_swap_pv,
    bench_swap_pv_kahan_stress,
    bench_ois_pv,
    bench_swap_dv01,
    bench_swap_par_rate
);
criterion_main!(benches);
