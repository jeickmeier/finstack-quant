//! Check trait and execution context.

use crate::evaluator::StatementResult;
use crate::types::FinancialModelSpec;
use crate::Result;

use super::types::{CheckCategory, CheckConfig, CheckResult};

/// Context provided to each [`Check`] during execution.
///
/// Holds references to the evaluated model, its results, and the
/// configuration that governs check behaviour.
pub struct CheckContext<'a> {
    /// The financial model specification being checked.
    pub model: &'a FinancialModelSpec,
    /// Evaluation results for the model.
    pub results: &'a StatementResult,
    /// Configuration controlling tolerances and filters.
    pub config: CheckConfig,
}

impl<'a> CheckContext<'a> {
    /// Create a context with default configuration.
    pub fn new(model: &'a FinancialModelSpec, results: &'a StatementResult) -> Self {
        Self {
            model,
            results,
            config: CheckConfig::default(),
        }
    }
}

/// A single validation check that can be executed against a financial model.
///
/// Implementors inspect the model and its results, returning a [`CheckResult`]
/// that may contain zero or more [`super::types::CheckFinding`]s.
pub trait Check: Send + Sync {
    /// Unique machine-readable identifier for this check (e.g. `"balance_sheet_identity"`).
    fn id(&self) -> &str;

    /// Human-readable name for display purposes.
    fn name(&self) -> &str;

    /// Category the check belongs to.
    fn category(&self) -> CheckCategory;

    /// Execute the check and return a result.
    fn execute(&self, context: &CheckContext) -> Result<CheckResult>;
}
