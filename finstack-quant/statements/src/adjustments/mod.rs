//! EBITDA normalization and add-back utilities.
//!
//! This module groups the public API used to compute adjusted metrics such as
//! adjusted EBITDA from statement results plus an explicit catalog of
//! add-backs, caps, and normalization policies.
//!
//! Start with [`NormalizationConfig`] to define the adjustment policy and
//! [`normalize`] to execute it.
//!
//! ## Conventions
//!
//! - Adjustment values are expressed in the same units as the statement metric
//!   being normalized.
//! - Caps and overrides are applied explicitly through the normalization config
//!   rather than inferred from accounting metadata.
//! - The output keeps both the adjusted total and an audit trail of applied
//!   adjustments so downstream reporting can explain the bridge from reported to
//!   adjusted values.

mod engine;
mod types;

pub use engine::{merge_into_results, normalize};
pub use types::{
    Adjustment, AdjustmentCap, AdjustmentValue, AppliedAdjustment, CapBaseMode,
    NormalizationConfig, NormalizationResult,
};
