//! An option referencing the observed extreme of the underlying asset.

use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::exotics::lookback_option::{
    LookbackOption, LookbackOptionBuilder,
};
use pyo3::prelude::*;

use super::convert::attributes_from_py;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOption")]
#[pyclass(
    name = "LookbackOption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyLookbackOption {
    pub(crate) inner: LookbackOption,
}

instrument_envelope_methods!(
    PyLookbackOption(
        "LookbackOption",
        module = "finstack_quant.valuations.instruments._typed_lookback_option"
    ),
    LookbackOption,
    builder = PyLookbackOptionBuilder("LookbackOptionBuilder", no_fields),
    seed = LookbackOption::builder(),
);
instrument_pricing_methods!(
    PyLookbackOption,
    "LookbackOption",
    module = "finstack_quant.valuations.instruments._typed_lookback_option"
);

#[pymethods]
impl PyLookbackOption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOption.to_builder")]
    fn to_builder(&self) -> PyLookbackOptionBuilder {
        PyLookbackOptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        LookbackOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOption.underlying_ticker")]
    #[getter]
    fn underlying_ticker(&self) -> String {
        self.inner.underlying_ticker.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOption.strike")]
    #[getter]
    fn strike<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.strike)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOption.option_type")]
    #[getter]
    fn option_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.option_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOption.lookback_type")]
    #[getter]
    fn lookback_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.lookback_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.expiry)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOption.expiry_fixing")]
    #[getter]
    fn expiry_fixing(&self) -> Option<f64> {
        self.inner.expiry_fixing
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOption.quantity")]
    #[getter]
    fn quantity(&self) -> f64 {
        self.inner.quantity
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOption.currency")]
    #[getter]
    fn currency(&self) -> String {
        self.inner.currency.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOption.day_count")]
    #[getter]
    fn day_count(&self) -> crate::bindings::core::dates::daycount::PyDayCount {
        crate::bindings::core::dates::daycount::PyDayCount::from_inner(self.inner.day_count)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOption.discount_curve_id")]
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOption.spot_id")]
    #[getter]
    fn spot_id(&self) -> String {
        self.inner.spot_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOption.vol_surface_id")]
    #[getter]
    fn vol_surface_id(&self) -> String {
        self.inner.vol_surface_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOption.div_yield_id")]
    #[getter]
    fn div_yield_id(&self) -> Option<String> {
        self.inner.div_yield_id.as_ref().map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOption.monitoring")]
    #[getter]
    fn monitoring<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.monitoring)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOption.observed_min")]
    #[getter]
    fn observed_min(&self) -> Option<f64> {
        self.inner.observed_min
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOption.observed_max")]
    #[getter]
    fn observed_max(&self) -> Option<f64> {
        self.inner.observed_max
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOptionBuilder")]
#[pyclass(
    name = "LookbackOptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyLookbackOptionBuilder {
    inner: Option<LookbackOptionBuilder>,
}

#[pymethods]
impl PyLookbackOptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: LookbackOptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOptionBuilder.underlying_ticker")]
    fn underlying_ticker<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: LookbackOptionBuilder| b.underlying_ticker(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOptionBuilder.strike")]
    fn strike<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "strike")?;
        builder_set!(slf, |b: LookbackOptionBuilder| b.strike_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOptionBuilder.option_type")]
    fn option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "option_type")?;
        builder_set!(slf, |b: LookbackOptionBuilder| b.option_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOptionBuilder.lookback_type")]
    fn lookback_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "lookback_type")?;
        builder_set!(slf, |b: LookbackOptionBuilder| b.lookback_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOptionBuilder.expiry")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: LookbackOptionBuilder| b.expiry(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOptionBuilder.expiry_fixing")]
    fn expiry_fixing<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: LookbackOptionBuilder| b.expiry_fixing_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOptionBuilder.quantity")]
    fn quantity<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: LookbackOptionBuilder| b.quantity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOptionBuilder.currency")]
    fn currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: LookbackOptionBuilder| b.currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOptionBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: LookbackOptionBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOptionBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: LookbackOptionBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOptionBuilder.spot_id")]
    fn spot_id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: LookbackOptionBuilder| b.spot_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOptionBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: LookbackOptionBuilder| b.vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOptionBuilder.div_yield_id")]
    fn div_yield_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: LookbackOptionBuilder| b.div_yield_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOptionBuilder.monitoring")]
    fn monitoring<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "monitoring")?;
        builder_set!(slf, |b: LookbackOptionBuilder| b.monitoring(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOptionBuilder.observed_min")]
    fn observed_min<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: LookbackOptionBuilder| b.observed_min_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOptionBuilder.observed_max")]
    fn observed_max<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: LookbackOptionBuilder| b.observed_max_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_lookback_option", "LookbackOptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: LookbackOptionBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyLookbackOption>()?;
    module.add_class::<PyLookbackOptionBuilder>()?;
    Ok(())
}
