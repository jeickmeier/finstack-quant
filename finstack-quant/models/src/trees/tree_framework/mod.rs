//! Generic tree-based pricing framework for financial instruments.
//!
//! This module provides a lattice pricing engine that separates instrument
//! payoff logic (`TreeValuator`) from lattice evolution (`TreeModel`), so the
//! same backward induction serves equity and short-rate trees.
//!
//! ## Serialization Policy
//!
//! Tree models and their parameter types are **transient runtime structures** and
//! do not implement `Serialize`/`Deserialize`. Tree configurations are created
//! on demand during pricing from market data, and no current use case requires
//! persisting them. If a future requirement emerges, add serde support only to
//! configuration structs (e.g. `EvolutionParams`) and keep runtime engine types
//! (`BinomialTree`, etc.) non-serializable.

pub use finstack_quant_core::math::time_grid::{map_date_to_step, map_exercise_dates_to_steps};

/// Standard state variable keys for consistency
pub mod state_keys {
    /// Underlying asset price (equity)
    pub const SPOT: &str = "spot";
    /// Risk-free interest rate
    pub const INTEREST_RATE: &str = "interest_rate";
    /// Hazard rate (default intensity) for credit modeling
    pub const HAZARD_RATE: &str = "hazard_rate";
    /// Dividend yield
    pub const DIVIDEND_YIELD: &str = "dividend_yield";
    /// Volatility
    pub const VOLATILITY: &str = "volatility";
    /// Discount factor at the current node (pre-computed for performance)
    pub const DF: &str = "df";
}

mod evolution;
mod node_state;
mod recombining;
mod traits;

#[cfg(test)]
mod tests;

pub use evolution::EvolutionParams;
pub(crate) use node_state::CachedValues;
pub use node_state::NodeState;
pub use recombining::{price_recombining_tree, single_factor_equity_state, RecombiningInputs};
pub use traits::{TreeGreeks, TreeModel, TreeValuator};
