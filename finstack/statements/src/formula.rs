//! Public formula helper functions for low-level identifier analysis.
//!
//! These helpers are shared with companion crates such as
//! `finstack-statements-analytics`, while graph traversal remains internal to
//! the statements crate. The implementation lives in `crate::utils::formula`,
//! which is `pub(crate)`; this module is the curated public boundary, so
//! downstream crates link against a stable surface even when the underlying
//! helpers are reorganised.

pub use crate::utils::formula::extract_all_identifiers;
