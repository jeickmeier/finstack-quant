//! Stochastic prepayment trait definition.
//!
//! The [`StochasticPrepayment`] trait provides a common interface for all
//! prepayment models that incorporate systematic risk factors.

/// Stochastic prepayment model interface.
///
/// Implementations provide conditional prepayment rates given:
/// - Loan seasoning (months since origination)
/// - Systematic factor realizations
/// - Market conditions (interest rates)
/// - AssetPool burnout state
///
/// # Mathematical Framework
///
/// General form:
/// ```text
/// SMM(t, Z) = f(base_smm, Z, market_rate, burnout)
/// ```
///
/// where:
/// - Z is the systematic factor realization
/// - market_rate is the current mortgage rate
/// - burnout captures historical prepayment exhaustion
pub trait StochasticPrepayment: Send + Sync + std::fmt::Debug {
    /// Conditional SMM given factor realizations.
    ///
    /// Returns the single monthly mortality rate conditional on:
    /// - `seasoning`: Months since origination
    /// - `factors`: Systematic factor values [prepay_factor, ...]
    /// - `market_rate`: Current mortgage rate (for refi incentive)
    /// - `burnout`: Burnout factor in [0, 1] (1 = no burnout)
    fn conditional_smm(
        &self,
        seasoning: u32,
        factors: &[f64],
        market_rate: f64,
        burnout: f64,
    ) -> f64;

    /// Expected (unconditional) SMM at given seasoning.
    ///
    /// This is E[SMM(t)] integrated over the factor distribution.
    fn expected_smm(&self, seasoning: u32) -> f64;

    /// Factor loading for correlation calculation.
    ///
    /// The factor loading β determines how sensitive prepayment is
    /// to the systematic factor:
    /// ```text
    /// CPR(Z) ≈ base_cpr × exp(β × Z × σ)
    /// ```
    fn factor_loading(&self) -> f64;

    /// Model name for diagnostics.
    fn model_name(&self) -> &'static str;

    /// Whether the model incorporates burnout.
    fn has_burnout(&self) -> bool {
        false
    }

    /// Whether the model is rate-sensitive (refi incentive).
    fn is_rate_sensitive(&self) -> bool {
        false
    }

    /// Update burnout factor based on historical prepayments.
    ///
    /// Returns new burnout factor given:
    /// - `current_burnout`: Current burnout state
    /// - `realized_smm`: SMM that actually occurred
    /// - `expected_smm`: Expected SMM at that time
    ///
    /// Standard burnout update:
    /// ```text
    /// burnout_new = burnout_old × (1 - decay × (realized_smm / expected_smm - 1))
    /// ```
    fn update_burnout(&self, current_burnout: f64, realized_smm: f64, expected_smm: f64) -> f64 {
        let ratio = if expected_smm > 1e-10 {
            realized_smm / expected_smm
        } else {
            1.0
        };

        // Default: no burnout update
        let decay = 0.0;
        let new_burnout = current_burnout * (1.0 - decay * (ratio - 1.0));
        new_burnout.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock prepayment model for testing the trait
    #[derive(Debug)]
    struct MockPrepayment {
        base_smm: f64,
        factor_loading: f64,
    }

    impl MockPrepayment {
        fn new(base_smm: f64, factor_loading: f64) -> Self {
            Self {
                base_smm,
                factor_loading,
            }
        }
    }

    impl StochasticPrepayment for MockPrepayment {
        fn conditional_smm(
            &self,
            _seasoning: u32,
            factors: &[f64],
            _market_rate: f64,
            burnout: f64,
        ) -> f64 {
            let z = factors.first().copied().unwrap_or(0.0);
            let shocked = self.base_smm * (self.factor_loading * z).exp();
            (shocked * burnout).clamp(0.0, 1.0)
        }

        fn expected_smm(&self, _seasoning: u32) -> f64 {
            self.base_smm
        }

        fn factor_loading(&self) -> f64 {
            self.factor_loading
        }

        fn model_name(&self) -> &'static str {
            "Mock Prepayment"
        }
    }

    #[test]
    fn test_conditional_smm_increases_with_positive_factor() {
        let model = MockPrepayment::new(0.01, 0.5);

        let smm_neg = model.conditional_smm(12, &[-1.0], 0.05, 1.0);
        let smm_zero = model.conditional_smm(12, &[0.0], 0.05, 1.0);
        let smm_pos = model.conditional_smm(12, &[1.0], 0.05, 1.0);

        // Positive factor loading means positive factor increases SMM
        assert!(smm_pos > smm_zero);
        assert!(smm_neg < smm_zero);
    }

    #[test]
    fn test_burnout_reduces_smm() {
        let model = MockPrepayment::new(0.01, 0.5);

        let smm_no_burnout = model.conditional_smm(12, &[0.0], 0.05, 1.0);
        let smm_with_burnout = model.conditional_smm(12, &[0.0], 0.05, 0.5);

        assert!(smm_with_burnout < smm_no_burnout);
        assert!((smm_with_burnout / smm_no_burnout - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_default_burnout_update_is_identity() {
        let model = MockPrepayment::new(0.01, 0.5);

        let new_burnout = model.update_burnout(0.8, 0.01, 0.01);
        assert!((new_burnout - 0.8).abs() < 1e-10);
    }
}
