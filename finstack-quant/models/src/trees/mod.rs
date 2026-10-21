//! Tree-based pricing models for American, Bermudan, and callable instruments.
//!
//! Provides binomial, trinomial, and multi-factor lattices for pricing
//! instruments with early exercise and embedded options.
//!
//! ## Serialization Policy
//!
//! Tree models and configuration types in this module are runtime-only structures
//! and do **not** implement `Serialize`/`Deserialize`. They are constructed
//! on-demand during pricing and not part of any persistent JSON schema.

pub(crate) mod binomial_tree;
pub(crate) mod hull_white_tree;
pub mod short_rate_tree;
pub mod tree_framework;
pub mod two_factor_rates_credit;

pub use binomial_tree::{BinomialTree, TreeType};
pub use hull_white_tree::{HullWhiteTree, HullWhiteTreeConfig};
