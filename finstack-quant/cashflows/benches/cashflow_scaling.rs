//! Scaling guards for `finstack-quant-cashflows`.
//!
//! Complements `cashflow_hot_paths.rs` (absolute cost at one size) by measuring
//! how cost grows with schedule length. Throughput counts coupons, synthetic
//! amortization rows, or accrued-interest queries as documented for each group.
//!
//! ```sh
//! mise run rust-bench-crate -- finstack-quant-cashflows cashflow_scaling
//! ```

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

#[path = "support/fixtures.rs"]
mod fixtures;

#[path = "support/checks.rs"]
mod checks;

#[path = "support/accrual_workloads.rs"]
mod accrual_workloads;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use finstack_quant_cashflows::accrual::{AccrualConfig, AccrualIndex, accrued_interest_amount};
use finstack_quant_cashflows::builder::{
    AmortizationSpec, CashFlowSchedule, CouponType, FixedCouponSpec, ScheduleParams,
};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DateExt, DayCount, StubKind, Tenor, adjust, calendar_by_id_strict,
};
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
        .principal(
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            base,
            maturity,
        )
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: dec!(0.06),
            schedule: ScheduleParams {
                frequency: Tenor::monthly(),
                day_count: DayCount::Act365F,
                business_day_convention: BusinessDayConvention::Unadjusted,
                calendar_id: "weekends_only".into(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
                roll_rule: finstack_quant_cashflows::builder::RollRule::None,
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
        .principal(
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            base,
            maturity,
        )
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: dec!(0.06),
            schedule: ScheduleParams {
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act365F,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                calendar_id: "usny".into(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: lag,
                adjust_accrual_dates,
                roll_rule: finstack_quant_cashflows::builder::RollRule::None,
            },
        })
        .build(None)
        .unwrap()
}

fn check_adjusted_schedule(
    schedule: &CashFlowSchedule,
    base: Date,
    adjust_accrual_dates: bool,
    lag: i32,
) {
    checks::rows(schedule, 82);
    checks::fixed_amounts(schedule);
    let calendar = calendar_by_id_strict("usny").unwrap();
    let coupons: Vec<_> = schedule.coupons().collect();
    assert_eq!(coupons.len(), 80);
    assert_eq!(schedule.get_meta().issue_date, Some(base));
    assert_eq!(
        schedule.get_meta().maturity,
        Some(base.add_months(240).unwrap())
    );
    for (i, coupon) in coupons.iter().enumerate() {
        let start = base.add_months(3 * i as i32).unwrap();
        let end = base.add_months(3 * (i + 1) as i32).unwrap();
        let adjusted_start =
            adjust(start, BusinessDayConvention::ModifiedFollowing, calendar).unwrap();
        let adjusted_end = adjust(end, BusinessDayConvention::ModifiedFollowing, calendar).unwrap();
        let accrual = coupon
            .get_accrual()
            .as_ref()
            .expect("fixed coupon accrual metadata");
        assert_eq!(
            accrual.start,
            if adjust_accrual_dates {
                adjusted_start
            } else {
                start
            }
        );
        assert_eq!(
            accrual.end,
            if adjust_accrual_dates {
                adjusted_end
            } else {
                end
            }
        );
        assert_eq!(
            coupon.get_date(),
            adjusted_end.add_business_days(lag, calendar).unwrap()
        );
    }
    let redemption = schedule.get_flows().last().unwrap();
    assert_eq!(redemption.get_kind(), CFKind::Notional);
    assert_eq!(redemption.get_date(), coupons.last().unwrap().get_date());
    checks::close(redemption.get_amount().amount(), 1_000_000.0, 1e-8);
}

fn check_amortizing_monthly(schedule: &CashFlowSchedule, base: Date, coupons: usize) {
    checks::rows(schedule, coupons * 2 + 1);
    let coupon_rows: Vec<_> = schedule.coupons().collect();
    let principal_rows: Vec<_> = schedule
        .get_flows()
        .iter()
        .filter(|flow| flow.get_kind() == CFKind::Amortization)
        .collect();
    assert_eq!(coupon_rows.len(), coupons);
    assert_eq!(principal_rows.len(), coupons);
    assert_eq!(schedule.get_meta().issue_date, Some(base));
    assert_eq!(
        schedule.get_meta().maturity,
        Some(base.add_months(coupons as i32).unwrap())
    );
    for (i, (coupon, principal)) in coupon_rows.iter().zip(&principal_rows).enumerate() {
        let start = base.add_months(i as i32).unwrap();
        let end = base.add_months((i + 1) as i32).unwrap();
        let accrual = coupon
            .get_accrual()
            .as_ref()
            .expect("monthly coupon accrual metadata");
        assert_eq!((accrual.start, accrual.end), (start, end));
        assert_eq!(coupon.get_date(), end);
        assert_eq!(principal.get_date(), end);
        assert_eq!(
            principal
                .get_balance_date()
                .expect("principal row balance date"),
            end
        );
        let opening = 1_000_000.0 * (coupons - i) as f64 / coupons as f64;
        let expected_coupon = opening * 0.06 * (end - start).whole_days() as f64 / 365.0;
        checks::close(coupon.get_amount().amount(), expected_coupon, 1e-6);
        checks::close(
            principal.get_amount().amount(),
            1_000_000.0 / coupons as f64,
            1e-6,
        );
    }
    checks::close(
        principal_rows
            .iter()
            .map(|flow| flow.get_amount().amount())
            .sum(),
        1_000_000.0,
        1e-6,
    );
    checks::close(
        schedule
            .outstanding_by_date()
            .unwrap()
            .last()
            .unwrap()
            .1
            .amount(),
        0.0,
        1e-6,
    );
}

/// Fixed-size business-day adjustment axes; this is not a length sweep.
fn bench_adjustment_axes(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_build_adjustment_axes_20y_q");

    for (label, adjust_accruals, lag) in [
        ("payment_only", false, 0),
        ("accrual_adjusted", true, 0),
        ("accrual_adjusted_lag2", true, 2),
    ] {
        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            let base = base_date();
            let schedule = build_adjusted(base, 20, adjust_accruals, lag);
            check_adjusted_schedule(&schedule, base, adjust_accruals, lag);
            drop(schedule);
            b.iter(|| {
                black_box(build_adjusted(
                    black_box(base),
                    20,
                    black_box(adjust_accruals),
                    black_box(lag),
                ))
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

    for years in [5i32, 10, 20, 40, 80] {
        let n = (years * 12) as u64;
        group.throughput(Throughput::Elements(n));
        group.bench_with_input(BenchmarkId::from_parameter(n), &years, |b, &y| {
            let base = base_date();
            let schedule = build_monthly(base, y);
            checks::fixed_bullet(&schedule, base, y, n as usize, false);
            drop(schedule);
            b.iter(|| black_box(build_monthly(black_box(base), black_box(y))));
        });
    }
    group.finish();
}

/// Construction with amortization must avoid scanning all boundaries per period.
fn bench_amortizing_build_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_build_amortizing_monthly");
    for years in [5i32, 10, 20, 40, 80] {
        let coupons = (years * 12) as u64;
        group.throughput(Throughput::Elements(coupons));
        group.bench_with_input(BenchmarkId::from_parameter(coupons), &years, |b, &y| {
            let base = base_date();
            let maturity = Date::from_calendar_date(2025 + y, Month::January, 15).unwrap();
            // Configuration remains outside timing; build still compiles/projects each call.
            let mut builder = CashFlowSchedule::builder();
            let _ = builder
                .principal(
                    Money::new(1_000_000.0, Currency::USD).unwrap(),
                    base,
                    maturity,
                )
                .amortization(AmortizationSpec::LinearTo {
                    final_notional: Money::new(0.0, Currency::USD).unwrap(),
                })
                .fixed_cf(FixedCouponSpec {
                    coupon_type: CouponType::Cash,
                    rate: dec!(0.06),
                    schedule: ScheduleParams {
                        frequency: Tenor::monthly(),
                        business_day_convention: BusinessDayConvention::Unadjusted,
                        calendar_id: "weekends_only".into(),
                        day_count: DayCount::Act365F,
                        stub: StubKind::None,
                        end_of_month: false,
                        payment_lag_days: 0,
                        adjust_accrual_dates: false,
                        roll_rule: finstack_quant_cashflows::builder::RollRule::None,
                    },
                });
            let schedule = builder.build(None).unwrap();
            check_amortizing_monthly(&schedule, base, coupons as usize);
            drop(schedule);
            b.iter(|| black_box(black_box(&builder).build(None).unwrap()));
        });
    }
    group.finish();
}

/// Cost of a single accrued-interest query against schedule length.
fn bench_accrued_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_accrued_single_query");

    for years in [5i32, 10, 20, 40] {
        let n = (years * 12) as u64;
        group.throughput(Throughput::Elements(n));
        group.bench_with_input(BenchmarkId::from_parameter(n), &years, |b, &y| {
            let base = base_date();
            let cfg = AccrualConfig::default();
            let as_of = base + time::Duration::days(400);
            let schedule = build_monthly(base, y);
            checks::fixed_bullet(&schedule, base, y, n as usize, false);
            checks::close(
                accrued_interest_amount(&schedule, as_of, &cfg).unwrap(),
                checks::linear_accrued(&schedule, as_of),
                1e-7,
            );
            b.iter(|| {
                black_box(
                    accrued_interest_amount(
                        black_box(&schedule),
                        black_box(as_of),
                        black_box(&cfg),
                    )
                    .unwrap(),
                )
            });
        });
    }
    group.finish();
}

/// Accrued interest once per exercise date: rebuild vs prebuilt [`AccrualIndex`].
fn bench_accrued_per_exercise_date(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_accrued_per_exercise_date");

    for years in [5i32, 10, 20, 40] {
        let n = (years * 12) as u64;
        group.throughput(Throughput::Elements(n));
        for (label, rebuild) in [("per_call_rebuild", true), ("prebuilt_index", false)] {
            group.bench_with_input(BenchmarkId::new(label, n), &years, |b, &y| {
                let base = base_date();
                let cfg = AccrualConfig::default();
                let schedule = build_monthly(base, y);
                checks::fixed_bullet(&schedule, base, y, n as usize, false);
                let exercise_dates: Vec<Date> = (0..n)
                    .map(|i| base + time::Duration::days((i as i64) * 30 + 15))
                    .collect();
                assert_eq!(exercise_dates.len(), n as usize);
                assert!(exercise_dates.windows(2).all(|pair| pair[0] < pair[1]));
                let index = AccrualIndex::build(&schedule, &cfg).unwrap();
                let mut expected_total = 0.0;
                let mut actual_total = 0.0;
                for &date in &exercise_dates {
                    let expected = checks::linear_accrued(&schedule, date);
                    let actual = if rebuild {
                        accrued_interest_amount(&schedule, date, &cfg).unwrap()
                    } else {
                        index.accrued_at(date).unwrap()
                    };
                    checks::close(actual, expected, 1e-7);
                    expected_total += expected;
                    actual_total += actual;
                }
                checks::close(actual_total, expected_total, 1e-6);
                drop(index);
                if rebuild {
                    b.iter(|| {
                        let mut acc = 0.0;
                        for &date in &exercise_dates {
                            acc += black_box(
                                accrued_interest_amount(
                                    black_box(&schedule),
                                    black_box(date),
                                    black_box(&cfg),
                                )
                                .unwrap(),
                            );
                        }
                        black_box(acc)
                    });
                } else {
                    b.iter(|| {
                        // One reusable index is deliberately prepared inside each operation.
                        let index =
                            AccrualIndex::build(black_box(&schedule), black_box(&cfg)).unwrap();
                        let mut acc = 0.0;
                        for &date in &exercise_dates {
                            acc += black_box(index.accrued_at(black_box(date)).unwrap());
                        }
                        black_box(acc)
                    });
                }
            });
        }
    }
    group.finish();
}

fn bench_build_overnight_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_build_overnight_quarterly");

    for years in [5i32, 10, 20] {
        let n = (years * 4) as u64;
        group.throughput(Throughput::Elements(n));
        group.bench_with_input(BenchmarkId::from_parameter(n), &years, |b, &y| {
            let base = fixtures::base_date();
            let market = fixtures::make_forward_market(base);
            let method =
                finstack_quant_cashflows::builder::FloatingLegCompounding::CompoundedInArrears {
                    lookback_days: 0,
                };
            let schedule = fixtures::build_overnight(base, y, method, &market);
            checks::floating_bullet(&schedule, base, y, n as usize, true);
            drop(schedule);
            b.iter(|| {
                black_box(fixtures::build_overnight(
                    black_box(base),
                    black_box(y),
                    method,
                    black_box(&market),
                ))
            });
        });
    }
    group.finish();
}

fn bench_floating_term_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_build_floating_term");

    for years in [5i32, 10, 20, 40] {
        let n = (years * 4) as u64;
        group.throughput(Throughput::Elements(n));
        group.bench_with_input(BenchmarkId::from_parameter(n), &years, |b, &y| {
            let base = fixtures::base_date();
            let market = fixtures::make_forward_market(base);
            let schedule = fixtures::build_floating_term(base, y, &market);
            checks::floating_bullet(&schedule, base, y, n as usize, false);
            drop(schedule);
            b.iter(|| {
                black_box(fixtures::build_floating_term(
                    black_box(base),
                    black_box(y),
                    black_box(&market),
                ))
            });
        });
    }
    group.finish();
}

fn bench_pv_by_period_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_pv_by_period");

    for years in [5i32, 10, 20, 40] {
        let n = (years * 12) as u64;
        group.throughput(Throughput::Elements(n));
        group.bench_with_input(BenchmarkId::from_parameter(n), &years, |b, &y| {
            let base = fixtures::base_date();
            let market = fixtures::make_discount_market(base);
            let disc_id = finstack_quant_core::types::CurveId::from("USD-OIS");
            let schedule = fixtures::build_monthly(base, y);
            let periods = fixtures::make_quarterly_periods(base, (y * 4) as u32 + 4);
            checks::fixed_bullet(&schedule, base, y, n as usize, false);
            let output = schedule
                .pv_by_period(&periods, &market, &disc_id, None, base)
                .unwrap();
            checks::period_pv(&schedule, &periods, &output, &market, base, None);
            drop(output);
            b.iter(|| {
                black_box(
                    black_box(&schedule)
                        .pv_by_period(
                            black_box(&periods),
                            black_box(&market),
                            black_box(&disc_id),
                            None,
                            base,
                        )
                        .unwrap(),
                )
            });
        });
    }
    group.finish();
}

fn bench_outstanding_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_outstanding_by_date");

    for n in [40usize, 80, 160, 320] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            let base = fixtures::base_date();
            let schedule = fixtures::make_amortizing_schedule(base, n);
            checks::amortization_path(&schedule, n, base);
            b.iter(|| black_box(black_box(&schedule).outstanding_by_date().unwrap()));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_build_scaling,
    bench_amortizing_build_scaling,
    bench_adjustment_axes,
    bench_build_overnight_scaling,
    bench_floating_term_scaling,
    bench_accrued_single,
    bench_accrued_per_exercise_date,
    accrual_workloads::register,
    bench_pv_by_period_scaling,
    bench_outstanding_scaling,
);
criterion_main!(benches);
