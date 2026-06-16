//! Term loan pricers.
//!
//! The term loan module supports deterministic discounting and (optionally) tree-based
//! pricing for callable structures.

pub(crate) mod discounting;
pub(crate) mod tree_engine;

pub use discounting::TermLoanDiscountingPricer;
pub use tree_engine::TermLoanTreePricer;
