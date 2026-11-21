//! An option on the documented average of an underlying asset.

use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::exotics::asian_option::{
    AsianOption, AsianOptionBuilder,
};
use pyo3::prelude::*;

use super::convert::attributes_from_py;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOption")]
#[pyclass(
    name = "AsianOption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyAsianOption {
    pub(crate) inner: AsianOption,
}

instrument_envelope_methods!(
    PyAsianOption(
        "AsianOption",
        module = "finstack_quant.valuations.instruments._typed_asian_option"
    ),
    AsianOption,
    builder = PyAsianOptionBuilder("AsianOptionBuilder", no_fields),
    seed = AsianOption::builder(),
);
instrument_pricing_methods!(
    PyAsianOption,
    "AsianOption",
    module = "finstack_quant.valuations.instruments._typed_asian_option"
);

#[pymethods]
impl PyAsianOption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOption.to_builder")]
    fn to_builder(&self) -> PyAsianOptionBuilder {
        PyAsianOptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        AsianOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOption.underlying_ticker")]
    #[getter]
    fn underlying_ticker(&self) -> String {
        self.inner.underlying_ticker.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOption.strike")]
    #[getter]
    fn strike(&self) -> f64 {
        self.inner.strike
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOption.option_type")]
    #[getter]
    fn option_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.option_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOption.averaging_method")]
    #[getter]
    fn averaging_method<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.averaging_method)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.expiry)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOption.fixing_dates")]
    #[getter]
    fn fixing_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        {
            let dates = &self.inner.fixing_dates;
            dates
                .iter()
                .map(|date| crate::bindings::date_utils::date_to_py(py, *date))
                .collect::<PyResult<Vec<_>>>()
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOption.quantity")]
    #[getter]
    fn quantity(&self) -> f64 {
        self.inner.quantity
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOption.currency")]
    #[getter]
    fn currency(&self) -> String {
        self.inner.currency.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOption.day_count")]
    #[getter]
    fn day_count(&self) -> crate::bindings::core::dates::daycount::PyDayCount {
        crate::bindings::core::dates::daycount::PyDayCount::from_inner(self.inner.day_count)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOption.discount_curve_id")]
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOption.spot_id")]
    #[getter]
    fn spot_id(&self) -> String {
        self.inner.spot_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOption.vol_surface_id")]
    #[getter]
    fn vol_surface_id(&self) -> String {
        self.inner.vol_surface_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOption.div_yield_id")]
    #[getter]
    fn div_yield_id(&self) -> Option<String> {
        self.inner.div_yield_id.as_ref().map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOption.past_fixings")]
    #[getter]
    fn past_fixings<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, f64)>> {
        self.inner
            .past_fixings
            .iter()
            .map(|(d, x)| crate::bindings::date_utils::date_to_py(py, *d).map(|d| (d, *x)))
            .collect()
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOptionBuilder")]
#[pyclass(
    name = "AsianOptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyAsianOptionBuilder {
    inner: Option<AsianOptionBuilder>,
}

#[pymethods]
impl PyAsianOptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: AsianOptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOptionBuilder.underlying_ticker")]
    fn underlying_ticker<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: AsianOptionBuilder| b.underlying_ticker(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOptionBuilder.strike")]
    fn strike<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AsianOptionBuilder| b.strike(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOptionBuilder.option_type")]
    fn option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "option_type")?;
        builder_set!(slf, |b: AsianOptionBuilder| b.option_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOptionBuilder.averaging_method")]
    fn averaging_method<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "averaging_method")?;
        builder_set!(slf, |b: AsianOptionBuilder| b.averaging_method(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOptionBuilder.expiry")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: AsianOptionBuilder| b.expiry(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOptionBuilder.fixing_dates")]
    fn fixing_dates<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = {
            let dates = value;
            dates
                .iter()
                .map(crate::bindings::date_utils::py_to_date)
                .collect::<PyResult<Vec<_>>>()?
        };
        builder_set!(slf, |b: AsianOptionBuilder| b.fixing_dates(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOptionBuilder.quantity")]
    fn quantity<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: AsianOptionBuilder| b.quantity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOptionBuilder.currency")]
    fn currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: AsianOptionBuilder| b.currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOptionBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: AsianOptionBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOptionBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: AsianOptionBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOptionBuilder.spot_id")]
    fn spot_id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: AsianOptionBuilder| b.spot_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOptionBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: AsianOptionBuilder| b.vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOptionBuilder.div_yield_id")]
    fn div_yield_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: AsianOptionBuilder| b.div_yield_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOptionBuilder.past_fixings")]
    fn past_fixings<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<(Bound<'_, PyAny>, f64)>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .into_iter()
            .map(|(d, x)| crate::bindings::date_utils::py_to_date(&d).map(|date| (date, x)))
            .collect::<PyResult<_>>()?;
        builder_set!(slf, |b: AsianOptionBuilder| b.past_fixings(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_asian_option", "AsianOptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: AsianOptionBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyAsianOption>()?;
    module.add_class::<PyAsianOptionBuilder>()?;
    Ok(())
}
