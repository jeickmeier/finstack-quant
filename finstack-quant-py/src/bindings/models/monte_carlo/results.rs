//! Result types for Monte Carlo simulations.

use crate::bindings::core::money::PyMoney;
use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
use crate::bindings::pandas_utils::dict_to_dataframe;
use finstack_quant_models::monte_carlo::results::MoneyEstimate;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Compact captured GBM paths for plotting and diagnostics.
#[pyclass(
    name = "GbmPathSummary",
    module = "finstack_quant.models.monte_carlo",
    frozen
)]
pub struct PyGbmPathSummary {
    inner: finstack_quant_models::monte_carlo::GbmPathSummary,
}

impl PyGbmPathSummary {
    pub(super) fn from_inner(inner: finstack_quant_models::monte_carlo::GbmPathSummary) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyGbmPathSummary {
    /// Number of independent path estimators.
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths
    }

    /// Total number of simulated sample paths.
    #[getter]
    fn num_simulated_paths(&self) -> usize {
        self.inner.num_simulated_paths
    }

    /// Shared path times in year fractions, including time zero.
    #[getter]
    fn times(&self) -> Vec<f64> {
        self.inner.times.clone()
    }

    /// Captured spot paths in deterministic path-id order.
    #[getter]
    fn paths(&self) -> Vec<Vec<f64>> {
        self.inner.paths.clone()
    }

    /// Export the captured paths as a pandas ``DataFrame`` indexed by time.
    ///
    /// Columns: ``path_0``, ``path_1``, ... — one column per captured path,
    /// in the deterministic path-id order Rust produced. The index is the
    /// shared time grid in year fractions, including time zero.
    ///
    /// Wide (time × path) rather than one row: it is the shape
    /// ``df.plot()`` and ``df.quantile(axis=1)`` expect for a path bundle, and
    /// every path already shares the one time grid. There is always at least
    /// one column: the engine rejects a zero-path simulation.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a captured path's length differs from the time grid's, which
    ///     would silently misalign the index.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let n_times = self.inner.times.len();
        let data = PyDict::new(py);
        for (path_id, path) in self.inner.paths.iter().enumerate() {
            if path.len() != n_times {
                return Err(crate::errors::value_error(format!(
                    "path {} has {} points but the time grid has {}",
                    path_id,
                    path.len(),
                    n_times
                )));
            }
            data.set_item(format!("path_{path_id}"), path.clone())?;
        }
        let index = self.inner.times.clone().into_pyobject(py)?.into_any();
        dict_to_dataframe(py, &data, Some(index))
    }

    fn __repr__(&self) -> String {
        format!(
            "GbmPathSummary(paths={}, points={})",
            self.inner.paths.len(),
            self.inner.times.len()
        )
    }
}

wire_methods!(
    PyGbmPathSummary,
    finstack_quant_models::monte_carlo::GbmPathSummary,
    "GbmPathSummary"
);
impl_repr_html_via_dataframe!(PyGbmPathSummary);

/// Monte Carlo pricing result with discounted statistics.
#[pyclass(
    name = "MoneyEstimate",
    module = "finstack_quant.models.monte_carlo",
    frozen
)]
pub struct PyMoneyEstimate {
    inner: MoneyEstimate,
}

impl PyMoneyEstimate {
    pub(super) fn from_inner(inner: MoneyEstimate) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyMoneyEstimate {
    /// Discounted mean present value.
    #[getter]
    fn mean(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.mean)
    }

    /// Standard error of the discounted mean.
    #[getter]
    fn stderr(&self) -> f64 {
        self.inner.stderr
    }

    /// Sample standard deviation (if available).
    #[getter]
    fn std_dev(&self) -> Option<f64> {
        self.inner.std_dev
    }

    /// Lower bound of the 95% CI.
    #[getter]
    fn ci_lower(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.ci_95.0)
    }

    /// Upper bound of the 95% CI.
    #[getter]
    fn ci_upper(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.ci_95.1)
    }

    /// Number of independent path estimators contributing to the result.
    ///
    /// Equals the configured `num_paths` when antithetic variates are off, or
    /// half the number of simulated paths when antithetic pairing is on.
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths
    }

    /// Total number of simulated sample paths.
    ///
    /// Equals ``num_paths`` without variance reduction, or ``2 * num_paths``
    /// when antithetic variates are enabled.
    #[getter]
    fn num_simulated_paths(&self) -> usize {
        self.inner.num_simulated_paths
    }

    /// Median of captured discounted path values (if captured).
    #[getter]
    fn median(&self) -> Option<f64> {
        self.inner.median
    }

    /// 25th percentile of captured discounted path values (if captured).
    #[getter]
    fn percentile_25(&self) -> Option<f64> {
        self.inner.percentile_25
    }

    /// 75th percentile of captured discounted path values (if captured).
    #[getter]
    fn percentile_75(&self) -> Option<f64> {
        self.inner.percentile_75
    }

    /// Minimum of captured discounted path values (if captured).
    #[getter]
    fn min(&self) -> Option<f64> {
        self.inner.min
    }

    /// Maximum of captured discounted path values (if captured).
    #[getter]
    fn max(&self) -> Option<f64> {
        self.inner.max
    }

    /// Relative standard error (stderr / |mean|).
    fn relative_stderr(&self) -> f64 {
        self.inner.relative_stderr()
    }

    fn __repr__(&self) -> String {
        format!(
            "MoneyEstimate(mean={}, stderr={:.6}, n={})",
            self.inner.mean, self.inner.stderr, self.inner.num_paths,
        )
    }
}

wire_methods!(PyMoneyEstimate, MoneyEstimate, "MoneyEstimate");

/// Raw numerical estimate (non-currency).
#[pyclass(
    name = "Estimate",
    module = "finstack_quant.models.monte_carlo",
    frozen
)]
pub struct PyEstimate {
    inner: finstack_quant_models::monte_carlo::estimate::Estimate,
}

impl PyEstimate {
    pub(super) fn from_inner(
        inner: finstack_quant_models::monte_carlo::estimate::Estimate,
    ) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyEstimate {
    /// Point estimate (mean).
    #[getter]
    fn mean(&self) -> f64 {
        self.inner.mean
    }

    /// Standard error.
    #[getter]
    fn stderr(&self) -> f64 {
        self.inner.stderr
    }

    /// Sample standard deviation (if available).
    #[getter]
    fn std_dev(&self) -> Option<f64> {
        self.inner.std_dev
    }

    /// Lower 95% CI bound.
    #[getter]
    fn ci_lower(&self) -> f64 {
        self.inner.ci_95.0
    }

    /// Upper 95% CI bound.
    #[getter]
    fn ci_upper(&self) -> f64 {
        self.inner.ci_95.1
    }

    /// Number of independent path estimators contributing to the estimate.
    ///
    /// Equals the configured ``num_paths`` without variance reduction, or
    /// half the number of simulated paths when antithetic pairing is on.
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths
    }

    /// Total number of simulated sample paths.
    ///
    /// Equals ``num_paths`` without variance reduction, or ``2 * num_paths``
    /// when antithetic variates are enabled.
    #[getter]
    fn num_simulated_paths(&self) -> usize {
        self.inner.num_simulated_paths
    }

    /// Median of captured path values (if captured).
    #[getter]
    fn median(&self) -> Option<f64> {
        self.inner.median
    }

    /// 25th percentile of captured path values (if captured).
    #[getter]
    fn percentile_25(&self) -> Option<f64> {
        self.inner.percentile_25
    }

    /// 75th percentile of captured path values (if captured).
    #[getter]
    fn percentile_75(&self) -> Option<f64> {
        self.inner.percentile_75
    }

    /// Minimum of captured path values (if captured).
    #[getter]
    fn min(&self) -> Option<f64> {
        self.inner.min
    }

    /// Maximum of captured path values (if captured).
    #[getter]
    fn max(&self) -> Option<f64> {
        self.inner.max
    }

    fn __repr__(&self) -> String {
        format!(
            "Estimate(mean={:.6}, stderr={:.6}, n={})",
            self.inner.mean, self.inner.stderr, self.inner.num_paths,
        )
    }
}

wire_methods!(
    PyEstimate,
    finstack_quant_models::monte_carlo::estimate::Estimate,
    "Estimate"
);

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyGbmPathSummary>()?;
    m.add_class::<PyMoneyEstimate>()?;
    m.add_class::<PyEstimate>()?;
    Ok(())
}
