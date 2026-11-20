//! A matched exchange of two currencies on distinct near and far settlement dates.

use crate::bindings::core::money::PyMoney;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::fx::fx_swap::{FxSwap, FxSwapBuilder};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwap")]
#[pyclass(
    name = "FxSwap",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFxSwap {
    pub(crate) inner: FxSwap,
}

instrument_envelope_methods!(
    PyFxSwap(
        "FxSwap",
        module = "finstack_quant.valuations.instruments._typed_fx_swap"
    ),
    FxSwap,
    builder = PyFxSwapBuilder("FxSwapBuilder", no_fields),
    seed = FxSwap::builder(),
);
instrument_pricing_methods!(
    PyFxSwap,
    "FxSwap",
    module = "finstack_quant.valuations.instruments._typed_fx_swap"
);

#[pymethods]
impl PyFxSwap {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwap.to_builder")]
    fn to_builder(&self) -> PyFxSwapBuilder {
        PyFxSwapBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwap.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        FxSwap::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwap.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwap.base_currency")]
    #[getter]
    fn base_currency(&self) -> String {
        self.inner.base_currency.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwap.quote_currency")]
    #[getter]
    fn quote_currency(&self) -> String {
        self.inner.quote_currency.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwap.near_date")]
    #[getter]
    fn near_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.near_date)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwap.far_date")]
    #[getter]
    fn far_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.far_date)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwap.notional")]
    #[getter]
    fn notional(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.notional)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwap.domestic_discount_curve_id")]
    #[getter]
    fn domestic_discount_curve_id(&self) -> String {
        self.inner.domestic_discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwap.foreign_discount_curve_id")]
    #[getter]
    fn foreign_discount_curve_id(&self) -> String {
        self.inner.foreign_discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwap.near_rate")]
    #[getter]
    fn near_rate(&self) -> Option<f64> {
        self.inner.near_rate
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwap.far_rate")]
    #[getter]
    fn far_rate(&self) -> Option<f64> {
        self.inner.far_rate
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwap.base_calendar_id")]
    #[getter]
    fn base_calendar_id(&self) -> Option<String> {
        self.inner
            .base_calendar_id
            .as_ref()
            .map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwap.quote_calendar_id")]
    #[getter]
    fn quote_calendar_id(&self) -> Option<String> {
        self.inner
            .quote_calendar_id
            .as_ref()
            .map(ToString::to_string)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwapBuilder")]
#[pyclass(
    name = "FxSwapBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyFxSwapBuilder {
    inner: Option<FxSwapBuilder>,
}

#[pymethods]
impl PyFxSwapBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwapBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: FxSwapBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwapBuilder.base_currency")]
    fn base_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: FxSwapBuilder| b.base_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwapBuilder.quote_currency")]
    fn quote_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: FxSwapBuilder| b.quote_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwapBuilder.near_date")]
    fn near_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: FxSwapBuilder| b.near_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwapBuilder.far_date")]
    fn far_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: FxSwapBuilder| b.far_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwapBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: FxSwapBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwapBuilder.domestic_discount_curve_id")]
    fn domestic_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: FxSwapBuilder| b.domestic_discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwapBuilder.foreign_discount_curve_id")]
    fn foreign_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: FxSwapBuilder| b.foreign_discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwapBuilder.near_rate")]
    fn near_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: FxSwapBuilder| b.near_rate_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwapBuilder.far_rate")]
    fn far_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: FxSwapBuilder| b.far_rate_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwapBuilder.base_calendar_id")]
    fn base_calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: FxSwapBuilder| b.base_calendar_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwapBuilder.quote_calendar_id")]
    fn quote_calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: FxSwapBuilder| b.quote_calendar_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_swap", "FxSwapBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: FxSwapBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyFxSwap>()?;
    module.add_class::<PyFxSwapBuilder>()?;
    Ok(())
}
