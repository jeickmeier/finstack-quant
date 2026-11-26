//! A swap on annualized realized FX variance.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::fx::fx_variance_swap::{
    FxVarianceSwap, FxVarianceSwapBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_trait_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap")]
#[pyclass(
    name = "FxVarianceSwap",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFxVarianceSwap {
    pub(crate) inner: FxVarianceSwap,
}

instrument_envelope_methods!(
    PyFxVarianceSwap(
        "FxVarianceSwap",
        module = "finstack_quant.valuations.instruments._typed_fx_variance_swap"
    ),
    FxVarianceSwap,
    builder = PyFxVarianceSwapBuilder("FxVarianceSwapBuilder", no_fields),
    seed = FxVarianceSwap::builder(),
);
instrument_trait_methods!(
    PyFxVarianceSwap,
    "FxVarianceSwap",
    module = "finstack_quant.valuations.instruments._typed_fx_variance_swap"
);

#[pymethods]
impl PyFxVarianceSwap {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.to_builder")]
    fn to_builder(&self) -> PyFxVarianceSwapBuilder {
        PyFxVarianceSwapBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        FxVarianceSwap::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.start_date")]
    #[getter]
    fn start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.start_date)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.maturity)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.settlement_date")]
    #[getter]
    fn settlement_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .settlement_date
            .map(|value| crate::bindings::date_utils::date_to_py(py, value))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.observation_frequency")]
    #[getter]
    fn observation_frequency<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.observation_frequency)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.realized_var_method")]
    #[getter]
    fn realized_var_method<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.realized_var_method)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.previous_close_date")]
    #[getter]
    fn previous_close_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .previous_close_date
            .map(|value| crate::bindings::date_utils::date_to_py(py, value))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.side")]
    #[getter]
    fn side<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.side)
    }
}

crate::bindings::macros::py_getters!(PyFxVarianceSwap |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.base_currency")]
    base_currency -> String = s.inner.base_currency.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.quote_currency")]
    quote_currency -> String = s.inner.quote_currency.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.spot_id")]
    spot_id -> Option<String> = s.inner.spot_id.as_ref().map(ToString::to_string);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.notional")]
    notional -> PyMoney = PyMoney::from_inner(s.inner.notional);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.strike_variance")]
    strike_variance -> f64 = s.inner.strike_variance;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.base_calendar_id")]
    base_calendar_id -> String = s.inner.base_calendar_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.quote_calendar_id")]
    quote_calendar_id -> String = s.inner.quote_calendar_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.observation_business_day_convention")]
    observation_business_day_convention -> PyResult<String> =
        super::convert::enum_to_py_string(&s.inner.observation_business_day_convention);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.observation_end_of_month")]
    observation_end_of_month -> bool = s.inner.observation_end_of_month;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.trading_days_per_year")]
    trading_days_per_year -> f64 = s.inner.trading_days_per_year;

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.open_series_id")]
    open_series_id -> Option<String> = s.inner.open_series_id.as_ref().map(ToString::to_string);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.high_series_id")]
    high_series_id -> Option<String> = s.inner.high_series_id.as_ref().map(ToString::to_string);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.low_series_id")]
    low_series_id -> Option<String> = s.inner.low_series_id.as_ref().map(ToString::to_string);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.close_series_id")]
    close_series_id -> Option<String> = s.inner.close_series_id.as_ref().map(ToString::to_string);

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.domestic_discount_curve_id")]
    domestic_discount_curve_id -> String = s.inner.domestic_discount_curve_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.foreign_discount_curve_id")]
    foreign_discount_curve_id -> String = s.inner.foreign_discount_curve_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.vol_surface_id")]
    vol_surface_id -> String = s.inner.vol_surface_id.to_string();

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwap.day_count")]
    day_count -> crate::bindings::core::dates::daycount::PyDayCount =
        crate::bindings::core::dates::daycount::PyDayCount::from_inner(s.inner.day_count);
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder")]
#[pyclass(
    name = "FxVarianceSwapBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyFxVarianceSwapBuilder {
    inner: Option<FxVarianceSwapBuilder>,
}

#[pymethods]
impl PyFxVarianceSwapBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.base_currency")]
    fn base_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.base_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.quote_currency")]
    fn quote_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.quote_currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.spot_id")]
    fn spot_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.spot_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.strike_variance")]
    fn strike_variance<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.strike_variance(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.start_date")]
    fn start_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.start_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.maturity")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.maturity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.settlement_date")]
    fn settlement_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(crate::bindings::date_utils::py_to_date)
            .transpose()?;
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.settlement_date_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.observation_frequency")]
    fn observation_frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "observation_frequency")?;
        builder_set!(slf, |b: FxVarianceSwapBuilder| b
            .observation_frequency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.base_calendar_id")]
    fn base_calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.base_calendar_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.quote_calendar_id")]
    fn quote_calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.quote_calendar_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.observation_business_day_convention")]
    fn observation_business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "observation_business_day_convention")?;
        builder_set!(slf, |b: FxVarianceSwapBuilder| b
            .observation_business_day_convention(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.observation_end_of_month")]
    fn observation_end_of_month<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: FxVarianceSwapBuilder| b
            .observation_end_of_month(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.realized_var_method")]
    fn realized_var_method<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "realized_var_method")?;
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.realized_var_method(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.trading_days_per_year")]
    fn trading_days_per_year<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: FxVarianceSwapBuilder| b
            .trading_days_per_year(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.open_series_id")]
    fn open_series_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.open_series_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.high_series_id")]
    fn high_series_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.high_series_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.low_series_id")]
    fn low_series_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.low_series_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.close_series_id")]
    fn close_series_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.close_series_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.previous_close_date")]
    fn previous_close_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(crate::bindings::date_utils::py_to_date)
            .transpose()?;
        builder_set!(slf, |b: FxVarianceSwapBuilder| b
            .previous_close_date_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.side")]
    fn side<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "side")?;
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.side(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.domestic_discount_curve_id")]
    fn domestic_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: FxVarianceSwapBuilder| b
            .domestic_discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.foreign_discount_curve_id")]
    fn foreign_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: FxVarianceSwapBuilder| b
            .foreign_discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_fx_variance_swap", "FxVarianceSwapBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: FxVarianceSwapBuilder| b.attributes(value))
    }
}

/// Register the FX variance swap and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyFxVarianceSwap>()?;
    module.add_class::<PyFxVarianceSwapBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["FxVarianceSwap", "FxVarianceSwapBuilder"];
