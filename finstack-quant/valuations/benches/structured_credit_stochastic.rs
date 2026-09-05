//! Structured-credit stochastic pricing benchmarks.
//!
//! Measures the Monte Carlo stochastic engine against a deterministic
//! waterfall baseline:
//! - Deterministic waterfall NPV (`value`) — isolates non-MC overhead
//! - Stochastic MC NPV across path counts and antithetic on/off

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, DealType, PoolAsset, PricingMode, StructuredCredit, Tranche, TrancheCoupon,
    TrancheSeniority, TrancheStructure,
};
use finstack_quant_valuations::instruments::Instrument;
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
        TrancheCoupon::Fixed { rate: 0.05 },
        legal_maturity(),
    )
    .unwrap();
    TrancheStructure::new(vec![tranche]).unwrap()
}

fn create_deal(id: &str, balance: f64) -> StructuredCredit {
    StructuredCredit::new_abs(
        id,
        simple_pool(balance),
        single_tranche_structure(balance),
        closing_date(),
        legal_maturity(),
        "USD-OIS",
    )
    .with_payment_calendar("nyse")
}

fn create_market() -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(closing_date())
        .knots(vec![(0.0, 1.0), (5.0, 0.95)])
        .build()
        .expect("discount curve");
    MarketContext::new().insert(disc)
}

/// Deterministic waterfall baseline: everything except the stochastic engine
/// (validation, schedule build, calendar resolution, waterfall execution).
fn bench_waterfall_baseline(c: &mut Criterion) {
    let deal = create_deal("SC-BENCH-WATERFALL", 1_000_000.0);
    let market = create_market();
    c.bench_function("structured_credit_waterfall_baseline_npv", |b| {
        b.iter(|| deal.value(black_box(&market), black_box(closing_date())))
    });
}

fn bench_stochastic_mc(c: &mut Criterion) {
    let deal = create_deal("SC-BENCH-STOCHASTIC", 1_000_000.0);
    let market = create_market();
    let mut group = c.benchmark_group("structured_credit_stochastic_mc");

    for num_paths in [100usize, 1000] {
        for antithetic in [false, true] {
            let mode = PricingMode::MonteCarlo {
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
                    b.iter(|| {
                        deal.price_stochastic_with_mode(
                            black_box(&market),
                            black_box(closing_date()),
                            mode.clone(),
                        )
                    })
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, bench_waterfall_baseline, bench_stochastic_mc);
criterion_main!(benches);
