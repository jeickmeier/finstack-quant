//! Result types for Monte Carlo simulations.

use crate::bindings::core::money::PyMoney;
use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
use crate::bindings::pandas_utils::dict_to_dataframe;
use crate::bindings::pydoc::pydoc;
use finstack_quant_models::monte_carlo::greeks::lrm::LrmGreeks;
use finstack_quant_models::monte_carlo::results::MoneyEstimate;
use finstack_quant_models::monte_carlo::simulate::PathSummary;
use numpy::PyArray1;
use pyo3::prelude::*;
use pyo3::types::PyDict;

#[doc = pydoc!("finstack_quant.models.monte_carlo", "PathSummary")]
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
    #[doc = pydoc!("finstack_quant.models.monte_carlo", "PathSummary.num_paths")]
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "PathSummary.num_simulated_paths")]
    #[getter]
    fn num_simulated_paths(&self) -> usize {
        self.inner.num_simulated_paths
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "PathSummary.dim")]
    #[getter]
    fn dim(&self) -> usize {
        self.inner.dim
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "PathSummary.times")]
    #[getter]
    fn times(&self) -> Vec<f64> {
        self.inner.times.clone()
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "PathSummary.factor_names")]
    #[getter]
    fn factor_names(&self) -> Vec<String> {
        self.inner.factor_names.clone()
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "PathSummary.values")]
    #[getter]
    fn values(&self) -> Vec<f64> {
        self.inner.values.clone()
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "PathSummary.to_dataframe")]
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

wire_methods!(
    PyPathSummary,
    PathSummary,
    "PathSummary",
    module = "finstack_quant.models.monte_carlo"
);
impl_repr_html_via_dataframe!(PyPathSummary);

#[doc = pydoc!("finstack_quant.models.monte_carlo", "MoneyEstimate")]
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
    #[doc = pydoc!("finstack_quant.models.monte_carlo", "MoneyEstimate.mean")]
    #[getter]
    fn mean(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.mean)
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "MoneyEstimate.stderr")]
    #[getter]
    fn stderr(&self) -> f64 {
        self.inner.stderr
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "MoneyEstimate.std_dev")]
    #[getter]
    fn std_dev(&self) -> Option<f64> {
        self.inner.std_dev
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "MoneyEstimate.ci_lower")]
    #[getter]
    fn ci_lower(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.ci_95.0)
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "MoneyEstimate.ci_upper")]
    #[getter]
    fn ci_upper(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.ci_95.1)
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "MoneyEstimate.num_paths")]
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "MoneyEstimate.num_simulated_paths")]
    #[getter]
    fn num_simulated_paths(&self) -> usize {
        self.inner.num_simulated_paths
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "MoneyEstimate.median")]
    #[getter]
    fn median(&self) -> Option<f64> {
        self.inner.median
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "MoneyEstimate.percentile_25")]
    #[getter]
    fn percentile_25(&self) -> Option<f64> {
        self.inner.percentile_25
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "MoneyEstimate.percentile_75")]
    #[getter]
    fn percentile_75(&self) -> Option<f64> {
        self.inner.percentile_75
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "MoneyEstimate.min")]
    #[getter]
    fn min(&self) -> Option<f64> {
        self.inner.min
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "MoneyEstimate.max")]
    #[getter]
    fn max(&self) -> Option<f64> {
        self.inner.max
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "MoneyEstimate.relative_stderr")]
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

wire_methods!(
    PyMoneyEstimate,
    MoneyEstimate,
    "MoneyEstimate",
    module = "finstack_quant.models.monte_carlo"
);

#[doc = pydoc!("finstack_quant.models.monte_carlo", "Estimate")]
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
    #[doc = pydoc!("finstack_quant.models.monte_carlo", "Estimate.mean")]
    #[getter]
    fn mean(&self) -> f64 {
        self.inner.mean
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "Estimate.stderr")]
    #[getter]
    fn stderr(&self) -> f64 {
        self.inner.stderr
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "Estimate.std_dev")]
    #[getter]
    fn std_dev(&self) -> Option<f64> {
        self.inner.std_dev
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "Estimate.ci_lower")]
    #[getter]
    fn ci_lower(&self) -> f64 {
        self.inner.ci_95.0
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "Estimate.ci_upper")]
    #[getter]
    fn ci_upper(&self) -> f64 {
        self.inner.ci_95.1
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "Estimate.num_paths")]
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "Estimate.num_simulated_paths")]
    #[getter]
    fn num_simulated_paths(&self) -> usize {
        self.inner.num_simulated_paths
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "Estimate.median")]
    #[getter]
    fn median(&self) -> Option<f64> {
        self.inner.median
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "Estimate.percentile_25")]
    #[getter]
    fn percentile_25(&self) -> Option<f64> {
        self.inner.percentile_25
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "Estimate.percentile_75")]
    #[getter]
    fn percentile_75(&self) -> Option<f64> {
        self.inner.percentile_75
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "Estimate.min")]
    #[getter]
    fn min(&self) -> Option<f64> {
        self.inner.min
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "Estimate.max")]
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
    "Estimate",
    module = "finstack_quant.models.monte_carlo"
);

#[doc = pydoc!("finstack_quant.models.monte_carlo", "LrmGreeks")]
#[pyclass(
    name = "LrmGreeks",
    module = "finstack_quant.models.monte_carlo",
    frozen
)]
pub struct PyLrmGreeks {
    pub(crate) inner: LrmGreeks,
}

impl PyLrmGreeks {
    pub(super) fn from_inner(inner: LrmGreeks) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyLrmGreeks {
    #[doc = pydoc!("finstack_quant.models.monte_carlo", "LrmGreeks.price")]
    #[getter]
    fn price(&self) -> PyMoneyEstimate {
        PyMoneyEstimate::from_inner(self.inner.price.clone())
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "LrmGreeks.delta")]
    #[getter]
    fn delta(&self) -> PyEstimate {
        PyEstimate::from_inner(self.inner.delta.clone())
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "LrmGreeks.vega")]
    #[getter]
    fn vega(&self) -> PyEstimate {
        PyEstimate::from_inner(self.inner.vega.clone())
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo", "LrmGreeks.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let greeks = &self.inner;
        let price = &greeks.price;
        let rows = [
            (
                price.mean.amount(),
                price.stderr,
                price.ci_95.0.amount(),
                price.ci_95.1.amount(),
            ),
            (
                greeks.delta.mean,
                greeks.delta.stderr,
                greeks.delta.ci_95.0,
                greeks.delta.ci_95.1,
            ),
            (
                greeks.vega.mean,
                greeks.vega.stderr,
                greeks.vega.ci_95.0,
                greeks.vega.ci_95.1,
            ),
        ];
        let data = PyDict::new(py);
        data.set_item("mean", rows.map(|row| row.0).to_vec())?;
        data.set_item("stderr", rows.map(|row| row.1).to_vec())?;
        data.set_item("ci_lower", rows.map(|row| row.2).to_vec())?;
        data.set_item("ci_upper", rows.map(|row| row.3).to_vec())?;
        let index = pyo3::types::PyList::new(py, ["price", "delta", "vega"])?;
        dict_to_dataframe(py, &data, Some(index.into_any()))
    }

    fn __repr__(&self) -> String {
        format!(
            "LrmGreeks(price={}, delta={:.6}, vega={:.6}, n={})",
            self.inner.price.mean,
            self.inner.delta.mean,
            self.inner.vega.mean,
            self.inner.price.num_paths,
        )
    }
}

wire_methods!(
    PyLrmGreeks,
    LrmGreeks,
    "LrmGreeks",
    module = "finstack_quant.models.monte_carlo"
);
impl_repr_html_via_dataframe!(PyLrmGreeks);

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPathSummary>()?;
    m.add_class::<PyLrmGreeks>()?;
    m.add_class::<PyMoneyEstimate>()?;
    m.add_class::<PyEstimate>()?;
    Ok(())
}
