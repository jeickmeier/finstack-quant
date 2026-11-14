//! Checked full-call cashflow benchmarks at representative fixed sizes.
//!
//! Fixture creation and independent output checks run only for selected cases.
//! Extraction-plus-NPV and clone-plus-merge deliberately retain their original
//! setup, allocation and destruction work inside the measured operation.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

#[path = "support/checks.rs"]
mod checks;
#[path = "support/fixtures.rs"]
mod fixtures;
#[path = "support/history_workloads.rs"]
mod history_workloads;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use finstack_quant_cashflows::accrual::{
    AccrualConfig, AccrualMethod, ExCouponRule, accrued_interest_amount,
};
use finstack_quant_cashflows::aggregation::{
    aggregate_by_period, aggregate_cashflows_checked, calendar_year_ladder,
};
use finstack_quant_cashflows::builder::{
    CashFlowSchedule, FloatingLegCompounding, Notional, merge_cashflow_schedules,
};
use finstack_quant_cashflows::{
    DatedFlowJson, build_cashflow_schedule_json, dated_flows_json, validate_cashflow_schedule_json,
};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::types::CurveId;
use std::hint::black_box;

fn bench_pv_by_period(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_pv_by_period");
    group.throughput(Throughput::Elements(22));
    group.bench_function("5y_q_22rows", |b| {
        let base = fixtures::base_date();
        let market = fixtures::make_discount_market(base);
        let disc_id = CurveId::from("USD-OIS");
        let schedule = fixtures::make_fixed_schedule(base, 5, Tenor::quarterly());
        let periods = fixtures::make_quarterly_periods(base, 24);
        checks::fixed_bullet(&schedule, base, 5, 20, true);
        let preflight = schedule
            .pv_by_period(&periods, &market, &disc_id, None, base)
            .unwrap();
        checks::period_pv(&schedule, &periods, &preflight, &market, base, None);
        drop(preflight);
        b.iter(|| {
            black_box(
                black_box(&schedule)
                    .pv_by_period(
                        black_box(&periods),
                        black_box(&market),
                        black_box(&disc_id),
                        None,
                        black_box(base),
                    )
                    .expect("valid risk-free reporting PV"),
            )
        });
    });
    group.finish();
}

fn bench_pv_by_period_credit(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_pv_by_period_credit");
    group.throughput(Throughput::Elements(22));
    for (label, recovery) in [("no_recovery", 0.0), ("with_recovery", 0.40)] {
        group.bench_with_input(
            BenchmarkId::new(label, "5y_q_22rows"),
            &recovery,
            |b, &recovery| {
                let base = fixtures::base_date();
                let hazard = HazardCurve::builder("BENCH-CREDIT")
                    .base_date(base)
                    .recovery_rate(recovery)
                    .knots([(0.0, 0.015), (5.0, 0.015), (10.0, 0.015)])
                    .build()
                    .unwrap();
                let market = fixtures::make_discount_market(base).insert(hazard);
                let disc_id = CurveId::from("USD-OIS");
                let credit_id = CurveId::from("BENCH-CREDIT");
                let schedule = fixtures::make_fixed_schedule(base, 5, Tenor::quarterly());
                let periods = fixtures::make_quarterly_periods(base, 24);
                checks::fixed_bullet(&schedule, base, 5, 20, true);
                let preflight = schedule
                    .pv_by_period(&periods, &market, &disc_id, Some(&credit_id), base)
                    .unwrap();
                checks::period_pv(
                    &schedule,
                    &periods,
                    &preflight,
                    &market,
                    base,
                    Some(recovery),
                );
                drop(preflight);
                b.iter(|| {
                    black_box(
                        black_box(&schedule)
                            .pv_by_period(
                                black_box(&periods),
                                black_box(&market),
                                black_box(&disc_id),
                                Some(black_box(&credit_id)),
                                black_box(base),
                            )
                            .expect("valid credit reporting PV"),
                    )
                });
            },
        );
    }
    group.finish();
}

fn bench_build_fixed_schedule(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_build_fixed");
    group.throughput(Throughput::Elements(20));
    group.bench_function("5y_q", |b| {
        let base = fixtures::base_date();
        checks::fixed_bullet(
            &fixtures::make_fixed_schedule(base, 5, Tenor::quarterly()),
            base,
            5,
            20,
            true,
        );
        b.iter(|| {
            black_box(fixtures::make_fixed_schedule(
                black_box(base),
                black_box(5),
                black_box(Tenor::quarterly()),
            ))
        });
    });
    group.finish();
}

fn bench_aggregate_by_period(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_aggregate_by_period");
    group.throughput(Throughput::Elements(120));
    group.bench_function("120inputs_20periods_20included", |b| {
        let base = fixtures::base_date();
        let flows = fixtures::make_dated_flows(120, base);
        let periods = fixtures::make_quarterly_periods(base, 20);
        assert_eq!(flows.len(), 120);
        assert_eq!(periods.len(), 20);
        let preflight = aggregate_by_period(&flows, &periods).expect("valid nominal aggregation");
        let mut included = 0;
        let mut occupied = 0;
        for period in &periods {
            let count = flows
                .iter()
                .filter(|(date, _)| period.start <= *date && *date < period.end)
                .count();
            included += count;
            if count == 0 {
                assert!(!preflight.contains_key(&period.id));
            } else {
                occupied += 1;
                let by_currency = preflight.get(&period.id).expect("occupied nominal bucket");
                assert_eq!(by_currency.len(), 1);
                let amount = by_currency.get(&Currency::USD).expect("USD aggregation");
                assert_eq!(amount.currency(), Currency::USD);
                checks::close(amount.amount(), count as f64 * 10_000.0, 0.0);
            }
        }
        assert_eq!(included, 20);
        assert_eq!(preflight.len(), occupied);
        for (index, (date, amount)) in flows.iter().enumerate() {
            assert_eq!(*date, base + time::Duration::days((index as i64 + 1) * 90));
            assert_eq!(amount.currency(), Currency::USD);
            checks::close(amount.amount(), 10_000.0, 0.0);
        }
        drop(preflight);
        b.iter(|| {
            black_box(
                aggregate_by_period(black_box(&flows), black_box(&periods))
                    .expect("valid nominal aggregation"),
            )
        });
    });
    group.finish();
}

fn bench_aggregate_precise(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_aggregate_precise");
    group.throughput(Throughput::Elements(120));
    group.bench_function("120", |b| {
        let flows = fixtures::make_dated_flows(120, fixtures::base_date());
        let preflight = aggregate_cashflows_checked(&flows, Currency::USD).unwrap();
        assert_eq!(preflight.currency(), Currency::USD);
        checks::close(preflight.amount(), 1_200_000.0, 0.0);
        b.iter(|| {
            black_box(
                aggregate_cashflows_checked(black_box(&flows), Currency::USD)
                    .expect("valid single-currency sum"),
            )
        });
    });
    group.finish();
}

fn bench_npv(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_extract_and_npv");
    group.throughput(Throughput::Elements(12));
    group.bench_function("5y_semiannual_12rows", |b| {
        let base = fixtures::base_date();
        let market = fixtures::make_discount_market(base);
        let disc = market.get_discount("USD-OIS").unwrap();
        let schedule = fixtures::make_fixed_schedule(base, 5, Tenor::semi_annual());
        checks::fixed_bullet(&schedule, base, 5, 10, true);
        let flows = schedule.dated_flows().unwrap();
        assert_eq!(flows.len(), 12);
        for (row, flow) in flows.iter().zip(schedule.get_flows()) {
            assert_eq!(*row, (flow.get_date(), flow.get_amount()));
        }
        let expected: f64 = flows
            .iter()
            .filter(|(date, _)| *date > base)
            .map(|(date, amount)| {
                amount.amount() * disc.df_between_dates(base, *date).expect("fixture DF")
            })
            .sum();
        let preflight =
            finstack_quant_core::cashflow::npv(disc.as_ref(), base, &flows, false).unwrap();
        assert_eq!(preflight.currency(), Currency::USD);
        checks::close(preflight.amount(), expected, 1e-6);
        drop(flows);
        b.iter(|| {
            let flows = black_box(&schedule)
                .dated_flows()
                .expect("valid dated cash extraction");
            black_box(
                finstack_quant_core::cashflow::npv(
                    black_box(disc.as_ref()),
                    black_box(base),
                    black_box(&flows),
                    false,
                )
                .expect("valid extracted-cash NPV"),
            )
        });
    });
    group.finish();
}

fn bench_merge_schedules(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_clone_and_merge_schedules");
    group.throughput(Throughput::Elements(240));
    group.bench_function("20schedules_240rows", |b| {
        let base = fixtures::base_date();
        let schedules: Vec<_> = (0..20)
            .map(|_| fixtures::make_fixed_schedule(base, 5, Tenor::semi_annual()))
            .collect();
        for schedule in &schedules {
            checks::fixed_bullet(schedule, base, 5, 10, true);
        }
        let preflight = merge_cashflow_schedules(
            schedules.clone(),
            fixtures::notional(20_000_000.0),
            DayCount::Act365F,
        );
        checks::rows(&preflight, 240);
        assert_eq!(preflight.coupons().count(), 200);
        assert_eq!(preflight.get_meta().issue_date, Some(base));
        assert_eq!(
            preflight.get_meta().maturity,
            schedules[0].get_meta().maturity
        );
        assert_eq!(
            preflight.get_meta().calendar_ids,
            vec!["weekends_only".to_string()]
        );
        assert!(preflight.get_meta().projected_fixings.is_empty());
        for input in schedules[0].get_flows() {
            assert_eq!(
                preflight
                    .get_flows()
                    .iter()
                    .filter(|row| row.get_date() == input.get_date()
                        && row.get_kind() == input.get_kind()
                        && row.get_amount() == input.get_amount())
                    .count(),
                20
            );
        }
        drop(preflight);
        b.iter(|| {
            black_box(merge_cashflow_schedules(
                black_box(schedules.clone()),
                Notional::par(black_box(20_000_000.0), Currency::USD)
                    .expect("valid merged notional"),
                DayCount::Act365F,
            ))
        });
    });
    group.finish();
}

fn bench_outstanding_by_date(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_outstanding_by_date");
    group.throughput(Throughput::Elements(40));
    group.bench_function("40", |b| {
        let base = fixtures::base_date();
        let schedule = fixtures::make_amortizing_schedule(base, 40);
        checks::amortization_path(&schedule, 40, base);
        b.iter(|| {
            black_box(
                black_box(&schedule)
                    .outstanding_by_date()
                    .expect("valid balance replay"),
            )
        });
    });
    group.finish();
}

fn bench_wal(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_wal");
    group.throughput(Throughput::Elements(40));
    group.bench_function("40", |b| {
        let base = fixtures::base_date();
        let schedule = fixtures::make_amortizing_schedule(base, 40);
        checks::amortization_path(&schedule, 40, base);
        let expected = 90.0 / 365.0 * 41.0 / 2.0;
        checks::close(
            schedule.wal(base).expect("valid principal WAL"),
            expected,
            1e-12,
        );
        b.iter(|| {
            black_box(
                black_box(&schedule)
                    .wal(black_box(base))
                    .expect("valid principal WAL"),
            )
        });
    });
    group.finish();
}

fn bench_build_floating(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_build_floating");
    group.throughput(Throughput::Elements(20));
    group.bench_function("term_5y_q", |b| {
        let base = fixtures::base_date();
        let market = fixtures::make_forward_market(base);
        checks::floating_bullet(
            &fixtures::build_floating_term(base, 5, &market),
            base,
            5,
            20,
            false,
        );
        b.iter(|| {
            black_box(fixtures::build_floating_term(
                black_box(base),
                black_box(5),
                black_box(&market),
            ))
        });
    });
    for (label, lookback_days) in [
        ("overnight_arrears_5y_q", 0),
        ("overnight_lookback5_5y_q", 5),
    ] {
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &lookback_days,
            |b, &lookback_days| {
                let base = fixtures::base_date();
                let market = fixtures::make_forward_market(base);
                let method = FloatingLegCompounding::CompoundedInArrears { lookback_days };
                let preflight = fixtures::build_overnight(base, 5, method, &market);
                checks::floating_bullet(&preflight, base, 5, 20, true);
                let mut first_observation = base;
                for _ in 0..lookback_days {
                    first_observation -= time::Duration::days(1);
                    while finstack_quant_core::dates::DateExt::is_weekend(first_observation) {
                        first_observation -= time::Duration::days(1);
                    }
                }
                assert_eq!(
                    preflight
                        .get_meta()
                        .projected_fixings
                        .first()
                        .expect("first raw fixing")
                        .date,
                    first_observation
                );
                drop(preflight);
                b.iter(|| {
                    black_box(fixtures::build_overnight(
                        black_box(base),
                        black_box(5),
                        black_box(method),
                        black_box(&market),
                    ))
                });
            },
        );
    }
    group.finish();
}

fn bench_build_structured(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_build_structured");
    group.bench_function("amortizing_linear_5y_q", |b| {
        let base = fixtures::base_date();
        let preflight = fixtures::build_amortizing_linear(base, 5);
        checks::rows(&preflight, 41);
        checks::fixed_amounts(&preflight);
        assert_eq!(preflight.coupons().count(), 20);
        let principal: Vec<_> = preflight
            .get_flows()
            .iter()
            .filter(|f| f.get_kind() == CFKind::Amortization)
            .collect();
        assert_eq!(principal.len(), 20);
        for flow in principal {
            checks::close(flow.get_amount().amount(), 50_000.0, 1e-8);
        }
        checks::close(
            preflight
                .outstanding_by_date()
                .unwrap()
                .last()
                .expect("final amortization")
                .1
                .amount(),
            0.0,
            1e-6,
        );
        drop(preflight);
        b.iter(|| {
            black_box(fixtures::build_amortizing_linear(
                black_box(base),
                black_box(5),
            ))
        });
    });
    group.bench_function("fixed_plus_periodic_fee_5y_q", |b| {
        let base = fixtures::base_date();
        let preflight = fixtures::build_fixed_with_periodic_fee(base, 5);
        checks::rows(&preflight, 42);
        checks::fixed_amounts(&preflight);
        assert_eq!(preflight.coupons().count(), 20);
        let fees: Vec<_> = preflight
            .get_flows()
            .iter()
            .filter(|f| f.get_kind() == CFKind::Fee)
            .collect();
        assert_eq!(fees.len(), 20);
        for fee in fees {
            let accrual = fee.get_accrual().as_ref().expect("fee accrual metadata");
            assert_eq!(accrual.get_day_count(), DayCount::Act360);
            checks::close(
                fee.get_amount().amount(),
                1_000_000.0
                    * 0.0025
                    * (accrual.get_end() - accrual.get_start()).whole_days() as f64
                    / 360.0,
                1e-7,
            );
        }
        drop(preflight);
        b.iter(|| {
            black_box(fixtures::build_fixed_with_periodic_fee(
                black_box(base),
                black_box(5),
            ))
        });
    });
    group.finish();
}

fn bench_json_bridge(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_json_bridge");
    group.bench_function("build_5y_q", |b| {
        let spec = fixtures::five_year_fixed_json();
        let preflight = build_cashflow_schedule_json(spec, None, None).unwrap();
        let parsed: CashFlowSchedule = serde_json::from_str(&preflight).unwrap();
        checks::fixed_bullet(&parsed, fixtures::base_date(), 5, 20, true);
        drop(parsed);
        drop(preflight);
        b.iter(|| {
            black_box(
                build_cashflow_schedule_json(black_box(spec), None, None)
                    .expect("valid JSON schedule build"),
            )
        });
    });
    group.bench_function("validate_5y_q", |b| {
        let schedule_json =
            build_cashflow_schedule_json(fixtures::five_year_fixed_json(), None, None).unwrap();
        let canonical = validate_cashflow_schedule_json(&schedule_json).unwrap();
        assert_eq!(canonical, schedule_json);
        let parsed: CashFlowSchedule = serde_json::from_str(&canonical).unwrap();
        checks::fixed_bullet(&parsed, fixtures::base_date(), 5, 20, true);
        drop(parsed);
        drop(canonical);
        b.iter(|| {
            black_box(
                validate_cashflow_schedule_json(black_box(&schedule_json))
                    .expect("valid JSON schedule canonicalization"),
            )
        });
    });
    group.bench_function("dated_flows_5y_q", |b| {
        let schedule_json =
            build_cashflow_schedule_json(fixtures::five_year_fixed_json(), None, None).unwrap();
        let schedule: CashFlowSchedule = serde_json::from_str(&schedule_json).unwrap();
        checks::fixed_bullet(&schedule, fixtures::base_date(), 5, 20, true);
        let preflight = dated_flows_json(&schedule_json).unwrap();
        let parsed: Vec<DatedFlowJson> = serde_json::from_str(&preflight).unwrap();
        assert_eq!(parsed.len(), 22);
        for (row, flow) in parsed.iter().zip(schedule.get_flows()) {
            assert_eq!((row.date, row.amount), (flow.get_date(), flow.get_amount()));
            assert_eq!(row.amount.currency(), Currency::USD);
        }
        drop(parsed);
        drop(schedule);
        drop(preflight);
        b.iter(|| {
            black_box(
                dated_flows_json(black_box(&schedule_json)).expect("valid JSON cash extraction"),
            )
        });
    });
    group.finish();
}

fn bench_calendar_year_ladder(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_calendar_year_ladder");
    group.throughput(Throughput::Elements(42));
    group.bench_function("10y_q_42rows", |b| {
        let base = fixtures::base_date();
        let schedule = fixtures::make_fixed_schedule(base, 10, Tenor::quarterly());
        checks::fixed_bullet(&schedule, base, 10, 40, true);
        let dates: Vec<Date> = schedule
            .get_flows()
            .iter()
            .map(|cf| cf.get_date())
            .collect();
        let kind_strings: Vec<String> = schedule
            .get_flows()
            .iter()
            .map(|cf| cf.get_kind().to_string())
            .collect();
        let kinds: Vec<&str> = kind_strings.iter().map(String::as_str).collect();
        for (label, flow) in kinds.iter().zip(schedule.get_flows()) {
            assert_eq!(label.parse::<CFKind>().unwrap(), flow.get_kind());
        }
        let amounts: Vec<f64> = schedule
            .get_flows()
            .iter()
            .map(|cf| cf.get_amount().amount())
            .collect();
        let pvs: Vec<f64> = amounts.iter().map(|amount| amount * 0.85).collect();
        let preflight = calendar_year_ladder(&dates, &kinds, &amounts, &pvs)
            .expect("canonical ladder kind labels");
        assert_eq!(preflight.len(), 11);
        for (index, row) in preflight.iter().enumerate() {
            assert_eq!(row.year, 2025 + index as i32);
            let principal = schedule
                .get_flows()
                .iter()
                .filter(|flow| {
                    flow.get_date().year() == row.year && flow.get_kind() == CFKind::Notional
                })
                .map(|flow| flow.get_amount().amount())
                .sum::<f64>();
            let coupon = schedule
                .coupons()
                .filter(|flow| flow.get_date().year() == row.year)
                .map(|flow| flow.get_amount().amount())
                .sum::<f64>();
            checks::close(row.principal, principal, 1e-7);
            checks::close(row.non_principal, coupon, 1e-7);
            checks::close(row.pv, (principal + coupon) * 0.85, 1e-7);
        }
        checks::close(preflight.iter().map(|row| row.principal).sum(), 0.0, 0.0);
        let expected_coupons =
            1_000_000.0 * 0.06 * (fixtures::maturity_of(base, 10) - base).whole_days() as f64
                / 365.0;
        checks::close(
            preflight.iter().map(|row| row.non_principal).sum(),
            expected_coupons,
            1e-6,
        );
        drop(preflight);
        b.iter(|| {
            black_box(
                calendar_year_ladder(
                    black_box(&dates),
                    black_box(&kinds),
                    black_box(&amounts),
                    black_box(&pvs),
                )
                .expect("valid calendar-year ladder"),
            )
        });
    });
    group.finish();
}

fn bench_accrued_variants(c: &mut Criterion) {
    let mut group = c.benchmark_group("cashflow_accrued_variants");
    group.bench_function("compounded_5y", |b| {
        let base = fixtures::base_date();
        let schedule = fixtures::make_fixed_schedule(base, 5, Tenor::quarterly());
        checks::fixed_bullet(&schedule, base, 5, 20, true);
        let as_of = base + time::Duration::days(400);
        let cfg = AccrualConfig {
            method: AccrualMethod::Compounded,
            ..Default::default()
        };
        let expected: f64 = schedule
            .coupons()
            .map(|flow| {
                let a = flow.get_accrual().as_ref().expect("coupon accrual");
                if a.get_start() <= as_of && as_of < flow.get_date() {
                    let fraction = (as_of.min(a.get_end()) - a.get_start()).whole_days() as f64
                        / (a.get_end() - a.get_start()).whole_days() as f64;
                    1_000_000.0
                        * ((1.0 + flow.get_amount().amount() / 1_000_000.0).powf(fraction) - 1.0)
                } else {
                    0.0
                }
            })
            .sum();
        checks::close(
            accrued_interest_amount(&schedule, as_of, &cfg)
                .unwrap()
                .amount(),
            expected,
            1e-7,
        );
        b.iter(|| {
            black_box(
                accrued_interest_amount(black_box(&schedule), black_box(as_of), black_box(&cfg))
                    .expect("valid compounded accrual")
                    .amount(),
            )
        });
    });
    for (label, active) in [
        ("ex_coupon_rule_outside_window_usny_5y", false),
        ("ex_coupon_active_negative_usny_5y", true),
    ] {
        group.bench_with_input(BenchmarkId::from_parameter(label), &active, |b, &active| {
            let base = fixtures::base_date();
            let schedule = fixtures::make_fixed_schedule(base, 5, Tenor::quarterly());
            checks::fixed_bullet(&schedule, base, 5, 20, true);
            let payment = Date::from_calendar_date(2026, time::Month::April, 15).unwrap();
            let as_of = if active {
                payment - time::Duration::days(1)
            } else {
                base + time::Duration::days(400)
            };
            let rule = ExCouponRule {
                days_before_coupon: 7,
                calendar_id: Some("usny".into()),
            };
            assert_eq!(rule.is_ex_coupon(payment, as_of).unwrap(), active);
            let expected = if active {
                -1_000_000.0 * 0.06 / 365.0
            } else {
                checks::linear_accrued(&schedule, as_of)
            };
            let cfg = AccrualConfig {
                ex_coupon: Some(rule),
                ..Default::default()
            };
            let preflight = accrued_interest_amount(&schedule, as_of, &cfg)
                .unwrap()
                .amount();
            checks::close(preflight, expected, 1e-7);
            assert_eq!(preflight < 0.0, active);
            b.iter(|| {
                black_box(
                    accrued_interest_amount(
                        black_box(&schedule),
                        black_box(as_of),
                        black_box(&cfg),
                    )
                    .expect("valid ex-coupon accrual")
                    .amount(),
                )
            });
        });
    }
    group.finish();
}

fn bench_history_builds(c: &mut Criterion) {
    for leg in [
        history_workloads::Leg::Term,
        history_workloads::Leg::Overnight,
    ] {
        let mut group = c.benchmark_group(leg.group());
        group.throughput(Throughput::Elements(history_workloads::COUPONS as u64));
        for case in history_workloads::cases(leg) {
            group.bench_with_input(
                BenchmarkId::from_parameter(case.suffix()),
                &case,
                |b, &case| {
                    let fixture =
                        history_workloads::fixture(case, history_workloads::mixed_cutoff());
                    let output = fixture
                        .builder
                        .build_as_of(&fixture.market, fixture.as_of)
                        .expect("valid mixed-history floating build");
                    drop(history_workloads::check(&fixture, &output));
                    drop(output);
                    let history_workloads::Fixture {
                        builder,
                        market,
                        as_of,
                        required_history,
                        history,
                        ..
                    } = fixture;
                    drop(required_history);
                    drop(history);
                    b.iter(|| {
                        black_box(
                            black_box(&builder)
                                .build_as_of(black_box(&market), black_box(as_of))
                                .expect("valid mixed-history floating build"),
                        )
                    });
                },
            );
        }
        group.finish();
    }
}

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
    bench_history_builds,
);
criterion_main!(benches);
