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
use crate::errors::core_to_py;
use finstack_quant_core::types::{CalendarId, CurveId, InstrumentId};
use finstack_quant_valuations::instruments::InstrumentJson;

use super::convert::{
    attributes_from_py, enum_to_py_string, money_from_py, money_to_py, rate_decimal_from_py,
};
use super::typed_legs::{PyFixedLegSpec, PyFloatLegSpec};
use crate::bindings::valuations::convert::{builder_repr, money_repr};
use crate::bindings::valuations::instruments::{
    decimal_from_f64, enum_from_str, instrument_expiry, opt_serde_to_py,
    serialize_typed_instrument_json, spec_from_py, stub_kind_from_py,
};
use crate::bindings::valuations::typed_macros::{
    builder_set, instrument_envelope_methods, instrument_pricing_methods,
};

type IrsBuilder = finstack_quant_valuations::instruments::rates::irs::InterestRateSwapBuilder;
type SwaptionBuilderInner =
    finstack_quant_valuations::instruments::rates::swaption::SwaptionBuilder;
type CapFloorBuilderInner =
    finstack_quant_valuations::instruments::rates::cap_floor::CapFloorBuilder;
type OtcMarginSpec = finstack_quant_margin::types::OtcMarginSpec;

/// Render a `Decimal` as a Python float literal for reprs.
fn decimal_f64(value: rust_decimal::Decimal) -> f64 {
    value.to_f64().unwrap_or(f64::NAN)
}

/// Typed wrapper for the Rust `InterestRateSwap` instrument.
///
/// Construct via ``InterestRateSwap.from_conventions`` (market conventions
/// resolved from the rate-index registry), ``InterestRateSwap.builder()``
/// with explicit ``FixedLegSpec`` / ``FloatLegSpec`` legs,
/// ``InterestRateSwap.example()`` or ``InterestRateSwap.from_json``.
/// Every public Rust field is readable as a property; ``price`` / ``metric``
/// run the same pricer as ``price_instrument``.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "InterestRateSwap",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyInterestRateSwap {
    /// Inner canonical Rust swap.
    pub(crate) inner: finstack_quant_valuations::instruments::InterestRateSwap,
}

impl PyInterestRateSwap {
    /// Serialize as the canonical instrument envelope accepted by the JSON loader.
    pub(crate) fn envelope_json(&self) -> PyResult<String> {
        serialize_typed_instrument_json(
            InstrumentJson::InterestRateSwap(self.inner.clone()),
            "InterestRateSwap",
        )
    }
}

instrument_envelope_methods!(
    PyInterestRateSwap,
    InterestRateSwap,
    "interest_rate_swap",
    PyInterestRateSwapBuilder,
    finstack_quant_valuations::instruments::InterestRateSwap::builder()
);
instrument_pricing_methods!(PyInterestRateSwap);

#[pymethods]
impl PyInterestRateSwap {
    /// Create a vanilla swap from registered rate-index conventions.
    ///
    /// Mirrors Rust ``InterestRateSwap::from_conventions`` (QuantLib
    /// ``MakeVanillaSwap`` ergonomics): day counts, frequencies, calendars,
    /// reset/payment lags and overnight compounding are resolved from the
    /// convention registry entry for ``index_id``.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique instrument identifier.
    /// notional : Money | float
    ///     Notional shared by both legs; a bare number needs ``currency``.
    /// side : {"pay", "receive"}
    ///     ``"pay"`` pays fixed / receives floating.
    /// fixed_rate : float | Rate
    ///     Fixed coupon as a decimal (``0.03`` = 3%) or a ``Rate``.
    /// start_date : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Effective date.
    /// maturity : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Maturity date.
    /// index_id : str
    ///     Registered rate index (e.g. ``"USD-SOFR"``, ``"USD-SOFR-3M"``,
    ///     ``"EUR-EURIBOR-6M"``).
    /// discount_curve_id : str
    ///     Discount curve identifier for both legs.
    /// forward_curve_id : str
    ///     Projection curve identifier for the floating leg.
    /// currency : str, optional
    ///     ISO-4217 code applied when ``notional`` is a bare number.
    ///
    /// Returns
    /// -------
    /// InterestRateSwap
    ///     The validated swap.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``side`` is unknown, ``index_id`` is not registered, a bare
    ///     ``notional`` has no ``currency``, or validation fails.
    /// TypeError
    ///     If ``fixed_rate``/``notional`` has an unsupported type or a date
    ///     cannot be interpreted.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import InterestRateSwap
    /// >>> swap = InterestRateSwap.from_conventions(
    /// ...     "IRS-5Y", 10_000_000.0, "pay", 0.035, "2025-01-15", "2030-01-15",
    /// ...     "USD-SOFR", "USD-OIS", "USD-SOFR", currency="USD",
    /// ... )
    /// >>> swap.float_leg.reset_lag_days
    /// 0
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
            fixed_rate: rate_decimal_from_py(fixed_rate, "fixed_rate")?,
            start_date: py_to_date(start_date)?,
            maturity: py_to_date(maturity)?,
            index_id,
            discount_curve_id,
            forward_curve_id,
        };
        let inner =
            finstack_quant_valuations::instruments::InterestRateSwap::from_conventions(params)
                .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Canonical 5-year USD pay-fixed swap (mirrors Rust
    /// ``InterestRateSwap::example``).
    ///
    /// Returns
    /// -------
    /// InterestRateSwap
    ///     Semi-annual 30/360 fixed vs quarterly ACT/360 ``USD-SOFR-3M`` with
    ///     a T-2 reset lag and ``usny`` calendar.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If construction fails (should not occur).
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import InterestRateSwap
    /// >>> InterestRateSwap.example().side
    /// 'pay'
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::InterestRateSwap::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Notional shared by both legs.
    #[getter]
    fn notional(&self) -> PyMoney {
        money_to_py(self.inner.notional)
    }

    /// Swap direction for the fixed leg: ``"pay"`` or ``"receive"``.
    #[getter]
    fn side(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.side)
    }

    /// Fixed leg specification.
    #[getter]
    fn fixed_leg(&self) -> PyFixedLegSpec {
        PyFixedLegSpec::from_inner(self.inner.fixed_leg.clone())
    }

    /// Floating leg specification.
    #[getter]
    fn float_leg(&self) -> PyFloatLegSpec {
        PyFloatLegSpec::from_inner(self.inner.float_leg.clone())
    }

    /// Whether fixed coupon accrual dates use that leg's calendar and business-day convention.
    #[getter]
    fn adjust_fixed_accrual_dates(&self) -> bool {
        self.inner.adjust_fixed_accrual_dates
    }

    /// Whether floating coupon accrual dates use that leg's calendar and business-day convention.
    #[getter]
    fn adjust_float_accrual_dates(&self) -> bool {
        self.inner.adjust_float_accrual_dates
    }

    /// OTC margin (CSA / initial-margin) specification in serde form, or ``None``.
    #[getter]
    fn margin_spec<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.margin_spec.as_ref())
    }

    /// Expiry date exposed by the ``Instrument`` trait, or ``None``.
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

/// Fluent builder for ``InterestRateSwap``; wraps the Rust
/// `FinancialBuilder`-generated builder (consuming setters).
///
/// Builders are consumed by build(); create a new builder per instrument.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "InterestRateSwapBuilder",
    skip_from_py_object
)]
pub struct PyInterestRateSwapBuilder {
    inner: Option<IrsBuilder>,
    fields: Vec<(&'static str, String)>,
}

crate::bindings::valuations::pricing::pricing_override_methods!(
    PyInterestRateSwap,
    PyInterestRateSwapBuilder,
    "InterestRateSwapBuilder",
    fields
);

#[pymethods]
impl PyInterestRateSwapBuilder {
    /// Set the instrument identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Unique identifier for the swap.
    ///
    /// Returns
    /// -------
    /// InterestRateSwapBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, id, format!("{value:?}"), |b: IrsBuilder| b
            .id(InstrumentId::new(value.to_string())))
    }

    /// Set the notional (both legs).
    ///
    /// Parameters
    /// ----------
    /// value : Money | float
    ///     Notional amount shared by both legs; a bare number needs ``currency``.
    /// currency : str, optional
    ///     ISO-4217 code applied when ``value`` is a bare number.
    ///
    /// Returns
    /// -------
    /// InterestRateSwapBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a bare number is given without ``currency``.
    #[pyo3(signature = (value, currency = None))]
    #[pyo3(text_signature = "($self, value, currency=None)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
        currency: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, currency, "notional")?;
        builder_set!(slf, notional, money_repr(money), |b: IrsBuilder| b
            .notional(money))
    }

    /// Set the swap direction: ``"pay"`` or ``"receive"`` (fixed leg).
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     ``"pay"`` to pay fixed/receive floating, ``"receive"`` for the
    ///     opposite.
    ///
    /// Returns
    /// -------
    /// InterestRateSwapBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized side.
    #[pyo3(text_signature = "($self, value)")]
    fn side<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let side = enum_from_str(value, "side")?;
        builder_set!(slf, side, format!("{value:?}"), |b: IrsBuilder| b
            .side(side))
    }

    /// Set the fixed leg specification.
    ///
    /// Parameters
    /// ----------
    /// value : FixedLegSpec
    ///     Fixed leg specification.
    ///
    /// Returns
    /// -------
    /// InterestRateSwapBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn fixed_leg<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyFixedLegSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, fixed_leg, value.__repr__(), |b: IrsBuilder| b
            .fixed_leg(value.inner.clone()))
    }

    /// Set the floating leg specification.
    ///
    /// Parameters
    /// ----------
    /// value : FloatLegSpec
    ///     Floating leg specification.
    ///
    /// Returns
    /// -------
    /// InterestRateSwapBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn float_leg<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyFloatLegSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, float_leg, value.__repr__(), |b: IrsBuilder| b
            .float_leg(value.inner.clone()))
    }

    /// Select adjustment of fixed coupon accrual dates independently of payment dates.
    ///
    /// # Arguments
    /// * `value` - True applies the fixed leg's calendar and business-day convention to accrual boundaries; false (default) retains contractual dates.
    #[pyo3(text_signature = "($self, value)")]
    fn adjust_fixed_accrual_dates<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            adjust_fixed_accrual_dates,
            value.to_string(),
            |b: IrsBuilder| b.adjust_fixed_accrual_dates(value)
        )
    }

    /// Select adjustment of floating coupon accrual dates independently of payment dates.
    ///
    /// # Arguments
    /// * `value` - True applies the floating leg's calendar and business-day convention to accrual boundaries; false (default) retains contractual dates.
    #[pyo3(text_signature = "($self, value)")]
    fn adjust_float_accrual_dates<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            adjust_float_accrual_dates,
            value.to_string(),
            |b: IrsBuilder| b.adjust_float_accrual_dates(value)
        )
    }

    /// Set the OTC margin (CSA / initial-margin) specification.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     Rust ``OtcMarginSpec`` in serde form (dict or JSON string).
    ///
    /// Returns
    /// -------
    /// InterestRateSwapBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not deserialize as an ``OtcMarginSpec``.
    #[pyo3(text_signature = "($self, value)")]
    fn margin_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: OtcMarginSpec = spec_from_py(py, value, "margin_spec")?;
        builder_set!(slf, margin_spec, "{...}".to_string(), |b: IrsBuilder| b
            .margin_spec(spec))
    }

    /// Set instrument attributes (tags and metadata).
    ///
    /// Parameters
    /// ----------
    /// value : Attributes | dict | None
    ///     Attribute bag: an ``Attributes`` or its serde ``dict`` form
    ///     (``{"tags": [...], "meta": {...}}``, as ``to_dict()`` returns it);
    ///     ``None`` clears it.
    ///
    /// Returns
    /// -------
    /// InterestRateSwapBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
    /// ValueError
    ///     If the ``dict`` has a key other than ``tags`` / ``meta`` or a non-string tag or value.
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
            |b: IrsBuilder| b.attributes(attrs)
        )
    }

    /// Build the validated swap.
    ///
    /// Runs the same validation as Rust ``InterestRateSwapBuilder::build``
    /// (structural invariants); pricing-time checks happen in ``price``.
    ///
    /// Returns
    /// -------
    /// InterestRateSwap
    ///     The validated swap.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed, a required field is missing
    ///     (the message names the builder and field), or the swap fails
    ///     validation.
    #[pyo3(text_signature = "($self)")]
    fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<PyInterestRateSwap> {
        let b = crate::bindings::valuations::convert::take_builder(&mut slf.inner)?;
        let inner = b.build().map_err(core_to_py)?;
        Ok(PyInterestRateSwap { inner })
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        builder_repr("InterestRateSwapBuilder", &self.fields)
    }
}

/// Typed wrapper for the Rust `Swaption` instrument.
///
/// Construct via ``Swaption.builder()``, ``Swaption.example()`` /
/// ``Swaption.example()`` or ``Swaption.from_json``. Every public
/// Rust field is readable as a property; ``get_strike`` / ``get_underlying_start_date``
/// / ``get_underlying_maturity`` / ``forward_swap_rate`` mirror the Rust accessors and
/// ``price`` / ``metric`` run the same pricer as ``price_instrument``.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "Swaption",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PySwaption {
    /// Inner canonical Rust swaption.
    pub(crate) inner: finstack_quant_valuations::instruments::Swaption,
}

impl PySwaption {
    /// Serialize as the canonical instrument envelope accepted by the JSON loader.
    pub(crate) fn envelope_json(&self) -> PyResult<String> {
        serialize_typed_instrument_json(InstrumentJson::Swaption(self.inner.clone()), "Swaption")
    }
}

instrument_envelope_methods!(
    PySwaption,
    Swaption,
    "swaption",
    PySwaptionBuilder,
    finstack_quant_valuations::instruments::Swaption::builder()
);
instrument_pricing_methods!(
    PySwaption,
    model_doc = [
        "     Explicit keys include ``\"black76\"``, ``\"normal\"`` and",
        "     ``\"hull_white_1f\"``.",
    ]
);

#[pymethods]
impl PySwaption {
    /// Canonical European 1Yx5Y USD payer swaption (mirrors Rust ``Swaption::example``).
    ///
    /// Returns
    /// -------
    /// Swaption
    ///     Cash-settled Black-vol swaption on a 3% 5-year swap, vol surface
    ///     ``USD-SWPNVOL``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the example instrument fails validation.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import Swaption
    /// >>> Swaption.example().get_strike()
    /// 0.03
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        Ok(Self {
            inner: finstack_quant_valuations::instruments::Swaption::example()
                .map_err(core_to_py)?,
        })
    }

    /// Forward swap rate of the underlying (mirrors Rust ``Swaption::forward_swap_rate``).
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market context holding the discount and forward curves.
    /// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Valuation date.
    ///
    /// Returns
    /// -------
    /// float
    ///     Par swap rate of the underlying as a decimal.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If a required curve is missing.
    /// RuntimeError
    ///     If the annuity or floating PV cannot be computed.
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

    /// Fixed strike of the underlying swap as a decimal (mirrors Rust ``get_strike``).
    #[pyo3(text_signature = "($self)")]
    fn get_strike(&self) -> f64 {
        decimal_f64(self.inner.get_strike())
    }

    /// Effective date of the underlying swap (mirrors Rust ``get_underlying_start_date``).
    #[pyo3(text_signature = "($self)")]
    fn get_underlying_start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.get_underlying_start_date())
    }

    /// Maturity of the underlying swap (mirrors Rust ``get_underlying_maturity``).
    #[pyo3(text_signature = "($self)")]
    fn get_underlying_maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.get_underlying_maturity())
    }

    /// Option type: ``"call"`` (payer) or ``"put"`` (receiver).
    #[getter]
    fn option_type(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.option_type)
    }

    /// Notional of the underlying swap.
    #[getter]
    fn notional(&self) -> PyMoney {
        money_to_py(self.inner.notional)
    }

    /// Option expiry date.
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.expiry)
    }

    /// Settlement method: ``"physical"`` or ``"cash"``.
    #[getter]
    fn settlement(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.settlement)
    }

    /// Cash settlement annuity method (serde string).
    #[getter]
    fn cash_settlement_method(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.cash_settlement_method)
    }

    /// Volatility model: ``"black"`` or ``"normal"``.
    #[getter]
    fn vol_model(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.vol_model)
    }

    /// Volatility surface identifier.
    #[getter]
    fn vol_surface_id(&self) -> String {
        self.inner.vol_surface_id.to_string()
    }

    /// Fixed leg of the underlying swap.
    #[getter]
    fn underlying_fixed_leg(&self) -> PyFixedLegSpec {
        PyFixedLegSpec::from_inner(self.inner.underlying_fixed_leg.clone())
    }

    /// Floating leg of the underlying swap.
    #[getter]
    fn underlying_float_leg(&self) -> PyFloatLegSpec {
        PyFloatLegSpec::from_inner(self.inner.underlying_float_leg.clone())
    }

    /// SABR parameters (``alpha``, ``beta``, ``nu``, ``rho``, ``shift``) as a dict, or ``None``.
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

/// Fluent builder for ``Swaption``; wraps the Rust
/// `FinancialBuilder`-generated builder (consuming setters).
///
/// Builders are consumed by build(); create a new builder per instrument.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "SwaptionBuilder",
    skip_from_py_object
)]
pub struct PySwaptionBuilder {
    inner: Option<SwaptionBuilderInner>,
    fields: Vec<(&'static str, String)>,
}

crate::bindings::valuations::pricing::pricing_override_methods!(
    PySwaption,
    PySwaptionBuilder,
    "SwaptionBuilder",
    fields
);

#[pymethods]
impl PySwaptionBuilder {
    /// Set the instrument identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Unique identifier for the swaption.
    ///
    /// Returns
    /// -------
    /// SwaptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, id, format!("{value:?}"), |b: SwaptionBuilderInner| b
            .id(InstrumentId::new(value.to_string())))
    }

    /// Set the option type: ``"call"`` (payer) or ``"put"`` (receiver).
    ///
    /// Parameters
    /// ----------
    /// value : {"call", "put"}
    ///     Option type of the swaption.
    ///
    /// Returns
    /// -------
    /// SwaptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized option type.
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
            |b: SwaptionBuilderInner| b.option_type(option_type)
        )
    }

    /// Set the notional amount of the underlying swap.
    ///
    /// Parameters
    /// ----------
    /// value : Money | float
    ///     Notional amount; a bare number needs ``currency``.
    /// currency : str, optional
    ///     ISO-4217 code applied when ``value`` is a bare number.
    ///
    /// Returns
    /// -------
    /// SwaptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a bare number is given without ``currency``.
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
            |b: SwaptionBuilderInner| b.notional(money)
        )
    }

    /// Set the option expiry date.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Option expiry date.
    ///
    /// Returns
    /// -------
    /// SwaptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let expiry = py_to_date(value)?;
        builder_set!(
            slf,
            expiry,
            expiry.to_string(),
            |b: SwaptionBuilderInner| b.expiry(expiry)
        )
    }

    /// Set the settlement method.
    ///
    /// Parameters
    /// ----------
    /// value : {"physical", "cash"}
    ///     Settlement method of the swaption.
    ///
    /// Returns
    /// -------
    /// SwaptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized settlement method.
    #[pyo3(text_signature = "($self, value)")]
    fn settlement<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let settlement = enum_from_str(value, "settlement")?;
        builder_set!(
            slf,
            settlement,
            format!("{value:?}"),
            |b: SwaptionBuilderInner| b.settlement(settlement)
        )
    }

    /// Set the cash settlement annuity method.
    ///
    /// Only affects pricing when ``settlement`` is ``"cash"``.
    ///
    /// Parameters
    /// ----------
    /// value : {"collateralized_cash_price", "par_yield", "isda_par_par", "zero_coupon"}
    ///     Cash settlement annuity method. ``"collateralized_cash_price"`` is
    ///     the default and discounts the physical fixed-leg annuity.
    ///
    /// Returns
    /// -------
    /// SwaptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized cash settlement method.
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
            |b: SwaptionBuilderInner| b.cash_settlement_method(method)
        )
    }

    /// Set the volatility model.
    ///
    /// Parameters
    /// ----------
    /// value : {"black", "normal"}
    ///     Volatility model used for pricing.
    ///
    /// Returns
    /// -------
    /// SwaptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized volatility model.
    #[pyo3(text_signature = "($self, value)")]
    fn vol_model<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let vol_model = enum_from_str(value, "vol_model")?;
        builder_set!(
            slf,
            vol_model,
            format!("{value:?}"),
            |b: SwaptionBuilderInner| b.vol_model(vol_model)
        )
    }

    /// Set the volatility surface identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Volatility surface identifier for option pricing.
    ///
    /// Returns
    /// -------
    /// SwaptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            vol_surface_id,
            format!("{value:?}"),
            |b: SwaptionBuilderInner| b.vol_surface_id(CurveId::new(value.to_string()))
        )
    }

    /// Set the complete fixed leg of the underlying swap.
    ///
    /// Parameters
    /// ----------
    /// value : FixedLegSpec
    ///     Fixed leg of the underlying swap.
    ///
    /// Returns
    /// -------
    /// SwaptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn underlying_fixed_leg<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyFixedLegSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            underlying_fixed_leg,
            value.__repr__(),
            |b: SwaptionBuilderInner| b.underlying_fixed_leg(value.inner.clone())
        )
    }

    /// Set the complete floating leg of the underlying swap.
    ///
    /// Parameters
    /// ----------
    /// value : FloatLegSpec
    ///     Floating leg of the underlying swap.
    ///
    /// Returns
    /// -------
    /// SwaptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn underlying_float_leg<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyFloatLegSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            underlying_float_leg,
            value.__repr__(),
            |b: SwaptionBuilderInner| b.underlying_float_leg(value.inner.clone())
        )
    }

    /// Set the SABR volatility model parameters.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     SABR parameters with fields ``alpha``, ``beta``, ``nu``, ``rho``
    ///     and optional ``shift`` (dict or JSON string).
    ///
    /// Returns
    /// -------
    /// SwaptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not deserialize as SABR parameters.
    #[pyo3(text_signature = "($self, value)")]
    fn sabr_params<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let sabr_params: finstack_quant_models::volatility::SabrParameters =
            spec_from_py(py, value, "sabr_params")?;
        builder_set!(
            slf,
            sabr_params,
            "{...}".to_string(),
            |b: SwaptionBuilderInner| b.sabr_params(sabr_params)
        )
    }

    /// Set instrument attributes (tags and metadata).
    ///
    /// Parameters
    /// ----------
    /// value : Attributes | dict | None
    ///     Attribute bag: an ``Attributes`` or its serde ``dict`` form
    ///     (``{"tags": [...], "meta": {...}}``, as ``to_dict()`` returns it);
    ///     ``None`` clears it.
    ///
    /// Returns
    /// -------
    /// SwaptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
    /// ValueError
    ///     If the ``dict`` has a key other than ``tags`` / ``meta`` or a non-string tag or value.
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
            |b: SwaptionBuilderInner| b.attributes(attrs)
        )
    }

    /// Build the validated swaption.
    ///
    /// Runs the same validation as Rust ``SwaptionBuilder::build``
    /// (structural invariants); pricing-time checks happen in ``price``.
    ///
    /// Returns
    /// -------
    /// Swaption
    ///     The validated swaption.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed, a required field is missing
    ///     (the message names the builder and field), or the swaption fails
    ///     validation.
    #[pyo3(text_signature = "($self)")]
    fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<PySwaption> {
        let b = crate::bindings::valuations::convert::take_builder(&mut slf.inner)?;
        let inner = b.build().map_err(core_to_py)?;
        Ok(PySwaption { inner })
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        builder_repr("SwaptionBuilder", &self.fields)
    }
}

/// Typed wrapper for the Rust `CapFloor` instrument.
///
/// Construct via ``CapFloor.builder()``, ``CapFloor.example()`` or
/// ``CapFloor.from_json``. Every public Rust field is readable as a property;
/// ``price`` / ``metric`` run the same pricer as ``price_instrument``.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CapFloor",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCapFloor {
    /// Inner canonical Rust cap/floor.
    pub(crate) inner: finstack_quant_valuations::instruments::CapFloor,
}

impl PyCapFloor {
    /// Serialize as the canonical instrument envelope accepted by the JSON loader.
    pub(crate) fn envelope_json(&self) -> PyResult<String> {
        serialize_typed_instrument_json(InstrumentJson::CapFloor(self.inner.clone()), "CapFloor")
    }
}

instrument_envelope_methods!(
    PyCapFloor,
    CapFloor,
    "cap_floor",
    PyCapFloorBuilder,
    finstack_quant_valuations::instruments::CapFloor::builder(),
    builder_doc = [
        " ",
        " Notes",
        " -----",
        " This factory does not raise; it returns a new instance with the documented defaults.",
        " Unset ``vol_type`` defaults to ``\"auto\"``: the surface is treated as",
        " a lognormal quote. Each caplet uses Black-76 when forward and strike",
        " are positive; otherwise the lognormal vol is converted to an",
        " equivalent normal vol and priced with Bachelier. A normal-vol",
        " surface must set ``vol_type`` to ``\"normal\"``.",
    ]
);
instrument_pricing_methods!(
    PyCapFloor,
    model_doc = [
        "     Explicit keys include ``\"black76\"``, ``\"normal\"`` and",
        "     ``\"hull_white_1f\"``.",
    ]
);

#[pymethods]
impl PyCapFloor {
    /// Canonical 5-year USD 3% cap (mirrors Rust ``CapFloor::example``).
    ///
    /// Returns
    /// -------
    /// CapFloor
    ///     Quarterly ACT/360 cap on ``USD-SOFR-3M`` discounted on ``USD-OIS``
    ///     with vol surface ``USD-CAPFLOOR-VOL``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If construction fails (should not occur).
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import CapFloor
    /// >>> CapFloor.example().strike
    /// 0.03
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::CapFloor::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Option type: ``"cap"``, ``"floor"``, ``"caplet"`` or ``"floorlet"``.
    #[getter]
    fn rate_option_type(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.rate_option_type)
    }

    /// Notional amount.
    #[getter]
    fn notional(&self) -> PyMoney {
        money_to_py(self.inner.notional)
    }

    /// Strike as a decimal rate.
    #[getter]
    fn strike(&self) -> f64 {
        decimal_f64(self.inner.strike)
    }

    /// Contractual margin added to the index, in basis points.
    #[getter]
    fn spread_bp(&self) -> f64 {
        decimal_f64(self.inner.spread_bp)
    }

    /// Start date of the underlying period.
    #[getter]
    fn start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start_date)
    }

    /// End date of the underlying period.
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    /// Payment frequency.
    #[getter]
    fn frequency(&self) -> PyTenor {
        PyTenor::from_inner(self.inner.frequency)
    }

    /// Accrual day-count convention.
    #[getter]
    fn day_count(&self) -> PyDayCount {
        PyDayCount::from_inner(self.inner.day_count)
    }

    /// Stub rule.
    #[getter]
    fn stub(&self) -> PyStubKind {
        PyStubKind::from_inner(self.inner.stub)
    }

    /// Business day convention (serde string).
    #[getter]
    fn business_day_convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.business_day_convention)
    }

    /// Holiday calendar identifier, or ``None``.
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id.as_ref().map(ToString::to_string)
    }

    /// Exercise style (serde string, e.g. ``"european"``).
    #[getter]
    fn exercise_style(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.exercise_style)
    }

    /// Settlement type (serde string, e.g. ``"cash"``).
    #[getter]
    fn settlement(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.settlement)
    }

    /// Discount curve identifier.
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    /// Forward curve identifier.
    #[getter]
    fn forward_curve_id(&self) -> String {
        self.inner.forward_curve_id.to_string()
    }

    /// Volatility surface identifier.
    #[getter]
    fn vol_surface_id(&self) -> String {
        self.inner.vol_surface_id.to_string()
    }

    /// Volatility convention: ``"lognormal"``, ``"shifted_lognormal"``, ``"normal"`` or ``"auto"``.
    #[getter]
    fn vol_type(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.vol_type)
    }

    /// Displacement shift for shifted-lognormal pricing.
    #[getter]
    fn vol_shift(&self) -> f64 {
        self.inner.vol_shift
    }

    /// Overnight coupon convention in serde form, or ``None``.
    #[getter]
    fn overnight_coupon<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.overnight_coupon.as_ref())
    }

    /// Dated premium ``(payment_date, Money)`` or ``None``.
    #[getter]
    fn premium<'py>(&self, py: Python<'py>) -> PyResult<Option<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .premium
            .map(|(date, amount)| Ok((date_to_py(py, date)?, money_to_py(amount))))
            .transpose()
    }

    /// Expiry date exposed by the ``Instrument`` trait, or ``None``.
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

/// Fluent builder for ``CapFloor``; wraps the Rust
/// `FinancialBuilder`-generated builder (consuming setters).
///
/// Builders are consumed by build(); create a new builder per instrument.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CapFloorBuilder",
    skip_from_py_object
)]
pub struct PyCapFloorBuilder {
    inner: Option<CapFloorBuilderInner>,
    fields: Vec<(&'static str, String)>,
}

crate::bindings::valuations::pricing::pricing_override_methods!(
    PyCapFloor,
    PyCapFloorBuilder,
    "CapFloorBuilder",
    fields
);

#[pymethods]
impl PyCapFloorBuilder {
    /// Set the instrument identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Unique identifier for the cap/floor.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, id, format!("{value:?}"), |b: CapFloorBuilderInner| b
            .id(InstrumentId::new(value.to_string())))
    }

    /// Set the option type.
    ///
    /// Parameters
    /// ----------
    /// value : {"cap", "floor", "caplet", "floorlet"}
    ///     Option type of the instrument: ``"cap"``/``"floor"`` for a series
    ///     of caplets/floorlets, or ``"caplet"``/``"floorlet"`` for a single
    ///     period.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized option type.
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
            |b: CapFloorBuilderInner| b.rate_option_type(rate_option_type)
        )
    }

    /// Set the notional amount.
    ///
    /// Parameters
    /// ----------
    /// value : Money | float
    ///     Notional amount; a bare number needs ``currency``.
    /// currency : str, optional
    ///     ISO-4217 code applied when ``value`` is a bare number.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a bare number is given without ``currency``.
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
            |b: CapFloorBuilderInner| b.notional(money)
        )
    }

    /// Set the strike.
    ///
    /// Parameters
    /// ----------
    /// value : float | Rate
    ///     Strike as a decimal (``0.05`` = 5%) or a ``Rate``.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not finite.
    /// TypeError
    ///     If ``value`` is neither a number nor a ``Rate``.
    #[pyo3(text_signature = "($self, value)")]
    fn strike<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let strike = rate_decimal_from_py(value, "strike")?;
        let strike = decimal_from_f64(strike, "strike")?;
        builder_set!(
            slf,
            strike,
            strike.to_string(),
            |b: CapFloorBuilderInner| b.strike(strike)
        )
    }

    /// Set the contractual margin added to the referenced rate.
    ///
    /// Parameters
    /// ----------
    /// value : float | Bps
    ///     Margin in basis points (``10`` = 10bp) or a ``Bps``, added after
    ///     projecting the index.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not finite.
    /// TypeError
    ///     If ``value`` is neither a number nor a ``Bps``.
    #[pyo3(text_signature = "($self, value)")]
    fn spread_bp<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spread_bp = crate::bindings::valuations::convert::bps_from_py(value, "spread_bp")?;
        let spread_bp = decimal_from_f64(spread_bp, "spread_bp")?;
        builder_set!(
            slf,
            spread_bp,
            spread_bp.to_string(),
            |b: CapFloorBuilderInner| b.spread_bp(spread_bp)
        )
    }

    /// Set the dated premium paid by the cap/floor holder.
    ///
    /// Parameters
    /// ----------
    /// payment_date : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Contractual premium payment date. Payments on or before the valuation
    ///     date are treated as settled and excluded from NPV.
    /// amount : Money | float
    ///     Non-negative premium outflow in the notional currency; a bare
    ///     number needs ``currency``.
    /// currency : str, optional
    ///     ISO-4217 code applied when ``amount`` is a bare number.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``payment_date`` cannot be converted to a date, a bare amount
    ///     has no ``currency``, or the builder was already consumed. Premium
    ///     amount and currency validation occurs in ``build``.
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
            |b: CapFloorBuilderInner| b.premium((payment_date, amount))
        )
    }

    /// Set the start date of the underlying period.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Start date of the underlying period.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
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
            |b: CapFloorBuilderInner| b.start_date(start_date)
        )
    }

    /// Set the end date of the underlying period.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     End date of the underlying period.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let maturity = py_to_date(value)?;
        builder_set!(
            slf,
            maturity,
            maturity.to_string(),
            |b: CapFloorBuilderInner| b.maturity(maturity)
        )
    }

    /// Set the payment frequency.
    ///
    /// Parameters
    /// ----------
    /// value : Tenor | str
    ///     Payment frequency for caps/floors.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a string ``value`` is not a recognized tenor string.
    #[pyo3(text_signature = "($self, value)")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let tenor = crate::bindings::valuations::convert::tenor_from_py(value, "frequency")?;
        builder_set!(
            slf,
            frequency,
            tenor.to_string(),
            |b: CapFloorBuilderInner| b.frequency(tenor)
        )
    }

    /// Set the day count convention.
    ///
    /// Parameters
    /// ----------
    /// value : DayCount | str
    ///     Day count convention.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a string ``value`` is not a recognized day-count name.
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
            |b: CapFloorBuilderInner| b.day_count(day_count)
        )
    }

    /// Set the stub rule (default ``"short_front"``).
    ///
    /// Parameters
    /// ----------
    /// value : StubKind | str
    ///     Stub rule.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized stub name.
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
            |b: CapFloorBuilderInner| b.stub(stub)
        )
    }

    /// Set the business day convention (default ``"modified_following"``).
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Serde name of the Rust ``BusinessDayConvention``.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized convention.
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
            |b: CapFloorBuilderInner| b.business_day_convention(convention)
        )
    }

    /// Set the holiday calendar identifier for schedule and roll conventions.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Holiday calendar identifier.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            calendar_id,
            format!("{value:?}"),
            |b: CapFloorBuilderInner| b.calendar_id(CalendarId::new(value.to_string()))
        )
    }

    /// Set the exercise style (default ``"european"``).
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Serde name of the Rust ``ExerciseStyle``.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized exercise style.
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
            |b: CapFloorBuilderInner| b.exercise_style(style)
        )
    }

    /// Set the settlement type (default ``"cash"``).
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Serde name of the Rust ``SettlementType``.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized settlement type.
    #[pyo3(text_signature = "($self, value)")]
    fn settlement<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let settlement = enum_from_str(value, "settlement")?;
        builder_set!(
            slf,
            settlement,
            format!("{value:?}"),
            |b: CapFloorBuilderInner| b.settlement(settlement)
        )
    }

    /// Set the discount curve identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Discount curve identifier.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            discount_curve_id,
            format!("{value:?}"),
            |b: CapFloorBuilderInner| b.discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    /// Set the forward curve identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Forward curve identifier.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn forward_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            forward_curve_id,
            format!("{value:?}"),
            |b: CapFloorBuilderInner| b.forward_curve_id(CurveId::new(value.to_string()))
        )
    }

    /// Set the volatility surface identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Volatility surface identifier.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            vol_surface_id,
            format!("{value:?}"),
            |b: CapFloorBuilderInner| b.vol_surface_id(CurveId::new(value.to_string()))
        )
    }

    /// Set the volatility type convention.
    ///
    /// Parameters
    /// ----------
    /// value : {"lognormal", "shifted_lognormal", "normal", "auto"}
    ///     Volatility convention. Must match the convention of the
    ///     configured volatility surface. ``"auto"`` (the default when unset)
    ///     resolves to ``"lognormal"``, pricing each caplet with Black-76
    ///     where well-defined and falling back to an equivalent Bachelier
    ///     price otherwise (e.g. a cap whose schedule crosses a zero forward
    ///     rate).
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized volatility type.
    #[pyo3(text_signature = "($self, value)")]
    fn vol_type<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let vol_type = enum_from_str(value, "vol_type")?;
        builder_set!(
            slf,
            vol_type,
            format!("{value:?}"),
            |b: CapFloorBuilderInner| b.vol_type(vol_type)
        )
    }

    /// Set the displacement shift used for shifted-lognormal pricing.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Displacement added to forward and strike. Must be non-negative.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn vol_shift<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            vol_shift,
            value.to_string(),
            |b: CapFloorBuilderInner| b.vol_shift(value)
        )
    }

    /// Set the overnight (RFR) coupon convention for compounded caplets.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     Rust ``OvernightCouponConvention`` in serde form, e.g.
    ///     ``{"compounding": {"compounded_in_arrears": {"lookback_days": 0}},
    ///     "payment_lag_days": 2}``.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not deserialize as an ``OvernightCouponConvention``.
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
            |b: CapFloorBuilderInner| b.overnight_coupon(convention)
        )
    }

    /// Set instrument attributes (tags and metadata).
    ///
    /// Parameters
    /// ----------
    /// value : Attributes | dict | None
    ///     Attribute bag: an ``Attributes`` or its serde ``dict`` form
    ///     (``{"tags": [...], "meta": {...}}``, as ``to_dict()`` returns it);
    ///     ``None`` clears it.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
    /// ValueError
    ///     If the ``dict`` has a key other than ``tags`` / ``meta`` or a non-string tag or value.
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
            |b: CapFloorBuilderInner| b.attributes(attrs)
        )
    }

    /// Build the validated cap/floor.
    ///
    /// Runs the same validation as Rust ``CapFloorBuilder::build``
    /// (structural invariants); pricing-time checks happen in ``price``.
    ///
    /// Returns
    /// -------
    /// CapFloor
    ///     The validated cap/floor.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed, a required field is missing
    ///     (the message names the builder and field), or the cap/floor fails
    ///     validation.
    #[pyo3(text_signature = "($self)")]
    fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<PyCapFloor> {
        let b = crate::bindings::valuations::convert::take_builder(&mut slf.inner)?;
        let inner = b.build().map_err(core_to_py)?;
        Ok(PyCapFloor { inner })
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        builder_repr("CapFloorBuilder", &self.fields)
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
