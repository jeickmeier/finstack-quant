//! A commodity option on a specified spot or forward pricing basis.

use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::commodity::commodity_spot_option::{
    CommoditySpotOption, CommoditySpotOptionBuilder,
};
use pyo3::prelude::*;

use super::convert::attributes_from_py;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOption")]
#[pyclass(
    name = "CommoditySpotOption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCommoditySpotOption {
    pub(crate) inner: CommoditySpotOption,
}

instrument_envelope_methods!(
    PyCommoditySpotOption(
        "CommoditySpotOption",
        module = "finstack_quant.valuations.instruments._typed_commodity_spot_option"
    ),
    CommoditySpotOption,
    builder = PyCommoditySpotOptionBuilder("CommoditySpotOptionBuilder", no_fields),
    seed = CommoditySpotOption::builder(),
);
instrument_pricing_methods!(
    PyCommoditySpotOption,
    "CommoditySpotOption",
    module = "finstack_quant.valuations.instruments._typed_commodity_spot_option"
);

#[pymethods]
impl PyCommoditySpotOption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOption.to_builder")]
    fn to_builder(&self) -> PyCommoditySpotOptionBuilder {
        PyCommoditySpotOptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        CommoditySpotOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOption.underlying")]
    #[getter]
    fn underlying<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.underlying)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOption.strike")]
    #[getter]
    fn strike(&self) -> f64 {
        self.inner.strike
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOption.option_type")]
    #[getter]
    fn option_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.option_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOption.exercise_style")]
    #[getter]
    fn exercise_style<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.exercise_style)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOption.exercise_dates")]
    #[getter]
    fn exercise_dates<'py>(&self, py: Python<'py>) -> PyResult<Option<Vec<Bound<'py, PyAny>>>> {
        self.inner
            .exercise_dates
            .as_ref()
            .map(|dates| {
                dates
                    .iter()
                    .map(|date| crate::bindings::date_utils::date_to_py(py, *date))
                    .collect::<PyResult<Vec<_>>>()
            })
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.expiry)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOption.quantity")]
    #[getter]
    fn quantity(&self) -> f64 {
        self.inner.quantity
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOption.multiplier")]
    #[getter]
    fn multiplier(&self) -> f64 {
        self.inner.multiplier
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOption.settlement")]
    #[getter]
    fn settlement<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.settlement)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOption.exercise")]
    #[getter]
    fn exercise<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.exercise)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOption.forward_curve_id")]
    #[getter]
    fn forward_curve_id(&self) -> String {
        self.inner.forward_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOption.discount_curve_id")]
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOption.vol_surface_id")]
    #[getter]
    fn vol_surface_id(&self) -> String {
        self.inner.vol_surface_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOption.spot_id")]
    #[getter]
    fn spot_id(&self) -> Option<String> {
        self.inner.spot_id.as_ref().map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOption.quoted_forward")]
    #[getter]
    fn quoted_forward(&self) -> Option<f64> {
        self.inner.quoted_forward
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOption.day_count")]
    #[getter]
    fn day_count(&self) -> crate::bindings::core::dates::daycount::PyDayCount {
        crate::bindings::core::dates::daycount::PyDayCount::from_inner(self.inner.day_count)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOptionBuilder")]
#[pyclass(
    name = "CommoditySpotOptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyCommoditySpotOptionBuilder {
    inner: Option<CommoditySpotOptionBuilder>,
}

#[pymethods]
impl PyCommoditySpotOptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: CommoditySpotOptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOptionBuilder.underlying")]
    fn underlying<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "underlying")?;
        builder_set!(slf, |b: CommoditySpotOptionBuilder| b.underlying(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOptionBuilder.strike")]
    fn strike<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CommoditySpotOptionBuilder| b.strike(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOptionBuilder.option_type")]
    fn option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "option_type")?;
        builder_set!(slf, |b: CommoditySpotOptionBuilder| b.option_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOptionBuilder.exercise_style")]
    fn exercise_style<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "exercise_style")?;
        builder_set!(slf, |b: CommoditySpotOptionBuilder| b.exercise_style(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOptionBuilder.exercise_dates")]
    fn exercise_dates<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<Vec<Bound<'_, PyAny>>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(|dates| {
                dates
                    .iter()
                    .map(crate::bindings::date_utils::py_to_date)
                    .collect::<PyResult<Vec<_>>>()
            })
            .transpose()?;
        builder_set!(slf, |b: CommoditySpotOptionBuilder| b
            .exercise_dates_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOptionBuilder.expiry")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: CommoditySpotOptionBuilder| b.expiry(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOptionBuilder.quantity")]
    fn quantity<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CommoditySpotOptionBuilder| b.quantity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOptionBuilder.multiplier")]
    fn multiplier<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CommoditySpotOptionBuilder| b.multiplier(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOptionBuilder.settlement")]
    fn settlement<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "settlement")?;
        builder_set!(slf, |b: CommoditySpotOptionBuilder| b.settlement(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOptionBuilder.exercise")]
    fn exercise<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "exercise")?;
        builder_set!(slf, |b: CommoditySpotOptionBuilder| b.exercise_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOptionBuilder.forward_curve_id")]
    fn forward_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommoditySpotOptionBuilder| b
            .forward_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOptionBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommoditySpotOptionBuilder| b
            .discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOptionBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommoditySpotOptionBuilder| b.vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOptionBuilder.spot_id")]
    fn spot_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: CommoditySpotOptionBuilder| b.spot_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOptionBuilder.quoted_forward")]
    fn quoted_forward<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CommoditySpotOptionBuilder| b
            .quoted_forward_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOptionBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: CommoditySpotOptionBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spot_option", "CommoditySpotOptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: CommoditySpotOptionBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCommoditySpotOption>()?;
    module.add_class::<PyCommoditySpotOptionBuilder>()?;
    Ok(())
}
