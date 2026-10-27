//! Python bindings for `finstack_quant_models::volatility::arbitrage`.
//!
//! Exposes a function-based API for model-free volatility surface arbitrage
//! detection. Vol surfaces are constructed internally from flat arrays so
//! callers can work directly with numpy-friendly inputs without first
//! building a `VolSurface` wrapper. The per-check functions return the serde
//! rows of the Rust `ArbitrageViolation`; the combined `check_surface_grid`
//! returns a typed `ArbitrageReport`.

use crate::bindings::macros::wire_methods;
use crate::bindings::pydoc::pydoc;
use finstack_quant_core::wire::serde_label;
use finstack_quant_models::volatility::arbitrage::{self as model_arbitrage, ArbitrageReport};
use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::bindings::pandas_utils::{serde_rows_to_dataframe, serde_to_py};
use crate::errors::core_to_py;

/// Flatten one violation into a long-format row for ``to_dataframe``.
#[derive(serde::Serialize)]
struct ViolationRow<'a> {
    violation_type: String,
    severity: String,
    strike: f64,
    expiry: f64,
    adjacent_expiry: Option<f64>,
    magnitude: f64,
    suggested_fix: Option<f64>,
    description: &'a str,
}

#[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "ArbitrageReport")]
#[pyclass(
    name = "ArbitrageReport",
    module = "finstack_quant.models.volatility",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyArbitrageReport {
    pub(crate) inner: ArbitrageReport,
}

#[pymethods]
impl PyArbitrageReport {
    #[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "ArbitrageReport.vol_surface_id")]
    #[getter]
    fn vol_surface_id(&self) -> &str {
        &self.inner.vol_surface_id
    }

    #[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "ArbitrageReport.passed")]
    #[getter]
    fn passed(&self) -> bool {
        self.inner.passed
    }

    #[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "ArbitrageReport.total_violations")]
    #[getter]
    fn total_violations(&self) -> usize {
        self.inner.violations.len()
    }

    #[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "ArbitrageReport.violations")]
    #[getter]
    fn violations<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.violations)
    }

    #[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "ArbitrageReport.by_severity")]
    #[getter]
    fn by_severity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.counts_by_severity)
    }

    #[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "ArbitrageReport.by_type")]
    #[getter]
    fn by_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.counts_by_type)
    }

    #[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "ArbitrageReport.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = self
            .inner
            .violations
            .iter()
            .map(|v| {
                Ok(ViolationRow {
                    violation_type: serde_label(&v.violation_type).map_err(core_to_py)?,
                    severity: serde_label(&v.severity).map_err(core_to_py)?,
                    strike: v.location.strike,
                    expiry: v.location.expiry,
                    adjacent_expiry: v.location.adjacent_expiry,
                    magnitude: v.magnitude,
                    suggested_fix: v.suggested_fix,
                    description: v.description.as_str(),
                })
            })
            .collect::<PyResult<Vec<_>>>()?;
        serde_rows_to_dataframe(py, &rows)
    }

    fn __repr__(&self) -> String {
        format!(
            "ArbitrageReport(passed={}, total_violations={}, vol_surface_id='{}')",
            if self.inner.passed { "True" } else { "False" },
            self.inner.violations.len(),
            self.inner.vol_surface_id
        )
    }
}

wire_methods!(
    PyArbitrageReport,
    ArbitrageReport,
    "ArbitrageReport",
    module = "finstack_quant.models.volatility._volatility_arbitrage"
);

#[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "check_butterfly_grid")]
#[pyfunction]
#[pyo3(signature = (strikes, expiries, vols, forward_prices, tolerance = model_arbitrage::DEFAULT_ARBITRAGE_TOLERANCE))]
fn check_butterfly_grid<'py>(
    py: Python<'py>,
    strikes: Vec<f64>,
    expiries: Vec<f64>,
    vols: Vec<Vec<f64>>,
    forward_prices: Vec<f64>,
    tolerance: f64,
) -> PyResult<Bound<'py, PyAny>> {
    let violations = model_arbitrage::check_butterfly_grid(
        &strikes,
        &expiries,
        &vols,
        forward_prices,
        tolerance,
    )
    .map_err(core_to_py)?;
    serde_to_py(py, &violations)
}

#[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "check_calendar_spread_grid")]
#[pyfunction]
#[pyo3(signature = (strikes, expiries, vols, forward_prices, tolerance = model_arbitrage::DEFAULT_ARBITRAGE_TOLERANCE))]
fn check_calendar_spread_grid<'py>(
    py: Python<'py>,
    strikes: Vec<f64>,
    expiries: Vec<f64>,
    vols: Vec<Vec<f64>>,
    forward_prices: Vec<f64>,
    tolerance: f64,
) -> PyResult<Bound<'py, PyAny>> {
    let violations = model_arbitrage::check_calendar_spread_grid(
        &strikes,
        &expiries,
        &vols,
        forward_prices,
        tolerance,
    )
    .map_err(core_to_py)?;
    serde_to_py(py, &violations)
}

#[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "check_local_vol_density_grid")]
#[pyfunction]
#[pyo3(signature = (strikes, expiries, vols, forward_prices))]
fn check_local_vol_density_grid<'py>(
    py: Python<'py>,
    strikes: Vec<f64>,
    expiries: Vec<f64>,
    vols: Vec<Vec<f64>>,
    forward_prices: Vec<f64>,
) -> PyResult<Bound<'py, PyAny>> {
    let violations =
        model_arbitrage::check_local_vol_density_grid(&strikes, &expiries, &vols, forward_prices)
            .map_err(core_to_py)?;
    serde_to_py(py, &violations)
}

#[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "check_surface_grid")]
#[pyfunction]
#[pyo3(signature = (strikes, expiries, vols, forward_prices, tolerance = model_arbitrage::DEFAULT_ARBITRAGE_TOLERANCE))]
fn check_surface_grid(
    strikes: Vec<f64>,
    expiries: Vec<f64>,
    vols: Vec<Vec<f64>>,
    forward_prices: Vec<f64>,
    tolerance: f64,
) -> PyResult<PyArbitrageReport> {
    model_arbitrage::check_surface_grid(&strikes, &expiries, &vols, forward_prices, tolerance)
        .map(|inner| PyArbitrageReport { inner })
        .map_err(core_to_py)
}

/// Register volatility arbitrage functions on `finstack_quant.models.volatility`.
pub fn register(_py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    parent.add_class::<PyArbitrageReport>()?;
    parent.add_function(wrap_pyfunction!(check_butterfly_grid, parent)?)?;
    parent.add_function(wrap_pyfunction!(check_calendar_spread_grid, parent)?)?;
    parent.add_function(wrap_pyfunction!(check_local_vol_density_grid, parent)?)?;
    parent.add_function(wrap_pyfunction!(check_surface_grid, parent)?)?;

    Ok(())
}
