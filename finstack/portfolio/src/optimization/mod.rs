#![allow(clippy::module_name_repetitions)]

//! Portfolio optimization on top of valuations.
//!
//! This module provides a deterministic, metric‑driven portfolio optimization
//! facility that operates entirely on top of existing valuation results.
//!
//! The main entry points are:
//!
//! - [`crate::optimization::PortfolioOptimizationProblem`] for declaring the
//!   optimization objective, weighting scheme, trade universe, and constraints
//! - [`crate::optimization::DefaultLpOptimizer`] for solving the resulting
//!   linear program
//! - [`crate::optimization::PortfolioOptimizationResult`] for inspecting
//!   optimal weights, implied quantities, and trade lists
//!
//! # Conventions
//!
//! - Optimization weights are abstract and must be interpreted via
//!   [`crate::optimization::WeightingScheme`]. In particular, `ValueWeight` means share of
//!   base-currency portfolio value, while `UnitScaling` means a multiplier on
//!   the current quantity for existing positions.
//! - Portfolio-level constraints assume metrics are comparable across positions.
//!   Mixed-currency portfolios therefore use base-currency quantities where the
//!   linearization would otherwise be ambiguous.
//! - The current optimizer is linear-program based. Covariance-driven or other
//!   quadratic risk constraints are intentionally out of scope for
//!   [`crate::optimization::DefaultLpOptimizer`].
//!
//! # Workflow
//!
//! 1. Value the current portfolio and confirm the required metrics are
//!    available.
//! 2. Define a [`crate::optimization::PortfolioOptimizationProblem`] with an
//!    [`crate::optimization::Objective`] and any
//!    [`crate::optimization::Constraint`] values.
//! 3. Solve with [`crate::optimization::DefaultLpOptimizer`].
//! 4. Inspect [`crate::optimization::PortfolioOptimizationResult::to_trade_list`]
//!    or rebuild a portfolio with
//!    [`crate::optimization::PortfolioOptimizationResult::to_rebalanced_portfolio`].
//!
//! # References
//!
//! - Active portfolio construction background:
//!   `docs/REFERENCES.md#grinoldKahn1999ActivePortfolio`
//! - Fixed-income risk and key-rate style metrics:
//!   `docs/REFERENCES.md#tuckman-serrat-fixed-income`

mod constraints;
mod decision;
mod helpers;
mod lp_solver;
mod problem;
mod result;
mod types;
mod universe;

pub(crate) mod tolerances {
    pub use super::types::{
        GROSS_BASE_TOL, MIN_WEIGHT_TOL, PV_PER_UNIT_TOL, SLACK_TOL, WEIGHT_TOL,
    };
}

pub use constraints::{Constraint, ConstraintValidationError, Inequality};
pub use helpers::{
    optimize_from_spec, optimize_with_parameters, OptimizationParameters, PortfolioOptimizationSpec,
};
pub use lp_solver::DefaultLpOptimizer;
pub use problem::PortfolioOptimizationProblem;
pub use result::{
    OptimizationStatus, PortfolioOptimizationResult, TradeDirection, TradeSpec, TradeType,
};
pub use types::{MetricExpr, MissingMetricPolicy, Objective, PerPositionMetric, WeightingScheme};
pub use universe::{CandidatePosition, PositionFilter, TradeUniverse};
