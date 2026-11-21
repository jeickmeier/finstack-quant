//! Result types for Monte Carlo simulations.

use crate::bindings::core::money::PyMoney;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::{tabular_methods, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::bindings::table_builders::TableBuilder;
use crate::errors::value_error;
use finstack_quant_models::monte_carlo::greeks::lrm::LrmGreeks;
use finstack_quant_models::monte_carlo::pricer::lsmc::{ExerciseBoundaryPoint, LsmcResult};
use finstack_quant_models::monte_carlo::results::MoneyEstimate;
use finstack_quant_models::monte_carlo::simulate::PathSummary;
use numpy::{PyArray1, PyArray3, PyArrayMethods};
use pyo3::prelude::*;
use pyo3::types::PyInt;

#[doc = pydoc!("finstack_quant.models.monte_carlo._results", "PathSummary")]
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

    fn validated_shape(&self) -> PyResult<[usize; 3]> {
        let summary = &self.inner;
        let shape = [
            summary.num_simulated_paths,
            summary.times.len(),
            summary.dim,
        ];
        let [paths, times, dim] = shape;
        let expected = paths
            .checked_mul(times)
            .and_then(|points| points.checked_mul(dim));
        if times == 0
            || dim == 0
            || dim != summary.factor_names.len()
            || expected != Some(summary.values.len())
        {
            return Err(crate::errors::value_error(
                "PathSummary dimensions do not match stored paths, times, and factors",
            ));
        }
        Ok(shape)
    }
}

#[pymethods]
impl PyPathSummary {
    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "PathSummary.num_paths")]
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "PathSummary.num_simulated_paths")]
    #[getter]
    fn num_simulated_paths(&self) -> usize {
        self.inner.num_simulated_paths
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "PathSummary.dim")]
    #[getter]
    fn dim(&self) -> usize {
        self.inner.dim
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "PathSummary.times")]
    #[getter]
    fn times(&self) -> Vec<f64> {
        self.inner.times.clone()
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "PathSummary.factor_names")]
    #[getter]
    fn factor_names(&self) -> Vec<String> {
        self.inner.factor_names.clone()
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "PathSummary.values")]
    #[getter]
    fn values(&self) -> Vec<f64> {
        self.inner.values.clone()
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "PathSummary.to_numpy")]
    fn to_numpy<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray3<f64>>> {
        let shape = self.validated_shape()?;
        PyArray1::from_vec(py, self.inner.values.clone()).reshape(shape)
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "PathSummary.terminal_values")]
    #[pyo3(signature = (factor=None))]
    fn terminal_values<'py>(
        &self,
        py: Python<'py>,
        factor: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Bound<'py, PyArray1<f64>>> {
        let [paths, times, dim] = self.validated_shape()?;
        let index = match factor {
            None => 0,
            Some(factor) => match factor.extract::<String>() {
                Ok(name) => self
                    .inner
                    .factor_names
                    .iter()
                    .position(|candidate| candidate == &name)
                    .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(name))?,
                Err(_) => factor
                    .cast::<PyInt>()?
                    .extract::<usize>()
                    .ok()
                    .filter(|index| *index < dim)
                    .ok_or_else(|| {
                        pyo3::exceptions::PyIndexError::new_err("factor index out of bounds")
                    })?,
            },
        };
        let values = (0..paths)
            .map(|path| self.inner.values[(path * times + (times - 1)) * dim + index])
            .collect();
        Ok(PyArray1::from_vec(py, values))
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "PathSummary.to_pandas")]
    #[pyo3(text_signature = "($self)")]
    fn to_pandas<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let bound = Bound::new(py, self.to_arrow_table()?)?;
        let frame = PyArrowTable::to_pandas(&bound, py)?;
        frame.call_method1("set_index", (["path", "time"],))
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

impl PyPathSummary {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let [paths, times, dim] = self.validated_shape()?;
        let summary = &self.inner;
        let total = paths * times;
        let mut path_col = Vec::with_capacity(total);
        let mut time_col = Vec::with_capacity(total);
        for p in 0..paths {
            let p_i64 =
                i64::try_from(p).map_err(|e| value_error(format!("path index overflow: {e}")))?;
            for &t in &summary.times {
                path_col.push(p_i64);
                time_col.push(t);
            }
        }
        let mut builder = TableBuilder::new()
            .i64("path", path_col)?
            .f64("time", time_col)?;
        for (factor, name) in summary.factor_names.iter().enumerate() {
            let column: Vec<f64> = summary
                .values
                .iter()
                .skip(factor)
                .step_by(dim)
                .copied()
                .collect();
            builder = builder.f64(name, column)?;
        }
        builder.build()
    }
}

tabular_methods!(
    PyPathSummary,
    "PathSummary",
    module = "finstack_quant.models.monte_carlo._results",
    custom_pandas
);
wire_methods!(
    PyPathSummary,
    PathSummary,
    "PathSummary",
    module = "finstack_quant.models.monte_carlo._results"
);

#[doc = pydoc!("finstack_quant.models.monte_carlo._results", "MoneyEstimate")]
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
    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "MoneyEstimate.mean")]
    #[getter]
    fn mean(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.mean)
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "MoneyEstimate.stderr")]
    #[getter]
    fn stderr(&self) -> f64 {
        self.inner.stderr
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "MoneyEstimate.std_dev")]
    #[getter]
    fn std_dev(&self) -> Option<f64> {
        self.inner.std_dev
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "MoneyEstimate.ci_lower")]
    #[getter]
    fn ci_lower(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.ci_95.0)
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "MoneyEstimate.ci_upper")]
    #[getter]
    fn ci_upper(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.ci_95.1)
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "MoneyEstimate.num_paths")]
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "MoneyEstimate.num_simulated_paths")]
    #[getter]
    fn num_simulated_paths(&self) -> usize {
        self.inner.num_simulated_paths
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "MoneyEstimate.median")]
    #[getter]
    fn median(&self) -> Option<f64> {
        self.inner.median
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "MoneyEstimate.percentile_25")]
    #[getter]
    fn percentile_25(&self) -> Option<f64> {
        self.inner.percentile_25
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "MoneyEstimate.percentile_75")]
    #[getter]
    fn percentile_75(&self) -> Option<f64> {
        self.inner.percentile_75
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "MoneyEstimate.min")]
    #[getter]
    fn min(&self) -> Option<f64> {
        self.inner.min
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "MoneyEstimate.max")]
    #[getter]
    fn max(&self) -> Option<f64> {
        self.inner.max
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "MoneyEstimate.relative_stderr")]
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
    module = "finstack_quant.models.monte_carlo._results"
);

#[doc = pydoc!("finstack_quant.models.monte_carlo._results", "Estimate")]
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
    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "Estimate.mean")]
    #[getter]
    fn mean(&self) -> f64 {
        self.inner.mean
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "Estimate.stderr")]
    #[getter]
    fn stderr(&self) -> f64 {
        self.inner.stderr
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "Estimate.std_dev")]
    #[getter]
    fn std_dev(&self) -> Option<f64> {
        self.inner.std_dev
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "Estimate.ci_lower")]
    #[getter]
    fn ci_lower(&self) -> f64 {
        self.inner.ci_95.0
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "Estimate.ci_upper")]
    #[getter]
    fn ci_upper(&self) -> f64 {
        self.inner.ci_95.1
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "Estimate.num_paths")]
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "Estimate.num_simulated_paths")]
    #[getter]
    fn num_simulated_paths(&self) -> usize {
        self.inner.num_simulated_paths
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "Estimate.median")]
    #[getter]
    fn median(&self) -> Option<f64> {
        self.inner.median
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "Estimate.percentile_25")]
    #[getter]
    fn percentile_25(&self) -> Option<f64> {
        self.inner.percentile_25
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "Estimate.percentile_75")]
    #[getter]
    fn percentile_75(&self) -> Option<f64> {
        self.inner.percentile_75
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "Estimate.min")]
    #[getter]
    fn min(&self) -> Option<f64> {
        self.inner.min
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "Estimate.max")]
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
    module = "finstack_quant.models.monte_carlo._results"
);

#[doc = pydoc!("finstack_quant.models.monte_carlo._results", "LrmGreeks")]
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
    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "LrmGreeks.price")]
    #[getter]
    fn price(&self) -> PyMoneyEstimate {
        PyMoneyEstimate::from_inner(self.inner.price.clone())
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "LrmGreeks.delta")]
    #[getter]
    fn delta(&self) -> PyEstimate {
        PyEstimate::from_inner(self.inner.delta.clone())
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "LrmGreeks.vega")]
    #[getter]
    fn vega(&self) -> PyEstimate {
        PyEstimate::from_inner(self.inner.vega.clone())
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "LrmGreeks.to_pandas")]
    #[pyo3(text_signature = "($self)")]
    fn to_pandas<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let bound = Bound::new(py, self.to_arrow_table()?)?;
        let frame = PyArrowTable::to_pandas(&bound, py)?;
        frame
            .call_method1("set_index", ("metric",))?
            .call_method1("rename_axis", (py.None(),))
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

impl PyLrmGreeks {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let greeks = &self.inner;
        let price = &greeks.price;
        TableBuilder::new()
            .utf8(
                "metric",
                vec!["price".to_string(), "delta".to_string(), "vega".to_string()],
            )?
            .f64(
                "mean",
                vec![price.mean.amount(), greeks.delta.mean, greeks.vega.mean],
            )?
            .f64(
                "stderr",
                vec![price.stderr, greeks.delta.stderr, greeks.vega.stderr],
            )?
            .f64(
                "ci_lower",
                vec![
                    price.ci_95.0.amount(),
                    greeks.delta.ci_95.0,
                    greeks.vega.ci_95.0,
                ],
            )?
            .f64(
                "ci_upper",
                vec![
                    price.ci_95.1.amount(),
                    greeks.delta.ci_95.1,
                    greeks.vega.ci_95.1,
                ],
            )?
            .build()
    }
}

tabular_methods!(
    PyLrmGreeks,
    "LrmGreeks",
    module = "finstack_quant.models.monte_carlo._results",
    custom_pandas
);
wire_methods!(
    PyLrmGreeks,
    LrmGreeks,
    "LrmGreeks",
    module = "finstack_quant.models.monte_carlo._results"
);

#[doc = pydoc!("finstack_quant.models.monte_carlo._results", "ExerciseBoundaryPoint")]
#[pyclass(
    name = "ExerciseBoundaryPoint",
    module = "finstack_quant.models.monte_carlo",
    frozen,
    skip_from_py_object
)]
pub(super) struct PyExerciseBoundaryPoint {
    inner: ExerciseBoundaryPoint,
}

#[pymethods]
impl PyExerciseBoundaryPoint {
    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "ExerciseBoundaryPoint.step")]
    #[getter]
    fn step(&self) -> usize {
        self.inner.step
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "ExerciseBoundaryPoint.time")]
    #[getter]
    fn time(&self) -> f64 {
        self.inner.time
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "ExerciseBoundaryPoint.critical_spot")]
    #[getter]
    fn critical_spot(&self) -> Option<f64> {
        self.inner.critical_spot
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "ExerciseBoundaryPoint.itm_paths")]
    #[getter]
    fn itm_paths(&self) -> usize {
        self.inner.itm_paths
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "ExerciseBoundaryPoint.exercised_paths")]
    #[getter]
    fn exercised_paths(&self) -> usize {
        self.inner.exercised_paths
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "ExerciseBoundaryPoint.continuation_coeffs")]
    #[getter]
    fn continuation_coeffs(&self) -> Vec<f64> {
        self.inner.continuation_coeffs.clone()
    }
}

#[doc = pydoc!("finstack_quant.models.monte_carlo._results", "LsmcResult")]
#[pyclass(
    name = "LsmcResult",
    module = "finstack_quant.models.monte_carlo",
    frozen,
    skip_from_py_object
)]
pub(super) struct PyLsmcResult {
    inner: LsmcResult,
}

impl PyLsmcResult {
    pub(super) fn from_inner(inner: LsmcResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyLsmcResult {
    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "LsmcResult.estimate")]
    #[getter]
    fn estimate(&self) -> PyMoneyEstimate {
        PyMoneyEstimate::from_inner(self.inner.estimate.clone())
    }

    #[doc = pydoc!("finstack_quant.models.monte_carlo._results", "LsmcResult.exercise_boundary")]
    #[getter]
    fn exercise_boundary(&self) -> Vec<PyExerciseBoundaryPoint> {
        self.inner
            .exercise_boundary
            .iter()
            .cloned()
            .map(|inner| PyExerciseBoundaryPoint { inner })
            .collect()
    }
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPathSummary>()?;
    m.add_class::<PyLrmGreeks>()?;
    m.add_class::<PyMoneyEstimate>()?;
    m.add_class::<PyExerciseBoundaryPoint>()?;
    m.add_class::<PyLsmcResult>()?;
    m.add_class::<PyEstimate>()?;
    Ok(())
}
