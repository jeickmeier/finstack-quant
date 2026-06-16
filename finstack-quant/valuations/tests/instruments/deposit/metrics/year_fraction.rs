//! Year fraction metric tests.

use crate::deposit::common::*;
use finstack_quant_core::dates::DayCount;
use finstack_quant_valuations::metrics::MetricId;

#[test]
fn test_yf_act360_6_months() {
    // Setup
    let base = date(2025, 1, 1);
    let dep = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .day_count(DayCount::Act360)
        .build();
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    // Execute
    let yf = compute_metric(&dep, &ctx, base, MetricId::Yf);

    // Validate - approximately 0.5 years for 6 months
    assert!(yf > 0.48 && yf < 0.52, "YF: {}", yf);
}

#[test]
fn test_yf_act365_one_year() {
    // Setup
    let base = date(2025, 1, 1);
    let dep = DepositBuilder::new(base)
        .maturity(date(2026, 1, 1))
        .day_count(DayCount::Act365F)
        .build();
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    // Execute
    let yf = compute_metric(&dep, &ctx, base, MetricId::Yf);

    // Validate - should be close to 1.0 for full year
    assert!((yf - 1.0).abs() < 0.01, "YF: {}", yf);
}

#[test]
fn test_yf_thirty360() {
    // Setup
    let base = date(2025, 1, 1);
    let dep = DepositBuilder::new(base)
        .start_date(date(2025, 1, 1))
        .maturity(date(2025, 7, 1))
        .day_count(DayCount::Thirty360)
        .build();
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    // Execute
    let yf = compute_metric(&dep, &ctx, base, MetricId::Yf);

    // Validate - 30/360 gives exactly 0.5 for Jan 1 to Jul 1
    assert!((yf - 0.5).abs() < 1e-10, "YF: {}", yf);
}

#[test]
fn test_yf_different_conventions_give_different_results() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep_360 = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .day_count(DayCount::Act360)
        .build();

    let dep_365 = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .day_count(DayCount::Act365F)
        .build();

    // Execute
    let yf_360 = compute_metric(&dep_360, &ctx, base, MetricId::Yf);
    let yf_365 = compute_metric(&dep_365, &ctx, base, MetricId::Yf);

    // Validate
    assert_ne!(yf_360, yf_365);
    assert!(yf_360 > yf_365); // Act/360 > Act/365 for same period
}

#[test]
fn test_yf_scales_with_period_length() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep_3m = DepositBuilder::new(base).maturity(date(2025, 4, 1)).build();

    let dep_6m = DepositBuilder::new(base).maturity(date(2025, 7, 1)).build();

    // Execute
    let yf_3m = compute_metric(&dep_3m, &ctx, base, MetricId::Yf);
    let yf_6m = compute_metric(&dep_6m, &ctx, base, MetricId::Yf);

    // Validate
    assert!(yf_6m > yf_3m);
    assert!((yf_6m / yf_3m - 2.0).abs() < 0.1); // Approximately 2x
}

#[test]
fn test_yf_zero_period() {
    // Setup - same start and end date is invalid
    let base = date(2025, 1, 1);
    let result = finstack_quant_valuations::instruments::Deposit::builder()
        .id(finstack_quant_core::types::InstrumentId::new("DEP-ZERO-YF"))
        .notional(finstack_quant_core::money::Money::new(
            1_000_000.0,
            finstack_quant_core::currency::Currency::USD,
        ))
        .start_date(base)
        .maturity(base)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
        .build();

    assert!(
        result.is_err(),
        "Zero period deposit should fail construction"
    );
}
