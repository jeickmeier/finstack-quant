//! Model-specific valuation details attached to [`ValuationResult`](super::ValuationResult).

use crate::pricer::ModelKey;

use super::composite::CompositeValuationDetails;
use super::structured_credit::StochasticPricingResult;

/// Model-specific typed valuation details.
///
/// These details are for rich structured outputs that do not fit the scalar
/// `measures` map while still belonging in the standard valuation envelope.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ValuationDetails {
    /// Recursive leg valuation and primitive exposure detail for a composite.
    Composite(CompositeValuationDetails),
    /// Credit-derivative pricing metadata.
    CreditDerivative(CreditDerivativeValuationDetails),
    /// Scenario-waterfall structured credit stochastic pricing result.
    StructuredCreditStochastic(StochasticPricingResult),
    /// FX-bearing instrument pricing metadata (forwards, options, quanto, …).
    ///
    /// Surfaces the [`FxMatrix`](finstack_quant_core::money::fx::FxMatrix) lookup
    /// chain that backed the spot rate used at pricing time, satisfying the
    /// project-wide FX-policy-visibility invariant for instruments that
    /// resolve their spot through the matrix.
    Fx(FxValuationDetails),
    /// Monte Carlo estimator and reproducibility metadata.
    MonteCarlo(MonteCarloValuationDetails),
}

/// Metadata for CDS-family valuation paths.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreditDerivativeValuationDetails {
    /// Registered model key used by the pricer.
    pub model_key: ModelKey,
    /// CDS integration method actually used, when applicable.
    pub integration_method: Option<String>,
}

/// Metadata for FX-bearing valuation paths (FX forwards, FX options, quanto).
///
/// Captures the FX policy applied at pricing time so downstream audit /
/// reconciliation can trace whether a quoted rate came from a direct
/// market quote or via triangulation through the matrix pivot currency.
/// Mirrors the "FX policy visibility" invariant in `ARCHITECTURE.md`: the
/// applied conversion strategy is recorded per layer (valuations, statements,
/// portfolio).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FxValuationDetails {
    /// `true` when the FX spot was obtained via triangulation through the
    /// [`FxMatrix`](finstack_quant_core::money::fx::FxMatrix) pivot currency
    /// rather than a direct quote. Mirrors
    /// [`FxRateResult.triangulated`](finstack_quant_core::money::fx::FxRateResult).
    /// `None` when the instrument resolved spot from an explicit market
    /// scalar (`fx_spot_id`) rather than the matrix.
    pub fx_triangulated: Option<bool>,
}

/// Reproducibility and convergence diagnostics for a Monte Carlo valuation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MonteCarloValuationDetails {
    /// Registered model used for the simulation.
    pub model_key: ModelKey,
    /// Sampling standard error of the discounted PV mean in the result
    /// currency. For LSMC valuations, this measures pricing-path uncertainty
    /// under the frozen fitted exercise policy. It excludes regression
    /// approximation, time-grid discretization, and model error.
    pub standard_error: f64,
    /// Number of independent paths used to fit an exercise or control policy.
    ///
    /// Zero for Monte Carlo engines that do not have a separate training
    /// stage.
    #[serde(serialize_with = "finstack_quant_core::wire::count::serialize")]
    #[cfg_attr(feature = "json-schema", schemars(with = "u32"))]
    pub training_paths: usize,
    /// Total factor paths simulated in the policy-training stage, including
    /// antithetic partners. Zero when no policy is trained.
    #[serde(serialize_with = "finstack_quant_core::wire::count::serialize")]
    #[cfg_attr(feature = "json-schema", schemars(with = "u32"))]
    pub training_simulated_paths: usize,
    /// Independent paths used to fit state-conditional make-whole reference
    /// values. Zero when no stochastic make-whole stage is required.
    #[serde(serialize_with = "finstack_quant_core::wire::count::serialize")]
    #[cfg_attr(feature = "json-schema", schemars(with = "u32"))]
    pub make_whole_training_paths: usize,
    /// Total factor paths simulated for state-conditional make-whole training,
    /// including antithetic partners. Zero when that stage is absent.
    #[serde(serialize_with = "finstack_quant_core::wire::count::serialize")]
    #[cfg_attr(feature = "json-schema", schemars(with = "u32"))]
    pub make_whole_training_simulated_paths: usize,
    /// Number of independent path estimators contributing to the mean.
    #[serde(serialize_with = "finstack_quant_core::wire::count::serialize")]
    #[cfg_attr(feature = "json-schema", schemars(with = "u32"))]
    pub estimator_paths: usize,
    /// Total number of simulated paths, including antithetic partners.
    #[serde(serialize_with = "finstack_quant_core::wire::count::serialize")]
    #[cfg_attr(feature = "json-schema", schemars(with = "u32"))]
    pub simulated_paths: usize,
    /// Deterministic random seed used for the run.
    pub seed: u64,
    /// Simulation times in year fractions, including zero and maturity.
    pub time_grid: Vec<f64>,
    /// Whether antithetic variates were enabled.
    pub antithetic: bool,
    /// Whether Sobol quasi-random sampling was enabled.
    pub sobol: bool,
    /// Whether Brownian-bridge ordering was enabled for Sobol paths.
    pub brownian_bridge: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // schema-rejection-test
    fn valuation_details_types_reject_unknown_fields() {
        let credit = CreditDerivativeValuationDetails {
            model_key: ModelKey::HazardRate,
            integration_method: Some("isda".to_string()),
        };
        let mut credit_json = serde_json::to_value(&credit).expect("serialize");
        credit_json["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_value::<CreditDerivativeValuationDetails>(credit_json).is_err());

        let mut fx_json = serde_json::to_value(FxValuationDetails {
            fx_triangulated: Some(false),
        })
        .expect("serialize");
        fx_json["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<FxValuationDetails>(fx_json).is_err());

        let mc = MonteCarloValuationDetails {
            model_key: ModelKey::MonteCarloGBM,
            standard_error: 0.01,
            training_paths: 0,
            training_simulated_paths: 0,
            make_whole_training_paths: 0,
            make_whole_training_simulated_paths: 0,
            estimator_paths: 100,
            simulated_paths: 200,
            seed: 42,
            time_grid: vec![0.0, 1.0],
            antithetic: true,
            sobol: false,
            brownian_bridge: false,
        };
        let mut mc_json = serde_json::to_value(&mc).expect("serialize");
        mc_json["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_value::<MonteCarloValuationDetails>(mc_json).is_err());

        let mut envelope_json = serde_json::to_value(ValuationDetails::Fx(FxValuationDetails {
            fx_triangulated: Some(true),
        }))
        .expect("serialize");
        envelope_json["extra"] = serde_json::json!("bad");
        assert!(serde_json::from_value::<ValuationDetails>(envelope_json).is_err());
    }
}
