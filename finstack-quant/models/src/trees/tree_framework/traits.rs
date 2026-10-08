//! Shared node, evolution, and backward-induction components for pricing trees.
//!
use finstack_quant_core::Result;

use super::node_state::NodeState;

/// Trait for instrument-specific valuation logic on a tree
pub trait TreeValuator: Send + Sync {
    /// Calculate the instrument's value at a terminal node (maturity)
    fn value_at_maturity(&self, state: &NodeState) -> Result<f64>;

    /// Hold-vs-exercise (or other) decision at an intermediate node, given
    /// the discounted expected continuation value from child nodes.
    ///
    /// # Arguments
    ///
    /// * `state` - Typed node state: step index, OAS and the per-node values
    ///   the tree models
    /// * `continuation_value` - Discounted expected value from child nodes
    /// * `dt` - Time step size in years
    fn value_at_node(&self, state: &NodeState, continuation_value: f64, dt: f64) -> Result<f64>;
}
