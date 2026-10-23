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
//! Low `Z` is the canonical stress state. Preset calibrations use positive
//! `ρ_R`, so negative factor realizations decrease recovery.
//!
//! # References
//!
//! - Default/recovery empirical evidence: `docs/REFERENCES.md#altman-et-al-2005-recovery`
//!
//! - Stochastic recovery model context: `docs/REFERENCES.md#andersen-sidenius-2005-rfl`
//!

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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case", try_from = "RawRecoverySpec")]
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

#[derive(serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum RawRecoverySpec {
    Constant {
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::ClosedUnitIntervalF64Wire")
        )]
        rate: f64,
    },
    MarketCorrelated {
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::ClosedUnitIntervalF64Wire")
        )]
        mean_recovery: f64,
        #[cfg_attr(feature = "json-schema", schemars(range(min = 0.0, max = 0.5)))]
        recovery_volatility: f64,
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::CorrelationWire")
        )]
        factor_correlation: f64,
    },
}

impl TryFrom<RawRecoverySpec> for RecoverySpec {
    type Error = Error;

    fn try_from(raw: RawRecoverySpec) -> Result<Self> {
        let spec = match raw {
            RawRecoverySpec::Constant { rate } => RecoverySpec::Constant { rate },
            RawRecoverySpec::MarketCorrelated {
                mean_recovery,
                recovery_volatility,
                factor_correlation,
            } => RecoverySpec::MarketCorrelated {
                mean_recovery,
                recovery_volatility,
                factor_correlation,
            },
        };
        spec.validate()?;
        Ok(spec)
    }
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
    /// * `mean` - Recovery at zero factor in [0.0, 1.0]. Endpoints produce
    ///   constant zero or full recovery. Typical: 0.40.
    /// * `vol` - Recovery volatility, finite and in [0.0, 0.5]. Typical: 0.20-0.30
    /// * `corr` - Correlation with factor, finite and in [-1.0, 1.0]. Typical: +0.30 to +0.50
    ///   (positive under the canonical low-factor-stress convention)
    ///
    /// # Returns
    ///
    /// A [`RecoverySpec::MarketCorrelated`] configuration.
    ///
    /// # Errors
    ///
    /// Returns [`crate::correlation::Error`] if `mean` is not finite or lies
    /// outside `[0.0, 1.0]`, or if `vol` is outside `[0, 0.5]`, `corr` is outside `[-1, 1]`,
    /// or either is non-finite.
    pub fn market_correlated(mean: f64, vol: f64, corr: f64) -> Result<Self> {
        validate_correlated_inputs(mean, vol, corr)?;
        Ok(RecoverySpec::MarketCorrelated {
            mean_recovery: mean,
            recovery_volatility: vol,
            factor_correlation: corr,
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
    pub fn market_standard_stochastic() -> Result<Self> {
        Self::market_correlated(0.40, 0.25, 0.40)
    }

    /// Validate the recovery specification without constructing a model.
    ///
    /// # Errors
    ///
    /// Returns [`crate::correlation::Error`] when any recovery parameter is
    /// non-finite or outside its documented range.
    pub fn validate(&self) -> Result<()> {
        match self {
            RecoverySpec::Constant { rate } => validate_recovery_input("rate", *rate, 0.0, 1.0),
            RecoverySpec::MarketCorrelated {
                mean_recovery,
                recovery_volatility,
                factor_correlation,
            } => validate_correlated_inputs(
                *mean_recovery,
                *recovery_volatility,
                *factor_correlation,
            ),
        }
    }

    /// Build the recovery model instance from this specification.
    ///
    /// # Returns
    ///
    /// A boxed [`RecoveryModel`] implementation matching the specification.
    ///
    /// # Errors
    ///
    /// Returns a validation error for invalid inputs, or a core computation
    /// error if expected-recovery integration fails.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_models::correlation::{RecoveryModel, RecoverySpec};
    ///
    /// let model = RecoverySpec::constant(0.40)?.build()?;
    /// assert_eq!(model.expected_recovery(), 0.40);
    /// # Ok::<(), finstack_quant_models::correlation::Error>(())
    /// ```
    pub fn build(&self) -> Result<Box<dyn RecoveryModel>> {
        Ok(match self {
            RecoverySpec::Constant { rate } => {
                Box::new(ConstantRecovery::new(*rate)?) as Box<dyn RecoveryModel>
            }
            RecoverySpec::MarketCorrelated {
                mean_recovery,
                recovery_volatility,
                factor_correlation,
            } => Box::new(CorrelatedRecovery::new(
                *mean_recovery,
                *recovery_volatility,
                *factor_correlation,
            )?),
        })
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

fn validate_correlated_inputs(mean: f64, vol: f64, corr: f64) -> Result<()> {
    validate_recovery_input("mean_recovery", mean, 0.0, 1.0)?;
    validate_recovery_input("recovery_volatility", vol, 0.0, 0.5)?;
    validate_recovery_input("factor_correlation", corr, -1.0, 1.0)
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
    fn invalid_recovery_inputs_fail_every_construction_path() {
        for (mean, vol, corr) in [
            (-0.1, 0.2, 0.4),
            (1.1, 0.2, 0.4),
            (0.4, -0.1, 0.4),
            (0.4, 0.6, 0.4),
            (0.4, 0.2, -1.1),
            (0.4, 0.2, 1.1),
        ] {
            assert!(CorrelatedRecovery::new(mean, vol, corr).is_err());
            assert!(RecoverySpec::market_correlated(mean, vol, corr).is_err());
            let spec = RecoverySpec::MarketCorrelated {
                mean_recovery: mean,
                recovery_volatility: vol,
                factor_correlation: corr,
            };
            assert!(spec.build().is_err());
            let wire = serde_json::to_value(spec).expect("wire");
            assert!(serde_json::from_value::<RecoverySpec>(wire).is_err());
        }
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(CorrelatedRecovery::new(bad, 0.2, 0.4).is_err());
            assert!(CorrelatedRecovery::new(0.4, bad, 0.4).is_err());
            assert!(CorrelatedRecovery::new(0.4, 0.2, bad).is_err());
        }
        for corr in [-1.0, 1.0] {
            let model = CorrelatedRecovery::new(0.4, 0.5, corr).expect("boundary inputs");
            assert!(model.expected_recovery().is_finite());
        }
    }

    #[test]
    fn building_preserves_every_accepted_recovery_location() {
        for mean in [0.0, 0.01, 0.03, 0.40, 0.98, 1.0] {
            let spec = RecoverySpec::market_correlated(mean, 0.0, 0.4).expect("valid recovery");
            let model = spec.build().expect("valid recovery model");
            assert!((model.conditional_recovery(0.0) - mean).abs() < 1e-12);
            assert!((model.expected_recovery() - mean).abs() < 1e-12);
        }
    }

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

        let market_std = RecoverySpec::market_standard_stochastic().expect("valid model inputs");
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
            let model = spec.build().expect("valid recovery model");
            assert!(model.expected_recovery() >= 0.0);
            assert!(model.expected_recovery() <= 1.0);
            assert!(!model.model_name().is_empty());
        }
    }

    #[test]
    fn test_recovery_spec_rejects_beta_variant() {
        // schema-rejection-test
        let err =
            serde_json::from_str::<RecoverySpec>(r#"{"type":"Beta","mean":0.4,"std_dev":0.15}"#)
                .expect_err("Beta recovery should not deserialize");

        assert!(err.to_string().contains("unknown variant"));
    }

    #[test]
    fn test_recovery_spec_rejects_frye_variant() {
        // schema-rejection-test
        let err = serde_json::from_str::<RecoverySpec>(
            r#"{"type":"Frye","base_lgd":0.6,"lgd_sensitivity":1.5}"#,
        )
        .expect_err("Frye recovery should not deserialize");

        assert!(err.to_string().contains("unknown variant"));
    }

    #[test]
    fn test_lgd_calculation() {
        let model = ConstantRecovery::new(0.40).expect("valid model inputs");
        assert!((model.lgd() - 0.60).abs() < 1e-10);
    }
}
