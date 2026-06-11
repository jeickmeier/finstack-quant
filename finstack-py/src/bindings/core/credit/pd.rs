//! Python bindings for `finstack_core::credit::pd` (calibration subset).

use finstack_core::credit::pd::{
    central_tendency as core_central_tendency, pit_to_ttc as core_pit_to_ttc,
    ttc_to_pit as core_ttc_to_pit, PdCycleParams,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use crate::errors::pd_calibration_to_py;

// ---------------------------------------------------------------------------
// PiT / TtC conversion
// ---------------------------------------------------------------------------

/// Convert a Point-in-Time PD to a Through-the-Cycle PD.
///
/// Uses the Merton-Vasicek single-factor model (Basel II IRB):
///
///   PD_TtC = Phi( Phi^{-1}(PD_PiT) * sqrt(1 - rho) + sqrt(rho) * z )
///
/// Arguments:
///     pit_pd: Point-in-Time PD in (0, 1).
///     asset_correlation: Asset correlation rho in (0, 1). Basel uses 0.12 - 0.24 for corporates.
///     cycle_index: Systematic risk factor z. 0 = average, < 0 = downturn, > 0 = benign.
#[pyfunction]
#[pyo3(text_signature = "(pit_pd, asset_correlation, cycle_index)")]
fn pit_to_ttc(pit_pd: f64, asset_correlation: f64, cycle_index: f64) -> PyResult<f64> {
    let params = PdCycleParams {
        asset_correlation,
        cycle_index,
    };
    core_pit_to_ttc(pit_pd, &params).map_err(pd_calibration_to_py)
}

/// Convert a Through-the-Cycle PD to a Point-in-Time PD.
///
/// Uses the Merton-Vasicek single-factor model (Basel II IRB):
///
///   PD_PiT = Phi( (Phi^{-1}(PD_TtC) - sqrt(rho) * z) / sqrt(1 - rho) )
///
/// Arguments:
///     ttc_pd: Through-the-Cycle PD in (0, 1).
///     asset_correlation: Asset correlation rho in (0, 1).
///     cycle_index: Systematic risk factor z. 0 = average, < 0 = downturn, > 0 = benign.
#[pyfunction]
#[pyo3(text_signature = "(ttc_pd, asset_correlation, cycle_index)")]
fn ttc_to_pit(ttc_pd: f64, asset_correlation: f64, cycle_index: f64) -> PyResult<f64> {
    let params = PdCycleParams {
        asset_correlation,
        cycle_index,
    };
    core_ttc_to_pit(ttc_pd, &params).map_err(pd_calibration_to_py)
}

/// Calibrate a central tendency (long-run average PD) from annual default rates
/// using the arithmetic mean (the standard regulatory TtC approach per
/// Basel IRB / EBA GL/2017/16).
///
/// Zero-default years are valid observations and are included in the average.
///
/// Returns the arithmetic mean in [0, 1].
#[pyfunction]
#[pyo3(text_signature = "(annual_default_rates)")]
fn central_tendency(annual_default_rates: Vec<f64>) -> PyResult<f64> {
    core_central_tendency(&annual_default_rates).map_err(pd_calibration_to_py)
}

// ---------------------------------------------------------------------------
// Register
// ---------------------------------------------------------------------------

/// Build the `finstack.core.credit.pd` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "pd")?;
    m.setattr(
        "__doc__",
        "Probability of default: PiT/TtC conversion (Merton-Vasicek) and central-tendency calibration.",
    )?;

    m.add_function(wrap_pyfunction!(pit_to_ttc, &m)?)?;
    m.add_function(wrap_pyfunction!(ttc_to_pit, &m)?)?;
    m.add_function(wrap_pyfunction!(central_tendency, &m)?)?;

    let all = PyList::new(py, ["pit_to_ttc", "ttc_to_pit", "central_tendency"])?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "pd",
        "finstack.core.credit",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
