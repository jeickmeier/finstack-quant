//! An option to enter a commodity swap.

use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::commodity::commodity_swaption::{
    CommoditySwaption, CommoditySwaptionBuilder,
};
use pyo3::prelude::*;

use super::convert::attributes_from_py;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaption")]
#[pyclass(
    name = "CommoditySwaption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCommoditySwaption {
    pub(crate) inner: CommoditySwaption,
}

instrument_envelope_methods!(
    PyCommoditySwaption(
        "CommoditySwaption",
        module = "finstack_quant.valuations.instruments._typed_commodity_swaption"
    ),
    CommoditySwaption,
    builder = PyCommoditySwaptionBuilder("CommoditySwaptionBuilder", no_fields),
    seed = CommoditySwaption::builder(),
);
instrument_pricing_methods!(
    PyCommoditySwaption,
    "CommoditySwaption",
    module = "finstack_quant.valuations.instruments._typed_commodity_swaption"
);

#[pymethods]
impl PyCommoditySwaption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaption.to_builder")]
    fn to_builder(&self) -> PyCommoditySwaptionBuilder {
        PyCommoditySwaptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        CommoditySwaption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaption.underlying")]
    #[getter]
    fn underlying<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.underlying)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaption.option_type")]
    #[getter]
    fn option_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.option_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.expiry)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaption.underlying_start_date")]
    #[getter]
    fn underlying_start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.underlying_start_date)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaption.underlying_maturity")]
    #[getter]
    fn underlying_maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.underlying_maturity)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaption.swap_frequency")]
    #[getter]
    fn swap_frequency<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.swap_frequency)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaption.fixed_price")]
    #[getter]
    fn fixed_price(&self) -> f64 {
        self.inner.fixed_price
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaption.quantity")]
    #[getter]
    fn quantity(&self) -> f64 {
        self.inner.quantity
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaption.forward_curve_id")]
    #[getter]
    fn forward_curve_id(&self) -> String {
        self.inner.forward_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaption.discount_curve_id")]
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaption.vol_surface_id")]
    #[getter]
    fn vol_surface_id(&self) -> String {
        self.inner.vol_surface_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaption.calendar_id")]
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id.as_ref().map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaption.business_day_convention")]
    #[getter]
    fn business_day_convention(&self) -> PyResult<String> {
        super::convert::enum_to_py_string(&self.inner.business_day_convention)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaption.day_count")]
    #[getter]
    fn day_count(&self) -> crate::bindings::core::dates::daycount::PyDayCount {
        crate::bindings::core::dates::daycount::PyDayCount::from_inner(self.inner.day_count)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaptionBuilder")]
#[pyclass(
    name = "CommoditySwaptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyCommoditySwaptionBuilder {
    inner: Option<CommoditySwaptionBuilder>,
}

#[pymethods]
impl PyCommoditySwaptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: CommoditySwaptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaptionBuilder.underlying")]
    fn underlying<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "underlying")?;
        builder_set!(slf, |b: CommoditySwaptionBuilder| b.underlying(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaptionBuilder.option_type")]
    fn option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "option_type")?;
        builder_set!(slf, |b: CommoditySwaptionBuilder| b.option_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaptionBuilder.expiry")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: CommoditySwaptionBuilder| b.expiry(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaptionBuilder.underlying_start_date")]
    fn underlying_start_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: CommoditySwaptionBuilder| b
            .underlying_start_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaptionBuilder.underlying_maturity")]
    fn underlying_maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: CommoditySwaptionBuilder| b
            .underlying_maturity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaptionBuilder.swap_frequency")]
    fn swap_frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "swap_frequency")?;
        builder_set!(slf, |b: CommoditySwaptionBuilder| b.swap_frequency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaptionBuilder.fixed_price")]
    fn fixed_price<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CommoditySwaptionBuilder| b.fixed_price(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaptionBuilder.quantity")]
    fn quantity<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CommoditySwaptionBuilder| b.quantity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaptionBuilder.forward_curve_id")]
    fn forward_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommoditySwaptionBuilder| b.forward_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaptionBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommoditySwaptionBuilder| b
            .discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaptionBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommoditySwaptionBuilder| b.vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaptionBuilder.calendar_id")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: CommoditySwaptionBuilder| b.calendar_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaptionBuilder.business_day_convention")]
    fn business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "business_day_convention")?;
        builder_set!(slf, |b: CommoditySwaptionBuilder| b
            .business_day_convention(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaptionBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: CommoditySwaptionBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_swaption", "CommoditySwaptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: CommoditySwaptionBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCommoditySwaption>()?;
    module.add_class::<PyCommoditySwaptionBuilder>()?;
    Ok(())
}
