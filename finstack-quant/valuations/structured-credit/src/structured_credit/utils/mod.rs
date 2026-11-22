//! Utility functions for structured credit instruments.
//!
//! This module provides helper functions used across the structured credit module:
//! - Rate conversions (CPR↔SMM, CDR↔MDR, PSA→CPR)
//! - Validation framework for waterfall specifications

pub(crate) mod amortization;
pub(crate) mod validation;

pub use validation::{ValidationError, validate_tiers};
