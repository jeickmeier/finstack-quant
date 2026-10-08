//! Financial statement checks framework.
//!
//! This module provides a pluggable validation system for financial models.
//! Checks inspect a [`crate::types::FinancialModelSpec`] and its
//! [`crate::evaluator::StatementResult`] to detect balance errors,
//! reasonableness violations, data gaps, and more.
//!
//! # Quick start
//!
//! ```
//! use finstack_quant_statements::checks::CheckSuite;
//! ```

pub mod builtins;
mod formula;
pub mod helpers;
pub mod suite;
pub mod traits;
pub mod types;

pub use suite::{
    BuiltinCheckSpec, CheckSuite, CheckSuiteBuilder, CheckSuiteSpec, FormulaCheckSpec,
};
pub use traits::{Check, CheckContext};
pub use types::{
    CheckCategory, CheckComparison, CheckConfig, CheckFinding, CheckReport, CheckResult,
    CheckSummary, Materiality, PeriodScope, Severity, SignConventionPolicy,
};
