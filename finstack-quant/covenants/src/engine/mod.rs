//! Point-in-time covenant evaluation, breach tracking, and consequence application.
//!
//! # Scope and conventions
//!
//! - **Test dates are caller-controlled.** [`Covenant::test_frequency`] is
//!   descriptive metadata only: the engine evaluates whenever the caller
//!   invokes [`CovenantEngine::evaluate`] with a `test_date` and does not
//!   itself generate or enforce a testing schedule.
//! - **Equity cures are not modeled.** Numerical compliance with the effective
//!   threshold before the cure deadline can cure an episode. A full waiver or
//!   an inactive test does not itself cure history; there is no mechanism for
//!   injecting sponsor equity into the tested metric.
//! - **Metric values are taken as-is (LTM contract).** The engine performs no
//!   trailing-twelve-month or other window aggregation. If a covenant is
//!   defined on an LTM basis (as most leverage/coverage covenants are), the
//!   supplied metric node must already encode it — e.g. a statements node
//!   defined via `ttm(ebitda)` — before being exposed through
//!   [`crate::CovenantMetricSource`].

pub(crate) mod covenant_engine;
pub(crate) mod evaluation;
pub(crate) mod types;

pub(crate) use covenant_engine::CovenantEngine;
pub(crate) use evaluation::{
    headroom_for, headroom_raw_for, is_covenant_breached, spec_metric_name,
    springing_condition_met, validate_metric, validated_ratio_value,
};
pub(crate) use types::{
    BoundKind, Covenant, CovenantConsequence, CovenantScope, CovenantSpec, CovenantType,
    SpringingCondition, ThresholdTest,
};
#[cfg(test)]
pub(super) use types::{CovenantWaiver, CovenantWindow};

#[cfg(test)]
mod tests;
