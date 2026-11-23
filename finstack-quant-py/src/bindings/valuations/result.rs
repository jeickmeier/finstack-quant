//! Python wrapper for `ValuationResult`.

use super::convert;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::pydoc::pydoc;
use crate::bindings::table_builders::serde_rows_to_arrow_table;
use crate::errors::display_to_py;
use pyo3::prelude::*;
use pyo3::types::PyDict;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations._result", "ValuationResult")]
#[pyclass(
    name = "ValuationResult",
    module = "finstack_quant.valuations",
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyValuationResult {
    pub(crate) inner: finstack_quant_valuations::results::ValuationResult,
}

impl PyValuationResult {
    fn wide_rows_arrow_table(
        rows: &[finstack_quant_valuations::results::ValuationRow],
    ) -> PyResult<PyArrowTable> {
        let identity: [(&str, &str); 4] = [
            ("instrument_id", "str"),
            ("as_of_date", "str"),
            ("pv", "float64"),
            ("currency", "str"),
        ];
        serde_rows_to_arrow_table(rows, &identity)
    }

    fn long_rows_arrow_table(
        rows: &[finstack_quant_valuations::results::ValuationLongRow],
    ) -> PyResult<PyArrowTable> {
        serde_rows_to_arrow_table(
            rows,
            &[
                ("metric", "str"),
                ("curve", "str"),
                ("bucket", "str"),
                ("value", "float64"),
            ],
        )
    }

    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        Self::wide_rows_arrow_table(&[self.inner.to_row()])
    }
}

crate::bindings::macros::tabular_methods!(
    PyValuationResult,
    "ValuationResult",
    module = "finstack_quant.valuations._result"
);

crate::bindings::macros::wire_methods!(
    PyValuationResult,
    finstack_quant_valuations::results::ValuationResult,
    "ValuationResult",
    module = "finstack_quant.valuations._result"
);

#[pymethods]
impl PyValuationResult {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations._result", "ValuationResult.as_of")]
    #[getter]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.as_of)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations._result", "ValuationResult.metrics")]
    #[getter]
    fn metrics<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (key, value) in &self.inner.measures {
            dict.set_item(key.as_str(), *value)?;
        }
        Ok(dict)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations._result", "ValuationResult.get_metric")]
    #[pyo3(text_signature = "($self, key)")]
    fn get_metric(&self, key: &str) -> Option<f64> {
        self.inner.metric_str(key)
    }

    /// ``result[key]``: scalar measure by key.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``key`` is not a measure of this result; the message lists the
    ///     five closest keys present.
    fn __getitem__(&self, key: &str) -> PyResult<f64> {
        self.inner.metric_str(key).ok_or_else(|| {
            let closest = self.inner.closest_metric_keys(key, 5);
            let hint = if closest.is_empty() {
                String::new()
            } else {
                format!("; closest keys: {}", closest.join(", "))
            };
            pyo3::exceptions::PyKeyError::new_err(format!(
                "metric '{key}' is not on this result ({} measures){hint}",
                self.inner.measures.len()
            ))
        })
    }

    /// ``key in result``: whether a measure with this key (canonical wire form) is present.
    fn __contains__(&self, key: &str) -> bool {
        self.inner.metric_str(key).is_some()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations._result", "ValuationResult.metric_series")]
    #[pyo3(text_signature = "($self, base)")]
    fn metric_series(&self, base: &str) -> PyResult<Vec<(Vec<String>, f64)>> {
        let base = base.parse().map_err(crate::errors::core_to_py)?;
        Ok(self.inner.metric_series(&base))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations._result", "ValuationResult.metric_series_table")]
    #[pyo3(text_signature = "($self, base)")]
    fn metric_series_table(&self, base: &str) -> PyResult<PyArrowTable> {
        Self::long_rows_arrow_table(&self.inner.to_series_rows(base))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations._result", "ValuationResult.to_long_table")]
    #[pyo3(text_signature = "($self)")]
    fn to_long_table(&self) -> PyResult<PyArrowTable> {
        Self::long_rows_arrow_table(&self.inner.to_long_rows())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations._result", "ValuationResult.metric_units")]
    #[pyo3(text_signature = "($self)")]
    fn metric_units<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (key, unit) in self.inner.metric_units() {
            dict.set_item(key, unit.as_str())?;
        }
        Ok(dict)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations._result", "ValuationResult.provenance")]
    #[getter]
    fn provenance<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .provenance
            .as_ref()
            .map(|provenance| serde_to_py(py, provenance))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations._result", "ValuationResult.explanation")]
    #[getter]
    fn explanation<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .explanation
            .as_ref()
            .map(|trace| serde_to_py(py, trace))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations._result", "ValuationResult.concat")]
    #[staticmethod]
    fn concat(results: Vec<PyRef<'_, Self>>) -> PyResult<PyArrowTable> {
        let rows: Vec<_> = results.iter().map(|result| result.inner.to_row()).collect();
        Self::wide_rows_arrow_table(&rows)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations._result", "ValuationResult.meta")]
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations._result", "ValuationResult.details")]
    #[getter]
    fn details<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .details
            .as_ref()
            .map(|details| serde_to_py(py, details))
            .transpose()
    }

    /// Structural equality: two results are equal when their JSON documents match.
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(other) = other.cast::<Self>() else {
            return Ok(false);
        };
        let lhs = serde_json::to_value(&self.inner).map_err(display_to_py)?;
        let rhs = serde_json::to_value(&other.borrow().inner).map_err(display_to_py)?;
        Ok(lhs == rhs)
    }

    fn __repr__(&self) -> String {
        format!(
            "ValuationResult(id={:?}, price={:.4}, currency={}, metrics={})",
            self.inner.instrument_id,
            self.inner.value.amount(),
            self.inner.value.currency(),
            self.inner.measures.len()
        )
    }
}

crate::bindings::macros::py_getters!(PyValuationResult |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations._result", "ValuationResult.instrument_id")]
    instrument_id -> &str = &s.inner.instrument_id;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations._result", "ValuationResult.schema_version")]
    schema_version -> u32 = s.inner.schema_version.into();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations._result", "ValuationResult.price")]
    get_price -> f64 = s.inner.value.amount();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations._result", "ValuationResult.value")]
    value -> crate::bindings::core::money::PyMoney = convert::money_to_py(s.inner.value);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations._result", "ValuationResult.currency")]
    currency -> String = s.inner.value.currency().to_string();
});
