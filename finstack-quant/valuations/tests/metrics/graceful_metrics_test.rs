//! Tests for graceful metric calculation error handling.
//!
//! Verifies strict metric calculation behavior.
//!
//! - In strict mode (default), unknown/missing metrics return errors.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Error;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::{MetricContext, MetricId, MetricRegistry};
use std::sync::Arc;
use time::macros::date;

#[test]
fn test_missing_metric_errors_in_strict_mode() {
    // Create a simple bond
    let bond = Bond::fixed(
        "TEST_BOND",
        Money::new(1000.0, Currency::USD),
        0.05, // 5% coupon
        date!(2024 - 01 - 01),
        date!(2025 - 01 - 01),
        "USD_OIS",
    )
    .unwrap();

    // Create a market context (empty for this test)
    let market = MarketContext::new();

    let as_of = date!(2024 - 06 - 01);
    let base_value = Money::new(1000.0, Currency::USD);

    // Create context
    let instrument_arc: Arc<dyn Instrument> = Arc::new(bond);
    let mut context = MetricContext::new(
        instrument_arc,
        Arc::new(market),
        as_of,
        base_value,
        MetricContext::default_config(),
    );

    // Create empty registry
    let registry = MetricRegistry::new();

    // Try to compute a metric that doesn't exist (e.g., Ytm)
    let metrics = vec![MetricId::Ytm];
    let result = registry.compute(&metrics, &mut context);

    match result {
        Err(Error::UnknownMetric { metric_id, .. }) => {
            assert_eq!(metric_id, "ytm");
        }
        Err(other) => panic!("unexpected error: {}", other),
        Ok(_) => panic!("expected strict mode to error on missing metric"),
    }
}
