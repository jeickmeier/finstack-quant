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
//! - Recovery-rate empirical context:
//!   `docs/REFERENCES.md#altman-et-al-2005-recovery`

use super::RecoveryModel;

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
    /// * `rate` - Recovery rate, clamped to [0, 1]
    ///
    /// # Returns
    ///
    /// A constant recovery model with the bounded recovery rate.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::correlation::{ConstantRecovery, RecoveryModel};
    ///
    /// let model = ConstantRecovery::new(0.40);
    /// assert_eq!(model.expected_recovery(), 0.40);
    /// assert_eq!(model.conditional_recovery(-3.0), 0.40);
    /// ```
    #[must_use]
    pub fn new(rate: f64) -> Self {
        Self {
            rate: rate.clamp(0.0, 1.0),
        }
    }

    /// ISDA standard recovery rate (40%).
    ///
    /// # Returns
    ///
    /// A constant 40% recovery model.
    #[must_use]
    pub fn isda_standard() -> Self {
        Self::new(0.40)
    }

    /// Senior secured recovery rate (55%).
    ///
    /// # Returns
    ///
    /// A constant 55% recovery model.
    #[must_use]
    pub fn senior_secured() -> Self {
        Self::new(0.55)
    }

    /// Subordinated debt recovery rate (25%).
    ///
    /// # Returns
    ///
    /// A constant 25% recovery model.
    #[must_use]
    pub fn subordinated() -> Self {
        Self::new(0.25)
    }

    /// Get the recovery rate.
    ///
    /// # Returns
    ///
    /// The constant recovery rate in decimal form.
    #[must_use]
    pub fn rate(&self) -> f64 {
        self.rate
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
        let model = ConstantRecovery::new(0.40);
        assert!((model.rate() - 0.40).abs() < 1e-10);
        assert_eq!(model.model_name(), "Constant Recovery");
    }

    #[test]
    fn test_constant_recovery_clamping() {
        let high = ConstantRecovery::new(1.5);
        assert!((high.rate() - 1.0).abs() < 1e-10);

        let low = ConstantRecovery::new(-0.1);
        assert!((low.rate() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_conditional_equals_unconditional() {
        let model = ConstantRecovery::new(0.40);

        // Conditional should equal unconditional for all market factors
        for z in [-3.0, -1.0, 0.0, 1.0, 3.0] {
            assert!((model.conditional_recovery(z) - model.expected_recovery()).abs() < 1e-10);
        }
    }

    #[test]
    fn test_lgd_calculation() {
        let model = ConstantRecovery::new(0.40);
        assert!((model.lgd() - 0.60).abs() < 1e-10);
    }

    #[test]
    fn test_is_not_stochastic() {
        let model = ConstantRecovery::new(0.40);
        assert!(!model.is_stochastic());
        assert!((model.recovery_volatility()).abs() < 1e-10);
    }

    #[test]
    fn test_standard_models() {
        let isda = ConstantRecovery::isda_standard();
        assert!((isda.rate() - 0.40).abs() < 1e-10);

        let senior = ConstantRecovery::senior_secured();
        assert!((senior.rate() - 0.55).abs() < 1e-10);

        let sub = ConstantRecovery::subordinated();
        assert!((sub.rate() - 0.25).abs() < 1e-10);
    }
}
