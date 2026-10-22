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
pub(crate) mod helpers;
mod suite;
mod traits;
pub(crate) mod types;

pub use helpers::{get_finite_node_value, sum_nodes};

pub use suite::{
    BuiltinCheckSpec, CheckSuite, CheckSuiteBuilder, CheckSuiteSpec, FormulaCheckSpec,
};
pub use traits::{Check, CheckContext};
pub use types::{
    CheckCategory, CheckComparison, CheckConfig, CheckFinding, CheckReport, CheckResult,
    CheckSummary, Materiality, PeriodScope, Severity, SignConventionPolicy,
};
