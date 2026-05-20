//! Hot-path benchmarks for `finstack-cashflows`.
//!
//! Covers the computationally intensive paths identified in the performance
//! review:
//!
//! - `sort_flows`: multi-key unstable sort at different schedule sizes
//! - `pv_by_period`: periodized PV aggregation (plain and credit-adjusted)
//! - `to_period_dataframe`: DataFrame export with O(n+m) cursor vs prior O(n×m)
//! - `build_with_curves`: full schedule generation (fixed bond, floating loan)
//! - `aggregate_by_period`: nominal dated-flow aggregation
//! - `npv`: per-instrument NPV (allocation-per-call pattern)
//! - `merge_cashflow_schedules`: k-way schedule concatenation + sort
//! - `outstanding_by_date`: balance-path tracking for amortizing instruments
//! - `weighted_average_life`: WAL over principal flows
//!
//! Run with:
//! ```sh
//! cargo bench -p finstack-cashflows
//! ```

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_cashflows::aggregation::{
    aggregate_by_period, aggregate_cashflows_checked, DateContext,
};
use finstack_cashflows::builder::schedule::{merge_cashflow_schedules, sort_flows};
use finstack_cashflows::builder::{
    CashFlowMeta, CashFlowSchedule, CashflowRepresentation, CouponType, FixedCouponSpec, Notional,
    PeriodDataFrameOptions,
};
use finstack_cashflows::primitives::{CFKind, CashFlow};
use finstack_cashflows::DatedFlow;
use finstack_core::cashflow::Discountable;
use finstack_core::currency::Currency;
use finstack_core::dates::{
    BusinessDayConvention, Date, DayCount, DayCountContext, Period, PeriodId, StubKind, Tenor,
};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use rust_decimal_macros::dec;
use std::hint::black_box;
use time::Month;

// =============================================================================
// Shared fixtures
// =============================================================================

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 15).unwrap()
}

/// Flat discount curve + flat hazard curve in a single `MarketContext`.
fn make_market(base: Date) -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.951),
            (3.0, 0.865),
            (5.0, 0.790),
            (10.0, 0.640),
            (30.0, 0.375),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    let hazard = HazardCurve::builder("USD-CREDIT")
        .base_date(base)
        .recovery_rate(0.40)
        .knots([(0.0, 0.015), (5.0, 0.015), (10.0, 0.015)])
        .build()
        .unwrap();

    MarketContext::new().insert(disc).insert(hazard)
}

/// Fixed-rate bullet bond schedule: `years` maturity, semi-annual or quarterly.
fn make_fixed_schedule(base: Date, years: i32, freq: Tenor) -> CashFlowSchedule {
    let maturity = Date::from_calendar_date(2025 + years, Month::January, 15).unwrap();
    CashFlowSchedule::builder()
        .principal(Money::new(1_000_000.0, Currency::USD), base, maturity)
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: dec!(0.06),
            freq,
            dc: DayCount::Act365F,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
        })
        .build_with_curves(None)
        .unwrap()
}

/// Quarterly reporting periods covering `n_quarters` from `base`.
fn make_quarterly_periods(base: Date, n_quarters: u32) -> Vec<Period> {
    let mut periods = Vec::with_capacity(n_quarters as usize);
    let mut year = base.year();
    let mut q = ((base.month() as u8 - 1) / 3) + 1;

    for _ in 0..n_quarters {
        let start_month = (q - 1) * 3 + 1;
        let end_month = q * 3;
        let end_year = if end_month == 12 { year + 1 } else { year };
        let end_m = if end_month == 12 { 1 } else { end_month + 1 };

        let start =
            Date::from_calendar_date(year, Month::try_from(start_month).unwrap(), 1).unwrap();
        let end = Date::from_calendar_date(end_year, Month::try_from(end_m).unwrap(), 1).unwrap();

        periods.push(Period {
            id: PeriodId::quarter(year, q),
            start,
            end,
            is_actual: true,
        });

        q += 1;
        if q > 4 {
            q = 1;
            year += 1;
        }
    }
    periods
}

/// Dated flows spanning `years` years with quarterly payments.
fn make_dated_flows(n: usize, base: Date) -> Vec<DatedFlow> {
    (0..n)
        .map(|i| {
            let days = (i as i64) * 90 + 90;
            let d = base + time::Duration::days(days);
            (d, Money::new(10_000.0, Currency::USD))
        })
        .collect()
}

/// Random-ish `CashFlow` slice of length `n` for sort benchmarks.
fn make_unsorted_flows(n: usize, base: Date) -> Vec<CashFlow> {
    (0..n)
        .map(|i| {
            // Interleave dates to produce a partially-unsorted sequence.
            let offset = ((i * 17 + 3) % n) as i64 * 90;
            CashFlow {
                date: base + time::Duration::days(offset),
                reset_date: None,
                amount: Money::new(1_000.0 + (i as f64 * 13.7), Currency::USD),
                kind: if i % 5 == 0 {
                    CFKind::Amortization
                } else {
                    CFKind::Fixed
                },
                accrual_factor: 0.25,
                rate: Some(0.06),
            }
        })
        .collect()
}

/// Build a minimal amortizing `CashFlowSchedule` with `n_principal` Amortization flows.
fn make_amortizing_schedule(base: Date, n_periods: usize) -> CashFlowSchedule {
    let per = 1_000_000.0 / n_periods as f64;
    let flows: Vec<CashFlow> = (0..n_periods)
        .map(|i| {
            let days = ((i + 1) as i64) * 90;
            CashFlow {
                date: base + time::Duration::days(days),
                reset_date: None,
                amount: Money::new(per, Currency::USD),
                kind: CFKind::Amortization,
                accrual_factor: 0.25,
                rate: None,
            }
        })
        .collect();

    finstack_cashflows::schedule_from_classified_flows(
        flows,
        DayCount::Act365F,
        finstack_cashflows::ScheduleBuildOpts {
            notional_hint: Some(Money::new(1_000_000.0, Currency::USD)),
            meta: Some(CashFlowMeta::default()),
            ..Default::default()
        },
    )
}

// =============================================================================
// Benchmark: sort_flows
// =============================================================================

fn bench_sort_flows(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_sort_flows");
    let base = base_date();

    for n in [20usize, 120, 360] {
        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(BenchmarkId::new("unsorted", n), &n, |b, &n| {
            let template = make_unsorted_flows(n, base);
            b.iter(|| {
                let mut flows = template.clone();
                sort_flows(black_box(&mut flows));
                flows
            });
        });

        group.bench_with_input(BenchmarkId::new("pre_sorted", n), &n, |b, &n| {
            let mut template = make_unsorted_flows(n, base);
            sort_flows(&mut template);
            b.iter(|| {
                let mut flows = template.clone();
                sort_flows(black_box(&mut flows));
                flows
            });
        });
    }

    group.finish();
}

// =============================================================================
// Benchmark: pv_by_period (plain, no credit)
// =============================================================================

fn bench_pv_by_period(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_pv_by_period");
    let base = base_date();
    let market = make_market(base);
    let disc = market.get_discount("USD-OIS").unwrap();

    for (years, label) in [(2i32, "2y_20cf"), (5, "5y_40cf"), (30, "30y_360cf")] {
        let schedule = make_fixed_schedule(base, years, Tenor::quarterly());
        let n_quarters = (years * 4) as u32 + 4;
        let periods = make_quarterly_periods(base, n_quarters);

        group.throughput(Throughput::Elements(schedule.flows.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            b.iter(|| {
                black_box(&schedule)
                    .pv_by_period(
                        black_box(&periods),
                        finstack_cashflows::builder::PvDiscountSource::Discount {
                            disc: black_box(disc.as_ref()),
                            credit: None,
                        },
                        DateContext::new(
                            black_box(base),
                            DayCount::Act365F,
                            DayCountContext::default(),
                        ),
                    )
                    .unwrap()
            });
        });
    }

    group.finish();
}

// =============================================================================
// Benchmark: pv_by_period credit-adjusted
// =============================================================================

fn bench_pv_by_period_credit(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_pv_by_period_credit");
    let base = base_date();
    let market = make_market(base);
    let disc = market.get_discount("USD-OIS").unwrap();
    let hazard = market.get_hazard("USD-CREDIT").unwrap();

    use finstack_cashflows::aggregation::DateContext;
    use finstack_core::market_data::traits::Survival;

    for (years, label) in [(5i32, "5y_40cf"), (30, "30y_360cf")] {
        let schedule = make_fixed_schedule(base, years, Tenor::quarterly());
        let n_quarters = (years * 4) as u32 + 4;
        let periods = make_quarterly_periods(base, n_quarters);
        let date_ctx = DateContext::new(base, DayCount::Act365F, DayCountContext::default());

        group.throughput(Throughput::Elements(schedule.flows.len() as u64));

        group.bench_with_input(BenchmarkId::new("no_recovery", label), label, |b, _| {
            b.iter(|| {
                let ctx = DateContext::new(base, DayCount::Act365F, DayCountContext::default());
                black_box(&schedule)
                    .pv_by_period(
                        black_box(&periods),
                        finstack_cashflows::builder::PvDiscountSource::Discount {
                            disc: black_box(disc.as_ref()),
                            credit: Some(finstack_cashflows::builder::PvCreditAdjustment {
                                hazard: Some(black_box(hazard.as_ref() as &dyn Survival)),
                                recovery_rate: None,
                            }),
                        },
                        black_box(ctx),
                    )
                    .unwrap()
            });
        });

        group.bench_with_input(BenchmarkId::new("with_recovery", label), label, |b, _| {
            let _ = date_ctx;
            b.iter(|| {
                let ctx = DateContext::new(base, DayCount::Act365F, DayCountContext::default());
                black_box(&schedule)
                    .pv_by_period(
                        black_box(&periods),
                        finstack_cashflows::builder::PvDiscountSource::Discount {
                            disc: black_box(disc.as_ref()),
                            credit: Some(finstack_cashflows::builder::PvCreditAdjustment {
                                hazard: Some(black_box(hazard.as_ref() as &dyn Survival)),
                                recovery_rate: Some(0.40),
                            }),
                        },
                        black_box(ctx),
                    )
                    .unwrap()
            });
        });
    }

    group.finish();
}

// =============================================================================
// Benchmark: to_period_dataframe (hot O(n+m) cursor path)
// =============================================================================

fn bench_period_dataframe(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_period_dataframe");
    let base = base_date();
    let market = make_market(base);

    for (years, n_periods, label) in [
        (5i32, 20u32, "5y_40cf_20p"),
        (10, 40, "10y_80cf_40p"),
        (30, 120, "30y_360cf_120p"),
    ] {
        let schedule = make_fixed_schedule(base, years, Tenor::quarterly());
        let periods = make_quarterly_periods(base, n_periods);

        group.throughput(Throughput::Elements(schedule.flows.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            let options = PeriodDataFrameOptions {
                as_of: Some(base),
                day_count: Some(DayCount::Act365F),
                ..Default::default()
            };
            b.iter(|| {
                black_box(&schedule)
                    .to_period_dataframe(
                        black_box(&periods),
                        black_box(&market),
                        "USD-OIS",
                        black_box(options.clone()),
                    )
                    .unwrap()
            });
        });

        group.bench_with_input(BenchmarkId::new("with_hazard", label), label, |b, _| {
            let options = PeriodDataFrameOptions {
                as_of: Some(base),
                day_count: Some(DayCount::Act365F),
                credit_curve_id: Some("USD-CREDIT"),
                ..Default::default()
            };
            b.iter(|| {
                black_box(&schedule)
                    .to_period_dataframe(
                        black_box(&periods),
                        black_box(&market),
                        "USD-OIS",
                        black_box(options.clone()),
                    )
                    .unwrap()
            });
        });
    }

    group.finish();
}

// =============================================================================
// Benchmark: build_with_curves (full schedule generation)
// =============================================================================

fn bench_build_fixed_schedule(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_build_fixed");
    let base = base_date();

    for (years, freq, label) in [
        (2i32, Tenor::semi_annual(), "2y_sa"),
        (5, Tenor::quarterly(), "5y_q"),
        (10, Tenor::semi_annual(), "10y_sa"),
        (30, Tenor::semi_annual(), "30y_sa"),
        (30, Tenor::monthly(), "30y_m"),
    ] {
        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            b.iter(|| {
                let maturity = Date::from_calendar_date(2025 + years, Month::January, 15).unwrap();
                CashFlowSchedule::builder()
                    .principal(
                        black_box(Money::new(1_000_000.0, Currency::USD)),
                        black_box(base),
                        black_box(maturity),
                    )
                    .fixed_cf(FixedCouponSpec {
                        coupon_type: CouponType::Cash,
                        rate: dec!(0.06),
                        freq: black_box(freq),
                        dc: DayCount::Act365F,
                        bdc: BusinessDayConvention::ModifiedFollowing,
                        calendar_id: "weekends_only".to_string(),
                        stub: StubKind::None,
                        end_of_month: false,
                        payment_lag_days: 0,
                    })
                    .build_with_curves(None)
                    .unwrap()
            });
        });
    }

    group.finish();
}

// =============================================================================
// Benchmark: aggregate_by_period (nominal dated-flow rollup)
// =============================================================================

fn bench_aggregate_by_period(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_aggregate_by_period");
    let base = base_date();

    for (n_flows, n_periods, label) in [
        (40usize, 8u32, "40f_8p"),
        (120, 20, "120f_20p"),
        (400, 40, "400f_40p"),
        (1000, 80, "1000f_80p"),
    ] {
        let flows = make_dated_flows(n_flows, base);
        let periods = make_quarterly_periods(base, n_periods);

        group.throughput(Throughput::Elements(n_flows as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            b.iter(|| aggregate_by_period(black_box(&flows), black_box(&periods)));
        });
    }

    group.finish();
}

// =============================================================================
// Benchmark: aggregate_cashflows_checked (compensated single-ccy sum)
// =============================================================================

fn bench_aggregate_precise(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_aggregate_precise");
    let base = base_date();

    for n in [40usize, 120, 400, 1000] {
        let flows = make_dated_flows(n, base);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| aggregate_cashflows_checked(black_box(&flows), Currency::USD).unwrap());
        });
    }

    group.finish();
}

// =============================================================================
// Benchmark: CashFlowSchedule::npv (per-instrument NPV, one allocation per call)
// =============================================================================

fn bench_npv(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_npv");
    let base = base_date();
    let market = make_market(base);
    let disc = market.get_discount("USD-OIS").unwrap();

    for (years, label) in [(2i32, "2y"), (5, "5y"), (30, "30y")] {
        let schedule = make_fixed_schedule(base, years, Tenor::semi_annual());

        group.throughput(Throughput::Elements(schedule.flows.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            b.iter(|| {
                black_box(&schedule)
                    .npv(
                        black_box(disc.as_ref()),
                        black_box(base),
                        Some(DayCount::Act365F),
                    )
                    .unwrap()
                    .amount()
            });
        });
    }

    group.finish();
}

// =============================================================================
// Benchmark: merge_cashflow_schedules (concat + re-sort)
// =============================================================================

fn bench_merge_schedules(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_merge_schedules");
    let base = base_date();

    for k in [5usize, 20, 50, 100] {
        let schedules: Vec<CashFlowSchedule> = (0..k)
            .map(|_| make_fixed_schedule(base, 5, Tenor::semi_annual()))
            .collect();

        let total_flows: u64 = schedules.iter().map(|s| s.flows.len() as u64).sum();
        group.throughput(Throughput::Elements(total_flows));

        group.bench_with_input(BenchmarkId::from_parameter(k), &k, |b, _| {
            b.iter(|| {
                merge_cashflow_schedules(
                    black_box(schedules.clone()),
                    Notional::par(black_box(1_000_000.0 * k as f64), Currency::USD),
                    DayCount::Act365F,
                )
                .expect("matched currencies should merge")
            });
        });
    }

    group.finish();
}

// =============================================================================
// Benchmark: outstanding_by_date (balance tracking)
// =============================================================================

fn bench_outstanding_by_date(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_outstanding_by_date");
    let base = base_date();

    for n in [8usize, 20, 40, 120] {
        let schedule = make_amortizing_schedule(base, n);

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(&schedule).outstanding_by_date().unwrap());
        });
    }

    group.finish();
}

// =============================================================================
// Benchmark: weighted_average_life
// =============================================================================

fn bench_wal(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_wal");
    let base = base_date();

    for n in [8usize, 20, 40, 120] {
        let schedule = make_amortizing_schedule(base, n);

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                black_box(&schedule)
                    .weighted_average_life(black_box(base))
                    .ok()
            });
        });
    }

    group.finish();
}

// =============================================================================
// Benchmark: normalize_public (filter + sort on every public schedule call)
// =============================================================================

fn bench_normalize_public(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_normalize_public");
    let base = base_date();

    for (years, label) in [(5i32, "5y"), (30, "30y")] {
        let schedule = make_fixed_schedule(base, years, Tenor::semi_annual());

        group.throughput(Throughput::Elements(schedule.flows.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            b.iter(|| {
                black_box(schedule.clone())
                    .normalize_public(black_box(base), CashflowRepresentation::Contractual)
            });
        });
    }

    group.finish();
}

// =============================================================================
// Registration
// =============================================================================

criterion_group!(
    benches,
    bench_sort_flows,
    bench_pv_by_period,
    bench_pv_by_period_credit,
    bench_period_dataframe,
    bench_build_fixed_schedule,
    bench_aggregate_by_period,
    bench_aggregate_precise,
    bench_npv,
    bench_merge_schedules,
    bench_outstanding_by_date,
    bench_wal,
    bench_normalize_public,
);
criterion_main!(benches);
