//! Aggregation metrics tests for portfolio.

mod common;

use common::*;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::PortfolioBuilder;
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_valuations::instruments::{Attributes, Instrument};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::InstrumentType;
use finstack_quant_valuations::results::ValuationResult;
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

finstack_quant_valuations::impl_empty_cashflow_provider!(
    FixedMetricInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
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
        InstrumentType::Bond
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

/// Prices fine but fails to produce any risk metric, so the position is
/// degraded to a PV-only valuation and contributes nothing to metric totals.
#[derive(Clone)]
struct MetricFailingInstrument {
    id: String,
    value: Money,
    attributes: Attributes,
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    MetricFailingInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl Instrument for MetricFailingInstrument {
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
        InstrumentType::Bond
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
        _as_of: Date,
        _metrics: &[MetricId],
        _options: finstack_quant_valuations::instruments::PricingOptions,
    ) -> finstack_quant_valuations::Result<ValuationResult> {
        Err(finstack_quant_core::Error::Input(finstack_quant_core::InputError::Invalid).into())
    }
}

/// A degraded position contributes zero to every metric total. Without a
/// portfolio-level record of which positions degraded, a DV01 total of 1000
/// from two 1000-DV01 positions is indistinguishable from a correct total.
/// `PortfolioMetrics::degraded_positions` makes the partial aggregate
/// self-describing.
#[test]
fn degraded_positions_are_reported_on_portfolio_metrics() {
    let as_of = base_date();
    let mut measures = IndexMap::new();
    measures.insert(MetricId::Dv01, 1000.0);

    let healthy = Position::new(
        "HEALTHY",
        "E1",
        "HEALTHY_INST",
        Arc::new(FixedMetricInstrument::new(
            "HEALTHY_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            measures,
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();
    let degraded = Position::new(
        "DEGRADED",
        "E1",
        "DEGRADED_INST",
        Arc::new(MetricFailingInstrument {
            id: "DEGRADED_INST".to_string(),
            value: Money::new(100.0, Currency::USD).expect("valid money fixture"),
            attributes: Attributes::new(),
        }),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("P")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E1"))
        .position(healthy)
        .position(degraded)
        .build()
        .unwrap();

    let market = market_with_usd();
    let valuation = finstack_quant_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &FinstackConfig::default(),
        &finstack_quant_portfolio::valuation::PortfolioValuationOptions {
            strict_risk: false,
            ..Default::default()
        },
    )
    .unwrap();
    let metrics = finstack_quant_portfolio::metrics::aggregate_metrics(
        &valuation,
        Currency::USD,
        &market,
        as_of,
    )
    .unwrap();

    // The total is partial: only the healthy position contributed.
    assert_eq!(metrics.get_total("dv01"), Some(1000.0));
    // Non-finite skips do not cover this failure mode.
    assert!(metrics.skipped_metrics.is_empty());
    // The partial total must be self-describing.
    assert_eq!(
        metrics
            .degraded_positions
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>(),
        vec!["DEGRADED".to_string()]
    );
}

/// `fx_delta` is a currency-denominated, position-linear sensitivity and must
/// aggregate like DV01. Metrics that are present per-position but excluded
/// from aggregation must be named on the result so the omission is visible.
#[test]
fn additive_currency_metrics_aggregate_and_omissions_are_reported() {
    let as_of = base_date();
    let mut measures = IndexMap::new();
    measures.insert(MetricId::FxDelta, 500.0);
    measures.insert(MetricId::custom("exotic_widget_score"), 7.0);

    let position = Position::new(
        "POS_FX",
        "E1",
        "FX_INST",
        Arc::new(FixedMetricInstrument::new(
            "FX_INST",
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
            measures,
        )),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("P")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E1"))
        .position(position)
        .build()
        .unwrap();

    let market = market_with_usd();
    let valuation = finstack_quant_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &FinstackConfig::default(),
        &Default::default(),
    )
    .unwrap();
    let metrics = finstack_quant_portfolio::metrics::aggregate_metrics(
        &valuation,
        Currency::USD,
        &market,
        as_of,
    )
    .unwrap();

    assert_eq!(metrics.get_total("fx_delta"), Some(500.0));
    assert!(metrics.get_total("exotic_widget_score").is_none());
    assert_eq!(
        metrics.unaggregated_metrics,
        vec!["exotic_widget_score".to_string()]
    );
}

#[test]
fn m17_aggregate_metrics_rejects_mismatched_base_currency() {
    let as_of = base_date();
    let end_date = as_of + Duration::days(30);

    let dep = Deposit::builder()
        .id("DEP_1M".into())
        .notional(Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
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
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E1"))
        .position(position)
        .build()
        .unwrap();

    let market = market_with_usd();
    let config = FinstackConfig::default();
    let valuation = finstack_quant_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .unwrap();

    let err = finstack_quant_portfolio::metrics::aggregate_metrics(
        &valuation,
        Currency::EUR,
        &market,
        as_of,
    )
    .expect_err("M-17: mismatched aggregation base currency must fail");
    assert!(
        err.to_string().contains("base_currency"),
        "unexpected error: {err}"
    );
}

#[test]
fn m17_aggregate_metrics_rejects_mismatched_as_of() {
    let as_of = base_date();
    let end_date = as_of + Duration::days(30);

    let dep = Deposit::builder()
        .id("DEP_1M".into())
        .notional(Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
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
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E1"))
        .position(position)
        .build()
        .unwrap();

    let market = market_with_usd();
    let config = FinstackConfig::default();
    let valuation = finstack_quant_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .unwrap();

    let err = finstack_quant_portfolio::metrics::aggregate_metrics(
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
        Money::new(100.0, Currency::USD).expect("valid money fixture"),
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
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E1"))
        .position(long)
        .position(short)
        .build()
        .unwrap();

    let market = market_with_usd();
    let config = FinstackConfig::default();
    let valuation = finstack_quant_portfolio::valuation::value_portfolio(
        &portfolio,
        &market,
        &config,
        &Default::default(),
    )
    .unwrap();
    let metrics = finstack_quant_portfolio::metrics::aggregate_metrics(
        &valuation,
        Currency::USD,
        &market,
        as_of,
    )
    .unwrap();

    let long_metrics = metrics.get_position_metrics("LONG").unwrap();
    let short_metrics = metrics.get_position_metrics("SHORT").unwrap();

    assert_eq!(long_metrics.get("ytm"), Some(&0.05));
    assert_eq!(short_metrics.get("ytm"), Some(&0.05));
    assert_eq!(long_metrics.get("dv01"), Some(&5.0));
    assert_eq!(short_metrics.get("dv01"), Some(&-7.5));
    assert_eq!(metrics.get_total("dv01"), Some(-2.5));
}
