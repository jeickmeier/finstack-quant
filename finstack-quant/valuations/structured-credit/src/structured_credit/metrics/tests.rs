use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_core::pricer::InstrumentType;

/// Pool and deal-level metrics that compute purely from the deal model are
/// reachable through the registry, so `price_instrument` and
/// `list_standard_metrics` surface them by id without extra wiring.
#[test]
fn data_complete_deal_metrics_are_registered_for_structured_credit() {
    let registry = crate::STRUCTURED_CREDIT.metrics();
    for id in [
        MetricId::CloWas,
        MetricId::AbsChargeOff,
        MetricId::AbsDelinquency,
        MetricId::AbsExcessSpread,
        MetricId::AbsPaymentRate,
        MetricId::AbsCreditEnhancement,
    ] {
        assert!(
            registry.is_applicable(&id, InstrumentType::StructuredCredit),
            "metric {id} should be registered for StructuredCredit"
        );
    }
}
