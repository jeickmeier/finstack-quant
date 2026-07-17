//! Valuation fallback tests for portfolio.

mod common;

use common::*;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::{Error, InputError};
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::PortfolioBuilder;
use finstack_quant_valuations::instruments::{Attributes, Instrument};
use finstack_quant_valuations::pricer::InstrumentType;
use finstack_quant_valuations::results::ValuationResult;
use std::any::Any;
use std::sync::Arc;

#[derive(Clone)]
struct ValueOnlyInstrument {
    id: String,
    currency: Currency,
    value: f64,
    attributes: Attributes,
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    ValueOnlyInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl ValueOnlyInstrument {
    fn new(id: &str, currency: Currency, value: f64) -> Self {
        Self {
            id: id.to_string(),
            currency,
            value,
            attributes: Attributes::new(),
        }
    }
}

impl Instrument for ValueOnlyInstrument {
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
        Ok(Money::new(self.value, self.currency))
    }

    fn price_with_metrics(
        &self,
        _curves: &MarketContext,
        _as_of: Date,
        _metrics: &[finstack_quant_valuations::metrics::MetricId],
        _options: finstack_quant_valuations::instruments::PricingOptions,
    ) -> finstack_quant_core::Result<ValuationResult> {
        Err(Error::Input(InputError::Invalid))
    }
}

#[test]
fn valuation_falls_back_when_metrics_fail() {
    let as_of = base_date();
    let inst = Arc::new(ValueOnlyInstrument::new("VO", Currency::USD, 123.45));
    let pos = Position::new("P", "E", "VO", inst, 1.0, PositionUnit::Units).unwrap();

    let portfolio = PortfolioBuilder::new("PF")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E"))
        .position(pos)
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

    let pv = valuation.get_position_value("P").unwrap();
    assert_eq!(pv.value_native.currency(), Currency::USD);
    assert!((pv.value_native.amount() - 123.45).abs() < 1e-9);
    assert!(
        valuation.has_degraded_risk(),
        "fallback valuation should mark the portfolio as degraded"
    );
    assert_eq!(valuation.degraded_positions().len(), 1);
    assert_eq!(valuation.degraded_positions()[0], "P");
    assert!(
        !pv.risk_metrics_complete,
        "position should be marked as missing requested risk metrics"
    );
    assert!(
        pv.risk_error
            .as_deref()
            .is_some_and(|msg| msg.contains("Invalid")),
        "expected the underlying metrics failure to be surfaced"
    );
}

#[test]
fn fallback_valuation_stamps_caller_config() {
    let as_of = base_date();
    let inst = Arc::new(ValueOnlyInstrument::new("VO_CONFIG", Currency::USD, 123.45));
    let pos = Position::new(
        "P_CONFIG",
        "E_CONFIG",
        "VO_CONFIG",
        inst,
        1.0,
        PositionUnit::Units,
    )
    .expect("position");
    let portfolio = PortfolioBuilder::new("PF_CONFIG")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E_CONFIG"))
        .position(pos)
        .build()
        .expect("portfolio");
    let mut config = FinstackConfig::default();
    config
        .rounding
        .output_scale
        .overrides
        .insert(Currency::USD, 4);

    let valuation = finstack_quant_portfolio::valuation::value_portfolio(
        &portfolio,
        &market_with_usd(),
        &config,
        &Default::default(),
    )
    .expect("fallback valuation");
    let result = valuation
        .get_position_value("P_CONFIG")
        .and_then(|value| value.valuation_result.as_ref())
        .expect("fallback result");

    assert_eq!(
        result
            .meta
            .rounding
            .output_scale_by_ccy
            .get(&Currency::USD)
            .copied(),
        Some(4),
        "PV-only fallback must stamp the caller's FinstackConfig",
    );
}
