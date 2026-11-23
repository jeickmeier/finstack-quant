//! Simulated structural Monte Carlo path Python wrapper.

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::bindings::table_builders::TableBuilder;
use finstack_quant_models::credit::SimulatedPaths;
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.models.credit._paths", "SimulatedPaths")]
#[pyclass(
    name = "SimulatedPaths",
    module = "finstack_quant.models.credit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PySimulatedPaths {
    pub(crate) inner: SimulatedPaths,
}

impl PySimulatedPaths {
    pub(crate) fn from_inner(inner: SimulatedPaths) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PySimulatedPaths {
    #[doc = pydoc!("finstack_quant.models.credit._paths", "SimulatedPaths.to_numpy")]
    fn to_numpy<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, numpy::PyArray2<f64>>> {
        use numpy::{PyArray1, PyArrayMethods};
        PyArray1::from_vec(py, self.inner.asset_values.clone())
            .reshape([self.inner.num_paths, self.inner.values_per_path()])
    }

    #[doc = pydoc!("finstack_quant.models.credit._paths", "SimulatedPaths.terminal_values")]
    fn terminal_values<'py>(&self, py: Python<'py>) -> Bound<'py, numpy::PyArray1<f64>> {
        let values = self
            .inner
            .asset_values
            .chunks_exact(self.inner.values_per_path())
            .filter_map(|row| row.last().copied())
            .collect();
        numpy::PyArray1::from_vec(py, values)
    }

    #[doc = pydoc!("finstack_quant.models.credit._paths", "SimulatedPaths.get")]
    fn get(&self, path_idx: usize, time_idx: usize) -> Option<f64> {
        self.inner.get(path_idx, time_idx)
    }

    #[doc = pydoc!("finstack_quant.models.credit._paths", "SimulatedPaths.path")]
    fn path(&self, path_idx: usize) -> Option<Vec<f64>> {
        self.inner.path(path_idx).map(<[f64]>::to_vec)
    }

    #[doc = pydoc!("finstack_quant.models.credit._paths", "SimulatedPaths.to_nested")]
    fn to_nested(&self) -> Vec<Vec<f64>> {
        self.inner.to_nested()
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("SimulatedPaths", &self.inner)
    }
}

py_getters!(PySimulatedPaths |s| {
    #[doc = pydoc!("finstack_quant.models.credit._paths", "SimulatedPaths.times")]
    times -> Vec<f64> = s.inner.times.clone();
    #[doc = pydoc!("finstack_quant.models.credit._paths", "SimulatedPaths.asset_values")]
    asset_values -> Vec<f64> = s.inner.asset_values.clone();
    #[doc = pydoc!("finstack_quant.models.credit._paths", "SimulatedPaths.num_paths")]
    num_paths -> usize = s.inner.num_paths;
    #[doc = pydoc!("finstack_quant.models.credit._paths", "SimulatedPaths.num_steps")]
    num_steps -> usize = s.inner.num_steps;
    #[doc = pydoc!("finstack_quant.models.credit._paths", "SimulatedPaths.values_per_path")]
    values_per_path -> usize = s.inner.values_per_path();
});

impl PySimulatedPaths {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let per_path = self.inner.values_per_path();
        let n = self.inner.num_paths * per_path;
        let mut path_col = Vec::with_capacity(n);
        let mut time_col = Vec::with_capacity(n);
        for p in 0..self.inner.num_paths {
            let p_i64 = i64::try_from(p)
                .map_err(|e| crate::errors::value_error(format!("path index overflow: {e}")))?;
            for &time in &self.inner.times {
                path_col.push(p_i64);
                time_col.push(time);
            }
        }
        TableBuilder::new()
            .i64("path", path_col)?
            .f64("time", time_col)?
            .f64("asset_value", self.inner.asset_values.clone())?
            .build()
    }
}

tabular_methods!(
    PySimulatedPaths,
    "SimulatedPaths",
    module = "finstack_quant.models.credit._paths"
);
wire_methods!(
    PySimulatedPaths,
    SimulatedPaths,
    "SimulatedPaths",
    module = "finstack_quant.models.credit._paths"
);
