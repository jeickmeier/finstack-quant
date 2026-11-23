//! Swaption Python bindings.

use super::swap::decimal_f64;
use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::extract_market;
use crate::bindings::module_utils::spec_from_py;
use crate::errors::core_to_py;
use finstack_quant_core::types::{CurveId, InstrumentId};

use super::super::convert::{attributes_from_py, enum_to_py_string, money_from_py, money_to_py};
use super::super::typed_legs::{PyFixedLegSpec, PyFloatLegSpec};
use crate::bindings::valuations::convert::money_repr;
use crate::bindings::valuations::convert::{enum_from_str, opt_serde_to_py};
use crate::bindings::valuations::typed_macros::{
    builder_set, instrument_envelope_methods, instrument_pricing_methods,
};

use finstack_quant_valuations::instruments::rates::swaption::SwaptionBuilder;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "Swaption")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "Swaption",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PySwaption {
    /// Inner canonical Rust swaption.
    pub(crate) inner: finstack_quant_valuations::instruments::rates::swaption::Swaption,
}

instrument_envelope_methods!(
    PySwaption(
        "Swaption",
        module = "finstack_quant.valuations.instruments._typed_rates.swaption"
    ),
    Swaption,
    builder = PySwaptionBuilder("SwaptionBuilder", fields),
    seed = finstack_quant_valuations::instruments::rates::swaption::Swaption::builder(),
);
instrument_pricing_methods!(
    PySwaption,
    "Swaption",
    module = "finstack_quant.valuations.instruments._typed_rates.swaption"
);

#[pymethods]
impl PySwaption {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "Swaption.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        Ok(Self {
            inner: finstack_quant_valuations::instruments::rates::swaption::Swaption::example()
                .map_err(core_to_py)?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "Swaption.forward_swap_rate")]
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn forward_swap_rate(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        let inner = self.inner.clone();
        py.detach(move || inner.forward_swap_rate(&market, as_of))
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "Swaption.get_strike")]
    #[pyo3(text_signature = "($self)")]
    fn get_strike(&self) -> f64 {
        decimal_f64(self.inner.get_strike())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "Swaption.get_underlying_start_date")]
    #[pyo3(text_signature = "($self)")]
    fn get_underlying_start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.get_underlying_start_date())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "Swaption.get_underlying_maturity")]
    #[pyo3(text_signature = "($self)")]
    fn get_underlying_maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.get_underlying_maturity())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "Swaption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.expiry)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "Swaption.sabr_params")]
    #[getter]
    fn sabr_params<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.sabr_params.as_ref())
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "Swaption(id={:?}, option_type={:?}, notional={}, expiry={}, strike={}, underlying_start_date={}, underlying_maturity={}, vol_surface_id={:?})",
            self.inner.id.as_str(),
            enum_to_py_string(&self.inner.option_type).unwrap_or_default(),
            money_repr(self.inner.notional),
            self.inner.expiry,
            self.inner.get_strike(),
            self.inner.get_underlying_start_date(),
            self.inner.get_underlying_maturity(),
            self.inner.vol_surface_id.as_str(),
        )
    }
}

crate::bindings::macros::py_getters!(PySwaption |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "Swaption.option_type")]
    option_type -> PyResult<String> = enum_to_py_string(&s.inner.option_type);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "Swaption.notional")]
    notional -> PyMoney = money_to_py(s.inner.notional);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "Swaption.settlement")]
    settlement -> PyResult<String> = enum_to_py_string(&s.inner.settlement);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "Swaption.cash_settlement_method")]
    cash_settlement_method -> PyResult<String> = enum_to_py_string(&s.inner.cash_settlement_method);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "Swaption.vol_model")]
    vol_model -> PyResult<String> = enum_to_py_string(&s.inner.vol_model);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "Swaption.vol_surface_id")]
    vol_surface_id -> String = s.inner.vol_surface_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "Swaption.underlying_fixed_leg")]
    underlying_fixed_leg -> PyFixedLegSpec =
        PyFixedLegSpec::from_inner(s.inner.underlying_fixed_leg.clone());

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "Swaption.underlying_float_leg")]
    underlying_float_leg -> PyFloatLegSpec =
        PyFloatLegSpec::from_inner(s.inner.underlying_float_leg.clone());
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "SwaptionBuilder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "SwaptionBuilder",
    skip_from_py_object
)]
pub struct PySwaptionBuilder {
    inner: Option<SwaptionBuilder>,
    fields: Vec<(&'static str, String)>,
}

#[pymethods]
impl PySwaptionBuilder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "SwaptionBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, id, format!("{value:?}"), |b: SwaptionBuilder| b
            .id(InstrumentId::new(value.to_string())))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "SwaptionBuilder.option_type")]
    #[pyo3(text_signature = "($self, value)")]
    fn option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let option_type = enum_from_str(value, "option_type")?;
        builder_set!(
            slf,
            option_type,
            format!("{value:?}"),
            |b: SwaptionBuilder| b.option_type(option_type)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "SwaptionBuilder.notional")]
    #[pyo3(text_signature = "($self, value)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, "notional")?;
        builder_set!(slf, notional, money_repr(money), |b: SwaptionBuilder| b
            .notional(money))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "SwaptionBuilder.expiry")]
    #[pyo3(text_signature = "($self, value)")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let expiry = py_to_date(value)?;
        builder_set!(slf, expiry, expiry.to_string(), |b: SwaptionBuilder| b
            .expiry(expiry))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "SwaptionBuilder.settlement")]
    #[pyo3(text_signature = "($self, value)")]
    fn settlement<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let settlement = enum_from_str(value, "settlement")?;
        builder_set!(
            slf,
            settlement,
            format!("{value:?}"),
            |b: SwaptionBuilder| b.settlement(settlement)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "SwaptionBuilder.cash_settlement_method")]
    #[pyo3(text_signature = "($self, value)")]
    fn cash_settlement_method<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let method = enum_from_str(value, "cash_settlement_method")?;
        builder_set!(
            slf,
            cash_settlement_method,
            format!("{value:?}"),
            |b: SwaptionBuilder| b.cash_settlement_method(method)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "SwaptionBuilder.vol_model")]
    #[pyo3(text_signature = "($self, value)")]
    fn vol_model<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let vol_model = enum_from_str(value, "vol_model")?;
        builder_set!(
            slf,
            vol_model,
            format!("{value:?}"),
            |b: SwaptionBuilder| b.vol_model(vol_model)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "SwaptionBuilder.vol_surface_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            vol_surface_id,
            format!("{value:?}"),
            |b: SwaptionBuilder| b.vol_surface_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "SwaptionBuilder.underlying_fixed_leg")]
    #[pyo3(text_signature = "($self, value)")]
    fn underlying_fixed_leg<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyFixedLegSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            underlying_fixed_leg,
            value.__repr__(),
            |b: SwaptionBuilder| b.underlying_fixed_leg(value.inner.clone())
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "SwaptionBuilder.underlying_float_leg")]
    #[pyo3(text_signature = "($self, value)")]
    fn underlying_float_leg<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyFloatLegSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            underlying_float_leg,
            value.__repr__(),
            |b: SwaptionBuilder| b.underlying_float_leg(value.inner.clone())
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "SwaptionBuilder.sabr_params")]
    #[pyo3(text_signature = "($self, value)")]
    fn sabr_params<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let sabr_params: finstack_quant_models::volatility::sabr::SabrParameters =
            spec_from_py(py, value, "sabr_params")?;
        builder_set!(
            slf,
            sabr_params,
            "{...}".to_string(),
            |b: SwaptionBuilder| b.sabr_params(sabr_params)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.swaption", "SwaptionBuilder.attributes")]
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
            |b: SwaptionBuilder| b.attributes(attrs)
        )
    }
}
