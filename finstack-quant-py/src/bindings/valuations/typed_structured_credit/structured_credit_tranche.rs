//! Typed per-note pricing without changing the originating waterfall.

use super::PyStructuredCredit;
use crate::bindings::pickle_support::reduce_via_json;
use crate::bindings::pydoc::pydoc;
use crate::bindings::valuations::convert::{
    parse_typed_instrument_json, serialize_typed_instrument_json,
};
use crate::bindings::valuations::typed_macros::instrument_trait_methods;
use crate::errors::core_to_py;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCreditTranche;
use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit_tranche", "StructuredCreditTranche")]
#[pyclass(
    name = "StructuredCreditTranche",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyStructuredCreditTranche {
    pub(crate) inner: StructuredCreditTranche,
}

instrument_trait_methods!(
    PyStructuredCreditTranche,
    "StructuredCreditTranche",
    module =
        "finstack_quant.valuations.instruments._typed_structured_credit.structured_credit_tranche"
);

#[pymethods]
impl PyStructuredCreditTranche {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit_tranche", "StructuredCreditTranche.from_deal")]
    #[staticmethod]
    fn from_deal(deal: &PyStructuredCredit, tranche_id: &str) -> PyResult<Self> {
        StructuredCreditTranche::from_deal(deal.inner.clone(), tranche_id)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit_tranche", "StructuredCreditTranche.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        StructuredCreditTranche::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit_tranche", "StructuredCreditTranche.id")]
    #[getter]
    fn id(&self) -> &str {
        self.inner.id()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit_tranche", "StructuredCreditTranche.tranche_id")]
    #[getter]
    fn tranche_id(&self) -> &str {
        self.inner.tranche_id()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit_tranche", "StructuredCreditTranche.from_json")]
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        parse_typed_instrument_json(json).map(|inner| Self { inner })
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit_tranche", "StructuredCreditTranche.to_json")]
    fn to_json(&self) -> PyResult<String> {
        serialize_typed_instrument_json(
            InstrumentJson::from(self.inner.clone()),
            "StructuredCreditTranche",
        )
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.structured_credit_tranche", "StructuredCreditTranche.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }
}
