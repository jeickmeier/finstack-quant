//! Python bindings for Expected Credit Loss (ECL) / IFRS 9 / CECL.
//!
//! Exposes the simplified staging-and-measurement workflow:
//!
//! - `Exposure` — a wrapper of the Rust `Exposure` plus the two lifetime PDs
//!   the simplified SICR test compares.
//! - `Stage`, `StagingConfig`, `QualitativeFlags`, `StageResult` — typed
//!   staging inputs and output.
//! - `classify_stage` — IFRS 9 three-stage classification with audit trail.
//! - `compute_ecl` / `compute_ecl_weighted` — single-scenario and
//!   probability-weighted ECL returning the full `WeightedEclResult`
//!   (per-scenario `EclResult` with `EclBucket` rows).
//!
//! PD term structures are passed as ``[(time_years, cumulative_pd), ...]``
//! knots; a ``(0.0, 0.0)`` anchor is inserted when absent.

mod impaired;
mod model;
mod staging;

use impaired::{
    PyImpairedEclResult, PyImpairedEclScenario, PyImpairedExposure, PyRecoveryEvidence,
    PyWeightedImpairedEclResult, compute_impaired_ecl, compute_impaired_ecl_weighted,
};
pub use model::{PyEclBucket, PyEclResult, PyExposure, PyWeightedEclResult};
use model::{PyEclScenario, PyEclScenarioOutcome, compute_ecl, compute_ecl_weighted};
use pyo3::prelude::*;
use staging::classify_stage;
pub use staging::{PyQualitativeFlags, PyStage, PyStageResult, PyStagingConfig};

/// Register ECL types and functions on the `statements_analytics` submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyImpairedEclScenario>()?;
    m.add_class::<PyWeightedImpairedEclResult>()?;
    m.add_function(wrap_pyfunction!(compute_impaired_ecl_weighted, m)?)?;
    m.add_class::<PyImpairedExposure>()?;
    m.add_class::<PyRecoveryEvidence>()?;
    m.add_class::<PyImpairedEclResult>()?;
    m.add_function(wrap_pyfunction!(compute_impaired_ecl, m)?)?;
    m.add_class::<PyStage>()?;
    m.add_class::<PyQualitativeFlags>()?;
    m.add_class::<PyStagingConfig>()?;
    m.add_class::<PyStageResult>()?;
    m.add_class::<PyExposure>()?;
    m.add_class::<PyEclBucket>()?;
    m.add_class::<PyEclResult>()?;
    m.add_class::<PyEclScenario>()?;
    m.add_class::<PyEclScenarioOutcome>()?;
    m.add_class::<PyWeightedEclResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(classify_stage, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(compute_ecl, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(compute_ecl_weighted, m)?)?;
    Ok(())
}
