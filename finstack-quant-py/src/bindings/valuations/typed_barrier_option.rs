//! An option whose payoff is conditional on an observed barrier.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::exotics::barrier_option::{
    BarrierOption, BarrierOptionBuilder,
};
use pyo3::prelude::*;

use super::convert::attributes_from_py;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption")]
#[pyclass(
    name = "BarrierOption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyBarrierOption {
    pub(crate) inner: BarrierOption,
}

instrument_envelope_methods!(
    PyBarrierOption(
        "BarrierOption",
        module = "finstack_quant.valuations.instruments._typed_barrier_option"
    ),
    BarrierOption,
    builder = PyBarrierOptionBuilder("BarrierOptionBuilder", no_fields),
    seed = BarrierOption::builder(),
);
instrument_pricing_methods!(
    PyBarrierOption,
    "BarrierOption",
    module = "finstack_quant.valuations.instruments._typed_barrier_option"
);

#[pymethods]
impl PyBarrierOption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.to_builder")]
    fn to_builder(&self) -> PyBarrierOptionBuilder {
        PyBarrierOptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        BarrierOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.underlying_ticker")]
    #[getter]
    fn underlying_ticker(&self) -> String {
        self.inner.underlying_ticker.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.strike")]
    #[getter]
    fn strike(&self) -> f64 {
        self.inner.strike
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.barrier")]
    #[getter]
    fn barrier(&self) -> f64 {
        self.inner.barrier
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.rebate")]
    #[getter]
    fn rebate(&self) -> Option<PyMoney> {
        self.inner.rebate.map(PyMoney::from_inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.rebate_timing")]
    #[getter]
    fn rebate_timing<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.rebate_timing)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.option_type")]
    #[getter]
    fn option_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.option_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.barrier_type")]
    #[getter]
    fn barrier_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.barrier_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.expiry)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.expiry_fixing")]
    #[getter]
    fn expiry_fixing(&self) -> Option<f64> {
        self.inner.expiry_fixing
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.observed_barrier_breached")]
    #[getter]
    fn observed_barrier_breached(&self) -> Option<bool> {
        self.inner.observed_barrier_breached
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.quantity")]
    #[getter]
    fn quantity(&self) -> f64 {
        self.inner.quantity
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.currency")]
    #[getter]
    fn currency(&self) -> String {
        self.inner.currency.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.day_count")]
    #[getter]
    fn day_count(&self) -> crate::bindings::core::dates::daycount::PyDayCount {
        crate::bindings::core::dates::daycount::PyDayCount::from_inner(self.inner.day_count)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.monitoring")]
    #[getter]
    fn monitoring<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.monitoring)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.discount_curve_id")]
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.spot_id")]
    #[getter]
    fn spot_id(&self) -> String {
        self.inner.spot_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.vol_surface_id")]
    #[getter]
    fn vol_surface_id(&self) -> String {
        self.inner.vol_surface_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOption.div_yield_id")]
    #[getter]
    fn div_yield_id(&self) -> Option<String> {
        self.inner.div_yield_id.as_ref().map(ToString::to_string)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder")]
#[pyclass(
    name = "BarrierOptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyBarrierOptionBuilder {
    inner: Option<BarrierOptionBuilder>,
}

#[pymethods]
impl PyBarrierOptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: BarrierOptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.underlying_ticker")]
    fn underlying_ticker<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: BarrierOptionBuilder| b.underlying_ticker(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.strike")]
    fn strike<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: BarrierOptionBuilder| b.strike(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.barrier")]
    fn barrier<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: BarrierOptionBuilder| b.barrier(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.rebate")]
    fn rebate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(|money| money.inner);
        builder_set!(slf, |b: BarrierOptionBuilder| b.rebate_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.rebate_timing")]
    fn rebate_timing<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "rebate_timing")?;
        builder_set!(slf, |b: BarrierOptionBuilder| b.rebate_timing_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.option_type")]
    fn option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "option_type")?;
        builder_set!(slf, |b: BarrierOptionBuilder| b.option_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.barrier_type")]
    fn barrier_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "barrier_type")?;
        builder_set!(slf, |b: BarrierOptionBuilder| b.barrier_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.expiry")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: BarrierOptionBuilder| b.expiry(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.expiry_fixing")]
    fn expiry_fixing<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: BarrierOptionBuilder| b.expiry_fixing_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.observed_barrier_breached")]
    fn observed_barrier_breached<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<bool>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: BarrierOptionBuilder| b
            .observed_barrier_breached_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.quantity")]
    fn quantity<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: BarrierOptionBuilder| b.quantity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.currency")]
    fn currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: BarrierOptionBuilder| b.currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: BarrierOptionBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.monitoring")]
    fn monitoring<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "monitoring")?;
        builder_set!(slf, |b: BarrierOptionBuilder| b.monitoring(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: BarrierOptionBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.spot_id")]
    fn spot_id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: BarrierOptionBuilder| b.spot_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: BarrierOptionBuilder| b.vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.div_yield_id")]
    fn div_yield_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: BarrierOptionBuilder| b.div_yield_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_barrier_option", "BarrierOptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: BarrierOptionBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyBarrierOption>()?;
    module.add_class::<PyBarrierOptionBuilder>()?;
    Ok(())
}
