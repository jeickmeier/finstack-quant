//! Shared node, evolution, and backward-induction components for pricing trees.
//!
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::HashMap;
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
    /// * `state` - Node state with cached common variables
    /// * `continuation_value` - Discounted expected value from child nodes
    /// * `dt` - Time step size (passed explicitly to avoid hash lookup)
    fn value_at_node(&self, state: &NodeState, continuation_value: f64, dt: f64) -> Result<f64>;
}

/// Trait for generic tree models (binomial, short-rate, two-factor)
pub trait TreeModel: Send + Sync {
    /// Price an instrument using this tree model
    ///
    /// # Arguments
    /// * `initial_vars` - Initial state variables at t=0
    /// * `time_to_maturity` - Total time to maturity in years
    /// * `market_context` - Market data context
    /// * `valuator` - Instrument-specific valuation logic
    #[must_use = "pricing result should not be discarded"]
    fn price<V: TreeValuator>(
        &self,
        initial_vars: HashMap<&'static str, f64>,
        time_to_maturity: f64,
        market_context: &MarketContext,
        valuator: &V,
    ) -> Result<f64>;
}

/// Greeks calculated from tree models.
///
/// # Units and Conventions
///
/// - **Delta**: Per unit of spot (e.g., delta=0.5 means $0.50 per $1 spot move)
/// - **Gamma**: Per unit of spot squared (second derivative)
/// - **Vega**: Per 1% absolute volatility move (e.g., 20% → 21%)
/// - **Theta**: Per day (negative for long positions typically)
/// - **Rho**: Per 1 basis point (0.01%) interest rate move
/// - **OAS01**: Per 1 basis point option-adjusted-spread move
#[derive(Debug, Clone)]
pub struct TreeGreeks {
    /// Instrument price
    pub price: f64,
    /// Delta (spot sensitivity per unit spot move)
    pub delta: f64,
    /// Gamma (curvature, second derivative w.r.t. spot)
    pub gamma: f64,
    /// Vega (volatility sensitivity per 1% vol move)
    pub vega: f64,
    /// Theta (time decay per day)
    pub theta: f64,
    /// Rho (interest rate sensitivity per 1bp rate move)
    pub rho: f64,
    /// OAS01 (option-adjusted-spread sensitivity per 1bp spread move)
    pub oas01: f64,
}
