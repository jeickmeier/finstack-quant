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

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::tree::TreePricer;
use finstack_quant_valuations::instruments::fixed_income::bond::{Bond, CallPut, CallPutSchedule};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::InstrumentPricingOverrides;
use finstack_quant_valuations::metrics::MetricId;
use std::hint::black_box;
use time::Month;

fn create_test_bond(maturity_years: i32) -> Bond {
    let issue = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2025 + maturity_years, Month::January, 1).unwrap();

    Bond::fixed(
        format!("BOND-{}Y", maturity_years),
        Money::new(1_000_000.0, Currency::USD),
        0.05, // 5% coupon
        issue,
        maturity,
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

    MarketContext::new().insert(curve).insert(forward_curve)
}

fn bench_bond_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_pv");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [2, 5, 10, 30].iter() {
        let bond = create_test_bond(*tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| bond.value(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

fn bench_bond_ytm(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_ytm_solve");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [2, 5, 10, 30].iter() {
        let mut bond = create_test_bond(*tenor);
        // Set quoted price to require YTM solving
        bond.instrument_pricing_overrides =
            InstrumentPricingOverrides::default().with_quoted_clean_price(95.0);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| {
                    bond.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&[MetricId::Ytm]),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                });
            },
        );
    }
    group.finish();
}

fn bench_bond_duration(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_duration");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [2, 5, 10, 30].iter() {
        let bond = create_test_bond(*tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| {
                    bond.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&[MetricId::DurationMod, MetricId::Convexity]),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                });
            },
        );
    }
    group.finish();
}

fn bench_bond_dv01(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_dv01");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [2, 5, 10, 30].iter() {
        let bond = create_test_bond(*tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| {
                    bond.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&[MetricId::Dv01]),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                });
            },
        );
    }
    group.finish();
}

fn bench_bond_yield_dv01(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_yield_dv01");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [2, 5, 10, 30].iter() {
        let mut bond = create_test_bond(*tenor);
        bond.instrument_pricing_overrides =
            InstrumentPricingOverrides::default().with_quoted_clean_price(99.25);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                b.iter(|| {
                    bond.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&[MetricId::YieldDv01]),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                });
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
                start_date: first_call,
                end_date: first_call,
                price_pct_of_par: 101.0,
                make_whole: None,
            });
        }

        if first_call_year + 1 < maturity_year {
            let second_call =
                Date::from_calendar_date(first_call_year + 1, Month::January, 1).unwrap();
            schedule.calls.push(CallPut {
                start_date: second_call,
                end_date: second_call,
                price_pct_of_par: 100.5,
                make_whole: None,
            });
        }

        if schedule.has_options() {
            bond.call_put = Some(schedule);
        }
    }

    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(99.0);
    bond
}

fn create_floating_note(maturity_years: i32) -> Bond {
    let issue = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2025 + maturity_years, Month::January, 1).unwrap();
    let mut bond = Bond::floating(
        format!("FRN-{}Y", maturity_years),
        Money::new(1_000_000.0, Currency::USD),
        "USD-SOFR-3M",
        150,
        issue,
        maturity,
        Tenor::quarterly(),
        DayCount::Act360,
        "USD-OIS",
    )
    .expect("Bond::floating should succeed with valid parameters");
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(100.25);
    bond
}

fn bench_callable_bond_tree_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_tree_pricing");
    let market = create_market();
    let market_ref = &market;
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [5, 10, 30] {
        let bond = create_callable_bond(tenor);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            &tenor,
            {
                move |b, _| {
                    b.iter(|| {
                        let pv = bond
                            .value(black_box(market_ref), black_box(as_of))
                            .unwrap_or_else(|e| panic!("tree pv failed: {e:?}"));
                        black_box(pv)
                    });
                }
            },
        );
    }
    group.finish();
}

fn bench_tree_oas_solver(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_tree_oas");
    let market = create_market();
    let market_ref = &market;
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let tree_pricer = TreePricer::new();
    let pricer_ref = &tree_pricer;

    for tenor in [5, 10, 30] {
        let bond = create_callable_bond(tenor);
        let clean = bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price
            .unwrap_or(99.0);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            &tenor,
            {
                move |b, _| {
                    b.iter(|| {
                        let oas = pricer_ref
                            .calculate_oas(
                                &bond,
                                black_box(market_ref),
                                black_box(as_of),
                                black_box(clean),
                            )
                            .unwrap_or_else(|e| panic!("tree oas failed: {e:?}"));
                        black_box(oas)
                    });
                }
            },
        );
    }
    group.finish();
}

/// Benchmark tree pricing at different step counts to measure backward induction performance.
///
/// This benchmark is useful for measuring the impact of the Vec vs HashMap optimization
/// in `BondValuator`. Higher step counts magnify the difference since backward induction
/// visits more nodes.
fn bench_tree_step_scaling(c: &mut Criterion) {
    use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::tree::TreePricerConfig;

    let mut group = c.benchmark_group("tree_step_scaling");
    let market = create_market();
    let market_ref = &market;
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Use a 10Y callable bond for consistent comparison
    let bond = create_callable_bond(10);
    let clean = bond
        .instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price
        .unwrap_or(99.0);

    {
        let steps = 100;
        let config = TreePricerConfig {
            tree_steps: steps,
            volatility: 0.01,
            tolerance: 1e-6,
            max_iterations: 50,
            initial_bracket_size_bp: Some(1000.0),
            mean_reversion: None,
            tree_model: Default::default(),
            tree_discount_curve_id: None,
            oas_quote_compounding: Default::default(),
            oas_price_basis: Default::default(),
            tree_compounding: Default::default(),
        };
        let pricer = TreePricer::with_config(config);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}steps", steps)),
            &steps,
            |b, _| {
                b.iter(|| {
                    let oas = pricer
                        .calculate_oas(
                            &bond,
                            black_box(market_ref),
                            black_box(as_of),
                            black_box(clean),
                        )
                        .unwrap_or_else(|e| panic!("tree oas failed: {e:?}"));
                    black_box(oas)
                });
            },
        );
    }
    group.finish();
}

fn bench_spread_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_spread_metrics");
    let market = create_market();
    let market_ref = &market;
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let base_fixed = {
        let mut bond = create_test_bond(10);
        bond.instrument_pricing_overrides =
            InstrumentPricingOverrides::default().with_quoted_clean_price(99.25);
        bond
    };

    let spread_cases = [
        ("z_spread_10Y", [MetricId::ZSpread]),
        ("i_spread_10Y", [MetricId::ISpread]),
        ("asw_par_10Y", [MetricId::ASWPar]),
        ("asw_market_10Y", [MetricId::ASWMarket]),
    ];

    for (label, metrics) in spread_cases {
        let bond = base_fixed.clone();
        group.bench_function(label, {
            move |b| {
                let metrics_slice: &[MetricId] = &metrics;
                b.iter(|| {
                    black_box(
                        bond.price_with_metrics(
                            black_box(market_ref),
                            black_box(as_of),
                            black_box(metrics_slice),
                            finstack_quant_valuations::instruments::PricingOptions::default(),
                        )
                        .expect(label),
                    )
                });
            }
        });
    }

    let frn = create_floating_note(5);
    let dm_metrics = [MetricId::DiscountMargin];
    group.bench_function("discount_margin_5Y", move |b| {
        let metrics_slice: &[MetricId] = &dm_metrics;
        b.iter(|| {
            black_box(
                frn.price_with_metrics(
                    black_box(market_ref),
                    black_box(as_of),
                    black_box(metrics_slice),
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .expect("discount margin"),
            )
        });
    });

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
    bench_spread_metrics
);
criterion_main!(benches);
