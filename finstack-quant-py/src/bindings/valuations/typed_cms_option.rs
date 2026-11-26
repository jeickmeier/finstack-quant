//! An option on the elected constant-maturity swap rate.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::rates::cms_option::{CmsOption, CmsOptionBuilder};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_trait_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption")]
#[pyclass(
    name = "CmsOption",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCmsOption {
    pub(crate) inner: CmsOption,
}

instrument_envelope_methods!(
    PyCmsOption(
        "CmsOption",
        module = "finstack_quant.valuations.instruments._typed_cms_option"
    ),
    CmsOption,
    builder = PyCmsOptionBuilder("CmsOptionBuilder", no_fields),
    seed = CmsOption::builder(),
);
instrument_trait_methods!(
    PyCmsOption,
    "CmsOption",
    module = "finstack_quant.valuations.instruments._typed_cms_option"
);

#[pymethods]
impl PyCmsOption {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.to_builder")]
    fn to_builder(&self) -> PyCmsOptionBuilder {
        PyCmsOptionBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        CmsOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.strike")]
    #[getter]
    fn strike<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::money::decimal_to_py(py, self.inner.strike)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.cms_tenor")]
    #[getter]
    fn cms_tenor<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.cms_tenor)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.fixing_dates")]
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

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.payment_dates")]
    #[getter]
    fn payment_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        {
            let dates = &self.inner.payment_dates;
            dates
                .iter()
                .map(|date| crate::bindings::date_utils::date_to_py(py, *date))
                .collect::<PyResult<Vec<_>>>()
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.accrual_fractions")]
    #[getter]
    fn accrual_fractions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.accrual_fractions)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.option_type")]
    #[getter]
    fn option_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.option_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.swap_fixed_frequency")]
    #[getter]
    fn swap_fixed_frequency<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.swap_fixed_frequency)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.swap_float_frequency")]
    #[getter]
    fn swap_float_frequency<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.swap_float_frequency)
    }
}

crate::bindings::macros::py_getters!(PyCmsOption |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.notional")]
    notional -> PyMoney = PyMoney::from_inner(s.inner.notional);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.day_count")]
    day_count -> crate::bindings::core::dates::daycount::PyDayCount = crate::bindings::core::dates::daycount::PyDayCount::from_inner(s.inner.day_count);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.index_id")]
    index_id -> Option<String> = s.inner.index_id.as_ref().map(ToString::to_string);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.cms_fixing_index_id")]
    cms_fixing_index_id -> Option<String> = s.inner.cms_fixing_index_id.as_ref().map(ToString::to_string);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.swap_fixed_day_count")]
    swap_fixed_day_count -> Option<crate::bindings::core::dates::daycount::PyDayCount> = s.inner.swap_fixed_day_count.map(crate::bindings::core::dates::daycount::PyDayCount::from_inner);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.swap_float_day_count")]
    swap_float_day_count -> Option<crate::bindings::core::dates::daycount::PyDayCount> = s.inner.swap_float_day_count.map(crate::bindings::core::dates::daycount::PyDayCount::from_inner);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.forward_curve_id")]
    forward_curve_id -> String = s.inner.forward_curve_id.to_string();
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOption.vol_surface_id")]
    vol_surface_id -> String = s.inner.vol_surface_id.to_string();
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder")]
#[pyclass(
    name = "CmsOptionBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyCmsOptionBuilder {
    inner: Option<CmsOptionBuilder>,
}

#[pymethods]
impl PyCmsOptionBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: CmsOptionBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder.strike")]
    fn strike<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::money::decimal_from_py(value)?;
        builder_set!(slf, |b: CmsOptionBuilder| b.strike(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder.cms_tenor")]
    fn cms_tenor<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "cms_tenor")?;
        builder_set!(slf, |b: CmsOptionBuilder| b.cms_tenor(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder.fixing_dates")]
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
        builder_set!(slf, |b: CmsOptionBuilder| b.fixing_dates(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder.payment_dates")]
    fn payment_dates<'py>(
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
        builder_set!(slf, |b: CmsOptionBuilder| b.payment_dates(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder.accrual_fractions")]
    fn accrual_fractions<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "accrual_fractions")?;
        builder_set!(slf, |b: CmsOptionBuilder| b.accrual_fractions(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder.option_type")]
    fn option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "option_type")?;
        builder_set!(slf, |b: CmsOptionBuilder| b.option_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: CmsOptionBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: CmsOptionBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder.index_id")]
    fn index_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: CmsOptionBuilder| b.index_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder.cms_fixing_index_id")]
    fn cms_fixing_index_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: CmsOptionBuilder| b.cms_fixing_index_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder.swap_fixed_frequency")]
    fn swap_fixed_frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "swap_fixed_frequency")?;
        builder_set!(slf, |b: CmsOptionBuilder| b.swap_fixed_frequency_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder.swap_float_frequency")]
    fn swap_float_frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "swap_float_frequency")?;
        builder_set!(slf, |b: CmsOptionBuilder| b.swap_float_frequency_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder.swap_fixed_day_count")]
    fn swap_fixed_day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(|term| super::convert::day_count_from_py(term, "swap_fixed_day_count"))
            .transpose()?;
        builder_set!(slf, |b: CmsOptionBuilder| b.swap_fixed_day_count_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder.swap_float_day_count")]
    fn swap_float_day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(|term| super::convert::day_count_from_py(term, "swap_float_day_count"))
            .transpose()?;
        builder_set!(slf, |b: CmsOptionBuilder| b.swap_float_day_count_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CmsOptionBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder.forward_curve_id")]
    fn forward_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CmsOptionBuilder| b.forward_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CmsOptionBuilder| b.vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_option", "CmsOptionBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: CmsOptionBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCmsOption>()?;
    module.add_class::<PyCmsOptionBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["CmsOption", "CmsOptionBuilder"];
