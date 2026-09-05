//! JSON-serializable helpers for portfolio optimization bindings.

use super::{
    Constraint, DefaultLpOptimizer, MissingMetricPolicy, Objective, PortfolioOptimizationProblem,
    PortfolioOptimizationResult, TradeUniverse, WeightingScheme,
};
use crate::error::Result;
use crate::portfolio::{Portfolio, PortfolioSpec};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::market_data::context::MarketContext;
use serde::{Deserialize, Serialize};

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
    /// Optional trade universe (tradeable/held filters and candidate
    /// additions). `None` means every existing position is tradeable and no
    /// candidates are considered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trade_universe: Option<TradeUniverse>,
}

fn default_weighting() -> WeightingScheme {
    WeightingScheme::ValueWeight
}

impl PortfolioOptimizationSpec {
    /// Create a spec with the Rust defaults (no constraints, value weighting,
    /// zero missing-metric policy, no label, full trade universe).
    ///
    /// # Arguments
    ///
    /// * `portfolio` - Serializable portfolio specification to optimize.
    /// * `objective` - Optimization objective.
    #[must_use]
    pub fn new(portfolio: PortfolioSpec, objective: Objective) -> Self {
        Self {
            portfolio,
            objective,
            constraints: Vec::new(),
            weighting: WeightingScheme::ValueWeight,
            missing_metric_policy: MissingMetricPolicy::Zero,
            label: None,
            trade_universe: None,
        }
    }

    /// Restrict the optimizer to a trade universe.
    ///
    /// # Arguments
    ///
    /// * `universe` - Tradeable/held filters plus candidate additions.
    #[must_use]
    pub fn with_trade_universe(mut self, universe: TradeUniverse) -> Self {
        self.trade_universe = Some(universe);
        self
    }
}

/// Run portfolio optimization from a JSON-friendly spec.
///
/// Builds the `Portfolio` from the embedded `PortfolioSpec`, constructs the
/// optimization problem, and returns the native
/// [`PortfolioOptimizationResult`] — which serializes to the canonical JSON
/// wire format via its `Serialize` impl.
///
/// # Arguments
///
/// * `spec` - Complete optimization specification containing portfolio,
///   objective, constraints, weighting, and missing-metric policy.
/// * `market` - Market context used to resolve any market-dependent metrics
///   required by the objective or constraints.
/// * `config` - Finstack configuration passed through to optimization and
///   valuation helpers.
///
/// # Errors
///
/// Propagates invalid portfolio-spec, objective, constraint, weighting, and
/// missing-metric-policy inputs, along with optimization and market-dependent
/// valuation failures.
pub fn optimize_from_spec(
    spec: &PortfolioOptimizationSpec,
    market: &MarketContext,
    config: &FinstackConfig,
) -> Result<PortfolioOptimizationResult> {
    let portfolio = Portfolio::from_spec(spec.portfolio.clone())?;
    let mut problem = PortfolioOptimizationProblem::new(portfolio, spec.objective.clone());
    problem.weighting = spec.weighting;
    problem.missing_metric_policy = spec.missing_metric_policy;
    problem.label = spec.label.clone();
    problem.constraints.extend(spec.constraints.iter().cloned());
    if let Some(universe) = &spec.trade_universe {
        problem = problem.with_trade_universe(universe.clone());
    }

    let optimizer = DefaultLpOptimizer;
    optimizer.optimize(&problem, market, config)
}
