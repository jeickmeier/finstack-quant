//! An option on the spread between two commodity prices.

use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::commodity::commodity_spread_option::{
    CommoditySpreadOption, CommoditySpreadOptionBuilder,
};
use pyo3::prelude::*;

use super::convert::attributes_from_py;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOption")]
#[pyclass(
    name = "CommoditySpreadOption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCommoditySpreadOption {
    pub(crate) inner: CommoditySpreadOption,
}

instrument_envelope_methods!(
    PyCommoditySpreadOption(
        "CommoditySpreadOption",
        module = "finstack_quant.valuations.instruments._typed_commodity_spread_option"
    ),
    CommoditySpreadOption,
    builder = PyCommoditySpreadOptionBuilder("CommoditySpreadOptionBuilder", no_fields),
    seed = CommoditySpreadOption::builder(),
);
instrument_pricing_methods!(
    PyCommoditySpreadOption,
    "CommoditySpreadOption",
    module = "finstack_quant.valuations.instruments._typed_commodity_spread_option"
);

#[pymethods]
impl PyCommoditySpreadOption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOption.to_builder")]
    fn to_builder(&self) -> PyCommoditySpreadOptionBuilder {
        PyCommoditySpreadOptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        CommoditySpreadOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOption.currency")]
    #[getter]
    fn currency(&self) -> String {
        self.inner.currency.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOption.option_type")]
    #[getter]
    fn option_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.option_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.expiry)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOption.strike")]
    #[getter]
    fn strike(&self) -> f64 {
        self.inner.strike
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOption.quantity")]
    #[getter]
    fn quantity(&self) -> f64 {
        self.inner.quantity
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOption.leg1_forward_curve_id")]
    #[getter]
    fn leg1_forward_curve_id(&self) -> String {
        self.inner.leg1_forward_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOption.leg2_forward_curve_id")]
    #[getter]
    fn leg2_forward_curve_id(&self) -> String {
        self.inner.leg2_forward_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOption.leg1_vol_surface_id")]
    #[getter]
    fn leg1_vol_surface_id(&self) -> String {
        self.inner.leg1_vol_surface_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOption.leg2_vol_surface_id")]
    #[getter]
    fn leg2_vol_surface_id(&self) -> String {
        self.inner.leg2_vol_surface_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOption.discount_curve_id")]
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOption.correlation")]
    #[getter]
    fn correlation(&self) -> f64 {
        self.inner.correlation
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOption.day_count")]
    #[getter]
    fn day_count(&self) -> crate::bindings::core::dates::daycount::PyDayCount {
        crate::bindings::core::dates::daycount::PyDayCount::from_inner(self.inner.day_count)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOptionBuilder")]
#[pyclass(
    name = "CommoditySpreadOptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyCommoditySpreadOptionBuilder {
    inner: Option<CommoditySpreadOptionBuilder>,
}

#[pymethods]
impl PyCommoditySpreadOptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: CommoditySpreadOptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOptionBuilder.currency")]
    fn currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(slf, |b: CommoditySpreadOptionBuilder| b.currency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOptionBuilder.option_type")]
    fn option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "option_type")?;
        builder_set!(slf, |b: CommoditySpreadOptionBuilder| b.option_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOptionBuilder.expiry")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: CommoditySpreadOptionBuilder| b.expiry(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOptionBuilder.strike")]
    fn strike<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CommoditySpreadOptionBuilder| b.strike(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOptionBuilder.quantity")]
    fn quantity<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CommoditySpreadOptionBuilder| b.quantity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOptionBuilder.leg1_forward_curve_id")]
    fn leg1_forward_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommoditySpreadOptionBuilder| b
            .leg1_forward_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOptionBuilder.leg2_forward_curve_id")]
    fn leg2_forward_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommoditySpreadOptionBuilder| b
            .leg2_forward_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOptionBuilder.leg1_vol_surface_id")]
    fn leg1_vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommoditySpreadOptionBuilder| b
            .leg1_vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOptionBuilder.leg2_vol_surface_id")]
    fn leg2_vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommoditySpreadOptionBuilder| b
            .leg2_vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOptionBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CommoditySpreadOptionBuilder| b
            .discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOptionBuilder.correlation")]
    fn correlation<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CommoditySpreadOptionBuilder| b.correlation(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOptionBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: CommoditySpreadOptionBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_commodity_spread_option", "CommoditySpreadOptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: CommoditySpreadOptionBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCommoditySpreadOption>()?;
    module.add_class::<PyCommoditySpreadOptionBuilder>()?;
    Ok(())
}
