//! DV01 metric tests for FX Spot.
//!
//! FX spot has no discount or forward curve dependency. Rate DV01 metrics are
//! intentionally not registered; FX risk is exposed through FxDelta and Fx01.
//! Requesting one is therefore rejected rather than silently omitted — see the
//! requested-metric contract in `src/pricer/README.md`.

use super::super::common::*;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::{instruments::Instrument, metrics::MetricId};

/// Request `metric` on an FX spot position and return the rejection message.
fn reject_message(metric: MetricId) -> String {
    let fx = eurusd_with_notional(1_000_000.0, 1.20).with_settlement(d(2025, 1, 17));
    let error = fx
        .price_with_metrics(
            &MarketContext::new(),
            test_date(),
            std::slice::from_ref(&metric),
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect_err("a metric with no FxSpot calculator must be rejected, not dropped");
    let message = error.to_string();
    assert!(
        message.contains(metric.as_str()) && message.contains("fx_spot"),
        "rejection must name the metric and the instrument type; got: {message}"
    );
    message
}

#[test]
fn test_dv01_not_registered_for_fx_spot() {
    reject_message(MetricId::Dv01);
}

#[test]
fn test_bucketed_dv01_not_registered_for_fx_spot() {
    reject_message(MetricId::BucketedDv01);
}

#[test]
fn fx_spot_still_computes_its_registered_metrics() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20).with_settlement(d(2025, 1, 17));
    let result = fx
        .price_with_metrics(
            &MarketContext::new(),
            test_date(),
            &[MetricId::SpotRate, MetricId::BaseAmount],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("registered FxSpot metrics must still compute");
    for metric in ["spot_rate", "base_amount"] {
        assert!(
            result.measures.contains_key(metric),
            "registered metric `{metric}` must be present"
        );
    }
}
