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
//! - Use [`crate::evaluator::Evaluator::evaluate`] for standard model evaluation.
//! - Use [`crate::evaluator::Evaluator::evaluate_with_market`] when the model
//!   references capital structure (`cs.*`) and you need instrument pricing.
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
mod precedence;
mod results;

pub use cashflow_export::{node_to_dated_schedule, PeriodDateConvention};
pub use context::EvaluationContext;
pub use dag::{evaluate_order, DependencyGraph};
pub use engine::{Evaluator, PreparedEvaluation};
pub use monte_carlo::{MonteCarloConfig, MonteCarloResults, PercentileSeries};
pub use results::{EvalWarning, NumericMode, ResultsMeta, StatementResult};
