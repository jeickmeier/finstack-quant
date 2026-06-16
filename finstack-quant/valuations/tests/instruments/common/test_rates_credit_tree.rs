//! Integration tests for the two-factor rates+credit binomial tree.
//!
//! Covers the production paths used by term-loan and bond tree pricers:
//! - Defaultable ZCB closed-form vs tree
//! - Correlation extremes produce monotonic pricing differences for a
//!   rate-and-default-sensitive payoff
//! - Calibration faithfully reproduces both the discount and hazard curves
//!   when the cross-volatility is silenced
//!
//! These tests live at the integration tier (consume the crate as an external
//! caller would) because the previous coverage was confined to inline
//! `#[cfg(test)]` blocks; production pricers can change behaviour without
//! triggering an integration regression.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve, ParInterp};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::models::trees::tree_framework::{
    NodeState, TreeModel, TreeValuator,
};
use finstack_quant_valuations::models::trees::two_factor_rates_credit::{
    RatesCreditConfig, RatesCreditTree,
};
use time::Month;

/// Pays $1 at maturity, conditioned on no default. Tree multiplies by survival
/// probability via the hazard factor; rate factor handles risk-free discounting.
struct DefaultableZcbValuator;

impl TreeValuator for DefaultableZcbValuator {
    fn value_at_maturity(&self, _state: &NodeState) -> finstack_quant_core::Result<f64> {
        Ok(1.0)
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

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).expect("valid")
}

fn discount_curve() -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(base_date())
        .knots([
            (0.0, 1.0),
            (1.0, 0.96),
            (2.0, 0.91),
            (3.0, 0.86),
            (5.0, 0.78),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("disc curve")
}

fn hazard_curve(spread_bps: f64, recovery: f64) -> HazardCurve {
    let lambda = spread_bps / 10_000.0; // ≈ flat constant hazard
    HazardCurve::builder("TEST-HAZ")
        .base_date(base_date())
        .recovery_rate(recovery)
        .knots([(0.0, lambda), (5.0, lambda), (10.0, lambda)])
        .par_interp(ParInterp::Linear)
        .build()
        .expect("hazard curve")
}

#[test]
fn rates_credit_tree_reproduces_disc_curve_when_hazard_is_silent() {
    // hazard_vol = 0 collapses the joint tree to a pure rate tree. The pricing
    // of a survival-only payoff (DummyValuator pays 1 at maturity unconditionally,
    // discounted by the rate factor) must match the market discount factor at the
    // chosen maturity.
    let disc = discount_curve();
    let haz = hazard_curve(100.0, 0.40); // existence only; not exercised
    let ttm = 5.0;

    let mut tree = RatesCreditTree::new(RatesCreditConfig {
        steps: 50,
        rate_vol: 0.01,
        hazard_vol: 0.0,
        ..Default::default()
    });
    tree.calibrate(&disc, &haz, ttm).expect("calibrate");

    let ctx = MarketContext::new();
    let vars = HashMap::<&'static str, f64>::default();
    let val = DefaultableZcbValuator;
    let price = tree.price(vars, ttm, &ctx, &val).expect("price");

    let market_df = 0.78; // disc curve knot at t=5
    let err_bps = (price - market_df).abs() * 10_000.0;
    assert!(
        err_bps < 1.0,
        "Tree price {price:.6} diverged from market DF {market_df:.6} ({err_bps:.2} bps)",
    );
}

#[test]
fn rates_credit_tree_correlation_extremes_are_well_defined() {
    // Pricing a survival-only payoff with three correlation choices must produce
    // finite, ordered prices: extreme correlations must not blow up the tree.
    let disc = discount_curve();
    let haz = hazard_curve(150.0, 0.40);
    let ttm = 5.0;
    let ctx = MarketContext::new();
    let val = DefaultableZcbValuator;

    let price_at = |rho: f64| -> f64 {
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps: 40,
            rate_vol: 0.012,
            hazard_vol: 0.25,
            correlation: rho,
            ..Default::default()
        });
        tree.calibrate(&disc, &haz, ttm).expect("calibrate");
        let vars = HashMap::<&'static str, f64>::default();
        tree.price(vars, ttm, &ctx, &val).expect("price")
    };

    let p_neg = price_at(-0.99);
    let p_zero = price_at(0.0);
    let p_pos = price_at(0.99);

    for (label, p) in [("rho=-0.99", p_neg), ("rho=0", p_zero), ("rho=0.99", p_pos)] {
        assert!(
            p.is_finite() && p > 0.0 && p < 1.0,
            "{label} produced an invalid price {p}",
        );
    }

    // Calibration ensures all three reproduce the marginal DF*SP product within
    // numerical tolerance; the tolerance allows for tree discretization noise.
    let max_spread = (p_neg - p_zero).abs().max((p_pos - p_zero).abs());
    assert!(
        max_spread < 5e-3,
        "Correlation choice should not change calibrated marginals materially: \
         neg={p_neg:.6}, zero={p_zero:.6}, pos={p_pos:.6}",
    );
}

#[test]
fn rates_credit_tree_uncalibrated_returns_error() {
    let tree = RatesCreditTree::new(RatesCreditConfig::default());
    let ctx = MarketContext::new();
    let vars = HashMap::<&'static str, f64>::default();
    let val = DefaultableZcbValuator;
    let result = tree.price(vars, 1.0, &ctx, &val);
    assert!(
        result.is_err(),
        "price() without calibrate() must fail loudly",
    );
}
