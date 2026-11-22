//! Python bindings for `finstack_quant_models::volatility::arbitrage`.
//!
//! Exposes a function-based API for model-free volatility surface arbitrage
//! detection. `check_surface` runs the suite on a core `VolSurface` with the
//! serde form of the Rust `ArbitrageCheckConfig`; the `*_grid` functions build
//! the surface internally from flat arrays so callers can work directly with
//! numpy-friendly inputs. The per-check functions return the serde rows of the
//! Rust `ArbitrageViolation`; `check_surface` and `check_surface_grid` return
//! a typed `ArbitrageReport`.

use crate::bindings::core::market_data::curves::PyVolSurface;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::module_utils::spec_from_py;
use crate::bindings::pydoc::pydoc;
use crate::bindings::table_builders::TableBuilder;
use finstack_quant_core::wire::serde_label;
use finstack_quant_models::volatility::arbitrage::{
    self as model_arbitrage, ArbitrageCheckConfig, ArbitrageReport,
};
use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::bindings::json_bridge::serde_to_py;
use crate::errors::core_to_py;

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

    fn __repr__(&self) -> String {
        format!(
            "ArbitrageReport(passed={}, total_violations={}, vol_surface_id='{}')",
            if self.inner.passed { "True" } else { "False" },
            self.inner.violations.len(),
            self.inner.vol_surface_id
        )
    }
}

py_getters!(PyArbitrageReport |s| {
    #[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "ArbitrageReport.vol_surface_id")]
    vol_surface_id -> &str = &s.inner.vol_surface_id;

    #[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "ArbitrageReport.passed")]
    passed -> bool = s.inner.passed;

    #[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "ArbitrageReport.total_violations")]
    total_violations -> usize = s.inner.violations.len();
});

impl PyArbitrageReport {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let mut violation_type = Vec::with_capacity(self.inner.violations.len());
        let mut severity = Vec::with_capacity(self.inner.violations.len());
        let mut strike = Vec::with_capacity(self.inner.violations.len());
        let mut expiry = Vec::with_capacity(self.inner.violations.len());
        let mut adjacent_expiry = Vec::with_capacity(self.inner.violations.len());
        let mut magnitude = Vec::with_capacity(self.inner.violations.len());
        let mut suggested_fix = Vec::with_capacity(self.inner.violations.len());
        let mut description = Vec::with_capacity(self.inner.violations.len());

        for v in &self.inner.violations {
            violation_type.push(serde_label(&v.violation_type).map_err(core_to_py)?);
            severity.push(serde_label(&v.severity).map_err(core_to_py)?);
            strike.push(v.location.strike);
            expiry.push(v.location.expiry);
            adjacent_expiry.push(v.location.adjacent_expiry);
            magnitude.push(v.magnitude);
            suggested_fix.push(v.suggested_fix);
            description.push(v.description.clone());
        }

        TableBuilder::new()
            .utf8("violation_type", violation_type)?
            .utf8("severity", severity)?
            .f64("strike", strike)?
            .f64("expiry", expiry)?
            .nullable_f64("adjacent_expiry", adjacent_expiry)?
            .f64("magnitude", magnitude)?
            .nullable_f64("suggested_fix", suggested_fix)?
            .utf8("description", description)?
            .build()
    }
}

tabular_methods!(
    PyArbitrageReport,
    "ArbitrageReport",
    module = "finstack_quant.models.volatility._volatility_arbitrage"
);
wire_methods!(
    PyArbitrageReport,
    ArbitrageReport,
    "ArbitrageReport",
    module = "finstack_quant.models.volatility._volatility_arbitrage"
);

#[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "check_butterfly_grid")]
#[pyfunction]
#[pyo3(signature = (expiries, strikes, vols, forward_prices, tolerance = model_arbitrage::DEFAULT_ARBITRAGE_TOLERANCE))]
fn check_butterfly_grid<'py>(
    py: Python<'py>,
    expiries: Vec<f64>,
    strikes: Vec<f64>,
    vols: Vec<Vec<f64>>,
    forward_prices: Vec<f64>,
    tolerance: f64,
) -> PyResult<Bound<'py, PyAny>> {
    let violations = model_arbitrage::check_butterfly_grid(
        &expiries,
        &strikes,
        &vols,
        forward_prices,
        tolerance,
    )
    .map_err(core_to_py)?;
    serde_to_py(py, &violations)
}

#[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "check_calendar_spread_grid")]
#[pyfunction]
#[pyo3(signature = (expiries, strikes, vols, forward_prices, tolerance = model_arbitrage::DEFAULT_ARBITRAGE_TOLERANCE))]
fn check_calendar_spread_grid<'py>(
    py: Python<'py>,
    expiries: Vec<f64>,
    strikes: Vec<f64>,
    vols: Vec<Vec<f64>>,
    forward_prices: Vec<f64>,
    tolerance: f64,
) -> PyResult<Bound<'py, PyAny>> {
    let violations = model_arbitrage::check_calendar_spread_grid(
        &expiries,
        &strikes,
        &vols,
        forward_prices,
        tolerance,
    )
    .map_err(core_to_py)?;
    serde_to_py(py, &violations)
}

#[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "check_local_vol_density_grid")]
#[pyfunction]
#[pyo3(signature = (expiries, strikes, vols, forward_prices))]
fn check_local_vol_density_grid<'py>(
    py: Python<'py>,
    expiries: Vec<f64>,
    strikes: Vec<f64>,
    vols: Vec<Vec<f64>>,
    forward_prices: Vec<f64>,
) -> PyResult<Bound<'py, PyAny>> {
    let violations =
        model_arbitrage::check_local_vol_density_grid(&expiries, &strikes, &vols, forward_prices)
            .map_err(core_to_py)?;
    serde_to_py(py, &violations)
}

#[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "check_surface_grid")]
#[pyfunction]
#[pyo3(signature = (expiries, strikes, vols, forward_prices, tolerance = model_arbitrage::DEFAULT_ARBITRAGE_TOLERANCE))]
fn check_surface_grid(
    expiries: Vec<f64>,
    strikes: Vec<f64>,
    vols: Vec<Vec<f64>>,
    forward_prices: Vec<f64>,
    tolerance: f64,
) -> PyResult<PyArbitrageReport> {
    model_arbitrage::check_surface_grid(&expiries, &strikes, &vols, forward_prices, tolerance)
        .map(|inner| PyArbitrageReport { inner })
        .map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.volatility._volatility_arbitrage", "check_surface")]
#[pyfunction]
#[pyo3(text_signature = "(surface, config)")]
fn check_surface(
    py: Python<'_>,
    surface: PyRef<'_, PyVolSurface>,
    config: &Bound<'_, PyAny>,
) -> PyResult<PyArbitrageReport> {
    let config: ArbitrageCheckConfig = spec_from_py(py, config, "ArbitrageCheckConfig")?;
    model_arbitrage::check_surface(&surface.inner, &config)
        .map(|inner| PyArbitrageReport { inner })
        .map_err(core_to_py)
}

/// Register volatility arbitrage functions on `finstack_quant.models.volatility`.
pub fn register(_py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    parent.add_class::<PyArbitrageReport>()?;
    parent.add_function(wrap_pyfunction!(check_butterfly_grid, parent)?)?;
    parent.add_function(wrap_pyfunction!(check_calendar_spread_grid, parent)?)?;
    parent.add_function(wrap_pyfunction!(check_local_vol_density_grid, parent)?)?;
    parent.add_function(wrap_pyfunction!(check_surface, parent)?)?;
    parent.add_function(wrap_pyfunction!(check_surface_grid, parent)?)?;

    Ok(())
}
