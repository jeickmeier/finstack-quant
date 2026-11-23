//! Historical VaR calculation engine.
//!
//! Implements Historical Value-at-Risk using historical simulation methodology.
//! Supports both full revaluation and Taylor approximation (Greeks-based) approaches.
//!
//! # References
//!
//! - VaR and Expected Shortfall interpretation: `docs/REFERENCES.md#mcneil-frey-embrechts-qrm`

mod engine;
#[cfg(test)]
mod tests;

#[cfg(test)]
use engine::*;
pub use engine::{VarResult, calculate_var};
