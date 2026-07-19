//! Integration scenarios tests for portfolio.

mod common;

use common::*;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::PortfolioBuilder;
use finstack_quant_scenarios::spec::{CurveKind, OperationSpec, ScenarioSpec};
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_valuations::instruments::{
    Attributes, Instrument, MarketDependencies, PricingOptions,
};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::InstrumentType;
use finstack_quant_valuations::results::ValuationResult;
use std::any::Any;
use std::sync::Arc;
use time::Duration;

#[test]
fn apply_and_revalue_succeeds() {
    let as_of = base_date();
    let end_date = as_of + Duration::days(30);

    let dep = Deposit::builder()
        .id("D".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()
        .unwrap();

    let pos = Position::new("P", "E", "D", Arc::new(dep), 1.0, PositionUnit::Units).unwrap();
    let portfolio = PortfolioBuilder::new("PF")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E"))
        .position(pos)
        .build()
        .unwrap();

    let market = market_with_usd();
    let config = FinstackConfig::default();

    // Get base valuation first
    let base_valuation = finstack_quant_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .unwrap();

    let scenario = ScenarioSpec {
        id: "s".to_string(),
        name: Some("s".to_string()),
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD".into(),
            discount_curve_id: None,
            bp: 10.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let (shocked_valuation, report) = finstack_quant_portfolio::scenarios::apply_and_revalue(
        &portfolio, &scenario, &market, &config,
    )
    .unwrap();
    assert!(report.operations_applied > 0);

    // Verify the shocked valuation differs from base
    // +10bp shift should change deposit value slightly
    let base_total = base_valuation.total_base_ccy.amount();
    let shocked_total = shocked_valuation.total_base_ccy.amount();

    // For a 30-day deposit, +10bp should have a small but measurable impact
    // Don't assert sign as deposits may behave differently than bonds
    assert!(
        (shocked_total - base_total).abs() > 0.01,
        "Scenario should have measurable impact: base={}, shocked={}",
        base_total,
        shocked_total
    );
}

/// End-to-end scenario P&L across a two-entity book: the drill-down must foot
/// to the headline, and the headline must agree with an independently computed
/// stressed-minus-base total.
#[test]
fn scenario_pnl_reconciles_end_to_end() {
    let as_of = base_date();

    let build_deposit = |id: &str, notional: f64, days: i64| {
        Deposit::builder()
            .id(id.into())
            .notional(Money::new(notional, Currency::USD))
            .start_date(as_of)
            .maturity(as_of + Duration::days(days))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .unwrap()
    };

    let pos_a = Position::new(
        "P_A",
        "E_A",
        "D_A",
        Arc::new(build_deposit("D_A", 1_000_000.0, 30)),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();
    let pos_b = Position::new(
        "P_B",
        "E_B",
        "D_B",
        Arc::new(build_deposit("D_B", 2_500_000.0, 180)),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("PF")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E_A"))
        .entity(Entity::new("E_B"))
        .position(pos_a)
        .position(pos_b)
        .build()
        .unwrap();

    let market = market_with_usd();
    let config = FinstackConfig::default();

    let scenario = ScenarioSpec {
        id: "pnl".to_string(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD".into(),
            discount_curve_id: None,
            bp: 100.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let (pnl, report) =
        finstack_quant_portfolio::scenarios::scenario_pnl(&portfolio, &scenario, &market, &config)
            .unwrap();

    assert!(report.operations_applied > 0);
    assert_eq!(pnl.by_position.len(), 2);
    assert_eq!(pnl.total.currency(), Currency::USD);

    // The drill-down foots to the headline.
    let drilldown: f64 = pnl.by_position.values().map(|m| m.amount()).sum();
    assert!(
        (drilldown - pnl.total.amount()).abs() < 1e-6,
        "drill-down {drilldown} must foot to total {}",
        pnl.total.amount()
    );

    // The headline agrees with an independently computed stressed-minus-base.
    let base = finstack_quant_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .unwrap();
    let (stressed, _) = finstack_quant_portfolio::scenarios::apply_and_revalue(
        &portfolio, &scenario, &market, &config,
    )
    .unwrap();
    let expected = stressed.total_base_ccy.amount() - base.total_base_ccy.amount();
    assert!(
        (pnl.total.amount() - expected).abs() < 1e-6,
        "total {} must equal stressed-minus-base {expected}",
        pnl.total.amount()
    );

    // A 100bp shock on a real book must move something.
    assert!(pnl.total.amount().abs() > 0.01);
}

/// A scenario with no operations must produce exactly zero P&L in every
/// position and in the total — the deterministic-baseline check.
#[test]
fn scenario_pnl_no_op_scenario_is_flat() {
    let as_of = base_date();

    let dep = Deposit::builder()
        .id("D".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(as_of + Duration::days(90))
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()
        .unwrap();

    let pos = Position::new("P", "E", "D", Arc::new(dep), 1.0, PositionUnit::Units).unwrap();
    let portfolio = PortfolioBuilder::new("PF")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E"))
        .position(pos)
        .build()
        .unwrap();

    let scenario = ScenarioSpec {
        id: "noop".to_string(),
        name: None,
        description: None,
        operations: vec![],
        priority: 0,
        resolution_mode: Default::default(),
    };

    let (pnl, _report) = finstack_quant_portfolio::scenarios::scenario_pnl(
        &portfolio,
        &scenario,
        &market_with_usd(),
        &FinstackConfig::default(),
    )
    .unwrap();

    assert_eq!(pnl.total.amount(), 0.0);
    for (position_id, delta) in &pnl.by_position {
        assert_eq!(delta.amount(), 0.0, "position '{position_id}' must be flat");
    }

    let shock = ScenarioSpec {
        id: "shock".to_string(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD".into(),
            discount_curve_id: None,
            bp: 25.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };
    let market = market_with_usd();
    let config = FinstackConfig::default();
    let batch = finstack_quant_portfolio::scenarios::scenario_pnl_batch(
        &portfolio,
        &[scenario, shock.clone()],
        &market,
        &config,
    )
    .expect("scenario batch");
    let (standalone, _) =
        finstack_quant_portfolio::scenarios::scenario_pnl(&portfolio, &shock, &market, &config)
            .expect("standalone scenario");

    assert_eq!(
        batch
            .iter()
            .map(|item| item.scenario_id.as_str())
            .collect::<Vec<_>>(),
        vec!["noop", "shock"],
    );
    assert_eq!(batch[0].pnl.total.amount(), 0.0);
    assert!(
        (batch[1].pnl.total.amount() - standalone.total.amount()).abs() < 1e-10,
        "batched and standalone P&L must match"
    );
}

#[derive(Clone)]
struct ScenarioFailureInstrument {
    attributes: Attributes,
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    ScenarioFailureInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl ScenarioFailureInstrument {
    fn checked_value(&self, market: &MarketContext) -> finstack_quant_core::Result<Money> {
        let flag = match market.get_price("SCENARIO_FAILURE_FLAG")? {
            MarketScalar::Unitless(value) => *value,
            MarketScalar::Price(value) => value.amount(),
        };
        if flag < 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "earlier scenario valuation failure".to_string(),
            ));
        }
        Ok(Money::new(flag, Currency::USD))
    }
}

impl Instrument for ScenarioFailureInstrument {
    fn id(&self) -> &str {
        "SCENARIO_FAILURE_INSTRUMENT"
    }

    fn key(&self) -> InstrumentType {
        InstrumentType::Basket
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
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
        market: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        self.checked_value(market)
    }

    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        _metrics: &[MetricId],
        options: PricingOptions,
    ) -> finstack_quant_core::Result<ValuationResult> {
        let config = options.config.as_deref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "scenario test expected the executor request config".to_string(),
            )
        })?;
        Ok(ValuationResult::stamped_with_config(
            self.id(),
            as_of,
            self.checked_value(market)?,
            config,
        ))
    }

    fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
        let mut dependencies = MarketDependencies::new();
        dependencies.add_spot_id("SCENARIO_FAILURE_FLAG");
        Ok(dependencies)
    }
}

#[test]
fn scenario_batch_reports_earliest_error_across_application_and_valuation_phases() {
    let portfolio = PortfolioBuilder::new("ERROR_ORDER_PORTFOLIO")
        .base_ccy(Currency::USD)
        .as_of(base_date())
        .entity(Entity::new("ENTITY"))
        .position(
            Position::new(
                "ERROR_ORDER_POSITION",
                "ENTITY",
                "SCENARIO_FAILURE_INSTRUMENT",
                Arc::new(ScenarioFailureInstrument {
                    attributes: Attributes::new(),
                }),
                1.0,
                PositionUnit::Units,
            )
            .expect("valid error-order position"),
        )
        .build()
        .expect("valid error-order portfolio");
    let market =
        market_with_usd().insert_price("SCENARIO_FAILURE_FLAG", MarketScalar::Unitless(1.0));
    let scenarios = vec![
        ScenarioSpec {
            id: "earlier_valuation_error".to_string(),
            name: None,
            description: None,
            operations: vec![OperationSpec::EquityPricePct {
                ids: vec!["SCENARIO_FAILURE_FLAG".to_string()],
                pct: -200.0,
            }],
            priority: 0,
            resolution_mode: Default::default(),
        },
        ScenarioSpec {
            id: "later_application_error".to_string(),
            name: None,
            description: None,
            operations: vec![OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "MISSING_CURVE".into(),
                discount_curve_id: None,
                bp: 10.0,
            }],
            priority: 0,
            resolution_mode: Default::default(),
        },
    ];

    for _ in 0..8 {
        let error = finstack_quant_portfolio::scenarios::scenario_pnl_batch(
            &portfolio,
            &scenarios,
            &market,
            &FinstackConfig::default(),
        )
        .expect_err("both scenarios are configured to fail");
        let message = error.to_string();
        assert!(
            message.contains("earlier scenario valuation failure"),
            "the first logical scenario error must win across phases: {message}"
        );
        assert!(
            !message.contains("MISSING_CURVE"),
            "a later application error must not mask an earlier valuation error: {message}"
        );
    }
}
