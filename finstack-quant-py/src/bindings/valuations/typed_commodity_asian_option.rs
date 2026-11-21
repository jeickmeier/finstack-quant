//! A commodity option on the documented average across a fixing schedule.

use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::commodity::commodity_asian_option::{
    CommodityAsianOption, CommodityAsianOptionBuilder,
};
use pyo3::prelude::*;

use super::convert::attributes_from_py;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOption")]
#[pyclass(
    name = "CommodityAsianOption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCommodityAsianOption {
    pub(crate) inner: CommodityAsianOption,
}

instrument_envelope_methods!(
    PyCommodityAsianOption(
        "CommodityAsianOption",
        module = "finstack_quant.valuations.instruments._typed_commodity_asian_option"
    ),
    CommodityAsianOption,
    builder = PyCommodityAsianOptionBuilder("CommodityAsianOptionBuilder", no_fields),
    seed = CommodityAsianOption::builder(),
);
instrument_pricing_methods!(
    PyCommodityAsianOption,
    "CommodityAsianOption",
    module = "finstack_quant.valuations.instruments._typed_commodity_asian_option"
);

#[pymethods]
impl PyCommodityAsianOption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOption.to_builder")]
    fn to_builder(&self) -> PyCommodityAsianOptionBuilder {
        PyCommodityAsianOptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        CommodityAsianOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOption.underlying")]
    #[getter]
    fn underlying<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.underlying)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOption.strike")]
    #[getter]
    fn strike(&self) -> f64 {
        self.inner.strike
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOption.option_type")]
    #[getter]
    fn option_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.option_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOption.averaging_method")]
    #[getter]
    fn averaging_method<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.averaging_method)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOption.fixing_dates")]
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

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOption.past_fixings")]
    #[getter]
    fn past_fixings<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, f64)>> {
        self.inner
            .past_fixings
            .iter()
            .map(|(d, x)| crate::bindings::date_utils::date_to_py(py, *d).map(|d| (d, *x)))
            .collect()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOption.quantity")]
    #[getter]
    fn quantity(&self) -> f64 {
        self.inner.quantity
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.expiry)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOption.forward_curve_id")]
    #[getter]
    fn forward_curve_id(&self) -> String {
        self.inner.forward_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOption.discount_curve_id")]
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOption.vol_surface_id")]
    #[getter]
    fn vol_surface_id(&self) -> String {
        self.inner.vol_surface_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOption.day_count")]
    #[getter]
    fn day_count(&self) -> crate::bindings::core::dates::daycount::PyDayCount {
        crate::bindings::core::dates::daycount::PyDayCount::from_inner(self.inner.day_count)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOptionBuilder")]
#[pyclass(
    name = "CommodityAsianOptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyCommodityAsianOptionBuilder {
    inner: Option<CommodityAsianOptionBuilder>,
}

#[pymethods]
impl PyCommodityAsianOptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: CommodityAsianOptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOptionBuilder.underlying")]
    fn underlying<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "underlying")?;
        builder_set!(slf, |b: CommodityAsianOptionBuilder| b.underlying(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOptionBuilder.strike")]
    fn strike<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CommodityAsianOptionBuilder| b.strike(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOptionBuilder.option_type")]
    fn option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "option_type")?;
        builder_set!(slf, |b: CommodityAsianOptionBuilder| b.option_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOptionBuilder.averaging_method")]
    fn averaging_method<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "averaging_method")?;
        builder_set!(slf, |b: CommodityAsianOptionBuilder| b
            .averaging_method(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOptionBuilder.fixing_dates")]
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
        builder_set!(slf, |b: CommodityAsianOptionBuilder| b.fixing_dates(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOptionBuilder.past_fixings")]
    fn past_fixings<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<(Bound<'_, PyAny>, f64)>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .into_iter()
            .map(|(d, x)| crate::bindings::date_utils::py_to_date(&d).map(|date| (date, x)))
            .collect::<PyResult<_>>()?;
        builder_set!(slf, |b: CommodityAsianOptionBuilder| b.past_fixings(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOptionBuilder.quantity")]
    fn quantity<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CommodityAsianOptionBuilder| b.quantity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOptionBuilder.expiry")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: CommodityAsianOptionBuilder| b.expiry(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOptionBuilder.forward_curve_id")]
    fn forward_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommodityAsianOptionBuilder| b
            .forward_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOptionBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommodityAsianOptionBuilder| b
            .discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOptionBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommodityAsianOptionBuilder| b
            .vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOptionBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: CommodityAsianOptionBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_asian_option", "CommodityAsianOptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: CommodityAsianOptionBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCommodityAsianOption>()?;
    module.add_class::<PyCommodityAsianOptionBuilder>()?;
    Ok(())
}
