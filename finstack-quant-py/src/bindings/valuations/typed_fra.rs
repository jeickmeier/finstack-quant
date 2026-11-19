//! Typed bindings for forward rate agreements.

use crate::bindings::core::dates::daycount::{PyDayCount, extract_day_count};
use crate::bindings::core::money::{PyMoney, decimal_from_py, decimal_to_py};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::{CalendarId, CurveId, InstrumentId};
use finstack_quant_valuations::instruments::rates::fra::{
    ForwardRateAgreement, ForwardRateAgreementBuilder,
};
use pyo3::prelude::*;

use super::convert::{
    attributes_from_py, enum_from_str, enum_to_py_string, instrument_expiry, money_from_py,
};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreement")]
#[pyclass(
    name = "ForwardRateAgreement",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyForwardRateAgreement {
    pub(crate) inner: ForwardRateAgreement,
}

instrument_envelope_methods!(
    PyForwardRateAgreement(
        "ForwardRateAgreement",
        module = "finstack_quant.valuations.instruments._typed_fra"
    ),
    ForwardRateAgreement,
    builder = PyForwardRateAgreementBuilder("ForwardRateAgreementBuilder", no_fields),
    seed = ForwardRateAgreement::builder(),
);
instrument_pricing_methods!(
    PyForwardRateAgreement,
    "ForwardRateAgreement",
    module = "finstack_quant.valuations.instruments._typed_fra"
);

#[pymethods]
impl PyForwardRateAgreement {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreement.to_builder")]
    fn to_builder(&self) -> PyForwardRateAgreementBuilder {
        PyForwardRateAgreementBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreement.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        ForwardRateAgreement::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreement.fixing_date")]
    #[getter]
    fn fixing_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .fixing_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreement.reset_lag_days")]
    #[getter]
    fn reset_lag_days(&self) -> i32 {
        self.inner.reset_lag_days
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreement.observed_fixing")]
    #[getter]
    fn observed_fixing(&self) -> Option<f64> {
        self.inner.observed_fixing
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreement.forward_curve_id")]
    #[getter]
    fn forward_curve_id(&self) -> &str {
        self.inner.forward_curve_id.as_str()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreement.side")]
    #[getter]
    fn side(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.side)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreement.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreement.start_date")]
    #[getter]
    fn start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start_date)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreement.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreement.fixed_rate")]
    #[getter]
    fn fixed_rate<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        decimal_to_py(py, self.inner.fixed_rate)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreement.day_count")]
    #[getter]
    fn day_count(&self) -> PyDayCount {
        PyDayCount::from_inner(self.inner.day_count)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreement.fixing_business_day_convention")]
    #[getter]
    fn fixing_business_day_convention(&self) -> PyResult<Option<String>> {
        self.inner
            .fixing_business_day_convention
            .as_ref()
            .map(enum_to_py_string)
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreement.fixing_calendar_id")]
    #[getter]
    fn fixing_calendar_id(&self) -> Option<&str> {
        self.inner
            .fixing_calendar_id
            .as_ref()
            .map(CalendarId::as_str)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreement.notional")]
    #[getter]
    fn notional(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.notional)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreement.discount_curve_id")]
    #[getter]
    fn discount_curve_id(&self) -> &str {
        self.inner.discount_curve_id.as_str()
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreementBuilder")]
#[pyclass(
    name = "ForwardRateAgreementBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyForwardRateAgreementBuilder {
    inner: Option<ForwardRateAgreementBuilder>,
}

#[pymethods]
impl PyForwardRateAgreementBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreementBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: ForwardRateAgreementBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreementBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = money_from_py(value, "notional")?;
        builder_set!(slf, |b: ForwardRateAgreementBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreementBuilder.start_date")]
    fn start_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_date(value)?;
        builder_set!(slf, |b: ForwardRateAgreementBuilder| b.start_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreementBuilder.maturity")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_date(value)?;
        builder_set!(slf, |b: ForwardRateAgreementBuilder| b.maturity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreementBuilder.fixed_rate")]
    fn fixed_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = decimal_from_py(value)?;
        builder_set!(slf, |b: ForwardRateAgreementBuilder| b.fixed_rate(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreementBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = extract_day_count(value)?;
        builder_set!(slf, |b: ForwardRateAgreementBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreementBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = CurveId::new(value);
        builder_set!(slf, |b: ForwardRateAgreementBuilder| b
            .discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreementBuilder.fixing_business_day_convention")]
    fn fixing_business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = enum_from_str(value, "fixing_business_day_convention")?;
        builder_set!(slf, |b: ForwardRateAgreementBuilder| b
            .fixing_business_day_convention(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreementBuilder.fixing_calendar_id")]
    fn fixing_calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = CalendarId::new(value);
        builder_set!(slf, |b: ForwardRateAgreementBuilder| b
            .fixing_calendar_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreementBuilder.fixing_date")]
    fn fixing_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_date(value)?;
        builder_set!(slf, |b: ForwardRateAgreementBuilder| b.fixing_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreementBuilder.reset_lag_days")]
    fn reset_lag_days<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: i32,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: ForwardRateAgreementBuilder| b
            .reset_lag_days(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreementBuilder.observed_fixing")]
    fn observed_fixing<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: ForwardRateAgreementBuilder| b
            .observed_fixing(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreementBuilder.forward_curve_id")]
    fn forward_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = CurveId::new(value);
        builder_set!(slf, |b: ForwardRateAgreementBuilder| b
            .forward_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreementBuilder.side")]
    fn side<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = enum_from_str(value, "side")?;
        builder_set!(slf, |b: ForwardRateAgreementBuilder| b.side(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fra", "ForwardRateAgreementBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: ForwardRateAgreementBuilder| b.attributes(value))
    }
}

/// Register the forward rate agreement and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyForwardRateAgreement>()?;
    module.add_class::<PyForwardRateAgreementBuilder>()?;
    Ok(())
}
