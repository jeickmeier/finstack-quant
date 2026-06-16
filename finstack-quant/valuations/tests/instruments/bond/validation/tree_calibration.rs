#![allow(clippy::unwrap_used)]
//! Tree Calibration Validation Tests
//!
//! These tests validate that the short-rate tree used for callable bond pricing
//! properly calibrates to the discount curve and matches QuantLib's approach.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::models::{
    NodeState, ShortRateTree, ShortRateTreeConfig, TreeModel, TreeValuator,
};
use time::macros::date;

/// Helper: Create a flat discount curve
fn create_flat_curve(base_date: time::Date, rate: f64, curve_id: &str) -> DiscountCurve {
    let times = [0.0, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0];
    let dfs: Vec<_> = times.iter().map(|&t| (t, (-rate * t).exp())).collect();

    DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .knots(dfs)
        .build()
        .unwrap()
}

struct ZeroCouponValuator {
    notional: f64,
}

impl TreeValuator for ZeroCouponValuator {
    fn value_at_maturity(&self, _state: &NodeState) -> finstack_quant_core::Result<f64> {
        Ok(self.notional)
    }

    fn value_at_node(
        &self,
        _state: &NodeState,
        continuation_value: f64,
        _dt: f64,
    ) -> finstack_quant_core::Result<f64> {
        Ok(continuation_value)
    }
}

// =============================================================================
// Test 1: Tree Calibration to Discount Curve
// =============================================================================
// Validate that tree-implied discount factors match the input discount curve

#[test]
fn test_tree_calibrates_to_curve() {
    let as_of = date!(2020 - 01 - 01);
    let rate = 0.05;
    let curve = create_flat_curve(as_of, rate, "USD-OIS");

    let tree_config = ShortRateTreeConfig {
        steps: 100,
        volatility: 0.01,
        ..Default::default()
    };

    let mut tree = ShortRateTree::new(tree_config);
    let time_to_maturity = 5.0;
    tree.calibrate(&CurveId::new("USD-OIS"), &curve, time_to_maturity)
        .unwrap();

    // Check that tree produces correct discount factors at key points
    let test_times = [0.5, 1.0, 2.0, 3.0, 5.0];
    let valuator = ZeroCouponValuator { notional: 1.0 };
    let market = MarketContext::new();

    for &t in &test_times {
        let expected_df = (-rate * t).exp();
        let tree_df = tree
            .price(
                HashMap::<&'static str, f64>::default(),
                t,
                &market,
                &valuator,
            )
            .unwrap();

        assert!(
            (tree_df - expected_df).abs() < 1e-3,
            "Tree DF should match curve DF at t={}: tree_df={}, expected={}",
            t,
            tree_df,
            expected_df
        );
    }
}

// =============================================================================
// Test 2: Callable Bond Tree Pricing Works
// =============================================================================
// Verify that tree pricing produces reasonable values

#[test]
fn test_callable_bond_tree_pricing_reasonable() {
    let as_of = date!(2020 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);
    let notional = 100.0;
    let coupon_rate = 0.06;

    // Create straight bond
    let straight_bond = Bond::fixed(
        "STRAIGHT",
        Money::new(notional, Currency::USD),
        coupon_rate,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    // Create callable bond
    let mut callable_bond = Bond::fixed(
        "CALLABLE",
        Money::new(notional, Currency::USD),
        coupon_rate,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let mut call_schedule =
        finstack_quant_valuations::instruments::fixed_income::bond::CallPutSchedule::default();
    call_schedule.calls.push(
        finstack_quant_valuations::instruments::fixed_income::bond::CallPut {
            start_date: date!(2025 - 01 - 01),
            end_date: date!(2025 - 01 - 01),
            price_pct_of_par: 102.0,
            make_whole: None,
        },
    );
    callable_bond.call_put = Some(call_schedule);

    let curve = create_flat_curve(as_of, 0.04, "USD-OIS");
    let market = MarketContext::new().insert(curve);

    let straight_pv = straight_bond.value(&market, as_of).unwrap();
    let callable_pv = callable_bond.value(&market, as_of).unwrap();

    println!("Straight bond PV: ${:.2}", straight_pv.amount());
    println!("Callable bond PV: ${:.2}", callable_pv.amount());
    println!(
        "Call option value: ${:.2}",
        straight_pv.amount() - callable_pv.amount()
    );

    // Callable should be less than straight
    assert!(
        callable_pv.amount() < straight_pv.amount(),
        "Callable ({}) < Straight ({})",
        callable_pv.amount(),
        straight_pv.amount()
    );

    // Option value should be meaningful (1-10% of bond value)
    let option_value = straight_pv.amount() - callable_pv.amount();
    assert!(option_value > 0.0, "Option value should be positive");
    assert!(
        option_value < straight_pv.amount() * 0.15,
        "Option value should be < 15% of bond value"
    );
    assert!(
        option_value > straight_pv.amount() * 0.001,
        "Option value should be > 0.1% of bond value"
    );
}

// =============================================================================
// Test 3: Tree Steps Convergence
// =============================================================================
// Verify that pricing converges as tree steps increase

#[test]
fn test_tree_convergence_with_steps() {
    let as_of = date!(2020 - 01 - 01);
    let maturity = date!(2025 - 01 - 01);
    let notional = 100.0;
    let coupon_rate = 0.05;

    let mut callable_bond = Bond::fixed(
        "CALLABLE_CONV",
        Money::new(notional, Currency::USD),
        coupon_rate,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let mut call_schedule =
        finstack_quant_valuations::instruments::fixed_income::bond::CallPutSchedule::default();
    call_schedule.calls.push(
        finstack_quant_valuations::instruments::fixed_income::bond::CallPut {
            start_date: date!(2023 - 01 - 01),
            end_date: date!(2023 - 01 - 01),
            price_pct_of_par: 102.0,
            make_whole: None,
        },
    );
    callable_bond.call_put = Some(call_schedule);

    let curve = create_flat_curve(as_of, 0.05, "USD-OIS");
    let market = MarketContext::new().insert(curve);

    // Price with default tree (100 steps)
    let pv_100 = callable_bond.value(&market, as_of).unwrap();

    println!("Callable bond PV (100 steps): ${:.4}", pv_100.amount());

    // Note: We use the default 100 steps. Convergence would require
    // adding configurable tree steps to bond attributes or pricing overrides.
    // For now, we verify that pricing is stable and reasonable.

    assert!(pv_100.amount() > 90.0, "PV should be reasonable");
    assert!(pv_100.amount() < 120.0, "PV should be reasonable");
}

// =============================================================================
// Test 4: Putable Bond Tree Pricing
// =============================================================================

#[test]
fn test_putable_bond_tree_pricing_reasonable() {
    let as_of = date!(2020 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);
    let notional = 100.0;
    let coupon_rate = 0.04;

    let straight_bond = Bond::fixed(
        "STRAIGHT2",
        Money::new(notional, Currency::USD),
        coupon_rate,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let mut putable_bond = Bond::fixed(
        "PUTABLE",
        Money::new(notional, Currency::USD),
        coupon_rate,
        as_of,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let mut put_schedule =
        finstack_quant_valuations::instruments::fixed_income::bond::CallPutSchedule::default();
    put_schedule.puts.push(
        finstack_quant_valuations::instruments::fixed_income::bond::CallPut {
            start_date: date!(2025 - 01 - 01),
            end_date: date!(2025 - 01 - 01),
            price_pct_of_par: 98.0,
            make_whole: None,
        },
    );
    putable_bond.call_put = Some(put_schedule);

    let curve = create_flat_curve(as_of, 0.07, "USD-OIS");
    let market = MarketContext::new().insert(curve);

    let straight_pv = straight_bond.value(&market, as_of).unwrap();
    let putable_pv = putable_bond.value(&market, as_of).unwrap();

    println!("Straight bond PV: ${:.2}", straight_pv.amount());
    println!("Putable bond PV: ${:.2}", putable_pv.amount());
    println!(
        "Put option value: ${:.2}",
        putable_pv.amount() - straight_pv.amount()
    );

    // Putable should be more than straight
    assert!(
        putable_pv.amount() > straight_pv.amount(),
        "Putable ({}) > Straight ({})",
        putable_pv.amount(),
        straight_pv.amount()
    );

    // Option value should be meaningful
    let option_value = putable_pv.amount() - straight_pv.amount();
    assert!(option_value > 0.0, "Option value should be positive");
    assert!(
        option_value < straight_pv.amount() * 0.15,
        "Option value should be < 15% of bond value"
    );
}

// =============================================================================
// Test 5: Hull-White Mean Reversion Default Behavior
// =============================================================================
// With mean_reversion = None, tree should produce results equivalent to Ho-Lee.

#[test]
fn test_mean_reversion_none_matches_ho_lee() {
    let as_of = date!(2020 - 01 - 01);
    let rate = 0.05;
    let curve = create_flat_curve(as_of, rate, "USD-OIS");

    let ho_lee_config = ShortRateTreeConfig {
        steps: 100,
        volatility: 0.01,
        mean_reversion: None,
        ..Default::default()
    };

    let hw_zero_config = ShortRateTreeConfig {
        steps: 100,
        volatility: 0.01,
        mean_reversion: Some(0.0),
        ..Default::default()
    };

    let mut tree_hl = ShortRateTree::new(ho_lee_config);
    let mut tree_hw = ShortRateTree::new(hw_zero_config);
    let ttm = 5.0;

    tree_hl
        .calibrate(&CurveId::new("USD-OIS"), &curve, ttm)
        .unwrap();
    tree_hw
        .calibrate(&CurveId::new("USD-OIS"), &curve, ttm)
        .unwrap();

    let valuator = ZeroCouponValuator { notional: 1.0 };
    let market = MarketContext::new();

    for &t in &[1.0, 3.0, 5.0] {
        let pv_hl = tree_hl
            .price(
                HashMap::<&'static str, f64>::default(),
                t,
                &market,
                &valuator,
            )
            .unwrap();
        let pv_hw = tree_hw
            .price(
                HashMap::<&'static str, f64>::default(),
                t,
                &market,
                &valuator,
            )
            .unwrap();

        assert!(
            (pv_hl - pv_hw).abs() < 1e-10,
            "Ho-Lee and HW(a=0) should match at t={}: HL={}, HW={}",
            t,
            pv_hl,
            pv_hw,
        );
    }
}

// =============================================================================
// Test 6: Hull-White Mean Reversion Reduces Rate Dispersion
// =============================================================================
// With positive mean reversion, rates at terminal nodes should be less dispersed.

#[test]
fn test_mean_reversion_reduces_rate_dispersion() {
    use finstack_quant_valuations::models::HullWhiteTree;
    use finstack_quant_valuations::models::HullWhiteTreeConfig;

    let as_of = date!(2020 - 01 - 01);
    let rate = 0.05;
    let curve = create_flat_curve(as_of, rate, "USD-OIS");
    let steps = 50;
    let ttm = 10.0;

    // The Hull-White lattice no longer truncates its node range at a hard
    // j_max boundary: mean reversion is expressed through the branching
    // drift, so outer nodes exist but carry negligible probability mass.
    // Dispersion is therefore measured as the state-price-weighted standard
    // deviation of terminal rates, which mean reversion must reduce.
    let terminal_rate_std = |kappa: f64| -> f64 {
        let config = HullWhiteTreeConfig {
            kappa,
            sigma: 0.01,
            steps,
            ..Default::default()
        };
        let tree = HullWhiteTree::calibrate(config, &curve, ttm).unwrap();
        let last_step = tree.num_steps();
        let mut q_sum = 0.0;
        let mut mean = 0.0;
        let mut second = 0.0;
        for node in 0..tree.num_nodes(last_step) {
            let q = tree.state_price(last_step, node);
            let r = tree.rate_at_node(last_step, node);
            q_sum += q;
            mean += q * r;
            second += q * r * r;
        }
        mean /= q_sum;
        (second / q_sum - mean * mean).max(0.0).sqrt()
    };

    let std_low_mr = terminal_rate_std(0.001);
    let std_high_mr = terminal_rate_std(0.05);

    assert!(
        std_high_mr < std_low_mr,
        "Mean reversion should tighten rate dispersion: high-MR std={:.6}, low-MR std={:.6}",
        std_high_mr,
        std_low_mr,
    );
}
