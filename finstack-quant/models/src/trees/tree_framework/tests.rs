//! Shared node, evolution, and backward-induction components for pricing trees.
//!
use super::*;
use finstack_quant_core::Result;

/// Valuator recording the node state it is handed.
struct StateProbe {
    seen: std::sync::Mutex<Vec<NodeState>>,
}

impl TreeValuator for StateProbe {
    fn value_at_maturity(&self, state: &NodeState) -> Result<f64> {
        self.record(state);
        Ok(0.0)
    }

    fn value_at_node(&self, state: &NodeState, continuation_value: f64, _dt: f64) -> Result<f64> {
        self.record(state);
        Ok(continuation_value)
    }
}

impl StateProbe {
    fn record(&self, state: &NodeState) {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(*state);
        }
    }

    fn price(lattice: RecombiningLattice<'_>) -> Vec<NodeState> {
        let probe = Self {
            seen: std::sync::Mutex::new(Vec::new()),
        };
        price_recombining_tree(RecombiningInputs {
            steps: 1,
            time_to_maturity: 1.0,
            valuator: &probe,
            prob_up: 0.5,
            prob_down: 0.5,
            lattice,
        })
        .expect("price");
        probe.seen.into_inner().expect("probe states")
    }
}

#[test]
fn spot_lattice_hands_spot_and_flat_rate_to_the_valuator() {
    let seen = StateProbe::price(RecombiningLattice::Spot {
        spot: 100.0,
        up_factor: 2.0,
        down_factor: 0.5,
        interest_rate: 0.03,
    });
    let node = |step: usize, spot: f64| NodeState {
        step,
        spot: Some(spot),
        interest_rate: Some(0.03),
        ..NodeState::default()
    };
    assert_eq!(seen, [node(1, 50.0), node(1, 200.0), node(0, 100.0)]);
    assert_eq!(seen[0].spot(), Some(50.0));
    assert_eq!(seen[0].interest_rate(), Some(0.03));
    assert_eq!(seen[0].hazard_rate(), None);
    assert_eq!(seen[0].discount_factor(), None);
}

#[test]
fn short_rate_lattice_hands_node_rate_and_oas_to_the_valuator() {
    let node_rate = |step: usize, node: usize| 0.01 * (step + node) as f64;
    let discount_rate = |_step: usize, _node: usize| 0.0;
    let oas_bp = |_step: usize, _node: usize| 125.0;
    let seen = StateProbe::price(RecombiningLattice::ShortRate {
        node_rate: &node_rate,
        discount_rate: &discount_rate,
        oas_bp: &oas_bp,
    });
    let node = |step: usize, rate: f64| NodeState {
        step,
        oas_bp: 125.0,
        interest_rate: Some(rate),
        ..NodeState::default()
    };
    assert_eq!(seen, [node(1, 0.01), node(1, 0.02), node(0, 0.0)]);
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
