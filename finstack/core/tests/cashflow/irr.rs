//! Tests for IRR (Internal Rate of Return) and XIRR calculations.
//!
//! These tests verify:
//! - Golden values against published financial references
//! - Edge cases and boundary conditions
//! - Numerical stability near singularities
//! - Input validation
//!
//! # References
//!
//! - Brealey, Myers, Allen - *Principles of Corporate Finance* (13th ed), Chapters 5-6
//! - CFA Institute - *Global Investment Performance Standards (GIPS)*, Appendix D
//! - Hull - *Options, Futures, and Other Derivatives* (10th ed), Chapter 4
//! - Microsoft Excel XIRR function specification

use finstack_core::cashflow::{irr, xirr, xirr_with_daycount, xirr_with_daycount_ctx};
use finstack_core::dates::{Date, DayCount, DayCountContext, Tenor};
use finstack_core::Error;
use time::Month;

// =============================================================================
// Constants and Helpers
// =============================================================================

/// Tolerance for XIRR comparisons (matches Excel precision)
const XIRR_TOLERANCE: f64 = 1e-6;

/// Helper to create dates
fn d(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).unwrap(), day).unwrap()
}

/// Compute NPV at a given rate for dated cashflows (Act/365F)
fn compute_dated_npv(flows: &[(Date, f64)], rate: f64) -> f64 {
    if flows.is_empty() {
        return 0.0;
    }
    let first_date = flows[0].0;
    let dc = DayCount::Act365F;
    let ctx = DayCountContext::default();

    flows
        .iter()
        .map(|&(date, amount)| {
            let years = dc.year_fraction(first_date, date, ctx).unwrap_or(0.0);
            amount / (1.0 + rate).powf(years)
        })
        .sum()
}

/// Compute NPV at a given rate for periodic cashflows
fn compute_periodic_npv(amounts: &[f64], rate: f64) -> f64 {
    amounts
        .iter()
        .enumerate()
        .map(|(i, &a)| a / (1.0 + rate).powi(i as i32))
        .sum()
}

// =============================================================================
// Golden Value Tests - Textbook References
// =============================================================================

/// Reference: Brealey, Myers, Allen - Principles of Corporate Finance (13th ed)
/// Chapter 5, Example 5.3 (simplified for annual IRR)
///
/// Investment pattern: -$1000 upfront, receive $400 annually for 3 years
/// IRR solves: -1000 + 400/(1+r) + 400/(1+r)^2 + 400/(1+r)^3 = 0
/// Solution: r ≈ 9.70% (from textbook)
#[test]
fn xirr_brealey_myers_three_year_annuity() {
    let flows = vec![
        (d(2024, 1, 1), -1000.0),
        (d(2025, 1, 1), 400.0),
        (d(2026, 1, 1), 400.0),
        (d(2027, 1, 1), 400.0),
    ];

    let result = xirr(&flows, None).expect("XIRR calculation should succeed");

    // Verify NPV at computed rate is approximately zero
    let npv_at_irr = compute_dated_npv(&flows, result);
    assert!(
        npv_at_irr.abs() < 0.01,
        "NPV at IRR should be ~0, got {}",
        npv_at_irr
    );

    // The textbook IRR of 9.70% is for exact annual periods
    // With Act/365F day count and 2024 leap year, expect slight variation
    // Verify result is in reasonable range
    assert!(
        result > 0.09 && result < 0.11,
        "IRR should be ~9.7%, got {:.4}%",
        result * 100.0
    );
}

/// Reference: Hull - Options, Futures, and Other Derivatives (10th ed)
/// Chapter 4, Example 4.4 (bond yield calculation pattern)
///
/// Simple 2-period return: invest $1000, receive $1100 after 1 year
/// Expected: 10% annual return adjusted for day count
#[test]
fn xirr_hull_simple_annual_return() {
    let flows = vec![(d(2025, 1, 1), -1000.0), (d(2026, 1, 1), 1100.0)];

    let result = xirr(&flows, None).expect("XIRR calculation should succeed");

    // 2025 is not a leap year, so year fraction = 365/365 = 1.0
    // Expected: (1100/1000)^(1/1) - 1 = 0.10
    let expected = 0.10;
    assert!(
        (result - expected).abs() < XIRR_TOLERANCE,
        "XIRR should be {:.6}, got {:.6}",
        expected,
        result
    );

    // Verify NPV at IRR is zero
    let npv_at_irr = compute_dated_npv(&flows, result);
    assert!(
        npv_at_irr.abs() < 0.01,
        "NPV at IRR should be ~0, got {}",
        npv_at_irr
    );
}

// =============================================================================
// Golden Value Tests - Private Equity / GIPS Style
// =============================================================================

/// Reference: CFA Institute GIPS Standards - PE/VC performance measurement
///
/// J-curve pattern typical in PE: early drawdowns, later distributions
/// Capital calls: -400K, -300K, -300K over first 18 months
/// Distributions: 200K, 400K, 800K over years 2-4
#[test]
fn xirr_gips_pe_j_curve() {
    let flows = vec![
        (d(2020, 1, 1), -400_000.0), // Initial commitment
        (d(2020, 7, 1), -300_000.0), // Second drawdown
        (d(2021, 1, 1), -300_000.0), // Third drawdown
        (d(2022, 1, 1), 200_000.0),  // First distribution
        (d(2023, 1, 1), 400_000.0),  // Second distribution
        (d(2024, 1, 1), 800_000.0),  // Exit distribution
    ];

    let result = xirr(&flows, None).expect("XIRR calculation should succeed");

    // Verify NPV at computed rate is approximately zero
    let npv_at_irr = compute_dated_npv(&flows, result);
    assert!(
        npv_at_irr.abs() < 1.0,
        "NPV at IRR should be ~0, got {}",
        npv_at_irr
    );

    // Total invested: 1M, total returned: 1.4M over ~4 years
    // Expected IRR: ~10-15% annually (typical PE)
    assert!(
        result > 0.05 && result < 0.25,
        "PE J-curve IRR should be ~10-15%, got {:.2}%",
        result * 100.0
    );
}

/// Reference: GIPS methodology for VC fund performance
///
/// High-return VC scenario: 5x MOIC over 1.5 years
/// Investment: $100K at t=0, exit: $500K at t=1.5 years
#[test]
fn xirr_gips_vc_high_return() {
    let flows = vec![
        (d(2022, 1, 1), -100_000.0),
        (d(2023, 7, 1), 500_000.0), // 1.5 years later
    ];

    let result = xirr(&flows, None).expect("XIRR calculation should succeed");

    // Verify NPV at computed rate is approximately zero
    let npv_at_irr = compute_dated_npv(&flows, result);
    assert!(
        npv_at_irr.abs() < 1.0,
        "NPV at IRR should be ~0, got {}",
        npv_at_irr
    );

    // 5x return over 1.5 years → IRR should be roughly 200-250%
    // (5)^(1/1.5) - 1 ≈ 2.08 = 208%
    assert!(
        result > 1.5 && result < 3.0,
        "VC high-return IRR should be ~200%, got {:.1}%",
        result * 100.0
    );
}

// =============================================================================
// Golden Value Tests - Excel XIRR Parity
// =============================================================================

/// Reference: Microsoft Excel XIRR function
///
/// Test case matching Excel XIRR calculation:
/// Total invested: $10,000, total received: $11,000 over 1 year
/// With quarterly compounding of ~3.9% per quarter, annualized IRR is ~16.6%
#[test]
fn xirr_excel_quarterly_returns() {
    let flows = vec![
        (d(2024, 1, 15), -10_000.0),
        (d(2024, 4, 15), 2_750.0),
        (d(2024, 7, 15), 2_750.0),
        (d(2024, 10, 15), 2_750.0),
        (d(2025, 1, 15), 2_750.0),
    ];

    let result = xirr(&flows, None).expect("XIRR calculation should succeed");

    // Verify NPV at computed rate is approximately zero
    let npv_at_irr = compute_dated_npv(&flows, result);
    assert!(
        npv_at_irr.abs() < 0.01,
        "NPV at IRR should be ~0, got {}",
        npv_at_irr
    );

    // The annualized IRR for these quarterly flows is approximately 16.6%
    // This is higher than simple 10% because of time value of early returns
    assert!(
        result > 0.15 && result < 0.18,
        "IRR should be ~16.6%, got {:.2}%",
        result * 100.0
    );
}

/// Reference: Microsoft Excel XIRR with irregular dates
///
/// Random irregular payment schedule
#[test]
fn xirr_excel_irregular_dates() {
    let flows = vec![
        (d(2024, 3, 7), -50_000.0),
        (d(2024, 5, 23), -25_000.0),
        (d(2024, 8, 14), 10_000.0),
        (d(2024, 11, 30), 20_000.0),
        (d(2025, 2, 28), 30_000.0),
        (d(2025, 6, 15), 25_000.0),
    ];

    let result = xirr(&flows, None).expect("XIRR calculation should succeed");

    // Verify NPV at computed rate is approximately zero
    let npv_at_irr = compute_dated_npv(&flows, result);
    assert!(
        npv_at_irr.abs() < 1.0,
        "NPV at IRR should be ~0, got {}",
        npv_at_irr
    );

    // Total invested: 75K, total returned: 85K over ~15 months
    // Positive return expected
    assert!(
        result > 0.0,
        "IRR should be positive, got {:.2}%",
        result * 100.0
    );
}

// =============================================================================
// Day Count Convention Tests
// =============================================================================

/// Test that Act/365F and Act/360 produce different but related results
#[test]
fn xirr_daycount_act365f_vs_act360() {
    let flows = [
        (d(2024, 1, 1), -100_000.0),
        (d(2024, 7, 1), 102_500.0), // 6-month return
    ];

    let result_365 = xirr_with_daycount(&flows, DayCount::Act365F, None)
        .expect("XIRR with Act/365F should succeed");
    let result_360 = xirr_with_daycount(&flows, DayCount::Act360, None)
        .expect("XIRR with Act/360 should succeed");

    // Both should be positive returns
    assert!(result_365 > 0.0, "Act/365F result should be positive");
    assert!(result_360 > 0.0, "Act/360 result should be positive");

    // Act/360 has a larger year fraction (182/360 > 182/365), so the annualized
    // rate is lower for the same absolute return: result_360 < result_365.
    // In the linear approximation (r ≈ return / yf), the ratio is:
    //   result_360 / result_365 ≈ yf_365 / yf_360 = (182/365) / (182/360) = 360/365
    let ratio = result_360 / result_365;
    let expected_ratio = 360.0 / 365.0;
    assert!(
        (ratio - expected_ratio).abs() < 0.005,
        "Act/360 vs Act/365F ratio should be ~{:.4}, got {:.4}",
        expected_ratio,
        ratio
    );
}

// =============================================================================
// Edge Cases - Break-Even and Boundaries
// =============================================================================

/// Test XIRR with exactly break-even return (IRR = 0)
#[test]
fn xirr_breakeven_zero_return() {
    let flows = [(d(2024, 1, 1), -100_000.0), (d(2025, 1, 1), 100_000.0)];

    let result = xirr(&flows, None).expect("XIRR calculation should succeed");

    // Should be exactly 0% return
    // 2024 is leap year: 366 days, so year fraction = 366/365
    // Solve: -100000 + 100000/(1+r)^(366/365) = 0
    // r = 0 is the solution
    assert!(
        result.abs() < 1e-6,
        "Break-even XIRR should be ~0%, got {:.6}%",
        result * 100.0
    );
}

/// Test XIRR with same-day cashflows (instant return)
#[test]
fn xirr_same_day_instant_return() {
    // Investment and return on same day - mathematically undefined IRR
    // This should return a typed error (no panic).
    let flows = [(d(2024, 6, 15), -100_000.0), (d(2024, 6, 15), 110_000.0)];

    let result = xirr(&flows, None);
    assert!(
        result.is_err(),
        "Same-day cashflows have constant NPV and should not yield a defined IRR"
    );

    // After date-aggregation the two flows collapse into a single net
    // cashflow, so TooFewPoints is the expected error path.  Accept
    // either that or a convergence validation message.
    match result.unwrap_err() {
        Error::Input(_) => {} // e.g. TooFewPoints after aggregation
        Error::Validation(msg) => {
            assert!(
                msg.contains("no convergence") || msg.contains("failed"),
                "Expected a convergence/validation message, got: {}",
                msg
            );
        }
        other => panic!("Expected input or validation error, got: {other:?}"),
    }
}

/// Test XIRR with very long duration (30 years)
#[test]
fn xirr_long_duration_30_years() {
    let flows = vec![
        (d(2024, 1, 1), -100_000.0),
        (d(2054, 1, 1), 500_000.0), // 30 years later
    ];

    let result = xirr(&flows, None).expect("XIRR calculation should succeed");

    // Verify NPV at computed rate is approximately zero
    let npv_at_irr = compute_dated_npv(&flows, result);
    assert!(
        npv_at_irr.abs() < 1.0,
        "NPV at IRR should be ~0, got {}",
        npv_at_irr
    );

    // 5x return over 30 years → IRR = 5^(1/t) - 1 where t is the Act/365F year fraction.
    // Using the actual year fraction avoids the ~1e-4 error from leap-year accumulation.
    let ctx = finstack_core::dates::DayCountContext::default();
    let t_30y = DayCount::Act365F
        .year_fraction(d(2024, 1, 1), d(2054, 1, 1), ctx)
        .unwrap();
    let expected = (5.0_f64).powf(1.0 / t_30y) - 1.0;
    assert!(
        (result - expected).abs() < XIRR_TOLERANCE,
        "30-year IRR should be ~{:.4}%, got {:.4}%",
        expected * 100.0,
        result * 100.0
    );
}

// =============================================================================
// Edge Cases - Near Singularity (r approaching -1)
// =============================================================================

#[test]
fn irr_handles_near_minus_one_singularity() {
    // Cashflows that solve to r approaching -1 (near-total loss)
    // Investment: -100, Return: 0.5 (99.5% loss)
    let amounts = vec![-100.0, 0.5];
    let result = irr(&amounts, None);

    match result {
        Ok(irr) => {
            // IRR should be approximately -99.5%
            assert!(
                irr > -1.0,
                "IRR must be greater than -100% (singularity): got {}",
                irr
            );
            assert!(
                irr < -0.9,
                "IRR for 99.5% loss should be < -90%: got {}",
                irr
            );

            // Verify NPV at computed IRR is approximately zero
            let npv_at_irr = compute_periodic_npv(&amounts, irr);
            assert!(
                npv_at_irr.abs() < 1.0,
                "NPV at IRR should be ~0, got {}",
                npv_at_irr
            );
        }
        Err(_) => {
            // Acceptable to error on extreme cases near singularity
            // The solver may fail to converge for r very close to -1
        }
    }
}

#[test]
fn irr_handles_total_loss() {
    // Total loss scenario: invest 100, get 0 back
    // This has no mathematical solution (would require r = -infinity)
    let amounts = [-100.0, 0.0];
    let result = irr(&amounts, None);

    // Should error (no sign change) or handle gracefully
    // Zero return means no sign change since -100 and 0 don't have opposite signs
    assert!(
        result.is_err(),
        "Total loss (no positive cashflow) should error"
    );
}

#[test]
fn irr_handles_99_percent_loss() {
    // Severe loss: invest 100, get 1 back
    let amounts = vec![-100.0, 1.0];
    let result = irr(&amounts, Some(-0.5)); // Provide hint near -99%

    match result {
        Ok(irr) => {
            // IRR should be approximately -99%
            assert!(irr < -0.9, "IRR should be < -90%: got {}", irr);
            assert!(irr > -1.0, "IRR should be > -100%: got {}", irr);

            // Verify NPV at computed IRR
            let npv_at_irr = compute_periodic_npv(&amounts, irr);
            assert!(
                npv_at_irr.abs() < 1e-6,
                "NPV at IRR should be ~0, got {}",
                npv_at_irr
            );
        }
        Err(_) => {
            // May fail to converge near singularity - acceptable
        }
    }
}

// =============================================================================
// Edge Cases - Multiple Roots (Non-Conventional Cashflows)
// =============================================================================

#[test]
fn irr_multiple_sign_changes_is_not_rejected_as_ambiguous() {
    // Non-conventional cashflow pattern with multiple sign changes
    // These can have multiple mathematical IRR solutions
    // Pattern: -100, +320, -320, +100 (mining project style)
    let amounts = [-100.0, 320.0, -320.0, 100.0];
    let result = irr(&amounts, None);
    if let Err(err) = result {
        assert!(
            !err.to_string().contains("multiple sign changes"),
            "solver should attempt a root instead of rejecting ambiguity: {err}"
        );
    }
}

#[test]
fn irr_two_sign_changes_pattern_is_not_rejected_as_ambiguous() {
    // Simpler two-sign-change pattern: invest, profit, reinvest
    // -100, +200, -50
    let amounts = [-100.0, 200.0, -50.0];
    let result = irr(&amounts, None);
    if let Err(err) = result {
        assert!(
            !err.to_string().contains("multiple sign changes"),
            "solver should attempt a root instead of rejecting ambiguity: {err}"
        );
    }
}

// =============================================================================
// Edge Cases - Extreme Returns
// =============================================================================

#[test]
fn irr_handles_very_high_return() {
    // VC-style return: 10x in one period (900% IRR)
    let amounts = vec![-100.0, 1000.0];
    let result = irr(&amounts, None);

    let irr = result.expect("High return IRR should converge");
    assert!(
        (irr - 9.0).abs() < 1e-6,
        "10x return should be 900% IRR, got {:.2}%",
        irr * 100.0
    );

    // Flows are normalized by max |amount| inside the solver (2026-06-09
    // scale-free acceptance tolerance), so the residual
    // bound is relative to the largest flow (1000), not absolute.
    let npv_at_irr = compute_periodic_npv(&amounts, irr);
    assert!(
        npv_at_irr.abs() / 1000.0 < 1e-6,
        "NPV at IRR should be ~0, got {}",
        npv_at_irr
    );
}

#[test]
fn irr_handles_100x_return() {
    // Unicorn VC return: 100x (9900% IRR)
    let amounts = vec![-100.0, 10000.0];
    let result = irr(&amounts, Some(50.0)); // Provide high initial guess

    let irr = result.expect("100x return IRR should converge");
    // Rate tolerance is relative for extreme rates: the solver normalizes
    // flows by max |amount| , so convergence
    // is scale-free and the absolute rate error scales with (1 + r).
    assert!(
        ((irr - 99.0) / 99.0).abs() < 1e-6,
        "100x return should be 9900% IRR, got {:.4}%",
        irr * 100.0
    );

    // Residual bound relative to the largest flow (10000), see above.
    let npv_at_irr = compute_periodic_npv(&amounts, irr);
    assert!(
        npv_at_irr.abs() / 10_000.0 < 1e-6,
        "NPV at IRR should be ~0, got {}",
        npv_at_irr
    );
}

#[test]
fn irr_handles_small_positive_return() {
    // Near-zero positive return: 0.1%
    let amounts = vec![-100.0, 100.1];
    let result = irr(&amounts, None);

    let irr = result.expect("Small positive return IRR should converge");
    assert!(
        (irr - 0.001).abs() < 1e-6,
        "0.1% return expected, got {:.4}%",
        irr * 100.0
    );

    let npv_at_irr = compute_periodic_npv(&amounts, irr);
    assert!(
        npv_at_irr.abs() < 1e-6,
        "NPV at IRR should be ~0, got {}",
        npv_at_irr
    );
}

#[test]
fn irr_handles_small_negative_return() {
    // Near-zero negative return: -0.1%
    let amounts = vec![-100.0, 99.9];
    let result = irr(&amounts, None);

    let irr = result.expect("Small negative return IRR should converge");
    assert!(
        (irr + 0.001).abs() < 1e-6,
        "-0.1% return expected, got {:.4}%",
        irr * 100.0
    );

    let npv_at_irr = compute_periodic_npv(&amounts, irr);
    assert!(
        npv_at_irr.abs() < 1e-6,
        "NPV at IRR should be ~0, got {}",
        npv_at_irr
    );
}

// =============================================================================
// XIRR Duration and Pattern Edge Cases
// =============================================================================

#[test]
fn xirr_handles_very_short_duration() {
    // Very short investment: 1 week
    let flows = vec![
        (d(2025, 1, 1), -100_000.0),
        (d(2025, 1, 8), 101_000.0), // 1% return in 1 week
    ];

    let result = xirr(&flows, None);
    let irr = result.expect("Short duration XIRR should converge");

    // 1% return in 1 week ≈ 67% annualized (approximately (1.01)^52 - 1)
    // Actual depends on day count
    assert!(
        irr > 0.5,
        "Annualized return should be > 50%, got {:.2}%",
        irr * 100.0
    );

    let npv_at_irr = compute_dated_npv(&flows, irr);
    assert!(
        npv_at_irr.abs() < 1.0,
        "NPV at IRR should be ~0, got {}",
        npv_at_irr
    );
}

#[test]
fn xirr_handles_same_day_multiple_flows() {
    // Multiple flows on same day - should aggregate correctly
    let flows = vec![
        (d(2025, 1, 1), -50_000.0),
        (d(2025, 1, 1), -50_000.0), // Second investment same day
        (d(2026, 1, 1), 110_000.0),
    ];

    let result = xirr(&flows, None);
    let irr = result.expect("Same-day flows XIRR should converge");

    // Total: -100K, +110K over 1 year = 10% return
    // Similar to single investment of -100K
    assert!(
        irr > 0.05 && irr < 0.15,
        "IRR should be ~10%, got {:.2}%",
        irr * 100.0
    );

    let npv_at_irr = compute_dated_npv(&flows, irr);
    assert!(
        npv_at_irr.abs() < 1.0,
        "NPV at IRR should be ~0, got {}",
        npv_at_irr
    );
}

#[test]
fn xirr_handles_distant_horizon() {
    // Very long horizon: 50 years
    let flows = vec![
        (d(2025, 1, 1), -100_000.0),
        (d(2075, 1, 1), 1_000_000.0), // 10x over 50 years
    ];

    let result = xirr(&flows, None);
    let irr = result.expect("Long horizon XIRR should converge");

    // 10x over 50 years: IRR = 10^(1/t) - 1 where t is the Act/365F year fraction.
    // Using the actual year fraction avoids the ~1e-4 error from leap-year accumulation.
    let ctx = finstack_core::dates::DayCountContext::default();
    let t_50y = DayCount::Act365F
        .year_fraction(d(2025, 1, 1), d(2075, 1, 1), ctx)
        .unwrap();
    let expected = (10.0_f64).powf(1.0 / t_50y) - 1.0;
    assert!(
        (irr - expected).abs() < XIRR_TOLERANCE,
        "50-year 10x IRR should be ~{:.4}%, got {:.4}%",
        expected * 100.0,
        irr * 100.0
    );

    let npv_at_irr = compute_dated_npv(&flows, irr);
    assert!(
        npv_at_irr.abs() < 1.0,
        "NPV at IRR should be ~0, got {}",
        npv_at_irr
    );
}

#[test]
fn xirr_supports_contextual_day_counts_via_explicit_ctx() {
    let flows = vec![(d(2025, 1, 1), -1000.0), (d(2026, 1, 1), 1100.0)];
    let ctx = DayCountContext {
        frequency: Some(Tenor::annual()),
        calendar: None,
        bus_basis: None,
        coupon_period: None,
    };

    let result = xirr_with_daycount_ctx(&flows, DayCount::ActActIsma, ctx, None)
        .expect("contextual day-count XIRR should succeed");

    assert!((result - 0.10).abs() < XIRR_TOLERANCE);
}

// =============================================================================
// Input Validation Tests
// =============================================================================

#[test]
fn irr_rejects_single_cashflow() {
    let amounts = [-100.0];
    let result = irr(&amounts, None);
    assert!(result.is_err(), "Single cashflow should error");
}

#[test]
fn irr_rejects_empty_cashflows() {
    let amounts: Vec<f64> = vec![];
    let result = irr(&amounts, None);
    assert!(result.is_err(), "Empty cashflows should error");
}

#[test]
fn irr_rejects_all_positive_cashflows() {
    // All positive = no investment pattern
    let amounts = [100.0, 200.0, 300.0];
    let result = irr(&amounts, None);
    assert!(result.is_err(), "All positive cashflows should error");
}

#[test]
fn irr_rejects_all_negative_cashflows() {
    // All negative = no return pattern
    let amounts = [-100.0, -200.0, -300.0];
    let result = irr(&amounts, None);
    assert!(result.is_err(), "All negative cashflows should error");
}

#[test]
fn xirr_rejects_single_flow() {
    let flows = [(d(2025, 1, 1), -100_000.0)];
    let result = xirr(&flows, None);
    assert!(result.is_err(), "Single dated flow should error");
}

#[test]
fn xirr_rejects_all_same_sign() {
    let flows = [(d(2025, 1, 1), 100_000.0), (d(2026, 1, 1), 200_000.0)];
    let result = xirr(&flows, None);
    assert!(result.is_err(), "All same sign flows should error");
}

#[test]
fn irr_attempts_ambiguous_multi_root_cashflows() {
    let amounts = [-100.0, 320.0, -320.0, 100.0];
    let result = irr(&amounts, None);
    if let Err(err) = result {
        assert!(
            !err.to_string().contains("multiple sign changes"),
            "non-conventional cashflows should no longer be rejected explicitly: {err}"
        );
    }
}

#[test]
fn xirr_attempts_unsorted_multi_sign_change_flows_after_sorting() {
    let flows = [
        (d(2026, 1, 1), 250.0),
        (d(2025, 1, 1), -100.0),
        (d(2027, 1, 1), -175.0),
    ];

    let result = xirr(&flows, None);
    if let Err(err) = result {
        assert!(
            !err.to_string().contains("multiple sign changes"),
            "dated multi-sign-change cashflows should be attempted after sorting: {err}"
        );
    }
}

#[test]
fn xirr_ignores_zero_flows_when_detecting_sign_changes() {
    let flows = [
        (d(2025, 1, 1), -100.0),
        (d(2025, 6, 1), 0.0),
        (d(2026, 1, 1), 120.0),
    ];

    let result =
        xirr(&flows, None).expect("zero-valued intermediate flows should not create ambiguity");
    let npv_at_irr = compute_dated_npv(&flows, result);
    assert!(
        npv_at_irr.abs() < 0.01,
        "NPV at IRR should be ~0, got {}",
        npv_at_irr
    );
}
