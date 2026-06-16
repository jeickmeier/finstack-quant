//! Tests for rounding policy stamping and display in attribution.

use crate::common::test_utils::TestInstrument;
use finstack_quant_attribution::{
    attribute_pnl_parallel, AttributionMethod, ExecutionPolicy, PnlAttribution,
};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use std::sync::Arc;
use time::macros::date;

#[test]
fn parallel_stamps_configured_rounding_context() {
    let as_of_t0 = date!(2025 - 01 - 01);
    let as_of_t1 = date!(2025 - 01 - 02);

    let instrument: Arc<dyn Instrument> = Arc::new(TestInstrument::new(
        "TEST-ROUND",
        Money::new(1_000.0, Currency::USD),
    ));
    let market_t0 = MarketContext::new();
    let market_t1 = MarketContext::new();

    let mut config = FinstackConfig::default();
    config
        .rounding
        .output_scale
        .overrides
        .insert(Currency::USD, 4);
    config
        .rounding
        .ingest_scale
        .overrides
        .insert(Currency::USD, 4);

    let attribution = attribute_pnl_parallel(
        &instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .expect("Attribution should succeed");

    let rounding = attribution.meta.rounding;
    assert_eq!(
        rounding.output_scale_by_ccy.get(&Currency::USD),
        Some(&4),
        "Output scale for USD should reflect configured rounding"
    );
    assert_eq!(
        rounding.ingest_scale_by_ccy.get(&Currency::USD),
        Some(&4),
        "Ingest scale for USD should reflect configured rounding"
    );
}

#[test]
fn explain_uses_stamped_rounding_context() {
    // Build attribution with explicit rounding context and ensure explain() runs without
    // falling back to default rounding.
    let as_of_t0 = date!(2025 - 01 - 01);
    let as_of_t1 = date!(2025 - 01 - 02);
    let rounding = finstack_quant_core::config::rounding_context_from(&FinstackConfig::default());

    let mut attr = PnlAttribution::new(
        Money::new(1000.0, Currency::USD),
        "EXPLAIN",
        as_of_t0,
        as_of_t1,
        AttributionMethod::Parallel,
    );
    attr.meta.rounding = rounding;

    // Set non-zero components to exercise formatting paths
    attr.carry = Money::new(10.0, Currency::USD);
    attr.fx_pnl = Money::new(5.0, Currency::USD);
    attr.compute_residual()
        .expect("Residual computation should succeed");

    let explanation = attr.explain();
    assert!(
        explanation.contains("Total P&L"),
        "Explain output should be produced using the stamped rounding context"
    );
}

/// Quant review minor (tests-10): the rounding-context stamp is a workspace
/// invariant for ALL result envelopes, so pin it for every method, not just
/// Parallel.
#[test]
fn all_methods_stamp_configured_rounding_context() {
    use finstack_quant_attribution::{
        attribute_pnl_metrics_based, attribute_pnl_taylor, attribute_pnl_waterfall,
        default_waterfall_order, TaylorAttributionConfig,
    };
    use finstack_quant_valuations::instruments::PricingOptions;

    let as_of_t0 = date!(2025 - 01 - 01);
    let as_of_t1 = date!(2025 - 01 - 02);
    let instrument: Arc<dyn Instrument> = Arc::new(TestInstrument::new(
        "TEST-ROUND-ALL",
        Money::new(1_000.0, Currency::USD),
    ));
    let market = MarketContext::new();

    let mut config = FinstackConfig::default();
    config
        .rounding
        .output_scale
        .overrides
        .insert(Currency::USD, 4);
    config
        .rounding
        .ingest_scale
        .overrides
        .insert(Currency::USD, 4);

    let assert_stamp = |attr: &PnlAttribution, method: &str| {
        assert_eq!(
            attr.meta.rounding.output_scale_by_ccy.get(&Currency::USD),
            Some(&4),
            "{method}: output scale must reflect the configured rounding"
        );
        assert_eq!(
            attr.meta.rounding.ingest_scale_by_ccy.get(&Currency::USD),
            Some(&4),
            "{method}: ingest scale must reflect the configured rounding"
        );
    };

    let waterfall = attribute_pnl_waterfall(
        &instrument,
        &market,
        &market,
        as_of_t0,
        as_of_t1,
        &config,
        default_waterfall_order(),
        false,
        None,
    )
    .expect("waterfall attribution should succeed");
    assert_stamp(&waterfall, "waterfall");

    // Taylor and metrics-based take no FinstackConfig: they stamp the
    // DEFAULT rounding context. Pin that a context is stamped (the default
    // has no per-ccy overrides) so a dropped stamp regresses loudly.
    let taylor = attribute_pnl_taylor(
        &instrument,
        &market,
        &market,
        as_of_t0,
        as_of_t1,
        &TaylorAttributionConfig::default(),
        ExecutionPolicy::Serial,
    )
    .expect("taylor attribution should succeed");
    let default_rounding =
        finstack_quant_core::config::rounding_context_from(&FinstackConfig::default());
    assert_eq!(
        taylor.meta.rounding.output_scale_by_ccy, default_rounding.output_scale_by_ccy,
        "taylor must stamp the default rounding context"
    );

    let val_t0 = instrument
        .price_with_metrics(&market, as_of_t0, &[], PricingOptions::default())
        .expect("t0 valuation");
    let val_t1 = instrument
        .price_with_metrics(&market, as_of_t1, &[], PricingOptions::default())
        .expect("t1 valuation");
    let metrics_based = attribute_pnl_metrics_based(
        &instrument,
        &market,
        &market,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("metrics-based attribution should succeed");
    assert_eq!(
        metrics_based.meta.rounding.output_scale_by_ccy, default_rounding.output_scale_by_ccy,
        "metrics-based must stamp the default rounding context"
    );
}
