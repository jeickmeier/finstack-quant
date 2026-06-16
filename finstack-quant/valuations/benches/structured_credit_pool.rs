//! Structured credit pool benchmarks for valuations.

use criterion::{criterion_group, criterion_main, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, DealType, PoolAsset, StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority,
    TrancheStructure,
};
use time::Month;

fn benchmark_pool_flows(c: &mut Criterion) {
    let maturity = Date::from_calendar_date(2050, Month::January, 1).unwrap();
    let as_of = Date::from_calendar_date(2023, Month::January, 1).unwrap();

    // Create a large pool with 10,000 assets
    let mut pool = AssetPool::new("LARGE_POOL", DealType::RMBS, Currency::USD);
    for i in 0..10_000 {
        pool.assets.push(PoolAsset::floating_rate_loan(
            format!("LOAN_{}", i),
            Money::new(100_000.0, Currency::USD),
            "SOFR-1M",
            200.0,
            maturity,
            DayCount::Act360,
        ));
    }

    let tranche = Tranche::new(
        "A1",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(1_000_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity,
    )
    .unwrap();
    let tranches = TrancheStructure::new(vec![tranche]).unwrap();
    let instrument =
        StructuredCredit::new_rmbs("BENCH_DEAL", pool, tranches, as_of, maturity, "USD-OIS")
            .with_payment_calendar("nyse");

    let context = MarketContext::new();

    let mut group = c.benchmark_group("structured_credit_pool");
    group.sample_size(10);

    group.bench_function("calculate_pool_flows_10k", |b| {
        b.iter(|| {
            // We can't easily call the private calculate_pool_flows directly,
            // but we can call run_simulation which wraps it.
            // However, run_simulation does a lot more.
            // Ideally we would expose the internal function for benchmarking or benchmark the public API.
            // Let's benchmark the public API `run_simulation` as it reflects end-to-end performance.
            let _ =
                finstack_quant_valuations::instruments::fixed_income::structured_credit::run_simulation(
                    &instrument,
                    &context,
                    as_of,
                );
        })
    });

    group.finish();
}

criterion_group!(benches, benchmark_pool_flows);
criterion_main!(benches);
