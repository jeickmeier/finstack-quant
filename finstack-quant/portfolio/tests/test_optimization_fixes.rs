//! Test optimization fixes tests for portfolio.

use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{create_date, Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::builder::PortfolioBuilder;
use finstack_quant_portfolio::optimization::{
    CandidatePosition, DefaultLpOptimizer, MetricExpr, MissingMetricPolicy, Objective,
    PerPositionMetric, PortfolioOptimizationProblem, WeightingScheme,
};
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_valuations::instruments::{Attributes, Instrument};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::InstrumentType;
use finstack_quant_valuations::results::ValuationResult;
use indexmap::IndexMap;
use std::any::Any;
use std::sync::Arc;
use time::Month;

// Mock market context builder (simplified)
fn build_mock_market() -> finstack_quant_core::market_data::context::MarketContext {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    let as_of = create_date(2024, Month::January, 1).unwrap();
    // Build a flat 5% yield curve using knots
    // 5% continuously compounded rate roughly.
    // Discount factor at T=1 is exp(-0.05*1) = 0.9512
    let flat_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(vec![(0.0, 1.0), (10.0, 0.6065)]) // exp(-0.05 * 10) = 0.6065
        .build()
        .expect("Curve build failed");

    let mut market = MarketContext::new();
    market = market.insert(flat_curve);
    market
}

fn test_deposit(
    id: &str,
    notional: f64,
    as_of: Date,
) -> Result<Deposit, Box<dyn std::error::Error>> {
    Ok(Deposit::builder()
        .id(id.into())
        .notional(Money::new(notional, Currency::USD).expect("valid money fixture"))
        .start_date(as_of)
        .maturity(create_date(2024, Month::February, 1)?)
        .day_count(DayCount::Act365F)
        .discount_curve_id("USD-OIS".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()?)
}

fn build_multi_currency_market() -> finstack_quant_core::market_data::context::MarketContext {
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    struct StaticFx {
        rate: f64,
    }

    impl FxProvider for StaticFx {
        fn rate(
            &self,
            _from: Currency,
            _to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
            Ok(self.rate)
        }
    }

    let as_of = create_date(2024, Month::January, 1).unwrap();
    let usd_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(vec![(0.0, 1.0), (10.0, 0.6065)])
        .build()
        .expect("USD curve should build");
    let eur_curve = DiscountCurve::builder("EUR-OIS")
        .base_date(as_of)
        .knots(vec![(0.0, 1.0), (10.0, 0.6065)])
        .build()
        .expect("EUR curve should build");

    MarketContext::new()
        .insert(usd_curve)
        .insert(eur_curve)
        .insert_fx(FxMatrix::new(Arc::new(StaticFx { rate: 1.2 })))
}

#[derive(Clone)]
struct MetricInstrument {
    id: String,
    value: Money,
    measures: IndexMap<MetricId, f64>,
    attributes: Attributes,
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    MetricInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl MetricInstrument {
    fn new(id: &str, value: Money, measures: IndexMap<MetricId, f64>) -> Self {
        Self {
            id: id.to_string(),
            value,
            measures,
            attributes: Attributes::new(),
        }
    }
}

impl Instrument for MetricInstrument {
    /// Test mock: reads no market data.
    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
    {
        Ok(finstack_quant_valuations::instruments::MarketDependencies::new())
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> InstrumentType {
        InstrumentType::Basket
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut Attributes {
        &mut self.attributes
    }

    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }

    fn base_value(
        &self,
        _curves: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        Ok(self.value)
    }

    fn price_with_metrics(
        &self,
        _curves: &MarketContext,
        as_of: Date,
        _metrics: &[MetricId],
        _options: finstack_quant_valuations::instruments::PricingOptions,
    ) -> finstack_quant_valuations::Result<ValuationResult> {
        Ok(ValuationResult::stamped(self.id(), as_of, self.value)
            .with_measures(self.measures.clone()))
    }
}

#[test]
fn test_notional_weighting() -> Result<(), Box<dyn std::error::Error>> {
    let as_of = create_date(2024, Month::January, 1)?;

    // Deposit 1: Long 1M USD
    let dep1 = Deposit::builder()
        .id("DEP_LONG".into())
        .notional(Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"))
        .start_date(as_of)
        .maturity(create_date(2024, Month::February, 1)?)
        .day_count(DayCount::Act365F)
        .discount_curve_id("USD-OIS".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()?;

    let dep2 = Deposit::builder()
        .id("DEP_SHORT".into())
        .notional(Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"))
        .start_date(as_of)
        .maturity(create_date(2024, Month::February, 1)?)
        .day_count(DayCount::Act365F)
        .discount_curve_id("USD-OIS".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()?;

    let p1 = Position::new(
        "POS_LONG",
        "ENT_A",
        "DEP_LONG",
        Arc::new(dep1),
        1.0,
        PositionUnit::Notional(Some(Currency::USD)),
    )?;

    let p2 = Position::new(
        "POS_SHORT",
        "ENT_A",
        "DEP_SHORT",
        Arc::new(dep2),
        -1.0,
        PositionUnit::Notional(Some(Currency::USD)),
    )?;

    let portfolio = PortfolioBuilder::new("HEDGED_PORTFOLIO")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(p1)
        .position(p2)
        .build()?;

    // With NotionalWeight, Total Notional = 1M + |-1M| = 2M.
    // Weights should be 0.5 and -0.5.

    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::Constant(1.0),
            filter: None,
        }),
    );
    problem.weighting = WeightingScheme::NotionalWeight;

    let market = build_mock_market();
    let config = FinstackConfig::default();
    let optimizer = DefaultLpOptimizer;

    let result = optimizer.optimize(&problem, &market, &config)?;

    println!("Status: {:?}", result.status);
    println!("Current Weights: {:?}", result.current_weights);

    let w_long = result.current_weights.get("POS_LONG").unwrap();
    let w_short = result.current_weights.get("POS_SHORT").unwrap();

    assert!(w_long.is_finite());
    assert!(w_short.is_finite());
    // Expect approx 0.5 and -0.5
    assert!((w_long - 0.5).abs() < 1e-4);
    assert!((w_short + 0.5).abs() < 1e-4);

    Ok(())
}

#[test]
fn test_candidate_batching() -> Result<(), Box<dyn std::error::Error>> {
    let as_of = create_date(2024, Month::January, 1)?;

    let portfolio = PortfolioBuilder::new("EMPTY_PORTFOLIO")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .build()?;

    // The objective is incidental — this test exercises candidate batch
    // valuation. PvBase is rejected in aggregated expressions when
    // candidates are in scope (their pv_base = 0 would be silently
    // ignored), so use a candidate-safe constant metric.
    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::Constant(1.0),
            filter: None,
        }),
    );

    // Add 10 candidate deposits
    for i in 0..10 {
        let dep = Deposit::builder()
            .id(format!("CAND_DEP_{}", i).into())
            .notional(Money::new(100_000.0, Currency::USD).expect("valid money fixture"))
            .start_date(as_of)
            .maturity(create_date(2024, Month::February, 1)?)
            .day_count(DayCount::Act365F)
            .discount_curve_id("USD-OIS".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()?;

        let cand = CandidatePosition::new(
            format!("CAND_{}", i),
            "ENT_A",
            Arc::new(dep),
            PositionUnit::Units,
        )
        .with_max_weight(0.1);

        problem.trade_universe.candidates.push(cand);
    }

    let market = build_mock_market();
    let config = FinstackConfig::default();
    let optimizer = DefaultLpOptimizer;

    let result = optimizer.optimize(&problem, &market, &config)?;

    assert!(result.status.is_feasible());
    assert_eq!(result.optimal_weights.len(), 10);

    Ok(())
}

#[test]
fn test_missing_metric_exclude_freezes_position_at_current_weight() {
    let as_of = create_date(2024, Month::January, 1).unwrap();
    let mut rich_measures = IndexMap::new();
    rich_measures.insert(MetricId::Ytm, 0.08);

    let missing_metric = Position::new(
        "POS_MISSING",
        "ENT_A",
        "MISSING",
        Arc::new(MetricInstrument::new(
            "MISSING",
            Money::new(50.0, Currency::USD).expect("valid money fixture"),
            IndexMap::new(),
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();
    let rich_metric = Position::new(
        "POS_RICH",
        "ENT_A",
        "RICH",
        Arc::new(MetricInstrument::new(
            "RICH",
            Money::new(50.0, Currency::USD).expect("valid money fixture"),
            rich_measures,
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("PORTFOLIO")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(missing_metric)
        .position(rich_metric)
        .build()
        .unwrap();

    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::ValueWeightedAverage {
            metric: PerPositionMetric::Metric(MetricId::Ytm),
            filter: None,
        }),
    );
    problem.missing_metric_policy = MissingMetricPolicy::Exclude;

    let market = build_mock_market();
    let config = FinstackConfig::default();
    let optimizer = DefaultLpOptimizer;
    let result = optimizer
        .optimize(&problem, &market, &config)
        .expect("Exclude policy should freeze missing-metric positions");

    assert_eq!(result.current_weights.get("POS_MISSING"), Some(&0.5));
    assert_eq!(result.optimal_weights.get("POS_MISSING"), Some(&0.5));
    assert_eq!(result.optimal_weights.get("POS_RICH"), Some(&0.5));
}

#[test]
fn test_pv_native_objective_rejected_in_aggregated_expression() {
    let as_of = create_date(2024, Month::January, 1).unwrap();

    let usd_position = Position::new(
        "POS_USD",
        "ENT_A",
        "USD_INST",
        Arc::new(MetricInstrument::new(
            "USD_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            IndexMap::new(),
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();
    let eur_position = Position::new(
        "POS_EUR",
        "ENT_A",
        "EUR_INST",
        Arc::new(MetricInstrument::new(
            "EUR_INST",
            Money::new(100.0, Currency::EUR).expect("valid money fixture"),
            IndexMap::new(),
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("MULTI_CCY_PORTFOLIO")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(usd_position)
        .position(eur_position)
        .build()
        .unwrap();

    // PvNative is no longer silently substituted for PvBase: aggregated
    // objectives over multi-currency portfolios must be explicit about which
    // numeraire they sum in.
    let problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::PvNative,
            filter: None,
        }),
    );

    let market = build_multi_currency_market();
    let config = FinstackConfig::default();
    let optimizer = DefaultLpOptimizer;
    let err = optimizer
        .optimize(&problem, &market, &config)
        .expect_err("PvNative in WeightedSum must error");
    let msg = err.to_string();
    assert!(
        msg.contains("PvNative") && msg.contains("PvBase"),
        "error should mention PvNative and PvBase: {msg}"
    );
}

#[test]
fn test_short_candidates_can_take_negative_weights() {
    let portfolio = PortfolioBuilder::new("EMPTY_PORTFOLIO")
        .base_currency(Currency::USD)
        .as_of(create_date(2024, Month::January, 1).unwrap())
        .build()
        .unwrap();

    let short_candidate = CandidatePosition::new(
        "SHORT_CANDIDATE",
        "ENT_A",
        Arc::new(MetricInstrument::new(
            "SHORT_CANDIDATE",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            IndexMap::new(),
        )),
        PositionUnit::Units,
    )
    .with_max_weight(0.4);
    let long_candidate = CandidatePosition::new(
        "LONG_CANDIDATE",
        "ENT_A",
        Arc::new(MetricInstrument::new(
            "LONG_CANDIDATE",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            IndexMap::new(),
        )),
        PositionUnit::Units,
    )
    .with_min_weight(0.6)
    .with_max_weight(0.6);

    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Minimize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::Constant(1.0),
            filter: None,
        }),
    );
    problem.weighting = WeightingScheme::UnitScaling;
    problem.constraints =
        vec![finstack_quant_portfolio::optimization::Constraint::Budget { rhs: 0.2 }];
    problem = problem.with_trade_universe(
        finstack_quant_portfolio::optimization::TradeUniverse::default()
            .allow_shorting_candidates()
            .with_candidate(short_candidate)
            .with_candidate(long_candidate),
    );

    let market = build_mock_market();
    let config = FinstackConfig::default();
    let optimizer = DefaultLpOptimizer;
    let result = optimizer.optimize(&problem, &market, &config).unwrap();

    assert_eq!(result.optimal_weights.get("SHORT_CANDIDATE"), Some(&-0.4));
    assert_eq!(result.optimal_weights.get("LONG_CANDIDATE"), Some(&0.6));
    assert_eq!(
        result.implied_quantities.get("SHORT_CANDIDATE"),
        Some(&-0.4)
    );
    assert_eq!(result.implied_quantities.get("LONG_CANDIDATE"), Some(&0.6));

    let trades = result.to_trade_list();
    assert!(
        trades.iter().any(|trade| {
            trade.position_id == "SHORT_CANDIDATE"
                && trade.trade_type
                    == finstack_quant_portfolio::optimization::TradeType::NewPosition
        }),
        "MO-10: selected short candidates must be classified as new positions"
    );
    let rebalanced = result
        .to_rebalanced_portfolio()
        .expect("MO-7: selected candidates should materialize");
    assert_eq!(
        rebalanced
            .get_position("SHORT_CANDIDATE")
            .map(|position| position.quantity),
        Some(-0.4),
        "MO-7: rebalanced portfolio should include the short candidate"
    );
    assert_eq!(
        rebalanced
            .get_position("LONG_CANDIDATE")
            .map(|position| position.quantity),
        Some(0.6),
        "MO-7: rebalanced portfolio should include the long candidate"
    );
}

#[test]
fn m7_existing_short_accepts_negative_weight_bounds() -> Result<(), Box<dyn std::error::Error>> {
    let as_of = create_date(2024, Month::January, 1)?;
    let long_instrument = Arc::new(MetricInstrument::new(
        "LONG_INST",
        Money::new(100.0, Currency::USD).expect("valid money fixture"),
        IndexMap::new(),
    ));
    let short_instrument = Arc::new(MetricInstrument::new(
        "SHORT_INST",
        Money::new(100.0, Currency::USD).expect("valid money fixture"),
        IndexMap::new(),
    ));
    let long = Position::new(
        "POS_LONG",
        "ENT_A",
        "LONG_INST",
        long_instrument,
        1.0,
        PositionUnit::Units,
    )?;
    let short = Position::new(
        "POS_SHORT",
        "ENT_A",
        "SHORT_INST",
        short_instrument,
        -1.0,
        PositionUnit::Units,
    )?;
    let portfolio = PortfolioBuilder::new("SHORT_BOOK")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(long)
        .position(short)
        .build()?;
    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::Constant(0.0),
            filter: None,
        }),
    );
    problem.constraints = vec![
        finstack_quant_portfolio::optimization::Constraint::WeightBounds {
            filter: finstack_quant_portfolio::optimization::PositionFilter::ByPositionIds(vec![
                "POS_LONG".into(),
            ]),
            min: 0.75,
            max: 1.0,
            label: Some("M-7 fixed long bounds".to_string()),
        },
        finstack_quant_portfolio::optimization::Constraint::WeightBounds {
            filter: finstack_quant_portfolio::optimization::PositionFilter::ByPositionIds(vec![
                "POS_SHORT".into(),
            ]),
            min: -1.0,
            max: -0.25,
            label: Some("M-7 existing short bounds".to_string()),
        },
        finstack_quant_portfolio::optimization::Constraint::Budget { rhs: 0.5 },
    ];

    let optimizer = DefaultLpOptimizer;
    let result = optimizer.optimize(&problem, &MarketContext::new(), &FinstackConfig::default())?;

    assert!(
        result.status.is_feasible(),
        "M-7: existing short negative bounds should remain feasible, got {:?}",
        result.status
    );
    let short_weight = result
        .optimal_weights
        .get("POS_SHORT")
        .expect("short position optimized");
    assert!(*short_weight <= -0.25 && *short_weight >= -1.0);
    Ok(())
}

#[test]
fn m8_candidate_entity_filters_apply_to_metric_constraints(
) -> Result<(), Box<dyn std::error::Error>> {
    let as_of = create_date(2024, Month::January, 1)?;
    let portfolio = PortfolioBuilder::new("EMPTY")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .build()?;
    let non_target = CandidatePosition::new(
        "NON_TARGET_CAND",
        "OTHER",
        Arc::new(MetricInstrument::new(
            "NON_TARGET",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            IndexMap::new(),
        )),
        PositionUnit::Units,
    );
    let target = CandidatePosition::new(
        "TARGET_CAND",
        "TARGET",
        Arc::new(MetricInstrument::new(
            "TARGET",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            IndexMap::new(),
        )),
        PositionUnit::Units,
    );
    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::Constant(0.0),
            filter: None,
        }),
    )
    .with_trade_universe(
        finstack_quant_portfolio::optimization::TradeUniverse::default()
            .with_candidate(non_target)
            .with_candidate(target),
    );
    problem.constraints.push(
        finstack_quant_portfolio::optimization::Constraint::MetricBound {
            label: Some("M-8 target candidate exposure".to_string()),
            metric: MetricExpr::WeightedSum {
                metric: PerPositionMetric::Constant(1.0),
                filter: Some(
                    finstack_quant_portfolio::optimization::PositionFilter::ByEntityId(
                        "TARGET".into(),
                    ),
                ),
            },
            op: finstack_quant_portfolio::optimization::Inequality::Ge,
            rhs: 1.0,
        },
    );

    let optimizer = DefaultLpOptimizer;
    let result = optimizer.optimize(&problem, &MarketContext::new(), &FinstackConfig::default())?;

    assert!(
        result.status.is_feasible(),
        "M-8: candidate entity filter should make the target constraint feasible, got {:?}",
        result.status
    );
    assert_eq!(result.optimal_weights.get("TARGET_CAND"), Some(&1.0));
    Ok(())
}

#[test]
fn m9_turnover_slack_uses_actual_turnover() -> Result<(), Box<dyn std::error::Error>> {
    let as_of = create_date(2024, Month::January, 1)?;
    let mut low_measures = IndexMap::new();
    low_measures.insert(MetricId::Ytm, 0.0);
    let mut high_measures = IndexMap::new();
    high_measures.insert(MetricId::Ytm, 0.10);

    let low = Position::new(
        "POS_LOW",
        "ENT_A",
        "LOW_INST",
        Arc::new(MetricInstrument::new(
            "LOW_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            low_measures,
        )),
        1.0,
        PositionUnit::Units,
    )?;
    let high = Position::new(
        "POS_HIGH",
        "ENT_A",
        "HIGH_INST",
        Arc::new(MetricInstrument::new(
            "HIGH_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            high_measures,
        )),
        1.0,
        PositionUnit::Units,
    )?;
    let portfolio = PortfolioBuilder::new("TURNOVER_BOOK")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(low)
        .position(high)
        .build()?;
    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::Metric(MetricId::Ytm),
            filter: None,
        }),
    );
    problem.constraints = vec![
        finstack_quant_portfolio::optimization::Constraint::Budget { rhs: 1.0 },
        finstack_quant_portfolio::optimization::Constraint::MaxTurnover {
            label: Some("M-9 turnover".to_string()),
            max_turnover: 0.5,
        },
    ];

    let optimizer = DefaultLpOptimizer;
    let result = optimizer.optimize(&problem, &MarketContext::new(), &FinstackConfig::default())?;

    assert!(
        result.status.is_feasible(),
        "M-9: turnover-constrained optimization should remain feasible, got {:?}",
        result.status
    );
    assert!(
        (result.turnover() - 0.5).abs() < 1e-8,
        "M-9: expected solver to consume the turnover budget, got {}",
        result.turnover()
    );
    let slack = result
        .constraint_slacks
        .get("M-9 turnover")
        .expect("turnover slack is reported");
    assert!(
        slack.abs() < 1e-8,
        "M-9: turnover slack should be based on actual solved turnover, got {slack}"
    );
    Ok(())
}

#[test]
fn m9_duplicate_turnover_constraints_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let as_of = create_date(2024, Month::January, 1)?;
    let first = Position::new(
        "POS_A",
        "ENT_A",
        "A_INST",
        Arc::new(MetricInstrument::new(
            "A_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            IndexMap::new(),
        )),
        1.0,
        PositionUnit::Units,
    )?;
    let second = Position::new(
        "POS_B",
        "ENT_A",
        "B_INST",
        Arc::new(MetricInstrument::new(
            "B_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            IndexMap::new(),
        )),
        1.0,
        PositionUnit::Units,
    )?;
    let portfolio = PortfolioBuilder::new("DUPLICATE_TURNOVER_BOOK")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(first)
        .position(second)
        .build()?;
    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::Constant(0.0),
            filter: None,
        }),
    );
    problem.constraints = vec![
        finstack_quant_portfolio::optimization::Constraint::Budget { rhs: 1.0 },
        finstack_quant_portfolio::optimization::Constraint::MaxTurnover {
            label: Some("M-9 turnover A".to_string()),
            max_turnover: 0.4,
        },
        finstack_quant_portfolio::optimization::Constraint::MaxTurnover {
            label: Some("M-9 turnover B".to_string()),
            max_turnover: 0.2,
        },
    ];

    let optimizer = DefaultLpOptimizer;
    let err = optimizer
        .optimize(&problem, &MarketContext::new(), &FinstackConfig::default())
        .expect_err("M-9: duplicate turnover constraints must fail fast");
    assert!(
        err.to_string().contains("M-9") && err.to_string().contains("MaxTurnover"),
        "M-9: duplicate turnover error should identify the review fix, got {err}"
    );
    Ok(())
}

#[test]
fn mo6_filtered_value_weighted_average_metric_bound_uses_filtered_denominator(
) -> Result<(), Box<dyn std::error::Error>> {
    let as_of = create_date(2024, Month::January, 1)?;
    let mut low_measures = IndexMap::new();
    low_measures.insert(MetricId::Ytm, 0.0);
    let mut high_measures = IndexMap::new();
    high_measures.insert(MetricId::Ytm, 10.0);
    let low = Position::new(
        "POS_LOW",
        "ENT_A",
        "LOW_INST",
        Arc::new(MetricInstrument::new(
            "LOW_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            low_measures,
        )),
        1.0,
        PositionUnit::Units,
    )?;
    let high = Position::new(
        "POS_HIGH",
        "ENT_A",
        "HIGH_INST",
        Arc::new(MetricInstrument::new(
            "HIGH_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            high_measures,
        )),
        1.0,
        PositionUnit::Units,
    )?;
    let portfolio = PortfolioBuilder::new("FILTERED_AVG")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(low)
        .position(high)
        .build()?;
    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::Metric(MetricId::Ytm),
            filter: None,
        }),
    );
    problem.constraints = vec![
        finstack_quant_portfolio::optimization::Constraint::Budget { rhs: 1.0 },
        finstack_quant_portfolio::optimization::Constraint::MetricBound {
            label: Some("MO-6 high average cap".to_string()),
            metric: MetricExpr::ValueWeightedAverage {
                metric: PerPositionMetric::Metric(MetricId::Ytm),
                filter: Some(
                    finstack_quant_portfolio::optimization::PositionFilter::ByPositionIds(vec![
                        "POS_HIGH".into(),
                    ]),
                ),
            },
            op: finstack_quant_portfolio::optimization::Inequality::Le,
            rhs: 5.0,
        },
    ];

    let optimizer = DefaultLpOptimizer;
    let result = optimizer.optimize(&problem, &MarketContext::new(), &FinstackConfig::default())?;

    assert!(
        result.status.is_feasible(),
        "MO-6: filtered average problem should remain feasible, got {:?}",
        result.status
    );
    assert!(
        result
            .optimal_weights
            .get("POS_HIGH")
            .copied()
            .unwrap_or_default()
            .abs()
            < 1e-8,
        "MO-6: filtered average cap should force the high-metric bucket out"
    );
    Ok(())
}

#[test]
fn mo8_value_weight_existing_zero_pv_position_errors() -> Result<(), Box<dyn std::error::Error>> {
    let as_of = create_date(2024, Month::January, 1)?;
    let zero_pv = Position::new(
        "POS_ZERO",
        "ENT_A",
        "ZERO_INST",
        Arc::new(MetricInstrument::new(
            "ZERO_INST",
            Money::new(0.0, Currency::USD).expect("valid money fixture"),
            IndexMap::new(),
        )),
        1.0,
        PositionUnit::Units,
    )?;
    let portfolio = PortfolioBuilder::new("ZERO_PV")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(zero_pv)
        .build()?;
    let problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::Constant(0.0),
            filter: None,
        }),
    );

    let optimizer = DefaultLpOptimizer;
    let err = optimizer
        .optimize(&problem, &MarketContext::new(), &FinstackConfig::default())
        .expect_err("MO-8: zero-PV existing ValueWeight positions must fail fast");
    assert!(err.to_string().contains("MO-8"), "unexpected error: {err}");
    Ok(())
}

#[test]
fn notional_weight_rejects_missing_instrument_notional() -> Result<(), Box<dyn std::error::Error>> {
    let as_of = create_date(2024, Month::January, 1)?;
    let percentage = Position::new(
        "POS_PERCENT",
        "ENT_A",
        "PERCENT_INST",
        Arc::new(MetricInstrument::new(
            "PERCENT_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            IndexMap::new(),
        )),
        50.0,
        PositionUnit::Percentage,
    )?;
    let unit = Position::new(
        "POS_UNIT",
        "ENT_A",
        "UNIT_INST",
        Arc::new(MetricInstrument::new(
            "UNIT_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            IndexMap::new(),
        )),
        1.0,
        PositionUnit::Units,
    )?;
    let portfolio = PortfolioBuilder::new("NOTIONAL_SCALE")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(percentage)
        .position(unit)
        .build()?;
    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::Constant(0.0),
            filter: None,
        }),
    );
    problem.weighting = WeightingScheme::NotionalWeight;

    let err = DefaultLpOptimizer
        .optimize(&problem, &MarketContext::new(), &FinstackConfig::default())
        .expect_err("NotionalWeight must fail when instrument.notional() is None");
    assert!(
        err.to_string().contains("NotionalWeight"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[test]
fn test_notional_weighting_implied_quantities_use_notional_denominator() {
    let as_of = create_date(2024, Month::January, 1).unwrap();

    let pos1 = Position::new(
        "POS_1",
        "ENT_A",
        "DEP_1",
        Arc::new(test_deposit("DEP_1", 1_000_000.0, as_of).unwrap()),
        1.0,
        PositionUnit::Notional(Some(Currency::USD)),
    )
    .unwrap();
    let pos2 = Position::new(
        "POS_2",
        "ENT_A",
        "DEP_2",
        Arc::new(test_deposit("DEP_2", 1_000_000.0, as_of).unwrap()),
        3.0,
        PositionUnit::Notional(Some(Currency::USD)),
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("NOTIONAL_PORTFOLIO")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(pos1)
        .position(pos2)
        .build()
        .unwrap();

    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::Constant(1.0),
            filter: None,
        }),
    );
    problem.weighting = WeightingScheme::NotionalWeight;
    problem = problem
        .with_constraint(
            finstack_quant_portfolio::optimization::Constraint::WeightBounds {
                label: Some("pin_pos_1".to_string()),
                filter: finstack_quant_portfolio::optimization::PositionFilter::ByPositionIds(
                    vec!["POS_1".into()],
                ),
                min: 0.25,
                max: 0.25,
            },
        )
        .with_constraint(
            finstack_quant_portfolio::optimization::Constraint::WeightBounds {
                label: Some("pin_pos_2".to_string()),
                filter: finstack_quant_portfolio::optimization::PositionFilter::ByPositionIds(
                    vec!["POS_2".into()],
                ),
                min: 0.75,
                max: 0.75,
            },
        );

    let market = build_mock_market();
    let config = FinstackConfig::default();
    let optimizer = DefaultLpOptimizer;
    let result = optimizer.optimize(&problem, &market, &config).unwrap();

    assert_eq!(result.implied_quantities.get("POS_1"), Some(&1.0));
    assert_eq!(result.implied_quantities.get("POS_2"), Some(&3.0));
}

/// MO-19: an unfiltered `ValueWeightedAverage` objective is lowered to the
/// plain linear form `Σ wᵢ·mᵢ`, which equals the true average only when the
/// weights sum to 1. With `Budget { rhs: -1.0 }` over two short positions the
/// reported objective is sign-flipped by `Σw = -1`, so the solver maximizes
/// the *lowest*-yield allocation. The optimizer must reject the combination
/// instead of returning Optimal with the wrong weights.
#[test]
fn mo19_unfiltered_vwa_objective_with_non_unit_budget_is_rejected() {
    let as_of = create_date(2024, Month::January, 1).unwrap();
    let mut high_measures = IndexMap::new();
    high_measures.insert(MetricId::Ytm, 0.10);
    let mut low_measures = IndexMap::new();
    low_measures.insert(MetricId::Ytm, 0.01);

    let high = Position::new(
        "POS_HI",
        "ENT_A",
        "HI_INST",
        Arc::new(MetricInstrument::new(
            "HI_INST",
            Money::new(-100.0, Currency::USD).expect("valid money fixture"),
            high_measures,
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();
    let low = Position::new(
        "POS_LO",
        "ENT_A",
        "LO_INST",
        Arc::new(MetricInstrument::new(
            "LO_INST",
            Money::new(-100.0, Currency::USD).expect("valid money fixture"),
            low_measures,
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("NET_SHORT_BOOK")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(high)
        .position(low)
        .build()
        .unwrap();

    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::ValueWeightedAverage {
            metric: PerPositionMetric::Metric(MetricId::Ytm),
            filter: None,
        }),
    );
    problem.constraints =
        vec![finstack_quant_portfolio::optimization::Constraint::Budget { rhs: -1.0 }];

    let err = DefaultLpOptimizer
        .optimize(&problem, &MarketContext::new(), &FinstackConfig::default())
        .expect_err("MO-19: VWA objective with Σw = -1 budget must fail, not sign-flip");
    let msg = err.to_string();
    assert!(
        msg.contains("MO-19") && msg.contains("Budget"),
        "error should carry the MO-19 tag and mention the budget: {msg}"
    );
}

/// Companion to MO-19: with an explicit `Budget { rhs: 1.0 }` (Σw = 1) the
/// unfiltered average objective is exactly `Σ wᵢ·mᵢ`, so the problem must
/// still solve and pick the high-yield name.
#[test]
fn mo19_unfiltered_vwa_objective_with_unit_budget_picks_high_yield() {
    let as_of = create_date(2024, Month::January, 1).unwrap();
    let mut high_measures = IndexMap::new();
    high_measures.insert(MetricId::Ytm, 0.10);
    let mut low_measures = IndexMap::new();
    low_measures.insert(MetricId::Ytm, 0.01);

    let high = Position::new(
        "POS_HI",
        "ENT_A",
        "HI_INST",
        Arc::new(MetricInstrument::new(
            "HI_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            high_measures,
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();
    let low = Position::new(
        "POS_LO",
        "ENT_A",
        "LO_INST",
        Arc::new(MetricInstrument::new(
            "LO_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            low_measures,
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("LONG_BOOK")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(high)
        .position(low)
        .build()
        .unwrap();

    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::ValueWeightedAverage {
            metric: PerPositionMetric::Metric(MetricId::Ytm),
            filter: None,
        }),
    );
    problem.constraints =
        vec![finstack_quant_portfolio::optimization::Constraint::Budget { rhs: 1.0 }];

    let result = DefaultLpOptimizer
        .optimize(&problem, &MarketContext::new(), &FinstackConfig::default())
        .expect("MO-19: unit budget keeps the VWA objective well-posed");
    assert!(
        result.status.is_feasible(),
        "unit-budget VWA objective should solve, got {:?}",
        result.status
    );
    assert!(
        (result.optimal_weights.get("POS_HI").copied().unwrap_or(0.0) - 1.0).abs() < 1e-8,
        "maximizing the average yield should allocate to the high-yield name: {:?}",
        result.optimal_weights
    );
}

/// An infeasible result must not report `turnover() == 0.0` — that is
/// indistinguishable from "already optimal, nothing to trade". It reports
/// NaN, consistent with `objective_value = NaN` on failed solves.
#[test]
fn infeasible_result_turnover_is_nan() {
    let as_of = create_date(2024, Month::January, 1).unwrap();
    let position = Position::new(
        "POS_A",
        "ENT_A",
        "A_INST",
        Arc::new(MetricInstrument::new(
            "A_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            IndexMap::new(),
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();
    let portfolio = PortfolioBuilder::new("INFEASIBLE_BOOK")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(position)
        .build()
        .unwrap();

    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::Constant(1.0),
            filter: None,
        }),
    );
    // w >= 0.8 for the only position, but the budget demands Σw = 0.5.
    problem.constraints = vec![
        finstack_quant_portfolio::optimization::Constraint::WeightBounds {
            label: Some("pin high".to_string()),
            filter: finstack_quant_portfolio::optimization::PositionFilter::ByPositionIds(vec![
                "POS_A".into(),
            ]),
            min: 0.8,
            max: 1.0,
        },
        finstack_quant_portfolio::optimization::Constraint::Budget { rhs: 0.5 },
    ];

    let result = DefaultLpOptimizer
        .optimize(&problem, &MarketContext::new(), &FinstackConfig::default())
        .expect("infeasible problems return a status-carrying result, not Err");
    assert!(
        !result.status.is_feasible(),
        "conflicting bounds/budget should be infeasible, got {:?}",
        result.status
    );
    assert!(
        result.turnover().is_nan(),
        "turnover of an infeasible result must be NaN (no solution), got {}",
        result.turnover()
    );
    assert!(result.to_trade_list().is_empty());
    assert!(result.binding_constraints().is_empty());
}

/// `MissingMetricPolicy::Exclude` positions keep their current weight and are
/// excluded from constraint evaluation — they must not sit in a
/// `ValueWeightedAverage` bound denominator with metric 0, where each frozen
/// weight contributes `−w·rhs` and can declare a feasible average infeasible.
#[test]
fn exclude_policy_removes_missing_metric_positions_from_vwa_bound_denominator() {
    let as_of = create_date(2024, Month::January, 1).unwrap();
    let mut scored_measures = IndexMap::new();
    scored_measures.insert(MetricId::Ytm, 700.0);

    let scored = Position::new(
        "POS_SCORED",
        "ENT_A",
        "SCORED_INST",
        Arc::new(MetricInstrument::new(
            "SCORED_INST",
            Money::new(50.0, Currency::USD).expect("valid money fixture"),
            scored_measures,
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();
    let unscored = Position::new(
        "POS_UNSCORED",
        "ENT_A",
        "UNSCORED_INST",
        Arc::new(MetricInstrument::new(
            "UNSCORED_INST",
            Money::new(50.0, Currency::USD).expect("valid money fixture"),
            IndexMap::new(),
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("SCORED_BOOK")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(scored)
        .position(unscored)
        .build()
        .unwrap();

    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::Metric(MetricId::Ytm),
            filter: None,
        }),
    );
    problem.missing_metric_policy = MissingMetricPolicy::Exclude;
    problem.constraints.push(
        finstack_quant_portfolio::optimization::Constraint::MetricBound {
            label: Some("avg score floor".to_string()),
            metric: MetricExpr::ValueWeightedAverage {
                metric: PerPositionMetric::Metric(MetricId::Ytm),
                filter: None,
            },
            op: finstack_quant_portfolio::optimization::Inequality::Ge,
            rhs: 500.0,
        },
    );

    let result = DefaultLpOptimizer
        .optimize(&problem, &MarketContext::new(), &FinstackConfig::default())
        .expect("Exclude policy problem should solve");
    assert!(
        result.status.is_feasible(),
        "average of the scored basket is 700 >= 500 and must be feasible, got {:?}",
        result.status
    );
    // The unscored position is frozen at its current weight, not distorted
    // into the average.
    assert_eq!(result.optimal_weights.get("POS_UNSCORED"), Some(&0.5));
    assert_eq!(result.optimal_weights.get("POS_SCORED"), Some(&0.5));
}

/// `ValueWeightedAverage` bound slacks are reported in metric units
/// (`rhs − achieved average`), not in the internal weight×metric units of the
/// `Σ_F wᵢ(mᵢ − rhs)` linearization — with a filtered weight sum of 0.5 the
/// raw row slack is half the metric-unit headroom.
#[test]
fn vwa_bound_slack_is_reported_in_metric_units() {
    let as_of = create_date(2024, Month::January, 1).unwrap();
    let mut scored_measures = IndexMap::new();
    scored_measures.insert(MetricId::Ytm, 8.0);

    let scored = Position::new(
        "POS_SCORED",
        "ENT_A",
        "SCORED_INST",
        Arc::new(MetricInstrument::new(
            "SCORED_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            scored_measures,
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();
    let other = Position::new(
        "POS_OTHER",
        "ENT_A",
        "OTHER_INST",
        Arc::new(MetricInstrument::new(
            "OTHER_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            IndexMap::new(),
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("SLACK_BOOK")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(scored)
        .position(other)
        .build()
        .unwrap();

    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::Constant(1.0),
            filter: None,
        }),
    );
    problem.constraints = vec![
        finstack_quant_portfolio::optimization::Constraint::WeightBounds {
            label: Some("pin scored".to_string()),
            filter: finstack_quant_portfolio::optimization::PositionFilter::ByPositionIds(vec![
                "POS_SCORED".into(),
            ]),
            min: 0.5,
            max: 0.5,
        },
        finstack_quant_portfolio::optimization::Constraint::MetricBound {
            label: Some("vwa_slack".to_string()),
            metric: MetricExpr::ValueWeightedAverage {
                metric: PerPositionMetric::Metric(MetricId::Ytm),
                filter: Some(
                    finstack_quant_portfolio::optimization::PositionFilter::ByPositionIds(vec![
                        "POS_SCORED".into(),
                    ]),
                ),
            },
            op: finstack_quant_portfolio::optimization::Inequality::Le,
            rhs: 10.0,
        },
    ];

    let result = DefaultLpOptimizer
        .optimize(&problem, &MarketContext::new(), &FinstackConfig::default())
        .expect("VWA slack problem should solve");
    assert!(result.status.is_feasible(), "got {:?}", result.status);

    // Achieved filtered average = 8.0, bound rhs = 10.0 → headroom is 2.0
    // metric units regardless of the filtered weight sum (0.5 here).
    let slack = result
        .constraint_slacks
        .get("vwa_slack")
        .copied()
        .expect("VWA bound slack is reported");
    assert!(
        (slack - 2.0).abs() < 1e-8,
        "slack must be in metric units (rhs − achieved average = 2.0), got {slack}"
    );
}

/// Duplicate `Budget` constraints are ambiguous (two Σw = rhs equalities) and
/// must be rejected up-front, mirroring the M-9 duplicate-turnover rejection.
#[test]
fn mo21_duplicate_budget_constraints_are_rejected() {
    let as_of = create_date(2024, Month::January, 1).unwrap();
    let position = Position::new(
        "POS_A",
        "ENT_A",
        "A_INST",
        Arc::new(MetricInstrument::new(
            "A_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            IndexMap::new(),
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();
    let portfolio = PortfolioBuilder::new("DUPLICATE_BUDGET_BOOK")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(position)
        .build()
        .unwrap();

    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::Constant(1.0),
            filter: None,
        }),
    );
    problem.constraints = vec![
        finstack_quant_portfolio::optimization::Constraint::Budget { rhs: 0.6 },
        finstack_quant_portfolio::optimization::Constraint::Budget { rhs: 0.4 },
    ];

    let err = DefaultLpOptimizer
        .optimize(&problem, &MarketContext::new(), &FinstackConfig::default())
        .expect_err("MO-21: duplicate Budget constraints must fail fast");
    let msg = err.to_string();
    assert!(
        msg.contains("MO-21") && msg.contains("Budget"),
        "error should carry the MO-21 tag and mention Budget: {msg}"
    );
}

#[test]
fn mo9_unit_scaling_without_budget_does_not_synthesize_sum_multiplier_budget() {
    let as_of = create_date(2024, Month::January, 1).unwrap();
    let position = Position::new(
        "POS_1",
        "ENT_A",
        "INST_1",
        Arc::new(MetricInstrument::new(
            "INST_1",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            IndexMap::new(),
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("UNIT_SCALING_PORTFOLIO")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(position)
        .build()
        .unwrap();

    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::Constant(1.0),
            filter: None,
        }),
    );
    problem.weighting = WeightingScheme::UnitScaling;
    problem.constraints.clear();

    let err = DefaultLpOptimizer
        .optimize(&problem, &build_mock_market(), &FinstackConfig::default())
        .expect_err("MO-9: UnitScaling must not get a synthetic budget");
    assert!(err.to_string().contains("MO-9"), "unexpected error: {err}");
}

#[test]
fn notional_weight_uses_instrument_deal_notional() -> Result<(), Box<dyn std::error::Error>> {
    let as_of = create_date(2024, Month::January, 1)?;
    let small = Position::new(
        "POS_1M",
        "ENT_A",
        "DEP_1M",
        Arc::new(test_deposit("DEP_1M", 1_000_000.0, as_of)?),
        1.0,
        PositionUnit::Notional(Some(Currency::USD)),
    )?;
    let large = Position::new(
        "POS_2M",
        "ENT_A",
        "DEP_2M",
        Arc::new(test_deposit("DEP_2M", 2_000_000.0, as_of)?),
        1.0,
        PositionUnit::Notional(Some(Currency::USD)),
    )?;
    let portfolio = PortfolioBuilder::new("DEAL_NOTIONAL")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(small)
        .position(large)
        .build()?;
    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::Constant(0.0),
            filter: None,
        }),
    );
    problem.weighting = WeightingScheme::NotionalWeight;
    problem = problem
        .with_constraint(
            finstack_quant_portfolio::optimization::Constraint::WeightBounds {
                label: Some("pin_1m".to_string()),
                filter: finstack_quant_portfolio::optimization::PositionFilter::ByPositionIds(
                    vec!["POS_1M".into()],
                ),
                min: 1.0 / 3.0,
                max: 1.0 / 3.0,
            },
        )
        .with_constraint(
            finstack_quant_portfolio::optimization::Constraint::WeightBounds {
                label: Some("pin_2m".to_string()),
                filter: finstack_quant_portfolio::optimization::PositionFilter::ByPositionIds(
                    vec!["POS_2M".into()],
                ),
                min: 2.0 / 3.0,
                max: 2.0 / 3.0,
            },
        );

    let result =
        DefaultLpOptimizer.optimize(&problem, &build_mock_market(), &FinstackConfig::default())?;
    assert!(
        (result.current_weights["POS_1M"] - (1.0 / 3.0)).abs() < 1e-9,
        "1M deal should be one-third of 3M gross, got {}",
        result.current_weights["POS_1M"]
    );
    assert!(
        (result.current_weights["POS_2M"] - (2.0 / 3.0)).abs() < 1e-9,
        "2M deal should be two-thirds of 3M gross, got {}",
        result.current_weights["POS_2M"]
    );
    assert!(
        (result.implied_quantities["POS_1M"] - 1.0).abs() < 1e-9,
        "implied lots must stay 1, not dollar notional, got {}",
        result.implied_quantities["POS_1M"]
    );
    assert!(
        (result.implied_quantities["POS_2M"] - 1.0).abs() < 1e-9,
        "implied lots must stay 1, not dollar notional, got {}",
        result.implied_quantities["POS_2M"]
    );
    Ok(())
}

#[test]
fn value_weight_percentage_reconstructs_via_scale_factor() -> Result<(), Box<dyn std::error::Error>>
{
    let as_of = create_date(2024, Month::January, 1)?;
    let percentage = Position::new(
        "POS_PERCENT",
        "ENT_A",
        "PERCENT_INST",
        Arc::new(MetricInstrument::new(
            "PERCENT_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            IndexMap::new(),
        )),
        50.0,
        PositionUnit::Percentage,
    )?;
    let unit = Position::new(
        "POS_UNIT",
        "ENT_A",
        "UNIT_INST",
        Arc::new(MetricInstrument::new(
            "UNIT_INST",
            Money::new(50.0, Currency::USD).expect("valid money fixture"),
            IndexMap::new(),
        )),
        1.0,
        PositionUnit::Units,
    )?;
    let portfolio = PortfolioBuilder::new("PCT_SCALE")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(percentage)
        .position(unit)
        .build()?;
    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::Constant(1.0),
            filter: None,
        }),
    );
    problem = problem
        .with_constraint(
            finstack_quant_portfolio::optimization::Constraint::WeightBounds {
                label: Some("pin_pct".to_string()),
                filter: finstack_quant_portfolio::optimization::PositionFilter::ByPositionIds(
                    vec!["POS_PERCENT".into()],
                ),
                min: 0.25,
                max: 0.25,
            },
        )
        .with_constraint(
            finstack_quant_portfolio::optimization::Constraint::WeightBounds {
                label: Some("pin_unit".to_string()),
                filter: finstack_quant_portfolio::optimization::PositionFilter::ByPositionIds(
                    vec!["POS_UNIT".into()],
                ),
                min: 0.75,
                max: 0.75,
            },
        );

    let result =
        DefaultLpOptimizer.optimize(&problem, &MarketContext::new(), &FinstackConfig::default())?;
    assert!(
        (result.implied_quantities["POS_PERCENT"] - 25.0).abs() < 1e-9,
        "Percentage 50 at half the current PV share must reconstruct 25 points, not scale 0.25; got {}",
        result.implied_quantities["POS_PERCENT"]
    );
    assert!(
        (result.implied_quantities["POS_UNIT"] - 1.5).abs() < 1e-9,
        "Units position should reconstruct 1.5, got {}",
        result.implied_quantities["POS_UNIT"]
    );
    Ok(())
}

/// `MissingMetricPolicy::Exclude` freezes the missing name and drops it from
/// a `WeightedSum` coefficient vector the same way VWA already does.
#[test]
fn exclude_policy_skips_missing_metric_in_weighted_sum() {
    let as_of = create_date(2024, Month::January, 1).unwrap();
    let mut high_measures = IndexMap::new();
    high_measures.insert(MetricId::Ytm, 0.10);
    let mut low_measures = IndexMap::new();
    low_measures.insert(MetricId::Ytm, 0.01);

    let high = Position::new(
        "POS_HI",
        "ENT_A",
        "HI_INST",
        Arc::new(MetricInstrument::new(
            "HI_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            high_measures,
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();
    let low = Position::new(
        "POS_LO",
        "ENT_A",
        "LO_INST",
        Arc::new(MetricInstrument::new(
            "LO_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            low_measures,
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();
    let missing = Position::new(
        "POS_MISSING",
        "ENT_A",
        "MISSING_INST",
        Arc::new(MetricInstrument::new(
            "MISSING_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            IndexMap::new(),
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("EXCLUDE_WS")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENT_A"))
        .position(high)
        .position(low)
        .position(missing)
        .build()
        .unwrap();

    let mut problem = PortfolioOptimizationProblem::new(
        portfolio,
        Objective::Maximize(MetricExpr::WeightedSum {
            metric: PerPositionMetric::Metric(MetricId::Ytm),
            filter: None,
        }),
    );
    problem.missing_metric_policy = MissingMetricPolicy::Exclude;

    let result = DefaultLpOptimizer
        .optimize(&problem, &MarketContext::new(), &FinstackConfig::default())
        .expect("Exclude WeightedSum should solve");
    assert!(result.status.is_feasible(), "got {:?}", result.status);
    assert!(
        (result.optimal_weights["POS_MISSING"] - (1.0 / 3.0)).abs() < 1e-9,
        "missing Ytm must stay frozen at current weight, got {}",
        result.optimal_weights["POS_MISSING"]
    );
    assert!(
        result.optimal_weights["POS_HI"] > result.optimal_weights["POS_LO"],
        "free weight should move toward the high-Ytm name"
    );
}
