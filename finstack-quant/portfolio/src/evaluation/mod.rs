//! Request-scoped portfolio evaluation planning and execution.
//!
//! This module is the single production path for portfolio position pricing,
//! state-local pricing-option ownership, Rayon scheduling, and deterministic
//! valuation assembly. Public workflow functions remain in their existing
//! modules and compile their work into this internal executor.

mod executor;
mod plan;
mod state;

pub(crate) use executor::{
    evaluate_raw_portfolio, PositionExecution, RawEvaluationInput, RawSelectiveSeed,
};
pub(crate) use plan::{
    BaseCurrencyPolicy, EvaluationMetricProfile, EvaluationProfile, EvaluationProvenance,
    ParentResult, PortfolioEvaluationPlan, PositionInvalidation, RiskFailurePolicy,
};
