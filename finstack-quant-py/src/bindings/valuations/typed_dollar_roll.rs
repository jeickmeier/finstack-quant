//! A paired purchase and sale of agency TBA contracts on distinct value dates.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::fixed_income::dollar_roll::{
    DollarRoll, DollarRollBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll")]
#[pyclass(
    name = "DollarRoll",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyDollarRoll {
    pub(crate) inner: DollarRoll,
}

instrument_envelope_methods!(
    PyDollarRoll(
        "DollarRoll",
        module = "finstack_quant.valuations.instruments._typed_dollar_roll"
    ),
    DollarRoll,
    builder = PyDollarRollBuilder("DollarRollBuilder", no_fields),
    seed = DollarRoll::builder(),
);
instrument_pricing_methods!(
    PyDollarRoll,
    "DollarRoll",
    module = "finstack_quant.valuations.instruments._typed_dollar_roll"
);

#[pymethods]
impl PyDollarRoll {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.to_builder")]
    fn to_builder(&self) -> PyDollarRollBuilder {
        PyDollarRollBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        DollarRoll::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.agency")]
    #[getter]
    fn agency<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.agency)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.coupon")]
    #[getter]
    fn coupon(&self) -> f64 {
        self.inner.coupon
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.term")]
    #[getter]
    fn term<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.term)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.notional")]
    #[getter]
    fn notional(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.notional)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.front_settlement_year")]
    #[getter]
    fn front_settlement_year(&self) -> i32 {
        self.inner.front_settlement_year
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.front_settlement_month")]
    #[getter]
    fn front_settlement_month(&self) -> u8 {
        self.inner.front_settlement_month
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.back_settlement_year")]
    #[getter]
    fn back_settlement_year(&self) -> i32 {
        self.inner.back_settlement_year
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.back_settlement_month")]
    #[getter]
    fn back_settlement_month(&self) -> u8 {
        self.inner.back_settlement_month
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.settlement_class")]
    #[getter]
    fn settlement_class<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.settlement_class)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.front_settlement_date")]
    #[getter]
    fn front_settlement_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .front_settlement_date
            .map(|value| crate::bindings::date_utils::date_to_py(py, value))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.back_settlement_date")]
    #[getter]
    fn back_settlement_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .back_settlement_date
            .map(|value| crate::bindings::date_utils::date_to_py(py, value))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.front_price")]
    #[getter]
    fn front_price(&self) -> f64 {
        self.inner.front_price
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.back_price")]
    #[getter]
    fn back_price(&self) -> f64 {
        self.inner.back_price
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.trade_date")]
    #[getter]
    fn trade_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .trade_date
            .map(|value| crate::bindings::date_utils::date_to_py(py, value))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.prepayment_spec")]
    #[getter]
    fn prepayment_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.prepayment_spec)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.discount_curve_id")]
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRoll.repo_curve_id")]
    #[getter]
    fn repo_curve_id(&self) -> Option<String> {
        self.inner.repo_curve_id.as_ref().map(ToString::to_string)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder")]
#[pyclass(
    name = "DollarRollBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyDollarRollBuilder {
    inner: Option<DollarRollBuilder>,
}

#[pymethods]
impl PyDollarRollBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: DollarRollBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder.agency")]
    fn agency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "agency")?;
        builder_set!(slf, |b: DollarRollBuilder| b.agency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder.coupon")]
    fn coupon<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: DollarRollBuilder| b.coupon(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder.term")]
    fn term<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "term")?;
        builder_set!(slf, |b: DollarRollBuilder| b.term(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: DollarRollBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder.front_settlement_year")]
    fn front_settlement_year<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: i32,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: DollarRollBuilder| b.front_settlement_year(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder.front_settlement_month")]
    fn front_settlement_month<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: u8,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: DollarRollBuilder| b.front_settlement_month(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder.back_settlement_year")]
    fn back_settlement_year<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: i32,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: DollarRollBuilder| b.back_settlement_year(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder.back_settlement_month")]
    fn back_settlement_month<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: u8,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: DollarRollBuilder| b.back_settlement_month(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder.settlement_class")]
    fn settlement_class<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "settlement_class")?;
        builder_set!(slf, |b: DollarRollBuilder| b.settlement_class(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder.front_settlement_date")]
    fn front_settlement_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(crate::bindings::date_utils::py_to_date)
            .transpose()?;
        builder_set!(slf, |b: DollarRollBuilder| b
            .front_settlement_date_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder.back_settlement_date")]
    fn back_settlement_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(crate::bindings::date_utils::py_to_date)
            .transpose()?;
        builder_set!(slf, |b: DollarRollBuilder| b
            .back_settlement_date_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder.front_price")]
    fn front_price<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: DollarRollBuilder| b.front_price(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder.back_price")]
    fn back_price<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: DollarRollBuilder| b.back_price(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder.trade_date")]
    fn trade_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(crate::bindings::date_utils::py_to_date)
            .transpose()?;
        builder_set!(slf, |b: DollarRollBuilder| b.trade_date_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder.prepayment_spec")]
    fn prepayment_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "prepayment_spec")?;
        builder_set!(slf, |b: DollarRollBuilder| b.prepayment_spec_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: DollarRollBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder.repo_curve_id")]
    fn repo_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: DollarRollBuilder| b.repo_curve_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_dollar_roll", "DollarRollBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: DollarRollBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyDollarRoll>()?;
    module.add_class::<PyDollarRollBuilder>()?;
    Ok(())
}
