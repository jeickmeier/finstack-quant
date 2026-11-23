//! Python bindings for the credit-correlation module.
//!
//! Exposes copula models, recovery models, factor models, and joint
//! probability utilities to Python under `finstack_quant.models.correlation`,
//! mirroring the Rust module [`finstack_quant_models::correlation`].

mod copula;
mod latent;
mod matrix;
mod recovery;

use crate::bindings::pydoc::pydoc;
use copula::{PyCopula, PyCopulaSpec};
use finstack_quant_models::correlation as corr;
use latent::{
    PyLatentFactorKind, PyLatentFactorSpec, PyLatentMultiFactor, PyLatentSingleFactor,
    PyLatentTwoFactor,
};
use matrix::{
    PyCorrelatedBernoulli, PyCreditExposure, PyPortfolioLossConfig, PyPortfolioLossResult,
    PyTrancheLossStatistics, cholesky_correlation, correlation_bounds, nearest_correlation,
    simulate_portfolio_loss, validate_correlation_matrix,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};
use recovery::{PyRecoveryModel, PyRecoverySpec};

/// Register the `correlation` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "correlation")?;
    m.setattr(
        "__doc__",
        pydoc!("finstack_quant.models.correlation", "__module__"),
    )?;

    m.add_class::<PyCopulaSpec>()?;
    m.add_class::<PyCopula>()?;
    m.add_class::<PyRecoverySpec>()?;
    m.add_class::<PyRecoveryModel>()?;
    m.add_class::<PyLatentFactorSpec>()?;
    m.add_class::<PyLatentFactorKind>()?;
    m.add_class::<PyLatentSingleFactor>()?;
    m.add_class::<PyLatentTwoFactor>()?;
    m.add_class::<PyLatentMultiFactor>()?;
    m.add_class::<PyCorrelatedBernoulli>()?;
    m.add_class::<PyCreditExposure>()?;
    m.add_class::<PyPortfolioLossConfig>()?;
    m.add_class::<PyPortfolioLossResult>()?;
    m.add_class::<PyTrancheLossStatistics>()?;
    m.add("MAX_PORTFOLIO_LOSS_PATHS", corr::MAX_PORTFOLIO_LOSS_PATHS)?;
    m.add_function(wrap_pyfunction!(correlation_bounds, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_correlation_matrix, &m)?)?;
    m.add_function(wrap_pyfunction!(nearest_correlation, &m)?)?;
    m.add_function(wrap_pyfunction!(cholesky_correlation, &m)?)?;
    m.add_function(wrap_pyfunction!(simulate_portfolio_loss, &m)?)?;

    let all = PyList::new(
        py,
        [
            "Copula",
            "CopulaSpec",
            "CorrelatedBernoulli",
            "CreditExposure",
            "LatentFactorKind",
            "LatentFactorSpec",
            "LatentMultiFactor",
            "LatentSingleFactor",
            "LatentTwoFactor",
            "MAX_PORTFOLIO_LOSS_PATHS",
            "PortfolioLossConfig",
            "PortfolioLossResult",
            "RecoveryModel",
            "RecoverySpec",
            "TrancheLossStatistics",
            "cholesky_correlation",
            "correlation_bounds",
            "nearest_correlation",
            "simulate_portfolio_loss",
            "validate_correlation_matrix",
        ],
    )?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;

    Ok(())
}
