//! A constant-maturity swap on a specified swap-rate index.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::rates::cms_swap::{CmsSwap, CmsSwapBuilder};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap")]
#[pyclass(
    name = "CmsSwap",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCmsSwap {
    pub(crate) inner: CmsSwap,
}

instrument_envelope_methods!(
    PyCmsSwap(
        "CmsSwap",
        module = "finstack_quant.valuations.instruments._typed_cms_swap"
    ),
    CmsSwap,
    builder = PyCmsSwapBuilder("CmsSwapBuilder", no_fields),
    seed = CmsSwap::builder(),
);
instrument_pricing_methods!(
    PyCmsSwap,
    "CmsSwap",
    module = "finstack_quant.valuations.instruments._typed_cms_swap"
);

#[pymethods]
impl PyCmsSwap {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.to_builder")]
    fn to_builder(&self) -> PyCmsSwapBuilder {
        PyCmsSwapBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        CmsSwap::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.side")]
    #[getter]
    fn side<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.side)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.cms_tenor")]
    #[getter]
    fn cms_tenor<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.cms_tenor)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.fixing_dates")]
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

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.payment_dates")]
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

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.accrual_fractions")]
    #[getter]
    fn accrual_fractions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.accrual_fractions)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.cms_spread_bp")]
    #[getter]
    fn cms_spread_bp<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::money::decimal_to_py(py, self.inner.cms_spread_bp)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.swap_fixed_frequency")]
    #[getter]
    fn swap_fixed_frequency<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.swap_fixed_frequency)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.swap_float_frequency")]
    #[getter]
    fn swap_float_frequency<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.swap_float_frequency)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.funding_leg")]
    #[getter]
    fn funding_leg<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.funding_leg)
    }
}

crate::bindings::macros::py_getters!(PyCmsSwap |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.notional")]
    notional -> PyMoney = PyMoney::from_inner(s.inner.notional);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.day_count")]
    day_count -> crate::bindings::core::dates::daycount::PyDayCount = crate::bindings::core::dates::daycount::PyDayCount::from_inner(s.inner.day_count);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.cms_cap")]
    cms_cap -> Option<f64> = s.inner.cms_cap;
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.cms_floor")]
    cms_floor -> Option<f64> = s.inner.cms_floor;
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.index_id")]
    index_id -> Option<String> = s.inner.index_id.as_ref().map(ToString::to_string);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.cms_fixing_index_id")]
    cms_fixing_index_id -> Option<String> = s.inner.cms_fixing_index_id.as_ref().map(ToString::to_string);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.swap_fixed_day_count")]
    swap_fixed_day_count -> Option<crate::bindings::core::dates::daycount::PyDayCount> = s.inner.swap_fixed_day_count.map(crate::bindings::core::dates::daycount::PyDayCount::from_inner);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.swap_float_day_count")]
    swap_float_day_count -> Option<crate::bindings::core::dates::daycount::PyDayCount> = s.inner.swap_float_day_count.map(crate::bindings::core::dates::daycount::PyDayCount::from_inner);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.forward_curve_id")]
    forward_curve_id -> String = s.inner.forward_curve_id.to_string();
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwap.vol_surface_id")]
    vol_surface_id -> String = s.inner.vol_surface_id.to_string();
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder")]
#[pyclass(
    name = "CmsSwapBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyCmsSwapBuilder {
    inner: Option<CmsSwapBuilder>,
}

#[pymethods]
impl PyCmsSwapBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: CmsSwapBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: CmsSwapBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.side")]
    fn side<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "side")?;
        builder_set!(slf, |b: CmsSwapBuilder| b.side(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.cms_tenor")]
    fn cms_tenor<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "cms_tenor")?;
        builder_set!(slf, |b: CmsSwapBuilder| b.cms_tenor(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.fixing_dates")]
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
        builder_set!(slf, |b: CmsSwapBuilder| b.fixing_dates(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.payment_dates")]
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
        builder_set!(slf, |b: CmsSwapBuilder| b.payment_dates(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.accrual_fractions")]
    fn accrual_fractions<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "accrual_fractions")?;
        builder_set!(slf, |b: CmsSwapBuilder| b.accrual_fractions(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: CmsSwapBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.cms_spread_bp")]
    fn cms_spread_bp<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::money::decimal_from_py(value)?;
        builder_set!(slf, |b: CmsSwapBuilder| b.cms_spread_bp(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.cms_cap")]
    fn cms_cap<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CmsSwapBuilder| b.cms_cap_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.cms_floor")]
    fn cms_floor<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: CmsSwapBuilder| b.cms_floor_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.index_id")]
    fn index_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: CmsSwapBuilder| b.index_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.cms_fixing_index_id")]
    fn cms_fixing_index_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: CmsSwapBuilder| b.cms_fixing_index_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.swap_fixed_frequency")]
    fn swap_fixed_frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "swap_fixed_frequency")?;
        builder_set!(slf, |b: CmsSwapBuilder| b.swap_fixed_frequency_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.swap_float_frequency")]
    fn swap_float_frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "swap_float_frequency")?;
        builder_set!(slf, |b: CmsSwapBuilder| b.swap_float_frequency_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.swap_fixed_day_count")]
    fn swap_fixed_day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(|term| super::convert::day_count_from_py(term, "swap_fixed_day_count"))
            .transpose()?;
        builder_set!(slf, |b: CmsSwapBuilder| b.swap_fixed_day_count_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.swap_float_day_count")]
    fn swap_float_day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value
            .map(|term| super::convert::day_count_from_py(term, "swap_float_day_count"))
            .transpose()?;
        builder_set!(slf, |b: CmsSwapBuilder| b.swap_float_day_count_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.funding_leg")]
    fn funding_leg<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "funding_leg")?;
        builder_set!(slf, |b: CmsSwapBuilder| b.funding_leg(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CmsSwapBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.forward_curve_id")]
    fn forward_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CmsSwapBuilder| b.forward_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: CmsSwapBuilder| b.vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_cms_swap", "CmsSwapBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: CmsSwapBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCmsSwap>()?;
    module.add_class::<PyCmsSwapBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["CmsSwap", "CmsSwapBuilder"];
