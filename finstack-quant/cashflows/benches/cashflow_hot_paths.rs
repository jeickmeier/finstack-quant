//! Hot-path benchmarks for `finstack-quant-cashflows`.
//!
//! Covers the computationally intensive paths identified in the performance
//! review:
//!
//! - `pv_by_period`: periodized PV aggregation (plain and credit-adjusted)
//! - `build`: full schedule generation (fixed bond, floating loan)
//! - `aggregate_by_period`: nominal dated-flow aggregation
//! - `npv`: per-instrument NPV (allocation-per-call pattern)
//! - `merge_cashflow_schedules`: k-way schedule concatenation + sort
//! - `outstanding_by_date`: balance-path tracking for amortizing instruments
//! - `weighted_average_life`: WAL over principal flows
//!
//! Run with:
//! ```sh
//! cargo bench -p finstack-quant-cashflows
//! ```

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

#[path = "support/fixtures.rs"]
mod fixtures;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_quant_cashflows::aggregation::{
    aggregate_by_period, aggregate_cashflows_checked, calendar_year_ladder, DateContext,
};
use finstack_quant_cashflows::builder::schedule::merge_cashflow_schedules;
use finstack_quant_cashflows::builder::{
    CashFlowMeta, CashFlowSchedule, CouponType, FixedCouponSpec, Notional,
};
use finstack_quant_cashflows::primitives::{CFKind, CashFlow};
use finstack_quant_cashflows::DatedFlows;
use finstack_quant_cashflows::{
    accrued_interest_amount, build_cashflow_schedule_json, dated_flows_json,
    validate_cashflow_schedule_json, AccrualConfig, AccrualMethod, ExCouponRule,
};
use finstack_quant_core::cashflow::Discountable;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, DayCountContext, Period, PeriodId, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use rust_decimal_macros::dec;
use std::hint::black_box;
use time::Month;

// Shared fixtures

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
fn make_fixed_schedule(base: Date, years: i32, frequency: Tenor) -> CashFlowSchedule {
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
            schedule: finstack_quant_cashflows::builder::ScheduleParams {
                frequency,

                day_count: DayCount::Act365F,

                business_day_convention: BusinessDayConvention::ModifiedFollowing,

                calendar_id: "weekends_only".to_string(),

                stub: StubKind::None,

                end_of_month: false,

                payment_lag_days: 0,

                adjust_accrual_dates: false,
                roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
            },
        })
        .build(None)
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
            id: PeriodId::quarter(year, q).expect("valid period fixture"),
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
fn make_dated_flows(n: usize, base: Date) -> DatedFlows {
    (0..n)
        .map(|i| {
            let days = (i as i64) * 90 + 90;
            let d = base + time::Duration::days(days);
            (
                d,
                Money::new(10_000.0, Currency::USD).expect("valid money fixture"),
            )
        })
        .collect()
}

/// Build a minimal amortizing `CashFlowSchedule` with `n_principal` Amortization flows.
fn make_amortizing_schedule(base: Date, n_periods: usize) -> CashFlowSchedule {
    let per = 1_000_000.0 / n_periods as f64;
    let flows: Vec<CashFlow> = (0..n_periods)
        .map(|i| {
            let days = ((i + 1) as i64) * 90;
            CashFlow::new(
                base + time::Duration::days(days),
                None,
                Money::new(per, Currency::USD).expect("valid money fixture"),
                CFKind::Amortization,
                0.25,
                None,
            )
        })
        .collect();

    finstack_quant_cashflows::schedule_from_classified_flows(
        flows,
        DayCount::Act365F,
        finstack_quant_cashflows::ScheduleBuildOpts {
            notional_hint: Some(
                Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            ),
            meta: CashFlowMeta {
                issue_date: Some(base),
                ..Default::default()
            },
        },
    )
}

// Benchmark: pv_by_period (plain, no credit)

fn bench_pv_by_period(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_pv_by_period");
    let base = base_date();
    let market = make_market(base);
    let disc = market.get_discount("USD-OIS").unwrap();

    {
        let (years, label) = (5i32, "5y_40cf");
        let schedule = make_fixed_schedule(base, years, Tenor::quarterly());
        let n_quarters = (years * 4) as u32 + 4;
        let periods = make_quarterly_periods(base, n_quarters);

        group.throughput(Throughput::Elements(schedule.get_flows().len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            b.iter(|| {
                black_box(&schedule)
                    .pv_by_period_with_discounting(
                        black_box(&periods),
                        finstack_quant_cashflows::builder::PvDiscountSource::Discount {
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

// Benchmark: pv_by_period credit-adjusted

fn bench_pv_by_period_credit(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_pv_by_period_credit");
    let base = base_date();
    let market = make_market(base);
    let disc = market.get_discount("USD-OIS").unwrap();
    let hazard = market.get_hazard("USD-CREDIT").unwrap();

    use finstack_quant_cashflows::aggregation::DateContext;
    use finstack_quant_core::market_data::traits::Survival;

    {
        let (years, label) = (5i32, "5y_40cf");
        let schedule = make_fixed_schedule(base, years, Tenor::quarterly());
        let n_quarters = (years * 4) as u32 + 4;
        let periods = make_quarterly_periods(base, n_quarters);
        let date_ctx = DateContext::new(base, DayCount::Act365F, DayCountContext::default());

        group.throughput(Throughput::Elements(schedule.get_flows().len() as u64));

        group.bench_with_input(BenchmarkId::new("no_recovery", label), label, |b, _| {
            b.iter(|| {
                let ctx = DateContext::new(base, DayCount::Act365F, DayCountContext::default());
                black_box(&schedule)
                    .pv_by_period_with_discounting(
                        black_box(&periods),
                        finstack_quant_cashflows::builder::PvDiscountSource::Discount {
                            disc: black_box(disc.as_ref()),
                            credit: Some(finstack_quant_cashflows::builder::PvCreditAdjustment {
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
                    .pv_by_period_with_discounting(
                        black_box(&periods),
                        finstack_quant_cashflows::builder::PvDiscountSource::Discount {
                            disc: black_box(disc.as_ref()),
                            credit: Some(finstack_quant_cashflows::builder::PvCreditAdjustment {
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

// Benchmark: build (full schedule generation)

fn bench_build_fixed_schedule(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_build_fixed");
    let base = base_date();

    {
        let (years, frequency, label) = (5i32, Tenor::quarterly(), "5y_q");
        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            b.iter(|| {
                let maturity = Date::from_calendar_date(2025 + years, Month::January, 15).unwrap();
                CashFlowSchedule::builder()
                    .principal(
                        black_box(
                            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
                        ),
                        black_box(base),
                        black_box(maturity),
                    )
                    .fixed_cf(FixedCouponSpec {
                        coupon_type: CouponType::Cash,
                        rate: dec!(0.06),
                        schedule: finstack_quant_cashflows::builder::ScheduleParams {
                            frequency: black_box(frequency),

                            day_count: DayCount::Act365F,

                            business_day_convention: BusinessDayConvention::ModifiedFollowing,

                            calendar_id: "weekends_only".to_string(),

                            stub: StubKind::None,

                            end_of_month: false,

                            payment_lag_days: 0,

                            adjust_accrual_dates: false,
                            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
                        },
                    })
                    .build(None)
                    .unwrap()
            });
        });
    }

    group.finish();
}

// Benchmark: aggregate_by_period (nominal dated-flow rollup)

fn bench_aggregate_by_period(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_aggregate_by_period");
    let base = base_date();

    {
        let (n_flows, n_periods, label) = (120usize, 20u32, "120f_20p");
        let flows = make_dated_flows(n_flows, base);
        let periods = make_quarterly_periods(base, n_periods);

        group.throughput(Throughput::Elements(n_flows as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            b.iter(|| aggregate_by_period(black_box(&flows), black_box(&periods)));
        });
    }

    group.finish();
}

// Benchmark: aggregate_cashflows_checked (compensated single-ccy sum)

fn bench_aggregate_precise(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_aggregate_precise");
    let base = base_date();

    {
        let n = 120usize;
        let flows = make_dated_flows(n, base);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| aggregate_cashflows_checked(black_box(&flows), Currency::USD).unwrap());
        });
    }

    group.finish();
}

// Benchmark: CashFlowSchedule::npv (per-instrument NPV, one allocation per call)

fn bench_npv(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_npv");
    let base = base_date();
    let market = make_market(base);
    let disc = market.get_discount("USD-OIS").unwrap();

    {
        let (years, label) = (5i32, "5y");
        let schedule = make_fixed_schedule(base, years, Tenor::semi_annual());

        group.throughput(Throughput::Elements(schedule.get_flows().len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            b.iter(|| {
                black_box(&schedule)
                    .npv(black_box(disc.as_ref()), black_box(base))
                    .unwrap()
                    .amount()
            });
        });
    }

    group.finish();
}

// Benchmark: merge_cashflow_schedules (concat + re-sort)

fn bench_merge_schedules(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_merge_schedules");
    let base = base_date();

    {
        let k = 20usize;
        let schedules: Vec<CashFlowSchedule> = (0..k)
            .map(|_| make_fixed_schedule(base, 5, Tenor::semi_annual()))
            .collect();

        let total_flows: u64 = schedules.iter().map(|s| s.get_flows().len() as u64).sum();
        group.throughput(Throughput::Elements(total_flows));

        group.bench_with_input(BenchmarkId::from_parameter(k), &k, |b, _| {
            b.iter(|| {
                merge_cashflow_schedules(
                    black_box(schedules.clone()),
                    Notional::par(black_box(1_000_000.0 * k as f64), Currency::USD)
                        .expect("valid notional fixture"),
                    DayCount::Act365F,
                )
            });
        });
    }

    group.finish();
}

// Benchmark: outstanding_by_date (balance tracking)

fn bench_outstanding_by_date(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_outstanding_by_date");
    let base = base_date();

    {
        let n = 40usize;
        let schedule = make_amortizing_schedule(base, n);

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(&schedule).outstanding_by_date().unwrap());
        });
    }

    group.finish();
}

// Benchmark: weighted_average_life

fn bench_wal(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_wal");
    let base = base_date();

    {
        let n = 40usize;
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

fn bench_build_floating(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_build_floating");
    let base = fixtures::base_date();
    let market = fixtures::make_forward_market(base);

    group.bench_function("term_5y_q", |b| {
        b.iter(|| fixtures::build_floating_term(black_box(base), 5, black_box(&market)));
    });
    group.bench_function("overnight_arrears_5y_q", |b| {
        b.iter(|| {
            fixtures::build_overnight(
                black_box(base),
                5,
                finstack_quant_cashflows::builder::OvernightCompoundingMethod::CompoundedInArrears,
                black_box(&market),
            )
        });
    });
    group.bench_function("overnight_lookback5_5y_q", |b| {
        b.iter(|| {
            fixtures::build_overnight(
                black_box(base),
                5,
                finstack_quant_cashflows::builder::OvernightCompoundingMethod::CompoundedWithLookback {
                    lookback_days: 5,
                },
                black_box(&market),
            )
        });
    });
    group.finish();
}

fn bench_build_structured(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_build_structured");
    let base = fixtures::base_date();
    group.bench_function("amortizing_linear_5y_q", |b| {
        b.iter(|| fixtures::build_amortizing_linear(black_box(base), 5));
    });
    group.bench_function("fixed_plus_periodic_fee_5y_q", |b| {
        b.iter(|| fixtures::build_fixed_with_periodic_fee(black_box(base), 5));
    });
    group.finish();
}

fn bench_json_bridge(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_json_bridge");
    let spec = fixtures::five_year_fixed_json();
    let schedule_json = build_cashflow_schedule_json(spec, None).unwrap();

    group.bench_function("build_5y_q", |b| {
        b.iter(|| build_cashflow_schedule_json(black_box(spec), None).unwrap());
    });
    group.bench_function("validate_5y_q", |b| {
        b.iter(|| validate_cashflow_schedule_json(black_box(&schedule_json)).unwrap());
    });
    group.bench_function("dated_flows_5y_q", |b| {
        b.iter(|| dated_flows_json(black_box(&schedule_json)).unwrap());
    });
    group.finish();
}

fn bench_calendar_year_ladder(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_calendar_year_ladder");
    let base = fixtures::base_date();
    let schedule = fixtures::make_fixed_schedule(base, 10, Tenor::quarterly());
    let dates: Vec<Date> = schedule.get_flows().iter().map(|cf| cf.date).collect();
    let kinds: Vec<&str> = schedule
        .get_flows()
        .iter()
        .map(|cf| {
            if cf.kind.is_principal_like() {
                "principal"
            } else {
                "coupon"
            }
        })
        .collect();
    let amounts: Vec<f64> = schedule
        .get_flows()
        .iter()
        .map(|cf| cf.amount.amount())
        .collect();
    let pvs: Vec<f64> = amounts.iter().map(|amount| amount * 0.85).collect();

    group.throughput(Throughput::Elements(dates.len() as u64));
    group.bench_function("10y_80cf", |b| {
        b.iter(|| {
            calendar_year_ladder(
                black_box(&dates),
                black_box(&kinds),
                black_box(&amounts),
                black_box(&pvs),
            )
            .unwrap()
        });
    });
    group.finish();
}

fn bench_accrued_variants(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_accrued_variants");
    let base = fixtures::base_date();
    let schedule = fixtures::make_fixed_schedule(base, 5, Tenor::quarterly());
    let as_of = base + time::Duration::days(400);

    group.bench_function("compounded_5y", |b| {
        let cfg = AccrualConfig {
            method: AccrualMethod::Compounded,
            ..Default::default()
        };
        b.iter(|| {
            accrued_interest_amount(black_box(&schedule), black_box(as_of), black_box(&cfg))
                .unwrap()
        });
    });
    group.bench_function("ex_coupon_usny_5y", |b| {
        let cfg = AccrualConfig {
            ex_coupon: Some(ExCouponRule {
                days_before_coupon: 7,
                calendar_id: Some("usny".to_string()),
            }),
            ..Default::default()
        };
        b.iter(|| {
            accrued_interest_amount(black_box(&schedule), black_box(as_of), black_box(&cfg))
                .unwrap()
        });
    });
    group.finish();
}

// Registration

criterion_group!(
    benches,
    bench_pv_by_period,
    bench_pv_by_period_credit,
    bench_build_fixed_schedule,
    bench_build_floating,
    bench_build_structured,
    bench_json_bridge,
    bench_calendar_year_ladder,
    bench_accrued_variants,
    bench_aggregate_by_period,
    bench_aggregate_precise,
    bench_npv,
    bench_merge_schedules,
    bench_outstanding_by_date,
    bench_wal,
);
criterion_main!(benches);
