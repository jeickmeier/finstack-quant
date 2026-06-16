//! Utility functions for structured credit instruments.
//!
//! This module provides helper functions used across the structured credit module:
//! - Rate conversions (CPR↔SMM, CDR↔MDR, PSA→CPR)
//! - Simulation helpers (recovery queue, period flows)
//! - Validation framework for waterfall specifications
//! - Rate projection helpers for floating rate assets

pub(crate) mod rate_helpers;
pub(crate) mod rates;
pub(crate) mod simulation;
pub(crate) mod validation;

// Re-export commonly used functions
pub(crate) use rates::frequency_periods_per_year;
pub use rates::{cdr_to_mdr, cpr_to_smm, mdr_to_cdr, psa_to_cpr, smm_to_cpr};
pub use validation::{get_validation_errors, is_valid_waterfall_spec, ValidationError};
