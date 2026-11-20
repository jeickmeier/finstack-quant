//! A cash or asset payout contingent on the FX strike.

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::fx::fx_digital_option::{
    FxDigitalOption, FxDigitalOptionBuilder,
};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use super::convert::attributes_from_py;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOption")]
#[pyclass(
    name = "FxDigitalOption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFxDigitalOption {
    pub(crate) inner: FxDigitalOption,
}

instrument_envelope_methods!(
    PyFxDigitalOption(
        "FxDigitalOption",
        module = "finstack_quant.valuations.instruments._typed_fx_digital_option"
    ),
    FxDigitalOption,
    builder = PyFxDigitalOptionBuilder("FxDigitalOptionBuilder", no_fields),
    seed = FxDigitalOption::builder(),
);
instrument_pricing_methods!(
    PyFxDigitalOption,
    "FxDigitalOption",
    module = "finstack_quant.valuations.instruments._typed_fx_digital_option"
);

super::typed_macros::instrument_greek_methods!(
    PyFxDigitalOption,
    "FxDigitalOption",
    "finstack_quant.valuations.instruments._typed_fx_digital_option",
    [delta, gamma, vega, theta, rho, foreign_rho]
);

#[pymethods]
impl PyFxDigitalOption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOption.greeks")]
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

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOption.to_builder")]
    fn to_builder(&self) -> PyFxDigitalOptionBuilder {
        PyFxDigitalOptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        FxDigitalOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOption.base_currency")]
    #[getter]
    fn base_currency(&self) -> String {
        self.inner.base_currency.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOption.quote_currency")]
    #[getter]
    fn quote_currency(&self) -> String {
        self.inner.quote_currency.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOption.strike")]
    #[getter]
    fn strike(&self) -> f64 {
        self.inner.strike
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOption.option_type")]
    #[getter]
    fn option_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.option_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOption.payout_type")]
    #[getter]
    fn payout_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.payout_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOption.payout_amount")]
    #[getter]
    fn payout_amount(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.payout_amount)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.expiry)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOption.day_count")]
    #[getter]
    fn day_count(&self) -> PyDayCount {
        PyDayCount::from_inner(self.inner.day_count)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOption.notional")]
    #[getter]
    fn notional(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.notional)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOption.domestic_discount_curve_id")]
    #[getter]
    fn domestic_discount_curve_id(&self) -> String {
        self.inner.domestic_discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOption.foreign_discount_curve_id")]
    #[getter]
    fn foreign_discount_curve_id(&self) -> String {
        self.inner.foreign_discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOption.vol_surface_id")]
    #[getter]
    fn vol_surface_id(&self) -> String {
        self.inner.vol_surface_id.to_string()
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOptionBuilder")]
#[pyclass(
    name = "FxDigitalOptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyFxDigitalOptionBuilder {
    inner: Option<FxDigitalOptionBuilder>,
}

#[pymethods]
impl PyFxDigitalOptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: FxDigitalOptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOptionBuilder.base_currency")]
    fn base_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: FxDigitalOptionBuilder| b.base_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOptionBuilder.quote_currency")]
    fn quote_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: FxDigitalOptionBuilder| b.quote_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOptionBuilder.strike")]
    fn strike<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: FxDigitalOptionBuilder| b.strike(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOptionBuilder.option_type")]
    fn option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "option_type")?;
        builder_set!(slf, |b: FxDigitalOptionBuilder| b.option_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOptionBuilder.payout_type")]
    fn payout_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "payout_type")?;
        builder_set!(slf, |b: FxDigitalOptionBuilder| b.payout_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOptionBuilder.payout_amount")]
    fn payout_amount<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: FxDigitalOptionBuilder| b.payout_amount(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOptionBuilder.expiry")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: FxDigitalOptionBuilder| b.expiry(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOptionBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: FxDigitalOptionBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOptionBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: FxDigitalOptionBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOptionBuilder.domestic_discount_curve_id")]
    fn domestic_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: FxDigitalOptionBuilder| b
            .domestic_discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOptionBuilder.foreign_discount_curve_id")]
    fn foreign_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: FxDigitalOptionBuilder| b
            .foreign_discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOptionBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: FxDigitalOptionBuilder| b.vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_digital_option", "FxDigitalOptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: FxDigitalOptionBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyFxDigitalOption>()?;
    module.add_class::<PyFxDigitalOptionBuilder>()?;
    Ok(())
}
