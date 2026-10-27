//! Typed FX instruments: `FxForward` and `FxOption`.
//! Mirrors the `PyInterestRateSwap` pattern in `typed_rates.rs`.
//!
//! The `instrument_pricing_methods!` /
//! `instrument_envelope_methods!` / `builder_set!` macros shared by every
//! typed wrapper live in `typed_macros.rs`.

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::bindings::core::currency::PyCurrency;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::extract_market;
use crate::errors::core_to_py;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fx::fx_option::{
    FxDeltaConvention, FxDeltaConventionKind,
};
use finstack_quant_valuations::instruments::Instrument;

use super::convert::enum_from_str;
use super::convert::{
    attributes_from_py, bdc_from_py, date_repr, day_count_from_py, enum_to_py_string, float_repr,
    money_repr, money_to_py, opt_repr,
};
use super::pricing::typed_option_greeks;
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

use finstack_quant_valuations::instruments::fx::fx_forward::FxForwardBuilder;
use finstack_quant_valuations::instruments::fx::fx_option::FxOptionBuilder;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForward")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "FxForward",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFxForward {
    /// Inner canonical Rust FX forward.
    pub(crate) inner: finstack_quant_valuations::instruments::fx::fx_forward::FxForward,
}

instrument_envelope_methods!(
    PyFxForward(
        "FxForward",
        module = "finstack_quant.valuations.instruments._typed_fx"
    ),
    FxForward,
    builder = PyFxForwardBuilder("FxForwardBuilder", fields),
    seed = finstack_quant_valuations::instruments::fx::fx_forward::FxForward::builder(),
);
instrument_pricing_methods!(
    PyFxForward,
    "FxForward",
    module = "finstack_quant.valuations.instruments._typed_fx"
);

#[pymethods]
impl PyFxForward {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForward.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::fx::fx_forward::FxForward::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForward.from_trade_date")]
    #[staticmethod]
    #[pyo3(signature = (id, base_currency, quote_currency, trade_date, tenor, notional,
                        domestic_discount_curve_id, foreign_discount_curve_id, *,
                        base_calendar_id=None, quote_calendar_id=None, settlement_days=None,
                        business_day_convention=None, end_of_month=false))]
    #[pyo3(
        text_signature = "(id, base_currency, quote_currency, trade_date, tenor, notional, \
domestic_discount_curve_id, foreign_discount_curve_id, *, base_calendar_id=None, \
quote_calendar_id=None, settlement_days=None, business_day_convention=None, \
end_of_month=False)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn from_trade_date(
        id: &str,
        base_currency: &Bound<'_, PyAny>,
        quote_currency: &Bound<'_, PyAny>,
        trade_date: &Bound<'_, PyAny>,
        tenor: &Bound<'_, PyAny>,
        notional: &Bound<'_, PyAny>,
        domestic_discount_curve_id: &str,
        foreign_discount_curve_id: &str,
        base_calendar_id: Option<String>,
        quote_calendar_id: Option<String>,
        settlement_days: Option<u32>,
        business_day_convention: Option<&Bound<'_, PyAny>>,
        end_of_month: bool,
    ) -> PyResult<Self> {
        let base = crate::bindings::core::currency::extract_currency(base_currency)?;
        let quote = crate::bindings::core::currency::extract_currency(quote_currency)?;
        let convention = business_day_convention
            .filter(|value| !value.is_none())
            .map(|value| bdc_from_py(value, "business_day_convention"))
            .transpose()?;
        let inner =
            finstack_quant_valuations::instruments::fx::fx_forward::FxForward::from_trade_date(
                InstrumentId::new(id.to_string()),
                base,
                quote,
                py_to_date(trade_date)?,
                crate::bindings::core::dates::tenor::extract_tenor(tenor)?,
                super::convert::money_from_py(notional, Some(base.as_ref()), "notional")?,
                CurveId::new(domestic_discount_curve_id.to_string()),
                CurveId::new(foreign_discount_curve_id.to_string()),
                base_calendar_id,
                quote_calendar_id,
                settlement_days,
                convention,
                end_of_month,
            )
            .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForward.with_forward_points")]
    #[pyo3(text_signature = "($self, spot_rate, forward_points)")]
    fn with_forward_points(&self, spot_rate: f64, forward_points: f64) -> PyResult<Self> {
        self.inner
            .clone()
            .with_forward_points(spot_rate, forward_points)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForward.with_forward_pips")]
    #[pyo3(text_signature = "($self, spot_rate, pips)")]
    fn with_forward_pips(&self, spot_rate: f64, pips: f64) -> PyResult<Self> {
        self.inner
            .clone()
            .with_forward_pips(spot_rate, pips)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForward.market_forward_rate")]
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn market_forward_rate(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        self.inner
            .market_forward_rate(&market, as_of)
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForward.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForward.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        Instrument::expiry(&self.inner)
            .map(|d| date_to_py(py, d))
            .transpose()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "FxForward(id={:?}, pair='{}{}', notional={}, maturity={}, contract_rate={})",
            self.inner.id.as_str(),
            self.inner.base_currency,
            self.inner.quote_currency,
            money_repr(self.inner.notional),
            date_repr(self.inner.maturity),
            opt_repr(self.inner.contract_rate.map(float_repr)),
        )
    }
}

crate::bindings::macros::py_getters!(PyFxForward |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForward.base_currency")]
    base_currency -> PyCurrency = PyCurrency::from_inner(s.inner.base_currency);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForward.quote_currency")]
    quote_currency -> PyCurrency = PyCurrency::from_inner(s.inner.quote_currency);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForward.notional")]
    notional -> PyMoney = money_to_py(s.inner.notional);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForward.contract_rate")]
    contract_rate -> Option<f64> = s.inner.contract_rate;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForward.domestic_discount_curve_id")]
    domestic_discount_curve_id -> String = s.inner.domestic_discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForward.foreign_discount_curve_id")]
    foreign_discount_curve_id -> String = s.inner.foreign_discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForward.quoted_spot")]
    quoted_spot -> Option<f64> = s.inner.quoted_spot;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForward.base_calendar_id")]
    base_calendar_id -> Option<String> = s.inner.base_calendar_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForward.quote_calendar_id")]
    quote_calendar_id -> Option<String> =
        s.inner.quote_calendar_id.as_ref().map(ToString::to_string);
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForwardBuilder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "FxForwardBuilder",
    skip_from_py_object
)]
pub struct PyFxForwardBuilder {
    inner: Option<FxForwardBuilder>,
    fields: Vec<(&'static str, String)>,
}

#[pymethods]
impl PyFxForwardBuilder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForwardBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, id, format!("{value:?}"), |b: FxForwardBuilder| b
            .id(InstrumentId::new(value.to_string())))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForwardBuilder.base_currency")]
    #[pyo3(text_signature = "($self, value)")]
    fn base_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let ccy = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(
            slf,
            base_currency,
            format!("Currency('{ccy}')"),
            |b: FxForwardBuilder| b.base_currency(ccy)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForwardBuilder.quote_currency")]
    #[pyo3(text_signature = "($self, value)")]
    fn quote_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let ccy = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(
            slf,
            quote_currency,
            format!("Currency('{ccy}')"),
            |b: FxForwardBuilder| b.quote_currency(ccy)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForwardBuilder.maturity")]
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let maturity = py_to_date(value)?;
        builder_set!(slf, maturity, date_repr(maturity), |b: FxForwardBuilder| b
            .maturity(maturity))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForwardBuilder.notional")]
    #[pyo3(text_signature = "($self, value)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = value.inner;
        builder_set!(slf, notional, money_repr(money), |b: FxForwardBuilder| b
            .notional(money))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForwardBuilder.contract_rate")]
    #[pyo3(text_signature = "($self, value)")]
    fn contract_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            contract_rate,
            float_repr(value),
            |b: FxForwardBuilder| b.contract_rate(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForwardBuilder.domestic_discount_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn domestic_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            domestic_discount_curve_id,
            format!("{value:?}"),
            |b: FxForwardBuilder| b.domestic_discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForwardBuilder.foreign_discount_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn foreign_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            foreign_discount_curve_id,
            format!("{value:?}"),
            |b: FxForwardBuilder| b.foreign_discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForwardBuilder.quoted_spot")]
    #[pyo3(text_signature = "($self, value)")]
    fn quoted_spot<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            quoted_spot,
            float_repr(value),
            |b: FxForwardBuilder| b.quoted_spot(value)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForwardBuilder.base_calendar_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn base_calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            base_calendar_id,
            format!("{value:?}"),
            |b: FxForwardBuilder| b.base_calendar_id(value.into())
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForwardBuilder.quote_calendar_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn quote_calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            quote_calendar_id,
            format!("{value:?}"),
            |b: FxForwardBuilder| b.quote_calendar_id(value.into())
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxForwardBuilder.attributes")]
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        let shown = value.repr()?.to_string();
        builder_set!(slf, attributes, shown, |b: FxForwardBuilder| b
            .attributes(attrs))
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOption")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "FxOption",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFxOption {
    /// Inner canonical Rust FX option.
    pub(crate) inner: finstack_quant_valuations::instruments::fx::fx_option::FxOption,
}

instrument_envelope_methods!(
    PyFxOption(
        "FxOption",
        module = "finstack_quant.valuations.instruments._typed_fx"
    ),
    FxOption,
    builder = PyFxOptionBuilder("FxOptionBuilder", fields),
    seed = finstack_quant_valuations::instruments::fx::fx_option::FxOption::builder(),
);
instrument_pricing_methods!(
    PyFxOption,
    "FxOption",
    module = "finstack_quant.valuations.instruments._typed_fx"
);

/// Build an `FxDeltaConvention` from the loose Python inputs.
fn delta_convention_from_parts(
    kind: &str,
    premium_currency: &Bound<'_, PyAny>,
    venue: &str,
) -> PyResult<FxDeltaConvention> {
    let kind: FxDeltaConventionKind = enum_from_str(kind, "delta convention kind")?;
    FxDeltaConvention::new(
        kind,
        crate::bindings::core::currency::extract_currency(premium_currency)?,
        venue,
    )
    .map_err(core_to_py)
}

#[pymethods]
impl PyFxOption {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOption.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::fx::fx_option::FxOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOption.european")]
    #[staticmethod]
    #[pyo3(signature = (id, base_currency, quote_currency, strike, expiry, notional, vol_surface_id,
                        option_type, delta_convention_kind, premium_currency, venue))]
    #[pyo3(
        text_signature = "(id, base_currency, quote_currency, strike, expiry, notional, \
vol_surface_id, option_type, delta_convention_kind, premium_currency, venue)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn european(
        id: &str,
        base_currency: &Bound<'_, PyAny>,
        quote_currency: &Bound<'_, PyAny>,
        strike: f64,
        expiry: &Bound<'_, PyAny>,
        notional: &Bound<'_, PyAny>,
        vol_surface_id: &str,
        option_type: &str,
        delta_convention_kind: &str,
        premium_currency: &Bound<'_, PyAny>,
        venue: &str,
    ) -> PyResult<Self> {
        let base = crate::bindings::core::currency::extract_currency(base_currency)?;
        let quote = crate::bindings::core::currency::extract_currency(quote_currency)?;
        let inner = finstack_quant_valuations::instruments::fx::fx_option::FxOption::european(
            InstrumentId::new(id.to_string()),
            base,
            quote,
            strike,
            py_to_date(expiry)?,
            super::convert::money_from_py(notional, Some(base.as_ref()), "notional")?,
            CurveId::new(vol_surface_id.to_string()),
            enum_from_str(option_type, "option_type")?,
            delta_convention_from_parts(delta_convention_kind, premium_currency, venue)?,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOption.implied_vol")]
    #[pyo3(text_signature = "($self, market, as_of, target_price)")]
    fn implied_vol(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        target_price: f64,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        let as_of = py_to_date(as_of)?;
        self.inner
            .implied_vol(&market, as_of, target_price)
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOption.greeks")]
    #[pyo3(signature = (market, as_of, model="default"))]
    fn greeks<'py>(
        &self,
        py: Python<'py>,
        market: &Bound<'py, PyAny>,
        as_of: &Bound<'py, PyAny>,
        model: &str,
    ) -> PyResult<Bound<'py, PyDict>> {
        typed_option_greeks(py, Box::new(self.inner.clone()), market, as_of, model)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOption.delta_convention")]
    #[getter]
    fn delta_convention<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        out.set_item("kind", self.inner.delta_convention.kind.to_string())?;
        out.set_item(
            "premium_currency",
            self.inner.delta_convention.premium_currency.to_string(),
        )?;
        out.set_item("venue", self.inner.delta_convention.venue.clone())?;
        Ok(out)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOption.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.expiry)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "FxOption(id={:?}, pair='{}{}', option_type={:?}, strike={}, expiry={}, notional={})",
            self.inner.id.as_str(),
            self.inner.base_currency,
            self.inner.quote_currency,
            enum_to_py_string(&self.inner.option_type).unwrap_or_default(),
            float_repr(self.inner.strike),
            date_repr(self.inner.expiry),
            money_repr(self.inner.notional),
        )
    }
}

crate::bindings::macros::py_getters!(PyFxOption |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOption.base_currency")]
    base_currency -> PyCurrency = PyCurrency::from_inner(s.inner.base_currency);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOption.quote_currency")]
    quote_currency -> PyCurrency = PyCurrency::from_inner(s.inner.quote_currency);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOption.strike")]
    strike -> f64 = s.inner.strike;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOption.option_type")]
    option_type -> PyResult<String> = enum_to_py_string(&s.inner.option_type);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOption.day_count")]
    day_count -> crate::bindings::core::dates::daycount::PyDayCount =
        crate::bindings::core::dates::daycount::PyDayCount::from_inner(s.inner.day_count);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOption.notional")]
    notional -> PyMoney = money_to_py(s.inner.notional);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOption.domestic_discount_curve_id")]
    domestic_discount_curve_id -> String = s.inner.domestic_discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOption.foreign_discount_curve_id")]
    foreign_discount_curve_id -> String = s.inner.foreign_discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOption.vol_surface_id")]
    vol_surface_id -> String = s.inner.vol_surface_id.to_string();
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOptionBuilder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "FxOptionBuilder",
    skip_from_py_object
)]
pub struct PyFxOptionBuilder {
    inner: Option<FxOptionBuilder>,
    fields: Vec<(&'static str, String)>,
}

#[pymethods]
impl PyFxOptionBuilder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOptionBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, id, format!("{value:?}"), |b: FxOptionBuilder| b
            .id(InstrumentId::new(value.to_string())))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOptionBuilder.base_currency")]
    #[pyo3(text_signature = "($self, value)")]
    fn base_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let ccy = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(
            slf,
            base_currency,
            format!("Currency('{ccy}')"),
            |b: FxOptionBuilder| b.base_currency(ccy)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOptionBuilder.quote_currency")]
    #[pyo3(text_signature = "($self, value)")]
    fn quote_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let ccy = crate::bindings::core::currency::extract_currency(value)?;
        builder_set!(
            slf,
            quote_currency,
            format!("Currency('{ccy}')"),
            |b: FxOptionBuilder| b.quote_currency(ccy)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOptionBuilder.strike")]
    #[pyo3(text_signature = "($self, value)")]
    fn strike<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, strike, float_repr(value), |b: FxOptionBuilder| b
            .strike(value))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOptionBuilder.option_type")]
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
            |b: FxOptionBuilder| b.option_type(option_type)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOptionBuilder.delta_convention")]
    #[pyo3(text_signature = "($self, kind, premium_currency, venue)")]
    fn delta_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        kind: &str,
        premium_currency: &Bound<'_, PyAny>,
        venue: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let convention = delta_convention_from_parts(kind, premium_currency, venue)?;
        let shown = format!(
            "({kind:?}, Currency('{}'), {venue:?})",
            convention.premium_currency
        );
        builder_set!(slf, delta_convention, shown, |b: FxOptionBuilder| b
            .delta_convention(convention))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOptionBuilder.expiry")]
    #[pyo3(text_signature = "($self, value)")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let expiry = py_to_date(value)?;
        builder_set!(slf, expiry, date_repr(expiry), |b: FxOptionBuilder| b
            .expiry(expiry))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOptionBuilder.day_count")]
    #[pyo3(text_signature = "($self, value)")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let day_count = day_count_from_py(value, "day_count")?;
        builder_set!(
            slf,
            day_count,
            format!("DayCount('{day_count}')"),
            |b: FxOptionBuilder| b.day_count(day_count)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOptionBuilder.notional")]
    #[pyo3(text_signature = "($self, value)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = value.inner;
        builder_set!(slf, notional, money_repr(money), |b: FxOptionBuilder| b
            .notional(money))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOptionBuilder.domestic_discount_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn domestic_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            domestic_discount_curve_id,
            format!("{value:?}"),
            |b: FxOptionBuilder| b.domestic_discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOptionBuilder.foreign_discount_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn foreign_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            foreign_discount_curve_id,
            format!("{value:?}"),
            |b: FxOptionBuilder| b.foreign_discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOptionBuilder.vol_surface_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            vol_surface_id,
            format!("{value:?}"),
            |b: FxOptionBuilder| b.vol_surface_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_fx", "FxOptionBuilder.attributes")]
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        let shown = value.repr()?.to_string();
        builder_set!(slf, attributes, shown, |b: FxOptionBuilder| b
            .attributes(attrs))
    }
}

/// Register the typed FX instruments on the instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFxForward>()?;
    m.add_class::<PyFxForwardBuilder>()?;
    m.add_class::<PyFxOption>()?;
    m.add_class::<PyFxOptionBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
///
/// Extend this list (sorted) when adding a class or function here; `mod.rs`
/// merges every submodule list so registration stays in one place per file.
pub(crate) const EXPORTS: &[&str] = &[
    "FxForward",
    "FxForwardBuilder",
    "FxOption",
    "FxOptionBuilder",
];
