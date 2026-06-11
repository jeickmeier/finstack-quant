// Unit tests for Hull-White tree construction and calibration.

use crate::models::trees::{HullWhiteTree, HullWhiteTreeConfig};
use finstack_core::dates::Date;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::interp::InterpStyle;
use time::Month;

fn test_discount_curve() -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"))
        .knots([
            (0.0, 1.0),
            (0.5, 0.985),
            (1.0, 0.97),
            (2.0, 0.94),
            (5.0, 0.85),
            (10.0, 0.70),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("Valid curve")
}

#[test]
fn test_tree_calibration_basic() {
    let config = HullWhiteTreeConfig::new(0.03, 0.01, 50);
    let curve = test_discount_curve();

    let tree = HullWhiteTree::calibrate(config, &curve, 5.0);
    assert!(tree.is_ok(), "Tree calibration should succeed");

    let tree = tree.unwrap();
    assert_eq!(tree.num_steps(), 50);
}

#[test]
fn test_tree_calibration_preserves_discount_factors() {
    // Use 200 steps for production-quality < 1 bp accuracy
    let config = HullWhiteTreeConfig::new(0.03, 0.01, 200);
    let curve = test_discount_curve();

    let tree = HullWhiteTree::calibrate(config, &curve, 5.0).expect("Calibration should succeed");

    // State prices should sum to discount factors at each step
    // Industry standard: calibration error < 1 basis point (0.0001)
    for step in [40, 100, 200] {
        let t = tree.time_at_step(step);
        let target_df = curve.df(t);
        let sum_q: f64 = (0..tree.num_nodes(step))
            .map(|j| tree.state_price(step, j))
            .sum();

        let error = (sum_q - target_df).abs();
        let error_bps = (error / target_df) * 10000.0;

        // Production tolerance: < 1 basis point
        assert!(
            error_bps < 1.0,
            "State price calibration error {:.6} ({:.4} bps) exceeds 1 bp at step {} (t={:.2})",
            error,
            error_bps,
            step,
            t
        );
    }
}

#[test]
fn test_tree_bond_price_at_maturity() {
    // Use 200 steps for production-quality < 1 bp accuracy
    let config = HullWhiteTreeConfig::new(0.03, 0.01, 200);
    let curve = test_discount_curve();

    let tree = HullWhiteTree::calibrate(config, &curve, 2.0).expect("Calibration should succeed");
    let final_step = tree.num_steps();

    // Bond price at maturity should be 1.0 (exactly)
    // Industry standard: < 1 basis point error for bond prices
    for node in 0..tree.num_nodes(final_step) {
        let bp = tree.bond_price(final_step, node, 2.0, &curve);
        let error_bps = (bp - 1.0).abs() * 10000.0;
        assert!(
            error_bps < 1.0, // < 1 bp tolerance
            "Bond price at maturity should be 1.0, got {:.8} (error: {:.4} bps) at node {}",
            bp,
            error_bps,
            node
        );
    }
}

#[test]
fn test_tree_forward_swap_rate() {
    // Use 200 steps for production-quality < 1 bp accuracy
    let config = HullWhiteTreeConfig::new(0.03, 0.01, 200);
    let curve = test_discount_curve();

    let tree = HullWhiteTree::calibrate(config, &curve, 5.0).expect("Calibration should succeed");

    // Payment times for a 5Y swap with semi-annual payments
    let payment_times: Vec<f64> = (1..=10).map(|i| i as f64 * 0.5).collect();
    let accrual_fractions = vec![0.5; 10];

    // Forward swap rate at t=0 should match market forward rate
    let swap_rate_at_root = tree.forward_swap_rate(
        0,
        0,
        0.0, // swap start
        5.0, // swap end
        &payment_times,
        &accrual_fractions,
        &curve,
    );

    // Calculate expected market forward rate
    let df_start = curve.df(0.0);
    let df_end = curve.df(5.0);
    let annuity: f64 = payment_times
        .iter()
        .zip(accrual_fractions.iter())
        .map(|(&t, &tau)| tau * curve.df(t))
        .sum();
    let market_forward = (df_start - df_end) / annuity;

    // Production standard: swap rate accuracy < 1 basis point (0.0001)
    let error_bps = (swap_rate_at_root - market_forward).abs() * 10000.0;
    assert!(
        error_bps < 1.0, // < 1 bp tolerance
        "Tree forward rate {:.8} should match market forward {:.8} (error: {:.4} bps)",
        swap_rate_at_root,
        market_forward,
        error_bps
    );
}

#[test]
fn test_tree_backward_induction_unit_payoff() {
    // Use 200 steps for production-quality < 1 bp accuracy
    let config = HullWhiteTreeConfig::new(0.03, 0.01, 200);
    let curve = test_discount_curve();

    let tree = HullWhiteTree::calibrate(config, &curve, 1.0).expect("Calibration should succeed");
    let final_step = tree.num_steps();

    // Unit payoff at all terminal nodes should give approximately the discount factor
    let terminal = vec![1.0; tree.num_nodes(final_step)];
    let value = tree
        .backward_induction(&terminal, |_, _, cont| cont)
        .expect("terminal values sized to final step");

    let target_df = curve.df(1.0);
    let error = (value - target_df).abs();
    let error_bps = (error / target_df) * 10000.0;

    // Production standard: pricing error < 1 basis point
    assert!(
        error_bps < 1.0,
        "Unit payoff value {:.8} should match df {:.8} (error: {:.4} bps)",
        value,
        target_df,
        error_bps
    );
}

#[test]
fn test_tree_backward_induction_zero_payoff() {
    let config = HullWhiteTreeConfig::new(0.03, 0.01, 20);
    let curve = test_discount_curve();

    let tree = HullWhiteTree::calibrate(config, &curve, 1.0).expect("Calibration should succeed");

    // Zero payoff should give zero value
    let terminal = vec![0.0; tree.num_nodes(20)];
    let value = tree
        .backward_induction(&terminal, |_, _, cont| cont)
        .expect("terminal values sized to final step");

    assert!(value.abs() < 1e-10, "Zero payoff should give zero value");
}

#[test]
fn test_tree_time_to_step_mapping() {
    let config = HullWhiteTreeConfig::new(0.03, 0.01, 200);
    let curve = test_discount_curve();

    let tree = HullWhiteTree::calibrate(config, &curve, 10.0).expect("Calibration should succeed");

    // Check time-to-step mapping
    assert_eq!(tree.time_to_step(0.0), 0);
    assert_eq!(tree.time_to_step(5.0), 100);
    assert_eq!(tree.time_to_step(10.0), 200);

    // Edge cases
    assert_eq!(tree.time_to_step(-1.0), 0);
    assert_eq!(tree.time_to_step(15.0), 200); // Clamped to max
}

#[test]
fn test_tree_different_parameters() {
    let curve = test_discount_curve();

    // Higher mean reversion - use 200 steps for < 1 bp accuracy
    let config_high_kappa = HullWhiteTreeConfig::new(0.10, 0.01, 200);
    let tree_high_kappa =
        HullWhiteTree::calibrate(config_high_kappa, &curve, 5.0).expect("Should succeed");

    // Lower mean reversion
    let config_low_kappa = HullWhiteTreeConfig::new(0.01, 0.01, 200);
    let tree_low_kappa =
        HullWhiteTree::calibrate(config_low_kappa, &curve, 5.0).expect("Should succeed");

    // Both should calibrate to same discount factors
    // Production standard: calibration should be parameter-independent to < 1 bp
    let t = 2.5;
    let step = tree_high_kappa.time_to_step(t);
    let target_df = curve.df(t);

    let sum_high: f64 = (0..tree_high_kappa.num_nodes(step))
        .map(|j| tree_high_kappa.state_price(step, j))
        .sum();

    let sum_low: f64 = (0..tree_low_kappa.num_nodes(step))
        .map(|j| tree_low_kappa.state_price(step, j))
        .sum();

    let error_high_bps = ((sum_high - target_df).abs() / target_df) * 10000.0;
    let error_low_bps = ((sum_low - target_df).abs() / target_df) * 10000.0;

    assert!(
        error_high_bps < 1.0,
        "High kappa tree should match df (error: {:.4} bps)",
        error_high_bps
    );
    assert!(
        error_low_bps < 1.0,
        "Low kappa tree should match df (error: {:.4} bps)",
        error_low_bps
    );
}

#[test]
fn test_tree_higher_volatility() {
    let curve = test_discount_curve();

    // Low volatility - use 200 steps for production quality
    let config_low_vol = HullWhiteTreeConfig::new(0.03, 0.005, 200);
    let tree_low = HullWhiteTree::calibrate(config_low_vol, &curve, 5.0).expect("Should succeed");

    // High volatility
    let config_high_vol = HullWhiteTreeConfig::new(0.03, 0.02, 200);
    let tree_high = HullWhiteTree::calibrate(config_high_vol, &curve, 5.0).expect("Should succeed");

    // Higher volatility should lead to more spread in rates
    let step = 100; // mid-point at 200 steps
    let low_rate_spread =
        tree_low.rate_at_node(step, tree_low.num_nodes(step) - 1) - tree_low.rate_at_node(step, 0);
    let high_rate_spread = tree_high.rate_at_node(step, tree_high.num_nodes(step) - 1)
        - tree_high.rate_at_node(step, 0);

    assert!(
        high_rate_spread > low_rate_spread,
        "Higher vol should give wider rate spread"
    );
}
