//! A basket aggregating the prices of signed constituents.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::exotics::basket::{Basket, BasketBuilder};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_basket", "Basket")]
#[pyclass(
    name = "Basket",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyBasket {
    pub(crate) inner: Basket,
}

instrument_envelope_methods!(
    PyBasket(
        "Basket",
        module = "finstack_quant.valuations.instruments._typed_basket"
    ),
    Basket,
    builder = PyBasketBuilder("BasketBuilder", no_fields),
    seed = Basket::builder(),
);
instrument_pricing_methods!(
    PyBasket,
    "Basket",
    module = "finstack_quant.valuations.instruments._typed_basket"
);

#[pymethods]
impl PyBasket {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basket", "Basket.to_builder")]
    fn to_builder(&self) -> PyBasketBuilder {
        PyBasketBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basket", "Basket.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        Basket::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basket", "Basket.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basket", "Basket.constituents")]
    #[getter]
    fn constituents<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.constituents)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basket", "Basket.expense_ratio")]
    #[getter]
    fn expense_ratio(&self) -> f64 {
        self.inner.expense_ratio
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basket", "Basket.reporting_currency")]
    #[getter]
    fn reporting_currency(&self) -> String {
        self.inner.reporting_currency.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basket", "Basket.notional")]
    #[getter]
    fn notional(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.notional)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basket", "Basket.discount_curve_id")]
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basket", "Basket.pricing_config")]
    #[getter]
    fn pricing_config<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.pricing_config)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_basket", "BasketBuilder")]
#[pyclass(
    name = "BasketBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyBasketBuilder {
    inner: Option<BasketBuilder>,
}

#[pymethods]
impl PyBasketBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basket", "BasketBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: BasketBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basket", "BasketBuilder.constituents")]
    fn constituents<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "constituents")?;
        builder_set!(slf, |b: BasketBuilder| b.constituents(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basket", "BasketBuilder.expense_ratio")]
    fn expense_ratio<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: BasketBuilder| b.expense_ratio(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basket", "BasketBuilder.reporting_currency")]
    fn reporting_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: BasketBuilder| b.reporting_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basket", "BasketBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: BasketBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basket", "BasketBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: BasketBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basket", "BasketBuilder.pricing_config")]
    fn pricing_config<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "pricing_config")?;
        builder_set!(slf, |b: BasketBuilder| b.pricing_config(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_basket", "BasketBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: BasketBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyBasket>()?;
    module.add_class::<PyBasketBuilder>()?;
    Ok(())
}
