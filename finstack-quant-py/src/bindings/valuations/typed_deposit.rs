//! Typed bindings for money-market deposits.

use crate::bindings::core::dates::daycount::{PyDayCount, extract_day_count};
use crate::bindings::core::money::{PyMoney, decimal_from_py, decimal_to_py};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::{CalendarId, CurveId, InstrumentId};
use finstack_quant_valuations::instruments::rates::deposit::{Deposit, DepositBuilder};
use pyo3::prelude::*;

use super::convert::{
    attributes_from_py, enum_from_str, enum_to_py_string, instrument_expiry, money_from_py,
};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_trait_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "Deposit")]
#[pyclass(
    name = "Deposit",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyDeposit {
    pub(crate) inner: Deposit,
}

instrument_envelope_methods!(
    PyDeposit(
        "Deposit",
        module = "finstack_quant.valuations.instruments._typed_deposit"
    ),
    Deposit,
    builder = PyDepositBuilder("DepositBuilder", no_fields),
    seed = Deposit::builder(),
);
instrument_trait_methods!(
    PyDeposit,
    "Deposit",
    module = "finstack_quant.valuations.instruments._typed_deposit"
);

#[pymethods]
impl PyDeposit {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "Deposit.to_builder")]
    fn to_builder(&self) -> PyDepositBuilder {
        PyDepositBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "Deposit.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        Deposit::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "Deposit.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "Deposit.start_date")]
    #[getter]
    fn start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start_date)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "Deposit.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "Deposit.fixed_rate")]
    #[getter]
    fn fixed_rate<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .fixed_rate
            .map(|rate| decimal_to_py(py, rate))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "Deposit.business_day_convention")]
    #[getter]
    fn business_day_convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.business_day_convention)
    }
}

crate::bindings::macros::py_getters!(PyDeposit |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "Deposit.day_count")]
    day_count -> PyDayCount = PyDayCount::from_inner(s.inner.day_count);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "Deposit.calendar_id")]
    calendar_id -> Option<String> = s.inner.calendar_id.as_ref().map(ToString::to_string);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "Deposit.notional")]
    notional -> PyMoney = PyMoney::from_inner(s.inner.notional);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "Deposit.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "DepositBuilder")]
#[pyclass(
    name = "DepositBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyDepositBuilder {
    inner: Option<DepositBuilder>,
}

#[pymethods]
impl PyDepositBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "DepositBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: DepositBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "DepositBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = money_from_py(value, "notional")?;
        builder_set!(slf, |b: DepositBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "DepositBuilder.start_date")]
    fn start_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_date(value)?;
        builder_set!(slf, |b: DepositBuilder| b.start_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "DepositBuilder.maturity")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_date(value)?;
        builder_set!(slf, |b: DepositBuilder| b.maturity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "DepositBuilder.fixed_rate")]
    fn fixed_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = decimal_from_py(value)?;
        builder_set!(slf, |b: DepositBuilder| b.fixed_rate(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "DepositBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = extract_day_count(value)?;
        builder_set!(slf, |b: DepositBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "DepositBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = CurveId::new(value);
        builder_set!(slf, |b: DepositBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "DepositBuilder.business_day_convention")]
    fn business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = enum_from_str(value, "business_day_convention")?;
        builder_set!(slf, |b: DepositBuilder| b.business_day_convention(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "DepositBuilder.calendar_id")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = CalendarId::new(value);
        builder_set!(slf, |b: DepositBuilder| b.calendar_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_deposit", "DepositBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: DepositBuilder| b.attributes(value))
    }
}

/// Register the deposit instrument and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyDeposit>()?;
    module.add_class::<PyDepositBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["Deposit", "DepositBuilder"];
