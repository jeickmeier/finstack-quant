//! Bond pricing benchmarks.
//!
//! Measures performance of critical bond pricing operations:
//! - YTM solver convergence
//! - Duration and convexity calculations
//! - DV01 calculation
//! - Yield-basis DV01 calculation
//! - Clean/dirty price computation
//!
//! Market Standards Review (Week 5)

mod support;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_cashflows::builder::CouponType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve, HazardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fixed_income::bond::{
    Bond, CallPut, CallPutSchedule, CashflowSpec,
};
use finstack_quant_valuations::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations::metrics::MetricId;
use std::hint::black_box;
use time::Month;

fn check_money(value: &Money, currency: Currency) {
    assert_eq!(value.currency(), currency);
    assert!(value.amount().is_finite());
}

fn check_valuation(
    result: &finstack_quant_valuations::results::ValuationResult,
    currency: Currency,
    metrics: &[MetricId],
) {
    check_money(&result.value, currency);
    assert!(result.measures.values().all(|value| value.is_finite()));
    for metric in metrics {
        let prefix = format!("{}::", metric.as_str());
        assert!(
            result.measures.contains_key(metric)
                || result
                    .measures
                    .keys()
                    .any(|key| key.as_str().starts_with(&prefix)),
            "missing requested metric {metric}"
        );
    }
}

fn create_test_bond(maturity_years: i32) -> Bond {
    let issue = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2025 + maturity_years, Month::January, 1).unwrap();

    Bond::fixed(
        format!("BOND-{}Y", maturity_years),
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"), // 5% coupon
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("Bond::fixed should succeed with valid parameters")
}

fn create_market() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (2.0, 0.96),
            (5.0, 0.88),
            (10.0, 0.70),
            (30.0, 0.40),
        ])
        .interp(InterpStyle::MonotoneConvex)
        .build()
        .unwrap();

    let forward_curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .knots([(0.0, 0.045), (1.0, 0.047), (3.0, 0.05), (5.0, 0.052)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let fixings = ScalarTimeSeries::new(
        "FIXING:USD-SOFR-3M",
        vec![(
            Date::from_calendar_date(2024, Month::December, 30).unwrap(),
            0.045,
        )],
        None,
    )
    .unwrap();

    MarketContext::new()
        .insert(curve)
        .insert(forward_curve)
        .insert_series(fixings)
}

fn create_hazard_market() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let hazard = HazardCurve::builder("USD-HAZARD")
        .base_date(base)
        .recovery_rate(0.40)
        .knots([(0.0, 0.02), (10.0, 0.02)])
        .build()
        .unwrap();
    create_market().insert(hazard)
}

fn bench_bond_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_pv");
    for tenor in [2, 5, 10, 30] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            &tenor,
            |b, tenor| {
                let market = create_market();
                let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
                let bond = create_test_bond(*tenor);
                support::bench(
                    b,
                    || {
                        bond.value(black_box(&market), black_box(as_of))
                            .expect("bond PV")
                    },
                    |value| check_money(value, Currency::USD),
                );
            },
        );
    }
    group.finish();
}

fn bench_bond_ytm(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_ytm_solve");
    for tenor in [2, 5, 10, 30] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            &tenor,
            |b, tenor| {
                let market = create_market();
                let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
                let mut bond = create_test_bond(*tenor);
                bond.instrument_pricing_overrides =
                    InstrumentPricingOverrides::default().with_quoted_clean_price_pct(95.0);
                let metrics = [MetricId::Ytm];
                support::bench(
                    b,
                    || {
                        bond.price_with_metrics(
                            black_box(&market),
                            black_box(as_of),
                            black_box(&metrics),
                            finstack_quant_valuations::instruments::PricingOptions::default(),
                        )
                        .expect("bond metrics")
                    },
                    |result| check_valuation(result, Currency::USD, &metrics),
                );
            },
        );
    }
    group.finish();
}

fn bench_bond_duration(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_duration");
    for tenor in [2, 5, 10, 30] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            &tenor,
            |b, tenor| {
                let market = create_market();
                let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
                let bond = create_test_bond(*tenor);
                let metrics = [MetricId::DurationMod, MetricId::Convexity];
                support::bench(
                    b,
                    || {
                        bond.price_with_metrics(
                            black_box(&market),
                            black_box(as_of),
                            black_box(&metrics),
                            finstack_quant_valuations::instruments::PricingOptions::default(),
                        )
                        .expect("bond metrics")
                    },
                    |result| check_valuation(result, Currency::USD, &metrics),
                );
            },
        );
    }
    group.finish();
}

fn bench_bond_dv01(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_dv01");
    for tenor in [2, 5, 10, 30] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            &tenor,
            |b, tenor| {
                let market = create_market();
                let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
                let bond = create_test_bond(*tenor);
                let metrics = [MetricId::Dv01];
                support::bench(
                    b,
                    || {
                        bond.price_with_metrics(
                            black_box(&market),
                            black_box(as_of),
                            black_box(&metrics),
                            finstack_quant_valuations::instruments::PricingOptions::default(),
                        )
                        .expect("bond metrics")
                    },
                    |result| check_valuation(result, Currency::USD, &metrics),
                );
            },
        );
    }
    group.finish();
}

fn bench_bond_yield_dv01(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_yield_dv01");
    for tenor in [2, 5, 10, 30] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            &tenor,
            |b, tenor| {
                let market = create_market();
                let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
                let mut bond = create_test_bond(*tenor);
                bond.instrument_pricing_overrides =
                    InstrumentPricingOverrides::default().with_quoted_clean_price_pct(99.25);
                let metrics = [MetricId::YieldDv01];
                support::bench(
                    b,
                    || {
                        bond.price_with_metrics(
                            black_box(&market),
                            black_box(as_of),
                            black_box(&metrics),
                            finstack_quant_valuations::instruments::PricingOptions::default(),
                        )
                        .expect("bond metrics")
                    },
                    |result| check_valuation(result, Currency::USD, &metrics),
                );
            },
        );
    }
    group.finish();
}

fn create_callable_bond(maturity_years: i32) -> Bond {
    let mut bond = create_test_bond(maturity_years);
    let issue_year = bond.issue_date.year();
    let maturity_year = bond.maturity.year();
    let total_years = (maturity_year - issue_year).max(0);

    if total_years > 1 {
        let call_offset = (total_years / 2).max(1);
        let first_call_year = (issue_year + call_offset).min(maturity_year - 1);
        let mut schedule = CallPutSchedule::default();

        if first_call_year < maturity_year {
            let first_call = Date::from_calendar_date(first_call_year, Month::January, 1).unwrap();
            schedule.calls.push(CallPut {
                start: first_call,
                end: first_call,
                price_pct_of_par: 101.0,
                make_whole: None,
            });
        }

        if first_call_year + 1 < maturity_year {
            let second_call =
                Date::from_calendar_date(first_call_year + 1, Month::January, 1).unwrap();
            schedule.calls.push(CallPut {
                start: second_call,
                end: second_call,
                price_pct_of_par: 100.5,
                make_whole: None,
            });
        }

        if schedule.has_options() {
            bond.call_put = Some(schedule);
        }
    }

    bond.instrument_pricing_overrides = InstrumentPricingOverrides::default()
        .with_quoted_clean_price_pct(99.0)
        .with_hw1f_sigma(0.01)
        .with_hw1f_mean_reversion(0.03);
    bond
}

fn create_floating_note(maturity_years: i32) -> Bond {
    let issue = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2025 + maturity_years, Month::January, 1).unwrap();
    let mut bond = Bond::floating(
        finstack_quant_valuations::instruments::fixed_income::bond::FloatingBondParams {
            id: format!("FRN-{}Y", maturity_years).into(),
            notional: Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: 150.into(),
            issue_date: issue,
            maturity,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            discount_curve_id: "USD-OIS".into(),
        },
    )
    .expect("Bond::floating should succeed with valid parameters");
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price_pct(100.25);
    bond
}

fn bench_callable_bond_tree_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_tree_pricing");
    for tenor in [5, 10, 30] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            &tenor,
            |b, tenor| {
                let market = create_market();
                let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
                let mut bond = create_callable_bond(*tenor);
                // Remove the solver's quote so value executes the model rollback.
                bond.instrument_pricing_overrides
                    .market_quotes
                    .quoted_clean_price_pct = None;
                assert!(
                    bond.call_put
                        .as_ref()
                        .is_some_and(CallPutSchedule::has_options)
                );
                assert!(bond.credit_curve_id.is_none());
                assert_eq!(
                    bond.instrument_pricing_overrides.model_config.hw1f_sigma,
                    Some(0.01)
                );
                support::bench(
                    b,
                    || {
                        bond.value(black_box(&market), black_box(as_of))
                            .expect("callable tree PV")
                    },
                    |value| check_money(value, Currency::USD),
                );
            },
        );
    }
    group.finish();
}

/// OAS through the production metric (prepared tree, Brent on OAS).
fn bench_tree_oas_solver(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_tree_oas");
    for tenor in [5, 10, 30] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            &tenor,
            |b, tenor| {
                let market = create_market();
                let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
                let bond = create_callable_bond(*tenor);
                assert!(
                    bond.call_put
                        .as_ref()
                        .is_some_and(CallPutSchedule::has_options)
                );
                let metrics = [MetricId::Oas];
                support::bench(
                    b,
                    || {
                        bond.price_with_metrics(
                            black_box(&market),
                            black_box(as_of),
                            &metrics,
                            finstack_quant_valuations::instruments::PricingOptions::default(),
                        )
                        .expect("tree OAS")
                    },
                    |result| check_valuation(result, Currency::USD, &metrics),
                );
            },
        );
    }
    group.finish();
}

/// OAS metric on a 10Y callable at an explicit 100-step Hull-White tree.
fn bench_tree_step_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_step_scaling");
    let steps = 100;
    group.bench_with_input(
        BenchmarkId::from_parameter(format!("{}steps", steps)),
        &steps,
        |b, steps| {
            let market = create_market();
            let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
            let mut bond = create_callable_bond(10);
            bond.instrument_pricing_overrides.model_config.tree_steps = Some(*steps);
            assert_eq!(
                bond.instrument_pricing_overrides.model_config.tree_steps,
                Some(100)
            );
            let metrics = [MetricId::Oas];
            support::bench(
                b,
                || {
                    bond.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        &metrics,
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                    .expect("100-step tree OAS")
                },
                |result| check_valuation(result, Currency::USD, &metrics),
            );
        },
    );
    group.finish();
}

fn bench_spread_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_spread_metrics");
    for (label, metric) in [
        ("z_spread_10Y", MetricId::ZSpread),
        ("i_spread_10Y", MetricId::ISpread),
        ("asw_par_10Y", MetricId::ASWPar),
        ("asw_market_10Y", MetricId::ASWMarket),
    ] {
        group.bench_function(label, |b| {
            let market = create_market();
            let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
            let mut bond = create_test_bond(10);
            bond.instrument_pricing_overrides =
                InstrumentPricingOverrides::default().with_quoted_clean_price_pct(99.25);
            let metrics = [metric.clone()];
            support::bench(
                b,
                || {
                    bond.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&metrics),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                    .expect(label)
                },
                |result| check_valuation(result, Currency::USD, &metrics),
            );
        });
    }
    group.bench_function("discount_margin_5Y", |b| {
        let market = create_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let frn = create_floating_note(5);
        let metrics = [MetricId::DiscountMargin];
        support::bench(
            b,
            || {
                frn.price_with_metrics(
                    black_box(&market),
                    black_box(as_of),
                    black_box(&metrics),
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .expect("discount margin")
            },
            |result| check_valuation(result, Currency::USD, &metrics),
        );
    });
    group.finish();
}

const BOND_LSMC_BENCH_PATHS: usize = 64;

fn create_stochastic_hazard_callable(
    maturity_years: i32,
    floating_pik: bool,
    paths: usize,
) -> Bond {
    let issue = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2025 + maturity_years, Month::January, 1).unwrap();
    let mut bond = if floating_pik {
        let mut bond = Bond::floating(
            finstack_quant_valuations::instruments::fixed_income::bond::FloatingBondParams {
                id: format!("FRN-PIK-CALL-{}Y", maturity_years).into(),
                notional: Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
                forward_curve_id: "USD-SOFR-3M".into(),
                spread_bp: 200.into(),
                issue_date: issue,
                maturity,
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                discount_curve_id: "USD-OIS".into(),
            },
        )
        .expect("floating PIK benchmark bond");
        if let CashflowSpec::Floating(spec) = &mut bond.cashflow_spec {
            spec.coupon_type = CouponType::Pik;
            spec.rate_spec.index_floor_bp = Some(rust_decimal::Decimal::ZERO);
        }
        bond
    } else {
        create_test_bond(maturity_years)
    };
    bond.credit_curve_id = Some(CurveId::new("USD-HAZARD"));
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
            end: Date::from_calendar_date(2024 + maturity_years, Month::January, 1).unwrap(),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: Vec::new(),
    });
    let model = &mut bond.instrument_pricing_overrides.model_config;
    model.hw1f_sigma = Some(0.01);
    model.hazard_sigma = Some(0.02);
    model.rate_credit_correlation = Some(0.25);
    model.mc_paths = Some(paths);
    model.mc_antithetic = Some(true);
    bond
}

/// Exercise the actual stochastic rates-credit LSMC path. Each case carries
/// an inclusive multi-year call window, so the engine builds daily exercise
/// opportunities. Routine runs use 64 paths to keep the full benchmark suite
/// bounded; the path count is included in every benchmark name.
fn bench_stochastic_hazard_callable_lsmc(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_stochastic_hazard_lsmc");
    let paths = BOND_LSMC_BENCH_PATHS;
    for tenor in [5, 10] {
        for (case, floating_pik) in [
            ("fixed_daily_call", false),
            ("floating_pik_daily_call", true),
        ] {
            let id = format!("{case}_{tenor}Y_{paths}paths");
            group.bench_function(id, |b| {
                let market = create_hazard_market();
                let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
                let bond = create_stochastic_hazard_callable(tenor, floating_pik, paths);
                assert!(bond.credit_curve_id.is_some());
                assert!(
                    bond.call_put
                        .as_ref()
                        .is_some_and(CallPutSchedule::has_options)
                );
                assert_eq!(
                    bond.instrument_pricing_overrides.model_config.mc_paths,
                    Some(paths)
                );
                assert_eq!(
                    bond.instrument_pricing_overrides.model_config.hazard_sigma,
                    Some(0.02)
                );
                support::bench(
                    b,
                    || {
                        bond.value(black_box(&market), black_box(as_of))
                            .expect("stochastic hazard bond")
                    },
                    |value| check_money(value, Currency::USD),
                );
            });
        }
    }
    group.finish();
}

/// A 10Y bond callable at par every day of a 5-year window (2030-2035),
/// priced on the Hull-White tree and through the production OAS metric.
fn create_call_window_bond() -> Bond {
    let mut bond = create_test_bond(10);
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: Date::from_calendar_date(2030, Month::January, 1).unwrap(),
            end: Date::from_calendar_date(2035, Month::January, 1).unwrap(),
            price_pct_of_par: 100.0,
            make_whole: None,
        }],
        puts: Vec::new(),
    });
    bond.instrument_pricing_overrides = InstrumentPricingOverrides::default()
        .with_quoted_clean_price_pct(99.0)
        .with_hw1f_sigma(0.01)
        .with_hw1f_mean_reversion(0.03);
    bond
}

fn bench_call_window(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_call_window");
    group.bench_function("tree_pv_5y_window", |b| {
        let market = create_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        let mut unquoted = create_call_window_bond();
        unquoted
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price_pct = None;
        support::bench(
            b,
            || {
                unquoted
                    .value(black_box(&market), black_box(as_of))
                    .expect("call-window tree PV")
            },
            |value| check_money(value, Currency::USD),
        );
    });
    for (label, metrics) in [
        (
            "workout_metrics_5y_window",
            vec![
                MetricId::Ytw,
                MetricId::ISpread,
                MetricId::ZSpread,
                MetricId::DurationMac,
                MetricId::DurationMod,
                MetricId::Convexity,
                MetricId::YieldDv01,
                MetricId::ASWMarket,
            ],
        ),
        ("oas_metric_5y_window", vec![MetricId::Oas]),
    ] {
        group.bench_function(label, |b| {
            let market = create_market();
            let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
            let quoted = create_call_window_bond();
            support::bench(
                b,
                || {
                    quoted
                        .price_with_metrics(
                            black_box(&market),
                            black_box(as_of),
                            &metrics,
                            finstack_quant_valuations::instruments::PricingOptions::default(),
                        )
                        .expect(label)
                },
                |result| check_valuation(result, Currency::USD, &metrics),
            );
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_bond_pv,
    bench_bond_ytm,
    bench_bond_duration,
    bench_bond_dv01,
    bench_bond_yield_dv01,
    bench_callable_bond_tree_pv,
    bench_tree_oas_solver,
    bench_tree_step_scaling,
    bench_spread_metrics,
    bench_stochastic_hazard_callable_lsmc,
    bench_call_window
);
criterion_main!(benches);
