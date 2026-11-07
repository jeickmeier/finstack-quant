//! Structured-credit stochastic pricing benchmarks.
//!
//! Measures the Monte Carlo stochastic engine against a deterministic
//! waterfall baseline:
//! - Deterministic waterfall NPV (`value`) — isolates non-MC overhead
//! - Stochastic MC NPV across path counts and antithetic on/off

mod support;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, DealType, PoolAsset, StandardDealParams, StructuredCredit, Tranche, TrancheStructure,
};
use finstack_quant_valuations::results::structured_credit::{
    StochasticPricingResult, StructuredCreditPricingMode, TrancheSeniority,
};
use std::hint::black_box;
use time::Month;

fn closing_date() -> Date {
    Date::from_calendar_date(2024, Month::January, 1).unwrap()
}

fn legal_maturity() -> Date {
    Date::from_calendar_date(2030, Month::January, 1).unwrap()
}

fn simple_pool(balance: f64) -> AssetPool {
    let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "A1",
        Money::new(balance, Currency::USD).expect("valid money fixture"),
        0.06,
        Date::from_calendar_date(2029, Month::January, 1).unwrap(),
        DayCount::Thirty360,
    ));
    pool
}

fn single_tranche_structure(balance: f64) -> TrancheStructure {
    let tranche = Tranche::new(
        "SENIOR",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(balance, Currency::USD).expect("valid money fixture"),
        RateSpec::Fixed { rate: 0.05 },
        legal_maturity(),
    )
    .unwrap();
    TrancheStructure::new(vec![tranche]).unwrap()
}

fn create_deal(id: &str, balance: f64) -> StructuredCredit {
    StructuredCredit::new_standard(
        DealType::Abs,
        StandardDealParams {
            id: id.into(),
            pool: simple_pool(balance),
            tranches: single_tranche_structure(balance),
            closing_date: closing_date(),
            maturity: legal_maturity(),
            discount_curve_id: "USD-OIS".into(),
            calendar_id: Some("nyse".into()),
        },
    )
    .expect("valid structured-credit dates")
}

fn create_market() -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(closing_date())
        .knots(vec![(0.0, 1.0), (5.0, 0.95)])
        .build()
        .expect("discount curve");
    MarketContext::new().insert(disc)
}

fn validate_money(value: &Money) {
    assert_eq!(value.currency(), Currency::USD);
    assert!(value.amount().is_finite());
}

fn validate_stochastic(result: &StochasticPricingResult, mode: &StructuredCreditPricingMode) {
    assert_eq!(&result.pricing_mode, mode);
    let StructuredCreditPricingMode::MonteCarlo {
        num_paths,
        antithetic,
    } = mode;
    assert_eq!(
        result.num_paths,
        num_paths * if *antithetic { 2 } else { 1 }
    );
    for money in [
        result.npv,
        result.expected_loss,
        result.unexpected_loss,
        result.expected_shortfall,
        result.expected_collateral_draws,
        result.draw_option_cost,
    ] {
        validate_money(&money);
    }
    assert!(result.npv.amount() > 0.0);
    for value in [
        result.clean_price,
        result.dirty_price,
        result.es_confidence,
        result.pv_std_error,
        result.pv_confidence_interval.0,
        result.pv_confidence_interval.1,
        result.unfunded_draw_path_fraction,
    ] {
        assert!(value.is_finite());
    }
    assert!(result.pv_std_error >= 0.0);
    assert!(result.pv_confidence_interval.0 <= result.npv.amount());
    assert!(result.pv_confidence_interval.1 >= result.npv.amount());
    assert_eq!(result.tranche_results.len(), 1);
    let tranche = &result.tranche_results[0];
    assert_eq!(tranche.tranche_id, "SENIOR");
    assert_eq!(tranche.seniority, TrancheSeniority::Senior);
    assert_eq!(tranche.attach_pct, 0.0);
    assert_eq!(tranche.detach_pct, 100.0);
    assert!(tranche.paths_with_principal <= result.num_paths);
    for money in [
        tranche.npv,
        tranche.expected_loss,
        tranche.unexpected_loss,
        tranche.expected_shortfall,
        tranche.draw_option_cost,
    ] {
        validate_money(&money);
    }
    for value in [tranche.price_pct, tranche.wal, tranche.credit_duration] {
        assert!(value.is_finite());
    }
    assert!(result.draw_option_cost_paths.is_empty());
}

/// Deterministic waterfall baseline: everything except the stochastic engine
/// (validation, schedule build, calendar resolution, waterfall execution).
fn bench_waterfall_baseline(c: &mut Criterion) {
    c.bench_function("structured_credit_waterfall_baseline_npv", |b| {
        let deal = create_deal("SC-BENCH-WATERFALL", 1_000_000.0);
        let market = create_market();
        support::bench(
            b,
            || {
                deal.value(black_box(&market), black_box(closing_date()))
                    .expect("structured-credit deterministic waterfall NPV")
            },
            |value| {
                validate_money(value);
                assert!(value.amount() > 0.0);
            },
        );
    });
}

fn bench_stochastic_mc(c: &mut Criterion) {
    let mut group = c.benchmark_group("structured_credit_stochastic_mc");

    for num_paths in [100usize, 1000] {
        for antithetic in [false, true] {
            let mode = StructuredCreditPricingMode::MonteCarlo {
                num_paths,
                antithetic,
            };
            group.bench_with_input(
                BenchmarkId::new(
                    if antithetic {
                        format!("{num_paths}_paths_antithetic")
                    } else {
                        format!("{num_paths}_paths")
                    },
                    num_paths,
                ),
                &mode,
                |b, mode| {
                    let deal = create_deal("SC-BENCH-STOCHASTIC", 1_000_000.0);
                    let market = create_market();
                    support::bench(
                        b,
                        || {
                            deal.price_stochastic_with_mode(
                                black_box(&market),
                                black_box(closing_date()),
                                mode.clone(),
                            )
                            .expect("structured-credit stochastic NPV")
                        },
                        |result| validate_stochastic(result, mode),
                    );
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, bench_waterfall_baseline, bench_stochastic_mc);
criterion_main!(benches);
