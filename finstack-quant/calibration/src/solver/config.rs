//! Convergence settings consumed by calibration solvers.

use serde::{Deserialize, Serialize};

/// Serializable convergence settings for calibration.
///
/// Calibration owns its bracket search. This configuration contains only the
/// tolerance and iteration budget consumed by its numerical solvers.
///
/// # Examples
///
/// ```
/// use finstack_quant_calibration::SolverConfig;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = SolverConfig::default().with_tolerance(1e-12).with_max_iterations(200);
/// let json = serde_json::to_string(&config)?;
/// # let _ = json;
/// # Ok(())
/// # }
/// ```
#[cfg_attr(feature = "ts_export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct SolverConfig {
    /// Numerical convergence tolerance; distinct from economic fit acceptance.
    tolerance: f64,
    /// Maximum iterations available to each solver invocation.
    max_iterations: usize,
}

impl SolverConfig {
    /// Get the numerical convergence tolerance (default `1e-12`).
    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }

    /// Get the maximum solver iterations (default `100`).
    pub fn max_iterations(&self) -> usize {
        self.max_iterations
    }

    /// Set the numerical convergence tolerance.
    ///
    /// # Arguments
    ///
    /// * `tolerance` - Positive finite numerical stopping tolerance. This is
    ///   separate from the calibrated instrument's repricing acceptance tolerance.
    pub fn with_tolerance(mut self, tolerance: f64) -> Self {
        self.tolerance = tolerance;
        self
    }

    /// Set the solver iteration budget.
    ///
    /// # Arguments
    ///
    /// * `max_iterations` - Maximum number of iterations per numerical solve.
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            tolerance: 1e-12,
            max_iterations: 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn solver_config_defaults_and_wire_shape() {
        let config: SolverConfig = serde_json::from_value(json!({})).expect("defaults");
        assert_eq!(config, SolverConfig::default());
        assert_eq!(
            serde_json::to_value(config).expect("serialize"),
            json!({
                "tolerance": 1e-12,
                "max_iterations": 100,
            })
        );
    }

    #[test]
    fn solver_config_rejects_unused_bracket_controls() {
        for field in [
            "bracket_expansion",
            "initial_bracket_size",
            "bracket_min",
            "bracket_max",
        ] {
            let error = serde_json::from_value::<SolverConfig>(json!({field: 1.0}))
                .expect_err("unknown bracket controls must not be silently accepted");
            assert!(error.to_string().contains(field));
        }
    }

    #[test]
    fn solver_config_serde_roundtrip() {
        let config = SolverConfig::default()
            .with_tolerance(1e-14)
            .with_max_iterations(200);
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: SolverConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, deserialized);
    }
}
