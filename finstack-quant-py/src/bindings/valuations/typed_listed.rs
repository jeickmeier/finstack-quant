//! Shared listed contract terms for typed futures builders.

use crate::bindings::core::currency::extract_currency;
use crate::bindings::date_utils::py_to_date;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::{core_to_py, value_error};
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
}

crate::bindings::macros::wire_methods!(
    PyListedFutureTerms,
    ListedFutureTerms,
    "ListedFutureTerms",
    module = "finstack_quant.valuations.instruments._typed_listed",
    validate
);

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_listed", "FutureOptionTerms")]
#[pyclass(
    name = "FutureOptionTerms",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyFutureOptionTerms {
    pub(crate) inner: finstack_quant_valuations::instruments::FutureOptionTerms,
}

#[pymethods]
impl PyFutureOptionTerms {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_listed", "FutureOptionTerms.__init__")]
    #[new]
    #[pyo3(signature = (**fields))]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, pyo3::types::PyDict>>) -> PyResult<Self> {
        let inner: finstack_quant_valuations::instruments::FutureOptionTerms =
            crate::bindings::module_utils::fields_from_py(py, fields, "futures option terms")?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_listed", "FutureOptionTerms.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::FutureOptionTerms::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_listed", "FutureOptionTerms.to_dict")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner)
    }
}
