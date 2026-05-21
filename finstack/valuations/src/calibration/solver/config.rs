//! Solver configuration for calibration procedures.
//!
//! Provides serializable solver configuration that can be persisted
//! in calibration reports for full reproducibility.

use finstack_core::math::solver::BrentSolver;
use serde::{Deserialize, Serialize};

/// Serializable solver configuration for calibration.
///
/// Wraps the Brent 1D solver used by the bootstrap path. Serializes transparently
/// as the underlying [`BrentSolver`] fields.
///
/// # Examples
///
/// ```ignore
/// use finstack_valuations::calibration::SolverConfig;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = SolverConfig::brent_default().with_tolerance(1e-12).with_max_iterations(200);
/// let json = serde_json::to_string(&config)?;
/// # let _ = json;
/// # Ok(())
/// # }
/// ```
#[cfg_attr(feature = "ts_export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(transparent)]
pub struct SolverConfig {
    /// Full Brent solver state from `finstack-core`.
    #[cfg_attr(feature = "ts_export", ts(skip))]
    solver: BrentSolver,
}

impl SolverConfig {
    /// Create a Brent solver config with default settings.
    pub fn brent_default() -> Self {
        Self {
            solver: BrentSolver::default(),
        }
    }

    /// Get the convergence tolerance.
    pub fn tolerance(&self) -> f64 {
        self.solver.tolerance
    }

    /// Get the maximum number of iterations.
    pub fn max_iterations(&self) -> usize {
        self.solver.max_iterations
    }

    /// Set the tolerance (builder pattern).
    pub fn with_tolerance(mut self, tolerance: f64) -> Self {
        self.solver.tolerance = tolerance;
        self
    }

    /// Set the maximum iterations (builder pattern).
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.solver.max_iterations = max_iterations;
        self
    }
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self::brent_default()
    }
}

impl From<BrentSolver> for SolverConfig {
    fn from(solver: BrentSolver) -> Self {
        Self { solver }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solver_config_default_is_brent() {
        let config = SolverConfig::default();
        assert_eq!(config.tolerance(), BrentSolver::default().tolerance);
        assert_eq!(
            config.max_iterations(),
            BrentSolver::default().max_iterations
        );
    }

    #[test]
    fn test_solver_config_from_brent() {
        let brent = BrentSolver::default();
        let config: SolverConfig = brent.into();
        assert_eq!(config.tolerance(), BrentSolver::default().tolerance);
    }

    #[test]
    fn test_solver_config_serde_roundtrip() {
        let config = SolverConfig::brent_default()
            .with_tolerance(1e-14)
            .with_max_iterations(200);
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: SolverConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_solver_config_json_format() {
        let config = SolverConfig::brent_default().with_tolerance(1e-12);
        let json = serde_json::to_string(&config).expect("serialize");
        assert!(json.contains(r#""tolerance""#));
    }
}
