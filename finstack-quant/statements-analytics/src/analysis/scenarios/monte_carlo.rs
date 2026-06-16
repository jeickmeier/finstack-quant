//! Monte Carlo analysis facade for statement forecasts.
//!
//! The evaluator owns the Monte Carlo runtime and result types so the core
//! execution layer does not depend on the analysis namespace. This module keeps
//! the historical analysis-facing import path stable by re-exporting those
//! evaluator types.

pub use finstack_quant_statements::evaluator::{
    MonteCarloConfig, MonteCarloResults, PercentileSeries,
};
