//! Tests for generic pricing infrastructure.

// Placeholder for generic pricing tests
// These would test the GenericInstrumentPricer and curve dependencies

use crate::common::test_helpers::standard_market;

#[test]
fn test_standard_market_available_for_pricing() {
    // Basic sanity check: the shared market fixture exposes required discount curves
    let market = standard_market();
    assert!(market.get_discount("USD-OIS").is_ok());
}
