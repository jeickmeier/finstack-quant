//! InterestRateSwap Python bindings.

use pyo3::prelude::*;
use rust_decimal::prelude::ToPrimitive;

use crate::bindings::core::money::PyMoney;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;

use super::super::convert::{attributes_from_py, enum_to_py_string, money_from_py, money_to_py};
use super::super::typed_legs::{PyFixedLegSpec, PyFloatLegSpec};
use crate::bindings::valuations::convert::money_repr;
use crate::bindings::valuations::convert::{enum_from_str, instrument_expiry, opt_serde_to_py};
use crate::bindings::valuations::typed_macros::{
    builder_set, instrument_envelope_methods, instrument_trait_methods,
};

use finstack_quant_valuations::instruments::rates::irs::InterestRateSwapBuilder;

/// Render a `Decimal` as a Python float literal for reprs.
pub(super) fn decimal_f64(value: rust_decimal::Decimal) -> f64 {
    value.to_f64().unwrap_or(f64::NAN)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwap")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "InterestRateSwap",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyInterestRateSwap {
    /// Inner canonical Rust swap.
    pub(crate) inner: finstack_quant_valuations::instruments::rates::irs::InterestRateSwap,
}

instrument_envelope_methods!(
    PyInterestRateSwap(
        "InterestRateSwap",
        module = "finstack_quant.valuations.instruments._typed_rates.swap"
    ),
    InterestRateSwap,
    builder = PyInterestRateSwapBuilder("InterestRateSwapBuilder", fields),
    seed = finstack_quant_valuations::instruments::rates::irs::InterestRateSwap::builder(),
);
instrument_trait_methods!(
    PyInterestRateSwap,
    "InterestRateSwap",
    module = "finstack_quant.valuations.instruments._typed_rates.swap"
);

#[pymethods]
impl PyInterestRateSwap {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwap.from_conventions")]
    #[staticmethod]
    #[pyo3(text_signature = "(params)")]
    fn from_conventions(params: &Bound<'_, PyAny>) -> PyResult<Self> {
        let params = crate::bindings::module_utils::py_to_serde::<
            finstack_quant_valuations::instruments::rates::irs::ConventionSwapParams,
        >(params.py(), params, "ConventionSwapParams")?;
        finstack_quant_valuations::instruments::rates::irs::InterestRateSwap::from_conventions(
            params,
        )
        .map(|inner| Self { inner })
        .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwap.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::rates::irs::InterestRateSwap::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwap.margin_spec")]
    #[getter]
    fn margin_spec<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.margin_spec.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwap.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwap.to_builder")]
    fn to_builder(&self) -> PyInterestRateSwapBuilder {
        PyInterestRateSwapBuilder {
            inner: Some(self.inner.to_builder()),
            fields: Vec::new(),
        }
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "InterestRateSwap(id={:?}, notional={}, side={:?}, fixed_rate={}, start={}, end={}, forward_curve_id={:?})",
            self.inner.id.as_str(),
            money_repr(self.inner.notional),
            enum_to_py_string(&self.inner.side).unwrap_or_default(),
            self.inner.fixed_leg.rate,
            self.inner.fixed_leg.start,
            self.inner.fixed_leg.end,
            self.inner.float_leg.forward_curve_id.as_str(),
        )
    }
}

crate::bindings::macros::py_getters!(PyInterestRateSwap |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwap.notional")]
    notional -> PyMoney = money_to_py(s.inner.notional);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwap.side")]
    side -> PyResult<String> = enum_to_py_string(&s.inner.side);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwap.fixed_leg")]
    fixed_leg -> PyFixedLegSpec = PyFixedLegSpec::from_inner(s.inner.fixed_leg.clone());

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwap.float_leg")]
    float_leg -> PyFloatLegSpec = PyFloatLegSpec::from_inner(s.inner.float_leg.clone());

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwap.adjust_fixed_accrual_dates")]
    adjust_fixed_accrual_dates -> bool = s.inner.adjust_fixed_accrual_dates;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwap.adjust_float_accrual_dates")]
    adjust_float_accrual_dates -> bool = s.inner.adjust_float_accrual_dates;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwapBuilder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "InterestRateSwapBuilder",
    skip_from_py_object
)]
pub struct PyInterestRateSwapBuilder {
    inner: Option<InterestRateSwapBuilder>,
    fields: Vec<(&'static str, String)>,
}

#[pymethods]
impl PyInterestRateSwapBuilder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwapBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            id,
            format!("{value:?}"),
            |b: InterestRateSwapBuilder| b.id(InstrumentId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwapBuilder.notional")]
    #[pyo3(text_signature = "($self, value)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, "notional")?;
        builder_set!(
            slf,
            notional,
            money_repr(money),
            |b: InterestRateSwapBuilder| b.notional(money)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwapBuilder.side")]
    #[pyo3(text_signature = "($self, value)")]
    fn side<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let side = enum_from_str(value, "side")?;
        builder_set!(
            slf,
            side,
            format!("{value:?}"),
            |b: InterestRateSwapBuilder| b.side(side)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwapBuilder.fixed_leg")]
    #[pyo3(text_signature = "($self, value)")]
    fn fixed_leg<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyFixedLegSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            fixed_leg,
            value.__repr__(),
            |b: InterestRateSwapBuilder| b.fixed_leg(value.inner.clone())
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwapBuilder.float_leg")]
    #[pyo3(text_signature = "($self, value)")]
    fn float_leg<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyFloatLegSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            float_leg,
            value.__repr__(),
            |b: InterestRateSwapBuilder| b.float_leg(value.inner.clone())
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwapBuilder.adjust_fixed_accrual_dates")]
    #[pyo3(text_signature = "($self, value)")]
    fn adjust_fixed_accrual_dates<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            adjust_fixed_accrual_dates,
            value.to_string(),
            |b: InterestRateSwapBuilder| b.adjust_fixed_accrual_dates(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwapBuilder.adjust_float_accrual_dates")]
    #[pyo3(text_signature = "($self, value)")]
    fn adjust_float_accrual_dates<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            adjust_float_accrual_dates,
            value.to_string(),
            |b: InterestRateSwapBuilder| b.adjust_float_accrual_dates(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwapBuilder.margin_spec")]
    #[pyo3(text_signature = "($self, value)")]
    fn margin_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec = crate::bindings::margin::types::extract_otc_margin_spec(py, value)?;
        builder_set!(
            slf,
            margin_spec,
            "{...}".to_string(),
            |b: InterestRateSwapBuilder| b.margin_spec(spec)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swap", "InterestRateSwapBuilder.attributes")]
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        builder_set!(
            slf,
            attributes,
            "Attributes(...)".to_string(),
            |b: InterestRateSwapBuilder| b.attributes(attrs)
        )
    }
}
