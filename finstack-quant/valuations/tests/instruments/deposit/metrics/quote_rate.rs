//! Quote rate metric tests.

use crate::deposit::common::*;
use finstack_quant_valuations::metrics::MetricId;

#[test]
fn test_quote_rate_returns_set_rate() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let dep = DepositBuilder::new(base).quote_rate(0.0375).build();

    // Execute
    let quote = compute_metric(&dep, &ctx, base, MetricId::QuoteRate);

    // Validate
    assert!((quote - 0.0375).abs() < RATE_TOLERANCE);
}

#[test]
fn test_quote_rate_with_zero() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let dep = DepositBuilder::new(base).quote_rate(0.0).build();

    // Execute
    let quote = compute_metric(&dep, &ctx, base, MetricId::QuoteRate);

    // Validate
    assert!(quote.abs() < RATE_TOLERANCE);
}

#[test]
fn test_quote_rate_with_negative_rate() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let dep = DepositBuilder::new(base).quote_rate(-0.005).build();

    // Execute
    let quote = compute_metric(&dep, &ctx, base, MetricId::QuoteRate);

    // Validate
    assert!((quote + 0.005).abs() < RATE_TOLERANCE);
}

#[test]
fn test_quote_rate_with_high_rate() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let dep = DepositBuilder::new(base).quote_rate(0.15).build();

    // Execute
    let quote = compute_metric(&dep, &ctx, base, MetricId::QuoteRate);

    // Validate
    assert!((quote - 0.15).abs() < RATE_TOLERANCE);
}
