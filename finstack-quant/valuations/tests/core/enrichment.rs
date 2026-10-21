//! Metric enrichment through a caller-supplied registry.

use finstack_quant_core::dates::Date;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::{ModelKey, PricerRegistry, PricingError};
use finstack_quant_valuations::results::ValuationResult;
use std::sync::Arc;

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::StubKind;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Rate;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::{MetricCalculator, MetricContext, MetricRegistry};
use finstack_quant_valuations::pricer::{InstrumentType, Pricer, PricerKey};
use std::sync::atomic::{AtomicUsize, Ordering};
use time::macros::date;

struct FixedHazardBondPricer {
    calls: Arc<AtomicUsize>,
}

impl Pricer for FixedHazardBondPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Bond, ModelKey::HazardRate)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        _market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ValuationResult::stamped(
            instrument.id(),
            as_of,
            Money::from((321_i64, Currency::USD)),
        ))
    }
}

struct RepriceSpreadMetric;

impl MetricCalculator for RepriceSpreadMetric {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        context.reprice_raw(context.curves.as_ref(), context.as_of)
    }
}

#[test]
fn spread_equivalent_metric_preserves_selected_custom_registry() {
    let as_of = date!(2025 - 01 - 15);
    let bond = Bond::fixed(
        "SPREAD-DISPATCH",
        Money::from((1_000_i64, Currency::USD)),
        Rate::from_decimal(0.04).expect("valid rate fixture"),
        date!(2020 - 01 - 15),
        date!(2030 - 01 - 15),
        StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("valid bond");
    let calls = Arc::new(AtomicUsize::new(0));
    let mut pricers = PricerRegistry::new();
    pricers
        .register(FixedHazardBondPricer {
            calls: Arc::clone(&calls),
        })
        .expect("unique custom pricer");
    let mut metrics = MetricRegistry::new();
    metrics
        .register_metric(
            MetricId::ZSpread,
            Arc::new(RepriceSpreadMetric),
            &[InstrumentType::Bond],
        )
        .expect("unique custom metric");

    let result = pricers
        .price_with_metrics(
            &bond,
            ModelKey::HazardRate,
            &MarketContext::new(),
            as_of,
            &[MetricId::ZSpread],
            finstack_quant_valuations::instruments::PricingOptions::default()
                .with_metric_registry(Arc::new(metrics)),
        )
        .expect("spread metric must reprice through the selected registry");

    assert_eq!(result.value.amount(), 321.0);
    assert_eq!(result.measures.get(&MetricId::ZSpread), Some(&321.0));
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "base PV and spread-metric reprice must use the selected pricer"
    );
}
