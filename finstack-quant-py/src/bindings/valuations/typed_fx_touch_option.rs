//! An FX touch or no-touch claim under a specified monitoring interval.

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::fx::fx_touch_option::{
    FxTouchOption, FxTouchOptionBuilder,
};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use super::convert::attributes_from_py;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOption")]
#[pyclass(
    name = "FxTouchOption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFxTouchOption {
    pub(crate) inner: FxTouchOption,
}

instrument_envelope_methods!(
    PyFxTouchOption(
        "FxTouchOption",
        module = "finstack_quant.valuations.instruments._typed_fx_touch_option"
    ),
    FxTouchOption,
    builder = PyFxTouchOptionBuilder("FxTouchOptionBuilder", no_fields),
    seed = FxTouchOption::builder(),
);
instrument_pricing_methods!(
    PyFxTouchOption,
    "FxTouchOption",
    module = "finstack_quant.valuations.instruments._typed_fx_touch_option"
);

super::typed_macros::instrument_greek_methods!(
    PyFxTouchOption,
    "FxTouchOption",
    "finstack_quant.valuations.instruments._typed_fx_touch_option",
    [delta, gamma, vega, theta, rho, foreign_rho]
);

#[pymethods]
impl PyFxTouchOption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOption.greeks")]
    #[pyo3(signature = (market, as_of, model="default", extras=None))]
    fn greeks<'py>(
        &self,
        py: Python<'py>,
        market: &Bound<'py, PyAny>,
        as_of: &Bound<'py, PyAny>,
        model: &str,
        extras: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyDict>> {
        super::pricing::typed_option_greeks(
            py,
            Box::new(self.inner.clone()),
            market,
            as_of,
            model,
            super::pricing::price_extras_from_py(py, extras)?,
        )
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOption.to_builder")]
    fn to_builder(&self) -> PyFxTouchOptionBuilder {
        PyFxTouchOptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        FxTouchOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOption.base_currency")]
    #[getter]
    fn base_currency(&self) -> String {
        self.inner.base_currency.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOption.quote_currency")]
    #[getter]
    fn quote_currency(&self) -> String {
        self.inner.quote_currency.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOption.barrier")]
    #[getter]
    fn barrier(&self) -> f64 {
        self.inner.barrier
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOption.touch_type")]
    #[getter]
    fn touch_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.touch_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOption.barrier_direction")]
    #[getter]
    fn barrier_direction<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.barrier_direction)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOption.payout_amount")]
    #[getter]
    fn payout_amount(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.payout_amount)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOption.payout_timing")]
    #[getter]
    fn payout_timing<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.payout_timing)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.expiry)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOption.monitoring_start_date")]
    #[getter]
    fn monitoring_start_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .monitoring_start_date
            .map(|value| crate::bindings::date_utils::date_to_py(py, value))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOption.day_count")]
    #[getter]
    fn day_count(&self) -> PyDayCount {
        PyDayCount::from_inner(self.inner.day_count)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOption.domestic_discount_curve_id")]
    #[getter]
    fn domestic_discount_curve_id(&self) -> String {
        self.inner.domestic_discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOption.foreign_discount_curve_id")]
    #[getter]
    fn foreign_discount_curve_id(&self) -> String {
        self.inner.foreign_discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOption.vol_surface_id")]
    #[getter]
    fn vol_surface_id(&self) -> String {
        self.inner.vol_surface_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOption.observed_barrier_breached")]
    #[getter]
    fn observed_barrier_breached(&self) -> Option<bool> {
        self.inner.observed_barrier_breached
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOptionBuilder")]
#[pyclass(
    name = "FxTouchOptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyFxTouchOptionBuilder {
    inner: Option<FxTouchOptionBuilder>,
}

#[pymethods]
impl PyFxTouchOptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: FxTouchOptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOptionBuilder.base_currency")]
    fn base_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: FxTouchOptionBuilder| b.base_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOptionBuilder.quote_currency")]
    fn quote_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: FxTouchOptionBuilder| b.quote_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOptionBuilder.barrier")]
    fn barrier<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: FxTouchOptionBuilder| b.barrier(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOptionBuilder.touch_type")]
    fn touch_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "touch_type")?;
        builder_set!(slf, |b: FxTouchOptionBuilder| b.touch_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOptionBuilder.barrier_direction")]
    fn barrier_direction<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "barrier_direction")?;
        builder_set!(slf, |b: FxTouchOptionBuilder| b.barrier_direction(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOptionBuilder.payout_amount")]
    fn payout_amount<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: FxTouchOptionBuilder| b.payout_amount(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOptionBuilder.payout_timing")]
    fn payout_timing<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "payout_timing")?;
        builder_set!(slf, |b: FxTouchOptionBuilder| b.payout_timing(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOptionBuilder.expiry")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: FxTouchOptionBuilder| b.expiry(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOptionBuilder.monitoring_start_date")]
    fn monitoring_start_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(crate::bindings::date_utils::py_to_date)
            .transpose()?;
        builder_set!(slf, |b: FxTouchOptionBuilder| b
            .monitoring_start_date_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOptionBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: FxTouchOptionBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOptionBuilder.domestic_discount_curve_id")]
    fn domestic_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: FxTouchOptionBuilder| b
            .domestic_discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOptionBuilder.foreign_discount_curve_id")]
    fn foreign_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: FxTouchOptionBuilder| b
            .foreign_discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOptionBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: FxTouchOptionBuilder| b.vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOptionBuilder.observed_barrier_breached")]
    fn observed_barrier_breached<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<bool>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: FxTouchOptionBuilder| b
            .observed_barrier_breached_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_touch_option", "FxTouchOptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: FxTouchOptionBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyFxTouchOption>()?;
    module.add_class::<PyFxTouchOptionBuilder>()?;
    Ok(())
}
