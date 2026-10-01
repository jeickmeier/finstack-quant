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

use finstack_quant_core::{Error, InputError, Result};
use serde::{Deserialize, Serialize};

/// Recovery model specification.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
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
/// Deserialization enforces the same parameter invariants as the constructors.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawDynamicRecoverySpec")]
pub struct DynamicRecoverySpec {
    /// Base (reference) recovery rate `R_0`.
    base_recovery: f64,
    /// Base (reference) notional `N_0`.
    base_notional: f64,
    /// Recovery model governing the notional-to-recovery mapping.
    model: RecoveryModel,
}

#[derive(Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "json-schema", schemars(rename = "DynamicRecoverySpec"))]
#[serde(deny_unknown_fields)]
struct RawDynamicRecoverySpec {
    /// Base (reference) recovery rate `R_0`.
    base_recovery: f64,
    /// Base (reference) notional `N_0`.
    base_notional: f64,
    /// Recovery model governing the notional-to-recovery mapping.
    model: RecoveryModel,
}

impl TryFrom<RawDynamicRecoverySpec> for DynamicRecoverySpec {
    type Error = Error;

    fn try_from(raw: RawDynamicRecoverySpec) -> Result<Self> {
        Self::new(raw.base_recovery, raw.base_notional, raw.model)
    }
}

impl DynamicRecoverySpec {
    fn new(base_recovery: f64, base_notional: f64, model: RecoveryModel) -> Result<Self> {
        if !(0.0..=1.0).contains(&base_recovery) {
            return Err(InputError::Invalid.into());
        }
        if !base_notional.is_finite() || base_notional <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }
        match model {
            RecoveryModel::InversePower { exponent }
                if !exponent.is_finite() || exponent <= 0.0 || exponent > 1.0 =>
            {
                return Err(Error::Validation(
                    "inverse-power recovery exponent must be finite and in (0, 1]".into(),
                ));
            }
            RecoveryModel::LinearDecline { sensitivity, .. }
                if !sensitivity.is_finite() || sensitivity < 0.0 =>
            {
                return Err(Error::Validation(
                    "linear-decline recovery sensitivity must be finite and non-negative".into(),
                ));
            }
            _ => {}
        }
        if let RecoveryModel::FlooredInverse { floor }
        | RecoveryModel::LinearDecline { floor, .. } = model
        {
            if !floor.is_finite() || !(0.0..=base_recovery).contains(&floor) {
                return Err(Error::Validation(format!(
                    "recovery floor must be finite and in [0, {base_recovery}], got {floor}"
                )));
            }
        }
        Ok(Self {
            base_recovery,
            base_notional,
            model,
        })
    }

    /// Create a constant recovery spec (ignores notional changes).
    ///
    /// This produces identical results to fixed-recovery pricing.
    ///
    /// # Arguments
    ///
    /// * `recovery` - Finite fraction of outstanding notional recovered on
    ///   default, in `[0, 1]`.
    ///
    /// # Errors
    ///
    /// Returns an error if `recovery` is outside `[0, 1]`.
    pub fn constant(recovery: f64) -> Result<Self> {
        Self::new(recovery, 1.0, RecoveryModel::Constant)
    }

    /// Create an inverse-linear recovery spec.
    ///
    /// `R(N) = R_0 * (N_0 / N)`, clamped to `[0, R_0]`.
    ///
    /// # Arguments
    ///
    /// * `base_recovery` - Finite recovery fraction at the reference notional,
    ///   in `[0, 1]`.
    /// * `base_notional` - Finite positive reference debt amount, in the same
    ///   monetary units as subsequent notional queries.
    ///
    /// # Errors
    ///
    /// Returns an error if `base_recovery` is outside `[0, 1]` or
    /// `base_notional` is non-finite or non-positive.
    pub fn inverse_linear(base_recovery: f64, base_notional: f64) -> Result<Self> {
        Self::new(base_recovery, base_notional, RecoveryModel::InverseLinear)
    }

    /// Create an inverse-power recovery spec.
    ///
    /// `R(N) = R_0 * (N_0 / N)^exponent`, clamped to `[0, R_0]`.
    ///
    /// # Arguments
    ///
    /// * `base_recovery` - Finite reference recovery fraction in `[0, 1]`.
    /// * `base_notional` - Finite positive reference debt amount in the query
    ///   notional's monetary units.
    /// * `exponent` - Finite dilution exponent in `(0, 1]`; values below one
    ///   soften recovery's decline relative to inverse-linear dilution.
    ///
    /// # Errors
    ///
    /// Returns an error if `base_recovery` is outside `[0, 1]`,
    /// the reference notional is non-finite or non-positive, or `exponent`
    /// is not finite and in `(0, 1]`.
    pub fn inverse_power(base_recovery: f64, base_notional: f64, exponent: f64) -> Result<Self> {
        Self::new(
            base_recovery,
            base_notional,
            RecoveryModel::InversePower { exponent },
        )
    }

    /// Create a floored inverse recovery spec.
    ///
    /// `R(N) = max(floor, R_0 * (N_0 / N))`, clamped to `[0, R_0]`.
    ///
    /// # Arguments
    ///
    /// * `base_recovery` - Finite reference recovery fraction in `[0, 1]`.
    /// * `base_notional` - Finite positive reference debt amount in the query
    ///   notional's monetary units.
    /// * `floor` - Finite minimum recovery fraction in `[0, base_recovery]`.
    ///
    /// # Errors
    ///
    /// Returns an error if `base_recovery` is outside `[0, 1]`,
    /// the reference notional is non-finite or non-positive, or `floor` is
    /// not finite and in `[0, base_recovery]`.
    pub fn floored_inverse(base_recovery: f64, base_notional: f64, floor: f64) -> Result<Self> {
        Self::new(
            base_recovery,
            base_notional,
            RecoveryModel::FlooredInverse { floor },
        )
    }

    /// Create a linear-decline recovery spec.
    ///
    /// `R(N) = clamp(R_0 * (1 - sensitivity * (N/N_0 - 1)), floor, R_0)`.
    ///
    /// # Arguments
    ///
    /// * `base_recovery` - Finite reference recovery fraction in `[0, 1]`.
    /// * `base_notional` - Finite positive reference debt amount in the query
    ///   notional's monetary units.
    /// * `sensitivity` - Finite non-negative decline per unit increase in the
    ///   current-to-reference notional ratio; zero keeps recovery constant.
    /// * `floor` - Finite minimum recovery fraction in `[0, base_recovery]`.
    ///
    /// # Errors
    ///
    /// Returns an error if `base_recovery` is outside `[0, 1]`,
    /// the reference notional is non-finite or non-positive, the sensitivity
    /// is non-finite or negative, or the floor lies outside `[0, base_recovery]`.
    pub fn linear_decline(
        base_recovery: f64,
        base_notional: f64,
        sensitivity: f64,
        floor: f64,
    ) -> Result<Self> {
        Self::new(
            base_recovery,
            base_notional,
            RecoveryModel::LinearDecline { sensitivity, floor },
        )
    }

    /// Compute recovery rate given current accreted notional.
    ///
    /// All results are clamped to `[0.0, base_recovery]`.
    ///
    /// # Arguments
    ///
    /// * `current_notional` - Current accreted debt amount in the reference
    ///   notional's monetary units; a non-positive amount returns zero.
    pub fn recovery_at_notional(&self, current_notional: f64) -> f64 {
        if current_notional <= 0.0 {
            return 0.0;
        }
        if current_notional <= self.base_notional || self.base_recovery == 0.0 {
            return self.base_recovery;
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
                if sensitivity == 0.0 {
                    return self.base_recovery;
                }
                let ratio = current_notional / self.base_notional;
                let r = self.base_recovery * (1.0 - sensitivity * (ratio - 1.0));
                r.max(floor)
            }
        };
        raw.clamp(0.0, self.base_recovery)
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialization_rejects_recovery_that_would_overpay_or_panic() {
        for recovery in [-0.1, 1.5] {
            let json = serde_json::json!({
                "base_recovery": recovery,
                "base_notional": 100.0,
                "model": "constant"
            });
            assert!(serde_json::from_value::<DynamicRecoverySpec>(json).is_err());
        }
    }

    #[test]
    fn constructors_and_wire_reject_invalid_model_parameters() {
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(DynamicRecoverySpec::inverse_linear(0.4, invalid).is_err());
            assert!(DynamicRecoverySpec::inverse_power(0.4, 100.0, invalid).is_err());
            assert!(DynamicRecoverySpec::floored_inverse(0.4, 100.0, invalid).is_err());
            assert!(DynamicRecoverySpec::linear_decline(0.4, 100.0, invalid, 0.1).is_err());
        }
        assert!(DynamicRecoverySpec::inverse_power(0.4, 100.0, 1.1).is_err());
        assert!(DynamicRecoverySpec::linear_decline(0.4, 100.0, -0.1, 0.1).is_err());
        for model in [
            serde_json::json!({"inverse_power": {"exponent": -0.5}}),
            serde_json::json!({"inverse_power": {"exponent": 1.1}}),
            serde_json::json!({"floored_inverse": {"floor": 0.5}}),
            serde_json::json!({"linear_decline": {"sensitivity": -1.0, "floor": 0.1}}),
        ] {
            let json = serde_json::json!({
                "base_recovery": 0.4,
                "base_notional": 100.0,
                "model": model
            });
            assert!(serde_json::from_value::<DynamicRecoverySpec>(json).is_err());
        }
    }

    #[test]
    fn valid_models_round_trip_without_changing_recovery() {
        for spec in [
            DynamicRecoverySpec::constant(0.4).expect("constant"),
            DynamicRecoverySpec::inverse_linear(0.4, 100.0).expect("inverse"),
            DynamicRecoverySpec::inverse_power(0.4, 100.0, 0.5).expect("power"),
            DynamicRecoverySpec::floored_inverse(0.4, 100.0, 0.1).expect("floor"),
            DynamicRecoverySpec::linear_decline(0.4, 100.0, 0.5, 0.1).expect("decline"),
        ] {
            let restored: DynamicRecoverySpec = serde_json::from_value(
                serde_json::to_value(spec).expect("serialize valid recovery"),
            )
            .expect("deserialize valid recovery");
            assert_eq!(restored, spec);
            assert!(
                (restored.recovery_at_notional(150.0) - spec.recovery_at_notional(150.0)).abs()
                    < f64::EPSILON
            );
        }
    }

    #[test]
    fn zero_recovery_and_zero_sensitivity_remain_constant_at_extreme_notionals() {
        let zero = DynamicRecoverySpec::inverse_linear(0.0, f64::MAX).expect("zero recovery");
        assert!(zero.recovery_at_notional(f64::MIN_POSITIVE).abs() < f64::EPSILON);
        let constant = DynamicRecoverySpec::linear_decline(0.4, f64::MIN_POSITIVE, 0.0, 0.1)
            .expect("zero sensitivity");
        assert!((constant.recovery_at_notional(f64::MAX) - 0.4).abs() < f64::EPSILON);
    }

    /// A `floor` above `base_recovery` is inoperative — the outer
    /// `clamp(0, base_recovery)` silently overrides it — so the constructors
    /// must reject it rather than accept a floor that never applies.
    #[test]
    fn floor_above_base_recovery_is_rejected() {
        assert!(
            DynamicRecoverySpec::floored_inverse(0.40, 100.0, 0.50).is_err(),
            "floored_inverse must reject floor > base_recovery"
        );
        assert!(
            DynamicRecoverySpec::linear_decline(0.40, 100.0, 1.0, 0.50).is_err(),
            "linear_decline must reject floor > base_recovery"
        );
        // A floor equal to base_recovery is degenerate but consistent.
        assert!(DynamicRecoverySpec::floored_inverse(0.40, 100.0, 0.40).is_ok());
    }

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
