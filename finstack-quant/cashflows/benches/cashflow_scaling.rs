//! Scaling guards for `finstack-quant-cashflows`.
//!
//! Complements `cashflow_hot_paths.rs` (absolute cost at one size) by measuring
//! how cost grows with schedule length. Read ns-per-coupon across sizes: flat
//! is linear; rising means a super-linear term is back.
//!
//! ```sh
//! cargo bench -p finstack-quant-cashflows --bench cashflow_scaling
//! ```

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_quant_cashflows::builder::{
    CashFlowSchedule, CouponType, FixedCouponSpec, ScheduleParams,
};
use finstack_quant_cashflows::{AccrualConfig, AccrualIndex};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::money::Money;
use rust_decimal_macros::dec;
use std::hint::black_box;
use time::Month;

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 15).unwrap()
}

/// Monthly fixed-coupon schedule spanning `years` (unadjusted calendar).
fn build_monthly(base: Date, years: i32) -> CashFlowSchedule {
    let maturity = Date::from_calendar_date(2025 + years, Month::January, 15).unwrap();
    CashFlowSchedule::builder()
        .principal(Money::new(1_000_000.0, Currency::USD), base, maturity)
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: dec!(0.06),
            schedule: ScheduleParams {
                freq: Tenor::monthly(),
                dc: DayCount::Act365F,
                bdc: BusinessDayConvention::Unadjusted,
                calendar_id: "weekends_only".to_string(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
            },
        })
        .build(None)
        .unwrap()
}

/// Quarterly schedule on a holiday calendar, parameterised by adjustment axes.
fn build_adjusted(
    base: Date,
    years: i32,
    adjust_accrual_dates: bool,
    lag: i32,
) -> CashFlowSchedule {
    let maturity = Date::from_calendar_date(2025 + years, Month::January, 15).unwrap();
    CashFlowSchedule::builder()
        .principal(Money::new(1_000_000.0, Currency::USD), base, maturity)
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: dec!(0.06),
            schedule: ScheduleParams {
                freq: Tenor::quarterly(),
                dc: DayCount::Act365F,
                bdc: BusinessDayConvention::ModifiedFollowing,
                calendar_id: "usny".to_string(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: lag,
                adjust_accrual_dates,
            },
        })
        .build(None)
        .unwrap()
}

/// Business-day-adjustment cost by axis (`accrual_adjusted` ≈ `payment_only`).
fn bench_adjustment_axes(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_build_adjustment_axes_20y_q");
    let base = base_date();

    for (label, adjust_accruals, lag) in [
        ("payment_only", false, 0),
        ("accrual_adjusted", true, 0),
        ("accrual_adjusted_lag2", true, 2),
    ] {
        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            b.iter(|| {
                build_adjusted(
                    black_box(base),
                    20,
                    black_box(adjust_accruals),
                    black_box(lag),
                )
            });
        });
    }
    group.finish();
}

/// Schedule build must stay linear in coupon count.
///
/// Regression signal: ns-per-coupon should be roughly flat across sizes.
fn bench_build_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_build_monthly");
    let base = base_date();

    for years in [5i32, 10, 20, 40, 80] {
        let n = (years * 12) as u64;
        group.throughput(Throughput::Elements(n));
        group.bench_with_input(BenchmarkId::from_parameter(n), &years, |b, &y| {
            b.iter(|| build_monthly(black_box(base), black_box(y)));
        });
    }
    group.finish();
}

/// Cost of a single accrued-interest query against schedule length.
fn bench_accrued_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_accrued_single_query");
    let base = base_date();
    let cfg = AccrualConfig::default();
    let as_of = base + time::Duration::days(400);

    for years in [5i32, 10, 20, 40] {
        let n = (years * 12) as u64;
        let schedule = build_monthly(base, years);

        group.throughput(Throughput::Elements(n));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                finstack_quant_cashflows::accrued_interest_amount(
                    black_box(&schedule),
                    black_box(as_of),
                    black_box(&cfg),
                )
                .unwrap()
            });
        });
    }
    group.finish();
}

/// Accrued interest once per exercise date: rebuild vs prebuilt [`AccrualIndex`].
fn bench_accrued_per_exercise_date(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_accrued_per_exercise_date");
    let base = base_date();
    let cfg = AccrualConfig::default();

    for years in [5i32, 10, 20] {
        let n = (years * 12) as u64;
        let schedule = build_monthly(base, years);
        let exercise_dates: Vec<Date> = (0..n)
            .map(|i| base + time::Duration::days((i as i64) * 30 + 15))
            .collect();

        group.throughput(Throughput::Elements(n));

        group.bench_with_input(BenchmarkId::new("per_call_rebuild", n), &n, |b, _| {
            b.iter(|| {
                let mut acc = 0.0;
                for &d in &exercise_dates {
                    acc += finstack_quant_cashflows::accrued_interest_amount(
                        black_box(&schedule),
                        black_box(d),
                        black_box(&cfg),
                    )
                    .unwrap_or(0.0);
                }
                acc
            });
        });

        group.bench_with_input(BenchmarkId::new("prebuilt_index", n), &n, |b, _| {
            b.iter(|| {
                let index = AccrualIndex::build(black_box(&schedule), black_box(&cfg)).unwrap();
                let mut acc = 0.0;
                for &d in &exercise_dates {
                    acc += index.accrued_at(black_box(d)).unwrap_or(0.0);
                }
                acc
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_build_scaling,
    bench_adjustment_axes,
    bench_accrued_single,
    bench_accrued_per_exercise_date,
);
criterion_main!(benches);
