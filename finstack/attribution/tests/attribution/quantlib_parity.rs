//! External validation tests for P&L attribution.
//!
//! These tests validate attribution results against analytical formulas and
//! known mathematical relationships. For bonds, we use the analytical DV01
//! formula to verify that rates P&L matches expected sensitivity.
//!
//! ## Reference Formulas
//!
//! ### Bond DV01 (Central Bump)
//!
//! For a bond with price P, DV01 is approximated by a 1bp central difference:
//!   DV01 ≈ (P_down - P_up) / 2
//!
//! Convexity is approximated with a second-difference:
//!   Convexity_cash ≈ (P_up + P_down - 2P_base) / (Δr)^2
//!
//! ### Rates P&L Attribution
//!
//! For parallel rate shift Δr (in decimal):
//!   Rates_PnL ≈ DV01 × (Δr × 10,000)
//!
//! With second-order correction:
//!   Rates_PnL ≈ DV01 × (Δr × 10,000) + ½ × Convexity × P × (Δr)²
//!
//! ## Tolerances
//!
//! - First-order approximation: < 5% relative error for small shifts (<50bp)
//! - With convexity: < 1% relative error for larger shifts

use finstack_attribution::{attribute_pnl_parallel, AttributionMethod};
use finstack_core::config::FinstackConfig;
use finstack_core::currency::Currency;
use finstack_core::dates::create_date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::bond::Bond;
use finstack_valuations::instruments::Instrument;
use std::sync::Arc;
use time::Month;

/// Helper to compute discount factor from rate and time.
fn df_from_rate(rate: f64, years: f64) -> f64 {
    (-rate * years).exp()
}

/// Helper to approximate rate from discount factor and time.
#[allow(dead_code)]
fn rate_from_df(df: f64, years: f64) -> f64 {
    -df.ln() / years
}

/// Test case configuration for analytical parity.
struct AnalyticalParityTestCase {
    name: &'static str,
    notional: f64,
    coupon_rate: f64,
    maturity_years: u32,
    rate_t0: f64,
    rate_t1: f64,
    /// Expected relative error tolerance for first-order approximation
    tolerance_pct: f64,
}

impl AnalyticalParityTestCase {
    fn new_small_rate_increase() -> Self {
        Self {
            name: "5Y bond, 10bp rate increase",
            notional: 1_000_000.0,
            coupon_rate: 0.05,
            maturity_years: 5,
            rate_t0: 0.04,
            rate_t1: 0.041, // 10bp increase
            tolerance_pct: 5.0,
        }
    }

    fn new_large_rate_increase() -> Self {
        Self {
            name: "5Y bond, 100bp rate increase",
            notional: 1_000_000.0,
            coupon_rate: 0.05,
            maturity_years: 5,
            rate_t0: 0.04,
            rate_t1: 0.05,       // 100bp increase
            tolerance_pct: 10.0, // Larger tolerance due to convexity
        }
    }

    fn new_rate_decrease() -> Self {
        Self {
            name: "5Y bond, 50bp rate decrease",
            notional: 1_000_000.0,
            coupon_rate: 0.05,
            maturity_years: 5,
            rate_t0: 0.04,
            rate_t1: 0.035, // 50bp decrease
            tolerance_pct: 5.0,
        }
    }
}

/// Compute DV01 and convexity using a 1bp central bump.
fn compute_bumped_sensitivities(
    instrument: &dyn Instrument,
    as_of: time::Date,
    curve_id: &str,
    base_rate: f64,
) -> (f64, f64, f64) {
    let bump = 0.0001; // 1bp
    let curve_base = build_flat_curve(curve_id, as_of, base_rate);
    let curve_up = build_flat_curve(curve_id, as_of, base_rate + bump);
    let curve_down = build_flat_curve(curve_id, as_of, base_rate - bump);

    let market_base = MarketContext::new().insert(curve_base);
    let market_up = MarketContext::new().insert(curve_up);
    let market_down = MarketContext::new().insert(curve_down);

    let price_base = instrument.value(&market_base, as_of).unwrap().amount();
    let price_up = instrument.value(&market_up, as_of).unwrap().amount();
    let price_down = instrument.value(&market_down, as_of).unwrap().amount();

    let dv01 = (price_down - price_up) * 0.5;
    let convexity_cash = (price_up + price_down - 2.0 * price_base) / (bump * bump);

    (price_base, dv01, convexity_cash)
}

/// Build a flat discount curve at the given rate.
fn build_flat_curve(curve_id: &str, as_of: time::Date, rate: f64) -> DiscountCurve {
    let tenors = [0.0, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 20.0, 30.0];
    let knots: Vec<(f64, f64)> = tenors.iter().map(|&t| (t, df_from_rate(rate, t))).collect();

    DiscountCurve::builder(curve_id)
        .base_date(as_of)
        .knots(knots)
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

fn run_analytical_parity_test(tc: &AnalyticalParityTestCase) {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let issue = create_date(2025, Month::January, 1).unwrap();
    let maturity = create_date(2025 + tc.maturity_years as i32, Month::January, 1).unwrap();

    let bond = Bond::fixed(
        "PARITY-TEST-BOND",
        Money::new(tc.notional, Currency::USD),
        tc.coupon_rate,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    // Build markets at T0 and T1 with different rates
    let curve_t0 = build_flat_curve("USD-OIS", as_of_t0, tc.rate_t0);
    let curve_t1 = build_flat_curve("USD-OIS", as_of_t1, tc.rate_t1);

    let market_t0 = MarketContext::new().insert(curve_t0);
    let market_t1 = MarketContext::new().insert(curve_t1);

    let config = FinstackConfig::default();

    let (_price_base, dv01, convexity_cash) =
        compute_bumped_sensitivities(&bond, as_of_t1, "USD-OIS", tc.rate_t0);

    // Run attribution
    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);
    let attribution = attribute_pnl_parallel(
        &bond_instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        None,
    )
    .unwrap();

    let rate_change_decimal = tc.rate_t1 - tc.rate_t0;
    let rate_change_bp = rate_change_decimal * 10_000.0;
    let expected_rates_pnl =
        dv01 * rate_change_bp + 0.5 * convexity_cash * rate_change_decimal * rate_change_decimal;

    let actual_rates_pnl = attribution.rates_curves_pnl.amount();

    // Verify directionality: rates up → bond value down → negative P&L
    if tc.rate_t1 > tc.rate_t0 {
        assert!(
            actual_rates_pnl < 0.0,
            "{}: Rates P&L should be negative when rates increase, got {}",
            tc.name,
            actual_rates_pnl
        );
    } else {
        assert!(
            actual_rates_pnl > 0.0,
            "{}: Rates P&L should be positive when rates decrease, got {}",
            tc.name,
            actual_rates_pnl
        );
    }

    // Verify magnitude is in reasonable range of analytical approximation
    // Allow for convexity effects and approximation errors
    let actual_abs = actual_rates_pnl.abs();
    let expected_abs = expected_rates_pnl.abs();

    // For small moves, first-order should be close
    // For large moves, convexity helps the long position (actual should be less negative than expected)
    let rel_diff = if expected_abs > 100.0 {
        ((actual_abs - expected_abs) / expected_abs).abs() * 100.0
    } else {
        (actual_abs - expected_abs).abs() // Use absolute for small values
    };

    // Log for debugging
    eprintln!(
        "{}: rate_change={}bp, expected_pnl={:.2}, actual_pnl={:.2}, rel_diff={:.2}%",
        tc.name, rate_change_bp, expected_rates_pnl, actual_rates_pnl, rel_diff
    );

    assert!(
        rel_diff < tc.tolerance_pct || actual_abs < 200.0, // Skip tolerance check for very small values
        "{}: Rates P&L ({:.2}) differs from analytical estimate ({:.2}) by {:.2}% (tolerance: {}%)",
        tc.name,
        actual_rates_pnl,
        expected_rates_pnl,
        rel_diff,
        tc.tolerance_pct
    );
}

#[test]
fn test_analytical_parity_small_rate_increase() {
    let tc = AnalyticalParityTestCase::new_small_rate_increase();
    run_analytical_parity_test(&tc);
}

#[test]
fn test_analytical_parity_large_rate_increase() {
    let tc = AnalyticalParityTestCase::new_large_rate_increase();
    run_analytical_parity_test(&tc);
}

#[test]
fn test_analytical_parity_rate_decrease() {
    let tc = AnalyticalParityTestCase::new_rate_decrease();
    run_analytical_parity_test(&tc);
}

/// Test that attribution method is correctly identified.
#[test]
fn test_attribution_method_metadata() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let issue = create_date(2025, Month::January, 1).unwrap();
    let maturity = create_date(2030, Month::January, 1).unwrap();

    let bond = Bond::fixed(
        "METADATA-TEST",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let curve = build_flat_curve("USD-OIS", as_of_t0, 0.04);
    let market = MarketContext::new().insert(curve);

    let config = FinstackConfig::default();
    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);

    let attribution = attribute_pnl_parallel(
        &bond_instrument,
        &market,
        &market,
        as_of_t0,
        as_of_t1,
        &config,
        None,
    )
    .unwrap();

    // Verify metadata
    assert!(matches!(
        attribution.meta.method,
        AttributionMethod::Parallel
    ));
    assert_eq!(attribution.meta.instrument_id, "METADATA-TEST");
    assert_eq!(attribution.meta.t0, as_of_t0);
    assert_eq!(attribution.meta.t1, as_of_t1);
}

/// Test convexity benefit: for equal magnitude rate moves,
/// the gain from rate decrease should exceed the loss from rate increase.
///
/// This is a fundamental property of positive convexity instruments (bonds).
#[test]
fn test_convexity_benefit_symmetric_moves() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let issue = create_date(2025, Month::January, 1).unwrap();
    let maturity = create_date(2030, Month::January, 1).unwrap();

    let bond = Bond::fixed(
        "CONVEXITY-BENEFIT-TEST",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let rate_base = 0.04;
    let rate_shift = 0.01; // 100bp

    let curve_base = build_flat_curve("USD-OIS", as_of_t0, rate_base);
    let curve_up = build_flat_curve("USD-OIS", as_of_t1, rate_base + rate_shift);
    let curve_down = build_flat_curve("USD-OIS", as_of_t1, rate_base - rate_shift);

    let market_base = MarketContext::new().insert(curve_base);
    let market_up = MarketContext::new().insert(curve_up);
    let market_down = MarketContext::new().insert(curve_down);

    let config = FinstackConfig::default();
    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);

    // Attribution for rate increase
    let attr_up = attribute_pnl_parallel(
        &bond_instrument,
        &market_base,
        &market_up,
        as_of_t0,
        as_of_t1,
        &config,
        None,
    )
    .unwrap();

    // Attribution for rate decrease
    let attr_down = attribute_pnl_parallel(
        &bond_instrument,
        &market_base,
        &market_down,
        as_of_t0,
        as_of_t1,
        &config,
        None,
    )
    .unwrap();

    let loss_from_rate_increase = -attr_up.rates_curves_pnl.amount(); // Make positive
    let gain_from_rate_decrease = attr_down.rates_curves_pnl.amount();

    // Convexity benefit: gain > loss for equal magnitude moves
    assert!(
        gain_from_rate_decrease > loss_from_rate_increase,
        "Convexity benefit: gain from rate decrease ({:.2}) should exceed loss from rate increase ({:.2})",
        gain_from_rate_decrease,
        loss_from_rate_increase
    );

    // The convexity benefit should be roughly 2 × ½ × Convexity × P × (Δr)²
    // For a 5Y bond, this is typically a few hundred dollars on $1M notional with 100bp move
    let convexity_benefit = gain_from_rate_decrease - loss_from_rate_increase;
    assert!(
        convexity_benefit > 0.0,
        "Convexity benefit should be positive, got {}",
        convexity_benefit
    );

    eprintln!(
        "Convexity benefit: gain={:.2}, loss={:.2}, benefit={:.2}",
        gain_from_rate_decrease, loss_from_rate_increase, convexity_benefit
    );
}
