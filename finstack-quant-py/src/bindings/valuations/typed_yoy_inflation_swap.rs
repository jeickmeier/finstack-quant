//! A sequence of year-on-year inflation-linked exchanges.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::rates::inflation_swap::{
    YoYInflationSwap, YoYInflationSwapBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwap")]
#[pyclass(
    name = "YoYInflationSwap",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyYoYInflationSwap {
    pub(crate) inner: YoYInflationSwap,
}

instrument_envelope_methods!(
    PyYoYInflationSwap(
        "YoYInflationSwap",
        module = "finstack_quant.valuations.instruments._typed_yoy_inflation_swap"
    ),
    YoYInflationSwap,
    builder = PyYoYInflationSwapBuilder("YoYInflationSwapBuilder", no_fields),
    seed = YoYInflationSwap::builder(),
);
instrument_pricing_methods!(
    PyYoYInflationSwap,
    "YoYInflationSwap",
    module = "finstack_quant.valuations.instruments._typed_yoy_inflation_swap"
);

#[pymethods]
impl PyYoYInflationSwap {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwap.to_builder")]
    fn to_builder(&self) -> PyYoYInflationSwapBuilder {
        PyYoYInflationSwapBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwap.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        YoYInflationSwap::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwap.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwap.start_date")]
    #[getter]
    fn start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.start_date)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwap.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.maturity)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwap.fixed_rate")]
    #[getter]
    fn fixed_rate<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::money::decimal_to_py(py, self.inner.fixed_rate)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwap.frequency")]
    #[getter]
    fn frequency<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.frequency)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwap.side")]
    #[getter]
    fn side<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.side)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwap.lag")]
    #[getter]
    fn lag<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.lag)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwap.interpolation")]
    #[getter]
    fn interpolation<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.interpolation)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwap.business_day_convention")]
    #[getter]
    fn business_day_convention(&self) -> PyResult<String> {
        super::convert::enum_to_py_string(&self.inner.business_day_convention)
    }
}

crate::bindings::macros::py_getters!(PyYoYInflationSwap |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwap.notional")]
    notional -> PyMoney = PyMoney::from_inner(s.inner.notional);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwap.inflation_index_id")]
    inflation_index_id -> String = s.inner.inflation_index_id.to_string();
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwap.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwap.day_count")]
    day_count -> crate::bindings::core::dates::daycount::PyDayCount = crate::bindings::core::dates::daycount::PyDayCount::from_inner(s.inner.day_count);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwap.base_cpi")]
    base_cpi -> Option<f64> = s.inner.base_cpi;
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwap.calendar_id")]
    calendar_id -> Option<String> = s.inner.calendar_id.as_ref().map(ToString::to_string);
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwapBuilder")]
#[pyclass(
    name = "YoYInflationSwapBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyYoYInflationSwapBuilder {
    inner: Option<YoYInflationSwapBuilder>,
}

#[pymethods]
impl PyYoYInflationSwapBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwapBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: YoYInflationSwapBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwapBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: YoYInflationSwapBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwapBuilder.start_date")]
    fn start_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: YoYInflationSwapBuilder| b.start_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwapBuilder.maturity")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: YoYInflationSwapBuilder| b.maturity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwapBuilder.fixed_rate")]
    fn fixed_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::money::decimal_from_py(value)?;
        builder_set!(slf, |b: YoYInflationSwapBuilder| b.fixed_rate(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwapBuilder.frequency")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "frequency")?;
        builder_set!(slf, |b: YoYInflationSwapBuilder| b.frequency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwapBuilder.inflation_index_id")]
    fn inflation_index_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: YoYInflationSwapBuilder| b
            .inflation_index_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwapBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: YoYInflationSwapBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwapBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: YoYInflationSwapBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwapBuilder.side")]
    fn side<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "side")?;
        builder_set!(slf, |b: YoYInflationSwapBuilder| b.side(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwapBuilder.lag")]
    fn lag<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "lag")?;
        builder_set!(slf, |b: YoYInflationSwapBuilder| b.lag_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwapBuilder.interpolation")]
    fn interpolation<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "interpolation")?;
        builder_set!(slf, |b: YoYInflationSwapBuilder| b.interpolation_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwapBuilder.base_cpi")]
    fn base_cpi<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: YoYInflationSwapBuilder| b.base_cpi_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwapBuilder.business_day_convention")]
    fn business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "business_day_convention")?;
        builder_set!(slf, |b: YoYInflationSwapBuilder| b
            .business_day_convention(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwapBuilder.calendar_id")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: YoYInflationSwapBuilder| b.calendar_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_yoy_inflation_swap", "YoYInflationSwapBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: YoYInflationSwapBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyYoYInflationSwap>()?;
    module.add_class::<PyYoYInflationSwapBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["YoYInflationSwap", "YoYInflationSwapBuilder"];
