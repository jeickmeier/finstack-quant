//! End-to-end `DefaultLpOptimizer::optimize` on a bond book.
//!
//! Valuation plus metric discovery dominate; the LP itself is secondary.
//! The fixture is a uniform fixed-coupon book so the timed path is the
//! optimizer workflow rather than mixed-instrument pricing.

use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::optimization::{
    DefaultLpOptimizer, MetricExpr, Objective, PerPositionMetric, PortfolioOptimizationProblem,
};
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::PortfolioBuilder;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::InstrumentPricingOverrides;
use finstack_quant_valuations::metrics::MetricId;
use time::Month;

fn opt_market(as_of: Date) -> MarketContext {
    let curve = DiscountCurve::builder("USD")
        .base_date(as_of)
        .knots(vec![(0.0, 1.0), (1.0, 0.99), (3.0, 0.96), (5.0, 0.93)])
        .interp(InterpStyle::Linear)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .build()
        .expect("bench: discount curve");
    MarketContext::new().insert(curve)
}

fn bond_book(n_positions: usize, as_of: Date) -> finstack_quant_portfolio::Portfolio {
    let issue = as_of;
    let maturity =
        Date::from_calendar_date(as_of.year() + 5, Month::January, 1).expect("valid maturity");
    let mut builder = PortfolioBuilder::new("OPT_BENCH")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("FUND_A"));

    for index in 0..n_positions {
        let coupon = 0.03 + (index % 6) as f64 * 0.005;
        let mut bond = Bond::fixed(
            format!("BOND_{index:03}"),
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            finstack_quant_core::types::Rate::from_decimal(coupon).expect("valid rate fixture"),
            issue,
            maturity,
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD",
        )
        .expect("bench: bond");
        bond.instrument_pricing_overrides =
            InstrumentPricingOverrides::default().with_quoted_clean_price(100.0);
        let rating = match index % 3 {
            0 => "AAA",
            1 => "BBB",
            _ => "CCC",
        };
        let position = Position::new(
            format!("POS_{index:03}"),
            "FUND_A",
            format!("BOND_{index:03}"),
            Arc::new(bond),
            1.0,
            PositionUnit::FaceValue,
        )
        .expect("bench: position")
        .with_text_attribute("rating", rating);
        builder = builder.position(position);
    }
    builder.build().expect("bench: opt portfolio")
}

fn bench_optimize(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_optimization");
    group.sample_size(10);
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("as_of");
    let market = opt_market(as_of);
    let config = FinstackConfig::default();
    let optimizer = DefaultLpOptimizer;
    let objective = Objective::Maximize(MetricExpr::ValueWeightedAverage {
        metric: PerPositionMetric::Metric(MetricId::Ytm),
        filter: None,
    });

    for &n_positions in &[32_usize, 64] {
        let portfolio = bond_book(n_positions, as_of);
        let problem = PortfolioOptimizationProblem::new(portfolio, objective.clone());
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{n_positions}bonds")),
            &n_positions,
            |b, _| {
                b.iter(|| {
                    optimizer
                        .optimize(
                            std::hint::black_box(&problem),
                            std::hint::black_box(&market),
                            std::hint::black_box(&config),
                        )
                        .expect("bench: optimize")
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_optimize);
criterion_main!(benches);
