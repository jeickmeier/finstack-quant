//! Bucketed DV01 metric tests.

use crate::deposit::common::*;
use finstack_quant_valuations::metrics::MetricId;

#[test]
fn test_bucketed_dv01_calculation() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let dep = DepositBuilder::new(base).maturity(date(2025, 7, 1)).build();

    // Execute
    let bucketed_dv01 = compute_metric(&dep, &ctx, base, MetricId::BucketedDv01);

    // Validate - should be finite
    assert!(bucketed_dv01.is_finite());
}

#[test]
fn test_bucketed_dv01_scales_with_notional() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep_1m = DepositBuilder::new(base)
        .notional(finstack_quant_core::money::Money::new(
            1_000_000.0,
            finstack_quant_core::currency::Currency::USD,
        ))
        .maturity(date(2025, 7, 1))
        .build();

    let dep_2m = DepositBuilder::new(base)
        .notional(finstack_quant_core::money::Money::new(
            2_000_000.0,
            finstack_quant_core::currency::Currency::USD,
        ))
        .maturity(date(2025, 7, 1))
        .build();

    // Execute
    let bdv01_1m = compute_metric(&dep_1m, &ctx, base, MetricId::BucketedDv01);
    let bdv01_2m = compute_metric(&dep_2m, &ctx, base, MetricId::BucketedDv01);

    // Validate - should scale approximately linearly
    assert!((bdv01_2m / bdv01_1m - 2.0).abs() < 0.1);
}

#[test]
fn test_bucketed_dv01_different_maturities() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep_3m = DepositBuilder::new(base).maturity(date(2025, 4, 1)).build();

    let dep_1y = DepositBuilder::new(base).maturity(date(2026, 1, 1)).build();

    // Execute
    let bdv01_3m = compute_metric(&dep_3m, &ctx, base, MetricId::BucketedDv01);
    let bdv01_1y = compute_metric(&dep_1y, &ctx, base, MetricId::BucketedDv01);

    // Validate - both should be finite and negative (standard convention for deposits)
    assert!(bdv01_3m < 0.0);
    assert!(bdv01_1y < 0.0);
}
