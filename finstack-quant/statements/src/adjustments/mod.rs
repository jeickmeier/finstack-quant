//! Ordered normalization bridges from a reported metric to a separately
//! named adjusted metric, such as reported EBITDA to adjusted EBITDA.
//!
//! Start with [`NormalizationConfig`] to name the reported node, the new
//! output node and the ordered adjustments, [`normalize`] to build the bridge
//! and [`merge_into_results`] to publish the adjusted node.
//!
//! ## Conventions
//!
//! - Fixed amounts and absolute caps are typed: scalars for a scalar target,
//!   money in its currency for a monetary one. Nothing is converted between
//!   currencies.
//! - Each cap applies to its own adjustment, limits magnitude and keeps the
//!   sign. Nothing is inferred from accounting metadata; categories are
//!   presentation labels.
//! - The reported node is never overwritten. The adjusted metric is published
//!   under a new node id with its configuration, and every row of the bridge
//!   records its raw amount, cap base and limit, applied amount and running
//!   value.

mod engine;
mod types;

pub use engine::{merge_into_results, normalize};
pub use types::{
    Adjustment, AdjustmentCap, AdjustmentValue, AppliedAdjustment, CapBaseMode,
    NormalizationConfig, NormalizationResult,
};
