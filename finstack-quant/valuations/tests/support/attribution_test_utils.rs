//! Tests for the surrounding crate component and its documented behavior.
//!
use crate::instruments::common_impl::traits::{Attributes, Instrument};
use smallvec::SmallVec;
use crate::metrics::MetricId;
use crate::results::ValuationResult;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use std::sync::OnceLock;

#[derive(Clone)]
pub struct TestInstrument {
    id: String,
    value: Money,
    discount_curves: SmallVec<[CurveId; 2]>,
    forward_curves: SmallVec<[CurveId; 2]>,
}

impl TestInstrument {
    pub fn new(id: &str, value: Money) -> Self {
        Self {
            id: id.to_string(),
            value,
            discount_curves: SmallVec::<[CurveId; 2]>::new(),
            forward_curves: SmallVec::<[CurveId; 2]>::new(),
        }
    }

    pub fn with_discount_curves(mut self, curves: &[&str]) -> Self {
        self.discount_curves = curves.iter().map(|id| CurveId::new(*id)).collect();
        self
    }

    pub fn with_forward_curves(mut self, curves: &[&str]) -> Self {
        self.forward_curves = curves.iter().map(|id| CurveId::new(*id)).collect();
        self
    }
}

impl Instrument for TestInstrument {
    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> crate::pricer::InstrumentType {
        crate::pricer::InstrumentType::Bond
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn attributes(&self) -> &Attributes {
        static ATTRS: OnceLock<Attributes> = OnceLock::new();
        ATTRS.get_or_init(Attributes::default)
    }

    fn attributes_mut(&mut self) -> &mut Attributes {
        unreachable!("TestInstrument::attributes_mut should not be called in tests")
    }

    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::dependencies::MarketDependencies> {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        for curve in &self.discount_curves {
            deps.add_discount_curve(curve.clone());
        }
        for curve in &self.forward_curves {
            deps.add_forward_curve(curve.clone());
        }
        Ok(deps)
    }

    fn base_value(&self, market: &MarketContext, _as_of: Date) -> Result<Money> {
        let mut amt = self.value.amount();
        for id in &self.forward_curves {
            let fwd = market.get_forward(id.as_str())?;
            // Deterministic exposure to parallel forward moves (test-only stub).
            amt += fwd.rate(1.0) * 1_000_000.0;
        }
        Ok(Money::new(amt, self.value.currency()))
    }

    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        _metrics: &[MetricId],
        _options: finstack_quant_valuations::instruments::PricingOptions,
    ) -> Result<ValuationResult> {
        let value = self.value(market, as_of)?;
        Ok(ValuationResult::stamped(self.id(), as_of, value))
    }
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    TestInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);
