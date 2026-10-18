//! Smoke test of the framework crate on its own: the universal metric registry
//! and the pricer key vocabulary are usable without any instrument family.
//! The framework tests that need instruments run in the umbrella crate's
//! `core_framework` integration module.

use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_core::pricer::{Family, InstrumentType, ModelKey, PricerKey};

#[test]
fn unregistered_family_has_universal_metrics_and_no_pricers() {
    let family = Family::unregistered();
    assert!(family.metrics().has_metric(MetricId::Theta));
    assert!(family.metrics().has_metric(MetricId::CarryTotal));
    assert!(family
        .pricers()
        .get_pricer(PricerKey::new(InstrumentType::Bond, ModelKey::Discounting))
        .is_none());
}
