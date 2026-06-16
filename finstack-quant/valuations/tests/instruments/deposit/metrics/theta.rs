//! Theta (time decay) metric tests.

use crate::deposit::common::*;
use finstack_quant_valuations::metrics::MetricId;

#[test]
fn test_theta_calculation_exists() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let dep = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(0.03)
        .build();

    // Execute
    let theta = compute_metric(&dep, &ctx, base, MetricId::Theta);

    // Validate - theta exists and is finite
    assert!(theta.is_finite());
}

#[test]
fn test_theta_with_zero_rate() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let dep = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(0.0)
        .build();

    // Execute
    let theta = compute_metric(&dep, &ctx, base, MetricId::Theta);

    // Validate
    assert!(theta.is_finite());
}

#[test]
fn test_theta_magnitude_reasonable() {
    // Setup - theta should be of reasonable magnitude relative to PV.
    // Use as_of one day after start so the initial negative notional is
    // already excluded from the schedule (avoids a PV discontinuity when
    // the theta bump rolls past the notional date).
    let start = date(2025, 1, 1);
    let as_of = date(2025, 1, 2);
    let ctx = ctx_with_standard_disc(start, "USD-OIS");
    let dep = DepositBuilder::new(start)
        .maturity(date(2025, 7, 1))
        .quote_rate(0.03)
        .build();

    let pv = dep.value(&ctx, as_of).unwrap();

    // Execute
    let theta = compute_metric(&dep, &ctx, as_of, MetricId::Theta);

    // Validate - theta should be materially smaller than PV in absolute terms
    assert!(
        theta.abs() < pv.amount().abs() * 0.2,
        "theta magnitude {} too large vs pv {}",
        theta,
        pv.amount()
    );
}

#[test]
fn test_theta_longer_maturity() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep_short = DepositBuilder::new(base)
        .maturity(date(2025, 4, 1))
        .quote_rate(0.03)
        .build();

    let dep_long = DepositBuilder::new(base)
        .maturity(date(2026, 1, 1))
        .quote_rate(0.03)
        .build();

    // Execute
    let theta_short = compute_metric(&dep_short, &ctx, base, MetricId::Theta);
    let theta_long = compute_metric(&dep_long, &ctx, base, MetricId::Theta);

    // Validate - both should be finite
    assert!(theta_short.is_finite());
    assert!(theta_long.is_finite());
}
