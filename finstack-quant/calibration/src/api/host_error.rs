//! Host-language attribute payload for [`ExecuteError`].
//!
//! Python and WASM attach these fields; they do not rebuild the structured
//! error from `category` / `stage` / diagnostics pieces.

use super::engine::{ExecuteError, ExecutionSolverDiagnostics, ExecutionStage};
use super::errors::StrictLoadDiagnostic;

/// Stable Py/WASM exception attributes for a calibration execution failure.
///
/// Field names match the host contract: `kind`, `stage`, `step_id`,
/// `solver_diagnostics`, `details` and `diagnostics`. Bindings convert the
/// structured fields with their standard serde conversion (a JS object /
/// Python dict or list), so both hosts expose the same shapes.
#[derive(Debug, Clone, PartialEq)]
pub struct HostExecuteError {
    /// Exception / `Error` message shown to the caller.
    pub message: String,
    /// Programmatic error category attached as `kind`.
    pub kind: String,
    /// Pipeline stage identifier (`ingestion`, `solver`, …).
    pub stage: ExecutionStage,
    /// Failing calibration step identifier, when the failure is step-scoped.
    pub step_id: Option<String>,
    /// Fit-acceptance diagnostics, when the failure came from the solver.
    pub solver_diagnostics: Option<ExecutionSolverDiagnostics>,
    /// Pretty-printed [`super::engine::ExecutionErrorDetails`] JSON.
    pub details: String,
    /// Structured strict-load diagnostics (pointer, message, expected
    /// version, ...); empty unless ingestion rejected the document.
    pub diagnostics: Vec<StrictLoadDiagnostic>,
}

impl ExecuteError {
    /// Flatten this error into the Py/WASM attribute payload.
    ///
    /// `kind` is the execution category and `details` the existing
    /// [`ExecuteError::to_json`] document.
    #[must_use]
    pub fn host_error(&self) -> HostExecuteError {
        let details = self.details();
        HostExecuteError {
            message: details.cause,
            kind: details.category,
            stage: details.stage,
            step_id: details.step_id,
            solver_diagnostics: details.solver_diagnostics,
            details: self.to_json(),
            diagnostics: details.diagnostics,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::api::engine::{ExecuteError, ExecutionStage};
    use crate::api::errors::EnvelopeError;

    #[test]
    fn host_error_uses_kind_not_category_and_carries_typed_diagnostics() {
        let error = ExecuteError::envelope(
            ExecutionStage::Solver,
            EnvelopeError::SolverNotConverged {
                step_id: "hazard".to_string(),
                max_residual: 2e-6,
                tolerance: 1e-6,
                iterations: 12,
                worst_quote_id: Some("CDS-5Y".to_string()),
                worst_quote_residual: Some(2e-6),
            },
        );
        let host = error.host_error();
        assert_eq!(host.kind, "solver_not_converged");
        assert_eq!(host.stage.as_str(), "solver");
        assert_eq!(host.step_id.as_deref(), Some("hazard"));
        let diagnostics = serde_json::to_value(
            host.solver_diagnostics
                .as_ref()
                .expect("solver diagnostics present"),
        )
        .expect("diagnostics serialize");
        assert_eq!(diagnostics["worst_quote_id"], "CDS-5Y");
        assert_eq!(diagnostics["iterations"], 12);
        let details: serde_json::Value = serde_json::from_str(&host.details).expect("details JSON");
        assert_eq!(details["category"], host.kind);
        assert_eq!(details["stage"], host.stage.as_str());
    }
}
