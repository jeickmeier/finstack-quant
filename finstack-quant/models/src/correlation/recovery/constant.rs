//! Constant recovery rate model.
//!
//! The simplest recovery model with a fixed rate regardless of market conditions.
//! This is the traditional assumption used in standard CDS and CDO pricing.
//!
//! # Typical Values
//!
//! - Senior unsecured debt: 40% (ISDA standard)
//! - Senior secured debt: 50-60%
//! - Subordinated debt: 20-30%
//! - High yield: 30-35%
//!
//! # References
//!
//! - Recovery-rate empirical context: `docs/REFERENCES.md#altman-et-al-2005-recovery`
//!

use super::{validate_recovery_input, RecoveryModel, Result};

/// Constant recovery rate model.
///
/// Recovery is fixed and does not vary with market conditions.
/// This is the baseline model compatible with standard Gaussian copula.
///
/// # References
///
/// - `docs/REFERENCES.md#altman-et-al-2005-recovery`
#[derive(Debug, Clone)]
pub struct ConstantRecovery {
    /// Fixed recovery rate ∈ [0, 1]
    rate: f64,
}

impl ConstantRecovery {
    /// Create a constant recovery model.
    ///
    /// # Arguments
    /// * `rate` - Recovery rate, finite and in [0, 1]
    ///
    /// # Returns
    ///
    /// A constant recovery model with the bounded recovery rate.
    ///
    /// # Errors
    ///
    /// Returns a validation error if `rate` is non-finite or outside `[0, 1]`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_models::correlation::{ConstantRecovery, RecoveryModel};
    ///
    /// let model = ConstantRecovery::new(0.40).expect("valid model inputs");
    /// assert_eq!(model.expected_recovery(), 0.40);
    /// assert_eq!(model.conditional_recovery(-3.0), 0.40);
    /// ```
    pub fn new(rate: f64) -> Result<Self> {
        validate_recovery_input("rate", rate, 0.0, 1.0)?;
        Ok(Self { rate })
    }
}

impl RecoveryModel for ConstantRecovery {
    fn expected_recovery(&self) -> f64 {
        self.rate
    }

    fn conditional_recovery(&self, _market_factor: f64) -> f64 {
        // Constant: recovery doesn't depend on market factor
        self.rate
    }

    fn recovery_volatility(&self) -> f64 {
        0.0
    }

    fn model_name(&self) -> &'static str {
        "Constant Recovery"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_recovery_creation() {
        let model = ConstantRecovery::new(0.40).expect("valid model inputs");
        assert!((model.rate - 0.40).abs() < 1e-10);
        assert_eq!(model.model_name(), "Constant Recovery");
    }

    #[test]
    fn invalid_constant_recovery_is_rejected() {
        for rate in [-0.1, 1.5, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(ConstantRecovery::new(rate).is_err());
        }
    }

    #[test]
    fn test_conditional_equals_unconditional() {
        let model = ConstantRecovery::new(0.40).expect("valid model inputs");

        for z in [-3.0, -1.0, 0.0, 1.0, 3.0] {
            assert!((model.conditional_recovery(z) - model.expected_recovery()).abs() < 1e-10);
        }
    }

    #[test]
    fn test_lgd_calculation() {
        let model = ConstantRecovery::new(0.40).expect("valid model inputs");
        assert!((model.lgd() - 0.60).abs() < 1e-10);
    }

    #[test]
    fn test_is_not_stochastic() {
        let model = ConstantRecovery::new(0.40).expect("valid model inputs");
        assert!(!model.is_stochastic());
        assert!((model.recovery_volatility()).abs() < 1e-10);
    }
}
