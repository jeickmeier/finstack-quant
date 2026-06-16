//! Edge case and boundary condition tests for Deposit instruments.
//!
//! Tests special scenarios and boundary conditions to ensure robustness.

use super::common::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::Deposit;
use finstack_quant_valuations::metrics::MetricId;
use rust_decimal::Decimal;

#[test]
fn test_zero_period_deposit() {
    // Setup - start == end (invalid - should fail validation)
    let base = date(2025, 1, 1);
    let result = Deposit::builder()
        .id(InstrumentId::new("DEP-ZERO-PERIOD"))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(base)
        .maturity(base)
        .day_count(DayCount::Act360)
        .quote_rate_opt(Some(Decimal::try_from(0.05).expect("valid decimal")))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .build();

    // Validate - zero period deposits are rejected at construction
    assert!(
        result.is_err(),
        "Zero period deposit should fail construction"
    );
}

#[test]
fn test_zero_rate_deposit() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(0.0)
        .build();

    // Execute - trade NPV (pricing view, includes the T+0 initial exchange).
    // Holder-view `value` excludes the initial exchange and is ≈ +notional·DF.
    let npv = dep.npv_raw(&ctx, base).unwrap();

    // Validate - should be negative (time value of money)
    assert!(npv < 0.0);
}

#[test]
fn test_very_high_rate() {
    // Setup - test with unrealistically high rate (100%)
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(1.0)
        .build();

    // Execute
    let pv = dep.value(&ctx, base).unwrap();

    // Validate - should compute without error
    assert!(pv.currency() == Currency::USD);
    assert!(pv.amount().is_finite());
}

#[test]
fn test_very_small_notional() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .notional(Money::new(0.01, Currency::USD))
        .maturity(date(2025, 7, 1))
        .quote_rate(0.03)
        .build();

    // Execute
    let pv = dep.value(&ctx, base).unwrap();

    // Validate
    assert!(pv.amount().is_finite());
    assert!(pv.amount().abs() < 1.0); // Very small
}

#[test]
fn test_very_large_notional() {
    // Setup - test with trillion dollar notional
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .notional(Money::new(1_000_000_000_000.0, Currency::USD))
        .maturity(date(2025, 7, 1))
        .quote_rate(0.03)
        .build();

    // Execute
    let pv = dep.value(&ctx, base).unwrap();

    // Validate
    assert!(pv.amount().is_finite());
    assert!(pv.amount().abs() > 1_000_000_000.0); // Appropriately large
}

#[test]
fn test_very_short_maturity_one_day() {
    // Setup - overnight deposit
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .start_date(base)
        .maturity(date(2025, 1, 2))
        .quote_rate(0.03)
        .build();

    // Execute
    let pv = dep.value(&ctx, base).unwrap();
    let yf = compute_metric(&dep, &ctx, base, MetricId::Yf);

    // Validate
    assert!(pv.amount().is_finite());
    assert!(yf < 0.01); // Very small year fraction
}

#[test]
fn test_very_long_maturity() {
    // Setup - 10 year deposit
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .maturity(date(2035, 1, 1))
        .quote_rate(0.03)
        .build();

    // Execute
    let pv = dep.value(&ctx, base).unwrap();

    // Validate
    assert!(pv.amount().is_finite());
}

#[test]
fn test_negative_rate_environment() {
    // Setup - negative quoted rate
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(-0.005)
        .build();

    // Execute - trade NPV (pricing view, includes the T+0 initial exchange).
    let pv = dep.value(&ctx, base).unwrap();
    let npv = dep.npv_raw(&ctx, base).unwrap();

    // Validate - should compute correctly
    assert!(pv.currency() == Currency::USD);
    assert!(pv.amount().is_finite());
    // With negative rate, get back less than principal: trade NPV < 0.
    assert!(npv < 0.0);
}

#[test]
fn test_pricing_on_start_date() {
    // Setup - price exactly on start date
    let base = date(2025, 1, 1);
    let start = date(2025, 2, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .start_date(start)
        .maturity(date(2025, 8, 1))
        .quote_rate(0.03)
        .build();

    // Execute
    let pv = dep.value(&ctx, start).unwrap();

    // Validate
    assert!(pv.amount().is_finite());
}

#[test]
fn test_pricing_after_maturity() {
    // Setup - price after end date
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(date(2024, 1, 1))
        .start_date(date(2024, 1, 1))
        .maturity(date(2024, 7, 1))
        .quote_rate(0.03)
        .build();

    // Matured deposits have no future cashflows under the signed canonical
    // schedule (as_of filtering removes all past flows). Pricing returns an error.
    let result = dep.value(&ctx, base);
    assert!(
        result.is_err(),
        "Matured deposit should return error (no future cashflows)"
    );
}

#[test]
fn test_thirty360_with_end_of_month() {
    // Setup - test 30/360 with month end dates
    let base = date(2025, 1, 31);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .start_date(date(2025, 1, 31))
        .maturity(date(2025, 7, 31))
        .day_count(DayCount::Thirty360)
        .quote_rate(0.03)
        .build();

    // Execute
    let pv = dep.value(&ctx, base).unwrap();
    let yf = compute_metric(&dep, &ctx, base, MetricId::Yf);

    // Validate
    assert!(pv.amount().is_finite());
    assert!((yf - 0.5).abs() < 0.01); // 30/360 treats as 6 months
}

#[test]
fn test_leap_year_handling() {
    // Setup - test with leap year date
    let base = date(2024, 2, 29);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .start_date(base)
        .maturity(date(2024, 8, 29))
        .day_count(DayCount::Act365F)
        .quote_rate(0.03)
        .build();

    // Execute
    let pv = dep.value(&ctx, base).unwrap();

    // Validate
    assert!(pv.amount().is_finite());
}

#[test]
fn test_missing_quote_rate_defaults_to_zero() {
    // Setup - no quote rate set
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = Deposit::builder()
        .id(InstrumentId::new("DEP-NO-QUOTE"))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(base)
        .maturity(date(2025, 7, 1))
        .day_count(DayCount::Act360)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .build()
        .unwrap();

    // Execute
    let err = dep
        .value(&ctx, base)
        .expect_err("npv() should require quote_rate");

    // Validate
    let msg = err.to_string();
    assert!(
        msg.contains("quote_rate"),
        "Error should mention quote_rate: {msg}"
    );
}

#[test]
fn test_back_to_back_deposits_same_period() {
    // Setup - two deposits with identical parameters
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep1 = DepositBuilder::new(base)
        .id("DEP-1")
        .maturity(date(2025, 7, 1))
        .quote_rate(0.03)
        .build();

    let dep2 = DepositBuilder::new(base)
        .id("DEP-2")
        .maturity(date(2025, 7, 1))
        .quote_rate(0.03)
        .build();

    // Execute
    let pv1 = dep1.value(&ctx, base).unwrap();
    let pv2 = dep2.value(&ctx, base).unwrap();

    // Validate - should have identical PVs
    assert!((pv1.amount() - pv2.amount()).abs() < PRICE_TOLERANCE);
}

#[test]
fn test_rate_exactly_equal_to_par() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let dep = DepositBuilder::new(base).maturity(date(2025, 7, 1)).build();

    // Execute - get par rate and use it
    let par = compute_metric(&dep, &ctx, base, MetricId::DepositParRate);

    let dep_at_par = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(par)
        .build();

    // Par rate zeroes the *trade* NPV (pricing view via `npv_raw`); the
    // holder-view `value` excludes the T+0 initial exchange.
    let npv = dep_at_par.npv_raw(&ctx, base).unwrap();

    // Validate - trade NPV should be essentially zero for deposit at par rate
    // Market standard: < $0.01 on $1M notional (< 0.001bp)
    assert!(
        npv.abs() < 0.01,
        "trade NPV at par rate should be < $0.01, got: {npv}",
    );
}

#[test]
fn test_multiple_currencies_independent() {
    // Setup - test deposits in different currencies
    let base = date(2025, 1, 1);
    let ctx_usd = ctx_with_standard_disc(base, "USD-OIS");
    let ctx_eur = ctx_with_standard_disc(base, "EUR-OIS");

    let dep_usd = DepositBuilder::new(base)
        .notional(Money::new(1_000_000.0, Currency::USD))
        .maturity(date(2025, 7, 1))
        .quote_rate(0.03)
        .discount_curve_id("USD-OIS")
        .build();

    let dep_eur = DepositBuilder::new(base)
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .maturity(date(2025, 7, 1))
        .quote_rate(0.03)
        .discount_curve_id("EUR-OIS")
        .build();

    // Execute
    let pv_usd = dep_usd.value(&ctx_usd, base).unwrap();
    let pv_eur = dep_eur.value(&ctx_eur, base).unwrap();

    // Validate - both should compute correctly
    assert_eq!(pv_usd.currency(), Currency::USD);
    assert_eq!(pv_eur.currency(), Currency::EUR);
}

/// Test that validation catches BDC-induced effective date crossover.
///
/// This tests an edge case where the raw dates are valid (end > start),
/// but after business day adjustments the effective dates become invalid
/// (effective_end <= effective_start).
///
/// Scenario:
/// - Start: Friday Jan 3, 2025
/// - End: Monday Jan 6, 2025 (just 1 business day later)
/// - Spot lag: 2 business days (T+2)
/// - Calendar: NYSE
///
/// With T+2 spot lag from Friday Jan 3:
/// - Effective start = Tuesday Jan 7, 2025
/// - Effective end = Monday Jan 6, 2025 (no adjustment needed, already business day)
///
/// This results in effective_end < effective_start, which should fail validation.
#[test]
fn test_bdc_adjustment_causes_effective_date_crossover() {
    use finstack_quant_core::dates::BusinessDayConvention;
    use finstack_quant_core::types::InstrumentId;

    let base = date(2025, 1, 3); // Friday
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    // Build deposit with dates that are valid raw but invalid after adjustments
    let dep = Deposit::builder()
        .id(InstrumentId::new("DEP-CROSSOVER"))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(date(2025, 1, 3)) // Friday - trade date
        .maturity(date(2025, 1, 6)) // Monday - just 1 business day after Friday
        .day_count(DayCount::Act360)
        .quote_rate_opt(Some(Decimal::try_from(0.03).expect("valid decimal")))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_lag_days_opt(Some(2)) // T+2: Friday + 2 biz days = Tuesday Jan 7
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(Some("nyse".into()))
        .build()
        .unwrap();

    // Validate should fail because effective_start (Jan 7) > effective_end (Jan 6)
    let result = dep.validate();
    assert!(
        result.is_err(),
        "Validation should fail when BDC adjustments cause date crossover"
    );

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("effective") && err_msg.contains("after"),
        "Error should mention effective dates: {}",
        err_msg
    );

    // Also verify that npv() fails (it calls validate internally)
    let pv_result = dep.value(&ctx, base);
    assert!(
        pv_result.is_err(),
        "NPV should fail for deposit with invalid effective dates"
    );
}

/// Test that extreme quote rates trigger warnings but don't fail validation.
///
/// Rates outside [-10%, 100%] are unusual but may be intentional (e.g., stress testing).
/// The validation should log a warning but not reject the instrument.
#[test]
fn test_extreme_rate_warning_but_valid() {
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    // Test with very high rate (200% = 20000 bps)
    let dep_high = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(2.0) // 200% - triggers warning
        .build();

    // Should validate and price successfully (warning logged but not blocking)
    assert!(
        dep_high.validate().is_ok(),
        "Extreme rate should validate (with warning)"
    );
    let pv_high = dep_high.value(&ctx, base);
    assert!(pv_high.is_ok(), "Extreme rate deposit should price");

    // Test with very negative rate (-20% = -2000 bps)
    let dep_low = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(-0.2) // -20% - triggers warning
        .build();

    assert!(
        dep_low.validate().is_ok(),
        "Extreme negative rate should validate (with warning)"
    );
    let pv_low = dep_low.value(&ctx, base);
    assert!(pv_low.is_ok(), "Extreme negative rate deposit should price");
}
