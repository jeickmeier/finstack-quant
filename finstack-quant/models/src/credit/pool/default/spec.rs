//! Stochastic default specification.
//!
//! Provides a serializable specification enum for stochastic default models,
//! enabling configuration and deferred construction.

use super::super::clamped_cdr_to_mdr;
use super::{
    CopulaBasedDefault, FactorCorrelatedDefault, IntensityProcessDefault, StochasticDefault,
};
use crate::correlation::copula::CopulaSpec;
use finstack_quant_cashflows::builder::specs::DefaultModelSpec;

/// Stochastic default model specification.
///
/// Allows default model selection and configuration without
/// constructing the full model.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "model", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum StochasticDefaultSpec {
    /// Use deterministic default model (no stochastic component).
    Deterministic(DefaultModelSpec),

    /// Copula-based default correlation model.
    ///
    /// Uses Li (2000) framework with specified copula.
    Copula {
        /// Base annual CDR
        base_cdr: f64,
        /// Copula specification
        copula_spec: CopulaSpec,
        /// Asset correlation
        correlation: f64,
    },

    /// Intensity process (Cox model) for default.
    ///
    /// Mean-reverting intensity with factor loading.
    IntensityProcess {
        /// Base annual hazard rate
        base_hazard: f64,
        /// Loading (β) on the systematic factor, clamped to [-1, 1]
        factor_loading: f64,
        /// Mean reversion speed
        mean_reversion: f64,
        /// Intensity volatility
        volatility: f64,
        /// Asset correlation
        #[serde(default = "default_correlation")]
        correlation: f64,
    },

    /// Factor-correlated CDR model.
    ///
    /// Simple model that shocks base CDR by systematic factor.
    FactorCorrelated {
        /// Base deterministic default specification
        base_spec: DefaultModelSpec,
        /// Factor loading
        factor_loading: f64,
        /// CDR volatility
        cdr_volatility: f64,
    },
}

fn default_correlation() -> f64 {
    0.20
}

impl Default for StochasticDefaultSpec {
    fn default() -> Self {
        StochasticDefaultSpec::Deterministic(DefaultModelSpec::cdr_2pct())
    }
}

impl StochasticDefaultSpec {
    /// Create a deterministic (non-stochastic) default spec.
    pub fn deterministic(spec: DefaultModelSpec) -> Self {
        StochasticDefaultSpec::Deterministic(spec)
    }

    /// Create a copula-based default spec with Gaussian copula.
    pub fn gaussian_copula(base_cdr: f64, correlation: f64) -> Self {
        StochasticDefaultSpec::Copula {
            base_cdr: base_cdr.clamp(0.0, 1.0),
            copula_spec: CopulaSpec::Gaussian,
            correlation: correlation.clamp(0.0, 0.99),
        }
    }

    /// Create a copula-based default spec with Student-t copula.
    pub fn student_t_copula(base_cdr: f64, correlation: f64, degrees_of_freedom: f64) -> Self {
        StochasticDefaultSpec::Copula {
            base_cdr: base_cdr.clamp(0.0, 1.0),
            copula_spec: CopulaSpec::StudentT { degrees_of_freedom },
            correlation: correlation.clamp(0.0, 0.99),
        }
    }

    /// Create an intensity process default spec.
    pub fn intensity_process(
        base_hazard: f64,
        factor_loading: f64,
        mean_reversion: f64,
        volatility: f64,
    ) -> Self {
        StochasticDefaultSpec::IntensityProcess {
            base_hazard: base_hazard.clamp(0.0, 1.0),
            factor_loading: factor_loading.clamp(-1.0, 1.0),
            mean_reversion: mean_reversion.clamp(0.0, 10.0),
            volatility: volatility.clamp(0.0, 2.0),
            correlation: 0.20,
        }
    }

    /// Create a factor-correlated default spec.
    pub fn factor_correlated(
        base_spec: DefaultModelSpec,
        factor_loading: f64,
        cdr_volatility: f64,
    ) -> Self {
        StochasticDefaultSpec::FactorCorrelated {
            base_spec,
            factor_loading: factor_loading.clamp(-1.0, 1.0),
            cdr_volatility: cdr_volatility.clamp(0.0, 1.0),
        }
    }

    /// Build the stochastic default model from this specification.
    ///
    /// Returns `Ok(None)` for deterministic specs.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid copula specs (e.g. Student-t with
    /// `dof ≤ 2`) instead of silently falling back to a Gaussian copula
    /// against t-quantile thresholds.
    ///
    pub fn build(&self) -> finstack_quant_core::Result<Option<Box<dyn StochasticDefault>>> {
        Ok(match self {
            StochasticDefaultSpec::Deterministic(_) => None,

            StochasticDefaultSpec::Copula {
                base_cdr,
                copula_spec,
                correlation,
            } => Some(Box::new(CopulaBasedDefault::new(
                *base_cdr,
                copula_spec.clone(),
                *correlation,
            )?)),

            StochasticDefaultSpec::IntensityProcess {
                base_hazard,
                factor_loading,
                mean_reversion: _,
                volatility,
                correlation,
            } => Some(Box::new(
                IntensityProcessDefault::new(*base_hazard, *factor_loading, *volatility)
                    .with_correlation(*correlation),
            )),

            StochasticDefaultSpec::FactorCorrelated {
                base_spec,
                factor_loading,
                cdr_volatility,
            } => Some(Box::new(FactorCorrelatedDefault::new(
                base_spec.clone(),
                *factor_loading,
                *cdr_volatility,
            ))),
        })
    }

    /// Check if this is a stochastic specification.
    pub fn is_stochastic(&self) -> bool {
        !matches!(self, StochasticDefaultSpec::Deterministic(_))
    }

    /// Get the correlation if this is a stochastic model.
    ///
    /// For `FactorCorrelated` this returns the factor loading as a proxy —
    /// the model has no separate asset-correlation parameter (see
    /// `FactorCorrelatedDefault::correlation`).
    pub fn correlation(&self) -> Option<f64> {
        match self {
            StochasticDefaultSpec::Deterministic(_) => None,
            StochasticDefaultSpec::Copula { correlation, .. }
            | StochasticDefaultSpec::IntensityProcess { correlation, .. } => Some(*correlation),
            StochasticDefaultSpec::FactorCorrelated { factor_loading, .. } => Some(*factor_loading),
        }
    }

    /// Get the first-month annualized CDR, or the configured hazard rate.
    pub fn base_rate(&self) -> f64 {
        match self {
            StochasticDefaultSpec::Deterministic(spec)
            | StochasticDefaultSpec::FactorCorrelated {
                base_spec: spec, ..
            } => spec
                .mdr(1)
                .ok()
                .and_then(|mdr| finstack_quant_cashflows::builder::mdr_to_cdr(mdr).ok())
                .unwrap_or(0.0),
            StochasticDefaultSpec::Copula { base_cdr, .. } => *base_cdr,
            StochasticDefaultSpec::IntensityProcess { base_hazard, .. } => *base_hazard,
        }
    }

    /// Get the base MDR (monthly default rate) for this specification.
    ///
    /// Returns the unconditional expected MDR before factor shocks are applied.
    pub fn base_mdr(&self) -> f64 {
        clamped_cdr_to_mdr(self.base_rate())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spec_default() {
        let spec = StochasticDefaultSpec::default();
        assert!(!spec.is_stochastic());
    }

    #[test]
    fn test_gaussian_copula_spec() {
        let spec = StochasticDefaultSpec::gaussian_copula(0.02, 0.20);

        assert!(spec.is_stochastic());
        assert_eq!(spec.correlation(), Some(0.20));
        assert!((spec.base_rate() - 0.02).abs() < 1e-10);

        let model = spec.build().expect("valid Gaussian copula spec");
        assert!(model.is_some());
    }

    #[test]
    fn test_invalid_student_t_dof_fails_to_build() {
        let spec: StochasticDefaultSpec = serde_json::from_str(
            r#"{
                "model": "copula",
                "base_cdr": 0.02,
                "copula_spec": {"type": "student_t", "degrees_of_freedom": 2.0},
                "correlation": 0.20
            }"#,
        )
        .expect("spec deserializes; validation happens at build");
        assert!(
            spec.build().is_err(),
            "Student-t dof <= 2 must be a hard error, not a Gaussian fallback"
        );
    }

    #[test]
    fn test_intensity_process_spec() {
        let spec = StochasticDefaultSpec::intensity_process(0.02, 0.5, 0.5, 0.30);

        assert!(spec.is_stochastic());

        let model = spec
            .build()
            .expect("valid spec")
            .expect("Should build intensity process model");
        assert!(format!("{model:?}").starts_with("IntensityProcessDefault"));
    }

    /// Every stochastic default/prepay variant clamps its systematic-factor
    /// loading to the same documented range, [-1, 1].
    #[test]
    fn intensity_process_factor_loading_clamps_to_unit_range() {
        let spec = StochasticDefaultSpec::intensity_process(0.02, 1.5, 0.5, 0.30);
        let StochasticDefaultSpec::IntensityProcess { factor_loading, .. } = spec else {
            panic!("expected an intensity-process spec");
        };
        assert_eq!(factor_loading, 1.0);
    }

    #[test]
    fn intensity_process_factor_loading_round_trips() {
        let spec = StochasticDefaultSpec::intensity_process(0.02, 0.5, 0.5, 0.30);
        let json = serde_json::to_value(&spec).expect("serializes");
        assert_eq!(json["factor_loading"], 0.5);
        let back: StochasticDefaultSpec = serde_json::from_value(json).expect("round-trips");
        assert_eq!(back, spec);
    }

    #[test]
    // schema-rejection-test
    fn intensity_process_rejects_retired_factor_sensitivity_key() {
        let legacy = serde_json::json!({
            "model": "intensity_process",
            "base_hazard": 0.02,
            "factor_sensitivity": 0.5,
            "mean_reversion": 0.5,
            "volatility": 0.3,
        });
        assert!(serde_json::from_value::<StochasticDefaultSpec>(legacy).is_err());
    }

    #[test]
    fn test_factor_correlated_spec_builds_model() {
        let spec = StochasticDefaultSpec::factor_correlated(
            DefaultModelSpec::constant_cdr(0.02),
            0.4,
            0.2,
        );

        assert!(spec.is_stochastic());
        assert_eq!(spec.correlation(), Some(0.4));

        let model = spec
            .build()
            .expect("valid spec")
            .expect("factor-correlated model should build");
        assert!(format!("{model:?}").starts_with("FactorCorrelatedDefault"));
        // Canonical convention: low factor = stress, so a negative factor
        // realization must raise the conditional MDR above its expectation.
        assert!(model.conditional_mdr(12, &[-2.0]) > model.expected_mdr(12));
    }

    #[test]
    fn test_deterministic_build_returns_none() {
        let spec = StochasticDefaultSpec::deterministic(DefaultModelSpec::constant_cdr(0.02));

        assert!(!spec.is_stochastic());
        assert!(spec.build().expect("deterministic spec is valid").is_none());
    }
}
