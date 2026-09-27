use finstack_quant_core::{Error, Result};

/// Asset dynamics specification for the Merton model.
///
/// Controls the stochastic process assumed for the firm's asset value.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AssetDynamics {
    /// Standard geometric Brownian motion (lognormal diffusion).
    GeometricBrownian,
    /// Jump-diffusion process (Merton 1976) with Poisson jumps.
    JumpDiffusion {
        /// Poisson jump arrival intensity (jumps per year).
        jump_intensity: f64,
        /// Mean log-jump size.
        jump_mean: f64,
        /// Volatility of log-jump size.
        jump_vol: f64,
    },
    /// CreditGrades model extension with stochastic-barrier survival adjustment.
    ///
    /// `default_probability()` applies the standard approximate CreditGrades
    /// survival function using the log-barrier volatility parameter.
    CreditGrades {
        /// Log-normal barrier volatility `λ`: the standard deviation of the
        /// natural log of the default barrier (Finger et al. 2002,
        /// "CreditGrades Technical Document"). Despite the field name, this
        /// is *not* a generic uncertainty scalar — it is the lognormal
        /// dispersion of the global recovery rate, entering the survival
        /// formula as `a_t² = σ²t + λ²` and the barrier shift `exp(λ²)`.
        barrier_uncertainty: f64,
        /// Mean recovery rate at default.
        mean_recovery: f64,
    },
}

impl AssetDynamics {
    /// Reject parameter values that fall outside the process's domain.
    ///
    /// Every construction path runs this, so a model can never hold a
    /// negative jump intensity, a negative log-barrier volatility, or a
    /// recovery rate outside `[0, 1]`. Without it those values would be
    /// silently sanitized (or produce a wrong drift compensator) far from
    /// the point where they entered the system.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] naming the offending field when a jump
    /// parameter is non-finite or negative, when `barrier_uncertainty` is
    /// non-finite or negative, or when `mean_recovery` is outside `[0, 1]`.
    pub(super) fn validate(&self) -> Result<()> {
        match *self {
            Self::GeometricBrownian => Ok(()),
            Self::JumpDiffusion {
                jump_intensity,
                jump_mean,
                jump_vol,
            } => {
                if !(jump_intensity.is_finite() && jump_intensity >= 0.0) {
                    return Err(Error::Validation(format!(
                        "AssetDynamics::JumpDiffusion: jump_intensity must be finite and >= 0, \
                         got {jump_intensity}"
                    )));
                }
                if !jump_mean.is_finite() {
                    return Err(Error::Validation(format!(
                        "AssetDynamics::JumpDiffusion: jump_mean must be finite, got {jump_mean}"
                    )));
                }
                if !(jump_vol.is_finite() && jump_vol >= 0.0) {
                    return Err(Error::Validation(format!(
                        "AssetDynamics::JumpDiffusion: jump_vol must be finite and >= 0, \
                         got {jump_vol}"
                    )));
                }
                Ok(())
            }
            Self::CreditGrades {
                barrier_uncertainty,
                mean_recovery,
            } => {
                if !(barrier_uncertainty.is_finite() && barrier_uncertainty >= 0.0) {
                    return Err(Error::Validation(format!(
                        "AssetDynamics::CreditGrades: barrier_uncertainty (log-barrier vol λ) \
                         must be finite and >= 0, got {barrier_uncertainty}"
                    )));
                }
                if !(0.0..=1.0).contains(&mean_recovery) {
                    return Err(Error::Validation(format!(
                        "AssetDynamics::CreditGrades: mean_recovery must be in [0, 1], \
                         got {mean_recovery}"
                    )));
                }
                Ok(())
            }
        }
    }
}

/// Barrier monitoring type for default determination.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum MertonBarrierType {
    /// Default only assessed at maturity (classic Merton).
    Terminal,
    /// Continuous barrier monitoring (Black-Cox extension).
    FirstPassage {
        /// Growth rate of the default barrier over time.
        barrier_growth_rate: f64,
    },
}

#[cfg(test)]
mod tests {
    use super::super::{AssetDynamics, MertonBarrierType, MertonModel};

    #[test]
    fn new_rejects_invalid_inputs() {
        assert!(MertonModel::new(0.0, 0.20, 80.0, 0.05).is_err());
        assert!(MertonModel::new(-1.0, 0.20, 80.0, 0.05).is_err());
        assert!(MertonModel::new(100.0, -0.20, 80.0, 0.05).is_err());
        assert!(MertonModel::new(100.0, 0.20, 0.0, 0.05).is_err());
    }

    // Construction validation

    #[test]
    fn new_with_dynamics_rejects_out_of_domain_dynamics() {
        let build = |dynamics| {
            MertonModel::new_with_dynamics(
                100.0,
                0.20,
                80.0,
                0.05,
                0.0,
                MertonBarrierType::Terminal,
                dynamics,
            )
        };
        assert!(build(AssetDynamics::JumpDiffusion {
            jump_intensity: -0.5,
            jump_mean: 0.0,
            jump_vol: 0.1,
        })
        .is_err());
        assert!(build(AssetDynamics::JumpDiffusion {
            jump_intensity: 0.5,
            jump_mean: 0.0,
            jump_vol: -0.1,
        })
        .is_err());
        assert!(build(AssetDynamics::JumpDiffusion {
            jump_intensity: f64::NAN,
            jump_mean: 0.0,
            jump_vol: 0.1,
        })
        .is_err());
    }

    #[test]
    fn credit_grades_requires_flat_first_passage_barrier() {
        let build = |barrier_type| {
            MertonModel::new_with_dynamics(
                100.0,
                0.20,
                80.0,
                0.05,
                0.0,
                barrier_type,
                AssetDynamics::CreditGrades {
                    barrier_uncertainty: 0.30,
                    mean_recovery: 0.40,
                },
            )
        };
        assert!(build(MertonBarrierType::Terminal).is_err());
        assert!(build(MertonBarrierType::FirstPassage {
            barrier_growth_rate: 0.02
        })
        .is_err());
        assert!(build(MertonBarrierType::FirstPassage {
            barrier_growth_rate: 0.0
        })
        .is_ok());
        // A negative log-barrier volatility is rejected rather than clamped.
        assert!(MertonModel::new_with_dynamics(
            100.0,
            0.20,
            80.0,
            0.05,
            0.0,
            MertonBarrierType::FirstPassage {
                barrier_growth_rate: 0.0
            },
            AssetDynamics::CreditGrades {
                barrier_uncertainty: -0.30,
                mean_recovery: 0.40,
            },
        )
        .is_err());
    }

    #[test]
    fn deserialization_enforces_constructor_invariants() {
        let valid = MertonModel::new(100.0, 0.25, 80.0, 0.04).expect("valid");
        let json = serde_json::to_string(&valid).expect("serialize");
        let round_tripped: MertonModel = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(round_tripped, valid);

        // Negative volatility must not survive deserialization.
        let bad_vol = json.replace("\"asset_vol\":0.25", "\"asset_vol\":-0.25");
        assert!(serde_json::from_str::<MertonModel>(&bad_vol).is_err());

        // Neither must an unsupported dynamics/barrier pairing.
        let jd_first_passage = r#"{
                "asset_value": 100.0,
                "asset_vol": 0.2,
                "debt_barrier": 80.0,
                "risk_free_rate": 0.05,
                "payout_rate": 0.0,
                "barrier_type": {"first_passage": {"barrier_growth_rate": 0.0}},
                "dynamics": {"jump_diffusion": {"jump_intensity": 0.5, "jump_mean": -0.3, "jump_vol": 0.15}}
            }"#;
        assert!(serde_json::from_str::<MertonModel>(jd_first_passage).is_err());

        // Nor an out-of-domain jump intensity.
        let negative_intensity = r#"{
                "asset_value": 100.0,
                "asset_vol": 0.2,
                "debt_barrier": 80.0,
                "risk_free_rate": 0.05,
                "payout_rate": 0.0,
                "barrier_type": "terminal",
                "dynamics": {"jump_diffusion": {"jump_intensity": -0.5, "jump_mean": -0.3, "jump_vol": 0.15}}
            }"#;
        assert!(serde_json::from_str::<MertonModel>(negative_intensity).is_err());
    }

    #[test]
    fn jump_diffusion_rejects_first_passage_barrier() {
        let result = MertonModel::new_with_dynamics(
            100.0,
            0.20,
            80.0,
            0.05,
            0.0,
            MertonBarrierType::FirstPassage {
                barrier_growth_rate: 0.0,
            },
            AssetDynamics::JumpDiffusion {
                jump_intensity: 0.5,
                jump_mean: -0.30,
                jump_vol: 0.15,
            },
        );
        assert!(result.is_err(), "JD + first passage has no closed form");
    }
}
