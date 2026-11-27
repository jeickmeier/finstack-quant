//! Python bindings for DCF, LBO and the orchestrated corporate analysis.
//!
//! Typed inputs (`TerminalValueSpec`, `EquityBridge`, `ValuationDiscounts`,
//! `LboCheckMappings`) and typed results (`CorporateValuationResult`,
//! `DcfSensitivityResult`, `LboResult`, `CorporateAnalysis`) wrap the Rust
//! types one-to-one. The Rust config types (`DcfOptions`, `LboConfig`) are
//! taken in their serde form (dict), where a typed input may
//! sit in place of its nested dict. A string is a value, never JSON text.

mod corporate;
mod inputs;
mod sensitivity;

pub use corporate::{PyCorporateAnalysis, PyLboCheckMappings, PyLboResult, PyWaccInputs};
use corporate::{dcf_sensitivity, evaluate_dcf, evaluate_lbo, run_corporate_analysis, wacc};
pub(crate) use inputs::PyStatementValuationCut;
use inputs::{
    PyCorporateDcfRequest, PyDcfInputs, PyDcfOptions, PyDcfProjection, PyTerminalFcffBasis,
    evaluate_corporate_dcf, evaluate_dcf_from_results,
};
pub use inputs::{PyEquityBridge, PyTerminalValueSpec, PyValuationDiscounts};
use pyo3::prelude::*;
pub use sensitivity::{PyCorporateValuationResult, PyDcfSensitivityResult};
use sensitivity::{
    PyDcfDistributionResult, PyDcfSensitivityGridResult, PyDcfSensitivityGridSpec,
    dcf_sensitivity_grid, dcf_sensitivity_on_results, evaluate_dcf_monte_carlo,
};

/// Register valuation types and functions.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyDcfDistributionResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(evaluate_dcf_monte_carlo, m)?)?;
    m.add_class::<PyDcfInputs>()?;
    m.add_class::<PyDcfOptions>()?;
    m.add_class::<PyStatementValuationCut>()?;
    m.add_class::<PyTerminalFcffBasis>()?;
    m.add_class::<PyDcfProjection>()?;
    m.add_class::<PyCorporateDcfRequest>()?;
    m.add_class::<PyDcfSensitivityGridSpec>()?;
    m.add_class::<PyDcfSensitivityGridResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(evaluate_corporate_dcf, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(evaluate_dcf_from_results, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(dcf_sensitivity_grid, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(dcf_sensitivity_on_results, m)?)?;
    m.add_class::<PyTerminalValueSpec>()?;
    m.add_class::<PyEquityBridge>()?;
    m.add_class::<PyValuationDiscounts>()?;
    m.add_class::<PyCorporateValuationResult>()?;
    m.add_class::<PyDcfSensitivityResult>()?;
    m.add_class::<PyLboCheckMappings>()?;
    m.add_class::<PyLboResult>()?;
    m.add_class::<PyCorporateAnalysis>()?;
    m.add_class::<PyWaccInputs>()?;
    m.add_function(pyo3::wrap_pyfunction!(evaluate_dcf, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(dcf_sensitivity, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(evaluate_lbo, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(wacc, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(run_corporate_analysis, m)?)?;
    Ok(())
}
