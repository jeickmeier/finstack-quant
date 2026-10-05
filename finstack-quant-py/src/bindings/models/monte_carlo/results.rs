//! Result types for Monte Carlo simulations.

use crate::bindings::core::money::PyMoney;
use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
use crate::bindings::pandas_utils::dict_to_dataframe;
use finstack_quant_models::monte_carlo::results::MoneyEstimate;
use finstack_quant_models::monte_carlo::simulate::PathSummary;
use numpy::PyArray1;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Simulated paths of a Markov process on a shared time grid.
#[pyclass(
    name = "PathSummary",
    module = "finstack_quant.models.monte_carlo",
    frozen
)]
pub struct PyPathSummary {
    pub(crate) inner: PathSummary,
}

impl PyPathSummary {
    pub(super) fn from_inner(inner: PathSummary) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPathSummary {
    /// Number of independent random streams requested.
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths
    }

    /// Number of stored paths: ``num_paths``, or ``2 * num_paths`` with
    /// antithetic sampling.
    #[getter]
    fn num_simulated_paths(&self) -> usize {
        self.inner.num_simulated_paths
    }

    /// State dimension; equals ``len(factor_names)``.
    #[getter]
    fn dim(&self) -> usize {
        self.inner.dim
    }

    /// Simulation times in years, starting at zero.
    #[getter]
    fn times(&self) -> Vec<f64> {
        self.inner.times.clone()
    }

    /// Name of each state component, in state-vector order.
    #[getter]
    fn factor_names(&self) -> Vec<String> {
        self.inner.factor_names.clone()
    }

    /// States in row-major ``[path][time][factor]`` order.
    #[getter]
    fn values(&self) -> Vec<f64> {
        self.inner.values.clone()
    }

    /// Export the paths as a long pandas ``DataFrame``.
    ///
    /// One row per stored path and time, indexed by a ``(path, time)``
    /// ``MultiIndex`` in the order Rust produced, with one float64 column per
    /// entry of ``factor_names``. ``frame["spot"].unstack("path")`` gives the
    /// time-by-path table of one factor.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``values`` does not hold ``num_simulated_paths * len(times) *
    ///     len(factor_names)`` entries, which is only possible for a summary
    ///     rebuilt from inconsistent JSON.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let summary = &self.inner;
        let dim = summary.factor_names.len();
        let expected = summary
            .num_simulated_paths
            .saturating_mul(summary.times.len())
            .saturating_mul(dim);
        if dim == 0 || summary.values.len() != expected {
            return Err(crate::errors::value_error(format!(
                "PathSummary holds {} values but {} paths x {} times x {} factors need {}",
                summary.values.len(),
                summary.num_simulated_paths,
                summary.times.len(),
                dim,
                expected
            )));
        }
        let data = PyDict::new(py);
        for (factor, name) in summary.factor_names.iter().enumerate() {
            let column: Vec<f64> = summary
                .values
                .iter()
                .skip(factor)
                .step_by(dim)
                .copied()
                .collect();
            data.set_item(name, PyArray1::from_vec(py, column))?;
        }
        let paths: Vec<usize> = (0..summary.num_simulated_paths).collect();
        let kwargs = PyDict::new(py);
        kwargs.set_item("names", ["path", "time"])?;
        let index = py.import("pandas")?.getattr("MultiIndex")?.call_method(
            "from_product",
            ((paths, summary.times.clone()),),
            Some(&kwargs),
        )?;
        dict_to_dataframe(py, &data, Some(index))
    }

    fn __repr__(&self) -> String {
        format!(
            "PathSummary(paths={}, points={}, factors={:?})",
            self.inner.num_simulated_paths,
            self.inner.times.len(),
            self.inner.factor_names
        )
    }
}

wire_methods!(PyPathSummary, PathSummary, "PathSummary");
impl_repr_html_via_dataframe!(PyPathSummary);

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
    m.add_class::<PyPathSummary>()?;
    m.add_class::<PyMoneyEstimate>()?;
    m.add_class::<PyEstimate>()?;
    Ok(())
}
