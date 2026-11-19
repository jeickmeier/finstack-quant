//! Shared listed contract terms for typed futures builders.

use crate::bindings::core::currency::extract_currency;
use crate::bindings::date_utils::py_to_date;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::{core_to_py, serde_json_to_py, value_error};
use finstack_quant_valuations::instruments::ListedFutureTerms;
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_listed", "ListedFutureTerms")]
#[pyclass(
    name = "ListedFutureTerms",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyListedFutureTerms {
    pub(crate) inner: ListedFutureTerms,
}

#[pymethods]
impl PyListedFutureTerms {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_listed", "ListedFutureTerms.__init__")]
    #[new]
    fn new(
        contracts: f64,
        multiplier: f64,
        currency: &Bound<'_, PyAny>,
        entry_price: f64,
        last_trading_date: &Bound<'_, PyAny>,
        settlement_date: &Bound<'_, PyAny>,
        position: &str,
    ) -> PyResult<Self> {
        ListedFutureTerms::new(
            contracts,
            multiplier,
            extract_currency(currency)?,
            entry_price,
            py_to_date(last_trading_date)?,
            py_to_date(settlement_date)?,
            position.parse().map_err(value_error)?,
        )
        .map(|inner| Self { inner })
        .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_listed", "ListedFutureTerms.with_contracts")]
    fn with_contracts(&self, contracts: f64) -> PyResult<Self> {
        self.inner
            .with_contracts(contracts)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_listed", "ListedFutureTerms.with_marks")]
    fn with_marks(
        &self,
        quoted_price: Option<f64>,
        settlement_price: Option<f64>,
    ) -> PyResult<Self> {
        self.inner
            .with_marks(quoted_price, settlement_price)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_listed", "ListedFutureTerms.with_settlement")]
    fn with_settlement(&self, settlement: &Bound<'_, PyAny>) -> PyResult<Self> {
        let settlement = py_to_serde(settlement.py(), settlement, "settlement")?;
        self.inner
            .with_settlement(settlement)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_listed", "ListedFutureTerms.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_listed", "ListedFutureTerms.to_json")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "listed future terms"))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_listed", "ListedFutureTerms.from_json")]
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: ListedFutureTerms =
            serde_json::from_str(json).map_err(|e| serde_json_to_py(e, "listed future terms"))?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }
}
