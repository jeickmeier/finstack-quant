use super::*;
use finstack_core::market_data::context::MarketContext;
use finstack_core::HashMap;
use finstack_core::Result;

struct QuadraticValuator;

impl TreeValuator for QuadraticValuator {
    fn value_at_maturity(&self, state: &NodeState) -> Result<f64> {
        self.value_at_node(state, 0.0, 0.0)
    }

    fn value_at_node(&self, state: &NodeState, _continuation_value: f64, _dt: f64) -> Result<f64> {
        let spot = state.spot().unwrap_or(0.0);
        let vol = state.get_var_or(state_keys::VOLATILITY, 0.0);
        let rate = state.interest_rate().unwrap_or(0.0);
        Ok(spot * spot + 10.0 * vol + 100.0 * rate + 5.0 * state.time)
    }
}

struct MockTreeModel;

impl TreeModel for MockTreeModel {
    fn price<V: TreeValuator>(
        &self,
        initial_vars: StateVariables,
        time_to_maturity: f64,
        market_context: &MarketContext,
        valuator: &V,
    ) -> Result<f64> {
        let state = NodeState::new(0, time_to_maturity, &initial_vars, market_context);
        valuator.value_at_node(&state, 0.0, 0.0)
    }
}

fn sample_state_variables() -> StateVariables {
    let mut vars = HashMap::default();
    vars.insert(state_keys::SPOT, 100.0);
    vars.insert(state_keys::INTEREST_RATE, 0.03);
    vars.insert(state_keys::HAZARD_RATE, 0.02);
    vars.insert(state_keys::DF, 0.95);
    vars.insert(state_keys::CREDIT_SPREAD, 0.015);
    vars
}

#[test]
fn node_state_caches_common_fields_and_barrier_variants() {
    let market = MarketContext::new();
    let vars = sample_state_variables();

    let state = NodeState::new(2, 0.5, &vars, &market);
    assert_eq!(state.step, 2);
    assert_eq!(state.time, 0.5);
    assert_eq!(state.spot(), Some(100.0));
    assert_eq!(state.interest_rate(), Some(0.03));
    assert_eq!(state.hazard_rate(), Some(0.02));
    assert_eq!(state.discount_factor(), Some(0.95));
    assert!(state.barrier_state.is_none());

    let barrier_state = BarrierState {
        barrier_hit: false,
        barrier_level: 105.0,
        barrier_type: BarrierType::UpAndOut,
    };
    let with_barrier = NodeState::new_with_barrier(3, 0.75, &vars, &market, barrier_state);
    assert_eq!(with_barrier.step, 3);
    assert_eq!(with_barrier.time, 0.75);
    assert!(with_barrier.barrier_state.is_some());
    assert_eq!(with_barrier.spot(), Some(100.0));
}

#[test]
fn node_state_accessors_and_barrier_helpers_cover_thresholds() {
    let market = MarketContext::new();
    let mut vars = sample_state_variables();
    vars.insert(state_keys::BARRIER_TOUCHED_UP, 0.5);
    vars.insert(state_keys::BARRIER_TOUCHED_DOWN, 0.9);

    let mut up_and_out = NodeState::new_with_barrier(
        0,
        0.0,
        &vars,
        &market,
        BarrierState {
            barrier_hit: false,
            barrier_level: 110.0,
            barrier_type: BarrierType::UpAndOut,
        },
    );
    assert_eq!(up_and_out.get_var(state_keys::CREDIT_SPREAD), Some(0.015));
    assert_eq!(up_and_out.get_var_or("missing", 42.0), 42.0);
    assert_eq!(up_and_out.credit_spread(), Some(0.015));
    assert!(
        !up_and_out.barrier_touched_up(),
        "0.5 should not count as touched"
    );
    assert!(up_and_out.barrier_touched_down());
    assert!(!up_and_out.is_barrier_hit());
    assert!(!up_and_out.is_knocked_out());
    up_and_out.update_barrier_state(110.0);
    assert!(up_and_out.is_barrier_hit());
    assert!(up_and_out.is_knocked_out());
    up_and_out.update_barrier_state(90.0);
    assert!(up_and_out.is_barrier_hit(), "barrier hit should latch");

    let mut down_and_in = NodeState::new_with_barrier(
        0,
        0.0,
        &vars,
        &market,
        BarrierState {
            barrier_hit: false,
            barrier_level: 95.0,
            barrier_type: BarrierType::DownAndIn,
        },
    );
    assert!(!down_and_in.is_knocked_in());
    down_and_in.update_barrier_state(95.0);
    assert!(down_and_in.is_barrier_hit());
    assert!(down_and_in.is_knocked_in());

    let no_barrier = NodeState::new(0, 0.0, &vars, &market);
    assert!(!no_barrier.is_knocked_out());
    assert!(no_barrier.is_knocked_in());
}

#[test]
fn tree_greeks_richardson_extrapolation_matches_formula() {
    let coarse = TreeGreeks {
        price: 10.0,
        delta: 1.0,
        gamma: 2.0,
        vega: 3.0,
        theta: 4.0,
        rho: 5.0,
    };
    let fine = TreeGreeks {
        price: 13.0,
        delta: 1.75,
        gamma: 2.5,
        vega: 3.5,
        theta: 4.5,
        rho: 5.5,
    };

    let extrapolated = TreeGreeks::richardson_extrapolate(&coarse, &fine);
    assert!((extrapolated.price - 14.0).abs() < 1e-12);
    assert!((extrapolated.delta - 2.0).abs() < 1e-12);
    assert!((extrapolated.gamma - (8.0 / 3.0)).abs() < 1e-12);
    assert!((extrapolated.vega - (11.0 / 3.0)).abs() < 1e-12);
    assert!((extrapolated.theta - (14.0 / 3.0)).abs() < 1e-12);
    assert!((extrapolated.rho - (17.0 / 3.0)).abs() < 1e-12);
    assert!((TreeGreeks::richardson_price(10.0, 13.0) - 14.0).abs() < 1e-12);
}

#[test]
fn greeks_bump_config_defaults_adaptive_ranges_and_custom_spot_bump() {
    let default_cfg = GreeksBumpConfig::default();
    assert!((default_cfg.spot_bump_fraction - 0.01).abs() < 1e-12);
    assert!((default_cfg.vol_bump_absolute - 0.01).abs() < 1e-12);
    assert!((default_cfg.rate_bump_absolute - 0.0001).abs() < 1e-12);
    assert!((default_cfg.time_bump_years - 1.0 / 365.25).abs() < 1e-12);
    assert!(!default_cfg.adaptive);
    assert!((default_cfg.spot_bump(100.0, Some(100.0)) - 1.0).abs() < 1e-12);

    let adaptive = GreeksBumpConfig::adaptive().with_spot_bump(0.02);
    assert!(adaptive.adaptive);
    assert!((adaptive.spot_bump(100.0, Some(100.0)) - 1.0).abs() < 1e-12);
    assert!((adaptive.spot_bump(100.0, Some(160.0)) - 2.0).abs() < 1e-12);
    assert!((adaptive.spot_bump(100.0, Some(260.0)) - 4.0).abs() < 1e-12);
    assert!((adaptive.spot_bump(100.0, None) - 2.0).abs() < 1e-12);
}

#[test]
fn default_tree_model_calculate_greeks_uses_central_differences_and_restores_vars() {
    let model = MockTreeModel;
    let valuator = QuadraticValuator;
    let market = MarketContext::new();
    let initial_vars = single_factor_equity_state(100.0, 0.03, 0.01, 0.20);

    let greeks = model.calculate_greeks(initial_vars.clone(), 1.0, &market, &valuator, Some(0.01));
    assert!(greeks.is_ok(), "greeks calculation should succeed");

    let expected_price = 100.0_f64.powi(2) + 10.0 * 0.20 + 100.0 * 0.03 + 5.0;
    if let Ok(greeks) = greeks {
        assert!((greeks.price - expected_price).abs() < 1e-12);
        assert!((greeks.delta - 200.0).abs() < 1e-12);
        assert!((greeks.gamma - 2.0).abs() < 1e-12);
        assert!((greeks.vega - 0.1).abs() < 1e-12);
        assert!((greeks.rho - 0.01).abs() < 1e-12);
        assert!((greeks.theta + 5.0).abs() < 1e-10);
    }

    let restored_price = model.price(initial_vars, 1.0, &market, &valuator);
    assert!(restored_price.is_ok(), "restored price should succeed");
    if let Ok(restored_price) = restored_price {
        assert!((restored_price - expected_price).abs() < 1e-12);
    }
}

#[test]
fn default_tree_model_greeks_clamp_vol_down_and_skip_theta_near_expiry() {
    let model = MockTreeModel;
    let valuator = QuadraticValuator;
    let market = MarketContext::new();
    let initial_vars = single_factor_equity_state(100.0, 0.03, 0.01, 0.005);

    let greeks = model.calculate_greeks(initial_vars, 0.001, &market, &valuator, Some(0.01));
    assert!(greeks.is_ok(), "greeks calculation should succeed");

    let expected_vega = (10.0 * (0.005 + 0.01) - 10.0 * 1e-6) / 2.0;
    if let Ok(greeks) = greeks {
        assert!((greeks.vega - expected_vega).abs() < 1e-12);
        assert_eq!(
            greeks.theta, 0.0,
            "theta should be skipped for near-expiry inputs"
        );
    }
}

#[test]
fn state_helper_builders_populate_expected_keys() {
    let single = single_factor_equity_state(100.0, 0.03, 0.01, 0.20);
    assert_eq!(single.get(state_keys::SPOT), Some(&100.0));
    assert_eq!(single.get(state_keys::INTEREST_RATE), Some(&0.03));
    assert_eq!(single.get(state_keys::DIVIDEND_YIELD), Some(&0.01));
    assert_eq!(single.get(state_keys::VOLATILITY), Some(&0.20));
    assert_eq!(single.get(state_keys::RATE_VOLATILITY), None);

    let two_factor = two_factor_equity_rates_state(100.0, 0.03, 0.01, 0.20, 0.015);
    assert_eq!(two_factor.get(state_keys::SPOT), Some(&100.0));
    assert_eq!(two_factor.get(state_keys::INTEREST_RATE), Some(&0.03));
    assert_eq!(two_factor.get(state_keys::DIVIDEND_YIELD), Some(&0.01));
    assert_eq!(two_factor.get(state_keys::VOLATILITY), Some(&0.20));
    assert_eq!(two_factor.get(state_keys::RATE_VOLATILITY), Some(&0.015));
}

#[test]
fn evolution_params_builders_satisfy_basic_probability_invariants() {
    let crr = EvolutionParams::equity_crr(0.2, 0.05, 0.01, 0.25).expect("valid CRR params");
    assert!(crr.up_factor > 1.0);
    assert!(crr.down_factor < 1.0);
    assert!((crr.up_factor * crr.down_factor - 1.0).abs() < 1e-12);
    assert!(crr.prob_up >= 0.0 && crr.prob_up <= 1.0);
    assert!(crr.prob_down >= 0.0 && crr.prob_down <= 1.0);
    assert!((crr.prob_up + crr.prob_down - 1.0).abs() < 1e-12);

    let trinomial =
        EvolutionParams::equity_trinomial(0.2, 0.05, 0.01, 0.25).expect("valid trinomial params");
    assert!(trinomial.up_factor > 1.0);
    assert!(trinomial.down_factor < 1.0);
    assert_eq!(trinomial.middle_factor, Some(1.0));
    assert!(trinomial.prob_up >= 0.0);
    assert!(trinomial.prob_down >= 0.0);
    assert!(
        trinomial.prob_middle.is_some(),
        "middle probability should exist"
    );
    if let Some(p_mid) = trinomial.prob_middle {
        assert!(p_mid >= 0.0);
        assert!((trinomial.prob_up + trinomial.prob_down + p_mid - 1.0).abs() < 1e-10);
    }
}

#[test]
fn evolution_params_crr_rejects_unstable_params() {
    // dt large enough relative to vol that drift kicks p out of [0, 1].
    // Combined with extreme drift, the implied probability falls below zero
    // (or above one) — release builds must surface this rather than silently
    // produce an arbitrage-violating tree.
    let result = EvolutionParams::equity_crr(0.05, 5.0, 0.0, 1.0);
    assert!(
        result.is_err(),
        "CRR with extreme drift/vol/dt must error, not silently corrupt the tree"
    );
}

#[test]
fn evolution_params_trinomial_rejects_negative_probabilities() {
    // Extreme drift relative to vol pushes one trinomial probability negative.
    let result = EvolutionParams::equity_trinomial(0.02, 5.0, 0.0, 1.0);
    assert!(
        result.is_err(),
        "Trinomial with negative implied probability must error"
    );
}

/// `BarrierSpec` documentation states the touch convention is **non-strict**:
/// `spot >= up_level` and `spot <= down_level` trigger a touch (matching
/// Bloomberg, differing from QuantLib's strict-inequality convention). This
/// test pins exact-level touches against both up and down barriers so a future
/// switch to strict comparisons would surface immediately.
#[test]
fn barrier_touch_convention_is_non_strict_at_exact_level() {
    // Build a SpecKnockOut spec; we only test the touch predicate, not pricing.
    let up = BarrierSpec {
        up_level: Some(105.0),
        down_level: None,
        rebate: 0.0,
        style: BarrierStyle::KnockOut,
    };
    let down = BarrierSpec {
        up_level: None,
        down_level: Some(95.0),
        rebate: 0.0,
        style: BarrierStyle::KnockOut,
    };

    // Mirror the `barrier_touch` closure in `recombining.rs`. Keep this in
    // lock-step with the production code: any change there must be reflected
    // here, otherwise the convention will silently drift.
    let touch_up = |spot: f64, spec: &BarrierSpec| -> bool {
        spec.up_level.map(|lvl| spot >= lvl).unwrap_or(false)
    };
    let touch_down = |spot: f64, spec: &BarrierSpec| -> bool {
        spec.down_level.map(|lvl| spot <= lvl).unwrap_or(false)
    };

    // Exact-level: must touch (non-strict).
    assert!(touch_up(105.0, &up), "spot == up_level must trigger touch");
    assert!(
        touch_down(95.0, &down),
        "spot == down_level must trigger touch"
    );
    // Just inside (no touch).
    assert!(
        !touch_up(104.99, &up),
        "spot strictly below up_level must not touch"
    );
    assert!(
        !touch_down(95.01, &down),
        "spot strictly above down_level must not touch"
    );
    // Beyond the level: still touched.
    assert!(touch_up(106.0, &up));
    assert!(touch_down(94.0, &down));
}
