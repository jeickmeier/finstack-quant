//! Result types for Monte Carlo simulations.

use crate::bindings::core::money::PyMoney;
use finstack_quant_monte_carlo::results::MoneyEstimate;
use pyo3::prelude::*;

/// Monte Carlo pricing result with discounted statistics.
#[pyclass(name = "MoneyEstimate", module = "finstack_quant.monte_carlo", frozen)]
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

/// Raw numerical estimate (non-currency).
#[pyclass(name = "Estimate", module = "finstack_quant.monte_carlo", frozen)]
pub struct PyEstimate {
    inner: finstack_quant_monte_carlo::estimate::Estimate,
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

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyMoneyEstimate>()?;
    m.add_class::<PyEstimate>()?;
    Ok(())
}
