//! DV01 metric tests for FX Spot.
//!
//! FX spot has no discount or forward curve dependency. Rate DV01 metrics are
//! intentionally not registered; FX risk is exposed through FxDelta and Fx01.

use super::super::common::*;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::{instruments::Instrument, metrics::MetricId};

#[test]
fn test_dv01_not_registered_for_fx_spot() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20).with_settlement(d(2025, 1, 17));
    let result = fx.price_with_metrics(
        &MarketContext::new(),
        test_date(),
        &[MetricId::Dv01],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    let result = result.expect("FxSpot should still price without DV01");
    assert!(
        !result.measures.contains_key("dv01"),
        "FxSpot should not emit a DV01 measure"
    );
}

#[test]
fn test_bucketed_dv01_not_registered_for_fx_spot() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20).with_settlement(d(2025, 1, 17));
    let result = fx.price_with_metrics(
        &MarketContext::new(),
        test_date(),
        &[MetricId::BucketedDv01],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    let result = result.expect("FxSpot should still price without BucketedDv01");
    assert!(
        !result.measures.contains_key("bucketed_dv01"),
        "FxSpot should not emit a BucketedDv01 measure"
    );
}
