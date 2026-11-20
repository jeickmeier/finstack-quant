//! A spot FX receipt with optional domestic discounting.

use crate::bindings::core::money::PyMoney;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::fx::fx_spot::{FxSpot, FxSpotBuilder};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpot")]
#[pyclass(
    name = "FxSpot",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFxSpot {
    pub(crate) inner: FxSpot,
}

instrument_envelope_methods!(
    PyFxSpot(
        "FxSpot",
        module = "finstack_quant.valuations.instruments._typed_fx_spot"
    ),
    FxSpot,
    builder = PyFxSpotBuilder("FxSpotBuilder", no_fields),
    seed = FxSpot::builder(),
);
instrument_pricing_methods!(
    PyFxSpot,
    "FxSpot",
    module = "finstack_quant.valuations.instruments._typed_fx_spot"
);

#[pymethods]
impl PyFxSpot {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpot.to_builder")]
    fn to_builder(&self) -> PyFxSpotBuilder {
        PyFxSpotBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpot.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        FxSpot::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpot.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpot.base_currency")]
    #[getter]
    fn base_currency(&self) -> String {
        self.inner.base_currency.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpot.quote_currency")]
    #[getter]
    fn quote_currency(&self) -> String {
        self.inner.quote_currency.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpot.settlement_date")]
    #[getter]
    fn settlement_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .settlement_date
            .map(|value| crate::bindings::date_utils::date_to_py(py, value))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpot.settlement_days")]
    #[getter]
    fn settlement_days(&self) -> Option<u32> {
        self.inner.settlement_days
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpot.quoted_spot")]
    #[getter]
    fn quoted_spot(&self) -> Option<f64> {
        self.inner.quoted_spot
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpot.domestic_discount_curve_id")]
    #[getter]
    fn domestic_discount_curve_id(&self) -> Option<String> {
        self.inner
            .domestic_discount_curve_id
            .as_ref()
            .map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpot.notional")]
    #[getter]
    fn notional(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.notional)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpot.base_calendar_id")]
    #[getter]
    fn base_calendar_id(&self) -> Option<String> {
        self.inner
            .base_calendar_id
            .as_ref()
            .map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpot.quote_calendar_id")]
    #[getter]
    fn quote_calendar_id(&self) -> Option<String> {
        self.inner
            .quote_calendar_id
            .as_ref()
            .map(ToString::to_string)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpotBuilder")]
#[pyclass(
    name = "FxSpotBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyFxSpotBuilder {
    inner: Option<FxSpotBuilder>,
}

#[pymethods]
impl PyFxSpotBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpotBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: FxSpotBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpotBuilder.base_currency")]
    fn base_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: FxSpotBuilder| b.base_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpotBuilder.quote_currency")]
    fn quote_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: FxSpotBuilder| b.quote_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpotBuilder.settlement_date")]
    fn settlement_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(crate::bindings::date_utils::py_to_date)
            .transpose()?;
        builder_set!(slf, |b: FxSpotBuilder| b.settlement_date_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpotBuilder.settlement_days")]
    fn settlement_days<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<u32>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: FxSpotBuilder| b.settlement_days_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpotBuilder.quoted_spot")]
    fn quoted_spot<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: FxSpotBuilder| b.quoted_spot_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpotBuilder.domestic_discount_curve_id")]
    fn domestic_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: FxSpotBuilder| b
            .domestic_discount_curve_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpotBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: FxSpotBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpotBuilder.base_calendar_id")]
    fn base_calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: FxSpotBuilder| b.base_calendar_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpotBuilder.quote_calendar_id")]
    fn quote_calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: FxSpotBuilder| b.quote_calendar_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_spot", "FxSpotBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: FxSpotBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyFxSpot>()?;
    module.add_class::<PyFxSpotBuilder>()?;
    Ok(())
}
