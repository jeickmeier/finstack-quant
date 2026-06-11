//! Stochastic recovery models for credit portfolio pricing.
//!
//! This module provides recovery-rate models used alongside latent-factor
//! default models. Recovery is expressed in decimals, so `0.40` means a 40%
//! recovery rate and `0.60` LGD.
//!
//! # Constant vs Stochastic Recovery
//!
//! - **Constant**: Simplest model, uses fixed recovery (e.g., 40%)
//! - **Stochastic**: Recovery varies with market conditions
//!
//! # Stochastic Recovery Models
//!
//! ## Market-Correlated (Andersen-Sidenius)
//!
//! Recovery is driven by a bounded transformation of a latent factor shock:
//! ```text
//! shock(Z) = ρ_R · σ_R · Z
//! R(Z) = bounded_transform(μ_R, shock(Z), min_R, max_R)
//! ```
//! where:
//! - `μ_R` is the target mean recovery at `Z = 0`
//! - `σ_R` is the recovery-volatility scale
//! - `ρ_R` controls the sign and magnitude of factor sensitivity
//! - the bounded transform maps the shocked recovery smoothly into
//!   `[min_R, max_R]`
//!
//! The sign convention for `Z` is caller-defined. In the current implementation,
//! the preset calibrations use a negative `ρ_R`, so negative factor realizations
//! increase recovery and positive realizations decrease it.
//!
//! # References
//!
//! - Default/recovery empirical evidence:
//!   `docs/REFERENCES.md#altman-et-al-2005-recovery`
//! - Stochastic recovery model context:
//!   `docs/REFERENCES.md#andersen-sidenius-2005-rfl`

mod constant;
mod correlated;

use crate::correlation::{Error, Result};

pub use constant::ConstantRecovery;
pub use correlated::CorrelatedRecovery;

/// Recovery rate model for credit portfolio pricing.
///
/// Implementations provide both unconditional expected recovery
/// and recovery conditional on market state.
pub trait RecoveryModel: Send + Sync + std::fmt::Debug {
    /// Expected (unconditional) recovery rate.
    ///
    /// This is the average recovery used for simple calculations
    /// and as the baseline for stochastic models.
    ///
    /// # Returns
    ///
    /// The unconditional recovery rate in decimal form.
    fn expected_recovery(&self) -> f64;

    /// Recovery rate conditional on the systematic market factor.
    ///
    /// For stochastic models, recovery varies with the supplied latent-factor
    /// realization. The sign convention is model-dependent: callers should treat
    /// positive and negative factor values as abstract states unless the concrete
    /// implementation documents a market interpretation.
    ///
    /// For constant models, this equals [`Self::expected_recovery`].
    ///
    /// # Arguments
    ///
    /// * `market_factor` - A latent-factor realization supplied by the caller.
    ///
    /// # Returns
    ///
    /// The conditional recovery rate in decimal form.
    fn conditional_recovery(&self, market_factor: f64) -> f64;

    /// Loss given default = 1 - recovery.
    ///
    /// # Returns
    ///
    /// The unconditional LGD in decimal form.
    fn lgd(&self) -> f64 {
        1.0 - self.expected_recovery()
    }

    /// Conditional LGD given market factor.
    ///
    /// # Arguments
    ///
    /// * `market_factor` - A latent-factor realization supplied by the caller.
    ///
    /// # Returns
    ///
    /// The conditional LGD in decimal form.
    fn conditional_lgd(&self, market_factor: f64) -> f64 {
        1.0 - self.conditional_recovery(market_factor)
    }

    /// Recovery-rate volatility scale used by the model.
    ///
    /// Returns `0.0` for constant models.
    ///
    /// # Returns
    ///
    /// The recovery-volatility scale in decimal form.
    fn recovery_volatility(&self) -> f64;

    /// Model name for diagnostics.
    ///
    /// # Returns
    ///
    /// A static human-readable model name.
    fn model_name(&self) -> &'static str;

    /// Whether this model is stochastic (varies with market factor).
    ///
    /// # Returns
    ///
    /// `true` if the model reports a non-zero recovery-volatility scale.
    fn is_stochastic(&self) -> bool {
        self.recovery_volatility() > 0.0
    }
}

/// Recovery model specification for configuration and serialization.
///
/// Allows recovery model selection without constructing the full model.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", deny_unknown_fields)]
#[non_exhaustive]
pub enum RecoverySpec {
    /// Constant recovery rate (current default behavior).
    Constant {
        /// Recovery rate ∈ [0, 1]
        rate: f64,
    },

    /// Recovery correlated with market factor (Andersen-Sidenius model).
    ///
    /// Uses the same bounded latent-factor shock as [`CorrelatedRecovery`]:
    /// the affine shock `correlation * volatility * Z` is passed through a
    /// smooth logistic transform so recovery stays inside the configured bounds.
    MarketCorrelated {
        /// Mean recovery rate
        mean_recovery: f64,
        /// Recovery volatility (standard deviation)
        recovery_volatility: f64,
        /// Correlation with the systematic factor. Under the canonical
        /// low-factor-stress convention this is typically POSITIVE: recovery
        /// falls when the factor falls, so defaults (high in stress) and
        /// recoveries co-move negatively.
        factor_correlation: f64,
    },
}

impl Default for RecoverySpec {
    fn default() -> Self {
        RecoverySpec::Constant { rate: 0.40 }
    }
}

impl RecoverySpec {
    /// Create constant recovery specification.
    ///
    /// # Arguments
    /// * `rate` - Recovery rate in [0.0, 1.0]
    ///
    /// # Returns
    ///
    /// A [`RecoverySpec::Constant`] configuration.
    ///
    /// # Errors
    ///
    /// Returns [`crate::correlation::Error`] if `rate` is not finite or lies
    /// outside `[0.0, 1.0]`.
    pub fn constant(rate: f64) -> Result<Self> {
        validate_recovery_input("rate", rate, 0.0, 1.0)?;
        Ok(RecoverySpec::Constant { rate })
    }

    /// Create market-correlated recovery specification.
    ///
    /// # Arguments
    /// * `mean` - Mean recovery rate in [0.0, 1.0]. Typical: 0.40
    /// * `vol` - Recovery volatility, clamped to [0.0, 0.5]. Typical: 0.20-0.30
    /// * `corr` - Correlation with factor, clamped to [-1.0, 1.0]. Typical: +0.30 to +0.50
    ///   (positive under the canonical low-factor-stress convention)
    ///
    /// # Returns
    ///
    /// A [`RecoverySpec::MarketCorrelated`] configuration.
    ///
    /// # Errors
    ///
    /// Returns [`crate::correlation::Error`] if `mean` is not finite or lies
    /// outside `[0.0, 1.0]`, or if `vol` / `corr` are not finite.
    pub fn market_correlated(mean: f64, vol: f64, corr: f64) -> Result<Self> {
        validate_recovery_input("mean_recovery", mean, 0.0, 1.0)?;
        validate_finite_recovery_input("recovery_volatility", vol)?;
        validate_finite_recovery_input("factor_correlation", corr)?;
        Ok(RecoverySpec::MarketCorrelated {
            mean_recovery: mean.clamp(0.0, 1.0),
            recovery_volatility: vol.clamp(0.0, 0.5),
            factor_correlation: corr.clamp(-1.0, 1.0),
        })
    }

    /// Create market-standard stochastic recovery.
    ///
    /// Uses typical calibration from CDX equity tranche:
    /// - Mean: 40%
    /// - Vol: 25%
    /// - Correlation: +40% (recovery falls when the systematic factor falls,
    ///   per the canonical low-factor-stress convention)
    ///
    /// # Returns
    ///
    /// The default stochastic-recovery specification used by this crate.
    #[must_use]
    pub fn market_standard_stochastic() -> Self {
        RecoverySpec::MarketCorrelated {
            mean_recovery: 0.40,
            recovery_volatility: 0.25,
            factor_correlation: 0.40,
        }
    }

    /// Build the recovery model instance from this specification.
    ///
    /// # Returns
    ///
    /// A boxed [`RecoveryModel`] implementation matching the specification.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::correlation::{RecoveryModel, RecoverySpec};
    ///
    /// let model = RecoverySpec::constant(0.40)?.build();
    /// assert_eq!(model.expected_recovery(), 0.40);
    /// # Ok::<(), finstack_valuations::correlation::Error>(())
    /// ```
    #[must_use]
    pub fn build(&self) -> Box<dyn RecoveryModel> {
        match self {
            RecoverySpec::Constant { rate } => Box::new(ConstantRecovery::new(*rate)),
            RecoverySpec::MarketCorrelated {
                mean_recovery,
                recovery_volatility,
                factor_correlation,
            } => Box::new(CorrelatedRecovery::new(
                *mean_recovery,
                *recovery_volatility,
                *factor_correlation,
            )),
        }
    }

    /// Get the location-parameter recovery rate from the specification.
    ///
    /// For [`RecoverySpec::Constant`], this is the constant recovery rate.
    /// For [`RecoverySpec::MarketCorrelated`], this returns the `mean_recovery`
    /// input (the target recovery at `Z = 0`), which differs from the
    /// Jensen-corrected unconditional mean `E_Z[R(Z)]` whenever the factor
    /// sensitivity is non-zero. To obtain the true expected recovery, call
    /// [`Self::build`] and then [`RecoveryModel::expected_recovery`].
    ///
    /// # Returns
    ///
    /// The location-parameter recovery in decimal form.
    #[must_use]
    pub fn expected_recovery(&self) -> f64 {
        match self {
            RecoverySpec::Constant { rate } => *rate,
            RecoverySpec::MarketCorrelated { mean_recovery, .. } => *mean_recovery,
        }
    }
}

fn validate_finite_recovery_input(field: &str, value: f64) -> Result<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(Error::InvalidRecoveryInput {
            field: field.to_string(),
            value,
            requirement: "must be finite".to_string(),
        })
    }
}

fn validate_recovery_input(field: &str, value: f64, min: f64, max: f64) -> Result<()> {
    validate_finite_recovery_input(field, value)?;
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(Error::InvalidRecoveryInput {
            field: field.to_string(),
            value,
            requirement: format!("must be in [{min}, {max}]"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_spec_default() {
        let spec = RecoverySpec::default();
        assert!(matches!(spec, RecoverySpec::Constant { rate } if (rate - 0.40).abs() < 1e-10));
    }

    #[test]
    fn test_recovery_spec_builders() {
        let constant = RecoverySpec::constant(0.35).expect("valid recovery rate");
        assert!((constant.expected_recovery() - 0.35).abs() < 1e-10);

        let correlated =
            RecoverySpec::market_correlated(0.40, 0.25, 0.40).expect("valid recovery inputs");
        assert!((correlated.expected_recovery() - 0.40).abs() < 1e-10);

        let market_std = RecoverySpec::market_standard_stochastic();
        assert!((market_std.expected_recovery() - 0.40).abs() < 1e-10);
    }

    #[test]
    fn test_recovery_spec_clamping() {
        // Recovery rates are caller input, so out-of-range values are rejected
        // rather than silently clamped.
        let high = RecoverySpec::constant(1.5);
        assert!(matches!(high, Err(Error::InvalidRecoveryInput { .. })));

        let low = RecoverySpec::constant(-0.1);
        assert!(matches!(low, Err(Error::InvalidRecoveryInput { .. })));
    }

    #[test]
    fn test_recovery_spec_build() {
        let specs = vec![
            RecoverySpec::constant(0.40).expect("valid constant recovery"),
            RecoverySpec::market_correlated(0.40, 0.25, 0.40)
                .expect("valid market-correlated recovery"),
        ];

        for spec in specs {
            let model = spec.build();
            assert!(model.expected_recovery() >= 0.0);
            assert!(model.expected_recovery() <= 1.0);
            assert!(!model.model_name().is_empty());
        }
    }

    #[test]
    fn test_recovery_spec_rejects_beta_variant() {
        let err =
            serde_json::from_str::<RecoverySpec>(r#"{"type":"Beta","mean":0.4,"std_dev":0.15}"#)
                .expect_err("Beta recovery should not deserialize");

        assert!(err.to_string().contains("unknown variant"));
    }

    #[test]
    fn test_recovery_spec_rejects_frye_variant() {
        let err = serde_json::from_str::<RecoverySpec>(
            r#"{"type":"Frye","base_lgd":0.6,"lgd_sensitivity":1.5}"#,
        )
        .expect_err("Frye recovery should not deserialize");

        assert!(err.to_string().contains("unknown variant"));
    }

    #[test]
    fn test_lgd_calculation() {
        let model = ConstantRecovery::new(0.40);
        assert!((model.lgd() - 0.60).abs() < 1e-10);
    }
}
