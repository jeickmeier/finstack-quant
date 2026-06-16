//! Test utilities for CDS Index tests.
//!
//! Provides shared helpers for creating market data, instruments, and assertions.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::constants::isda::STANDARD_RECOVERY_SENIOR;
use finstack_quant_valuations::instruments::credit_derivatives::cds::PayReceive;
use finstack_quant_valuations::instruments::credit_derivatives::cds_index::{
    CDSIndex, CDSIndexConstituent, CDSIndexParams,
};
use finstack_quant_valuations::instruments::CreditParams;

pub const STANDARD_HAZARD_RATE: f64 = 0.015;

/// Create a flat discount curve for testing
pub fn flat_discount_curve(id: &str, base: Date, rate: f64) -> DiscountCurve {
    let df_10y = (-rate * 10.0).exp();
    DiscountCurve::builder(id)
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (5.0, (-rate * 5.0).exp()), (10.0, df_10y)])
        .build()
        .unwrap()
}

/// Create a flat hazard curve for testing
pub fn flat_hazard_curve(id: &str, base: Date, recovery: f64, hazard_rate: f64) -> HazardCurve {
    let par_spread_bp = hazard_rate * 10000.0 * (1.0 - recovery);
    HazardCurve::builder(id)
        .base_date(base)
        .recovery_rate(recovery)
        .knots([(1.0, hazard_rate), (5.0, hazard_rate), (10.0, hazard_rate)])
        .par_spreads([(1.0, par_spread_bp), (5.0, par_spread_bp)])
        .build()
        .unwrap()
}

/// Create a standard market context for testing
pub fn standard_market_context(base: Date) -> MarketContext {
    let disc = flat_discount_curve("USD-OIS", base, 0.03);
    let hz = flat_hazard_curve(
        "HZ-INDEX",
        base,
        STANDARD_RECOVERY_SENIOR,
        STANDARD_HAZARD_RATE,
    );

    MarketContext::new().insert(disc).insert(hz)
}

/// Create a market context with multiple constituents
pub fn multi_constituent_market_context(base: Date, num_constituents: usize) -> MarketContext {
    let disc = flat_discount_curve("USD-OIS", base, 0.03);
    let mut ctx = MarketContext::new().insert(disc);

    for i in 0..num_constituents {
        let hz_id = format!("HZ{}", i + 1);
        let hz = flat_hazard_curve(&hz_id, base, STANDARD_RECOVERY_SENIOR, STANDARD_HAZARD_RATE);
        ctx = ctx.insert(hz);
    }

    // Add index-level hazard
    let hz_index = flat_hazard_curve(
        "HZ-INDEX",
        base,
        STANDARD_RECOVERY_SENIOR,
        STANDARD_HAZARD_RATE,
    );
    ctx = ctx.insert(hz_index);

    ctx
}

/// Create standard CDX IG preset.
pub fn standard_cdx_params() -> CDSIndexParams {
    CDSIndexParams::cdx_na_ig(42, 1, 100.0)
}

/// Create equal-weight constituents (active, not defaulted).
pub fn equal_weight_constituents(count: usize) -> Vec<CDSIndexConstituent> {
    (0..count)
        .map(|i| {
            CDSIndexConstituent::active(
                CreditParams::corporate_standard(format!("NAME{}", i + 1), format!("HZ{}", i + 1)),
                1.0 / count as f64,
            )
        })
        .collect()
}

/// Create a standard single-curve index
pub fn standard_single_curve_index(id: &str, start: Date, end: Date, notional: f64) -> CDSIndex {
    CDSIndex::from_preset(
        &standard_cdx_params(),
        id,
        Money::new(notional, Currency::USD),
        PayReceive::Pay,
        start,
        end,
        STANDARD_RECOVERY_SENIOR,
        "USD-OIS",
        "HZ-INDEX",
    )
    .expect("valid test parameters")
}

/// Create a standard constituents-based index
pub fn standard_constituents_index(
    id: &str,
    start: Date,
    end: Date,
    notional: f64,
    num_constituents: usize,
) -> CDSIndex {
    CDSIndex::from_preset(
        &standard_cdx_params(),
        id,
        Money::new(notional, Currency::USD),
        PayReceive::Pay,
        start,
        end,
        STANDARD_RECOVERY_SENIOR,
        "USD-OIS",
        "HZ-INDEX",
    )
    .expect("valid test parameters")
    .with_constituents(equal_weight_constituents(num_constituents))
}

/// Analytical par spread for a flat hazard rate and constant recovery (bps)
pub fn flat_hazard_par_spread_bps(hazard_rate: f64, recovery: f64) -> f64 {
    hazard_rate * (1.0 - recovery) * 10000.0
}

/// Assert money values are approximately equal (within tolerance)
pub fn assert_money_approx_eq(actual: Money, expected: Money, tolerance: f64, message: &str) {
    assert_eq!(actual.currency(), expected.currency());
    let diff = (actual.amount() - expected.amount()).abs();
    assert!(
        diff <= tolerance,
        "{}: expected {}, got {} (diff: {})",
        message,
        expected.amount(),
        actual.amount(),
        diff
    );
}

/// Assert relative error is within percentage tolerance
pub fn assert_relative_eq(actual: f64, expected: f64, pct_tolerance: f64, message: &str) {
    if expected.abs() < 1e-10 {
        assert!(
            actual.abs() < 1e-10,
            "{}: expected ~0, got {}",
            message,
            actual
        );
    } else {
        let rel_err = ((actual - expected) / expected).abs();
        assert!(
            rel_err <= pct_tolerance,
            "{}: expected {}, got {} (rel err: {:.2}%)",
            message,
            expected,
            actual,
            rel_err * 100.0
        );
    }
}

/// Assert value is within range
pub fn assert_in_range(value: f64, min: f64, max: f64, message: &str) {
    assert!(
        value >= min && value <= max,
        "{}: expected {} in range [{}, {}]",
        message,
        value,
        min,
        max
    );
}

/// Assert metric is positive
pub fn assert_positive(value: f64, metric_name: &str) {
    assert!(
        value > 0.0,
        "{} should be positive, got {}",
        metric_name,
        value
    );
}

/// Assert metric scales linearly with notional
pub fn assert_linear_scaling(
    value1: f64,
    notional1: f64,
    value2: f64,
    notional2: f64,
    metric_name: &str,
    tolerance: f64,
) {
    let expected_ratio = notional2 / notional1;
    let actual_ratio = value2 / value1;
    let ratio_diff = (actual_ratio - expected_ratio).abs();

    assert!(
        ratio_diff < tolerance,
        "{} should scale linearly with notional: {:.0}→{:.0} expected ratio {:.2}, got {:.2}",
        metric_name,
        value1,
        value2,
        expected_ratio,
        actual_ratio
    );
}
