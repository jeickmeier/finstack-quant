use super::*;
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_core::metrics::MetricRegistry;
use finstack_quant_valuations_core::pricer::InstrumentType;

#[test]
fn registers_fx_delta_convention_metrics() {
    let mut registry = MetricRegistry::new();
    register_fx_option_metrics(&mut registry).expect("FX option metric registration");
    let metrics = registry.metrics_for_instrument(InstrumentType::FxOption);

    assert!(metrics.contains(&MetricId::DeltaForward));
    assert!(metrics.contains(&MetricId::DeltaPremiumAdjustedSpot));
    assert!(metrics.contains(&MetricId::DeltaPremiumAdjustedForward));
}
