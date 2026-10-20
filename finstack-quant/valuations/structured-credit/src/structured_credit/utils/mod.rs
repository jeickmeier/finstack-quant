//! Utility functions for structured credit instruments.
//!
//! This module provides helper functions used across the structured credit module:
//! - Rate conversions (CPR↔SMM, CDR↔MDR, PSA→CPR)
//! - Simulation helpers (recovery queue, period flows)
//! - Validation framework for waterfall specifications

pub(crate) mod amortization;
pub(crate) mod simulation;
pub(crate) mod validation;

pub use validation::{validate_tiers, ValidationError};
