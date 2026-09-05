//! Typed FX instruments: `FxForward` and `FxOption`.
//! Mirrors the `PyInterestRateSwap` pattern in `typed_rates.rs`.
//!
//! This module also hosts the pricing helpers shared by every typed
//! instrument wrapper (`price_envelope`, `envelope_metric_value`,
//! `envelope_option_greeks`) and the `instrument_pricing_methods!` macro that
//! stamps the common `price` / `metric` / `market_dependencies` /
//! `default_model` / `attributes` / `to_dict` surface onto a wrapper.

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::bindings::core::currency::PyCurrency;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::extract::extract_market;
use crate::errors::{core_to_py, value_error};
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fx::fx_option::{
    FxDeltaConvention, FxDeltaConventionKind,
};
use finstack_quant_valuations::instruments::{Instrument, InstrumentJson};

use super::convert::{
    attributes_from_py, bdc_from_py, builder_repr, currency_from_py, date_repr, day_count_from_py,
    enum_to_py_string, float_repr, money_repr, money_to_py, opt_repr, tenor_from_py,
};
use super::instruments::{enum_from_str, serialize_typed_instrument_json};
use super::pricing::binding_pricing_options;
use super::PyValuationResult;

/// Price a typed instrument envelope through the canonical Rust pricer.
///
/// # Arguments
///
/// * `py` - GIL token; the pricer runs with the GIL released.
/// * `envelope_json` - Canonical `finstack_quant.instrument/1` envelope.
/// * `market` - `MarketContext` object or market-context JSON string.
/// * `as_of` - Valuation date (date-like or ISO string).
/// * `model` - Model key (`"default"` selects the instrument-native model).
/// * `metrics` - Metric identifiers to compute alongside the valuation.
/// * `pricing_options` - Optional `MetricPricingOverrides` JSON.
/// * `market_history` - Optional `MarketHistory` JSON for historical metrics.
#[allow(clippy::too_many_arguments)]
pub(crate) fn price_envelope(
    py: Python<'_>,
    envelope_json: String,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    model: &str,
    metrics: Vec<String>,
    pricing_options: Option<&str>,
    market_history: Option<&str>,
) -> PyResult<PyValuationResult> {
    let market = extract_market(py, market)?;
    let as_of = crate::bindings::date_utils::extract_date_iso(as_of)?;
    let model = model.to_owned();
    let pricing_options = pricing_options.map(str::to_owned);
    let market_history = market_history.map(str::to_owned);
    let instrument = py.detach(move || {
        finstack_quant_valuations::pricer::parse_boxed_instrument_from_json(
            &envelope_json,
            pricing_options.as_deref(),
        )
        .map_err(core_to_py)
    })?;
    let inner = py
        .detach(move || {
            finstack_quant_valuations::pricer::price_instrument(
                &instrument,
                &market,
                &as_of,
                &model,
                &metrics,
                market_history.as_deref(),
                binding_pricing_options(),
            )
        })
        .map_err(core_to_py)?;
    Ok(PyValuationResult { inner })
}

/// Compute one scalar metric for a typed instrument envelope.
///
/// # Arguments
///
/// * `py` - GIL token; the pricer runs with the GIL released.
/// * `envelope_json` - Canonical `finstack_quant.instrument/1` envelope.
/// * `market` - `MarketContext` object or market-context JSON string.
/// * `as_of` - Valuation date (date-like or ISO string).
/// * `model` - Model key (`"default"` selects the instrument-native model).
/// * `metric` - Fully qualified metric identifier (`"dv01"`, `"cs01"`, …).
pub(crate) fn envelope_metric_value(
    py: Python<'_>,
    envelope_json: String,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    model: &str,
    metric: &str,
) -> PyResult<f64> {
    let market = extract_market(py, market)?;
    let as_of = crate::bindings::date_utils::extract_date_iso(as_of)?;
    let model = model.to_owned();
    let metric = metric.to_owned();
    py.detach(move || {
        let instrument = finstack_quant_valuations::pricer::parse_boxed_instrument_from_json(
            &envelope_json,
            None,
        )?;
        finstack_quant_valuations::pricer::metric_value(
            &instrument,
            &market,
            &as_of,
            &model,
            &metric,
            binding_pricing_options(),
        )
    })
    .map_err(core_to_py)
}

/// Compute the standard option Greek set for a typed instrument envelope.
///
/// Mirrors the WASM `greeks` method: non-finite Greeks are rejected rather
/// than returned, so both hosts fail identically instead of one silently
/// yielding `NaN`.
///
/// # Arguments
///
/// * `py` - GIL token; the pricer runs with the GIL released.
/// * `envelope_json` - Canonical `finstack_quant.instrument/1` envelope.
/// * `market` - `MarketContext` object or market-context JSON string.
/// * `as_of` - Valuation date (date-like or ISO string).
/// * `model` - Model key (`"default"` selects the instrument-native model).
pub(crate) fn envelope_option_greeks<'py>(
    py: Python<'py>,
    envelope_json: String,
    market: &Bound<'py, PyAny>,
    as_of: &Bound<'py, PyAny>,
    model: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let market = extract_market(py, market)?;
    let as_of = crate::bindings::date_utils::extract_date_iso(as_of)?;
    let model = model.to_owned();
    let pairs = py
        .detach(move || {
            let instrument = finstack_quant_valuations::pricer::parse_boxed_instrument_from_json(
                &envelope_json,
                None,
            )?;
            finstack_quant_valuations::pricer::present_standard_option_greeks(
                &instrument,
                &market,
                &as_of,
                &model,
                binding_pricing_options(),
            )
        })
        .map_err(core_to_py)?;
    let out = PyDict::new(py);
    for (metric, value) in pairs {
        out.set_item(metric, value)?;
    }
    Ok(out)
}

/// Coerce an optional `dict | str` of `MetricPricingOverrides` to JSON.
///
/// # Arguments
///
/// * `py` - GIL token used for `json.dumps` on dict inputs.
/// * `obj` - `None`, a JSON string, or a dict of override fields.
pub(crate) fn pricing_options_json(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<String>> {
    match obj {
        None => Ok(None),
        Some(value) if value.is_none() => Ok(None),
        Some(value) => Ok(Some(crate::bindings::module_utils::py_to_json_string(
            py,
            value,
            "pricing_options",
        )?)),
    }
}

/// Stamp the pricing surface shared by every typed instrument wrapper.
///
/// Expands to a `#[pymethods]` block (the crate enables
/// `multiple-pymethods`) with `price`, `metric`, `market_dependencies`,
/// `default_model`, `attributes` and `to_dict`. The wrapper must expose
/// `pub(crate) inner` (the Rust instrument) and `envelope_json()`.
macro_rules! instrument_pricing_methods {
    ($ty:ident) => {
        #[pymethods]
        impl $ty {
            /// Price this instrument and return a typed ``ValuationResult``.
            ///
            /// Delegates to the same canonical Rust pricer entry point as
            /// ``price_instrument(self, market, as_of, model)``.
            ///
            /// Parameters
            /// ----------
            /// market : MarketContext | str
            ///     A ``MarketContext`` object or serialized market-context JSON.
            /// as_of : datetime.date | str
            ///     Valuation date, either a date-like object or an ISO 8601 string.
            /// model : str, optional
            ///     Model key (default ``"default"`` — the instrument-native model).
            /// metrics : list[str], optional
            ///     Metric identifiers to compute (e.g. ``["dv01", "theta"]``).
            ///     Empty or omitted means valuation only.
            /// pricing_options : dict | str | None
            ///     Optional ``MetricPricingOverrides`` (dict or JSON string) merged
            ///     into the instrument's ``pricing_overrides`` before pricing.
            /// market_history : str | None
            ///     Optional JSON ``MarketHistory`` scenarios required by ``hvar`` and
            ///     ``expected_shortfall`` metrics.
            ///
            /// Returns
            /// -------
            /// ValuationResult
            ///     Typed valuation envelope carrying value, currency, and metrics.
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If the market JSON, ``as_of``, or ``model`` is invalid, or the
            ///     selected pricer rejects the instrument.
            /// KeyError
            ///     If a curve, surface, or price the instrument depends on is
            ///     missing from ``market``.
            /// RuntimeError
            ///     If the pricer or a requested metric fails numerically.
            #[pyo3(signature = (market, as_of, model="default", metrics=None, pricing_options=None, market_history=None))]
            #[pyo3(text_signature = "($self, market, as_of, model='default', metrics=None, pricing_options=None, market_history=None)")]
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
            ) -> PyResult<$crate::bindings::valuations::PyValuationResult> {
                let pricing_options =
                    $crate::bindings::valuations::typed_fx::pricing_options_json(py, pricing_options)?;
                $crate::bindings::valuations::typed_fx::price_envelope(
                    py,
                    self.envelope_json()?,
                    market,
                    as_of,
                    model,
                    metrics.unwrap_or_default(),
                    pricing_options.as_deref(),
                    market_history,
                )
            }

            /// Compute one scalar metric for this instrument.
            ///
            /// Mirrors Rust ``pricer::metric_value``: the instrument is priced
            /// under ``model`` and the single metric ``metric_id`` is returned as
            /// a float.
            ///
            /// Parameters
            /// ----------
            /// market : MarketContext | str
            ///     A ``MarketContext`` object or serialized market-context JSON.
            /// as_of : datetime.date | str
            ///     Valuation date, either a date-like object or an ISO 8601 string.
            /// metric_id : str
            ///     Fully qualified metric identifier, e.g. ``"dv01"``,
            ///     ``"cs01"``, ``"delta"``.
            /// model : str, optional
            ///     Model key (default ``"default"`` — the instrument-native model).
            ///
            /// Returns
            /// -------
            /// float
            ///     The metric value in the metric's documented unit.
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If ``metric_id`` is unknown, ``as_of`` or ``model`` is invalid,
            ///     or the metric is not defined for this instrument.
            /// KeyError
            ///     If required market data is missing from ``market``.
            /// RuntimeError
            ///     If the metric computation fails numerically.
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
                $crate::bindings::valuations::typed_fx::envelope_metric_value(
                    py,
                    self.envelope_json()?,
                    market,
                    as_of,
                    model,
                    metric_id,
                )
            }

            /// Market objects this instrument needs for pricing.
            ///
            /// Mirrors Rust ``Instrument::market_dependencies``.
            ///
            /// Returns
            /// -------
            /// dict[str, object]
            ///     Serde view of ``MarketDependencies``: ``curves`` (discount /
            ///     forward / credit / inflation curve ids), ``credit_index_ids``,
            ///     ``market_scalar_ids``, ``volatility_dependencies``,
            ///     ``fx_pairs`` and ``series_ids``.
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If the instrument cannot enumerate its dependencies.
            #[pyo3(text_signature = "($self)")]
            fn market_dependencies<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
                let deps = finstack_quant_valuations::instruments::Instrument::market_dependencies(
                    &self.inner,
                )
                .map_err($crate::errors::core_to_py)?;
                $crate::bindings::pandas_utils::serde_to_py(py, &deps)
            }

            /// Model key the pricer uses when ``model="default"``.
            ///
            /// Returns
            /// -------
            /// str
            ///     Canonical model key, e.g. ``"hazard_rate"`` or ``"black76"``.
            #[getter]
            fn default_model(&self) -> String {
                finstack_quant_valuations::instruments::Instrument::default_model(&self.inner)
                    .to_string()
            }

            /// Free-form instrument attributes (tags and metadata).
            ///
            /// Returns
            /// -------
            /// Attributes
            ///     Copy of the instrument's attribute bag.
            #[getter]
            fn attributes(&self) -> $crate::bindings::core::types::PyAttributes {
                $crate::bindings::valuations::convert::attributes_to_py(
                    finstack_quant_valuations::instruments::Instrument::attributes(&self.inner),
                )
            }

            /// Instrument specification as a plain dict.
            ///
            /// Returns
            /// -------
            /// dict[str, object]
            ///     The canonical ``spec`` payload (the same fields ``to_json``
            ///     wraps in the ``finstack_quant.instrument/1`` envelope).
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If the instrument cannot be serialized.
            #[pyo3(text_signature = "($self)")]
            fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
                $crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
            }
        }
    };
}
pub(crate) use instrument_pricing_methods;

/// Stamp `__reduce__` / `from_json` / `to_json` / `id` / `builder` on a typed
/// instrument wrapper.
///
/// `$variant` is the `InstrumentJson` variant, `$type_tag` the serde type tag
/// (`"fx_forward"`), `$builder` the Python builder wrapper and `$seed` an
/// expression producing the seeded Rust builder.
macro_rules! instrument_envelope_methods {
    ($ty:ident, $variant:ident, $type_tag:literal, $builder:ident, $seed:expr) => {
        #[pymethods]
        impl $ty {
            /// Create a fluent builder (mirrors the Rust ``builder()``).
            ///
            /// Builders are consumed by ``build()``; create a new builder per
            /// instrument.
            ///
            /// Returns
            /// -------
            /// builder
            ///     A builder with fluent, consuming setter methods.
            #[staticmethod]
            #[pyo3(text_signature = "()")]
            fn builder() -> $builder {
                $builder {
                    inner: Some($seed),
                    fields: Vec::new(),
                }
            }

            /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
            ///
            /// Reconstruction goes through the same strict serde round-trip as
            /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
            /// format defines — there is no second state format that can drift.
            fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
                let from_json = py.get_type::<Self>().getattr("from_json")?;
                $crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
            }

            /// Deserialize a validated instrument from its canonical v1 envelope.
            ///
            /// Parameters
            /// ----------
            /// json : str
            #[doc = concat!(
                        "    A ``finstack_quant.instrument/1`` envelope carrying an exact \"",
                        $type_tag,
                        "\" payload. The UTF-8 input must not exceed 16 MiB. Bare payloads \
                 and cross-type coercion are rejected."
                    )]
            ///
            /// Returns
            /// -------
            /// instrument
            ///     The validated instrument.
            ///
            /// Raises
            /// ------
            /// ValueError
            #[doc = concat!(
                        "    If the input exceeds 16 MiB, is malformed, has an unsupported \
                 envelope schema, carries a type other than \"",
                        $type_tag,
                        "\", or fails validation."
                    )]
            #[staticmethod]
            #[pyo3(text_signature = "(json)")]
            fn from_json(json: &str) -> PyResult<Self> {
                match $crate::bindings::valuations::instruments::parse_typed_instrument_json(json)?
                {
                    InstrumentJson::$variant(inner) => Ok(Self { inner }),
                    _ => Err($crate::errors::value_error(concat!(
                        "expected instrument type \"",
                        $type_tag,
                        "\", got a different instrument type"
                    ))),
                }
            }

            /// Serialize to a canonical ``finstack_quant.instrument/1`` envelope.
            ///
            /// Returns
            /// -------
            /// str
            ///     Canonical instrument envelope accepted by ``price_instrument`` and
            ///     ``from_json``.
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If the value cannot be serialized to JSON.
            #[pyo3(text_signature = "($self)")]
            fn to_json(&self) -> PyResult<String> {
                self.envelope_json()
            }

            /// Instrument identifier.
            #[getter]
            fn id(&self) -> String {
                self.inner.id.to_string()
            }
        }
    };
}
pub(crate) use instrument_envelope_methods;

/// Shared `build()` body: take the Rust builder, run the single Rust
/// validation (`build()`), wrap.
///
/// # Arguments
///
/// * `inner` - Slot holding the consuming Rust builder (`None` once consumed).
pub(crate) fn take_builder<B>(inner: &mut Option<B>) -> PyResult<B> {
    inner
        .take()
        .ok_or_else(|| value_error("builder already consumed by build()"))
}

type FxForwardBuilderInner =
    finstack_quant_valuations::instruments::fx::fx_forward::FxForwardBuilder;
type FxOptionBuilderInner = finstack_quant_valuations::instruments::fx::fx_option::FxOptionBuilder;

/// Outright FX forward on a currency pair (typed wrapper for Rust ``FxForward``).
///
/// The notional is denominated in ``base_currency``; PV is reported in
/// ``quote_currency`` via covered interest parity (CIRP). A missing
/// ``contract_rate`` values the forward at-market (zero PV at inception).
///
/// Build with ``FxForward.builder()``, ``FxForward.from_trade_date(...)`` or
/// start from ``FxForward.example()``; instances are accepted directly by
/// ``price_instrument`` and expose ``price`` / ``metric`` themselves.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import FxForward
/// >>> fwd = FxForward.example()
/// >>> (fwd.base_currency.code, fwd.quote_currency.code, fwd.contract_rate)
/// ('EUR', 'USD', 1.12)
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "FxForward",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFxForward {
    /// Inner canonical Rust FX forward.
    pub(crate) inner: finstack_quant_valuations::instruments::FxForward,
}

impl PyFxForward {
    /// Serialize as the canonical instrument envelope accepted by the JSON loader.
    pub(crate) fn envelope_json(&self) -> PyResult<String> {
        serialize_typed_instrument_json(InstrumentJson::FxForward(self.inner.clone()), "FxForward")
    }
}

instrument_envelope_methods!(
    PyFxForward,
    FxForward,
    "fx_forward",
    PyFxForwardBuilder,
    finstack_quant_valuations::instruments::FxForward::builder()
);
instrument_pricing_methods!(PyFxForward);

#[pymethods]
impl PyFxForward {
    /// Canonical example: 6-month EUR/USD forward, EUR 1,000,000 at 1.12.
    ///
    /// Mirrors Rust ``FxForward::example()`` (curves ``USD-OIS`` /
    /// ``EUR-OIS``, maturity 2025-06-15).
    ///
    /// Returns
    /// -------
    /// FxForward
    ///     The validated example forward.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the canonical example fails validation (never for a released build).
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::FxForward::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Build a forward from a trade date and a standard FX tenor.
    ///
    /// Mirrors Rust ``FxForward::from_trade_date``: the spot date is rolled
    /// from ``trade_date`` by ``spot_lag_days`` business days (CLS-consistent
    /// pair roll), then ``tenor`` is added with the FX end-of-month rule and
    /// ``business_day_convention``.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique instrument identifier.
    /// base_currency : Currency | str
    ///     Base (foreign) currency; notional currency.
    /// quote_currency : Currency | str
    ///     Quote (domestic) currency; PV currency.
    /// trade_date : datetime.date | str
    ///     Trade date from which spot is rolled.
    /// tenor : Tenor | str
    ///     Standard FX tenor from spot, e.g. ``"3M"`` or ``Tenor.parse("6M")``.
    /// notional : Money | float
    ///     Notional in ``base_currency``; a bare float is tagged with that currency.
    /// domestic_discount_curve_id : str
    ///     Quote-currency discount curve identifier.
    /// foreign_discount_curve_id : str
    ///     Base-currency discount curve identifier.
    /// base_calendar_id : str | None
    ///     Base-currency holiday calendar; ``None`` uses weekends only.
    /// quote_calendar_id : str | None
    ///     Quote-currency holiday calendar; ``None`` uses weekends only.
    /// spot_lag_days : int | None
    ///     Spot lag in business days; ``None`` uses the market standard for
    ///     the pair (``FxForward.standard_spot_days``): T+1 for USD/CAD,
    ///     USD/TRY, USD/RUB and T+2 otherwise.
    /// business_day_convention : BusinessDayConvention | str | None
    ///     Roll rule applied to the maturity; ``None`` means ``"modified_following"``.
    /// end_of_month : bool
    ///     Apply the FX end-of-month rule when spot falls on month end.
    ///
    /// Returns
    /// -------
    /// FxForward
    ///     Validated at-market forward (no ``contract_rate``); chain
    ///     ``with_forward_points`` / ``with_forward_pips`` to fix the rate.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the currencies coincide, the tenor/date is invalid, or the
    ///     notional currency differs from ``base_currency``.
    /// KeyError
    ///     If a calendar identifier is unknown.
    #[staticmethod]
    #[pyo3(signature = (id, base_currency, quote_currency, trade_date, tenor, notional,
                        domestic_discount_curve_id, foreign_discount_curve_id, *,
                        base_calendar_id=None, quote_calendar_id=None, spot_lag_days=None,
                        business_day_convention=None, end_of_month=false))]
    #[pyo3(
        text_signature = "(id, base_currency, quote_currency, trade_date, tenor, notional, \
domestic_discount_curve_id, foreign_discount_curve_id, *, base_calendar_id=None, \
quote_calendar_id=None, spot_lag_days=None, business_day_convention=None, \
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
        spot_lag_days: Option<i32>,
        business_day_convention: Option<&Bound<'_, PyAny>>,
        end_of_month: bool,
    ) -> PyResult<Self> {
        let base = currency_from_py(base_currency, "base_currency")?;
        let quote = currency_from_py(quote_currency, "quote_currency")?;
        let bdc = match business_day_convention {
            Some(value) if !value.is_none() => bdc_from_py(value, "business_day_convention")?,
            _ => finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing,
        };
        let spot_lag_days = match spot_lag_days {
            Some(days) => days,
            None => i32::try_from(
                finstack_quant_valuations::instruments::FxForward::standard_spot_days(base, quote),
            )
            .map_err(|_| value_error("standard spot lag does not fit in i32"))?,
        };
        let inner = finstack_quant_valuations::instruments::FxForward::from_trade_date(
            InstrumentId::new(id.to_string()),
            base,
            quote,
            extract_date(trade_date)?,
            tenor_from_py(tenor, "tenor")?,
            super::convert::money_from_py(notional, Some(base.as_ref()), "notional")?,
            CurveId::new(domestic_discount_curve_id.to_string()),
            CurveId::new(foreign_discount_curve_id.to_string()),
            base_calendar_id,
            quote_calendar_id,
            spot_lag_days,
            bdc,
            end_of_month,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Market-standard spot lag (business days) for a currency pair.
    ///
    /// Mirrors Rust ``FxForward::standard_spot_days``.
    ///
    /// Parameters
    /// ----------
    /// base : Currency | str
    ///     Base currency of the pair.
    /// quote : Currency | str
    ///     Quote currency of the pair.
    ///
    /// Returns
    /// -------
    /// int
    ///     ``1`` for USD/CAD, USD/TRY, USD/RUB (either order); ``2`` otherwise.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a currency code is not ISO-4217.
    #[staticmethod]
    #[pyo3(text_signature = "(base, quote)")]
    fn standard_spot_days(base: &Bound<'_, PyAny>, quote: &Bound<'_, PyAny>) -> PyResult<u32> {
        Ok(
            finstack_quant_valuations::instruments::FxForward::standard_spot_days(
                currency_from_py(base, "base")?,
                currency_from_py(quote, "quote")?,
            ),
        )
    }

    /// Return a copy whose contract rate is ``spot_rate + forward_points``.
    ///
    /// Mirrors Rust ``FxForward::with_forward_points``. Forward points are
    /// in rate units (e.g. ``0.0025`` for 25 pips on EUR/USD); use
    /// ``with_forward_pips`` to pass pips directly.
    ///
    /// Parameters
    /// ----------
    /// spot_rate : float
    ///     Spot rate, quote currency per unit of base currency; must be positive.
    /// forward_points : float
    ///     Forward points in rate units, added to ``spot_rate``.
    ///
    /// Returns
    /// -------
    /// FxForward
    ///     New forward with ``contract_rate`` set.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``spot_rate`` is not positive/finite, or the resulting contract
    ///     rate is not positive.
    #[pyo3(text_signature = "($self, spot_rate, forward_points)")]
    fn with_forward_points(&self, spot_rate: f64, forward_points: f64) -> PyResult<Self> {
        self.inner
            .clone()
            .with_forward_points(spot_rate, forward_points)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Return a copy whose contract rate is ``spot_rate + pips * pip_size``.
    ///
    /// Mirrors Rust ``FxForward::with_forward_pips``; the pip size follows
    /// market convention (``0.01`` for JPY/KRW/HUF pairs, ``0.0001`` otherwise).
    ///
    /// Parameters
    /// ----------
    /// spot_rate : float
    ///     Spot rate, quote currency per unit of base currency; must be positive.
    /// pips : float
    ///     Forward points quoted in pips.
    ///
    /// Returns
    /// -------
    /// FxForward
    ///     New forward with ``contract_rate`` set.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``pips`` or ``spot_rate`` is not finite, or the resulting
    ///     contract rate is not positive.
    #[pyo3(text_signature = "($self, spot_rate, pips)")]
    fn with_forward_pips(&self, spot_rate: f64, pips: f64) -> PyResult<Self> {
        self.inner
            .clone()
            .with_forward_pips(spot_rate, pips)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Covered-interest-parity forward rate implied by the market.
    ///
    /// Mirrors Rust ``FxForward::market_forward_rate``:
    /// ``F = S * DF_foreign(T) / DF_domestic(T)``.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market carrying both discount curves and the FX matrix (or an
    ///     explicit ``spot_rate_override`` on the instrument).
    /// as_of : datetime.date | str
    ///     Valuation date.
    ///
    /// Returns
    /// -------
    /// float
    ///     Forward rate, quote currency per unit of base currency.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If a discount curve or the FX spot is missing from ``market``.
    /// ValueError
    ///     If the market JSON or ``as_of`` is invalid.
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn market_forward_rate(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        let as_of = extract_date(as_of)?;
        self.inner
            .market_forward_rate(&market, as_of)
            .map_err(core_to_py)
    }

    /// Base (foreign) currency; the notional currency.
    #[getter]
    fn base_currency(&self) -> PyCurrency {
        PyCurrency::from_inner(self.inner.base_currency)
    }

    /// Quote (domestic) currency; the PV currency.
    #[getter]
    fn quote_currency(&self) -> PyCurrency {
        PyCurrency::from_inner(self.inner.quote_currency)
    }

    /// Maturity / settlement date.
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    /// Notional amount in the base currency.
    #[getter]
    fn notional(&self) -> PyMoney {
        money_to_py(self.inner.notional)
    }

    /// Contract forward rate (quote per base), or ``None`` when at-market.
    #[getter]
    fn contract_rate(&self) -> Option<f64> {
        self.inner.contract_rate
    }

    /// Domestic (quote-currency) discount curve identifier.
    #[getter]
    fn domestic_discount_curve_id(&self) -> String {
        self.inner.domestic_discount_curve_id.to_string()
    }

    /// Foreign (base-currency) discount curve identifier.
    #[getter]
    fn foreign_discount_curve_id(&self) -> String {
        self.inner.foreign_discount_curve_id.to_string()
    }

    /// Explicit spot override (quote per base), or ``None`` to use the FX matrix.
    #[getter]
    fn spot_rate_override(&self) -> Option<f64> {
        self.inner.spot_rate_override
    }

    /// Base-currency holiday calendar identifier, if any.
    #[getter]
    fn base_calendar_id(&self) -> Option<String> {
        self.inner.base_calendar_id.clone()
    }

    /// Quote-currency holiday calendar identifier, if any.
    #[getter]
    fn quote_calendar_id(&self) -> Option<String> {
        self.inner.quote_calendar_id.clone()
    }

    /// Expiry as seen by the pricer (``None``: FX forwards carry no option expiry).
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

/// Fluent builder for ``FxForward``; wraps the Rust
/// ``FinancialBuilder``-generated builder (consuming setters).
///
/// Builders are consumed by ``build()``; create a new builder per instrument.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "FxForwardBuilder",
    skip_from_py_object
)]
pub struct PyFxForwardBuilder {
    inner: Option<FxForwardBuilderInner>,
    fields: Vec<(&'static str, String)>,
}

/// Apply one consuming Rust setter and record the field for ``__repr__``.
macro_rules! fx_forward_set {
    ($slf:ident, $field:ident, $repr:expr, $apply:expr) => {{
        let b = take_builder(&mut $slf.inner)?;
        $slf.inner = Some($apply(b));
        $slf.fields.push((stringify!($field), $repr));
        Ok($slf)
    }};
}

#[pymethods]
impl PyFxForwardBuilder {
    /// Set the instrument identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Unique identifier for the FX forward.
    ///
    /// Returns
    /// -------
    /// FxForwardBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        fx_forward_set!(slf, id, format!("{value:?}"), |b: FxForwardBuilderInner| b
            .id(InstrumentId::new(value.to_string())))
    }

    /// Set the base currency (foreign currency, numerator of the pair).
    ///
    /// Parameters
    /// ----------
    /// value : Currency | str
    ///     Base (foreign) currency, as a ``Currency`` or ISO-4217 code.
    ///
    /// Returns
    /// -------
    /// FxForwardBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a string code is not ISO-4217.
    #[pyo3(text_signature = "($self, value)")]
    fn base_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let ccy = currency_from_py(value, "base_currency")?;
        fx_forward_set!(
            slf,
            base_currency,
            format!("Currency('{ccy}')"),
            |b: FxForwardBuilderInner| b.base_currency(ccy)
        )
    }

    /// Set the quote currency (domestic currency, denominator of the pair).
    ///
    /// Parameters
    /// ----------
    /// value : Currency | str
    ///     Quote (domestic) currency; also the PV currency.
    ///
    /// Returns
    /// -------
    /// FxForwardBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a string code is not ISO-4217.
    #[pyo3(text_signature = "($self, value)")]
    fn quote_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let ccy = currency_from_py(value, "quote_currency")?;
        fx_forward_set!(
            slf,
            quote_currency,
            format!("Currency('{ccy}')"),
            |b: FxForwardBuilderInner| b.quote_currency(ccy)
        )
    }

    /// Set the maturity/settlement date.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | str
    ///     Maturity/settlement date (date-like or ISO 8601 string).
    ///
    /// Returns
    /// -------
    /// FxForwardBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let maturity = extract_date(value)?;
        fx_forward_set!(
            slf,
            maturity,
            date_repr(maturity),
            |b: FxForwardBuilderInner| b.maturity(maturity)
        )
    }

    /// Set the notional amount in base currency.
    ///
    /// Parameters
    /// ----------
    /// value : Money
    ///     Notional amount, denominated in the base currency.
    ///
    /// Returns
    /// -------
    /// FxForwardBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = value.inner;
        fx_forward_set!(
            slf,
            notional,
            money_repr(money),
            |b: FxForwardBuilderInner| b.notional(money)
        )
    }

    /// Set the contract forward rate (quote per base).
    ///
    /// If not set, the forward is valued at-market (zero PV at inception).
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Contract forward rate, quote currency per unit of base currency.
    ///
    /// Returns
    /// -------
    /// FxForwardBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn contract_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        fx_forward_set!(
            slf,
            contract_rate,
            float_repr(value),
            |b: FxForwardBuilderInner| b.contract_rate(value)
        )
    }

    /// Set the domestic (quote currency) discount curve identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Domestic (quote currency) discount curve identifier.
    ///
    /// Returns
    /// -------
    /// FxForwardBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn domestic_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        fx_forward_set!(
            slf,
            domestic_discount_curve_id,
            format!("{value:?}"),
            |b: FxForwardBuilderInner| b
                .domestic_discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    /// Set the foreign (base currency) discount curve identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Foreign (base currency) discount curve identifier.
    ///
    /// Returns
    /// -------
    /// FxForwardBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn foreign_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        fx_forward_set!(
            slf,
            foreign_discount_curve_id,
            format!("{value:?}"),
            |b: FxForwardBuilderInner| b.foreign_discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    /// Set an explicit spot rate override (quote per base).
    ///
    /// If not set, the spot rate is sourced from the market's FX matrix.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Spot FX rate, quote currency per unit of base currency.
    ///
    /// Returns
    /// -------
    /// FxForwardBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn spot_rate_override<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        fx_forward_set!(
            slf,
            spot_rate_override,
            float_repr(value),
            |b: FxForwardBuilderInner| b.spot_rate_override(value)
        )
    }

    /// Set the base currency calendar identifier for business day adjustment.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Base currency holiday calendar identifier.
    ///
    /// Returns
    /// -------
    /// FxForwardBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn base_calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        fx_forward_set!(
            slf,
            base_calendar_id,
            format!("{value:?}"),
            |b: FxForwardBuilderInner| b.base_calendar_id(value.to_string())
        )
    }

    /// Set the quote currency calendar identifier for business day adjustment.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Quote currency holiday calendar identifier.
    ///
    /// Returns
    /// -------
    /// FxForwardBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn quote_calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        fx_forward_set!(
            slf,
            quote_calendar_id,
            format!("{value:?}"),
            |b: FxForwardBuilderInner| b.quote_calendar_id(value.to_string())
        )
    }

    /// Set free-form instrument attributes (tags and metadata).
    ///
    /// Parameters
    /// ----------
    /// value : Attributes | dict[str, str] | None
    ///     Attribute bag; a dict populates metadata, with an optional
    ///     ``"tags"`` list entry populating tags.
    ///
    /// Returns
    /// -------
    /// FxForwardBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``value`` is neither ``Attributes``, a dict, nor ``None``.
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        let shown = value.repr()?.to_string();
        fx_forward_set!(slf, attributes, shown, |b: FxForwardBuilderInner| b
            .attributes(attrs))
    }

    /// Build the validated FX forward.
    ///
    /// Validation is the Rust ``FxForward::builder().build()`` invariants
    /// only; there is no additional binding-side check.
    ///
    /// Returns
    /// -------
    /// FxForward
    ///     The validated FX forward.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed, a required field is missing,
    ///     or the completed FX forward fails validation (for example,
    ///     ``base_currency`` equals ``quote_currency``).
    #[pyo3(text_signature = "($self)")]
    fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<PyFxForward> {
        let b = take_builder(&mut slf.inner)?;
        let inner = b.build().map_err(core_to_py)?;
        Ok(PyFxForward { inner })
    }

    /// Return ``repr(self)`` listing the fields set so far.
    fn __repr__(&self) -> String {
        builder_repr("FxForwardBuilder", &self.fields)
    }
}

/// Vanilla FX option priced with Garman–Kohlhagen (typed wrapper for Rust ``FxOption``).
///
/// ``strike`` is quoted as quote currency per unit of base currency; the
/// notional is in ``base_currency``. The option carries its pair/venue delta
/// convention so Greeks are reported the way the desk quotes them.
///
/// Build with ``FxOption.builder()`` or ``FxOption.european(...)``; start
/// from ``FxOption.example()`` for a ready-made EUR/USD call. Instances are
/// accepted directly by ``price_instrument`` and expose ``price`` /
/// ``metric`` / ``greeks`` themselves.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import FxOption
/// >>> opt = FxOption.example()
/// >>> (opt.option_type, opt.strike, opt.delta_convention["kind"])
/// ('call', 1.12, 'forward')
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "FxOption",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFxOption {
    /// Inner canonical Rust FX option.
    pub(crate) inner: finstack_quant_valuations::instruments::FxOption,
}

impl PyFxOption {
    /// Serialize as the canonical instrument envelope accepted by the JSON loader.
    pub(crate) fn envelope_json(&self) -> PyResult<String> {
        serialize_typed_instrument_json(InstrumentJson::FxOption(self.inner.clone()), "FxOption")
    }
}

instrument_envelope_methods!(
    PyFxOption,
    FxOption,
    "fx_option",
    PyFxOptionBuilder,
    finstack_quant_valuations::instruments::FxOption::builder()
);
instrument_pricing_methods!(PyFxOption);

/// Build an `FxDeltaConvention` from the loose Python inputs.
fn delta_convention_from_parts(
    kind: &str,
    premium_currency: &Bound<'_, PyAny>,
    venue: &str,
) -> PyResult<FxDeltaConvention> {
    let kind: FxDeltaConventionKind = enum_from_str(kind, "delta convention kind")?;
    FxDeltaConvention::new(
        kind,
        currency_from_py(premium_currency, "premium_currency")?,
        venue,
    )
    .map_err(core_to_py)
}

#[pymethods]
impl PyFxOption {
    /// Canonical example: EUR/USD call, strike 1.12, EUR 1,000,000.
    ///
    /// Mirrors Rust ``FxOption::example()`` (forward-delta convention,
    /// premium in USD, curves ``USD-OIS`` / ``EUR-OIS``, surface ``EURUSD-VOL``).
    ///
    /// Returns
    /// -------
    /// FxOption
    ///     The validated example option.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the canonical example fails validation (never for a released build).
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::FxOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Build a European FX option with currency-derived OIS curves.
    ///
    /// Mirrors Rust ``FxOption::european``: discount curves default to
    /// ``"<QUOTE>-OIS"`` (domestic) and ``"<BASE>-OIS"`` (foreign), with the
    /// pre-configured EUR/USD and GBP/USD underlying presets when applicable.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique instrument identifier.
    /// base_currency : Currency | str
    ///     Base (foreign) currency; notional currency.
    /// quote_currency : Currency | str
    ///     Quote (domestic) currency.
    /// strike : float
    ///     Strike, quote currency per unit of base currency.
    /// expiry : datetime.date | str
    ///     Expiry date.
    /// notional : Money | float
    ///     Notional in ``base_currency``; a bare float is tagged with that currency.
    /// vol_surface_id : str
    ///     FX volatility surface identifier.
    /// option_type : {"call", "put"}
    ///     Call or put on the base currency.
    /// delta_convention_kind : {"spot", "forward", "premium_adjusted_spot", "premium_adjusted_forward"}
    ///     Delta convention quoted by the venue.
    /// premium_currency : Currency | str
    ///     Currency in which the premium is paid (base or quote).
    /// venue : str
    ///     Non-empty market venue / quoting-source identifier.
    ///
    /// Returns
    /// -------
    /// FxOption
    ///     The validated option.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the currencies coincide, ``premium_currency`` is neither leg,
    ///     ``venue`` is blank, or the notional is not positive.
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
        let base = currency_from_py(base_currency, "base_currency")?;
        let quote = currency_from_py(quote_currency, "quote_currency")?;
        let inner = finstack_quant_valuations::instruments::FxOption::european(
            InstrumentId::new(id.to_string()),
            base,
            quote,
            strike,
            extract_date(expiry)?,
            super::convert::money_from_py(notional, Some(base.as_ref()), "notional")?,
            CurveId::new(vol_surface_id.to_string()),
            enum_from_str(option_type, "option_type")?,
            delta_convention_from_parts(delta_convention_kind, premium_currency, venue)?,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Implied volatility that reproduces ``target_price``.
    ///
    /// Mirrors Rust ``FxOption::implied_vol`` (Garman–Kohlhagen inversion).
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market carrying both discount curves and the FX spot.
    /// as_of : datetime.date | str
    ///     Valuation date.
    /// target_price : float
    ///     Observed option PV in quote currency (same scaling as ``price``).
    ///
    /// Returns
    /// -------
    /// float
    ///     Annualized lognormal volatility as a decimal (``0.10`` = 10%).
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If a curve or the spot is missing from ``market``.
    /// RuntimeError
    ///     If the root search does not converge (price outside no-arbitrage bounds).
    #[pyo3(text_signature = "($self, market, as_of, target_price)")]
    fn implied_vol(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        target_price: f64,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        let as_of = extract_date(as_of)?;
        self.inner
            .implied_vol(&market, as_of, target_price)
            .map_err(core_to_py)
    }

    /// Spot delta of the option.
    ///
    /// Returns
    /// -------
    /// float
    ///     Spot delta produced by the selected model.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an input is invalid, required market data is missing, pricing
    ///     fails, or the model does not produce delta.
    #[pyo3(signature = (market, as_of, model="default"))]
    fn delta(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        model: &str,
    ) -> PyResult<f64> {
        envelope_metric_value(py, self.envelope_json()?, market, as_of, model, "delta")
    }

    /// Spot gamma of the option.
    ///
    /// Returns
    /// -------
    /// float
    ///     Spot gamma produced by the selected model.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an input is invalid, required market data is missing, pricing
    ///     fails, or the model does not produce gamma.
    #[pyo3(signature = (market, as_of, model="default"))]
    fn gamma(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        model: &str,
    ) -> PyResult<f64> {
        envelope_metric_value(py, self.envelope_json()?, market, as_of, model, "gamma")
    }

    /// Vega of the option.
    ///
    /// Returns
    /// -------
    /// float
    ///     Vega produced by the selected model.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an input is invalid, required market data is missing, pricing
    ///     fails, or the model does not produce vega.
    #[pyo3(signature = (market, as_of, model="default"))]
    fn vega(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        model: &str,
    ) -> PyResult<f64> {
        envelope_metric_value(py, self.envelope_json()?, market, as_of, model, "vega")
    }

    /// Theta of the option.
    ///
    /// Returns
    /// -------
    /// float
    ///     Theta produced by the selected model.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an input is invalid, required market data is missing, pricing
    ///     fails, or the model does not produce theta.
    #[pyo3(signature = (market, as_of, model="default"))]
    fn theta(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        model: &str,
    ) -> PyResult<f64> {
        envelope_metric_value(py, self.envelope_json()?, market, as_of, model, "theta")
    }

    /// Domestic-rate rho of the option.
    ///
    /// Returns
    /// -------
    /// float
    ///     Domestic-rate rho produced by the selected model.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an input is invalid, required market data is missing, pricing
    ///     fails, or the model does not produce rho.
    #[pyo3(signature = (market, as_of, model="default"))]
    fn rho(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        model: &str,
    ) -> PyResult<f64> {
        envelope_metric_value(py, self.envelope_json()?, market, as_of, model, "rho")
    }

    /// Foreign-rate rho of the option.
    ///
    /// Returns
    /// -------
    /// float
    ///     Foreign-rate rho produced by the selected model.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an input is invalid, required market data is missing, pricing
    ///     fails, or the model does not produce foreign rho.
    #[pyo3(signature = (market, as_of, model="default"))]
    fn foreign_rho(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        model: &str,
    ) -> PyResult<f64> {
        envelope_metric_value(
            py,
            self.envelope_json()?,
            market,
            as_of,
            model,
            "foreign_rho",
        )
    }

    /// Vanna of the option.
    ///
    /// Returns
    /// -------
    /// float
    ///     Vanna produced by the selected model.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an input is invalid, required market data is missing, pricing
    ///     fails, or the model does not produce vanna.
    #[pyo3(signature = (market, as_of, model="default"))]
    fn vanna(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        model: &str,
    ) -> PyResult<f64> {
        envelope_metric_value(py, self.envelope_json()?, market, as_of, model, "vanna")
    }

    /// Volga of the option.
    ///
    /// Returns
    /// -------
    /// float
    ///     Volga produced by the selected model.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an input is invalid, required market data is missing, pricing
    ///     fails, or the model does not produce volga.
    #[pyo3(signature = (market, as_of, model="default"))]
    fn volga(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        model: &str,
    ) -> PyResult<f64> {
        envelope_metric_value(py, self.envelope_json()?, market, as_of, model, "volga")
    }

    /// Compute the standard FX option Greek set as a dict.
    ///
    /// Mirrors the WASM ``greeks`` method: Greeks the selected model cannot
    /// produce are omitted, and any non-finite Greek raises rather than being
    /// returned.
    ///
    /// Returns
    /// -------
    /// dict[str, float]
    ///     Mapping of Greek name to value for every Greek the model produced.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an input is invalid, required market data is missing, pricing
    ///     fails, or a returned Greek is non-finite.
    #[pyo3(signature = (market, as_of, model="default"))]
    fn greeks<'py>(
        &self,
        py: Python<'py>,
        market: &Bound<'py, PyAny>,
        as_of: &Bound<'py, PyAny>,
        model: &str,
    ) -> PyResult<Bound<'py, PyDict>> {
        envelope_option_greeks(py, self.envelope_json()?, market, as_of, model)
    }

    /// Base (foreign) currency; the notional currency.
    #[getter]
    fn base_currency(&self) -> PyCurrency {
        PyCurrency::from_inner(self.inner.base_currency)
    }

    /// Quote (domestic) currency.
    #[getter]
    fn quote_currency(&self) -> PyCurrency {
        PyCurrency::from_inner(self.inner.quote_currency)
    }

    /// Strike, quote currency per unit of base currency.
    #[getter]
    fn strike(&self) -> f64 {
        self.inner.strike
    }

    /// Option type: ``"call"`` or ``"put"`` on the base currency.
    #[getter]
    fn option_type(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.option_type)
    }

    /// Delta convention as ``{"kind", "premium_currency", "venue"}``.
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

    /// Expiry date.
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.expiry)
    }

    /// Day count used for the time-to-expiry year fraction (serde name).
    #[getter]
    fn day_count(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.day_count)
    }

    /// Notional amount in the base currency.
    #[getter]
    fn notional(&self) -> PyMoney {
        money_to_py(self.inner.notional)
    }

    /// Domestic (quote-currency) discount curve identifier.
    #[getter]
    fn domestic_discount_curve_id(&self) -> String {
        self.inner.domestic_discount_curve_id.to_string()
    }

    /// Foreign (base-currency) discount curve identifier.
    #[getter]
    fn foreign_discount_curve_id(&self) -> String {
        self.inner.foreign_discount_curve_id.to_string()
    }

    /// FX volatility surface identifier.
    #[getter]
    fn vol_surface_id(&self) -> String {
        self.inner.vol_surface_id.to_string()
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

/// Fluent builder for ``FxOption``; wraps the Rust
/// ``FinancialBuilder``-generated builder (consuming setters).
///
/// Builders are consumed by ``build()``; create a new builder per instrument.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "FxOptionBuilder",
    skip_from_py_object
)]
pub struct PyFxOptionBuilder {
    inner: Option<FxOptionBuilderInner>,
    fields: Vec<(&'static str, String)>,
}

/// Apply one consuming Rust setter and record the field for ``__repr__``.
macro_rules! fx_option_set {
    ($slf:ident, $field:ident, $repr:expr, $apply:expr) => {{
        let b = take_builder(&mut $slf.inner)?;
        $slf.inner = Some($apply(b));
        $slf.fields.push((stringify!($field), $repr));
        Ok($slf)
    }};
}

#[pymethods]
impl PyFxOptionBuilder {
    /// Set the instrument identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Unique identifier for the FX option.
    ///
    /// Returns
    /// -------
    /// FxOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        fx_option_set!(slf, id, format!("{value:?}"), |b: FxOptionBuilderInner| b
            .id(InstrumentId::new(value.to_string())))
    }

    /// Set the base currency (foreign currency).
    ///
    /// Parameters
    /// ----------
    /// value : Currency | str
    ///     Base (foreign) currency, as a ``Currency`` or ISO-4217 code.
    ///
    /// Returns
    /// -------
    /// FxOptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a string code is not ISO-4217.
    #[pyo3(text_signature = "($self, value)")]
    fn base_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let ccy = currency_from_py(value, "base_currency")?;
        fx_option_set!(
            slf,
            base_currency,
            format!("Currency('{ccy}')"),
            |b: FxOptionBuilderInner| b.base_currency(ccy)
        )
    }

    /// Set the quote currency (domestic currency).
    ///
    /// Parameters
    /// ----------
    /// value : Currency | str
    ///     Quote (domestic) currency, as a ``Currency`` or ISO-4217 code.
    ///
    /// Returns
    /// -------
    /// FxOptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a string code is not ISO-4217.
    #[pyo3(text_signature = "($self, value)")]
    fn quote_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let ccy = currency_from_py(value, "quote_currency")?;
        fx_option_set!(
            slf,
            quote_currency,
            format!("Currency('{ccy}')"),
            |b: FxOptionBuilderInner| b.quote_currency(ccy)
        )
    }

    /// Set the strike exchange rate (quote per base).
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Strike exchange rate, quote currency per unit of base currency.
    ///
    /// Returns
    /// -------
    /// FxOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn strike<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        fx_option_set!(slf, strike, float_repr(value), |b: FxOptionBuilderInner| b
            .strike(value))
    }

    /// Set the option type: ``"call"`` or ``"put"`` on base currency.
    ///
    /// Parameters
    /// ----------
    /// value : {"call", "put"}
    ///     Option type of the FX option.
    ///
    /// Returns
    /// -------
    /// FxOptionBuilder
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
        fx_option_set!(
            slf,
            option_type,
            format!("{value:?}"),
            |b: FxOptionBuilderInner| b.option_type(option_type)
        )
    }

    /// Set the pair/venue delta convention and premium currency.
    ///
    /// Parameters
    /// ----------
    /// kind : {"spot", "forward", "premium_adjusted_spot", "premium_adjusted_forward"}
    ///     Delta convention quoted by the venue.
    /// premium_currency : Currency | str
    ///     Currency in which the FX option premium is paid.
    /// venue : str
    ///     Non-empty market venue or quoting-source identifier.
    ///
    /// Returns
    /// -------
    /// FxOptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``kind`` is unknown or ``venue`` is blank.
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
        fx_option_set!(slf, delta_convention, shown, |b: FxOptionBuilderInner| b
            .delta_convention(convention))
    }

    /// Set the option expiry date.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | str
    ///     Option expiry date (date-like or ISO 8601 string).
    ///
    /// Returns
    /// -------
    /// FxOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let expiry = extract_date(value)?;
        fx_option_set!(slf, expiry, date_repr(expiry), |b: FxOptionBuilderInner| b
            .expiry(expiry))
    }

    /// Set the day count for the time-to-expiry year fraction.
    ///
    /// Parameters
    /// ----------
    /// value : DayCount | str
    ///     Day count convention; defaults to ``ACT/365F`` when never set.
    ///
    /// Returns
    /// -------
    /// FxOptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a string name is not a recognized day count.
    #[pyo3(text_signature = "($self, value)")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let day_count = day_count_from_py(value, "day_count")?;
        fx_option_set!(
            slf,
            day_count,
            format!("DayCount('{day_count}')"),
            |b: FxOptionBuilderInner| b.day_count(day_count)
        )
    }

    /// Set the notional amount in base currency.
    ///
    /// Parameters
    /// ----------
    /// value : Money
    ///     Notional amount, denominated in the base currency.
    ///
    /// Returns
    /// -------
    /// FxOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = value.inner;
        fx_option_set!(
            slf,
            notional,
            money_repr(money),
            |b: FxOptionBuilderInner| b.notional(money)
        )
    }

    /// Set the domestic currency discount curve identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Domestic currency discount curve identifier.
    ///
    /// Returns
    /// -------
    /// FxOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn domestic_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        fx_option_set!(
            slf,
            domestic_discount_curve_id,
            format!("{value:?}"),
            |b: FxOptionBuilderInner| b.domestic_discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    /// Set the foreign currency discount curve identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Foreign currency discount curve identifier.
    ///
    /// Returns
    /// -------
    /// FxOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn foreign_discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        fx_option_set!(
            slf,
            foreign_discount_curve_id,
            format!("{value:?}"),
            |b: FxOptionBuilderInner| b.foreign_discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    /// Set the FX volatility surface identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     FX volatility surface identifier for option pricing.
    ///
    /// Returns
    /// -------
    /// FxOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        fx_option_set!(
            slf,
            vol_surface_id,
            format!("{value:?}"),
            |b: FxOptionBuilderInner| b.vol_surface_id(CurveId::new(value.to_string()))
        )
    }

    /// Set free-form instrument attributes (tags and metadata).
    ///
    /// Parameters
    /// ----------
    /// value : Attributes | dict[str, str] | None
    ///     Attribute bag; a dict populates metadata, with an optional
    ///     ``"tags"`` list entry populating tags.
    ///
    /// Returns
    /// -------
    /// FxOptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``value`` is neither ``Attributes``, a dict, nor ``None``.
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        let shown = value.repr()?.to_string();
        fx_option_set!(slf, attributes, shown, |b: FxOptionBuilderInner| b
            .attributes(attrs))
    }

    /// Build the validated FX option.
    ///
    /// Validation is the Rust ``FxOption::builder().build()`` invariants
    /// only; there is no additional binding-side check.
    ///
    /// Returns
    /// -------
    /// FxOption
    ///     The validated FX option.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed, a required field is missing,
    ///     or the completed FX option fails validation (for example,
    ///     ``base_currency`` equals ``quote_currency``).
    #[pyo3(text_signature = "($self)")]
    fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<PyFxOption> {
        let b = take_builder(&mut slf.inner)?;
        let inner = b.build().map_err(core_to_py)?;
        Ok(PyFxOption { inner })
    }

    /// Return ``repr(self)`` listing the fields set so far.
    fn __repr__(&self) -> String {
        builder_repr("FxOptionBuilder", &self.fields)
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
pub(crate) const EXPORTS: &[&str] = &[];
