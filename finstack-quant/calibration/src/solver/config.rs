//! Convergence settings consumed by calibration solvers.

use serde::{Deserialize, Deserializer, Serialize};

/// Serializable convergence settings for calibration.
///
/// Calibration owns its bracket search. This configuration contains only the
/// tolerance and iteration budget consumed by its numerical solvers.
///
/// Deserialization runs [`SolverConfig::validate`], so a wire document with a
/// non-positive tolerance or a zero iteration budget is rejected at parse time.
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
#[derive(Debug, Clone, Serialize, PartialEq)]
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

    /// Check that the settings can drive a numerical solve.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] if `tolerance` is
    /// not a positive finite number or `max_iterations` is zero.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if !self.tolerance.is_finite() || self.tolerance <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "solver tolerance must be a positive finite number, got {}",
                self.tolerance
            )));
        }
        if self.max_iterations == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "solver max_iterations must be at least 1".to_string(),
            ));
        }
        Ok(())
    }
}

/// Serde wire form of [`SolverConfig`]; validated on conversion.
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SolverConfigWire {
    tolerance: f64,
    max_iterations: usize,
}

impl Default for SolverConfigWire {
    fn default() -> Self {
        let SolverConfig {
            tolerance,
            max_iterations,
        } = SolverConfig::default();
        Self {
            tolerance,
            max_iterations,
        }
    }
}

impl<'de> Deserialize<'de> for SolverConfig {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let SolverConfigWire {
            tolerance,
            max_iterations,
        } = SolverConfigWire::deserialize(deserializer)?;
        let config = Self {
            tolerance,
            max_iterations,
        };
        config.validate().map_err(serde::de::Error::custom)?;
        Ok(config)
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
    fn solver_config_validate_rejects_unusable_settings() {
        assert!(SolverConfig::default().validate().is_ok());
        for tolerance in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let error = SolverConfig::default()
                .with_tolerance(tolerance)
                .validate()
                .expect_err("non-positive or non-finite tolerance must be rejected");
            assert!(error.to_string().contains("solver tolerance"));
        }
        let error = SolverConfig::default()
            .with_max_iterations(0)
            .validate()
            .expect_err("zero iteration budget must be rejected");
        assert!(error.to_string().contains("max_iterations"));
    }

    #[test]
    fn solver_config_deserialization_validates() {
        for wire in [
            json!({"tolerance": 0.0}),
            json!({"tolerance": -1.0}),
            json!({"max_iterations": 0}),
        ] {
            let error = serde_json::from_value::<SolverConfig>(wire.clone())
                .expect_err("invalid solver settings must fail to parse");
            assert!(error.to_string().contains("solver"), "{wire}: {error}");
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
