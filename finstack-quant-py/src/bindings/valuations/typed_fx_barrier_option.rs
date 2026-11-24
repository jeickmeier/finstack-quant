//! An FX option whose activation depends on its contractual barrier observations.

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::fx::fx_barrier_option::{
    FxBarrierOption, FxBarrierOptionBuilder,
};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use super::convert::attributes_from_py;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption")]
#[pyclass(
    name = "FxBarrierOption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFxBarrierOption {
    pub(crate) inner: FxBarrierOption,
}

instrument_envelope_methods!(
    PyFxBarrierOption(
        "FxBarrierOption",
        module = "finstack_quant.valuations.instruments._typed_fx_barrier_option"
    ),
    FxBarrierOption,
    builder = PyFxBarrierOptionBuilder("FxBarrierOptionBuilder", no_fields),
    seed = FxBarrierOption::builder(),
);
instrument_pricing_methods!(
    PyFxBarrierOption,
    "FxBarrierOption",
    module = "finstack_quant.valuations.instruments._typed_fx_barrier_option"
);

super::typed_macros::instrument_greek_methods!(
    PyFxBarrierOption,
    "FxBarrierOption",
    "finstack_quant.valuations.instruments._typed_fx_barrier_option",
    [delta, gamma, vega, theta, rho, foreign_rho, vanna, volga]
);

#[pymethods]
impl PyFxBarrierOption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.greeks")]
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

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.to_builder")]
    fn to_builder(&self) -> PyFxBarrierOptionBuilder {
        PyFxBarrierOptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        FxBarrierOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.rebate_timing")]
    #[getter]
    fn rebate_timing<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.rebate_timing)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.option_type")]
    #[getter]
    fn option_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.option_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.barrier_type")]
    #[getter]
    fn barrier_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.barrier_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.expiry)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.monitoring_start_date")]
    #[getter]
    fn monitoring_start_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .monitoring_start_date
            .map(|value| crate::bindings::date_utils::date_to_py(py, value))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.monitoring")]
    #[getter]
    fn monitoring<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.monitoring)
    }
}

crate::bindings::macros::py_getters!(PyFxBarrierOption |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.strike")]
    strike -> f64 = s.inner.strike;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.barrier")]
    barrier -> f64 = s.inner.barrier;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.rebate")]
    rebate -> Option<PyMoney> = s.inner.rebate.map(PyMoney::from_inner);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.observed_barrier_breached")]
    observed_barrier_breached -> Option<bool> = s.inner.observed_barrier_breached;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.notional")]
    notional -> PyMoney = PyMoney::from_inner(s.inner.notional);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.base_currency")]
    base_currency -> String = s.inner.base_currency.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.quote_currency")]
    quote_currency -> String = s.inner.quote_currency.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.day_count")]
    day_count -> PyDayCount = PyDayCount::from_inner(s.inner.day_count);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.domestic_discount_curve_id")]
    domestic_discount_curve_id -> String = s.inner.domestic_discount_curve_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.foreign_discount_curve_id")]
    foreign_discount_curve_id -> String = s.inner.foreign_discount_curve_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.fx_spot_id")]
    fx_spot_id -> Option<String> = s.inner.fx_spot_id.as_ref().map(ToString::to_string);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOption.vol_surface_id")]
    vol_surface_id -> String = s.inner.vol_surface_id.to_string();
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder")]
#[pyclass(
    name = "FxBarrierOptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyFxBarrierOptionBuilder {
    inner: Option<FxBarrierOptionBuilder>,
}

#[pymethods]
impl PyFxBarrierOptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: FxBarrierOptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.strike")]
    fn strike<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: FxBarrierOptionBuilder| b.strike(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.barrier")]
    fn barrier<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: FxBarrierOptionBuilder| b.barrier(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.rebate")]
    fn rebate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(|money| money.inner);
        builder_set!(slf, |b: FxBarrierOptionBuilder| b.rebate_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.rebate_timing")]
    fn rebate_timing<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "rebate_timing")?;
        builder_set!(slf, |b: FxBarrierOptionBuilder| b.rebate_timing_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.option_type")]
    fn option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "option_type")?;
        builder_set!(slf, |b: FxBarrierOptionBuilder| b.option_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.barrier_type")]
    fn barrier_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "barrier_type")?;
        builder_set!(slf, |b: FxBarrierOptionBuilder| b.barrier_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.expiry")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: FxBarrierOptionBuilder| b.expiry(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.monitoring_start_date")]
    fn monitoring_start_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(crate::bindings::date_utils::py_to_date)
            .transpose()?;
        builder_set!(slf, |b: FxBarrierOptionBuilder| b
            .monitoring_start_date_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.observed_barrier_breached")]
    fn observed_barrier_breached<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<bool>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: FxBarrierOptionBuilder| b
            .observed_barrier_breached_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: FxBarrierOptionBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.base_currency")]
    fn base_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: FxBarrierOptionBuilder| b.base_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.quote_currency")]
    fn quote_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: FxBarrierOptionBuilder| b.quote_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: FxBarrierOptionBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.monitoring")]
    fn monitoring<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "monitoring")?;
        builder_set!(slf, |b: FxBarrierOptionBuilder| b.monitoring(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.domestic_discount_curve_id")]
    fn domestic_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: FxBarrierOptionBuilder| b
            .domestic_discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.foreign_discount_curve_id")]
    fn foreign_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: FxBarrierOptionBuilder| b
            .foreign_discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.fx_spot_id")]
    fn fx_spot_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: FxBarrierOptionBuilder| b.fx_spot_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: FxBarrierOptionBuilder| b.vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_barrier_option", "FxBarrierOptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: FxBarrierOptionBuilder| b.attributes(value))
    }
}

/// Register the FX barrier option and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyFxBarrierOption>()?;
    module.add_class::<PyFxBarrierOptionBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["FxBarrierOption", "FxBarrierOptionBuilder"];
