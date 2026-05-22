//! Scenario, sensitivity, and variance analysis.
//!
//! - [`scenario_set`] — named scenario registry with parent chaining and diff
//! - [`sensitivity`] — parameter sweeps, tornado charts, and grid analysis
//! - [`types`] — shared types for sensitivity analysis
//! - [`variance`] — baseline vs comparison variance and bridge decomposition
//! - [`monte_carlo`] — re-exports of Monte Carlo types from the evaluator
//!
//! This module is intentionally statement-local. [`ScenarioSet`] evaluates named
//! scalar overrides against a [`finstack_statements::FinancialModelSpec`] and
//! compares the resulting statement outputs. Cross-domain market, instrument,
//! rate-binding, and time-roll shocks belong to `finstack-scenarios`
//! (`ScenarioSpec` + `ExecutionContext`), not this module.

pub(crate) mod monte_carlo;
pub(crate) mod scenario_set;
pub(crate) mod sensitivity;
pub(crate) mod types;
pub(crate) mod variance;

pub use monte_carlo::{MonteCarloConfig, MonteCarloResults, PercentileSeries};
pub use scenario_set::{ScenarioDefinition, ScenarioDiff, ScenarioResults, ScenarioSet};
pub use sensitivity::{generate_tornado_entries, SensitivityAnalyzer};
pub use types::{
    ParameterSpec, SensitivityConfig, SensitivityMode, SensitivityResult, TornadoEntry,
};
pub use variance::{
    BridgeChart, BridgeStep, VarianceAnalyzer, VarianceConfig, VarianceReport, VarianceRow,
};
