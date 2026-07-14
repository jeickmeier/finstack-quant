//! Stochastic prepayment specification.
//!
//! Provides a serializable specification enum for stochastic prepayment models,
//! enabling configuration and deferred construction.

use super::super::calibrations::{clo_standard, rmbs_standard};
use super::{
    FactorCorrelatedPrepay, RegimeSwitchingPrepay, RichardRollPrepay, StochasticPrepayment,
};
use crate::cashflow::builder::specs::PrepaymentModelSpec;
use crate::instruments::fixed_income::structured_credit::utils::rates::clamped_cpr_to_smm;

/// Stochastic prepayment model specification.
///
/// Allows prepayment model selection and configuration without
/// constructing the full model, enabling serialization and deferred construction.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "model", deny_unknown_fields)]
#[non_exhaustive]
pub enum StochasticPrepaySpec {
    /// Use deterministic prepayment model (no stochastic component).
    Deterministic(PrepaymentModelSpec),

    /// Factor-correlated prepayment model.
    ///
    /// Simple stochastic model that shocks base CPR by systematic factor.
    FactorCorrelated {
        /// Base deterministic prepayment specification
        base_spec: PrepaymentModelSpec,
        /// Factor loading (typical: 0.3-0.5)
        factor_loading: f64,
        /// CPR volatility (typical: 0.15-0.30)
        cpr_volatility: f64,
    },

    /// Richard-Roll prepayment model for RMBS.
    ///
    /// Full stochastic model with refinancing incentive, seasoning,
    /// burnout, and optional seasonality.
    RichardRoll {
        /// Base CPR at full seasoning
        base_cpr: f64,
        /// Refinancing sensitivity (gamma)
        refi_sensitivity: f64,
        /// AssetPool weighted average coupon
        pool_coupon: f64,
        /// Burnout decay rate
        burnout_rate: f64,
        /// Factor loading for correlation
        #[serde(default = "default_factor_loading")]
        factor_loading: f64,
        /// CPR volatility
        #[serde(default = "default_cpr_volatility")]
        cpr_volatility: f64,
    },

    /// Regime-switching prepayment model.
    ///
    /// Two-state Markov model for prepayment regimes (high/low).
    RegimeSwitching {
        /// CPR in low prepayment regime
        low_cpr: f64,
        /// CPR in high prepayment regime
        high_cpr: f64,
        /// Transition probability: low -> high (per month)
        transition_up: f64,
        /// Transition probability: high -> low (per month)
        transition_down: f64,
        /// Factor loading for systematic prepayment shocks.
        #[serde(default = "default_factor_loading")]
        factor_loading: f64,
        /// CPR volatility for systematic prepayment shocks.
        #[serde(default = "default_cpr_volatility")]
        cpr_volatility: f64,
    },
}

fn default_factor_loading() -> f64 {
    0.4
}

fn default_cpr_volatility() -> f64 {
    0.20
}

impl Default for StochasticPrepaySpec {
    fn default() -> Self {
        StochasticPrepaySpec::Deterministic(PrepaymentModelSpec::psa_100())
    }
}

impl StochasticPrepaySpec {
    /// Create a deterministic (non-stochastic) prepayment spec.
    pub fn deterministic(spec: PrepaymentModelSpec) -> Self {
        StochasticPrepaySpec::Deterministic(spec)
    }

    /// Create a factor-correlated prepayment spec.
    pub fn factor_correlated(
        base_spec: PrepaymentModelSpec,
        factor_loading: f64,
        cpr_volatility: f64,
    ) -> Self {
        StochasticPrepaySpec::FactorCorrelated {
            base_spec,
            factor_loading: factor_loading.clamp(-1.0, 1.0),
            cpr_volatility: cpr_volatility.clamp(0.0, 1.0),
        }
    }

    /// Create a Richard-Roll prepayment spec.
    pub fn richard_roll(
        base_cpr: f64,
        refi_sensitivity: f64,
        pool_coupon: f64,
        burnout_rate: f64,
    ) -> Self {
        StochasticPrepaySpec::RichardRoll {
            base_cpr: base_cpr.clamp(0.0, 1.0),
            refi_sensitivity: refi_sensitivity.clamp(0.0, 10.0),
            pool_coupon,
            burnout_rate: burnout_rate.clamp(0.0, 1.0),
            factor_loading: 0.4,
            cpr_volatility: 0.20,
        }
    }

    /// Create a regime-switching prepayment spec.
    pub fn regime_switching(
        low_cpr: f64,
        high_cpr: f64,
        transition_up: f64,
        transition_down: f64,
    ) -> Self {
        StochasticPrepaySpec::RegimeSwitching {
            low_cpr: low_cpr.clamp(0.0, 1.0),
            high_cpr: high_cpr.clamp(0.0, 1.0),
            transition_up: transition_up.clamp(0.0, 1.0),
            transition_down: transition_down.clamp(0.0, 1.0),
            factor_loading: default_factor_loading(),
            cpr_volatility: default_cpr_volatility(),
        }
    }

    /// RMBS agency standard calibration.
    ///
    /// Uses the registry-backed `rmbs_standard` calibration profile.
    pub fn rmbs_agency(pool_coupon: f64) -> Self {
        let calibration = rmbs_standard();
        StochasticPrepaySpec::RichardRoll {
            base_cpr: calibration.base_cpr,
            refi_sensitivity: calibration.refi_sensitivity,
            pool_coupon,
            burnout_rate: calibration.burnout_rate,
            factor_loading: calibration.prepay_factor_loading,
            cpr_volatility: calibration.cpr_volatility,
        }
    }

    /// CLO standard calibration.
    ///
    /// Uses the registry-backed `clo_standard` calibration profile.
    pub fn clo_standard() -> Self {
        let calibration = clo_standard();
        StochasticPrepaySpec::FactorCorrelated {
            base_spec: PrepaymentModelSpec::constant_cpr(calibration.base_cpr),
            factor_loading: calibration.prepay_factor_loading,
            cpr_volatility: calibration.cpr_volatility,
        }
    }

    /// Build the stochastic prepayment model from this specification.
    ///
    /// Returns None for deterministic specs (caller should use the underlying
    /// PrepaymentModelSpec directly).
    pub fn build(&self) -> Option<Box<dyn StochasticPrepayment>> {
        match self {
            StochasticPrepaySpec::Deterministic(_) => None,

            StochasticPrepaySpec::FactorCorrelated {
                base_spec,
                factor_loading,
                cpr_volatility,
            } => Some(Box::new(FactorCorrelatedPrepay::new(
                base_spec.clone(),
                *factor_loading,
                *cpr_volatility,
            ))),

            StochasticPrepaySpec::RichardRoll {
                base_cpr,
                refi_sensitivity,
                pool_coupon,
                burnout_rate,
                factor_loading,
                cpr_volatility,
            } => Some(Box::new(RichardRollPrepay::with_all_params(
                *base_cpr,
                *refi_sensitivity,
                20.0, // default refi_slope
                *pool_coupon,
                *burnout_rate,
                0.0, // no seasonality by default
                *factor_loading,
                *cpr_volatility,
                30, // default ramp months
            ))),

            StochasticPrepaySpec::RegimeSwitching {
                low_cpr,
                high_cpr,
                transition_up,
                transition_down,
                factor_loading,
                cpr_volatility,
            } => Some(Box::new(RegimeSwitchingPrepay::new(
                *low_cpr,
                *high_cpr,
                *transition_up,
                *transition_down,
                *factor_loading,
                *cpr_volatility,
            ))),
        }
    }

    /// Check if this is a stochastic specification.
    pub fn is_stochastic(&self) -> bool {
        !matches!(self, StochasticPrepaySpec::Deterministic(_))
    }

    /// Get the factor loading if this is a stochastic model.
    pub fn factor_loading(&self) -> Option<f64> {
        match self {
            StochasticPrepaySpec::Deterministic(_) => None,
            StochasticPrepaySpec::FactorCorrelated { factor_loading, .. }
            | StochasticPrepaySpec::RichardRoll { factor_loading, .. }
            | StochasticPrepaySpec::RegimeSwitching { factor_loading, .. } => Some(*factor_loading),
        }
    }

    /// Get the base SMM (single monthly mortality rate) for this specification.
    ///
    /// Returns the unconditional expected SMM before factor shocks are applied.
    pub fn base_smm(&self) -> f64 {
        match self {
            StochasticPrepaySpec::Deterministic(spec) => {
                // Convert annual CPR to monthly SMM
                clamped_cpr_to_smm(spec.cpr)
            }
            StochasticPrepaySpec::FactorCorrelated { base_spec, .. } => {
                clamped_cpr_to_smm(base_spec.cpr)
            }
            StochasticPrepaySpec::RichardRoll { base_cpr, .. } => clamped_cpr_to_smm(*base_cpr),
            StochasticPrepaySpec::RegimeSwitching {
                low_cpr, high_cpr, ..
            } => {
                // Average of low and high states
                let avg_cpr = (low_cpr + high_cpr) / 2.0;
                clamped_cpr_to_smm(avg_cpr)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spec_default() {
        let spec = StochasticPrepaySpec::default();
        assert!(!spec.is_stochastic());
    }

    #[test]
    fn test_factor_correlated_spec() {
        let spec =
            StochasticPrepaySpec::factor_correlated(PrepaymentModelSpec::psa(1.0), 0.4, 0.20);

        assert!(spec.is_stochastic());
        assert_eq!(spec.factor_loading(), Some(0.4));

        let model = spec.build();
        assert!(model.is_some());
    }

    #[test]
    fn test_richard_roll_spec() {
        let spec = StochasticPrepaySpec::richard_roll(0.06, 2.0, 0.045, 0.10);

        assert!(spec.is_stochastic());
        assert!(spec.factor_loading().is_some());

        let model = spec.build();
        assert!(model.is_some());

        let model = model.expect("Should build Richard-Roll model");
        assert_eq!(model.model_name(), "Richard-Roll Prepayment Model");
    }

    #[test]
    fn test_regime_switching_spec_builds_model() {
        let spec = StochasticPrepaySpec::regime_switching(0.04, 0.18, 0.10, 0.20);

        assert!(spec.is_stochastic());
        assert!(spec.factor_loading().is_some());

        let model = spec.build().expect("regime-switching model should build");
        assert_eq!(model.model_name(), "Regime-Switching Prepayment");
        assert!(model.conditional_smm(24, &[2.0], 0.05, 1.0) > model.expected_smm(24));
    }

    #[test]
    fn test_deterministic_build_returns_none() {
        let spec = StochasticPrepaySpec::deterministic(PrepaymentModelSpec::psa(1.0));

        assert!(!spec.is_stochastic());
        assert!(spec.build().is_none());
    }

    #[test]
    fn test_standard_calibrations() {
        let rmbs = StochasticPrepaySpec::rmbs_agency(0.045);
        assert!(rmbs.is_stochastic());

        let clo = StochasticPrepaySpec::clo_standard();
        assert!(clo.is_stochastic());
    }

    #[test]
    fn test_clamping() {
        // Factor loading should be clamped to [-1, 1]
        let spec = StochasticPrepaySpec::factor_correlated(
            PrepaymentModelSpec::psa(1.0),
            5.0, // Too high
            2.0, // Too high
        );

        assert_eq!(spec.factor_loading(), Some(1.0));
    }
}
