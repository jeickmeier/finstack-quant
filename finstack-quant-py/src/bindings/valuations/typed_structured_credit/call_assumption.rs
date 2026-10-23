//! Typed optional-redemption assumption (`CallAssumption`).

use pyo3::prelude::*;

use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::pandas_utils::serde_to_py;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    CallAssumption, CallScope,
};

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CallAssumption")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CallAssumption",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCallAssumption {
    /// Inner canonical Rust call assumption.
    pub(crate) inner: CallAssumption,
}

crate::bindings::macros::wire_methods!(
    PyCallAssumption,
    CallAssumption,
    "CallAssumption",
    module = "finstack_quant.valuations.instruments",
    to_dict
);

#[pymethods]
impl PyCallAssumption {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CallAssumption.__init__")]
    #[new]
    #[pyo3(signature = (date, price_pct, tranche_id=None))]
    #[pyo3(text_signature = "(date, price_pct, tranche_id=None)")]
    fn new(date: &Bound<'_, PyAny>, price_pct: f64, tranche_id: Option<&str>) -> PyResult<Self> {
        let date = py_to_date(date)?;
        let inner = match tranche_id {
            Some(id) => CallAssumption::for_tranche(date, price_pct, id),
            None => CallAssumption::new(date, price_pct),
        };
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CallAssumption.date")]
    #[getter]
    fn date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.date)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CallAssumption.scope_spec")]
    #[getter]
    fn scope_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.scope)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CallAssumption(date={}, price_pct={}, scope={:?}, tranche_id={})",
            self.inner.date,
            self.inner.price_pct,
            self.scope(),
            crate::bindings::valuations::convert::opt_repr(self.tranche_id())
        )
    }
}

crate::bindings::macros::py_getters!(PyCallAssumption |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CallAssumption.price_pct")]
    price_pct -> f64 = s.inner.price_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CallAssumption.scope")]
    scope -> &'static str =
        match s.inner.scope { CallScope::Deal => "deal", CallScope::Tranche(_) => "tranche" };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "CallAssumption.tranche_id")]
    tranche_id -> Option<String> = match &s.inner.scope {
        CallScope::Deal => None,
        CallScope::Tranche(id) => Some(id.clone()),
    };
});
