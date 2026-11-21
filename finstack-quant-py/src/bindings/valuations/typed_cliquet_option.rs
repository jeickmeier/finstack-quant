//! An option on capped and floored returns over a sequence of equity resets.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::equity::cliquet_option::{
    CliquetOption, CliquetOptionBuilder,
};
use pyo3::prelude::*;

use super::convert::attributes_from_py;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption")]
#[pyclass(
    name = "CliquetOption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCliquetOption {
    pub(crate) inner: CliquetOption,
}

instrument_envelope_methods!(
    PyCliquetOption(
        "CliquetOption",
        module = "finstack_quant.valuations.instruments._typed_cliquet_option"
    ),
    CliquetOption,
    builder = PyCliquetOptionBuilder("CliquetOptionBuilder", no_fields),
    seed = CliquetOption::builder(),
);
instrument_pricing_methods!(
    PyCliquetOption,
    "CliquetOption",
    module = "finstack_quant.valuations.instruments._typed_cliquet_option"
);

#[pymethods]
impl PyCliquetOption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption.to_builder")]
    fn to_builder(&self) -> PyCliquetOptionBuilder {
        PyCliquetOptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        CliquetOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption.underlying_ticker")]
    #[getter]
    fn underlying_ticker(&self) -> String {
        self.inner.underlying_ticker.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption.reset_dates")]
    #[getter]
    fn reset_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        {
            let dates = &self.inner.reset_dates;
            dates
                .iter()
                .map(|date| crate::bindings::date_utils::date_to_py(py, *date))
                .collect::<PyResult<Vec<_>>>()
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.expiry)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption.local_cap")]
    #[getter]
    fn local_cap(&self) -> f64 {
        self.inner.local_cap
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption.local_floor")]
    #[getter]
    fn local_floor(&self) -> f64 {
        self.inner.local_floor
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption.global_cap")]
    #[getter]
    fn global_cap(&self) -> f64 {
        self.inner.global_cap
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption.global_floor")]
    #[getter]
    fn global_floor(&self) -> f64 {
        self.inner.global_floor
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption.notional")]
    #[getter]
    fn notional(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.notional)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption.day_count")]
    #[getter]
    fn day_count(&self) -> crate::bindings::core::dates::daycount::PyDayCount {
        crate::bindings::core::dates::daycount::PyDayCount::from_inner(self.inner.day_count)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption.discount_curve_id")]
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption.spot_id")]
    #[getter]
    fn spot_id(&self) -> String {
        self.inner.spot_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption.vol_surface_id")]
    #[getter]
    fn vol_surface_id(&self) -> String {
        self.inner.vol_surface_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption.path_model")]
    #[getter]
    fn path_model<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.path_model)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption.div_yield_id")]
    #[getter]
    fn div_yield_id(&self) -> Option<String> {
        self.inner.div_yield_id.as_ref().map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption.initial_level")]
    #[getter]
    fn initial_level(&self) -> Option<f64> {
        self.inner.initial_level
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption.past_fixings")]
    #[getter]
    fn past_fixings<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, f64)>> {
        self.inner
            .past_fixings
            .iter()
            .map(|(d, x)| crate::bindings::date_utils::date_to_py(py, *d).map(|d| (d, *x)))
            .collect()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOption.payoff_type")]
    #[getter]
    fn payoff_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.payoff_type)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder")]
#[pyclass(
    name = "CliquetOptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyCliquetOptionBuilder {
    inner: Option<CliquetOptionBuilder>,
}

#[pymethods]
impl PyCliquetOptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: CliquetOptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder.underlying_ticker")]
    fn underlying_ticker<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CliquetOptionBuilder| b.underlying_ticker(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder.reset_dates")]
    fn reset_dates<'py>(
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
        builder_set!(slf, |b: CliquetOptionBuilder| b.reset_dates(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder.expiry")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: CliquetOptionBuilder| b.expiry(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder.local_cap")]
    fn local_cap<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CliquetOptionBuilder| b.local_cap(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder.local_floor")]
    fn local_floor<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CliquetOptionBuilder| b.local_floor(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder.global_cap")]
    fn global_cap<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CliquetOptionBuilder| b.global_cap(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder.global_floor")]
    fn global_floor<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CliquetOptionBuilder| b.global_floor(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: CliquetOptionBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: CliquetOptionBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CliquetOptionBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder.spot_id")]
    fn spot_id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CliquetOptionBuilder| b.spot_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CliquetOptionBuilder| b.vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder.path_model")]
    fn path_model<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "path_model")?;
        builder_set!(slf, |b: CliquetOptionBuilder| b.path_model(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder.div_yield_id")]
    fn div_yield_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: CliquetOptionBuilder| b.div_yield_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder.initial_level")]
    fn initial_level<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CliquetOptionBuilder| b.initial_level_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder.past_fixings")]
    fn past_fixings<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<(Bound<'_, PyAny>, f64)>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .into_iter()
            .map(|(d, x)| crate::bindings::date_utils::py_to_date(&d).map(|date| (date, x)))
            .collect::<PyResult<_>>()?;
        builder_set!(slf, |b: CliquetOptionBuilder| b.past_fixings(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder.payoff_type")]
    fn payoff_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "payoff_type")?;
        builder_set!(slf, |b: CliquetOptionBuilder| b.payoff_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cliquet_option", "CliquetOptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: CliquetOptionBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCliquetOption>()?;
    module.add_class::<PyCliquetOptionBuilder>()?;
    Ok(())
}
