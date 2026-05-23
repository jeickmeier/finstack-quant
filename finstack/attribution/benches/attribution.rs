//! P&L attribution benchmarks.

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, Criterion};
use finstack_attribution::{
    attribute_pnl_parallel, attribute_pnl_waterfall, default_waterfall_order,
};
use finstack_core::config::FinstackConfig;
use finstack_core::currency::Currency;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::bond::Bond;
use finstack_valuations::instruments::Instrument;
use std::hint::black_box;
use std::sync::Arc;
use time::Date;
use time::Month;

#[allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]
mod test_utils {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../valuations/tests/support/test_utils.rs"
    ));
}

/// Flat USD-OIS-style discount curve; `rate` is a continuously compounded zero approx.
fn build_flat_curve(curve_id: &str, as_of: Date, rate: f64) -> DiscountCurve {
    let tenors = [0.0, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 20.0, 30.0];
    let knots: Vec<(f64, f64)> = tenors.iter().map(|&t| (t, (-rate * t).exp())).collect();

    DiscountCurve::builder(curve_id)
        .base_date(as_of)
        .knots(knots)
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

const BASE_RATE: f64 = 0.04;
const SHIFT_BP: f64 = 5.0;

fn markets_t0_t1(as_of_t0: Date, as_of_t1: Date) -> (MarketContext, MarketContext) {
    let curve_t0 = build_flat_curve("USD-OIS", as_of_t0, BASE_RATE);
    let curve_t1 = build_flat_curve("USD-OIS", as_of_t1, BASE_RATE + SHIFT_BP / 10_000.0);
    (
        MarketContext::new().insert(curve_t0),
        MarketContext::new().insert(curve_t1),
    )
}

fn sample_bond(id: &str, maturity_year_offset: i32) -> Bond {
    let issue = test_utils::date(2025, 1, 1);
    let maturity =
        Date::from_calendar_date(2025 + maturity_year_offset, Month::January, 1).unwrap();
    Bond::fixed(
        id,
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap()
}

fn bench_attribution_parallel_1_bond(c: &mut Criterion) {
    let as_of_t0 = test_utils::date(2025, 1, 15);
    let as_of_t1 = test_utils::date(2025, 1, 16);
    let (market_t0, market_t1) = markets_t0_t1(as_of_t0, as_of_t1);
    let config = FinstackConfig::default();
    let bond: Arc<dyn Instrument> = Arc::new(sample_bond("BENCH-BOND-1", 5));

    c.bench_function("parallel_1_bond", |b| {
        b.iter(|| {
            let attr = attribute_pnl_parallel(
                &bond,
                black_box(&market_t0),
                black_box(&market_t1),
                black_box(as_of_t0),
                black_box(as_of_t1),
                &config,
                None,
            )
            .unwrap();
            black_box(attr);
        });
    });
}

fn bench_attribution_waterfall_1_bond(c: &mut Criterion) {
    let as_of_t0 = test_utils::date(2025, 1, 15);
    let as_of_t1 = test_utils::date(2025, 1, 16);
    let (market_t0, market_t1) = markets_t0_t1(as_of_t0, as_of_t1);
    let config = FinstackConfig::default();
    let bond: Arc<dyn Instrument> = Arc::new(sample_bond("BENCH-BOND-1", 5));
    let factor_order = default_waterfall_order();

    c.bench_function("waterfall_1_bond", |b| {
        b.iter(|| {
            let attr = attribute_pnl_waterfall(
                &bond,
                black_box(&market_t0),
                black_box(&market_t1),
                black_box(as_of_t0),
                black_box(as_of_t1),
                &config,
                factor_order.clone(),
                false,
                None,
            )
            .unwrap();
            black_box(attr);
        });
    });
}

fn bench_attribution_parallel_5_bonds(c: &mut Criterion) {
    let as_of_t0 = test_utils::date(2025, 1, 15);
    let as_of_t1 = test_utils::date(2025, 1, 16);
    let (market_t0, market_t1) = markets_t0_t1(as_of_t0, as_of_t1);
    let config = FinstackConfig::default();
    let bonds: Vec<Arc<dyn Instrument>> = (0..5)
        .map(|i| {
            let id = format!("BENCH-BOND-{i}");
            Arc::new(sample_bond(id.as_str(), 3 + i * 2)) as Arc<dyn Instrument>
        })
        .collect();

    c.bench_function("parallel_5_bonds", |b| {
        b.iter(|| {
            for bond in &bonds {
                let attr = attribute_pnl_parallel(
                    bond,
                    black_box(&market_t0),
                    black_box(&market_t1),
                    black_box(as_of_t0),
                    black_box(as_of_t1),
                    &config,
                    None,
                )
                .unwrap();
                black_box(attr);
            }
        });
    });
}

criterion_group!(
    attribution_parallel_1_bond,
    bench_attribution_parallel_1_bond
);
criterion_group!(
    attribution_waterfall_1_bond,
    bench_attribution_waterfall_1_bond
);
criterion_group!(
    attribution_parallel_5_bonds,
    bench_attribution_parallel_5_bonds
);
criterion_main!(
    attribution_parallel_1_bond,
    attribution_waterfall_1_bond,
    attribution_parallel_5_bonds
);
