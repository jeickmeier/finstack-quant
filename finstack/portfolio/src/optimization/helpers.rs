//! Helper functions for portfolio optimization use cases.
//!
//! These helpers live in the core crate so that bindings (Python, WASM)
//! only need to perform type conversions and can pass through directly
//! to Rust logic.

use super::{
    Constraint, DefaultLpOptimizer, MissingMetricPolicy, Objective, PortfolioOptimizationProblem,
    PortfolioOptimizationResult, WeightingScheme,
};
use crate::error::Result;
use crate::portfolio::{Portfolio, PortfolioSpec};
use finstack_core::config::FinstackConfig;
use finstack_core::market_data::context::MarketContext;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// General-purpose optimization spec (JSON-friendly)
// ---------------------------------------------------------------------------

/// JSON-serializable specification for a portfolio optimization problem.
///
/// This type bridges the gap between the JSON-first binding pattern and the
/// internal [`PortfolioOptimizationProblem`] which holds a live `Portfolio`.
/// Bindings deserialize this spec, build the `Portfolio` from the embedded
/// [`PortfolioSpec`], and then run the optimizer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortfolioOptimizationSpec {
    /// Portfolio specification (same format as `value_portfolio`).
    pub portfolio: PortfolioSpec,
    /// Optimization objective.
    pub objective: Objective,
    /// Constraints on the optimized portfolio.
    #[serde(default)]
    pub constraints: Vec<Constraint>,
    /// How weights are defined.
    #[serde(default = "default_weighting")]
    pub weighting: WeightingScheme,
    /// Policy for handling positions missing required metrics.
    #[serde(default)]
    pub missing_metric_policy: MissingMetricPolicy,
    /// Optional label for auditability.
    #[serde(default)]
    pub label: Option<String>,
}

fn default_weighting() -> WeightingScheme {
    WeightingScheme::ValueWeight
}

/// Run portfolio optimization from a JSON-friendly spec.
///
/// Builds the `Portfolio` from the embedded `PortfolioSpec`, constructs the
/// optimization problem, and returns the native
/// [`PortfolioOptimizationResult`] — which serializes to the canonical JSON
/// wire format via its `Serialize` impl.
pub fn optimize_from_spec(
    spec: &PortfolioOptimizationSpec,
    market: &MarketContext,
    config: &FinstackConfig,
) -> Result<PortfolioOptimizationResult> {
    let portfolio = Portfolio::from_spec(spec.portfolio.clone())?;
    optimize_from_parts(
        &portfolio,
        &spec.objective,
        &spec.constraints,
        spec.weighting,
        spec.missing_metric_policy,
        spec.label.as_deref(),
        market,
        config,
    )
}

/// Run portfolio optimization against an already-built `Portfolio`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn optimize_from_parts(
    portfolio: &Portfolio,
    objective: &Objective,
    constraints: &[Constraint],
    weighting: WeightingScheme,
    missing_metric_policy: MissingMetricPolicy,
    label: Option<&str>,
    market: &MarketContext,
    config: &FinstackConfig,
) -> Result<PortfolioOptimizationResult> {
    let mut problem = PortfolioOptimizationProblem::new(portfolio.clone(), objective.clone());
    problem.weighting = weighting;
    problem.missing_metric_policy = missing_metric_policy;
    problem.label = label.map(str::to_string);
    problem = problem.with_constraints(constraints.to_vec());

    let optimizer = DefaultLpOptimizer;
    optimizer.optimize(&problem, market, config)
}
