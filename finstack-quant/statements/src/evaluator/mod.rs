//! Evaluator for financial models.
//!
//! The evaluator is responsible for:
//! - Building dependency graphs (DAG)
//! - Resolving evaluation order (topological sort)
//! - Applying precedence rules (Value > Forecast > Formula)
//! - Evaluating formulas for each period
//! - Handling where clause masking
//!
//! ## Where To Start
//!
//! - Use [`crate::evaluator::Evaluator::evaluate`] for standard model evaluation,
//!   including fixed-rate capital structure.
//! - Use [`crate::evaluator::Evaluator::evaluate_with_market`] when
//!   capital-structure instruments need market data or explicit values need an
//!   as-of cutoff.
//! - Use [`crate::evaluator::StatementResult::rollup`] to roll results up to a
//!   coarser frequency (quarterly to annual).
//! - Use [`crate::evaluator::StatementResult`] as the canonical output envelope for downstream
//!   analysis, reporting, and exports; call its `to_table_*` methods when you
//!   need a serializable tabular representation.
//!
//! ## Conventions
//!
//! - Node precedence is `Value > Forecast > Formula`.
//! - Result values are stored as scalar `f64` outputs, with optional
//!   `NodeValueType` metadata preserving monetary-vs-scalar interpretation.
//! - Capital-structure outputs in [`crate::evaluator::StatementResult`] follow reporting-currency
//!   semantics when FX conversion is available; otherwise multi-currency totals
//!   may remain unavailable.

mod capital_structure_runtime;
mod cashflow_export;
mod context;
mod dag;
mod engine;
pub(crate) mod export;
mod forecast_eval;
pub mod formula;
mod formula_aggregates;
mod formula_dispatch;
mod formula_ewm;
pub(crate) mod formula_helpers;
mod formula_timeseries;
pub(crate) mod monte_carlo;
mod period_history;
mod precedence;
mod results;
mod rollup;

pub use cashflow_export::{node_to_dated_schedule, PeriodDateConvention};
pub use context::EvaluationContext;
pub use dag::{evaluate_order, DependencyGraph};
pub use engine::{Evaluator, PreparedEvaluation};
pub use monte_carlo::{MonteCarloConfig, MonteCarloResults, PercentileSeries};
pub use period_history::PeriodHistory;
pub use results::{
    CapitalStructureClaimCategory, CapitalStructureWarning, CellSource, EvalStats, EvalWarning,
    NumericMode, StatementResult,
};
pub use rollup::{RollupMethod, RollupSpec};
