//! Scenario, sensitivity, and variance analysis.
//!
//! - [`scenario_set`] — named scenario registry with parent chaining and diff
//! - [`sensitivity`] — parameter sweeps, tornado charts, and grid analysis
//! - [`types`] — shared types for sensitivity analysis
//! - [`variance`] — baseline vs comparison variance and bridge decomposition
//!
//! This module is intentionally statement-local. [`ScenarioSet`] evaluates named
//! scalar overrides against a [`finstack_quant_statements::types::FinancialModelSpec`] and
//! compares the resulting statement outputs. Cross-domain market, instrument,
//! rate-binding, and time-roll shocks belong to `finstack-quant-scenarios`
//! (`ScenarioSpec` + `ExecutionContext`), not this module.

pub(crate) mod scenario_set;
pub(crate) mod sensitivity;
pub(crate) mod types;
pub(crate) mod variance;

pub(crate) use types::TornadoEntry;
