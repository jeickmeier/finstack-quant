//! Stochastic default specification.
//!
//! Provides a serializable specification enum for stochastic default models,
//! enabling configuration and deferred construction.

use super::super::clamped_cdr_to_mdr;
use super::{
    CopulaBasedDefault, FactorCorrelatedDefault, HazardCurveDefault, IntensityProcessDefault,
    StochasticDefault,
};
use crate::correlation::copula::CopulaSpec;
use finstack_quant_cashflows::builder::specs::DefaultModelSpec;
use finstack_quant_core::market_data::term_structures::HazardCurve;

/// Stochastic default model specification.
///
/// Allows default model selection and configuration without
/// constructing the full model.
#[derive(Debug, Clone, serde::Deserialize)]
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

    /// Hazard curve-based default model.
    ///
    /// Uses a market-calibrated hazard curve (e.g., from CDS spreads)
    /// with factor-based stochastic shocks.
    ///
    /// Note: This variant cannot be serialized/deserialized directly as it
    /// contains a HazardCurve. Use `build_from_hazard_curve` for construction.
    #[serde(skip)]
    HazardCurveBased {
        /// The calibrated hazard curve
        hazard_curve: Box<HazardCurve>,
        /// Factor loading (β) for systematic risk shocks
        factor_loading: f64,
        /// Volatility of intensity shocks (σ)
        volatility: f64,
        /// Asset correlation for default distribution
        correlation: f64,
    },
}

impl serde::Serialize for StochasticDefaultSpec {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(serde::Serialize)]
        #[serde(tag = "model", rename_all = "snake_case")]
        enum PersistedStochasticDefaultSpec<'a> {
            Deterministic(&'a DefaultModelSpec),
            Copula {
                base_cdr: f64,
                copula_spec: &'a CopulaSpec,
                correlation: f64,
            },
            IntensityProcess {
                base_hazard: f64,
                factor_loading: f64,
                mean_reversion: f64,
                volatility: f64,
                correlation: f64,
            },
            FactorCorrelated {
                base_spec: &'a DefaultModelSpec,
                factor_loading: f64,
                cdr_volatility: f64,
            },
        }

        let persisted = match self {
            Self::Deterministic(spec) => PersistedStochasticDefaultSpec::Deterministic(spec),
            Self::Copula {
                base_cdr,
                copula_spec,
                correlation,
            } => PersistedStochasticDefaultSpec::Copula {
                base_cdr: *base_cdr,
                copula_spec,
                correlation: *correlation,
            },
            Self::IntensityProcess {
                base_hazard,
                factor_loading,
                mean_reversion,
                volatility,
                correlation,
            } => PersistedStochasticDefaultSpec::IntensityProcess {
                base_hazard: *base_hazard,
                factor_loading: *factor_loading,
                mean_reversion: *mean_reversion,
                volatility: *volatility,
                correlation: *correlation,
            },
            Self::FactorCorrelated {
                base_spec,
                factor_loading,
                cdr_volatility,
            } => PersistedStochasticDefaultSpec::FactorCorrelated {
                base_spec,
                factor_loading: *factor_loading,
                cdr_volatility: *cdr_volatility,
            },
            Self::HazardCurveBased { .. } => {
                return Err(serde::ser::Error::custom(
                    "StochasticDefaultSpec::HazardCurveBased is a derived calibrated-curve \
                     artifact and cannot be persisted; reconstruct it with \
                     build_from_hazard_curve and persist calibration prior_market instead",
                ));
            }
        };
        serde::Serialize::serialize(&persisted, serializer)
    }
}

fn default_correlation() -> f64 {
    0.20
}

impl PartialEq for StochasticDefaultSpec {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Deterministic(a), Self::Deterministic(b)) => a == b,
            (
                Self::Copula {
                    base_cdr: a1,
                    copula_spec: a2,
                    correlation: a3,
                },
                Self::Copula {
                    base_cdr: b1,
                    copula_spec: b2,
                    correlation: b3,
                },
            ) => a1 == b1 && a2 == b2 && a3 == b3,
            (
                Self::IntensityProcess {
                    base_hazard: a1,
                    factor_loading: a2,
                    mean_reversion: a3,
                    volatility: a4,
                    correlation: a5,
                },
                Self::IntensityProcess {
                    base_hazard: b1,
                    factor_loading: b2,
                    mean_reversion: b3,
                    volatility: b4,
                    correlation: b5,
                },
            ) => a1 == b1 && a2 == b2 && a3 == b3 && a4 == b4 && a5 == b5,
            (
                Self::FactorCorrelated {
                    base_spec: a1,
                    factor_loading: a2,
                    cdr_volatility: a3,
                },
                Self::FactorCorrelated {
                    base_spec: b1,
                    factor_loading: b2,
                    cdr_volatility: b3,
                },
            ) => a1 == b1 && a2 == b2 && a3 == b3,
            (
                Self::HazardCurveBased {
                    hazard_curve: a1,
                    factor_loading: a2,
                    volatility: a3,
                    correlation: a4,
                },
                Self::HazardCurveBased {
                    hazard_curve: b1,
                    factor_loading: b2,
                    volatility: b3,
                    correlation: b4,
                },
            ) => {
                // Compare by curve ID since HazardCurve doesn't impl PartialEq
                a1.id() == b1.id() && a2 == b2 && a3 == b3 && a4 == b4
            }
            _ => false,
        }
    }
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

    /// Create a hazard curve-based default spec.
    ///
    /// Uses a market-calibrated hazard curve with factor shocks.
    ///
    /// # Arguments
    ///
    /// * `hazard_curve` - Calibrated hazard curve (e.g., from CDS spreads)
    /// * `factor_loading` - Loading (β) on the systematic factor, clamped to [-1, 1] (typical: 0.3-0.8)
    pub fn from_hazard_curve(hazard_curve: HazardCurve, factor_loading: f64) -> Self {
        StochasticDefaultSpec::HazardCurveBased {
            hazard_curve: Box::new(hazard_curve),
            factor_loading: factor_loading.clamp(-1.0, 1.0),
            volatility: 0.30,
            correlation: 0.20,
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
    /// Engines pricing a SEASONED pool should use
    /// [`Self::build_with_seasoning_offset`] so hazard-curve-based models
    /// index the curve by time-from-valuation instead of loan age.
    pub fn build(&self) -> finstack_quant_core::Result<Option<Box<dyn StochasticDefault>>> {
        self.build_with_seasoning_offset(0)
    }

    /// Build the model with the pool's seasoning at valuation (months).
    ///
    /// Only `HazardCurveBased` consumes the offset: its curve is anchored at
    /// the valuation date, so lookups must subtract the initial seasoning
    /// from the loan-age `seasoning` the engines pass in. The other models
    /// are seasoning-curve based (loan age is the correct index) and ignore
    /// the offset.
    ///
    /// # Arguments
    ///
    /// * `seasoning_offset_months` - Seasoning offset months used by the algorithm, subject to the enclosing type invariants and documented units.
    pub fn build_with_seasoning_offset(
        &self,
        seasoning_offset_months: u32,
    ) -> finstack_quant_core::Result<Option<Box<dyn StochasticDefault>>> {
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

            StochasticDefaultSpec::HazardCurveBased {
                hazard_curve,
                factor_loading,
                volatility,
                correlation,
            } => Some(Box::new(
                HazardCurveDefault::new((**hazard_curve).clone(), *factor_loading)
                    .with_volatility(*volatility)
                    .with_correlation(*correlation)
                    .with_seasoning_offset(seasoning_offset_months),
            )),
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
            | StochasticDefaultSpec::IntensityProcess { correlation, .. }
            | StochasticDefaultSpec::HazardCurveBased { correlation, .. } => Some(*correlation),
            StochasticDefaultSpec::FactorCorrelated { factor_loading, .. } => Some(*factor_loading),
        }
    }

    /// Get the base CDR/hazard rate.
    pub fn base_rate(&self) -> f64 {
        match self {
            StochasticDefaultSpec::Deterministic(spec) => spec.cdr,
            StochasticDefaultSpec::Copula { base_cdr, .. } => *base_cdr,
            StochasticDefaultSpec::IntensityProcess { base_hazard, .. } => *base_hazard,
            StochasticDefaultSpec::FactorCorrelated { base_spec, .. } => base_spec.cdr,
            StochasticDefaultSpec::HazardCurveBased { hazard_curve, .. } => {
                // Approximate 1-year default probability as the base rate
                1.0 - hazard_curve.sp(1.0)
            }
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
    use time::macros::date;

    #[test]
    fn test_spec_default() {
        let spec = StochasticDefaultSpec::default();
        assert!(!spec.is_stochastic());
    }

    #[test]
    fn hazard_curve_variant_has_typed_persistence_error() {
        let curve = HazardCurve::builder("ACME-HAZARD")
            .base_date(date!(2026 - 01 - 01))
            .knots([(1.0, 0.01), (5.0, 0.02)])
            .recovery_rate(0.40)
            .build()
            .expect("valid hazard curve");
        let spec = StochasticDefaultSpec::from_hazard_curve(curve, 0.5);

        let error =
            serde_json::to_string(&spec).expect_err("derived hazard curves are not persisted");
        let message = error.to_string();
        assert!(message.contains("build_from_hazard_curve"), "{message}");
        assert!(message.contains("prior_market"), "{message}");
        assert!(message.contains("derived"), "{message}");
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
        assert_eq!(model.model_name(), "Intensity Process Default Model");
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
        assert_eq!(model.model_name(), "Factor-Correlated Default");
        // Canonical convention: low factor = stress, so a negative factor
        // realization must raise the conditional MDR above its expectation.
        assert!(model.conditional_mdr(12, &[-2.0], &Default::default()) > model.expected_mdr(12));
    }

    #[test]
    fn test_deterministic_build_returns_none() {
        let spec = StochasticDefaultSpec::deterministic(DefaultModelSpec::constant_cdr(0.02));

        assert!(!spec.is_stochastic());
        assert!(spec.build().expect("deterministic spec is valid").is_none());
    }
}
