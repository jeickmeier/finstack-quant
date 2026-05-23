//! Generic tree-based pricing framework for financial instruments.
//!
//! This module provides a flexible lattice pricing engine that can accommodate
//! various tree types (binomial, trinomial) and multiple state variables
//! (equity + rates, equity + credit spread, etc.) without requiring code changes
//! to the core pricing logic.
//!
//! Barrier option support is provided via the `BarrierState` structure in
//! `NodeState`. Tree models can track barrier conditions and check knock-in/out
//! status using the provided helper methods.
//!
//! NOTE: Performance enhancements (parallel Greeks, caching of node values,
//!       and optional SIMD) are intentionally deferred to keep the initial
//!       implementation simple and deterministic.
//!
//! ## Serialization Policy
//!
//! Tree models and their parameter types are **transient runtime structures** and
//! do not currently implement `Serialize`/`Deserialize`. This is by design:
//! - Tree configurations are created on-demand during pricing
//! - Parameters are derived from market data or hardcoded defaults
//! - No current use case requires persisting tree configurations
//!
//! If a future requirement emerges (e.g., scenario storage, calibration persistence),
//! add serde support **only to configuration structs** (e.g., `TreeParameters`,
//! `EvolutionParams`) using `#[derive(Serialize, Deserialize, schemars::JsonSchema)]`.
//! Keep runtime engine types (`BinomialTree`, etc.) non-serializable.
//!
//! See `docs/TREE_PARAMS_SERIALIZATION_AUDIT.md` for audit results and extension pattern.

pub use finstack_core::math::time_grid::{
    map_date_to_step, map_dates_to_steps, map_exercise_dates_to_steps,
};

/// Standard state variable keys for consistency
pub mod state_keys {
    /// Underlying asset price (equity)
    pub const SPOT: &str = "spot";
    /// Risk-free interest rate
    pub const INTEREST_RATE: &str = "interest_rate";
    /// Credit spread
    pub const CREDIT_SPREAD: &str = "credit_spread";
    /// Hazard rate (default intensity) for credit modeling
    pub const HAZARD_RATE: &str = "hazard_rate";
    /// Dividend yield
    pub const DIVIDEND_YIELD: &str = "dividend_yield";
    /// Volatility
    pub const VOLATILITY: &str = "volatility";
    /// Barrier touched up-flag (1.0 if touched at this node, else 0.0)
    pub const BARRIER_TOUCHED_UP: &str = "barrier_touched_up";
    /// Barrier touched down-flag (1.0 if touched at this node, else 0.0)
    pub const BARRIER_TOUCHED_DOWN: &str = "barrier_touched_down";
    /// Discount factor at the current node (pre-computed for performance)
    pub const DF: &str = "df";
    /// Rate volatility for two-factor equity+rates models
    pub const RATE_VOLATILITY: &str = "rate_volatility";
}

mod evolution;
mod node_state;
mod recombining;
mod traits;

#[cfg(test)]
mod tests;

pub use evolution::{
    BarrierSpec, BarrierStyle, EvolutionParams, StateGenerator, TreeBranching, TreeParameters,
};
pub use node_state::{BarrierState, BarrierType, NodeState, StateVariables};
pub use recombining::{
    price_recombining_tree, single_factor_equity_state, two_factor_equity_rates_state,
    RecombiningInputs,
};
pub use traits::{GreeksBumpConfig, TreeGreeks, TreeModel, TreeValuator};
