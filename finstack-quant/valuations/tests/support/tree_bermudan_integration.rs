//! Tests for the surrounding crate component and its documented behavior.
//!
// Unit tests for Hull-White tree construction and calibration.

use crate::models::trees::{HullWhiteTree, HullWhiteTreeConfig};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
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
    let config = HullWhiteTreeConfig::new(0.03, 0.01, 50);
    let curve = test_discount_curve();

    let tree = HullWhiteTree::calibrate(config, &curve, 5.0).expect("Calibration should succeed");

    // State prices should sum to discount factors at each step
    for step in [10, 25, 50] {
        let t = tree.time_at_step(step);
        let target_df = curve.df(t);
        let sum_q: f64 = (0..tree.num_nodes(step))
            .map(|j| tree.state_price(step, j))
            .sum();

        assert!(
            (sum_q - target_df).abs() < 0.01,
            "State price sum {} != target df {} at step {} (t={})",
            sum_q,
            target_df,
            step,
            t
        );
    }
}

#[test]
fn test_tree_bond_price_at_maturity() {
    let config = HullWhiteTreeConfig::new(0.03, 0.01, 20);
    let curve = test_discount_curve();

    let tree = HullWhiteTree::calibrate(config, &curve, 2.0).expect("Calibration should succeed");

    // Bond price at maturity should be approximately 1.0
    for node in 0..tree.num_nodes(20) {
        let bp = tree.bond_price(20, node, 2.0, &curve);
        assert!(
            (bp - 1.0).abs() < 0.05,
            "Bond price at maturity should be ~1.0, got {} at node {}",
            bp,
            node
        );
    }
}

#[test]
fn test_tree_forward_swap_rate() {
    let config = HullWhiteTreeConfig::new(0.03, 0.01, 50);
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

    assert!(
        (swap_rate_at_root - market_forward).abs() < 0.005,
        "Tree forward rate {} should match market forward {}",
        swap_rate_at_root,
        market_forward
    );
}

#[test]
fn test_tree_backward_induction_unit_payoff() {
    let config = HullWhiteTreeConfig::new(0.03, 0.01, 20);
    let curve = test_discount_curve();

    let tree = HullWhiteTree::calibrate(config, &curve, 1.0).expect("Calibration should succeed");

    // Unit payoff at all terminal nodes should give approximately the discount factor
    let terminal = vec![1.0; tree.num_nodes(20)];
    let value = tree
        .backward_induction(&terminal, |_, _, cont| cont)
        .expect("terminal values sized to final step");

    let target_df = curve.df(1.0);
    assert!(
        (value - target_df).abs() < 0.02,
        "Unit payoff value {} should be close to df {}",
        value,
        target_df
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
    let config = HullWhiteTreeConfig::new(0.03, 0.01, 100);
    let curve = test_discount_curve();

    let tree = HullWhiteTree::calibrate(config, &curve, 10.0).expect("Calibration should succeed");

    // Check time-to-step mapping
    assert_eq!(tree.time_to_step(0.0), 0);
    assert_eq!(tree.time_to_step(5.0), 50);
    assert_eq!(tree.time_to_step(10.0), 100);

    // Edge cases
    assert_eq!(tree.time_to_step(-1.0), 0);
    assert_eq!(tree.time_to_step(15.0), 100); // Clamped to max
}

#[test]
fn test_tree_different_parameters() {
    let curve = test_discount_curve();

    // Higher mean reversion
    let config_high_kappa = HullWhiteTreeConfig::new(0.10, 0.01, 50);
    let tree_high_kappa =
        HullWhiteTree::calibrate(config_high_kappa, &curve, 5.0).expect("Should succeed");

    // Lower mean reversion
    let config_low_kappa = HullWhiteTreeConfig::new(0.01, 0.01, 50);
    let tree_low_kappa =
        HullWhiteTree::calibrate(config_low_kappa, &curve, 5.0).expect("Should succeed");

    // Both should calibrate to same discount factors
    let t = 2.5;
    let step = tree_high_kappa.time_to_step(t);
    let target_df = curve.df(t);

    let sum_high: f64 = (0..tree_high_kappa.num_nodes(step))
        .map(|j| tree_high_kappa.state_price(step, j))
        .sum();

    let sum_low: f64 = (0..tree_low_kappa.num_nodes(step))
        .map(|j| tree_low_kappa.state_price(step, j))
        .sum();

    assert!(
        (sum_high - target_df).abs() < 0.01,
        "High kappa tree should match df"
    );
    assert!(
        (sum_low - target_df).abs() < 0.01,
        "Low kappa tree should match df"
    );
}

#[test]
fn test_tree_higher_volatility() {
    let curve = test_discount_curve();

    // Low volatility
    let config_low_vol = HullWhiteTreeConfig::new(0.03, 0.005, 50);
    let tree_low = HullWhiteTree::calibrate(config_low_vol, &curve, 5.0).expect("Should succeed");

    // High volatility
    let config_high_vol = HullWhiteTreeConfig::new(0.03, 0.02, 50);
    let tree_high = HullWhiteTree::calibrate(config_high_vol, &curve, 5.0).expect("Should succeed");

    // Higher volatility should lead to more spread in rates
    let step = 25;
    let low_rate_spread =
        tree_low.rate_at_node(step, tree_low.num_nodes(step) - 1) - tree_low.rate_at_node(step, 0);
    let high_rate_spread = tree_high.rate_at_node(step, tree_high.num_nodes(step) - 1)
        - tree_high.rate_at_node(step, 0);

    assert!(
        high_rate_spread > low_rate_spread,
        "Higher vol should give wider rate spread"
    );
}
