//! Typed rates instruments: `InterestRateSwap`, `Swaption` and `CapFloor`.
//!
//! Mirrors the `PyBond` pattern in `instruments.rs`: frozen wrappers with one
//! getter per public Rust field, the serde surface (`to_json` / `from_json` /
//! pickle / `to_dict`), `price` / `metric` through the canonical pricer, and
//! consuming builders that wrap the Rust `FinancialBuilder` output one setter
//! for one setter.

use pyo3::prelude::*;
use rust_decimal::prelude::ToPrimitive;

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::schedule::PyStubKind;
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::extract_market;
use crate::bindings::module_utils::spec_from_py;
use crate::errors::core_to_py;
use finstack_quant_core::types::{CalendarId, CurveId, InstrumentId};

use super::convert::{attributes_from_py, enum_to_py_string, money_from_py, money_to_py};
use super::typed_legs::{PyFixedLegSpec, PyFloatLegSpec};
use crate::bindings::extract::extract_rate_decimal;
use crate::bindings::valuations::convert::money_repr;
use crate::bindings::valuations::convert::{
    decimal_from_f64, enum_from_str, instrument_expiry, opt_serde_to_py, stub_kind_from_py,
};
use crate::bindings::valuations::typed_macros::{
    builder_set, instrument_envelope_methods, instrument_pricing_methods,
};

use finstack_quant_margin::types::OtcMarginSpec;
use finstack_quant_valuations::instruments::rates::cap_floor::CapFloorBuilder;
use finstack_quant_valuations::instruments::rates::irs::InterestRateSwapBuilder;
use finstack_quant_valuations::instruments::rates::swaption::SwaptionBuilder;

/// Render a `Decimal` as a Python float literal for reprs.
fn decimal_f64(value: rust_decimal::Decimal) -> f64 {
    value.to_f64().unwrap_or(f64::NAN)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwap")]
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
        module = "finstack_quant.valuations.instruments._typed_rates"
    ),
    InterestRateSwap,
    builder = PyInterestRateSwapBuilder("InterestRateSwapBuilder", fields),
    seed = finstack_quant_valuations::instruments::rates::irs::InterestRateSwap::builder(),
);
instrument_pricing_methods!(
    PyInterestRateSwap,
    "InterestRateSwap",
    module = "finstack_quant.valuations.instruments._typed_rates"
);

#[pymethods]
impl PyInterestRateSwap {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwap.from_conventions")]
    #[staticmethod]
    #[pyo3(signature = (id, notional, side, fixed_rate, start_date, maturity, index_id, discount_curve_id, forward_curve_id, *, currency = None))]
    #[pyo3(
        text_signature = "(id, notional, side, fixed_rate, start_date, maturity, index_id, discount_curve_id, forward_curve_id, *, currency=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn from_conventions(
        id: &str,
        notional: &Bound<'_, PyAny>,
        side: &str,
        fixed_rate: &Bound<'_, PyAny>,
        start_date: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        index_id: &str,
        discount_curve_id: &str,
        forward_curve_id: &str,
        currency: Option<&str>,
    ) -> PyResult<Self> {
        let params = finstack_quant_valuations::instruments::rates::irs::ConventionSwapParams {
            id: InstrumentId::new(id.to_string()),
            notional: money_from_py(notional, currency, "notional")?,
            side: enum_from_str(side, "side")?,
            fixed_rate: extract_rate_decimal(fixed_rate, "fixed_rate")?,
            start_date: py_to_date(start_date)?,
            maturity: py_to_date(maturity)?,
            index_id,
            discount_curve_id,
            forward_curve_id,
        };
        let inner =
            finstack_quant_valuations::instruments::rates::irs::InterestRateSwap::from_conventions(
                params,
            )
            .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwap.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::rates::irs::InterestRateSwap::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwap.margin_spec")]
    #[getter]
    fn margin_spec<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.margin_spec.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwap.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwap.notional")]
    notional -> PyMoney = money_to_py(s.inner.notional);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwap.side")]
    side -> PyResult<String> = enum_to_py_string(&s.inner.side);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwap.fixed_leg")]
    fixed_leg -> PyFixedLegSpec = PyFixedLegSpec::from_inner(s.inner.fixed_leg.clone());

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwap.float_leg")]
    float_leg -> PyFloatLegSpec = PyFloatLegSpec::from_inner(s.inner.float_leg.clone());

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwap.adjust_fixed_accrual_dates")]
    adjust_fixed_accrual_dates -> bool = s.inner.adjust_fixed_accrual_dates;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwap.adjust_float_accrual_dates")]
    adjust_float_accrual_dates -> bool = s.inner.adjust_float_accrual_dates;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwapBuilder")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwapBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            id,
            format!("{value:?}"),
            |b: InterestRateSwapBuilder| b.id(InstrumentId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwapBuilder.notional")]
    #[pyo3(signature = (value, currency = None))]
    #[pyo3(text_signature = "($self, value, currency=None)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
        currency: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, currency, "notional")?;
        builder_set!(
            slf,
            notional,
            money_repr(money),
            |b: InterestRateSwapBuilder| b.notional(money)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwapBuilder.side")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwapBuilder.fixed_leg")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwapBuilder.float_leg")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwapBuilder.adjust_fixed_accrual_dates")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwapBuilder.adjust_float_accrual_dates")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwapBuilder.margin_spec")]
    #[pyo3(text_signature = "($self, value)")]
    fn margin_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: OtcMarginSpec = spec_from_py(py, value, "margin_spec")?;
        builder_set!(
            slf,
            margin_spec,
            "{...}".to_string(),
            |b: InterestRateSwapBuilder| b.margin_spec(spec)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "InterestRateSwapBuilder.attributes")]
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

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "Swaption")]
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
        module = "finstack_quant.valuations.instruments._typed_rates"
    ),
    Swaption,
    builder = PySwaptionBuilder("SwaptionBuilder", fields),
    seed = finstack_quant_valuations::instruments::rates::swaption::Swaption::builder(),
);
instrument_pricing_methods!(
    PySwaption,
    "Swaption",
    module = "finstack_quant.valuations.instruments._typed_rates"
);

#[pymethods]
impl PySwaption {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "Swaption.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        Ok(Self {
            inner: finstack_quant_valuations::instruments::rates::swaption::Swaption::example()
                .map_err(core_to_py)?,
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "Swaption.forward_swap_rate")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "Swaption.get_strike")]
    #[pyo3(text_signature = "($self)")]
    fn get_strike(&self) -> f64 {
        decimal_f64(self.inner.get_strike())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "Swaption.get_underlying_start_date")]
    #[pyo3(text_signature = "($self)")]
    fn get_underlying_start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.get_underlying_start_date())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "Swaption.get_underlying_maturity")]
    #[pyo3(text_signature = "($self)")]
    fn get_underlying_maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.get_underlying_maturity())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "Swaption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.expiry)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "Swaption.sabr_params")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "Swaption.option_type")]
    option_type -> PyResult<String> = enum_to_py_string(&s.inner.option_type);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "Swaption.notional")]
    notional -> PyMoney = money_to_py(s.inner.notional);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "Swaption.settlement")]
    settlement -> PyResult<String> = enum_to_py_string(&s.inner.settlement);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "Swaption.cash_settlement_method")]
    cash_settlement_method -> PyResult<String> = enum_to_py_string(&s.inner.cash_settlement_method);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "Swaption.vol_model")]
    vol_model -> PyResult<String> = enum_to_py_string(&s.inner.vol_model);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "Swaption.vol_surface_id")]
    vol_surface_id -> String = s.inner.vol_surface_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "Swaption.underlying_fixed_leg")]
    underlying_fixed_leg -> PyFixedLegSpec =
        PyFixedLegSpec::from_inner(s.inner.underlying_fixed_leg.clone());

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "Swaption.underlying_float_leg")]
    underlying_float_leg -> PyFloatLegSpec =
        PyFloatLegSpec::from_inner(s.inner.underlying_float_leg.clone());
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "SwaptionBuilder")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "SwaptionBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, id, format!("{value:?}"), |b: SwaptionBuilder| b
            .id(InstrumentId::new(value.to_string())))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "SwaptionBuilder.option_type")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "SwaptionBuilder.notional")]
    #[pyo3(signature = (value, currency = None))]
    #[pyo3(text_signature = "($self, value, currency=None)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
        currency: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, currency, "notional")?;
        builder_set!(slf, notional, money_repr(money), |b: SwaptionBuilder| b
            .notional(money))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "SwaptionBuilder.expiry")]
    #[pyo3(text_signature = "($self, value)")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let expiry = py_to_date(value)?;
        builder_set!(slf, expiry, expiry.to_string(), |b: SwaptionBuilder| b
            .expiry(expiry))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "SwaptionBuilder.settlement")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "SwaptionBuilder.cash_settlement_method")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "SwaptionBuilder.vol_model")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "SwaptionBuilder.vol_surface_id")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "SwaptionBuilder.underlying_fixed_leg")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "SwaptionBuilder.underlying_float_leg")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "SwaptionBuilder.sabr_params")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "SwaptionBuilder.attributes")]
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

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CapFloor",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCapFloor {
    /// Inner canonical Rust cap/floor.
    pub(crate) inner: finstack_quant_valuations::instruments::rates::cap_floor::CapFloor,
}

instrument_envelope_methods!(
    PyCapFloor(
        "CapFloor",
        module = "finstack_quant.valuations.instruments._typed_rates"
    ),
    CapFloor,
    builder = PyCapFloorBuilder("CapFloorBuilder", fields),
    seed = finstack_quant_valuations::instruments::rates::cap_floor::CapFloor::builder(),
);
instrument_pricing_methods!(
    PyCapFloor,
    "CapFloor",
    module = "finstack_quant.valuations.instruments._typed_rates"
);

#[pymethods]
impl PyCapFloor {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::rates::cap_floor::CapFloor::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.start_date")]
    #[getter]
    fn start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start_date)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.overnight_coupon")]
    #[getter]
    fn overnight_coupon<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.overnight_coupon.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.premium")]
    #[getter]
    fn premium<'py>(&self, py: Python<'py>) -> PyResult<Option<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .premium
            .map(|(date, amount)| Ok((date_to_py(py, date)?, money_to_py(amount))))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CapFloor(id={:?}, rate_option_type={:?}, notional={}, strike={}, start_date={}, maturity={}, forward_curve_id={:?}, vol_surface_id={:?})",
            self.inner.id.as_str(),
            enum_to_py_string(&self.inner.rate_option_type).unwrap_or_default(),
            money_repr(self.inner.notional),
            self.inner.strike,
            self.inner.start_date,
            self.inner.maturity,
            self.inner.forward_curve_id.as_str(),
            self.inner.vol_surface_id.as_str(),
        )
    }
}

crate::bindings::macros::py_getters!(PyCapFloor |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.rate_option_type")]
    rate_option_type -> PyResult<String> = enum_to_py_string(&s.inner.rate_option_type);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.notional")]
    notional -> PyMoney = money_to_py(s.inner.notional);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.strike")]
    strike -> f64 = decimal_f64(s.inner.strike);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.spread_bp")]
    spread_bp -> f64 = decimal_f64(s.inner.spread_bp);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.frequency")]
    frequency -> PyTenor = PyTenor::from_inner(s.inner.frequency);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.day_count")]
    day_count -> PyDayCount = PyDayCount::from_inner(s.inner.day_count);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.stub")]
    stub -> PyStubKind = PyStubKind::from_inner(s.inner.stub);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.business_day_convention")]
    business_day_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.business_day_convention);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.calendar_id")]
    calendar_id -> Option<String> = s.inner.calendar_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.exercise_style")]
    exercise_style -> PyResult<String> = enum_to_py_string(&s.inner.exercise_style);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.settlement")]
    settlement -> PyResult<String> = enum_to_py_string(&s.inner.settlement);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.forward_curve_id")]
    forward_curve_id -> String = s.inner.forward_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.vol_surface_id")]
    vol_surface_id -> String = s.inner.vol_surface_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.vol_type")]
    vol_type -> PyResult<String> = enum_to_py_string(&s.inner.vol_type);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloor.vol_shift")]
    vol_shift -> f64 = s.inner.vol_shift;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CapFloorBuilder",
    skip_from_py_object
)]
pub struct PyCapFloorBuilder {
    inner: Option<CapFloorBuilder>,
    fields: Vec<(&'static str, String)>,
}

#[pymethods]
impl PyCapFloorBuilder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, id, format!("{value:?}"), |b: CapFloorBuilder| b
            .id(InstrumentId::new(value.to_string())))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.rate_option_type")]
    #[pyo3(text_signature = "($self, value)")]
    fn rate_option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let rate_option_type = enum_from_str(value, "rate_option_type")?;
        builder_set!(
            slf,
            rate_option_type,
            format!("{value:?}"),
            |b: CapFloorBuilder| b.rate_option_type(rate_option_type)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.notional")]
    #[pyo3(signature = (value, currency = None))]
    #[pyo3(text_signature = "($self, value, currency=None)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
        currency: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, currency, "notional")?;
        builder_set!(slf, notional, money_repr(money), |b: CapFloorBuilder| b
            .notional(money))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.strike")]
    #[pyo3(text_signature = "($self, value)")]
    fn strike<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let strike = extract_rate_decimal(value, "strike")?;
        let strike = decimal_from_f64(strike, "strike")?;
        builder_set!(slf, strike, strike.to_string(), |b: CapFloorBuilder| b
            .strike(strike))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.spread_bp")]
    #[pyo3(text_signature = "($self, value)")]
    fn spread_bp<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spread_bp = crate::bindings::extract::extract_basis_points(value, "spread_bp")?;
        let spread_bp = decimal_from_f64(spread_bp, "spread_bp")?;
        builder_set!(
            slf,
            spread_bp,
            spread_bp.to_string(),
            |b: CapFloorBuilder| b.spread_bp(spread_bp)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.premium")]
    #[pyo3(signature = (payment_date, amount, currency = None))]
    #[pyo3(text_signature = "($self, payment_date, amount, currency=None)")]
    fn premium<'py>(
        mut slf: PyRefMut<'py, Self>,
        payment_date: &Bound<'_, PyAny>,
        amount: &Bound<'_, PyAny>,
        currency: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let payment_date = py_to_date(payment_date)?;
        let amount = money_from_py(amount, currency, "amount")?;
        builder_set!(
            slf,
            premium,
            format!("({payment_date}, {})", money_repr(amount)),
            |b: CapFloorBuilder| b.premium((payment_date, amount))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.start_date")]
    #[pyo3(text_signature = "($self, value)")]
    fn start_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let start_date = py_to_date(value)?;
        builder_set!(
            slf,
            start_date,
            start_date.to_string(),
            |b: CapFloorBuilder| b.start_date(start_date)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.maturity")]
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let maturity = py_to_date(value)?;
        builder_set!(slf, maturity, maturity.to_string(), |b: CapFloorBuilder| b
            .maturity(maturity))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.frequency")]
    #[pyo3(text_signature = "($self, value)")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let tenor = crate::bindings::core::dates::tenor::extract_tenor(value)?;
        builder_set!(slf, frequency, tenor.to_string(), |b: CapFloorBuilder| b
            .frequency(tenor))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.day_count")]
    #[pyo3(text_signature = "($self, value)")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let day_count =
            crate::bindings::valuations::convert::day_count_from_py(value, "day_count")?;
        builder_set!(
            slf,
            day_count,
            day_count.to_string(),
            |b: CapFloorBuilder| b.day_count(day_count)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.stub")]
    #[pyo3(text_signature = "($self, value)")]
    fn stub<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let stub = stub_kind_from_py(Some(value), "stub")?;
        builder_set!(
            slf,
            stub,
            format!("{:?}", enum_to_py_string(&stub).unwrap_or_default()),
            |b: CapFloorBuilder| b.stub(stub)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.business_day_convention")]
    #[pyo3(text_signature = "($self, value)")]
    fn business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let convention = super::convert::bdc_from_str(value, "business_day_convention")?;
        builder_set!(
            slf,
            business_day_convention,
            format!("{value:?}"),
            |b: CapFloorBuilder| b.business_day_convention(convention)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.calendar_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            calendar_id,
            format!("{value:?}"),
            |b: CapFloorBuilder| b.calendar_id(CalendarId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.exercise_style")]
    #[pyo3(text_signature = "($self, value)")]
    fn exercise_style<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let style = enum_from_str(value, "exercise_style")?;
        builder_set!(
            slf,
            exercise_style,
            format!("{value:?}"),
            |b: CapFloorBuilder| b.exercise_style(style)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.settlement")]
    #[pyo3(text_signature = "($self, value)")]
    fn settlement<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let settlement = enum_from_str(value, "settlement")?;
        builder_set!(
            slf,
            settlement,
            format!("{value:?}"),
            |b: CapFloorBuilder| b.settlement(settlement)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.discount_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            discount_curve_id,
            format!("{value:?}"),
            |b: CapFloorBuilder| b.discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.forward_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn forward_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            forward_curve_id,
            format!("{value:?}"),
            |b: CapFloorBuilder| b.forward_curve_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.vol_surface_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            vol_surface_id,
            format!("{value:?}"),
            |b: CapFloorBuilder| b.vol_surface_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.vol_type")]
    #[pyo3(text_signature = "($self, value)")]
    fn vol_type<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let vol_type = enum_from_str(value, "vol_type")?;
        builder_set!(slf, vol_type, format!("{value:?}"), |b: CapFloorBuilder| b
            .vol_type(vol_type))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.vol_shift")]
    #[pyo3(text_signature = "($self, value)")]
    fn vol_shift<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, vol_shift, value.to_string(), |b: CapFloorBuilder| b
            .vol_shift(value))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.overnight_coupon")]
    #[pyo3(text_signature = "($self, value)")]
    fn overnight_coupon<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let convention: finstack_quant_valuations::instruments::rates::cap_floor::OvernightCouponConvention =
            spec_from_py(py, value, "overnight_coupon")?;
        builder_set!(
            slf,
            overnight_coupon,
            "{...}".to_string(),
            |b: CapFloorBuilder| b.overnight_coupon(convention)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates", "CapFloorBuilder.attributes")]
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
            |b: CapFloorBuilder| b.attributes(attrs)
        )
    }
}

/// Register the typed rates instruments on the instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyInterestRateSwap>()?;
    m.add_class::<PyInterestRateSwapBuilder>()?;
    m.add_class::<PySwaption>()?;
    m.add_class::<PySwaptionBuilder>()?;
    m.add_class::<PyCapFloor>()?;
    m.add_class::<PyCapFloorBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
///
/// Extend this list (sorted) when adding a class or function here; `mod.rs`
/// merges every submodule list so registration stays in one place per file.
pub(crate) const EXPORTS: &[&str] = &[
    "CapFloor",
    "CapFloorBuilder",
    "InterestRateSwap",
    "InterestRateSwapBuilder",
    "Swaption",
    "SwaptionBuilder",
];
