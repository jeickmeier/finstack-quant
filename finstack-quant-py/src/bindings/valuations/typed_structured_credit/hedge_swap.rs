//! Typed interest-rate hedge settled through the waterfall (`HedgeSwap`).

use pyo3::prelude::*;

use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::valuations::convert::enum_to_py_string;
use crate::bindings::valuations::typed_rates::PyInterestRateSwap;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    HedgeSwap, SwapNotional, SwapPriority,
};

use super::super::convert::enum_from_str;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.hedge_swap", "HedgeSwap")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "HedgeSwap",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyHedgeSwap {
    /// Inner canonical Rust hedge.
    pub(crate) inner: HedgeSwap,
}

crate::bindings::macros::wire_methods!(
    PyHedgeSwap,
    HedgeSwap,
    "HedgeSwap",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.hedge_swap",
    to_dict
);

#[pymethods]
impl PyHedgeSwap {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.hedge_swap", "HedgeSwap.__init__")]
    #[new]
    #[pyo3(signature = (swap, notional=None, priority=None))]
    #[pyo3(text_signature = "(swap, notional=None, priority=None)")]
    fn new(
        py: Python<'_>,
        swap: PyRef<'_, PyInterestRateSwap>,
        notional: Option<&Bound<'_, PyAny>>,
        priority: Option<&str>,
    ) -> PyResult<Self> {
        let notional: SwapNotional = match notional {
            Some(value) => crate::bindings::module_utils::py_to_serde(py, value, "notional")?,
            None => SwapNotional::Contractual,
        };
        let priority: SwapPriority = match priority {
            Some(value) => enum_from_str(value, "priority")?,
            None => SwapPriority::SeniorFee,
        };
        Ok(Self {
            inner: HedgeSwap {
                swap: swap.inner.clone(),
                notional,
                priority,
            },
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.hedge_swap", "HedgeSwap.notional")]
    #[getter]
    fn notional<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.notional)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "HedgeSwap(swap={:?}, notional={}, priority={})",
            self.inner.swap.id.as_str(),
            serde_json::to_string(&self.inner.notional).unwrap_or_default(),
            enum_to_py_string(&self.inner.priority).unwrap_or_default()
        )
    }
}

crate::bindings::macros::py_getters!(PyHedgeSwap |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.hedge_swap", "HedgeSwap.swap")]
    swap -> PyInterestRateSwap = PyInterestRateSwap { inner: s.inner.swap.clone() };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.hedge_swap", "HedgeSwap.priority")]
    priority -> PyResult<String> = enum_to_py_string(&s.inner.priority);
});
