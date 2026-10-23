//! Typed priority-of-payments introspection (`Waterfall`).

use pyo3::prelude::*;

use crate::bindings::pandas_utils::serde_to_py;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::Waterfall;

use super::PyCoverageRules;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Waterfall")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "Waterfall",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyWaterfall {
    /// Inner canonical Rust waterfall.
    pub(crate) inner: Waterfall,
}

crate::bindings::macros::wire_methods!(
    PyWaterfall,
    Waterfall,
    "Waterfall",
    module = "finstack_quant.valuations.instruments",
    to_dict
);

#[pymethods]
impl PyWaterfall {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Waterfall.tiers")]
    #[getter]
    fn tiers<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.tiers)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Waterfall.coverage_tests")]
    #[pyo3(text_signature = "($self)")]
    fn coverage_tests<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let specs: Vec<_> = self.inner.coverage_tests().cloned().collect();
        serde_to_py(py, &specs)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "Waterfall(currency={}, tiers={})",
            self.inner.currency,
            self.inner.tiers.len()
        )
    }
}

crate::bindings::macros::py_getters!(PyWaterfall |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Waterfall.currency")]
    currency -> String = s.inner.currency.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "Waterfall.coverage_rules")]
    coverage_rules -> Option<PyCoverageRules> =
        s.inner.coverage_rules.clone().map(|inner| PyCoverageRules { inner });
});
