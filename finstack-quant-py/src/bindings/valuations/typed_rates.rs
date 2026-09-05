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
use crate::bindings::core::types::PyAttributes;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::extract::extract_market;
use crate::bindings::pandas_utils::serde_to_py;
use crate::errors::{core_to_py, value_error};
use finstack_quant_core::types::{CalendarId, CurveId, InstrumentId};
use finstack_quant_valuations::instruments::InstrumentJson;

use super::convert::{
    attributes_from_py, attributes_to_py, enum_to_py_string, money_from_py, money_to_py,
    rate_decimal_from_py,
};
use super::instruments::{
    builder_repr, decimal_from_f64, enum_from_str, instrument_default_model, instrument_expiry,
    instrument_market_dependencies, json_field, metric_typed_envelope, money_repr, opt_serde_to_py,
    parse_typed_instrument_json, price_typed_envelope, pricing_options_json,
    serialize_typed_instrument_json, spec_from_py, stub_kind_from_py,
};
use super::typed_legs::{PyFixedLegSpec, PyFloatLegSpec};
use super::PyValuationResult;

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
/// ``InterestRateSwap.example_standard()`` or ``InterestRateSwap.from_json``.
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

#[pymethods]
impl PyInterestRateSwap {
    /// Create a fluent builder (mirrors Rust ``InterestRateSwap::builder()``).
    ///
    /// Returns
    /// -------
    /// InterestRateSwapBuilder
    ///     A builder with fluent, consuming setter methods.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import InterestRateSwap
    /// >>> builder = InterestRateSwap.builder()
    /// >>> builder.id("EXAMPLE") is builder
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn builder() -> PyInterestRateSwapBuilder {
        PyInterestRateSwapBuilder {
            inner: Some(finstack_quant_valuations::instruments::InterestRateSwap::builder()),
            fields: Vec::new(),
        }
    }

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
    /// start : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Effective date.
    /// end : datetime.date | datetime.datetime | pandas.Timestamp | str
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
    /// >>> swap.float.reset_lag_days
    /// 0
    #[staticmethod]
    #[pyo3(signature = (id, notional, side, fixed_rate, start, end, index_id, discount_curve_id, forward_curve_id, *, currency = None))]
    #[pyo3(
        text_signature = "(id, notional, side, fixed_rate, start, end, index_id, discount_curve_id, forward_curve_id, *, currency=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn from_conventions(
        id: &str,
        notional: &Bound<'_, PyAny>,
        side: &str,
        fixed_rate: &Bound<'_, PyAny>,
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
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
            start: extract_date(start)?,
            end: extract_date(end)?,
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
    /// ``InterestRateSwap::example_standard``).
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
    /// >>> InterestRateSwap.example_standard().side
    /// 'pay'
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example_standard() -> PyResult<Self> {
        finstack_quant_valuations::instruments::InterestRateSwap::example_standard()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a validated swap from its canonical v1 envelope.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     A ``finstack_quant.instrument/1`` envelope containing an exact
    ///     ``"interest_rate_swap"`` payload. The UTF-8 input must not exceed
    ///     16 MiB. Bare payloads and cross-type coercion are rejected.
    ///
    /// Returns
    /// -------
    /// InterestRateSwap
    ///     The validated swap represented by the exact ``"interest_rate_swap"`` payload.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the input exceeds 16 MiB, is malformed, has an unsupported
    ///     envelope schema, carries another type, or fails swap validation.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import InterestRateSwap
    /// >>> try:
    /// ...     InterestRateSwap.from_json("{}")
    /// ... except ValueError as exc:
    /// ...     print("schema" in str(exc))
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        match parse_typed_instrument_json(json)? {
            InstrumentJson::InterestRateSwap(inner) => Ok(Self { inner }),
            _ => Err(value_error(
                "expected instrument type \"interest_rate_swap\", got a different instrument type",
            )),
        }
    }

    /// Serialize to a canonical ``finstack_quant.instrument/1`` envelope.
    ///
    /// Returns
    /// -------
    /// str
    ///     Canonical instrument envelope accepted by ``price_instrument`` and
    ///     ``InterestRateSwap.from_json``.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        self.envelope_json()
    }

    /// Serde form of the swap spec as a Python ``dict``.
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    /// Price the swap and return a ``ValuationResult``.
    ///
    /// Same pipeline and keyword surface as ``price_instrument``.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market context object or JSON string.
    /// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Valuation date.
    /// model : str, default "default"
    ///     Model key.
    /// metrics : list[str], optional
    ///     Metric identifiers to compute (e.g. ``["dv01", "par_rate"]``).
    /// pricing_options : dict | str, optional
    ///     ``MetricPricingOverrides`` merged into the instrument's overrides.
    /// market_history : str, optional
    ///     JSON ``MarketHistory`` scenarios for ``hvar`` / ``expected_shortfall``.
    ///
    /// Returns
    /// -------
    /// ValuationResult
    ///     Typed valuation envelope.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an input cannot be interpreted, the swap fails validation, or a
    ///     seasoned floating period needs a fixing that is absent (the message
    ///     names the ``FIXING:<index>`` series to insert).
    /// KeyError
    ///     If a required curve or metric is missing.
    /// RuntimeError
    ///     If pricing or a metric computation fails.
    #[pyo3(signature = (market, as_of, model="default", metrics=None, pricing_options=None, market_history=None))]
    #[pyo3(
        text_signature = "($self, market, as_of, model='default', metrics=None, pricing_options=None, market_history=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn price(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        model: &str,
        metrics: Option<Vec<String>>,
        pricing_options: Option<&Bound<'_, PyAny>>,
        market_history: Option<&str>,
    ) -> PyResult<PyValuationResult> {
        let options = pricing_options_json(py, pricing_options)?;
        price_typed_envelope(
            py,
            self.envelope_json()?,
            market,
            as_of,
            model,
            metrics,
            options,
            market_history,
        )
    }

    /// Compute one scalar metric (e.g. ``"dv01"``, ``"par_rate"``).
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market context object or JSON string.
    /// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Valuation date.
    /// metric_id : str
    ///     Registered metric identifier.
    /// model : str, default "default"
    ///     Model key.
    ///
    /// Returns
    /// -------
    /// float
    ///     The metric value.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``metric_id`` is unknown or an input cannot be interpreted.
    /// KeyError
    ///     If a required curve is missing.
    /// RuntimeError
    ///     If the metric computation fails.
    #[pyo3(signature = (market, as_of, metric_id, model="default"))]
    #[pyo3(text_signature = "($self, market, as_of, metric_id, model='default')")]
    fn metric(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        metric_id: &str,
        model: &str,
    ) -> PyResult<f64> {
        metric_typed_envelope(py, self.envelope_json()?, market, as_of, metric_id, model)
    }

    /// Instrument identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.to_string()
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
    fn fixed(&self) -> PyFixedLegSpec {
        PyFixedLegSpec::from_inner(self.inner.fixed.clone())
    }

    /// Floating leg specification.
    #[getter]
    fn float(&self) -> PyFloatLegSpec {
        PyFloatLegSpec::from_inner(self.inner.float.clone())
    }

    /// OTC margin (CSA / initial-margin) specification in serde form, or ``None``.
    #[getter]
    fn margin_spec<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.margin_spec.as_ref())
    }

    /// Instrument attributes (tags and metadata).
    #[getter]
    fn attributes(&self) -> PyAttributes {
        attributes_to_py(&self.inner.attributes)
    }

    /// Canonical model key used when ``model="default"``.
    #[getter]
    fn default_model(&self) -> String {
        instrument_default_model(&self.inner)
    }

    /// Expiry date exposed by the ``Instrument`` trait, or ``None``.
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    /// Market-data dependencies (discount/forward curves, fixings) as a dict.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Serde form of the Rust ``MarketDependencies``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the instrument cannot enumerate its dependencies.
    #[pyo3(text_signature = "($self)")]
    fn market_dependencies<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        instrument_market_dependencies(py, &self.inner)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "InterestRateSwap(id={:?}, notional={}, side={:?}, fixed_rate={}, start={}, end={}, forward_curve_id={:?})",
            self.inner.id.as_str(),
            money_repr(self.inner.notional),
            enum_to_py_string(&self.inner.side).unwrap_or_default(),
            self.inner.fixed.rate,
            self.inner.fixed.start,
            self.inner.fixed.end,
            self.inner.float.forward_curve_id.as_str(),
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

/// Take the wrapped Rust builder or fail if `build()` already consumed it.
fn take_irs(b: &mut PyInterestRateSwapBuilder) -> PyResult<IrsBuilder> {
    b.inner
        .take()
        .ok_or_else(|| value_error("builder already consumed by build()"))
}

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
        let b = take_irs(&mut slf)?;
        slf.inner = Some(b.id(InstrumentId::new(value.to_string())));
        slf.fields.push(("id", format!("{value:?}")));
        Ok(slf)
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
        let b = take_irs(&mut slf)?;
        slf.inner = Some(b.notional(money));
        slf.fields.push(("notional", money_repr(money)));
        Ok(slf)
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
        let b = take_irs(&mut slf)?;
        slf.inner = Some(b.side(side));
        slf.fields.push(("side", format!("{value:?}")));
        Ok(slf)
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
    fn fixed<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyFixedLegSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_irs(&mut slf)?;
        slf.inner = Some(b.fixed(value.inner.clone()));
        slf.fields.push(("fixed", value.__repr__()));
        Ok(slf)
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
    fn float<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyFloatLegSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_irs(&mut slf)?;
        slf.inner = Some(b.float(value.inner.clone()));
        slf.fields.push(("float", value.__repr__()));
        Ok(slf)
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
        let b = take_irs(&mut slf)?;
        slf.inner = Some(b.margin_spec(spec));
        slf.fields.push(("margin_spec", "{...}".to_string()));
        Ok(slf)
    }

    /// Set instrument attributes (tags and metadata).
    ///
    /// Parameters
    /// ----------
    /// value : Attributes | dict[str, str]
    ///     Attribute bag; a dict populates ``meta`` (a ``"tags"`` list entry
    ///     populates ``tags``).
    ///
    /// Returns
    /// -------
    /// InterestRateSwapBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``value`` is neither ``Attributes`` nor a dict.
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        let b = take_irs(&mut slf)?;
        slf.inner = Some(b.attributes(attrs));
        slf.fields
            .push(("attributes", "Attributes(...)".to_string()));
        Ok(slf)
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
        let b = take_irs(&mut slf)?;
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
/// ``Swaption.example_bermudan()`` or ``Swaption.from_json``. Every public
/// Rust field is readable as a property; ``get_strike`` / ``get_swap_start``
/// / ``get_swap_end`` / ``forward_swap_rate`` mirror the Rust accessors and
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

#[pymethods]
impl PySwaption {
    /// Create a fluent builder (mirrors Rust ``Swaption::builder()``).
    ///
    /// Returns
    /// -------
    /// SwaptionBuilder
    ///     A builder with fluent, consuming setter methods.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import Swaption
    /// >>> builder = Swaption.builder()
    /// >>> builder.id("EXAMPLE") is builder
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn builder() -> PySwaptionBuilder {
        PySwaptionBuilder {
            inner: Some(finstack_quant_valuations::instruments::Swaption::builder()),
            fields: Vec::new(),
        }
    }

    /// Canonical European 1Yx5Y USD payer swaption (mirrors Rust ``Swaption::example``).
    ///
    /// Returns
    /// -------
    /// Swaption
    ///     Cash-settled Black-vol swaption on a 3% 5-year swap, vol surface
    ///     ``USD-SWPNVOL``.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import Swaption
    /// >>> Swaption.example().get_strike()
    /// 0.03
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> Self {
        Self {
            inner: finstack_quant_valuations::instruments::Swaption::example(),
        }
    }

    /// Bermudan-exercise variant of the example (mirrors Rust
    /// ``Swaption::example_bermudan``).
    ///
    /// Returns
    /// -------
    /// Swaption
    ///     The example swaption with ``exercise_style == "bermudan"``.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import Swaption
    /// >>> Swaption.example_bermudan().exercise_style
    /// 'bermudan'
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example_bermudan() -> Self {
        Self {
            inner: finstack_quant_valuations::instruments::Swaption::example_bermudan(),
        }
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a validated swaption from its canonical v1 envelope.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     A ``finstack_quant.instrument/1`` envelope containing an exact
    ///     ``"swaption"`` payload. The UTF-8 input must not exceed 16 MiB.
    ///     Bare payloads and cross-type coercion are rejected.
    ///
    /// Returns
    /// -------
    /// Swaption
    ///     The validated swaption represented by the exact ``"swaption"`` payload.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the input exceeds 16 MiB, is malformed, has an unsupported
    ///     envelope schema, carries another type, or fails swaption validation.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import Swaption
    /// >>> try:
    /// ...     Swaption.from_json("{}")
    /// ... except ValueError as exc:
    /// ...     print("schema" in str(exc))
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        match parse_typed_instrument_json(json)? {
            InstrumentJson::Swaption(inner) => Ok(Self { inner }),
            _ => Err(value_error(
                "expected instrument type \"swaption\", got a different instrument type",
            )),
        }
    }

    /// Serialize to a canonical ``finstack_quant.instrument/1`` envelope.
    ///
    /// Returns
    /// -------
    /// str
    ///     Canonical instrument envelope accepted by ``price_instrument`` and
    ///     ``Swaption.from_json``.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        self.envelope_json()
    }

    /// Serde form of the swaption spec as a Python ``dict``.
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    /// Price the swaption and return a ``ValuationResult``.
    ///
    /// Same pipeline and keyword surface as ``price_instrument``.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market context object or JSON string.
    /// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Valuation date.
    /// model : str, default "default"
    ///     Model key (``"black76"``, ``"normal"``, ``"hull_white_1f"``, …).
    /// metrics : list[str], optional
    ///     Metric identifiers to compute.
    /// pricing_options : dict | str, optional
    ///     ``MetricPricingOverrides`` merged into the instrument's overrides.
    /// market_history : str, optional
    ///     JSON ``MarketHistory`` scenarios for ``hvar`` / ``expected_shortfall``.
    ///
    /// Returns
    /// -------
    /// ValuationResult
    ///     Typed valuation envelope.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an input cannot be interpreted or the swaption fails validation.
    /// KeyError
    ///     If a required curve, vol surface or metric is missing.
    /// RuntimeError
    ///     If pricing or a metric computation fails.
    #[pyo3(signature = (market, as_of, model="default", metrics=None, pricing_options=None, market_history=None))]
    #[pyo3(
        text_signature = "($self, market, as_of, model='default', metrics=None, pricing_options=None, market_history=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn price(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        model: &str,
        metrics: Option<Vec<String>>,
        pricing_options: Option<&Bound<'_, PyAny>>,
        market_history: Option<&str>,
    ) -> PyResult<PyValuationResult> {
        let options = pricing_options_json(py, pricing_options)?;
        price_typed_envelope(
            py,
            self.envelope_json()?,
            market,
            as_of,
            model,
            metrics,
            options,
            market_history,
        )
    }

    /// Compute one scalar metric (e.g. ``"delta"``, ``"vega"``).
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market context object or JSON string.
    /// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Valuation date.
    /// metric_id : str
    ///     Registered metric identifier.
    /// model : str, default "default"
    ///     Model key.
    ///
    /// Returns
    /// -------
    /// float
    ///     The metric value.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``metric_id`` is unknown or an input cannot be interpreted.
    /// KeyError
    ///     If a required curve or vol surface is missing.
    /// RuntimeError
    ///     If the metric computation fails.
    #[pyo3(signature = (market, as_of, metric_id, model="default"))]
    #[pyo3(text_signature = "($self, market, as_of, metric_id, model='default')")]
    fn metric(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        metric_id: &str,
        model: &str,
    ) -> PyResult<f64> {
        metric_typed_envelope(py, self.envelope_json()?, market, as_of, metric_id, model)
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
        let as_of = extract_date(as_of)?;
        let inner = self.inner.clone();
        py.detach(move || inner.forward_swap_rate(&market, as_of))
            .map_err(core_to_py)
    }

    /// Fixed strike of the underlying swap as a decimal (mirrors Rust ``get_strike``).
    #[pyo3(text_signature = "($self)")]
    fn get_strike(&self) -> f64 {
        decimal_f64(self.inner.get_strike())
    }

    /// Effective date of the underlying swap (mirrors Rust ``get_swap_start``).
    #[pyo3(text_signature = "($self)")]
    fn get_swap_start<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.get_swap_start())
    }

    /// Maturity of the underlying swap (mirrors Rust ``get_swap_end``).
    #[pyo3(text_signature = "($self)")]
    fn get_swap_end<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.get_swap_end())
    }

    /// Instrument identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.to_string()
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

    /// Exercise style: ``"european"``, ``"bermudan"`` or ``"american"``.
    #[getter]
    fn exercise_style(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.exercise_style)
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

    /// Instrument attributes (tags and metadata).
    #[getter]
    fn attributes(&self) -> PyAttributes {
        attributes_to_py(&self.inner.attributes)
    }

    /// Canonical model key used when ``model="default"``.
    #[getter]
    fn default_model(&self) -> String {
        instrument_default_model(&self.inner)
    }

    /// Market-data dependencies (curves, vol surface) as a dict.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Serde form of the Rust ``MarketDependencies``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the instrument cannot enumerate its dependencies.
    #[pyo3(text_signature = "($self)")]
    fn market_dependencies<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        instrument_market_dependencies(py, &self.inner)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "Swaption(id={:?}, option_type={:?}, notional={}, expiry={}, strike={}, swap_start={}, swap_end={}, vol_surface_id={:?})",
            self.inner.id.as_str(),
            enum_to_py_string(&self.inner.option_type).unwrap_or_default(),
            money_repr(self.inner.notional),
            self.inner.expiry,
            self.inner.get_strike(),
            self.inner.get_swap_start(),
            self.inner.get_swap_end(),
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

/// Take the wrapped Rust builder or fail if `build()` already consumed it.
fn take_swaption(b: &mut PySwaptionBuilder) -> PyResult<SwaptionBuilderInner> {
    b.inner
        .take()
        .ok_or_else(|| value_error("builder already consumed by build()"))
}

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
        let b = take_swaption(&mut slf)?;
        slf.inner = Some(b.id(InstrumentId::new(value.to_string())));
        slf.fields.push(("id", format!("{value:?}")));
        Ok(slf)
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
        let b = take_swaption(&mut slf)?;
        slf.inner = Some(b.option_type(option_type));
        slf.fields.push(("option_type", format!("{value:?}")));
        Ok(slf)
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
        let b = take_swaption(&mut slf)?;
        slf.inner = Some(b.notional(money));
        slf.fields.push(("notional", money_repr(money)));
        Ok(slf)
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
        let expiry = extract_date(value)?;
        let b = take_swaption(&mut slf)?;
        slf.inner = Some(b.expiry(expiry));
        slf.fields.push(("expiry", expiry.to_string()));
        Ok(slf)
    }

    /// Set the exercise style.
    ///
    /// Parameters
    /// ----------
    /// value : {"european", "bermudan", "american"}
    ///     Exercise style of the swaption.
    ///
    /// Returns
    /// -------
    /// SwaptionBuilder
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
        let exercise_style = enum_from_str(value, "exercise_style")?;
        let b = take_swaption(&mut slf)?;
        slf.inner = Some(b.exercise_style(exercise_style));
        slf.fields.push(("exercise_style", format!("{value:?}")));
        Ok(slf)
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
        let b = take_swaption(&mut slf)?;
        slf.inner = Some(b.settlement(settlement));
        slf.fields.push(("settlement", format!("{value:?}")));
        Ok(slf)
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
        let b = take_swaption(&mut slf)?;
        slf.inner = Some(b.cash_settlement_method(method));
        slf.fields
            .push(("cash_settlement_method", format!("{value:?}")));
        Ok(slf)
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
        let b = take_swaption(&mut slf)?;
        slf.inner = Some(b.vol_model(vol_model));
        slf.fields.push(("vol_model", format!("{value:?}")));
        Ok(slf)
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
        let b = take_swaption(&mut slf)?;
        slf.inner = Some(b.vol_surface_id(CurveId::new(value.to_string())));
        slf.fields.push(("vol_surface_id", format!("{value:?}")));
        Ok(slf)
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
        let b = take_swaption(&mut slf)?;
        slf.inner = Some(b.underlying_fixed_leg(value.inner.clone()));
        slf.fields.push(("underlying_fixed_leg", value.__repr__()));
        Ok(slf)
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
        let b = take_swaption(&mut slf)?;
        slf.inner = Some(b.underlying_float_leg(value.inner.clone()));
        slf.fields.push(("underlying_float_leg", value.__repr__()));
        Ok(slf)
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
        let b = take_swaption(&mut slf)?;
        slf.inner = Some(b.sabr_params(sabr_params));
        slf.fields.push(("sabr_params", "{...}".to_string()));
        Ok(slf)
    }

    /// Set the SABR volatility model parameters from a JSON string.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     JSON-encoded SABR parameters object with fields ``alpha``,
    ///     ``beta``, ``nu``, ``rho`` and optional ``shift``.
    ///
    /// Returns
    /// -------
    /// SwaptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not valid JSON for the SABR parameters shape.
    #[pyo3(text_signature = "($self, value)")]
    fn sabr_params_json<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let sabr_params: finstack_quant_models::volatility::SabrParameters =
            json_field(value, "sabr_params")?;
        let b = take_swaption(&mut slf)?;
        slf.inner = Some(b.sabr_params(sabr_params));
        slf.fields.push(("sabr_params", "{...}".to_string()));
        Ok(slf)
    }

    /// Set instrument attributes (tags and metadata).
    ///
    /// Parameters
    /// ----------
    /// value : Attributes | dict[str, str]
    ///     Attribute bag; a dict populates ``meta`` (a ``"tags"`` list entry
    ///     populates ``tags``).
    ///
    /// Returns
    /// -------
    /// SwaptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``value`` is neither ``Attributes`` nor a dict.
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        let b = take_swaption(&mut slf)?;
        slf.inner = Some(b.attributes(attrs));
        slf.fields
            .push(("attributes", "Attributes(...)".to_string()));
        Ok(slf)
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
        let b = take_swaption(&mut slf)?;
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

#[pymethods]
impl PyCapFloor {
    /// Create a fluent builder (mirrors Rust ``CapFloor::builder()``).
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     A builder with fluent, consuming setter methods.
    ///
    /// Notes
    /// -----
    /// This factory does not raise; it returns a new instance with the documented defaults.
    /// Unset ``vol_type`` defaults to ``"auto"``: the surface is treated as
    /// a lognormal quote. Each caplet uses Black-76 when forward and strike
    /// are positive; otherwise the lognormal vol is converted to an
    /// equivalent normal vol and priced with Bachelier. A normal-vol
    /// surface must set ``vol_type`` to ``"normal"``.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import CapFloor
    /// >>> builder = CapFloor.builder()
    /// >>> builder.id("EXAMPLE") is builder
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn builder() -> PyCapFloorBuilder {
        PyCapFloorBuilder {
            inner: Some(finstack_quant_valuations::instruments::CapFloor::builder()),
            fields: Vec::new(),
        }
    }

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

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a validated cap/floor from its canonical v1 envelope.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     A ``finstack_quant.instrument/1`` envelope containing an exact
    ///     ``"cap_floor"`` payload. The UTF-8 input must not exceed 16 MiB.
    ///     Bare payloads and cross-type coercion are rejected.
    ///
    /// Returns
    /// -------
    /// CapFloor
    ///     The validated cap/floor represented by the exact ``"cap_floor"`` payload.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the input exceeds 16 MiB, is malformed, has an unsupported
    ///     envelope schema, carries another type, or fails cap/floor validation.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import CapFloor
    /// >>> try:
    /// ...     CapFloor.from_json("{}")
    /// ... except ValueError as exc:
    /// ...     print("schema" in str(exc))
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        match parse_typed_instrument_json(json)? {
            InstrumentJson::CapFloor(inner) => Ok(Self { inner }),
            _ => Err(value_error(
                "expected instrument type \"cap_floor\", got a different instrument type",
            )),
        }
    }

    /// Serialize to a canonical ``finstack_quant.instrument/1`` envelope.
    ///
    /// Returns
    /// -------
    /// str
    ///     Canonical instrument envelope accepted by ``price_instrument`` and
    ///     ``CapFloor.from_json``.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        self.envelope_json()
    }

    /// Serde form of the cap/floor spec as a Python ``dict``.
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    /// Price the cap/floor and return a ``ValuationResult``.
    ///
    /// Same pipeline and keyword surface as ``price_instrument``.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market context object or JSON string.
    /// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Valuation date.
    /// model : str, default "default"
    ///     Model key (``"black76"``, ``"normal"``, ``"hull_white_1f"``, …).
    /// metrics : list[str], optional
    ///     Metric identifiers to compute.
    /// pricing_options : dict | str, optional
    ///     ``MetricPricingOverrides`` merged into the instrument's overrides.
    /// market_history : str, optional
    ///     JSON ``MarketHistory`` scenarios for ``hvar`` / ``expected_shortfall``.
    ///
    /// Returns
    /// -------
    /// ValuationResult
    ///     Typed valuation envelope.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an input cannot be interpreted or the instrument fails validation.
    /// KeyError
    ///     If a required curve, vol surface or metric is missing.
    /// RuntimeError
    ///     If pricing or a metric computation fails.
    #[pyo3(signature = (market, as_of, model="default", metrics=None, pricing_options=None, market_history=None))]
    #[pyo3(
        text_signature = "($self, market, as_of, model='default', metrics=None, pricing_options=None, market_history=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn price(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        model: &str,
        metrics: Option<Vec<String>>,
        pricing_options: Option<&Bound<'_, PyAny>>,
        market_history: Option<&str>,
    ) -> PyResult<PyValuationResult> {
        let options = pricing_options_json(py, pricing_options)?;
        price_typed_envelope(
            py,
            self.envelope_json()?,
            market,
            as_of,
            model,
            metrics,
            options,
            market_history,
        )
    }

    /// Compute one scalar metric (e.g. ``"delta"``, ``"vega"``).
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market context object or JSON string.
    /// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Valuation date.
    /// metric_id : str
    ///     Registered metric identifier.
    /// model : str, default "default"
    ///     Model key.
    ///
    /// Returns
    /// -------
    /// float
    ///     The metric value.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``metric_id`` is unknown or an input cannot be interpreted.
    /// KeyError
    ///     If a required curve or vol surface is missing.
    /// RuntimeError
    ///     If the metric computation fails.
    #[pyo3(signature = (market, as_of, metric_id, model="default"))]
    #[pyo3(text_signature = "($self, market, as_of, metric_id, model='default')")]
    fn metric(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        metric_id: &str,
        model: &str,
    ) -> PyResult<f64> {
        metric_typed_envelope(py, self.envelope_json()?, market, as_of, metric_id, model)
    }

    /// Instrument identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.to_string()
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

    /// Contractual spread added to the index, as a decimal rate.
    #[getter]
    fn spread(&self) -> f64 {
        decimal_f64(self.inner.spread)
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

    /// Instrument attributes (tags and metadata).
    #[getter]
    fn attributes(&self) -> PyAttributes {
        attributes_to_py(&self.inner.attributes)
    }

    /// Canonical model key used when ``model="default"``.
    #[getter]
    fn default_model(&self) -> String {
        instrument_default_model(&self.inner)
    }

    /// Expiry date exposed by the ``Instrument`` trait, or ``None``.
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    /// Market-data dependencies (curves, vol surface) as a dict.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Serde form of the Rust ``MarketDependencies``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the instrument cannot enumerate its dependencies.
    #[pyo3(text_signature = "($self)")]
    fn market_dependencies<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        instrument_market_dependencies(py, &self.inner)
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

/// Take the wrapped Rust builder or fail if `build()` already consumed it.
fn take_cap_floor(b: &mut PyCapFloorBuilder) -> PyResult<CapFloorBuilderInner> {
    b.inner
        .take()
        .ok_or_else(|| value_error("builder already consumed by build()"))
}

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
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.id(InstrumentId::new(value.to_string())));
        slf.fields.push(("id", format!("{value:?}")));
        Ok(slf)
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
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.rate_option_type(rate_option_type));
        slf.fields.push(("rate_option_type", format!("{value:?}")));
        Ok(slf)
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
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.notional(money));
        slf.fields.push(("notional", money_repr(money)));
        Ok(slf)
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
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.strike(strike));
        slf.fields.push(("strike", strike.to_string()));
        Ok(slf)
    }

    /// Set the contractual spread added to the referenced rate.
    ///
    /// Parameters
    /// ----------
    /// value : float | Rate
    ///     Spread in decimal rate units (``0.001`` = 10bp) or a ``Rate``,
    ///     added after projecting the index.
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
    fn spread<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spread = rate_decimal_from_py(value, "spread")?;
        let spread = decimal_from_f64(spread, "spread")?;
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.spread(spread));
        slf.fields.push(("spread", spread.to_string()));
        Ok(slf)
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
        let payment_date = extract_date(payment_date)?;
        let amount = money_from_py(amount, currency, "amount")?;
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.premium((payment_date, amount)));
        slf.fields.push((
            "premium",
            format!("({payment_date}, {})", money_repr(amount)),
        ));
        Ok(slf)
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
        let start_date = extract_date(value)?;
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.start_date(start_date));
        slf.fields.push(("start_date", start_date.to_string()));
        Ok(slf)
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
        let maturity = extract_date(value)?;
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.maturity(maturity));
        slf.fields.push(("maturity", maturity.to_string()));
        Ok(slf)
    }

    /// Set the payment frequency.
    ///
    /// Parameters
    /// ----------
    /// value : Tenor
    ///     Payment frequency for caps/floors.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyTenor>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let tenor = value.inner;
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.frequency(tenor));
        slf.fields.push(("frequency", tenor.to_string()));
        Ok(slf)
    }

    /// Set the day count convention.
    ///
    /// Parameters
    /// ----------
    /// value : DayCount
    ///     Day count convention.
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyDayCount>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let day_count = value.inner;
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.day_count(day_count));
        slf.fields.push(("day_count", day_count.to_string()));
        Ok(slf)
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
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.stub(stub));
        slf.fields.push((
            "stub",
            format!("{:?}", enum_to_py_string(&stub).unwrap_or_default()),
        ));
        Ok(slf)
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
        let convention = enum_from_str(value, "business_day_convention")?;
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.business_day_convention(convention));
        slf.fields
            .push(("business_day_convention", format!("{value:?}")));
        Ok(slf)
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
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.calendar_id(CalendarId::new(value.to_string())));
        slf.fields.push(("calendar_id", format!("{value:?}")));
        Ok(slf)
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
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.exercise_style(style));
        slf.fields.push(("exercise_style", format!("{value:?}")));
        Ok(slf)
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
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.settlement(settlement));
        slf.fields.push(("settlement", format!("{value:?}")));
        Ok(slf)
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
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.discount_curve_id(CurveId::new(value.to_string())));
        slf.fields.push(("discount_curve_id", format!("{value:?}")));
        Ok(slf)
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
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.forward_curve_id(CurveId::new(value.to_string())));
        slf.fields.push(("forward_curve_id", format!("{value:?}")));
        Ok(slf)
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
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.vol_surface_id(CurveId::new(value.to_string())));
        slf.fields.push(("vol_surface_id", format!("{value:?}")));
        Ok(slf)
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
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.vol_type(vol_type));
        slf.fields.push(("vol_type", format!("{value:?}")));
        Ok(slf)
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
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.vol_shift(value));
        slf.fields.push(("vol_shift", value.to_string()));
        Ok(slf)
    }

    /// Set the overnight (RFR) coupon convention for compounded caplets.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     Rust ``OvernightCouponConvention`` in serde form, e.g.
    ///     ``{"compounding": {"compounded_in_arrears": {"lookback_days": 0}},
    ///     "payment_delay_days": 2}``.
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
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.overnight_coupon(convention));
        slf.fields.push(("overnight_coupon", "{...}".to_string()));
        Ok(slf)
    }

    /// Set instrument attributes (tags and metadata).
    ///
    /// Parameters
    /// ----------
    /// value : Attributes | dict[str, str]
    ///     Attribute bag; a dict populates ``meta`` (a ``"tags"`` list entry
    ///     populates ``tags``).
    ///
    /// Returns
    /// -------
    /// CapFloorBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``value`` is neither ``Attributes`` nor a dict.
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        let b = take_cap_floor(&mut slf)?;
        slf.inner = Some(b.attributes(attrs));
        slf.fields
            .push(("attributes", "Attributes(...)".to_string()));
        Ok(slf)
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
        let b = take_cap_floor(&mut slf)?;
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
pub(crate) const EXPORTS: &[&str] = &[];
