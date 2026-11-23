//! Prescribed FRTB risk weights, correlations, and other regulatory parameters.
//!
//! The per-risk-class submodules ([`commodity`], [`csr`], [`equity`], [`fx`],
//! [`girr`]) expose `pub const` tables transcribed from the Basel
//! standardised approach, and are read directly by the charge-calculation
//! private delta, vega and curvature calculation stages.
//!
//! # Provenance
//!
//! | Item | Value |
//! |------|-------|
//! | Source document | Basel Committee on Banking Supervision, *Minimum capital requirements for market risk* (BCBS **d457**) |
//! | Publication date | 14 January 2019; corrected version published 25 February 2019 |
//! | Consolidated as | Basel Framework chapter **MAR21**, "Standardised approach: sensitivities-based method" |
//! | MAR21 version | Effective 1 January 2023 (implementation date revised 27 March 2020); text incorporates the FAQs published 5 July 2024 and 23 March 2026 |
//! | Primary sources verified | <https://www.bis.org/bcbs/publ/d457.pdf> and the BIS consolidated-framework PDF export <https://www.bis.org/baselframework/BaselFramework.pdf> |
//! | Last reviewed | 2026-08-20 |
//! | Review procedure | See `data/margin/README.md`, "FRTB parameter review" |
//!
pub mod commodity;
mod correlations;
pub mod csr;
pub mod equity;
pub mod fx;
pub mod girr;
#[cfg(test)]
mod tests;

pub(super) use correlations::{bucket_correlation, name_correlation, other_bucket};
