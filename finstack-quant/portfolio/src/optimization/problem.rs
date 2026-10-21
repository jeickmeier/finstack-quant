//! Problem, decision-space, constraint, and solver types for portfolio optimization.
//!
use super::constraints::Constraint;
use super::types::{MissingMetricPolicy, Objective, WeightingScheme};
use super::universe::TradeUniverse;
use crate::Portfolio;

/// Complete optimization problem specification.
#[derive(Clone, Debug)]
pub struct PortfolioOptimizationProblem {
    /// The existing portfolio to optimize.
    pub portfolio: Portfolio,

    /// How weights are defined (value‑weighted, notional, etc.).
    pub weighting: WeightingScheme,

    /// The trade universe: which positions can be traded and candidate instruments.
    /// Default: all existing portfolio positions are tradeable, no candidates.
    pub trade_universe: TradeUniverse,

    /// Optimization objective (maximize/minimize a metric expression).
    pub objective: Objective,

    /// Constraints on the optimized portfolio.
    pub constraints: Vec<Constraint>,

    /// Policy for handling positions missing required metrics.
    pub missing_metric_policy: MissingMetricPolicy,

    /// Optional label for auditability.
    pub label: Option<String>,
}

impl PortfolioOptimizationProblem {
    /// Create a basic problem optimizing all positions in the portfolio.
    ///
    /// This helper:
    /// - Uses `WeightingScheme::ValueWeight`
    /// - Uses a default `TradeUniverse` (all positions tradeable, no candidates)
    /// - Starts with no constraints. [`super::optimize`] adds the default
    ///   `Σw = 1` budget when the problem declares none, except under
    ///   `WeightingScheme::UnitScaling`, which requires an explicit
    ///   [`Constraint::Budget`] (or other bounding constraints).
    ///
    /// # Arguments
    ///
    /// * `portfolio` - Starting portfolio whose positions and metadata seed the problem.
    /// * `objective` - Portfolio-level target to maximize or minimize.
    ///
    /// # Returns
    ///
    /// A value-weighted problem over the full trade universe with no explicit
    /// constraints.
    #[must_use]
    pub fn new(portfolio: Portfolio, objective: Objective) -> Self {
        Self {
            portfolio,
            weighting: WeightingScheme::ValueWeight,
            trade_universe: TradeUniverse::default(),
            objective,
            constraints: Vec::new(),
            missing_metric_policy: MissingMetricPolicy::Zero,
            label: None,
        }
    }

    /// Set the trade universe.
    ///
    /// # Arguments
    ///
    /// * `universe` - Universe describing which existing positions may trade and
    ///   which new candidates may be added.
    ///
    /// # Returns
    ///
    /// The updated problem for fluent chaining.
    #[must_use]
    pub fn with_trade_universe(mut self, universe: TradeUniverse) -> Self {
        self.trade_universe = universe;
        self
    }

    /// Add a single constraint.
    ///
    /// # Arguments
    ///
    /// * `constraint` - Additional portfolio constraint.
    ///
    /// # Returns
    ///
    /// The updated problem for fluent chaining.
    #[must_use]
    pub fn with_constraint(mut self, constraint: Constraint) -> Self {
        self.constraints.push(constraint);
        self
    }
}
