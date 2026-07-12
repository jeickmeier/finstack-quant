//! Tests for NPV and discount factor calculations.
//!
//! These tests verify numerical stability and correctness of:
//! - NPV calculations with varying cashflow counts
//! - Extreme durations (50+ years)
//! - Large and small amounts
//! - Negative and zero rates
//! - Discount factor properties
//! - Currency safety
//!
//! # Test Curve Convention
//!
//! Test curves use **continuous compounding**: `DF(t) = exp(-rate * t)`

use finstack_quant_core::cashflow::npv;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::traits::{Discounting, TermStructure};
use finstack_quant_core::math::neumaier_sum;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use time::Month;

// =============================================================================
// Test Helpers
// =============================================================================

/// Tolerance for financial amount comparisons based on notional.
fn financial_tolerance(notional: f64) -> f64 {
    (notional.abs() * 1e-8).max(0.01)
}

/// Helper to create dates
fn d(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).unwrap(), day).unwrap()
}

/// Flat-rate discount curve with continuous compounding: DF(t) = exp(-rate * t)
struct FlatRateCurve {
    id: CurveId,
    base: Date,
    rate: f64,
}

impl FlatRateCurve {
    fn new(id: &str, base: Date, rate: f64) -> Self {
        Self {
            id: CurveId::new(id),
            base,
            rate,
        }
    }
}

impl TermStructure for FlatRateCurve {
    fn id(&self) -> &CurveId {
        &self.id
    }
}

impl Discounting for FlatRateCurve {
    fn base_date(&self) -> Date {
        self.base
    }

    fn df(&self, t: f64) -> f64 {
        // The t <= 0 clamp only matters for direct df() property tests below
        // (and for npv_with_options with include_past_flows): since the
        // the default npv excludes flows on/before the
        // valuation date and never requests a non-positive abscissa relative
        // to the valuation date.
        if t <= 0.0 {
            1.0
        } else {
            (-self.rate * t).exp()
        }
    }
}

// =============================================================================
// Large Cashflow Count Tests
// =============================================================================

#[test]
fn npv_100_cashflows_maintains_precision() {
    let base = d(2025, 1, 1);
    let curve = FlatRateCurve::new("TEST", base, 0.05);
    let dc = DayCount::Act365F;
    let ctx = DayCountContext::default();

    // 100 monthly cashflows of $1000 each, all strictly after the valuation
    // date (npv excludes flows on/before base by convention).
    let flows: Vec<(Date, Money)> = (1..=100)
        .map(|i| {
            let year = 2025 + (i / 12);
            let month = (i % 12) + 1;
            let date =
                Date::from_calendar_date(year, Month::try_from(month as u8).unwrap(), 1).unwrap();
            (date, Money::new(1000.0, Currency::USD))
        })
        .collect();

    let pv = npv(&curve, base, &flows).expect("NPV should succeed");

    // Compute expected using stable summation
    let expected_terms: Vec<f64> = flows
        .iter()
        .map(|(date, amt)| {
            let t = dc.year_fraction(base, *date, ctx).unwrap_or(0.0);
            amt.amount() * (-0.05 * t).exp()
        })
        .collect();
    let expected = neumaier_sum(expected_terms.iter().copied());

    assert!(
        (pv.amount() - expected).abs() < financial_tolerance(expected),
        "100 cashflows: expected {:.2}, got {:.2}",
        expected,
        pv.amount()
    );
}

#[test]
fn npv_500_cashflows_maintains_precision() {
    let base = d(2025, 1, 1);
    let curve = FlatRateCurve::new("TEST", base, 0.05);
    let dc = DayCount::Act365F;
    let ctx = DayCountContext::default();

    // 500 weekly cashflows of $100 each (~10 years), all strictly after the
    // valuation date (npv excludes flows on/before base by convention).
    let flows: Vec<(Date, Money)> = (1..=500)
        .map(|i| {
            let days = i * 7;
            let date = base + time::Duration::days(days as i64);
            (date, Money::new(100.0, Currency::USD))
        })
        .collect();

    let pv = npv(&curve, base, &flows).expect("NPV should succeed");

    // Compute expected using stable summation
    let expected_terms: Vec<f64> = flows
        .iter()
        .map(|(date, amt)| {
            let t = dc.year_fraction(base, *date, ctx).unwrap_or(0.0);
            amt.amount() * (-0.05 * t).exp()
        })
        .collect();
    let expected = neumaier_sum(expected_terms.iter().copied());

    assert!(
        (pv.amount() - expected).abs() < financial_tolerance(expected),
        "500 cashflows: expected {:.2}, got {:.2}",
        expected,
        pv.amount()
    );

    // PV should be less than undiscounted sum (50,000)
    assert!(
        pv.amount() < 50_000.0,
        "PV should be less than undiscounted sum"
    );
    assert!(pv.amount() > 0.0, "PV should be positive");
}

// =============================================================================
// Extreme Duration Tests
// =============================================================================

#[test]
fn npv_50_year_cashflow_is_positive_and_small() {
    let base = d(2025, 1, 1);
    let maturity = d(2075, 1, 1); // 50 years
    let curve = FlatRateCurve::new("TEST", base, 0.05);
    let dc = DayCount::Act365F;
    let ctx = DayCountContext::default();

    let flows = vec![(maturity, Money::new(1_000_000.0, Currency::USD))];
    let pv = npv(&curve, base, &flows).expect("NPV should succeed");

    // Calculate actual year fraction (includes leap years)
    let t = dc.year_fraction(base, maturity, ctx).unwrap();
    // DF at 5% continuous = exp(-0.05 * t)
    let expected_df = (-0.05_f64 * t).exp();
    let expected_pv = 1_000_000.0 * expected_df;

    assert!(pv.amount() > 0.0, "50-year PV must be positive");
    assert!(pv.amount() < 1_000_000.0, "50-year PV must be discounted");
    // Both expected_pv and the NPV function use the same year fraction, so
    // results should agree to near floating-point precision (1e-6 relative).
    assert!(
        (pv.amount() - expected_pv).abs() / expected_pv < 1e-6,
        "50-year PV should be ~{:.0}, got {:.0} (year fraction: {:.4})",
        expected_pv,
        pv.amount(),
        t
    );
}

#[test]
fn npv_100_year_cashflow_is_tiny_but_positive() {
    let base = d(2025, 1, 1);
    let maturity = d(2125, 1, 1); // 100 years
    let curve = FlatRateCurve::new("TEST", base, 0.05);

    let flows = vec![(maturity, Money::new(1_000_000.0, Currency::USD))];
    let pv = npv(&curve, base, &flows).expect("NPV should succeed");

    // DF(100y) at 5% continuous = exp(-0.05 * 100) = exp(-5) ≈ 0.00674
    // PV ≈ $6,738
    let expected_df = (-0.05_f64 * 100.0).exp();
    let expected_pv = 1_000_000.0 * expected_df;

    assert!(
        pv.amount() > 0.0,
        "100-year PV must be positive (no underflow)"
    );
    assert!(
        pv.amount() < expected_pv * 2.0,
        "100-year PV should be ~{:.0}, got {:.0}",
        expected_pv,
        pv.amount()
    );
}

#[test]
fn npv_cashflow_at_base_date_is_excluded_by_default() {
    //  npv follows
    // market-standard pricing semantics — flows on or before the valuation
    // date are excluded. A flow on the base date therefore prices to zero;
    // NpvOptions::include_past_flows restores the legacy notional-at-par.
    let base = d(2025, 1, 1);
    let curve = FlatRateCurve::new("TEST", base, 0.05);

    let flows = vec![(base, Money::new(100_000.0, Currency::USD))];
    let pv = npv(&curve, base, &flows).expect("NPV should succeed");
    assert_eq!(
        pv.amount(),
        0.0,
        "flow on the valuation date must be excluded by default"
    );

    let pv_incl = finstack_quant_core::cashflow::npv_with_options(
        &curve,
        base,
        DayCountContext::default(),
        finstack_quant_core::cashflow::NpvOptions::default().include_past_flows(true),
        &flows,
    )
    .expect("NPV with include_past_flows should succeed");
    assert!(
        (pv_incl.amount() - 100_000.0).abs() < financial_tolerance(100_000.0),
        "PV at t=0 with include_past_flows should equal notional, got {}",
        pv_incl.amount()
    );
}

// =============================================================================
// Rate Sensitivity Tests
// =============================================================================

#[test]
fn npv_negative_rate_inflates_value() {
    let base = d(2025, 1, 1);
    let curve = FlatRateCurve::new("TEST", base, -0.02); // Negative rate

    let flows = vec![(d(2026, 1, 1), Money::new(100.0, Currency::USD))];
    let pv = npv(&curve, base, &flows).expect("NPV should succeed");

    // Negative rate: DF > 1, so PV > FV
    // DF(1y) at -2% = exp(0.02) ≈ 1.0202
    let expected_df = (0.02_f64).exp();
    let expected_pv = 100.0 * expected_df;

    assert!(
        pv.amount() > 100.0,
        "Negative rate should inflate PV: got {}",
        pv.amount()
    );
    assert!(
        (pv.amount() - expected_pv).abs() < financial_tolerance(expected_pv),
        "PV with -2% rate should be ~{:.2}, got {:.2}",
        expected_pv,
        pv.amount()
    );
}

#[test]
fn npv_zero_rate_preserves_value() {
    let base = d(2025, 1, 1);
    let curve = FlatRateCurve::new("TEST", base, 0.0);

    let flows = vec![
        (d(2026, 1, 1), Money::new(1000.0, Currency::USD)),
        (d(2027, 1, 1), Money::new(1000.0, Currency::USD)),
        (d(2028, 1, 1), Money::new(1000.0, Currency::USD)),
    ];
    let pv = npv(&curve, base, &flows).expect("NPV should succeed");

    // Zero rate: DF = 1.0, so PV = sum of cashflows
    assert!(
        (pv.amount() - 3000.0).abs() < financial_tolerance(3000.0),
        "Zero rate PV should equal sum of cashflows: got {}",
        pv.amount()
    );
}

// =============================================================================
// Extreme Amount Tests
// =============================================================================

#[test]
fn npv_very_large_amounts() {
    let base = d(2025, 1, 1);
    let curve = FlatRateCurve::new("TEST", base, 0.05);

    // $1 trillion cashflow
    let flows = vec![(d(2026, 1, 1), Money::new(1e12, Currency::USD))];
    let pv = npv(&curve, base, &flows).expect("NPV should succeed");

    // DF(1y) at 5% = exp(-0.05) ≈ 0.9512
    let expected_df = (-0.05_f64).exp();
    let expected_pv = 1e12 * expected_df;

    assert!(
        (pv.amount() - expected_pv).abs() < financial_tolerance(expected_pv),
        "Large amount PV: expected {:.0}, got {:.0}",
        expected_pv,
        pv.amount()
    );
}

#[test]
fn npv_very_small_amounts() {
    let base = d(2025, 1, 1);
    let curve = FlatRateCurve::new("TEST", base, 0.05);

    // $0.01 cashflow (1 cent, minimum for USD)
    let flows = vec![(d(2026, 1, 1), Money::new(0.01, Currency::USD))];
    let pv = npv(&curve, base, &flows).expect("NPV should succeed");

    // PV should be positive but tiny
    assert!(pv.amount() > 0.0, "Small amount PV should be positive");
    assert!(pv.amount() < 0.01, "Small amount PV should be < original");
}

// =============================================================================
// Discount Factor Property Tests
// =============================================================================

#[test]
fn discount_factor_monotonically_decreases() {
    let base = d(2025, 1, 1);
    let curve = FlatRateCurve::new("TEST", base, 0.05);

    let times = [0.0, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 50.0];
    let mut prev_df = f64::MAX;

    for t in times {
        let df = curve.df(t);
        assert!(
            df <= prev_df,
            "DF must be monotonically decreasing: DF({}) = {} > DF(prev)",
            t,
            df
        );
        assert!(df > 0.0, "DF must be positive: DF({}) = {}", t, df);
        assert!(
            df <= 1.0,
            "DF must be <= 1.0 for positive rates: DF({}) = {}",
            t,
            df
        );
        prev_df = df;
    }
}

#[test]
fn discount_factor_at_zero_is_one() {
    let base = d(2025, 1, 1);
    let curve = FlatRateCurve::new("TEST", base, 0.05);

    let df_at_zero = curve.df(0.0);
    assert!(
        (df_at_zero - 1.0).abs() < 1e-12,
        "DF(0) must equal 1.0, got {}",
        df_at_zero
    );
}

#[test]
fn discount_factor_handles_negative_time() {
    // Cashflows before base date - verify graceful handling
    let base = d(2025, 1, 1);
    let curve = FlatRateCurve::new("TEST", base, 0.05);

    let df_negative = curve.df(-1.0);
    // Should return 1.0 for t <= 0 (past cashflows at par)
    assert!(
        (df_negative - 1.0).abs() < 1e-12,
        "DF(t<0) should be 1.0, got {}",
        df_negative
    );
}

// =============================================================================
// Currency Safety Tests
// =============================================================================

#[test]
fn npv_rejects_mixed_currencies() {
    let base = d(2025, 1, 1);
    let curve = FlatRateCurve::new("TEST", base, 0.05);

    let flows = vec![
        (d(2026, 1, 1), Money::new(1000.0, Currency::USD)),
        (d(2027, 1, 1), Money::new(1000.0, Currency::EUR)), // Different currency!
    ];

    let result = npv(&curve, base, &flows);

    // Should error due to currency mismatch
    assert!(result.is_err(), "NPV should reject mixed currencies");
}
