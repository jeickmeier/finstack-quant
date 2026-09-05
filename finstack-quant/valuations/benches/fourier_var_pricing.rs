//! Benchmark for the valuation-owned Taylor-VaR hot path.

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::metrics::risk::{
    calculate_var, MarketHistory, MarketScenario, RiskFactorShift, RiskFactorType, VarConfig,
    VarMethod,
};
use std::hint::black_box;
use time::Month;

fn make_var_market() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (0.5, 0.985),
            (1.0, 0.970),
            (2.0, 0.940),
            (3.0, 0.910),
            (5.0, 0.850),
            (7.0, 0.800),
            (10.0, 0.730),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    MarketContext::new().insert(disc)
}

fn make_var_bond(base: Date) -> Bond {
    Bond::fixed(
        "BOND-10Y",
        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.045).expect("valid rate fixture"),
        base,
        base + time::Duration::days(365 * 10),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .unwrap()
}

fn make_history(as_of: Date, n: usize) -> MarketHistory {
    let scenarios = (0..n)
        .map(|index| {
            let bump = ((index as f64) - (n as f64) / 2.0) * 0.0002;
            MarketScenario::new(
                as_of,
                vec![RiskFactorShift {
                    factor: RiskFactorType::DiscountRate {
                        curve_id: CurveId::from("USD-OIS"),
                        tenor_years: 5.0,
                    },
                    shift: bump,
                }],
            )
        })
        .collect();
    MarketHistory::new(as_of, 252, scenarios)
}

fn bench_taylor_var(c: &mut Criterion) {
    let mut group = c.benchmark_group("taylor_var");
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let market = make_var_market();
    let bond = make_var_bond(base);
    let config = VarConfig::var_99()
        .with_method(VarMethod::TaylorApproximation)
        .with_reporting_currency(Currency::USD);

    for &n in &[250_usize, 1000] {
        let history = make_history(base, n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &history, |b, history| {
            b.iter(|| {
                black_box(
                    calculate_var(black_box(&[&bond]), &market, history, base, &config).unwrap(),
                )
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_taylor_var);
criterion_main!(benches);
