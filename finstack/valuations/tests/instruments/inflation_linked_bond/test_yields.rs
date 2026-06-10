//! Real yield and breakeven inflation tests for Inflation-Linked Bonds
//!
//! Tests cover:
//! - Real yield calculation from clean price
//! - Yield-price relationship
//! - Breakeven inflation calculation
//! - Fisher equation approximation
//! - Error handling for invalid inputs

use super::common::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

fn running_under_coverage() -> bool {
    // `cargo llvm-cov` runs tests with LLVM coverage instrumentation enabled, which can slow down
    // execution significantly and make time-based assertions flaky.
    std::env::var_os("LLVM_PROFILE_FILE").is_some() || std::env::var_os("CARGO_LLVM_COV").is_some()
}

#[test]
fn test_real_yield_at_par() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.real_coupon = Decimal::try_from(0.02).expect("valid decimal"); // 2% coupon
    ilb.issue_date = d(2025, 1, 2);
    ilb.maturity = d(2030, 1, 2);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);
    let clean_price = 100.0; // At par

    // Act
    let y = ilb.real_yield(clean_price, &ctx, as_of).unwrap();

    // Assert - yield should be positive and reasonable
    // Note: "at par" for ILBs depends on inflation adjustments,
    // so yield may differ from coupon even at price=100
    assert!(y > 0.0);
    assert!(y < 0.15); // Reasonable upper bound
}

#[test]
fn test_real_yield_act_act_isma_example() {
    // The canonical TIPS example defaults to ACT/ACT (ICMA); `real_yield` must
    // not error with MissingFrequencyForActActIsma (regression: YTM path used
    // a default DayCountContext without the coupon frequency).
    use finstack_core::dates::DayCount;
    use finstack_valuations::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond;

    let ilb = InflationLinkedBond::example();
    assert_eq!(ilb.day_count, DayCount::ActActIsma);

    let (ctx, _) = market_context_with_index();
    let as_of = ilb.issue_date;
    let y = ilb
        .real_yield(100.0, &ctx, as_of)
        .expect("real_yield must succeed for ActActIsma TIPS");
    assert!(y > -0.05 && y < 0.15, "unreasonable real yield {y}");
}

#[test]
fn test_real_yield_premium_bond() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.real_coupon = Decimal::try_from(0.02).expect("valid decimal");
    ilb.issue_date = d(2020, 1, 2); // Issue in the past
    ilb.maturity = d(2030, 1, 2);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);
    let clean_price = 110.0; // Premium

    // Act
    let y = ilb.real_yield(clean_price, &ctx, as_of).unwrap();

    // Assert - yield should be positive and reasonable
    // Premium pricing for ILBs is complex due to inflation adjustments
    // For a premium bond (price > 100), yield should be less than coupon
    assert!(
        y < ilb.real_coupon.to_f64().unwrap(),
        "Premium bond should have yield < coupon, got yield={}, coupon={}",
        y,
        ilb.real_coupon
    );
    assert!(y > -0.05, "Yield should be reasonable, got {}", y);
    assert!(y < 0.15);
}

#[test]
fn test_real_yield_discount_bond() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.real_coupon = Decimal::try_from(0.02).expect("valid decimal");
    ilb.issue_date = d(2025, 1, 2);
    ilb.maturity = d(2030, 1, 2);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);
    let clean_price = 90.0; // Discount

    // Act
    let y = ilb.real_yield(clean_price, &ctx, as_of).unwrap();

    // Assert - discount bond → yield > coupon
    assert!(y > ilb.real_coupon.to_f64().unwrap());
}

#[test]
fn test_real_yield_price_relationship() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.real_coupon = Decimal::try_from(0.02).expect("valid decimal");
    ilb.issue_date = d(2025, 1, 2);
    ilb.maturity = d(2030, 1, 2);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act - calculate yields at different prices
    let prices = [80.0, 90.0, 100.0, 110.0, 120.0];
    let mut yields = Vec::new();
    for &price in &prices {
        let y = ilb.real_yield(price, &ctx, as_of).unwrap();
        yields.push(y);
    }

    // Assert - as price increases, yield decreases (inverse relationship)
    for i in 1..yields.len() {
        assert!(
            yields[i] < yields[i - 1],
            "yield should decrease as price increases"
        );
    }
}

#[test]
fn test_real_yield_uses_quoted_price_when_available() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.quoted_clean = Some(105.0);
    ilb.real_coupon = Decimal::try_from(0.02).expect("valid decimal");
    ilb.issue_date = d(2025, 1, 2);
    ilb.maturity = d(2030, 1, 2);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act - calculate yield using explicit price vs quoted price
    let y_explicit_street = ilb.real_yield(105.0, &ctx, as_of).unwrap();

    // Breakeven uses quoted_clean internally. `breakeven_inflation` now converts
    // the Street-compounded real yield to annual before applying the Fisher identity,
    // so the roundtrip must also compare in annual compounding.
    //
    // Convert Street real yield to annual for the consistency check:
    //   annual = (1 + street_semi/2)^2 - 1  (for semi-annual ILB, f=2)
    // We use the time-to-maturity for the Compounding::convert_rate path via:
    //   real_annual = (1 + y/2)^2 - 1  (exact for any t since it goes through the DF).
    let f = 2.0_f64; // semi-annual ILB
    let y_explicit_annual = (1.0 + y_explicit_street / f).powf(f) - 1.0;

    // The nominal yield passed to breakeven_inflation is expected in annual convention.
    let nominal_yield = 0.03_f64; // already annual
    let be = ilb.breakeven_inflation(nominal_yield, &ctx, as_of).unwrap();
    // Invert the annual Fisher identity: real_annual = (1 + nominal) / (1 + be) - 1
    let y_from_breakeven_annual = (1.0 + nominal_yield) / (1.0 + be) - 1.0;

    // Assert - both in annual compounding, should be consistent
    assert_approx_eq(
        y_explicit_annual,
        y_from_breakeven_annual,
        0.001,
        "yield consistency with quoted price",
    );
}

#[test]
fn test_real_yield_rejects_negative_price() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act & Assert
    let result = ilb.real_yield(-10.0, &ctx, as_of);
    assert!(result.is_err());
}

#[test]
fn test_real_yield_rejects_zero_price() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act & Assert
    let result = ilb.real_yield(0.0, &ctx, as_of);
    assert!(result.is_err());
}

#[test]
fn test_real_yield_rejects_infinite_price() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act & Assert
    let result = ilb.real_yield(f64::INFINITY, &ctx, as_of);
    assert!(result.is_err());
}

#[test]
fn test_real_yield_rejects_nan_price() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act & Assert
    let result = ilb.real_yield(f64::NAN, &ctx, as_of);
    assert!(result.is_err());
}

#[test]
fn test_real_yield_extreme_prices_produce_valid_results() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.real_coupon = Decimal::try_from(0.02).expect("valid decimal");
    ilb.issue_date = d(2025, 1, 2);
    ilb.maturity = d(2030, 1, 2);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act - extreme prices
    // Very high price → very low/negative yield
    let y_high_price = ilb.real_yield(200.0, &ctx, as_of).unwrap();
    // Very low price → very high yield
    let y_low_price = ilb.real_yield(10.0, &ctx, as_of).unwrap();

    // Assert - yields should be finite (solver converged) and follow inverse relationship
    assert!(
        y_high_price.is_finite(),
        "High price yield should be finite"
    );
    assert!(y_low_price.is_finite(), "Low price yield should be finite");
    assert!(
        y_high_price < y_low_price,
        "Higher price should give lower yield"
    );
}

#[test]
fn test_breakeven_inflation_basic() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.quoted_clean = Some(100.0);
    ilb.real_coupon = Decimal::try_from(0.01).expect("valid decimal"); // 1% real yield at par
    ilb.issue_date = d(2025, 1, 2);
    ilb.maturity = d(2030, 1, 2);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);
    let nominal_yield = 0.03; // 3% nominal yield

    // Act
    let breakeven = ilb.breakeven_inflation(nominal_yield, &ctx, as_of).unwrap();

    // Assert - Fisher approximation: breakeven ≈ nominal - real
    // Breakeven should be finite and reasonable
    assert!(breakeven.is_finite());
    assert!(breakeven > -0.05 && breakeven < 0.10); // Reasonable range
}

#[test]
fn test_breakeven_inflation_fisher_equation() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.quoted_clean = Some(100.0);
    ilb.real_coupon = Decimal::try_from(0.015).expect("valid decimal");
    ilb.issue_date = d(2025, 1, 2);
    ilb.maturity = d(2030, 1, 2);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    let real_yield_street = ilb.real_yield(100.0, &ctx, as_of).unwrap();
    // `breakeven_inflation` converts the Street-compounded real yield to annual
    // before applying Fisher identity. Convert here for the expected value.
    // sample_tips() uses semi-annual frequency, so f = 2.
    let f = 2.0_f64;
    let real_yield_annual = (1.0 + real_yield_street / f).powf(f) - 1.0;
    let nominal_yield = 0.04; // 4% annual (what we pass to breakeven_inflation)

    // Act
    let breakeven = ilb.breakeven_inflation(nominal_yield, &ctx, as_of).unwrap();

    // Assert - Exact Fisher equation applied in annual compounding:
    // (1 + nominal_annual) = (1 + real_annual) × (1 + breakeven)
    // So: breakeven = (1 + nominal_annual) / (1 + real_annual) - 1
    let expected_breakeven = (1.0 + nominal_yield) / (1.0 + real_yield_annual) - 1.0;
    assert_approx_eq(
        breakeven,
        expected_breakeven,
        0.0001,
        "Exact Fisher equation",
    );
}

#[test]
fn test_breakeven_inflation_varies_with_nominal_yield() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.quoted_clean = Some(100.0);
    ilb.real_coupon = Decimal::try_from(0.01).expect("valid decimal");
    ilb.issue_date = d(2025, 1, 2);
    ilb.maturity = d(2030, 1, 2);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    // Act
    let be_low = ilb.breakeven_inflation(0.02, &ctx, as_of).unwrap();
    let be_mid = ilb.breakeven_inflation(0.03, &ctx, as_of).unwrap();
    let be_high = ilb.breakeven_inflation(0.04, &ctx, as_of).unwrap();

    // Assert - higher nominal yield → higher breakeven (real stays constant)
    assert!(be_mid > be_low);
    assert!(be_high > be_mid);
}

#[test]
fn test_breakeven_inflation_uses_quoted_clean_default() {
    // Arrange
    let mut ilb1 = sample_tips();
    let mut ilb2 = sample_tips();

    ilb1.quoted_clean = Some(100.0);
    ilb2.quoted_clean = None; // No quoted price → should error

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);
    let nominal_yield = 0.03;

    // Act - with quoted_clean set, breakeven should succeed
    let be1 = ilb1
        .breakeven_inflation(nominal_yield, &ctx, as_of)
        .unwrap();
    assert!(be1.is_finite(), "breakeven should be a finite number");

    // Act - without quoted_clean, breakeven should return a validation error
    let err = ilb2
        .breakeven_inflation(nominal_yield, &ctx, as_of)
        .expect_err("should require quoted_clean");
    assert!(
        err.to_string().contains("quoted clean price"),
        "error should mention quoted clean price, got: {err}"
    );
}

#[test]
fn test_breakeven_can_be_negative() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.quoted_clean = Some(100.0);
    ilb.real_coupon = Decimal::try_from(0.05).expect("valid decimal"); // High real coupon
    ilb.issue_date = d(2025, 1, 2);
    ilb.maturity = d(2030, 1, 2);

    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);
    let nominal_yield = 0.03; // Low nominal yield

    // Act
    let breakeven = ilb.breakeven_inflation(nominal_yield, &ctx, as_of).unwrap();

    // Assert - if real > nominal, breakeven is negative (rare but possible)
    // Real ≈ 5% at par, nominal = 3% → breakeven ≈ -2%
    assert!(breakeven < 0.0);
}

#[test]
fn test_real_yield_varies_with_time_to_maturity() {
    // Arrange
    let (ctx, _) = market_context_with_index();

    // Long-dated bond
    let mut ilb_long = sample_tips();
    ilb_long.issue_date = d(2025, 1, 2);
    ilb_long.maturity = d(2035, 1, 2); // 10 years
    ilb_long.real_coupon = Decimal::try_from(0.02).expect("valid decimal");

    // Short-dated bond
    let mut ilb_short = sample_tips();
    ilb_short.issue_date = d(2025, 1, 2);
    ilb_short.maturity = d(2027, 1, 2); // 2 years
    ilb_short.real_coupon = Decimal::try_from(0.02).expect("valid decimal");

    let as_of = d(2025, 1, 2);
    let clean_price = 100.0;

    // Act
    let y_long = ilb_long.real_yield(clean_price, &ctx, as_of).unwrap();
    let y_short = ilb_short.real_yield(clean_price, &ctx, as_of).unwrap();

    // Assert - both yields should be positive and reasonable
    assert!(y_long > 0.0 && y_long < 0.15);
    assert!(y_short > 0.0 && y_short < 0.15);
}

#[test]
fn test_real_yield_different_day_counts() {
    // Arrange
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);
    let clean_price = 100.0;

    // Same bond with different day count conventions
    let mut ilb_actact = sample_tips();
    ilb_actact.day_count = finstack_core::dates::DayCount::ActAct;
    ilb_actact.real_coupon = Decimal::try_from(0.02).expect("valid decimal");
    ilb_actact.issue_date = d(2025, 1, 2);
    ilb_actact.maturity = d(2030, 1, 2);

    let mut ilb_30360 = sample_tips();
    ilb_30360.day_count = finstack_core::dates::DayCount::Thirty360;
    ilb_30360.real_coupon = Decimal::try_from(0.02).expect("valid decimal");
    ilb_30360.issue_date = d(2025, 1, 2);
    ilb_30360.maturity = d(2030, 1, 2);

    // Act
    let y_actact = ilb_actact.real_yield(clean_price, &ctx, as_of).unwrap();
    let y_30360 = ilb_30360.real_yield(clean_price, &ctx, as_of).unwrap();

    // Assert - different day counts → slightly different yields
    assert!(y_actact > 0.0);
    assert!(y_30360 > 0.0);
    // Should be close but not exactly equal
    assert!(relative_diff(y_actact, y_30360) < 0.01); // Within 1%
}

#[test]
fn test_real_yield_uk_gilt() {
    // Arrange
    let mut ilb = sample_uk_linker();
    ilb.quoted_clean = Some(105.0);

    let (ctx, _) = uk_market_context();
    let as_of = d(2025, 1, 2);

    // Act
    let y = ilb.real_yield(105.0, &ctx, as_of).unwrap();

    // Assert - yield should be positive and reasonable
    assert!(y > 0.0);
    assert!(y < 0.15);
}

#[test]
fn test_yield_calculation_performance() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, _) = market_context_with_index();
    let as_of = d(2025, 1, 2);

    if running_under_coverage() {
        // Coverage builds are expected to be slower; this test is intended to catch performance
        // regressions in normal, non-instrumented test runs.
        return;
    }

    // Act
    let start = std::time::Instant::now();
    for _ in 0..100 {
        let _ = ilb.real_yield(100.0, &ctx, as_of).unwrap();
    }
    let elapsed = start.elapsed();

    // Assert - 100 yield calculations should be fast (< 500ms)
    assert!(elapsed.as_millis() < 500);
}
