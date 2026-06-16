//! DV01 (dollar value of a basis point) metric tests.

use crate::deposit::common::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::metrics::MetricId;

#[test]
fn test_dv01_negative_for_long_deposits() {
    // Setup - deposits have negative DV01 (long position: rates up → value down)
    // Standard market convention: DV01 = PV(rate+1bp) - PV(base)
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let dep = DepositBuilder::new(base).maturity(date(2025, 7, 1)).build();

    // Execute
    let dv01 = compute_metric(&dep, &ctx, base, MetricId::Dv01);

    // Validate - DV01 should be negative for long positions (standard convention)
    assert!(
        dv01 < 0.0,
        "DV01 should be negative (rates up → PV down): {}",
        dv01
    );
    // Magnitude check: for 6m deposit with $1M notional, should be around -$50
    assert!(
        dv01.abs() > 40.0 && dv01.abs() < 60.0,
        "DV01 magnitude: {}",
        dv01
    );
}

#[test]
fn test_dv01_scales_with_notional() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep_1m = DepositBuilder::new(base)
        .notional(Money::new(1_000_000.0, Currency::USD))
        .maturity(date(2025, 7, 1))
        .build();

    let dep_2m = DepositBuilder::new(base)
        .notional(Money::new(2_000_000.0, Currency::USD))
        .maturity(date(2025, 7, 1))
        .build();

    // Execute
    let dv01_1m = compute_metric(&dep_1m, &ctx, base, MetricId::Dv01);
    let dv01_2m = compute_metric(&dep_2m, &ctx, base, MetricId::Dv01);

    // Validate - should scale linearly
    assert!((dv01_2m / dv01_1m - 2.0).abs() < 0.01);
}

#[test]
fn test_dv01_increases_with_maturity() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep_3m = DepositBuilder::new(base).maturity(date(2025, 4, 1)).build();

    let dep_1y = DepositBuilder::new(base).maturity(date(2026, 1, 1)).build();

    // Execute
    let dv01_3m = compute_metric(&dep_3m, &ctx, base, MetricId::Dv01);
    let dv01_1y = compute_metric(&dep_1y, &ctx, base, MetricId::Dv01);

    // Validate - longer maturity has higher DV01 magnitude (both are negative)
    assert!(dv01_1y.abs() > dv01_3m.abs());
}

#[test]
fn test_dv01_zero_for_zero_period() {
    // Setup - zero period deposit (start == end) is invalid
    let base = date(2025, 1, 1);
    let result = finstack_quant_valuations::instruments::Deposit::builder()
        .id(finstack_quant_core::types::InstrumentId::new(
            "DEP-ZERO-DV01",
        ))
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

#[test]
fn test_dv01_reasonable_magnitude() {
    // Setup - for $1mm notional, 6m deposit, DV01 magnitude should be ~$50
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .notional(Money::new(1_000_000.0, Currency::USD))
        .maturity(date(2025, 7, 1))
        .build();

    // Execute
    let dv01 = compute_metric(&dep, &ctx, base, MetricId::Dv01);

    // Validate - rough magnitude check (about 0.5 yrs * 1M notional * 1bp = ~$50)
    assert!(dv01.abs() > 40.0 && dv01.abs() < 60.0, "DV01: {}", dv01);
}

#[test]
fn test_dv01_with_different_day_counts() {
    // Setup - need non-zero rate to see day count effect on interest accrual
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep_360 = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .quote_rate(0.05) // 5% rate so interest accrual differs
        .build();

    let dep_365 = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .quote_rate(0.05) // Same rate but different accrual due to day count
        .build();

    // Execute
    let dv01_360 = compute_metric(&dep_360, &ctx, base, MetricId::Dv01);
    let dv01_365 = compute_metric(&dep_365, &ctx, base, MetricId::Dv01);

    // Validate - different day counts give slightly different DV01
    // because the interest accrual amounts differ (Act360 has higher YF)
    // Note: Discounting uses curve's day count, but interest accrual differs
    assert_ne!(
        dv01_360, dv01_365,
        "Day count should affect DV01 via interest accrual"
    );
}
