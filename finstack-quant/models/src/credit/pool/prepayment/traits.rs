//! Stochastic prepayment trait definition.
//!
//! The [`StochasticPrepayment`] trait provides a common interface for all
//! prepayment models that incorporate systematic risk factors.

/// State belonging to one simulated prepayment path.
///
/// Initialize through [`StochasticPrepayment::initial_state`] at the pool's
/// current seasoning, then retain the same state across monthly calls to
/// [`StochasticPrepayment::sample_smm`]. The default starts in the low regime
/// at origination; models without regimes leave this state unchanged.
#[derive(Debug, Clone, Default)]
pub struct PrepaymentState {
    pub(super) high_regime: bool,
}

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
    /// Initialize a path's latent state before its first simulated month.
    ///
    /// # Arguments
    ///
    /// * `seasoning` - Completed months since origination, before the first
    ///   simulated month. Regime models start low at origination and sample
    ///   the distribution after this many monthly transitions.
    /// * `uniform` - Independent uniform draw in `[0, 1)` used to select the
    ///   initial latent state; models without regimes ignore this draw.
    fn initial_state(&self, seasoning: u32, uniform: f64) -> PrepaymentState {
        let _ = (seasoning, uniform);
        PrepaymentState::default()
    }

    /// Advance one month and sample its SMM while retaining path state.
    ///
    /// Models without a latent regime use their factor-conditional rate.
    /// Regime models first transition from the prior month's state, then
    /// compute SMM from the resulting state's annual CPR.
    ///
    /// # Arguments
    ///
    /// * `seasoning` - Months since origination at the end of this simulated
    ///   month; advance sequentially by one month after initialization.
    /// * `factors` - Current systematic factor values, with the prepayment
    ///   shock in the first entry; an empty slice supplies a zero shock.
    /// * `market_rate` - Current annual mortgage refinancing rate as a decimal.
    /// * `burnout` - Remaining prepayment propensity in `[0, 1]`, where one
    ///   means no burnout.
    /// * `state` - This path's persistent state, initialized at the preceding
    ///   seasoning and updated to the current month's regime.
    /// * `uniform` - Fresh independent uniform draw in `[0, 1)` for this
    ///   month's latent transition, independent of systematic factors.
    fn sample_smm(
        &self,
        seasoning: u32,
        factors: &[f64],
        market_rate: f64,
        burnout: f64,
        state: &mut PrepaymentState,
        uniform: f64,
    ) -> f64 {
        let _ = (state, uniform);
        self.conditional_smm(seasoning, factors, market_rate, burnout)
    }

    /// Conditional SMM given factor realizations.
    ///
    /// Averages over latent regimes. Use [`Self::sample_smm`] for a simulated
    /// path that must preserve regime persistence.
    ///
    /// # Arguments
    ///
    /// * `seasoning` - Months since origination; regime probabilities start
    ///   in the low state at month zero.
    /// * `factors` - Systematic factor values `[prepay_factor, ...]`; an empty
    ///   slice supplies a zero prepayment shock.
    /// * `market_rate` - Annual mortgage refinancing rate as a decimal.
    /// * `burnout` - Burnout factor in `[0, 1]`, where one means no burnout.
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
    ///
    /// # Arguments
    ///
    /// * `seasoning` - Months since origination at which to average monthly
    ///   prepayment, including the latent regime distribution where present.
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
