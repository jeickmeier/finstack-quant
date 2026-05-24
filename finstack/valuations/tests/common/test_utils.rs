use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use finstack_core::Result;
use finstack_valuations::instruments::{
    Attributes, Instrument, InstrumentCurves, MarketDependencies,
};
use finstack_valuations::metrics::MetricId;
use finstack_valuations::results::ValuationResult;
use smallvec::SmallVec;
use std::sync::OnceLock;

#[derive(Clone)]
pub struct TestInstrument {
    id: String,
    value: Money,
    discount_curves: SmallVec<[CurveId; 2]>,
}

finstack_valuations::impl_empty_cashflow_provider!(
    TestInstrument,
    finstack_cashflows::builder::CashflowRepresentation::NoResidual
);

impl TestInstrument {
    pub fn new(id: &str, value: Money) -> Self {
        Self {
            id: id.to_string(),
            value,
            discount_curves: SmallVec::<[CurveId; 2]>::new(),
        }
    }

    pub fn with_discount_curves(mut self, curves: &[&str]) -> Self {
        self.discount_curves = curves.iter().map(|id| CurveId::new(*id)).collect();
        self
    }
}

impl Instrument for TestInstrument {
    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> finstack_valuations::pricer::InstrumentType {
        finstack_valuations::pricer::InstrumentType::Bond
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

    fn market_dependencies(&self) -> finstack_core::Result<MarketDependencies> {
        let mut deps = MarketDependencies::new();
        for curve in &self.discount_curves {
            deps.add_curves(
                InstrumentCurves::builder()
                    .discount(curve.clone())
                    .build()?,
            );
        }
        Ok(deps)
    }

    fn base_value(&self, _market: &MarketContext, _as_of: Date) -> Result<Money> {
        Ok(self.value)
    }

    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        _metrics: &[MetricId],
        _options: finstack_valuations::instruments::PricingOptions,
    ) -> Result<ValuationResult> {
        let value = self.value(market, as_of)?;
        Ok(ValuationResult::stamped(self.id(), as_of, value))
    }
}
