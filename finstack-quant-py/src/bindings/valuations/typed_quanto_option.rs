//! An equity option with contractually specified foreign-exchange conversion.

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::fx::quanto_option::{
    QuantoOption, QuantoOptionBuilder,
};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use super::convert::attributes_from_py;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption")]
#[pyclass(
    name = "QuantoOption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyQuantoOption {
    pub(crate) inner: QuantoOption,
}

instrument_envelope_methods!(
    PyQuantoOption(
        "QuantoOption",
        module = "finstack_quant.valuations.instruments._typed_quanto_option"
    ),
    QuantoOption,
    builder = PyQuantoOptionBuilder("QuantoOptionBuilder", no_fields),
    seed = QuantoOption::builder(),
);
instrument_pricing_methods!(
    PyQuantoOption,
    "QuantoOption",
    module = "finstack_quant.valuations.instruments._typed_quanto_option"
);

super::typed_macros::instrument_greek_methods!(
    PyQuantoOption,
    "QuantoOption",
    "finstack_quant.valuations.instruments._typed_quanto_option",
    [delta, gamma, vega, theta, rho, foreign_rho, vanna, volga]
);

#[pymethods]
impl PyQuantoOption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.greeks")]
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

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.to_builder")]
    fn to_builder(&self) -> PyQuantoOptionBuilder {
        PyQuantoOptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        QuantoOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.underlying_ticker")]
    #[getter]
    fn underlying_ticker(&self) -> String {
        self.inner.underlying_ticker.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.strike")]
    #[getter]
    fn strike(&self) -> f64 {
        self.inner.strike
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.option_type")]
    #[getter]
    fn option_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.option_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.expiry)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.notional")]
    #[getter]
    fn notional(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.notional)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.quantity")]
    #[getter]
    fn quantity(&self) -> Option<f64> {
        self.inner.quantity
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.payoff_fx_rate")]
    #[getter]
    fn payoff_fx_rate(&self) -> Option<f64> {
        self.inner.payoff_fx_rate
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.base_currency")]
    #[getter]
    fn base_currency(&self) -> String {
        self.inner.base_currency.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.quote_currency")]
    #[getter]
    fn quote_currency(&self) -> String {
        self.inner.quote_currency.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.correlation")]
    #[getter]
    fn correlation<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.correlation)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.day_count")]
    #[getter]
    fn day_count(&self) -> PyDayCount {
        PyDayCount::from_inner(self.inner.day_count)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.domestic_discount_curve_id")]
    #[getter]
    fn domestic_discount_curve_id(&self) -> String {
        self.inner.domestic_discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.foreign_discount_curve_id")]
    #[getter]
    fn foreign_discount_curve_id(&self) -> String {
        self.inner.foreign_discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.spot_id")]
    #[getter]
    fn spot_id(&self) -> String {
        self.inner.spot_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.vol_surface_id")]
    #[getter]
    fn vol_surface_id(&self) -> String {
        self.inner.vol_surface_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.div_yield_id")]
    #[getter]
    fn div_yield_id(&self) -> Option<String> {
        self.inner.div_yield_id.as_ref().map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.fx_spot_id")]
    #[getter]
    fn fx_spot_id(&self) -> Option<String> {
        self.inner.fx_spot_id.as_ref().map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOption.fx_vol_surface_id")]
    #[getter]
    fn fx_vol_surface_id(&self) -> Option<String> {
        self.inner
            .fx_vol_surface_id
            .as_ref()
            .map(ToString::to_string)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder")]
#[pyclass(
    name = "QuantoOptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyQuantoOptionBuilder {
    inner: Option<QuantoOptionBuilder>,
}

#[pymethods]
impl PyQuantoOptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: QuantoOptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.underlying_ticker")]
    fn underlying_ticker<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: QuantoOptionBuilder| b.underlying_ticker(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.strike")]
    fn strike<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: QuantoOptionBuilder| b.strike(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.option_type")]
    fn option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "option_type")?;
        builder_set!(slf, |b: QuantoOptionBuilder| b.option_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.expiry")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: QuantoOptionBuilder| b.expiry(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: QuantoOptionBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.quantity")]
    fn quantity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: QuantoOptionBuilder| b.quantity_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.payoff_fx_rate")]
    fn payoff_fx_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: QuantoOptionBuilder| b.payoff_fx_rate_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.base_currency")]
    fn base_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: QuantoOptionBuilder| b.base_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.quote_currency")]
    fn quote_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: QuantoOptionBuilder| b.quote_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.correlation")]
    fn correlation<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "correlation")?;
        builder_set!(slf, |b: QuantoOptionBuilder| b.correlation(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: QuantoOptionBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.domestic_discount_curve_id")]
    fn domestic_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: QuantoOptionBuilder| b
            .domestic_discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.foreign_discount_curve_id")]
    fn foreign_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: QuantoOptionBuilder| b
            .foreign_discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.spot_id")]
    fn spot_id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: QuantoOptionBuilder| b.spot_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: QuantoOptionBuilder| b.vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.div_yield_id")]
    fn div_yield_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: QuantoOptionBuilder| b.div_yield_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.fx_spot_id")]
    fn fx_spot_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: QuantoOptionBuilder| b.fx_spot_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.fx_vol_surface_id")]
    fn fx_vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: QuantoOptionBuilder| b.fx_vol_surface_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_quanto_option", "QuantoOptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: QuantoOptionBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyQuantoOption>()?;
    module.add_class::<PyQuantoOptionBuilder>()?;
    Ok(())
}
