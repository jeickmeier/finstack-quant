//! Generic tree-based pricing framework for financial instruments.
//!
//! This module provides a lattice pricing engine that separates instrument
//! payoff logic (`TreeValuator`) from lattice evolution (the tree types), so the
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

mod evolution;
mod node_state;
mod recombining;
mod traits;

#[cfg(test)]
mod tests;

pub use evolution::EvolutionParams;
pub use node_state::NodeState;
pub use recombining::{price_recombining_tree, RecombiningInputs, RecombiningLattice};
pub use traits::{TreeGreeks, TreeValuator};
