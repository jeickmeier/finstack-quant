//! Scenario, sensitivity, and variance analysis.
//!
//! - [`scenario_set`] — named scenario registry with parent chaining and diff
//! - [`sensitivity`] — parameter sweeps, tornado charts, and grid analysis
//! - [`types`] — shared types for sensitivity analysis
//! - [`variance`] — baseline vs comparison variance and bridge decomposition
//! - [`monte_carlo`] — re-exports of Monte Carlo types from the evaluator

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
