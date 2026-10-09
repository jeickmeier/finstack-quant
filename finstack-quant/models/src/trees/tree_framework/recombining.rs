//! Shared node, evolution, and backward-induction components for pricing trees.
//!
use finstack_quant_core::Result;

use super::node_state::NodeState;
use super::traits::TreeValuator;

/// Node values and discounting of the lattice driving the induction.
#[derive(Clone, Copy)]
pub enum RecombiningLattice<'a> {
    /// Multiplicative asset-price lattice discounted at a flat rate. Node
    /// values reach the valuator as [`NodeState::spot`].
    Spot {
        /// Asset price at the root node
        spot: f64,
        /// Multiplicative factor for up move (e.g., exp(σ√dt))
        up_factor: f64,
        /// Multiplicative factor for down move (e.g., exp(-σ√dt))
        down_factor: f64,
        /// Continuously compounded risk-free rate per annum used for discounting
        interest_rate: f64,
    },
    /// Calibrated short-rate lattice. Node values reach the valuator as
    /// [`NodeState::interest_rate`].
    ShortRate {
        /// Short rate at `(step, node)`
        node_rate: &'a dyn Fn(usize, usize) -> f64,
        /// Continuously compounded discounting rate at `(step, node)`,
        /// including any option-adjusted spread
        discount_rate: &'a dyn Fn(usize, usize) -> f64,
        /// Continuously compounded option-adjusted spread in basis points at
        /// `(step, node)`, passed to the valuator as [`NodeState::oas_bp`]
        oas_bp: &'a dyn Fn(usize, usize) -> f64,
    },
}

/// Inputs for the shared binomial recombining engine.
#[derive(Clone)]
pub struct RecombiningInputs<'a, V: TreeValuator> {
    /// Number of time steps in the tree
    pub steps: usize,
    /// Time to maturity in years
    pub time_to_maturity: f64,
    /// Payoff valuator implementing TreeValuator trait
    pub valuator: &'a V,
    /// Risk-neutral probability of up move
    pub prob_up: f64,
    /// Risk-neutral probability of down move
    pub prob_down: f64,
    /// Node values and discounting
    pub lattice: RecombiningLattice<'a>,
}

/// Price an instrument on a binomial recombining tree with backward induction.
///
/// Node `i` at step `n` has `i` up moves and `n - i` down moves. Payoffs are
/// evaluated at maturity and expected values are discounted backward to the
/// root. The evolving node value is the spot (equity trees) or the short rate
/// (short-rate trees), handed to the valuator in the matching [`NodeState`]
/// field.
///
/// # Arguments
///
/// * `inputs` - Complete tree configuration: step count, horizon in years,
///   valuator, branch probabilities and the lattice supplying node values and
///   discounting
///
/// # Returns
///
/// Present value of the instrument at time 0
pub fn price_recombining_tree<V: TreeValuator>(inputs: RecombiningInputs<'_, V>) -> Result<f64> {
    let dt = inputs.time_to_maturity / inputs.steps as f64;

    // Constant discount factor of the flat-rate spot lattice.
    let flat_df = match inputs.lattice {
        RecombiningLattice::Spot { interest_rate, .. } => (-interest_rate * dt).exp(),
        RecombiningLattice::ShortRate { .. } => 1.0,
    };
    let get_df = |step: usize, node: usize| -> f64 {
        match inputs.lattice {
            RecombiningLattice::Spot { .. } => flat_df,
            RecombiningLattice::ShortRate { discount_rate, .. } => {
                (-discount_rate(step, node) * dt).exp()
            }
        }
    };

    let state_for = |step: usize, node: usize, node_value: f64| -> NodeState {
        match inputs.lattice {
            RecombiningLattice::Spot { interest_rate, .. } => NodeState {
                step,
                spot: Some(node_value),
                interest_rate: Some(interest_rate),
                ..NodeState::default()
            },
            RecombiningLattice::ShortRate { oas_bp, .. } => NodeState {
                step,
                oas_bp: oas_bp(step, node),
                interest_rate: Some(node_value),
                ..NodeState::default()
            },
        }
    };

    // Node values for one level. The spot lattice is walked incrementally
    // from `spot * d^step`, multiplying by `u/d` per node.
    let level_values = |step: usize, out: &mut Vec<f64>| {
        out.clear();
        match inputs.lattice {
            RecombiningLattice::ShortRate { node_rate, .. } => {
                out.extend((0..=step).map(|i| node_rate(step, i)));
            }
            RecombiningLattice::Spot {
                spot,
                up_factor,
                down_factor,
                ..
            } => {
                let ud_ratio = up_factor / down_factor;
                let mut value = spot * down_factor.powi(step as i32);
                for i in 0..=step {
                    out.push(value);
                    if i < step {
                        value *= ud_ratio;
                    }
                }
            }
        }
    };

    let mut level = Vec::with_capacity(inputs.steps + 1);
    level_values(inputs.steps, &mut level);
    let mut values = Vec::with_capacity(inputs.steps + 1);
    for (i, &node_value) in level.iter().enumerate() {
        let terminal_state = state_for(inputs.steps, i, node_value);
        values.push(inputs.valuator.value_at_maturity(&terminal_state)?);
    }

    for step in (0..inputs.steps).rev() {
        level_values(step, &mut level);
        for (i, &node_value) in level.iter().enumerate() {
            let continuation =
                get_df(step, i) * (inputs.prob_up * values[i + 1] + inputs.prob_down * values[i]);
            let node_state = state_for(step, i, node_value);
            values[i] = inputs
                .valuator
                .value_at_node(&node_state, continuation, dt)?;
        }
        values.pop();
    }

    Ok(values[0])
}
