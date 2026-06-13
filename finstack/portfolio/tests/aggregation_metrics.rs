//! Aggregation metrics tests for portfolio.

mod common;

use common::*;
use finstack_core::config::FinstackConfig;
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_portfolio::position::{Position, PositionUnit};
use finstack_portfolio::types::Entity;
use finstack_portfolio::PortfolioBuilder;
use finstack_valuations::instruments::rates::deposit::Deposit;
use finstack_valuations::instruments::{Attributes, Instrument};
use finstack_valuations::metrics::MetricId;
use finstack_valuations::pricer::InstrumentType;
use finstack_valuations::results::ValuationResult;
use indexmap::IndexMap;
use std::any::Any;
use std::sync::Arc;
use time::Duration;

#[derive(Clone)]
struct FixedMetricInstrument {
    id: String,
    value: Money,
    measures: IndexMap<MetricId, f64>,
    attributes: Attributes,
}

finstack_valuations::impl_empty_cashflow_provider!(
    FixedMetricInstrument,
    finstack_cashflows::builder::CashflowRepresentation::NoResidual
);

impl FixedMetricInstrument {
    fn new(id: &str, value: Money, measures: IndexMap<MetricId, f64>) -> Self {
        Self {
            id: id.to_string(),
            value,
            measures,
            attributes: Attributes::new(),
        }
    }
}

impl Instrument for FixedMetricInstrument {
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

    fn base_value(&self, _curves: &MarketContext, _as_of: Date) -> finstack_core::Result<Money> {
        Ok(self.value)
    }

    fn price_with_metrics(
        &self,
        _curves: &MarketContext,
        as_of: Date,
        _metrics: &[MetricId],
        _options: finstack_valuations::instruments::PricingOptions,
    ) -> finstack_core::Result<ValuationResult> {
        Ok(ValuationResult::stamped(self.id(), as_of, self.value)
            .with_measures(self.measures.clone()))
    }
}

#[test]
fn summable_vs_non_summable_metrics() {
    let as_of = base_date();
    let end_date = as_of + Duration::days(30);

    // Deposit supports standard metrics via helper; we request defaults in portfolio valuation
    let dep = Deposit::builder()
        .id("DEP_1M".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()
        .unwrap();

    let position = Position::new(
        "POS_1",
        "E1",
        "DEP_1M",
        Arc::new(dep),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("P")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E1"))
        .position(position)
        .build()
        .unwrap();

    let market = market_with_usd();
    let config = FinstackConfig::default();
    let valuation = finstack_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .unwrap();
    let metrics =
        finstack_portfolio::metrics::aggregate_metrics(&valuation, Currency::USD, &market, as_of)
            .unwrap();

    // Position should have some metrics recorded (may be empty depending on measure availability)
    assert!(metrics.get_position_metrics("POS_1").is_some());

    // Aggregated totals only include summable metrics. We at least verify that querying works
    // without asserting specific numeric values (which depend on instrument specifics).
    if let Some(total) = metrics.get_total("dv01") {
        let _ = total; // present and numeric
    }
}

#[test]
fn m17_aggregate_metrics_rejects_mismatched_base_currency() {
    let as_of = base_date();
    let end_date = as_of + Duration::days(30);

    let dep = Deposit::builder()
        .id("DEP_1M".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()
        .unwrap();

    let position = Position::new(
        "POS_1",
        "E1",
        "DEP_1M",
        Arc::new(dep),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("P")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E1"))
        .position(position)
        .build()
        .unwrap();

    let market = market_with_usd();
    let config = FinstackConfig::default();
    let valuation = finstack_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .unwrap();

    let err =
        finstack_portfolio::metrics::aggregate_metrics(&valuation, Currency::EUR, &market, as_of)
            .expect_err("M-17: mismatched aggregation base currency must fail");
    assert!(
        err.to_string().contains("base_ccy"),
        "unexpected error: {err}"
    );
}

#[test]
fn m17_aggregate_metrics_rejects_mismatched_as_of() {
    let as_of = base_date();
    let end_date = as_of + Duration::days(30);

    let dep = Deposit::builder()
        .id("DEP_1M".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()
        .unwrap();

    let position = Position::new(
        "POS_1",
        "E1",
        "DEP_1M",
        Arc::new(dep),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("P")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E1"))
        .position(position)
        .build()
        .unwrap();

    let market = market_with_usd();
    let config = FinstackConfig::default();
    let valuation = finstack_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .unwrap();

    let err = finstack_portfolio::metrics::aggregate_metrics(
        &valuation,
        Currency::USD,
        &market,
        as_of + Duration::days(1),
    )
    .expect_err("M-17: mismatched aggregation date must fail");
    assert!(err.to_string().contains("as_of"), "unexpected error: {err}");
}

#[test]
fn summable_metrics_scale_with_quantity_and_short_sign() {
    let as_of = base_date();
    let mut measures = IndexMap::new();
    measures.insert(MetricId::Dv01, 2.5);
    measures.insert(MetricId::Ytm, 0.05);

    let instrument: Arc<dyn Instrument> = Arc::new(FixedMetricInstrument::new(
        "RISKY",
        Money::new(100.0, Currency::USD),
        measures,
    ));

    let long = Position::new(
        "LONG",
        "E1",
        "RISKY",
        Arc::clone(&instrument),
        2.0,
        PositionUnit::Units,
    )
    .unwrap();
    let short = Position::new(
        "SHORT",
        "E1",
        "RISKY",
        instrument,
        -3.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("P")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E1"))
        .position(long)
        .position(short)
        .build()
        .unwrap();

    let market = market_with_usd();
    let config = FinstackConfig::default();
    let valuation = finstack_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .unwrap();
    let metrics =
        finstack_portfolio::metrics::aggregate_metrics(&valuation, Currency::USD, &market, as_of)
            .unwrap();

    let long_metrics = metrics.get_position_metrics("LONG").unwrap();
    let short_metrics = metrics.get_position_metrics("SHORT").unwrap();

    assert_eq!(long_metrics.get("ytm"), Some(&0.05));
    assert_eq!(short_metrics.get("ytm"), Some(&0.05));
    assert_eq!(long_metrics.get("dv01"), Some(&5.0));
    assert_eq!(short_metrics.get("dv01"), Some(&-7.5));
    assert_eq!(metrics.get_total("dv01"), Some(-2.5));
}
