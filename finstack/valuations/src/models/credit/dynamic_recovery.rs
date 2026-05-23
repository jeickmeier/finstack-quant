//! Dynamic (notional-dependent) recovery rate model.
//!
//! Recovery rates decline as PIK accrual increases the notional relative to
//! the asset base. This captures the intuition that higher leverage dilutes
//! recovery in default.
//!
//! # Supported models
//!
//! - **Constant**: `R(t) = R_0` (ignores notional).
//! - **InverseLinear**: `R(t) = R_0 * (N_0 / N(t))` -- direct proportional dilution.
//! - **InversePower**: `R(t) = R_0 * (N_0 / N(t))^alpha`, `alpha in (0, 1]` -- softened decline.
//! - **FlooredInverse**: `R(t) = max(floor, R_0 * (N_0 / N(t)))`.
//! - **LinearDecline**: `R(t) = clamp(R_0 * (1 - beta * (N(t)/N_0 - 1)), floor, R_0)`.
//!
//! All computed recovery rates are clamped to `[0, base_recovery]`.

use finstack_core::{InputError, Result};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Recovery model specification.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
pub enum RecoveryModel {
    /// Constant recovery (ignores notional changes).
    Constant,
    /// `R(t) = R_0 * (N_0 / N(t))` -- direct proportional dilution.
    InverseLinear,
    /// `R(t) = R_0 * (N_0 / N(t))^alpha`, `alpha in (0, 1]` -- softened decline.
    InversePower {
        /// Power exponent (`alpha`).
        exponent: f64,
    },
    /// `R(t) = max(floor, R_0 * (N_0 / N(t)))`.
    FlooredInverse {
        /// Minimum recovery rate floor.
        floor: f64,
    },
    /// `R(t) = clamp(R_0 * (1 - beta * (N(t)/N_0 - 1)), floor, R_0)`.
    LinearDecline {
        /// Sensitivity of recovery to leverage increase (`beta`).
        sensitivity: f64,
        /// Minimum recovery rate floor.
        floor: f64,
    },
}

/// Specification for dynamic (notional-dependent) recovery rate.
///
/// Models the relationship between the accreted notional and the recovery
/// rate in default. As PIK accrual increases the notional relative to the
/// original base, recovery declines according to the chosen [`RecoveryModel`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DynamicRecoverySpec {
    /// Base (reference) recovery rate `R_0`.
    base_recovery: f64,
    /// Base (reference) notional `N_0`.
    base_notional: f64,
    /// Recovery model governing the notional-to-recovery mapping.
    model: RecoveryModel,
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl DynamicRecoverySpec {
    // -- Convenience constructors -------------------------------------------

    /// Validate base parameters common to all non-constant models.
    fn validate(base_recovery: f64, base_notional: f64) -> Result<()> {
        if !(0.0..=1.0).contains(&base_recovery) {
            return Err(InputError::Invalid.into());
        }
        if base_notional <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }
        Ok(())
    }

    /// Create a constant recovery spec (ignores notional changes).
    ///
    /// This produces identical results to fixed-recovery pricing.
    ///
    /// # Errors
    ///
    /// Returns an error if `recovery` is outside `[0, 1]`.
    pub fn constant(recovery: f64) -> Result<Self> {
        if !(0.0..=1.0).contains(&recovery) {
            return Err(InputError::Invalid.into());
        }
        Ok(Self {
            base_recovery: recovery,
            base_notional: 1.0,
            model: RecoveryModel::Constant,
        })
    }

    /// Create an inverse-linear recovery spec.
    ///
    /// `R(N) = R_0 * (N_0 / N)`, clamped to `[0, R_0]`.
    ///
    /// # Errors
    ///
    /// Returns an error if `base_recovery` is outside `[0, 1]` or
    /// `base_notional <= 0`.
    pub fn inverse_linear(base_recovery: f64, base_notional: f64) -> Result<Self> {
        Self::validate(base_recovery, base_notional)?;
        Ok(Self {
            base_recovery,
            base_notional,
            model: RecoveryModel::InverseLinear,
        })
    }

    /// Create an inverse-power recovery spec.
    ///
    /// `R(N) = R_0 * (N_0 / N)^exponent`, clamped to `[0, R_0]`.
    ///
    /// # Errors
    ///
    /// Returns an error if `base_recovery` is outside `[0, 1]`,
    /// `base_notional <= 0`, or `exponent <= 0`.
    pub fn inverse_power(base_recovery: f64, base_notional: f64, exponent: f64) -> Result<Self> {
        Self::validate(base_recovery, base_notional)?;
        if exponent <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }
        Ok(Self {
            base_recovery,
            base_notional,
            model: RecoveryModel::InversePower { exponent },
        })
    }

    /// Create a floored inverse recovery spec.
    ///
    /// `R(N) = max(floor, R_0 * (N_0 / N))`, clamped to `[0, R_0]`.
    ///
    /// # Errors
    ///
    /// Returns an error if `base_recovery` is outside `[0, 1]`,
    /// `base_notional <= 0`, or `floor` is negative.
    pub fn floored_inverse(base_recovery: f64, base_notional: f64, floor: f64) -> Result<Self> {
        Self::validate(base_recovery, base_notional)?;
        if floor < 0.0 {
            return Err(InputError::NegativeValue.into());
        }
        Ok(Self {
            base_recovery,
            base_notional,
            model: RecoveryModel::FlooredInverse { floor },
        })
    }

    /// Create a linear-decline recovery spec.
    ///
    /// `R(N) = clamp(R_0 * (1 - sensitivity * (N/N_0 - 1)), floor, R_0)`.
    ///
    /// # Errors
    ///
    /// Returns an error if `base_recovery` is outside `[0, 1]`,
    /// `base_notional <= 0`, or `floor` is negative.
    pub fn linear_decline(
        base_recovery: f64,
        base_notional: f64,
        sensitivity: f64,
        floor: f64,
    ) -> Result<Self> {
        Self::validate(base_recovery, base_notional)?;
        if floor < 0.0 {
            return Err(InputError::NegativeValue.into());
        }
        Ok(Self {
            base_recovery,
            base_notional,
            model: RecoveryModel::LinearDecline { sensitivity, floor },
        })
    }

    // -- Core computation ---------------------------------------------------

    /// Compute recovery rate given current accreted notional.
    ///
    /// All results are clamped to `[0.0, base_recovery]`.
    pub fn recovery_at_notional(&self, current_notional: f64) -> f64 {
        if current_notional <= 0.0 {
            return 0.0;
        }
        let raw = match self.model {
            RecoveryModel::Constant => self.base_recovery,
            RecoveryModel::InverseLinear => {
                self.base_recovery * (self.base_notional / current_notional)
            }
            RecoveryModel::InversePower { exponent } => {
                self.base_recovery * (self.base_notional / current_notional).powf(exponent)
            }
            RecoveryModel::FlooredInverse { floor } => {
                let inv = self.base_recovery * (self.base_notional / current_notional);
                inv.max(floor)
            }
            RecoveryModel::LinearDecline { sensitivity, floor } => {
                let ratio = current_notional / self.base_notional;
                let r = self.base_recovery * (1.0 - sensitivity * (ratio - 1.0));
                r.max(floor)
            }
        };
        // Clamp to [0, base_recovery]
        raw.clamp(0.0, self.base_recovery)
    }

    // -- Accessors ----------------------------------------------------------

    /// Returns the base (reference) recovery rate.
    pub fn base_recovery(&self) -> f64 {
        self.base_recovery
    }

    /// Returns the base (reference) notional.
    pub fn base_notional(&self) -> f64 {
        self.base_notional
    }

    /// Returns a reference to the recovery model.
    pub fn model(&self) -> &RecoveryModel {
        &self.model
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_recovery_unchanged() {
        let spec = DynamicRecoverySpec::constant(0.40).unwrap();
        assert!((spec.recovery_at_notional(150.0) - 0.40).abs() < 1e-10);
        assert!((spec.recovery_at_notional(50.0) - 0.40).abs() < 1e-10);
    }

    #[test]
    fn inverse_linear_declines_with_notional() {
        let spec = DynamicRecoverySpec::inverse_linear(0.40, 100.0).unwrap();
        let r_at_par = spec.recovery_at_notional(100.0);
        let r_at_150 = spec.recovery_at_notional(150.0);
        assert!((r_at_par - 0.40).abs() < 1e-10);
        assert!((r_at_150 - 0.40 * 100.0 / 150.0).abs() < 1e-10);
        assert!(r_at_150 < r_at_par);
    }

    #[test]
    fn inverse_power_softer_decline() {
        let spec = DynamicRecoverySpec::inverse_power(0.40, 100.0, 0.5).unwrap();
        let r_par = spec.recovery_at_notional(100.0);
        let r_200 = spec.recovery_at_notional(200.0);
        assert!((r_par - 0.40).abs() < 1e-10);
        // With exponent 0.5: R = 0.40 * (100/200)^0.5 = 0.40 * sqrt(0.5) ≈ 0.2828
        assert!((r_200 - 0.40 * (0.5_f64).sqrt()).abs() < 1e-6);
    }

    #[test]
    fn floored_inverse_respects_floor() {
        let spec = DynamicRecoverySpec::floored_inverse(0.40, 100.0, 0.15).unwrap();
        let r_extreme = spec.recovery_at_notional(1000.0);
        assert!(
            (r_extreme - 0.15).abs() < 1e-10,
            "Should be floored at 15%, got {r_extreme}"
        );
    }

    #[test]
    fn linear_decline_formula() {
        let spec = DynamicRecoverySpec::linear_decline(0.40, 100.0, 0.5, 0.10).unwrap();
        // At N=120: R = 0.40 * (1 - 0.5 * (120/100 - 1)) = 0.40 * (1 - 0.1) = 0.40 * 0.90 = 0.36
        let r = spec.recovery_at_notional(120.0);
        assert!((r - 0.36).abs() < 1e-6, "Got {r}");
    }

    #[test]
    fn linear_decline_respects_floor() {
        let spec = DynamicRecoverySpec::linear_decline(0.40, 100.0, 0.5, 0.10).unwrap();
        // At very high notional, should hit floor
        let r = spec.recovery_at_notional(10000.0);
        assert!(
            (r - 0.10).abs() < 1e-10,
            "Should be floored at 10%, got {r}"
        );
    }

    #[test]
    fn recovery_never_exceeds_base() {
        let spec = DynamicRecoverySpec::inverse_linear(0.40, 100.0).unwrap();
        // At lower notional (N < N_0), recovery should be capped at base_recovery
        let r = spec.recovery_at_notional(50.0);
        assert!(
            (r - 0.40).abs() < 1e-10,
            "Should cap at base_recovery, got {r}"
        );
    }

    #[test]
    fn rejects_invalid_recovery() {
        assert!(DynamicRecoverySpec::constant(1.5).is_err());
        assert!(DynamicRecoverySpec::constant(-0.1).is_err());
        assert!(DynamicRecoverySpec::inverse_linear(0.40, -100.0).is_err());
        assert!(DynamicRecoverySpec::inverse_linear(0.40, 0.0).is_err());
        assert!(DynamicRecoverySpec::inverse_power(0.40, 100.0, 0.0).is_err());
        assert!(DynamicRecoverySpec::floored_inverse(0.40, 100.0, -0.1).is_err());
    }
}
