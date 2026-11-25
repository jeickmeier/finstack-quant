//! An option on the spread between two constant-maturity swap rates.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::rates::cms_spread_option::{
    CmsSpreadOption, CmsSpreadOptionBuilder,
};
use pyo3::prelude::*;

use super::convert::attributes_from_py;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption")]
#[pyclass(
    name = "CmsSpreadOption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCmsSpreadOption {
    pub(crate) inner: CmsSpreadOption,
}

instrument_envelope_methods!(
    PyCmsSpreadOption(
        "CmsSpreadOption",
        module = "finstack_quant.valuations.instruments._typed_cms_spread_option"
    ),
    CmsSpreadOption,
    builder = PyCmsSpreadOptionBuilder("CmsSpreadOptionBuilder", no_fields),
    seed = CmsSpreadOption::builder(),
);
instrument_pricing_methods!(
    PyCmsSpreadOption,
    "CmsSpreadOption",
    module = "finstack_quant.valuations.instruments._typed_cms_spread_option"
);

#[pymethods]
impl PyCmsSpreadOption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.to_builder")]
    fn to_builder(&self) -> PyCmsSpreadOptionBuilder {
        PyCmsSpreadOptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        CmsSpreadOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.long_cms_tenor")]
    #[getter]
    fn long_cms_tenor<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.long_cms_tenor)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.short_cms_tenor")]
    #[getter]
    fn short_cms_tenor<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.short_cms_tenor)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.strike")]
    #[getter]
    fn strike<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::money::decimal_to_py(py, self.inner.strike)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.option_type")]
    #[getter]
    fn option_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.option_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.expiry)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.payment_date")]
    #[getter]
    fn payment_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.payment_date)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.swap_fixed_frequency")]
    #[getter]
    fn swap_fixed_frequency<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.swap_fixed_frequency)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.swap_float_frequency")]
    #[getter]
    fn swap_float_frequency<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.swap_float_frequency)
    }
}

crate::bindings::macros::py_getters!(PyCmsSpreadOption |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.notional")]
    notional -> PyMoney = PyMoney::from_inner(s.inner.notional);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.long_vol_surface_id")]
    long_vol_surface_id -> String = s.inner.long_vol_surface_id.to_string();
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.short_vol_surface_id")]
    short_vol_surface_id -> String = s.inner.short_vol_surface_id.to_string();
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.forward_curve_id")]
    forward_curve_id -> String = s.inner.forward_curve_id.to_string();
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.correlation")]
    correlation -> f64 = s.inner.correlation;
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.day_count")]
    day_count -> crate::bindings::core::dates::daycount::PyDayCount = crate::bindings::core::dates::daycount::PyDayCount::from_inner(s.inner.day_count);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.index_id")]
    index_id -> Option<String> = s.inner.index_id.as_ref().map(ToString::to_string);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.long_cms_fixing_index_id")]
    long_cms_fixing_index_id -> Option<String> = s.inner.long_cms_fixing_index_id.as_ref().map(ToString::to_string);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.short_cms_fixing_index_id")]
    short_cms_fixing_index_id -> Option<String> = s.inner.short_cms_fixing_index_id.as_ref().map(ToString::to_string);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.swap_fixed_day_count")]
    swap_fixed_day_count -> Option<crate::bindings::core::dates::daycount::PyDayCount> = s.inner.swap_fixed_day_count.map(crate::bindings::core::dates::daycount::PyDayCount::from_inner);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOption.swap_float_day_count")]
    swap_float_day_count -> Option<crate::bindings::core::dates::daycount::PyDayCount> = s.inner.swap_float_day_count.map(crate::bindings::core::dates::daycount::PyDayCount::from_inner);
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder")]
#[pyclass(
    name = "CmsSpreadOptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyCmsSpreadOptionBuilder {
    inner: Option<CmsSpreadOptionBuilder>,
}

#[pymethods]
impl PyCmsSpreadOptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.long_cms_tenor")]
    fn long_cms_tenor<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "long_cms_tenor")?;
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b.long_cms_tenor(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.short_cms_tenor")]
    fn short_cms_tenor<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "short_cms_tenor")?;
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b.short_cms_tenor(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.strike")]
    fn strike<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::money::decimal_from_py(value)?;
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b.strike(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.option_type")]
    fn option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "option_type")?;
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b.option_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.expiry")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b.expiry(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.payment_date")]
    fn payment_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b.payment_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.long_vol_surface_id")]
    fn long_vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b
            .long_vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.short_vol_surface_id")]
    fn short_vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b
            .short_vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.forward_curve_id")]
    fn forward_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b.forward_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.correlation")]
    fn correlation<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b.correlation(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.index_id")]
    fn index_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b.index_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.long_cms_fixing_index_id")]
    fn long_cms_fixing_index_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b
            .long_cms_fixing_index_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.short_cms_fixing_index_id")]
    fn short_cms_fixing_index_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b
            .short_cms_fixing_index_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.swap_fixed_frequency")]
    fn swap_fixed_frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "swap_fixed_frequency")?;
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b
            .swap_fixed_frequency_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.swap_float_frequency")]
    fn swap_float_frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "swap_float_frequency")?;
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b
            .swap_float_frequency_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.swap_fixed_day_count")]
    fn swap_fixed_day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(|term| super::convert::day_count_from_py(term, "swap_fixed_day_count"))
            .transpose()?;
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b
            .swap_fixed_day_count_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.swap_float_day_count")]
    fn swap_float_day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(|term| super::convert::day_count_from_py(term, "swap_float_day_count"))
            .transpose()?;
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b
            .swap_float_day_count_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_spread_option", "CmsSpreadOptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: CmsSpreadOptionBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCmsSpreadOption>()?;
    module.add_class::<PyCmsSpreadOptionBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["CmsSpreadOption", "CmsSpreadOptionBuilder"];
