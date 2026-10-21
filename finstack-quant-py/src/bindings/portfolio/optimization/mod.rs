//! Typed `#[pyclass]` wrappers for `finstack_quant_portfolio::optimization` spec
//! and result types.
//!
//! Callers build a specification programmatically using the same builder
//! pattern as the Rust API and inspect optimization results through typed
//! getters.
//!
//! Notes on what is bound and what is not:
//!
//! - All declarative variants of [`finstack_quant_portfolio::optimization::Constraint`],
//!   [`finstack_quant_portfolio::optimization::Objective`],
//!   [`finstack_quant_portfolio::optimization::MetricExpr`],
//!   [`finstack_quant_portfolio::optimization::PerPositionMetric`],
//!   [`finstack_quant_portfolio::optimization::PositionFilter`], and the unit-style enums
//!   (`Inequality`, `WeightingScheme`, `MissingMetricPolicy`,
//!   `OptimizationStatus`, `TradeDirection`, `TradeType`) are exposed.
//! - [`finstack_quant_portfolio::optimization::CandidatePosition`] is built from a
//!   typed instrument (or its JSON) and [`finstack_quant_portfolio::optimization::TradeUniverse`]
//!   from its classmethods.
//! - Optimization results round-trip through ``to_json`` / ``from_json``.

mod enums;
mod expressions;
mod optimize;
mod spec_result;
mod status_trade;

use pyo3::prelude::*;

use enums::{
    PyInequality, PyMissingMetricPolicy, PyTradeDirection, PyTradeType, PyWeightingScheme,
};
use expressions::{PyConstraint, PyMetricExpr, PyObjective, PyPerPositionMetric, PyPositionFilter};
use optimize::{optimize_portfolio, rebalance_from_spec};
use spec_result::{
    PyCandidatePosition, PyPortfolioOptimizationResult, PyPortfolioOptimizationSpec,
    PyTradeUniverse,
};
use status_trade::{PyOptimizationStatus, PyTradeSpec};

/// Register optimization spec/result classes on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyWeightingScheme>()?;
    m.add_class::<PyMissingMetricPolicy>()?;
    m.add_class::<PyInequality>()?;
    m.add_class::<PyTradeDirection>()?;
    m.add_class::<PyTradeType>()?;
    m.add_class::<PyPerPositionMetric>()?;
    m.add_class::<PyPositionFilter>()?;
    m.add_class::<PyMetricExpr>()?;
    m.add_class::<PyObjective>()?;
    m.add_class::<PyConstraint>()?;
    m.add_class::<PyCandidatePosition>()?;
    m.add_class::<PyTradeUniverse>()?;
    m.add_class::<PyOptimizationStatus>()?;
    m.add_class::<PyTradeSpec>()?;
    m.add_class::<PyPortfolioOptimizationSpec>()?;
    m.add_class::<PyPortfolioOptimizationResult>()?;

    m.add_function(wrap_pyfunction!(optimize_portfolio, m)?)?;
    m.add_function(wrap_pyfunction!(rebalance_from_spec, m)?)?;

    Ok(())
}
