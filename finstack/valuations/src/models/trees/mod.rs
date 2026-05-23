//! Tree-based pricing models for American, Bermudan, and path-dependent options.
//!
//! Provides implementations of binomial, trinomial, and multi-factor tree
//! methods for pricing options with early exercise and complex payoffs.
//!
//! ## Serialization Policy
//!
//! Tree models and configuration types in this module are runtime-only structures
//! and do **not** implement `Serialize`/`Deserialize`. They are constructed
//! on-demand during pricing and not part of any persistent JSON schema.
//!
//! See `docs/TREE_PARAMS_SERIALIZATION_AUDIT.md` for details and future extension pattern.

pub mod binomial_tree;
pub mod hull_white_tree;
pub mod short_rate_tree;
pub mod tree_framework;
pub mod two_factor_rates_credit;

pub use binomial_tree::{BinomialTree, TreeType};
pub use hull_white_tree::{HullWhiteTree, HullWhiteTreeConfig};
pub use short_rate_tree::{
    short_rate_keys, ShortRateModel, ShortRateTree, ShortRateTreeConfig, TreeCompounding,
    DEFAULT_LOGNORMAL_VOL, DEFAULT_NORMAL_VOL,
};
pub use tree_framework::{
    single_factor_equity_state, state_keys, two_factor_equity_rates_state, BarrierSpec,
    BarrierStyle, EvolutionParams, GreeksBumpConfig, NodeState, StateVariables, TreeBranching,
    TreeGreeks, TreeModel, TreeParameters, TreeValuator,
};
