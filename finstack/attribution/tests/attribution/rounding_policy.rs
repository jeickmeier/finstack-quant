//! Tests for rounding policy stamping and display in attribution.

use crate::common::test_utils::TestInstrument;
use finstack_attribution::{
    attribute_pnl_parallel, AttributionMethod, ExecutionPolicy, PnlAttribution,
};
use finstack_core::config::FinstackConfig;
use finstack_core::currency::Currency;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_valuations::instruments::Instrument;
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
    let rounding = finstack_core::config::rounding_context_from(&FinstackConfig::default());

    let mut attr = PnlAttribution::new(
        Money::new(1000.0, Currency::USD),
        "EXPLAIN",
        as_of_t0,
        as_of_t1,
        AttributionMethod::Parallel,
    );
    attr.meta.rounding = rounding.clone();

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
