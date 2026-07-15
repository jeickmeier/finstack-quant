//! Portfolio optimization example using finstack-quant-portfolio.
//!
//! This example demonstrates how to:
//! - Build a simple USD bond portfolio with rating tags
//! - Construct an optimization problem that maximizes value‑weighted YTM
//! - Add a CCC exposure constraint
//! - Run the LP‑based optimizer and inspect the results
//!
//! Run with:
//! ```bash
//! cargo run -p finstack-quant-portfolio --example portfolio_optimization
//! ```

use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::metrics::aggregate_metrics;
use finstack_quant_portfolio::optimization::{
    Constraint, DefaultLpOptimizer, MetricExpr, MissingMetricPolicy, Objective, PerPositionMetric,
    PortfolioOptimizationProblem, WeightingScheme,
};
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::PortfolioBuilder;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::InstrumentPricingOverrides;
use finstack_quant_valuations::metrics::MetricId;
use std::sync::Arc;
use time::Month;

fn build_market(as_of: Date) -> MarketContext {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;

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
        .expect("example discount curve should build");

    MarketContext::new().insert(curve)
}

fn build_bond_portfolio(as_of: Date) -> finstack_quant_portfolio::Portfolio {
    let issue = as_of;
    let maturity =
        Date::from_calendar_date(as_of.year() + 5, Month::January, 1).expect("valid maturity date");

    // All bonds use the same discount curve "USD" so that YTM is well-defined.
    let mut bond_aaa = Bond::fixed(
        "BOND_AAA",
        Money::new(1_000_000.0, Currency::USD),
        0.03,
        issue,
        maturity,
        "USD",
    )
    .expect("Bond::fixed should succeed with valid parameters");

    let mut bond_bbb = Bond::fixed(
        "BOND_BBB",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD",
    )
    .expect("Bond::fixed should succeed with valid parameters");

    let mut bond_ccc = Bond::fixed(
        "BOND_CCC",
        Money::new(1_000_000.0, Currency::USD),
        0.08,
        issue,
        maturity,
        "USD",
    )
    .expect("Bond::fixed should succeed with valid parameters");

    // Use explicit quoted clean prices so YTM is driven by market levels, not model PVs.
    bond_aaa.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(100.0);
    bond_bbb.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(100.0);
    bond_ccc.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(100.0);

    let pos_aaa = Position::new(
        "POS_AAA",
        "FUND_A",
        "BOND_AAA",
        Arc::new(bond_aaa),
        1.0,
        PositionUnit::FaceValue,
    )
    .expect("AAA position should build")
    .with_text_attribute("rating", "AAA");

    let pos_bbb = Position::new(
        "POS_BBB",
        "FUND_A",
        "BOND_BBB",
        Arc::new(bond_bbb),
        1.0,
        PositionUnit::FaceValue,
    )
    .expect("BBB position should build")
    .with_text_attribute("rating", "BBB");

    let pos_ccc = Position::new(
        "POS_CCC",
        "FUND_A",
        "BOND_CCC",
        Arc::new(bond_ccc),
        1.0,
        PositionUnit::FaceValue,
    )
    .expect("CCC position should build")
    .with_text_attribute("rating", "CCC");

    PortfolioBuilder::new("BOND_FUND")
        .name("Credit Portfolio – Optimization Example")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("FUND_A").with_name("Example Fund"))
        .position(pos_aaa)
        .position(pos_bbb)
        .position(pos_ccc)
        .build()
        .expect("bond portfolio should build")
}

fn main() -> finstack_quant_portfolio::Result<()> {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid example date");
    let market = build_market(as_of);
    let config = FinstackConfig::default();

    let portfolio = build_bond_portfolio(as_of);

    // Validate portfolio structure before optimization.
    portfolio.validate()?;

    // Objective: maximize value‑weighted average yield (YTM).
    let objective = Objective::Maximize(MetricExpr::ValueWeightedAverage {
        metric: PerPositionMetric::Metric(MetricId::Ytm),
        filter: None,
    });

    let mut problem = PortfolioOptimizationProblem::new(portfolio, objective);
    problem.weighting = WeightingScheme::ValueWeight;
    problem.missing_metric_policy = MissingMetricPolicy::Zero;
    problem.label = Some("max_yield_with_ccc_limit".to_string());

    // Constraint: CCC exposure <= 20% of the portfolio.
    problem = problem.with_constraint(
        Constraint::exposure_limit("rating", "CCC", 0.20)
            .expect("valid constraint")
            .with_label("ccc_limit"),
    );

    let optimizer = DefaultLpOptimizer;
    let result = optimizer.optimize(&problem, &market, &config)?;

    println!("=== Portfolio Optimization Example ===");
    println!("Label: {:?}", result.problem.label);
    println!("Status: {:?}", result.status);
    println!("Objective: maximize value‑weighted YTM");
    println!("Objective value (YTM): {:.6}", result.objective_value);

    // Inspect optimal weights.
    println!("\nOptimal weights by position:");
    for (pos_id, weight) in &result.optimal_weights {
        println!("  {pos_id}: {weight:.4}");
    }

    // Compute CCC weight directly from result.
    let mut ccc_weight = 0.0_f64;
    let portfolio_ref = &result.problem.portfolio;
    for (pos_id, &w) in &result.optimal_weights {
        if let Some(position) = portfolio_ref.get_position(pos_id.as_str()) {
            if position.attributes.get("rating").and_then(|v| v.as_text()) == Some("CCC") {
                ccc_weight += w;
            }
        }
    }
    println!("\nCCC exposure (by weight): {:.2}%", ccc_weight * 100.0);

    // Show a simple trade list.
    println!("\nSuggested trades (delta weights):");
    for trade in result.to_trade_list().iter().take(10) {
        println!(
            "  {}: {:?}, Δw = {:+.4}",
            trade.position_id,
            trade.direction,
            trade.target_weight - trade.current_weight
        );
    }

    // Optionally, compute aggregated metrics at the optimal allocation by
    // revaluing the portfolio with implied quantities.
    let rebalanced = result.to_rebalanced_portfolio()?;
    let valuation = finstack_quant_portfolio::valuation::value_portfolio(
        &rebalanced,
        &market,
        &config,
        &Default::default(),
    )?;
    let metrics: finstack_quant_portfolio::metrics::PortfolioMetrics =
        aggregate_metrics(&valuation, rebalanced.base_ccy, &market, rebalanced.as_of)?;

    println!("\nAggregated metrics at optimum (first few):");
    for (metric_id, metric) in metrics.aggregated.iter().take(5) {
        println!("  {metric_id}: total = {:.6}", metric.total);
    }

    Ok(())
}
