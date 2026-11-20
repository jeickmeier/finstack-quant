//! A swap on annualized realized equity variance.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::equity::variance_swap::{
    VarianceSwap, VarianceSwapBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap")]
#[pyclass(
    name = "VarianceSwap",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyVarianceSwap {
    pub(crate) inner: VarianceSwap,
}

instrument_envelope_methods!(
    PyVarianceSwap(
        "VarianceSwap",
        module = "finstack_quant.valuations.instruments._typed_variance_swap"
    ),
    VarianceSwap,
    builder = PyVarianceSwapBuilder("VarianceSwapBuilder", no_fields),
    seed = VarianceSwap::builder(),
);
instrument_pricing_methods!(
    PyVarianceSwap,
    "VarianceSwap",
    module = "finstack_quant.valuations.instruments._typed_variance_swap"
);

#[pymethods]
impl PyVarianceSwap {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.to_builder")]
    fn to_builder(&self) -> PyVarianceSwapBuilder {
        PyVarianceSwapBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        VarianceSwap::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.underlying_ticker")]
    #[getter]
    fn underlying_ticker(&self) -> String {
        self.inner.underlying_ticker.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.spot_id")]
    #[getter]
    fn spot_id(&self) -> String {
        self.inner.spot_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.vol_surface_id")]
    #[getter]
    fn vol_surface_id(&self) -> String {
        self.inner.vol_surface_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.div_yield_id")]
    #[getter]
    fn div_yield_id(&self) -> Option<String> {
        self.inner.div_yield_id.as_ref().map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.notional")]
    #[getter]
    fn notional(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.notional)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.strike_variance")]
    #[getter]
    fn strike_variance(&self) -> f64 {
        self.inner.strike_variance
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.start_date")]
    #[getter]
    fn start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.start_date)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.maturity)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.settlement_date")]
    #[getter]
    fn settlement_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .settlement_date
            .map(|value| crate::bindings::date_utils::date_to_py(py, value))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.observation_frequency")]
    #[getter]
    fn observation_frequency<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.observation_frequency)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.observation_calendar_id")]
    #[getter]
    fn observation_calendar_id(&self) -> String {
        self.inner.observation_calendar_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.observation_business_day_convention")]
    #[getter]
    fn observation_business_day_convention(&self) -> PyResult<String> {
        super::convert::enum_to_py_string(&self.inner.observation_business_day_convention)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.observation_end_of_month")]
    #[getter]
    fn observation_end_of_month(&self) -> bool {
        self.inner.observation_end_of_month
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.realized_var_method")]
    #[getter]
    fn realized_var_method<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.realized_var_method)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.trading_days_per_year")]
    #[getter]
    fn trading_days_per_year(&self) -> f64 {
        self.inner.trading_days_per_year
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.price_series_policy")]
    #[getter]
    fn price_series_policy<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.price_series_policy)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.open_series_id")]
    #[getter]
    fn open_series_id(&self) -> Option<String> {
        self.inner.open_series_id.as_ref().map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.high_series_id")]
    #[getter]
    fn high_series_id(&self) -> Option<String> {
        self.inner.high_series_id.as_ref().map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.low_series_id")]
    #[getter]
    fn low_series_id(&self) -> Option<String> {
        self.inner.low_series_id.as_ref().map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.close_series_id")]
    #[getter]
    fn close_series_id(&self) -> Option<String> {
        self.inner.close_series_id.as_ref().map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.previous_close_date")]
    #[getter]
    fn previous_close_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .previous_close_date
            .map(|value| crate::bindings::date_utils::date_to_py(py, value))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.side")]
    #[getter]
    fn side<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.side)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.discount_curve_id")]
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwap.day_count")]
    #[getter]
    fn day_count(&self) -> crate::bindings::core::dates::daycount::PyDayCount {
        crate::bindings::core::dates::daycount::PyDayCount::from_inner(self.inner.day_count)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder")]
#[pyclass(
    name = "VarianceSwapBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyVarianceSwapBuilder {
    inner: Option<VarianceSwapBuilder>,
}

#[pymethods]
impl PyVarianceSwapBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: VarianceSwapBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.underlying_ticker")]
    fn underlying_ticker<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: VarianceSwapBuilder| b.underlying_ticker(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.spot_id")]
    fn spot_id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: VarianceSwapBuilder| b.spot_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: VarianceSwapBuilder| b.vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.div_yield_id")]
    fn div_yield_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: VarianceSwapBuilder| b.div_yield_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: VarianceSwapBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.strike_variance")]
    fn strike_variance<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: VarianceSwapBuilder| b.strike_variance(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.start_date")]
    fn start_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: VarianceSwapBuilder| b.start_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.maturity")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: VarianceSwapBuilder| b.maturity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.settlement_date")]
    fn settlement_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(crate::bindings::date_utils::py_to_date)
            .transpose()?;
        builder_set!(slf, |b: VarianceSwapBuilder| b.settlement_date_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.observation_frequency")]
    fn observation_frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "observation_frequency")?;
        builder_set!(slf, |b: VarianceSwapBuilder| b.observation_frequency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.observation_calendar_id")]
    fn observation_calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: VarianceSwapBuilder| b
            .observation_calendar_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.observation_business_day_convention")]
    fn observation_business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "observation_business_day_convention")?;
        builder_set!(slf, |b: VarianceSwapBuilder| b
            .observation_business_day_convention(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.observation_end_of_month")]
    fn observation_end_of_month<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: VarianceSwapBuilder| b
            .observation_end_of_month(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.realized_var_method")]
    fn realized_var_method<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "realized_var_method")?;
        builder_set!(slf, |b: VarianceSwapBuilder| b.realized_var_method(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.trading_days_per_year")]
    fn trading_days_per_year<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: VarianceSwapBuilder| b.trading_days_per_year(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.price_series_policy")]
    fn price_series_policy<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "price_series_policy")?;
        builder_set!(slf, |b: VarianceSwapBuilder| b.price_series_policy(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.open_series_id")]
    fn open_series_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: VarianceSwapBuilder| b.open_series_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.high_series_id")]
    fn high_series_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: VarianceSwapBuilder| b.high_series_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.low_series_id")]
    fn low_series_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: VarianceSwapBuilder| b.low_series_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.close_series_id")]
    fn close_series_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: VarianceSwapBuilder| b.close_series_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.previous_close_date")]
    fn previous_close_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(crate::bindings::date_utils::py_to_date)
            .transpose()?;
        builder_set!(slf, |b: VarianceSwapBuilder| b
            .previous_close_date_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.side")]
    fn side<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "side")?;
        builder_set!(slf, |b: VarianceSwapBuilder| b.side(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: VarianceSwapBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: VarianceSwapBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_variance_swap", "VarianceSwapBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: VarianceSwapBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyVarianceSwap>()?;
    module.add_class::<PyVarianceSwapBuilder>()?;
    Ok(())
}
