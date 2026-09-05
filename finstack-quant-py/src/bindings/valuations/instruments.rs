//! Typed instrument classes for the `finstack_quant.valuations.instruments`
//! namespace.
//!
//! Thin wrappers over the canonical Rust structs
//! `finstack_quant_valuations::instruments::Bond` and
//! `finstack_quant_valuations::instruments::TermLoan`. Construction and
//! validation stay in Rust; the wrappers only convert to and from the canonical
//! `finstack_quant.instrument/1` envelope accepted by the JSON loader, expose
//! one getter per public Rust field, and route `price` / `metric` through the
//! same pricer entry points as `price_instrument`.

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::schedule::PyStubKind;
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::core::money::PyMoney;
use crate::bindings::core::types::PyAttributes;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::extract::extract_market;
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::valuations::merton_mc::{PyMertonMcConfig, PyMertonMcResult};
use crate::errors::{core_to_py, serde_json_to_py, value_error};
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::{Instrument, InstrumentEnvelope, InstrumentJson};

use super::convert::{
    attributes_from_py, attributes_to_py, bps_from_py, enum_to_py_string, money_from_py,
    money_to_py, opt_repr, rate_decimal_from_py,
};
use super::pricing::binding_pricing_options;
use super::PyValuationResult;

/// Parse a canonical typed-instrument envelope through the shared Rust path.
pub(crate) fn parse_typed_instrument_json(json: &str) -> PyResult<InstrumentJson> {
    finstack_quant_valuations::pricer::json::parse_instrument_from_json(json).map_err(core_to_py)
}

/// Serialize a typed instrument as the canonical v1 persistence envelope.
pub(crate) fn serialize_typed_instrument_json(
    instrument: InstrumentJson,
    what: &str,
) -> PyResult<String> {
    serde_json::to_string(&InstrumentEnvelope::new(instrument)).map_err(|err| {
        serde_json_to_py(
            err,
            &format!("failed to serialize {what} instrument envelope"),
        )
    })
}

// Shared helpers for typed instrument builders (bond/term_loan today; every
// later typed-instrument task reuses these).

/// Parse a serde-tagged unit-enum value from its snake_case string form.
///
/// Used by typed builders so Python passes plain strings (typed as
/// ``Literal[...]`` in the stubs) for Rust enums like ``PayReceive``.
pub(crate) fn enum_from_str<T: serde::de::DeserializeOwned>(
    value: &str,
    what: &str,
) -> PyResult<T> {
    serde_json::from_value(serde_json::Value::String(value.to_string()))
        .map_err(|err| value_error(format!("invalid {what}: {err}")))
}

/// Convert a Python float to `Decimal`, rejecting non-finite values.
pub(crate) fn decimal_from_f64(value: f64, what: &str) -> PyResult<rust_decimal::Decimal> {
    rust_decimal::Decimal::try_from(value)
        .map_err(|err| value_error(format!("invalid {what}: {err}")))
}

/// Parse a JSON sub-field string into a typed Rust spec value.
///
/// Used by ``*_json`` builder setters for deep nested config (margin specs,
/// waterfall rules, conversion terms) per the nested-spec rule in the plan.
pub(crate) fn json_field<T: serde::de::DeserializeOwned>(json: &str, what: &str) -> PyResult<T> {
    serde_json::from_str(json).map_err(|err| serde_json_to_py(err, &format!("invalid {what} JSON")))
}

/// Coerce `dict | str` to a serde-backed Rust spec value.
///
/// A `str` is parsed as JSON; any other object is round-tripped through
/// `json.dumps`. Used for nested specs that have no typed Python twin yet
/// (cashflow specs, call schedules, margin specs, amortization, …).
pub(crate) fn spec_from_py<T: serde::de::DeserializeOwned + Send>(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
    what: &str,
) -> PyResult<T> {
    if let Ok(json) = obj.extract::<&str>() {
        return json_field(json, what);
    }
    crate::bindings::module_utils::py_to_serde(py, obj, what)
}

/// Coerce `StubKind | str | None` to a Rust stub rule (`None` → short front).
pub(crate) fn stub_kind_from_py(
    obj: Option<&Bound<'_, PyAny>>,
    what: &str,
) -> PyResult<finstack_quant_core::dates::StubKind> {
    let Some(obj) = obj else {
        return Ok(finstack_quant_core::dates::StubKind::ShortFront);
    };
    if obj.is_none() {
        return Ok(finstack_quant_core::dates::StubKind::ShortFront);
    }
    if let Ok(stub) = obj.cast::<PyStubKind>() {
        return Ok(stub.borrow().inner);
    }
    if let Ok(name) = obj.extract::<&str>() {
        return enum_from_str(name, what);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(format!(
        "{what}: expected finstack_quant.core.dates.StubKind or a stub name such as \"none\" or \"short_front\", got {}",
        obj.get_type().name()?
    )))
}

/// Coerce `float | int | Rate` to a core `Rate` (decimal units).
pub(crate) fn rate_from_py(
    obj: &Bound<'_, PyAny>,
    what: &str,
) -> PyResult<finstack_quant_core::types::Rate> {
    rate_decimal_from_py(obj, what).and_then(|value| {
        finstack_quant_core::types::Rate::from_decimal(value).map_err(crate::errors::core_to_py)
    })
}

/// Coerce `float | int | Bps` to a core `Bps` (whole basis points, rounded).
pub(crate) fn bps_value_from_py(
    obj: &Bound<'_, PyAny>,
    what: &str,
) -> PyResult<finstack_quant_core::types::Bps> {
    let bp = bps_from_py(obj, what)?;
    if !bp.is_finite() {
        return Err(value_error(format!("{what}: basis points must be finite")));
    }
    let rounded = bp.round();
    if rounded > f64::from(i32::MAX) || rounded < f64::from(i32::MIN) {
        return Err(value_error(format!("{what}: basis points out of range")));
    }
    // Truncation is impossible after the range check above.
    Ok(finstack_quant_core::types::Bps::new(rounded as i32))
}

/// Coerce an optional pricing-options object (`dict | str | None`) to JSON.
pub(crate) fn pricing_options_json(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<String>> {
    match obj {
        Some(obj) if !obj.is_none() => {
            crate::bindings::module_utils::py_to_json_string(py, obj, "pricing_options").map(Some)
        }
        _ => Ok(None),
    }
}

/// Price a typed instrument envelope through the canonical Rust pricer.
///
/// Mirrors `price_instrument`; `pricing_options` is already JSON.
// PyO3 binding helper: mirrors the keyword surface of `price_instrument`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn price_typed_envelope(
    py: Python<'_>,
    envelope_json: String,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    model: &str,
    metrics: Option<Vec<String>>,
    pricing_options: Option<String>,
    market_history: Option<&str>,
) -> PyResult<PyValuationResult> {
    let market = extract_market(py, market)?;
    let as_of = crate::bindings::date_utils::extract_date_iso(as_of)?;
    let model = model.to_owned();
    let metrics = metrics.unwrap_or_default();
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
pub(crate) fn metric_typed_envelope(
    py: Python<'_>,
    envelope_json: String,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    metric_id: &str,
    model: &str,
) -> PyResult<f64> {
    let market = extract_market(py, market)?;
    let as_of = crate::bindings::date_utils::extract_date_iso(as_of)?;
    let model = model.to_owned();
    let metric_id = metric_id.to_owned();
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
            &metric_id,
            binding_pricing_options(),
        )
    })
    .map_err(core_to_py)
}

/// `Instrument::market_dependencies` as a Python dict (serde shape).
pub(crate) fn instrument_market_dependencies<'py>(
    py: Python<'py>,
    instrument: &dyn Instrument,
) -> PyResult<Bound<'py, PyAny>> {
    let deps = instrument.market_dependencies().map_err(core_to_py)?;
    serde_to_py(py, &deps)
}

/// `Instrument::default_model` as its canonical model key string.
pub(crate) fn instrument_default_model(instrument: &dyn Instrument) -> String {
    instrument.default_model().to_string()
}

/// `Instrument::expiry` as `datetime.date | None`.
pub(crate) fn instrument_expiry<'py>(
    py: Python<'py>,
    instrument: &dyn Instrument,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    instrument
        .expiry()
        .map(|date| date_to_py(py, date))
        .transpose()
}

/// Serialize an optional serde value as `dict | None`.
pub(crate) fn opt_serde_to_py<'py, T: serde::Serialize>(
    py: Python<'py>,
    value: Option<&T>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    value.map(|v| serde_to_py(py, v)).transpose()
}

/// Render a builder's set-so-far fields Python-style.
pub(crate) fn builder_repr(name: &str, fields: &[(&'static str, String)]) -> String {
    let body = fields
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}({body})")
}

/// Python-style repr of a `Money` value.
pub(crate) fn money_repr(value: finstack_quant_core::money::Money) -> String {
    format!(
        "Money({}, {:?})",
        value.amount(),
        value.currency().to_string()
    )
}

type BondBuilderInner = finstack_quant_valuations::instruments::fixed_income::bond::BondBuilder;
type TermLoanBuilderInner =
    finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoanBuilder;

/// Parse a `BondConvention` preset from its serde string.
fn bond_convention_from_str(
    value: &str,
) -> PyResult<finstack_quant_valuations::instruments::BondConvention> {
    enum_from_str(value, "convention")
}

/// Typed wrapper for the Rust `Bond` instrument.
///
/// Construct via ``Bond.fixed`` (US-corporate or a named convention preset),
/// ``Bond.with_convention``, ``Bond.floating`` /
/// ``Bond.floating_with_convention``, ``Bond.zero_coupon``, the
/// ``Bond.builder()`` fluent builder (callable, credit-curve, custom
/// day-count / frequency / settlement), the ``Bond.example*`` presets, or
/// ``Bond.from_json``. Every public Rust field is readable as a property;
/// ``price`` / ``metric`` run the same pricer as ``price_instrument``.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "Bond",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyBond {
    /// Inner canonical Rust bond.
    pub(crate) inner: finstack_quant_valuations::instruments::Bond,
}

impl PyBond {
    /// Serialize as the canonical instrument envelope accepted by the JSON loader.
    pub(crate) fn envelope_json(&self) -> PyResult<String> {
        serialize_typed_instrument_json(InstrumentJson::Bond(self.inner.clone()), "Bond")
    }
}

#[pymethods]
impl PyBond {
    /// Create a fluent builder (mirrors Rust ``Bond::builder()``).
    ///
    /// Returns
    /// -------
    /// BondBuilder
    ///     A builder with fluent, consuming setter methods.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import Bond
    /// >>> builder = Bond.builder()
    /// >>> builder.id("EXAMPLE") is builder
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn builder() -> PyBondBuilder {
        PyBondBuilder {
            inner: Some(finstack_quant_valuations::instruments::Bond::builder()),
            fields: Vec::new(),
        }
    }

    /// Create a fixed-rate bond from a settlement/day-count convention preset.
    ///
    /// Mirrors Rust ``Bond::fixed`` (``convention=None``, US corporate:
    /// semi-annual, 30/360, T+1) and ``Bond::with_convention`` followed by
    /// ``with_stub`` when ``convention`` names a preset.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique instrument identifier.
    /// notional : Money | float
    ///     Principal amount; a bare number needs ``currency``.
    /// coupon_rate : float | Rate
    ///     Annual coupon rate as a decimal (``0.05`` = 5%) or a ``Rate``.
    /// issue : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Issue date (ISO 8601 strings accepted).
    /// maturity : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Maturity date.
    /// stub : StubKind | str
    ///     Placement and length policy for an irregular coupon period
    ///     (``"none"``, ``"short_front"``, ``"long_front"``, ``"short_back"``,
    ///     ``"long_back"`` or a ``StubKind``).
    /// discount_curve_id : str
    ///     Discount curve identifier used for pricing.
    /// convention : str, optional
    ///     Bond convention preset: ``"us_treasury"``, ``"us_agency"``,
    ///     ``"german_bund"``, ``"uk_gilt"``, ``"french_oat"``, ``"jgb"``,
    ///     ``"us_corporate"`` (default) or ``"eur_corporate"``. Sets coupon
    ///     frequency, day count, calendar, business-day convention and
    ///     settlement lag.
    /// currency : str, optional
    ///     ISO-4217 code applied when ``notional`` is a bare number.
    ///
    /// Returns
    /// -------
    /// Bond
    ///     A validated fixed-rate bond.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``convention``/``stub`` is not a recognized name, a bare
    ///     ``notional`` has no ``currency``, or validation fails (e.g.
    ///     maturity not after issue).
    /// TypeError
    ///     If ``coupon_rate`` or ``notional`` has an unsupported type, or a
    ///     date cannot be interpreted.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import Bond
    /// >>> bond = Bond.fixed(
    /// ...     "BOND-1", 1_000_000.0, 0.05, "2024-01-01", "2034-01-01", "none", "USD-OIS",
    /// ...     currency="USD",
    /// ... )
    /// >>> bond.id
    /// 'BOND-1'
    #[staticmethod]
    #[pyo3(signature = (id, notional, coupon_rate, issue, maturity, stub, discount_curve_id, *, convention = None, currency = None))]
    #[pyo3(
        text_signature = "(id, notional, coupon_rate, issue, maturity, stub, discount_curve_id, *, convention=None, currency=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn fixed(
        id: &str,
        notional: &Bound<'_, PyAny>,
        coupon_rate: &Bound<'_, PyAny>,
        issue: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        stub: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        convention: Option<&str>,
        currency: Option<&str>,
    ) -> PyResult<Self> {
        let notional = money_from_py(notional, currency, "notional")?;
        let coupon_rate = rate_from_py(coupon_rate, "coupon_rate")?;
        let stub = stub_kind_from_py(Some(stub), "stub")?;
        let issue = extract_date(issue)?;
        let maturity = extract_date(maturity)?;
        let inner = match convention {
            None => finstack_quant_valuations::instruments::Bond::fixed(
                id,
                notional,
                coupon_rate,
                issue,
                maturity,
                stub,
                discount_curve_id,
            )
            .map_err(core_to_py)?,
            Some(name) => finstack_quant_valuations::instruments::Bond::with_convention(
                id,
                notional,
                coupon_rate,
                issue,
                maturity,
                bond_convention_from_str(name)?,
                discount_curve_id,
            )
            .map_err(core_to_py)?
            .with_stub(stub),
        };
        Ok(Self { inner })
    }

    /// Create a fixed-rate bond from a named market convention.
    ///
    /// Mirrors Rust ``Bond::with_convention``; the stub rule is the preset's
    /// own (use ``Bond.fixed(..., convention=...)`` to override it).
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique instrument identifier.
    /// notional : Money | float
    ///     Principal amount; a bare number needs ``currency``.
    /// coupon_rate : float | Rate
    ///     Annual coupon rate as a decimal (``0.05`` = 5%) or a ``Rate``.
    /// issue : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Issue date.
    /// maturity : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Maturity date.
    /// convention : str
    ///     Bond convention preset (``"us_treasury"``, ``"us_agency"``,
    ///     ``"german_bund"``, ``"uk_gilt"``, ``"french_oat"``, ``"jgb"``,
    ///     ``"us_corporate"``, ``"eur_corporate"``).
    /// discount_curve_id : str
    ///     Discount curve identifier used for pricing.
    /// currency : str, optional
    ///     ISO-4217 code applied when ``notional`` is a bare number.
    ///
    /// Returns
    /// -------
    /// Bond
    ///     A validated fixed-rate bond.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``convention`` is unknown, a bare ``notional`` has no
    ///     ``currency``, or validation fails.
    /// TypeError
    ///     If ``coupon_rate``/``notional`` has an unsupported type or a date
    ///     cannot be interpreted.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import Bond
    /// >>> gilt = Bond.with_convention(
    /// ...     "GILT", 1_000_000.0, 0.04, "2024-01-01", "2034-01-01", "uk_gilt", "GBP-OIS",
    /// ...     currency="GBP",
    /// ... )
    /// >>> gilt.settlement_days
    /// 1
    #[staticmethod]
    #[pyo3(signature = (id, notional, coupon_rate, issue, maturity, convention, discount_curve_id, *, currency = None))]
    #[pyo3(
        text_signature = "(id, notional, coupon_rate, issue, maturity, convention, discount_curve_id, *, currency=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn with_convention(
        id: &str,
        notional: &Bound<'_, PyAny>,
        coupon_rate: &Bound<'_, PyAny>,
        issue: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        convention: &str,
        discount_curve_id: &str,
        currency: Option<&str>,
    ) -> PyResult<Self> {
        let inner = finstack_quant_valuations::instruments::Bond::with_convention(
            id,
            money_from_py(notional, currency, "notional")?,
            rate_from_py(coupon_rate, "coupon_rate")?,
            extract_date(issue)?,
            extract_date(maturity)?,
            bond_convention_from_str(convention)?,
            discount_curve_id,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Create a floating-rate bond (FRN) linked to a forward index.
    ///
    /// Mirrors Rust ``Bond::floating``. Settlement, calendar, and
    /// business-day convention come from the notional currency:
    /// USD ``us_corporate`` (T+1, ``usny``), EUR ``eur_corporate`` (T+2,
    /// ``target2``), GBP ``uk_gilt`` (T+1), JPY ``jgb`` (T+2). Other
    /// currencies raise ``ValueError``; use ``Bond.floating_with_convention``
    /// to name the preset explicitly, or the builder.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique instrument identifier.
    /// notional : Money | float
    ///     Principal amount; a bare number needs ``currency``.
    /// index_id : str
    ///     Forward curve identifier (e.g. ``"USD-SOFR-3M"``).
    /// margin_bp : float | Bps
    ///     Spread over the index in whole basis points (fractions are
    ///     rounded).
    /// issue : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Issue date.
    /// maturity : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Maturity date.
    /// frequency : Tenor
    ///     Payment frequency (e.g. ``Tenor.quarterly()``).
    /// day_count : DayCount
    ///     Day count convention (e.g. ``DayCount.ACT_360``).
    /// discount_curve_id : str
    ///     Discount curve identifier used for pricing.
    /// currency : str, optional
    ///     ISO-4217 code applied when ``notional`` is a bare number.
    ///
    /// Returns
    /// -------
    /// Bond
    ///     A validated floating-rate note.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the notional currency has no mapped settlement convention,
    ///     ``notional`` is not finite and positive, or ``issue`` is not
    ///     strictly before ``maturity``.
    /// TypeError
    ///     If ``margin_bp``/``notional`` has an unsupported type or a date
    ///     cannot be interpreted.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.core.dates import DayCount, Tenor
    /// >>> from finstack_quant.valuations.instruments import Bond
    /// >>> frn = Bond.floating(
    /// ...     "FRN", 1000.0, "USD-SOFR-3M", 125.0, "2024-01-01", "2029-01-01",
    /// ...     Tenor.quarterly(), DayCount.ACT_360, "USD-OIS", currency="USD",
    /// ... )
    /// >>> frn.has_floating_coupons
    /// True
    #[staticmethod]
    #[pyo3(signature = (id, notional, index_id, margin_bp, issue, maturity, frequency, day_count, discount_curve_id, *, currency = None))]
    #[pyo3(
        text_signature = "(id, notional, index_id, margin_bp, issue, maturity, frequency, day_count, discount_curve_id, *, currency=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn floating(
        id: &str,
        notional: &Bound<'_, PyAny>,
        index_id: &str,
        margin_bp: &Bound<'_, PyAny>,
        issue: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        frequency: PyRef<'_, PyTenor>,
        day_count: PyRef<'_, PyDayCount>,
        discount_curve_id: &str,
        currency: Option<&str>,
    ) -> PyResult<Self> {
        let inner = finstack_quant_valuations::instruments::Bond::floating(
            id,
            money_from_py(notional, currency, "notional")?,
            index_id,
            bps_value_from_py(margin_bp, "margin_bp")?,
            extract_date(issue)?,
            extract_date(maturity)?,
            frequency.inner,
            day_count.inner,
            discount_curve_id,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Create a floating-rate bond with an explicit convention preset.
    ///
    /// Mirrors Rust ``Bond::floating_with_convention``.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique instrument identifier.
    /// notional : Money | float
    ///     Principal amount; a bare number needs ``currency``.
    /// index_id : str
    ///     Forward curve identifier (e.g. ``"USD-SOFR-3M"``).
    /// margin_bp : float | Bps
    ///     Spread over the index in whole basis points (fractions are
    ///     rounded).
    /// issue : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Issue date.
    /// maturity : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Maturity date.
    /// frequency : Tenor
    ///     Payment frequency.
    /// day_count : DayCount
    ///     Day count convention.
    /// convention : str
    ///     Bond convention preset (see ``Bond.with_convention``).
    /// discount_curve_id : str
    ///     Discount curve identifier used for pricing.
    /// currency : str, optional
    ///     ISO-4217 code applied when ``notional`` is a bare number.
    ///
    /// Returns
    /// -------
    /// Bond
    ///     A validated floating-rate note.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``convention`` is unknown, a bare ``notional`` has no
    ///     ``currency``, or validation fails.
    /// TypeError
    ///     If ``margin_bp``/``notional`` has an unsupported type or a date
    ///     cannot be interpreted.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.core.dates import DayCount, Tenor
    /// >>> from finstack_quant.valuations.instruments import Bond
    /// >>> frn = Bond.floating_with_convention(
    /// ...     "FRN-EUR", 1000.0, "EUR-EURIBOR-3M", 80.0, "2024-01-01", "2029-01-01",
    /// ...     Tenor.quarterly(), DayCount.ACT_360, "eur_corporate", "EUR-OIS", currency="EUR",
    /// ... )
    /// >>> frn.settlement_days
    /// 2
    #[staticmethod]
    #[pyo3(signature = (id, notional, index_id, margin_bp, issue, maturity, frequency, day_count, convention, discount_curve_id, *, currency = None))]
    #[pyo3(
        text_signature = "(id, notional, index_id, margin_bp, issue, maturity, frequency, day_count, convention, discount_curve_id, *, currency=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn floating_with_convention(
        id: &str,
        notional: &Bound<'_, PyAny>,
        index_id: &str,
        margin_bp: &Bound<'_, PyAny>,
        issue: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        frequency: PyRef<'_, PyTenor>,
        day_count: PyRef<'_, PyDayCount>,
        convention: &str,
        discount_curve_id: &str,
        currency: Option<&str>,
    ) -> PyResult<Self> {
        let inner = finstack_quant_valuations::instruments::Bond::floating_with_convention(
            id,
            money_from_py(notional, currency, "notional")?,
            index_id,
            bps_value_from_py(margin_bp, "margin_bp")?,
            extract_date(issue)?,
            extract_date(maturity)?,
            frequency.inner,
            day_count.inner,
            bond_convention_from_str(convention)?,
            discount_curve_id,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Create a zero-coupon bond (single principal redemption at maturity).
    ///
    /// Mirrors Rust ``Bond::zero_coupon``.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique instrument identifier.
    /// notional : Money | float
    ///     Redemption amount; a bare number needs ``currency``.
    /// issue : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Issue date.
    /// maturity : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Maturity (redemption) date.
    /// discount_curve_id : str
    ///     Discount curve identifier used for pricing.
    /// currency : str, optional
    ///     ISO-4217 code applied when ``notional`` is a bare number.
    ///
    /// Returns
    /// -------
    /// Bond
    ///     A validated zero-coupon bond.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a bare ``notional`` has no ``currency`` or ``maturity`` is not
    ///     after ``issue``.
    /// TypeError
    ///     If ``notional`` has an unsupported type or a date cannot be
    ///     interpreted.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import Bond
    /// >>> zc = Bond.zero_coupon("ZC", 1_000_000.0, "2024-01-01", "2029-01-01", "USD-OIS", currency="USD")
    /// >>> zc.has_floating_coupons
    /// False
    #[staticmethod]
    #[pyo3(signature = (id, notional, issue, maturity, discount_curve_id, *, currency = None))]
    #[pyo3(text_signature = "(id, notional, issue, maturity, discount_curve_id, *, currency=None)")]
    fn zero_coupon(
        id: &str,
        notional: &Bound<'_, PyAny>,
        issue: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        currency: Option<&str>,
    ) -> PyResult<Self> {
        let inner = finstack_quant_valuations::instruments::Bond::zero_coupon(
            id,
            money_from_py(notional, currency, "notional")?,
            extract_date(issue)?,
            extract_date(maturity)?,
            discount_curve_id,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Canonical example fixed-rate bond (mirrors Rust ``Bond::example``).
    ///
    /// Returns
    /// -------
    /// Bond
    ///     A 5-year USD 5% semi-annual bond discounted on ``USD-OIS``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If construction fails (should not occur).
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import Bond
    /// >>> Bond.example().discount_curve_id
    /// 'USD-OIS'
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::Bond::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Example floating-rate note (mirrors Rust ``Bond::example_floating``).
    ///
    /// Returns
    /// -------
    /// Bond
    ///     A USD SOFR-linked FRN.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If construction fails (should not occur).
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import Bond
    /// >>> Bond.example_floating().has_floating_coupons
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example_floating() -> PyResult<Self> {
        finstack_quant_valuations::instruments::Bond::example_floating()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Example callable bond (mirrors Rust ``Bond::example_callable``).
    ///
    /// Returns
    /// -------
    /// Bond
    ///     A fixed-rate bond carrying a call schedule.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If construction fails (should not occur).
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import Bond
    /// >>> Bond.example_callable().call_put is not None
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example_callable() -> PyResult<Self> {
        finstack_quant_valuations::instruments::Bond::example_callable()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Example amortizing bond (mirrors Rust ``Bond::example_amortizing``).
    ///
    /// Returns
    /// -------
    /// Bond
    ///     A fixed-rate bond with a principal amortization schedule.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If construction fails (should not occur).
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import Bond
    /// >>> "amortizing" in Bond.example_amortizing().cashflow_spec
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example_amortizing() -> PyResult<Self> {
        finstack_quant_valuations::instruments::Bond::example_amortizing()
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

    /// Deserialize a validated bond from its canonical v1 envelope.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     A ``finstack_quant.instrument/1`` envelope containing an exact
    ///     ``"bond"`` payload. The UTF-8 input must not exceed 16 MiB.
    ///     Bare payloads and cross-type coercion are rejected.
    ///
    /// Returns
    /// -------
    /// Bond
    ///     The validated bond represented by the exact ``"bond"`` payload.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the input exceeds 16 MiB, is malformed, has an unsupported
    ///     envelope schema, carries a type other than ``"bond"``, or fails
    ///     bond validation.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import Bond
    /// >>> Bond.from_json(Bond.example().to_json()).id == Bond.example().id
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        match parse_typed_instrument_json(json)? {
            InstrumentJson::Bond(inner) => Ok(Self { inner }),
            _ => Err(value_error(
                "expected instrument type \"bond\", got a different instrument type",
            )),
        }
    }

    /// Serialize to a canonical ``finstack_quant.instrument/1`` envelope.
    ///
    /// Returns
    /// -------
    /// str
    ///     Canonical instrument envelope accepted by ``price_instrument`` and
    ///     ``Bond.from_json``.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        self.envelope_json()
    }

    /// Serde form of the bond spec as a Python ``dict`` (the ``spec`` object
    /// inside the instrument envelope).
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    /// Price the bond and return a ``ValuationResult``.
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
    ///     Model key (``"discounting"``, ``"hazard_rate"``, ``"tree"``,
    ///     ``"rates_credit"``, …). For bonds, ``"discounting"`` and
    ///     ``"hazard_rate"`` are non-callable rates-only and fractional-
    ///     recovery-of-par models;
    ///     ``"tree"`` values rates-only rights; ``"rates_credit"`` values
    ///     call, put, and return-floor rights jointly with credit risk.
    /// metrics : list[str], optional
    ///     Metric identifiers to compute (e.g. ``["ytm", "dv01"]``).
    /// pricing_options : dict | str, optional
    ///     ``MetricPricingOverrides`` merged into the instrument's overrides.
    /// market_history : str, optional
    ///     JSON ``MarketHistory`` scenarios for ``hvar`` / ``expected_shortfall``.
    ///
    /// Returns
    /// -------
    /// ValuationResult
    ///     Typed valuation envelope. Stochastic ``"rates_credit"`` pricing
    ///     includes Monte Carlo convergence and reproducibility diagnostics in
    ///     ``details``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the market, date or options cannot be interpreted or the
    ///     instrument fails validation.
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

    /// Compute one scalar metric (e.g. ``"dv01"``, ``"ytm"``).
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
    ///     ``"default"`` uses the bond-native selection. Explicit keys are
    ///     ``"discounting"`` for non-callable rates-only PV, ``"hazard_rate"``
    ///     for non-callable fractional recovery of par, ``"tree"`` for
    ///     rates-only exercise rights, and ``"rates_credit"`` for joint
    ///     rates-credit valuation including call, put, and return-floor rights.
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

    /// Return a copy with a minimum MOIC return floor on early redemption.
    ///
    /// Mirrors Rust ``Bond::min_moic``.
    ///
    /// Parameters
    /// ----------
    /// multiple : float
    ///     Minimum multiple of invested capital (e.g. ``1.25``).
    ///
    /// Returns
    /// -------
    /// Bond
    ///     A new bond with ``return_floor`` set.
    #[pyo3(text_signature = "($self, multiple)")]
    fn min_moic(&self, multiple: f64) -> Self {
        Self {
            inner: self.inner.clone().min_moic(multiple),
        }
    }

    /// Return a copy with a minimum XIRR return floor on early redemption.
    ///
    /// Mirrors Rust ``Bond::min_xirr``.
    ///
    /// Parameters
    /// ----------
    /// rate : float | Rate
    ///     Target annualized IRR as a decimal (``0.12`` = 12%) or a ``Rate``.
    ///
    /// Returns
    /// -------
    /// Bond
    ///     A new bond with ``return_floor`` set.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``rate`` is neither a number nor a ``Rate``.
    #[pyo3(text_signature = "($self, rate)")]
    fn min_xirr(&self, rate: &Bound<'_, PyAny>) -> PyResult<Self> {
        let rate = rate_from_py(rate, "rate")?;
        Ok(Self {
            inner: self.inner.clone().min_xirr(rate),
        })
    }

    /// Instrument identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.to_string()
    }

    /// Principal amount.
    #[getter]
    fn notional(&self) -> PyMoney {
        money_to_py(self.inner.notional)
    }

    /// Issue date.
    #[getter]
    fn issue_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.issue_date)
    }

    /// Maturity date.
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    /// Coupon/cashflow specification in serde form (``{"fixed": {...}}``,
    /// ``{"floating": {...}}``, ``{"step_up": {...}}`` or ``{"amortizing": {...}}``).
    #[getter]
    fn cashflow_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.cashflow_spec)
    }

    /// Discount curve identifier.
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    /// Forward curve identifier for floating coupons, or ``None``.
    #[getter]
    fn forward_curve_id(&self) -> Option<String> {
        self.inner
            .forward_curve_id
            .as_ref()
            .map(ToString::to_string)
    }

    /// Hazard curve identifier for credit-risky pricing, or ``None``.
    #[getter]
    fn credit_curve_id(&self) -> Option<String> {
        self.inner.credit_curve_id.as_ref().map(ToString::to_string)
    }

    /// Funding curve identifier, or ``None``.
    #[getter]
    fn funding_curve_id(&self) -> Option<String> {
        self.inner
            .funding_curve_id
            .as_ref()
            .map(ToString::to_string)
    }

    /// Call/put schedule in serde form (``{"calls": [...], "puts": [...]}``), or ``None``.
    #[getter]
    fn call_put<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.call_put.as_ref())
    }

    /// Return-floor specification in serde form, or ``None``.
    #[getter]
    fn return_floor<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.return_floor.as_ref())
    }

    /// Explicit cashflow schedule in serde form, or ``None``.
    #[getter]
    fn custom_cashflows<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.custom_cashflows.as_ref())
    }

    /// Accrual method (serde string, e.g. ``"linear"``).
    #[getter]
    fn accrual_method(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.accrual_method)
    }

    /// Instrument attributes (tags and metadata).
    #[getter]
    fn attributes(&self) -> PyAttributes {
        attributes_to_py(&self.inner.attributes)
    }

    /// Settlement convention (``settlement_days`` / ``ex_coupon_days`` /
    /// ``ex_coupon_calendar_id``) as a dict, or ``None`` when unset.
    #[getter]
    fn settlement_convention<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.settlement_convention.as_ref())
    }

    /// Settlement lag in business days, or ``None`` when no convention is set.
    #[getter]
    fn settlement_days(&self) -> Option<u32> {
        self.inner.settlement_days()
    }

    /// Ex-coupon period in business days, or ``None`` when no convention is set.
    #[getter]
    fn ex_coupon_days(&self) -> Option<u32> {
        self.inner.ex_coupon_days()
    }

    /// ``True`` when coupons depend on forward-curve projection (FRNs).
    #[getter]
    fn has_floating_coupons(&self) -> bool {
        self.inner.has_floating_coupons()
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

    /// Market-data dependencies (curves, fixings, vol surfaces) as a dict.
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
            "Bond(id={:?}, notional={}, issue_date={}, maturity={}, discount_curve_id={:?}, credit_curve_id={})",
            self.inner.id.as_str(),
            money_repr(self.inner.notional),
            self.inner.issue_date,
            self.inner.maturity,
            self.inner.discount_curve_id.as_str(),
            opt_repr(self.inner.credit_curve_id.as_ref().map(|c| format!("{:?}", c.as_str()))),
        )
    }

    /// Price this bond with the Merton Monte Carlo structural credit engine.
    ///
    /// Uses geometric Brownian motion asset dynamics only. Floating-rate and
    /// amortizing cashflow specs are rejected. When the config's PIK schedule
    /// is the default uniform cash mode, the bond's ``CouponType`` overrides
    /// the schedule; otherwise the config schedule takes precedence.
    ///
    /// Parameters
    /// ----------
    /// config : MertonMcConfig
    ///     Merton MC simulation configuration including the structural model.
    /// discount_rate : float
    ///     Flat continuously compounded risk-free rate as a decimal used to
    ///     discount simulated cashflows (unless term-structure discount
    ///     factors are set on the config).
    /// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Valuation date.
    ///
    /// Returns
    /// -------
    /// MertonMcResult
    ///     Monte Carlo pricing result with clean/dirty prices and path stats.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``as_of`` is invalid, the bond has floating or amortizing
    ///     cashflows, or simulation parameters fail validation.
    #[pyo3(text_signature = "($self, config, discount_rate, as_of)")]
    fn price_merton_mc(
        &self,
        config: PyRef<'_, PyMertonMcConfig>,
        discount_rate: f64,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PyMertonMcResult> {
        let as_of = extract_date(as_of)?;
        let result = self
            .inner
            .price_merton_mc(&config.inner, discount_rate, as_of)
            .map_err(core_to_py)?;
        Ok(PyMertonMcResult::from_inner(result))
    }
}

/// Fluent builder for ``Bond``; wraps the Rust `FinancialBuilder`-generated
/// builder (consuming setters).
///
/// Builders are consumed by build(); create a new builder per instrument.
/// Nested specs (``cashflow_spec``, ``call_put``, ``return_floor``,
/// ``custom_cashflows``, ``settlement_convention``) accept a ``dict`` or a
/// JSON ``str`` in the Rust serde shape.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "BondBuilder",
    skip_from_py_object
)]
pub struct PyBondBuilder {
    inner: Option<BondBuilderInner>,
    fields: Vec<(&'static str, String)>,
}

/// Take the wrapped Rust builder or fail if `build()` already consumed it.
fn take_bond(b: &mut PyBondBuilder) -> PyResult<BondBuilderInner> {
    b.inner
        .take()
        .ok_or_else(|| value_error("builder already consumed by build()"))
}

#[pymethods]
impl PyBondBuilder {
    /// Set the instrument identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Unique identifier for the bond.
    ///
    /// Returns
    /// -------
    /// BondBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_bond(&mut slf)?;
        slf.inner = Some(b.id(InstrumentId::new(value.to_string())));
        slf.fields.push(("id", format!("{value:?}")));
        Ok(slf)
    }

    /// Set the principal amount.
    ///
    /// Parameters
    /// ----------
    /// value : Money | float
    ///     Principal; a bare number needs ``currency``.
    /// currency : str, optional
    ///     ISO-4217 code applied when ``value`` is a bare number.
    ///
    /// Returns
    /// -------
    /// BondBuilder
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
        let b = take_bond(&mut slf)?;
        slf.inner = Some(b.notional(money));
        slf.fields.push(("notional", money_repr(money)));
        Ok(slf)
    }

    /// Set the issue date.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Issue date. When omitted, Rust infers ``maturity - 365 days``.
    ///
    /// Returns
    /// -------
    /// BondBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn issue_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = extract_date(value)?;
        let b = take_bond(&mut slf)?;
        slf.inner = Some(b.issue_date(date));
        slf.fields.push(("issue_date", date.to_string()));
        Ok(slf)
    }

    /// Set the maturity date.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Maturity date.
    ///
    /// Returns
    /// -------
    /// BondBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = extract_date(value)?;
        let b = take_bond(&mut slf)?;
        slf.inner = Some(b.maturity(date));
        slf.fields.push(("maturity", date.to_string()));
        Ok(slf)
    }

    /// Set the coupon/cashflow specification.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     Rust ``CashflowSpec`` in serde form, e.g.
    ///     ``{"fixed": {"coupon_type": "cash", "rate": "0.05", "schedule": {...}}}``.
    ///     Start from ``Bond.example().cashflow_spec`` for the exact shape.
    ///
    /// Returns
    /// -------
    /// BondBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not deserialize as a ``CashflowSpec``.
    #[pyo3(text_signature = "($self, value)")]
    fn cashflow_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec =
            spec_from_py(py, value, "cashflow_spec")?;
        let b = take_bond(&mut slf)?;
        slf.inner = Some(b.cashflow_spec(spec));
        slf.fields.push(("cashflow_spec", "{...}".to_string()));
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
    /// BondBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_bond(&mut slf)?;
        slf.inner = Some(b.discount_curve_id(CurveId::new(value.to_string())));
        slf.fields.push(("discount_curve_id", format!("{value:?}")));
        Ok(slf)
    }

    /// Set the forward curve identifier used by floating coupons.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Forward curve identifier.
    ///
    /// Returns
    /// -------
    /// BondBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn forward_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_bond(&mut slf)?;
        slf.inner = Some(b.forward_curve_id(CurveId::new(value.to_string())));
        slf.fields.push(("forward_curve_id", format!("{value:?}")));
        Ok(slf)
    }

    /// Set the hazard curve identifier for ``"hazard_rate"`` or ``"rates_credit"`` pricing.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Hazard curve identifier consumed by scalar or joint rates-credit pricing.
    ///
    /// Returns
    /// -------
    /// BondBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn credit_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_bond(&mut slf)?;
        slf.inner = Some(b.credit_curve_id(CurveId::new(value.to_string())));
        slf.fields.push(("credit_curve_id", format!("{value:?}")));
        Ok(slf)
    }

    /// Set the funding curve identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Funding curve identifier.
    ///
    /// Returns
    /// -------
    /// BondBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn funding_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_bond(&mut slf)?;
        slf.inner = Some(b.funding_curve_id(CurveId::new(value.to_string())));
        slf.fields.push(("funding_curve_id", format!("{value:?}")));
        Ok(slf)
    }

    /// Set the call/put schedule.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     Rust ``CallPutSchedule`` in serde form:
    ///     ``{"calls": [{"start_date": "2027-01-15", "end_date": "2029-01-15",
    ///     "price_pct_of_par": 100.0}], "puts": []}``.
    ///
    /// Returns
    /// -------
    /// BondBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not deserialize as a ``CallPutSchedule``.
    #[pyo3(text_signature = "($self, value)")]
    fn call_put<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::bond::CallPutSchedule =
            spec_from_py(py, value, "call_put")?;
        let b = take_bond(&mut slf)?;
        slf.inner = Some(b.call_put(spec));
        slf.fields.push(("call_put", "{...}".to_string()));
        Ok(slf)
    }

    /// Set the return-floor specification (minimum MOIC / XIRR on early redemption).
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     Rust ``ReturnFloorSpec`` in serde form.
    ///
    /// Returns
    /// -------
    /// BondBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not deserialize as a ``ReturnFloorSpec``.
    #[pyo3(text_signature = "($self, value)")]
    fn return_floor<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::bond::ReturnFloorSpec =
            spec_from_py(py, value, "return_floor")?;
        let b = take_bond(&mut slf)?;
        slf.inner = Some(b.return_floor(spec));
        slf.fields.push(("return_floor", "{...}".to_string()));
        Ok(slf)
    }

    /// Set an explicit cashflow schedule that overrides generated coupons.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     Rust ``CashFlowSchedule`` in serde form.
    ///
    /// Returns
    /// -------
    /// BondBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not deserialize as a ``CashFlowSchedule``.
    #[pyo3(text_signature = "($self, value)")]
    fn custom_cashflows<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let schedule: finstack_quant_cashflows::builder::CashFlowSchedule =
            spec_from_py(py, value, "custom_cashflows")?;
        let b = take_bond(&mut slf)?;
        slf.inner = Some(b.custom_cashflows(schedule));
        slf.fields.push(("custom_cashflows", "{...}".to_string()));
        Ok(slf)
    }

    /// Set the accrual method.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Serde name of the Rust ``AccrualMethod`` (``"linear"`` is the default).
    ///
    /// Returns
    /// -------
    /// BondBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized accrual method.
    #[pyo3(text_signature = "($self, value)")]
    fn accrual_method<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let method: finstack_quant_valuations::instruments::fixed_income::bond::AccrualMethod =
            enum_from_str(value, "accrual_method")?;
        let b = take_bond(&mut slf)?;
        slf.inner = Some(b.accrual_method(method));
        slf.fields.push(("accrual_method", format!("{value:?}")));
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
    /// BondBuilder
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
        let b = take_bond(&mut slf)?;
        slf.inner = Some(b.attributes(attrs));
        slf.fields
            .push(("attributes", "Attributes(...)".to_string()));
        Ok(slf)
    }

    /// Set the settlement convention (settlement lag and ex-coupon period).
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``{"settlement_days": 2, "ex_coupon_days": 0, "ex_coupon_calendar_id": None}``.
    ///
    /// Returns
    /// -------
    /// BondBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not deserialize as a ``BondSettlementConvention``.
    #[pyo3(text_signature = "($self, value)")]
    fn settlement_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let convention: finstack_quant_valuations::instruments::fixed_income::bond::BondSettlementConvention =
            spec_from_py(py, value, "settlement_convention")?;
        let b = take_bond(&mut slf)?;
        slf.inner = Some(b.settlement_convention(convention));
        slf.fields
            .push(("settlement_convention", "{...}".to_string()));
        Ok(slf)
    }

    /// Build the validated bond.
    ///
    /// Returns
    /// -------
    /// Bond
    ///     The validated bond.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed, a required field is missing
    ///     (the message names the field), or the bond fails validation.
    #[pyo3(text_signature = "($self)")]
    fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<PyBond> {
        let b = take_bond(&mut slf)?;
        let inner = b.build().map_err(core_to_py)?;
        Ok(PyBond { inner })
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        builder_repr("BondBuilder", &self.fields)
    }
}

/// Typed wrapper for the Rust `TermLoan` instrument.
///
/// Construct via ``TermLoan.builder()`` (the Rust ``RateSpec`` accepts a bare
/// fixed rate or a floating spec), the ``TermLoan.example*`` presets, or
/// ``TermLoan.from_json``. Every public Rust field is readable as a property;
/// ``price`` / ``metric`` run the same pricer as ``price_instrument``.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "TermLoan",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyTermLoan {
    /// Inner canonical Rust term loan.
    pub(crate) inner: finstack_quant_valuations::instruments::TermLoan,
}

impl PyTermLoan {
    /// Serialize as the canonical instrument envelope accepted by the JSON loader.
    pub(crate) fn envelope_json(&self) -> PyResult<String> {
        serialize_typed_instrument_json(InstrumentJson::TermLoan(self.inner.clone()), "TermLoan")
    }
}

#[pymethods]
impl PyTermLoan {
    /// Create a fluent builder (mirrors Rust ``TermLoan::builder()``).
    ///
    /// Returns
    /// -------
    /// TermLoanBuilder
    ///     A builder with fluent, consuming setter methods.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import TermLoan
    /// >>> builder = TermLoan.builder()
    /// >>> builder.id("EXAMPLE") is builder
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn builder() -> PyTermLoanBuilder {
        PyTermLoanBuilder {
            inner: Some(finstack_quant_valuations::instruments::TermLoan::builder()),
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
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a validated term loan from its canonical v1 envelope.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     A ``finstack_quant.instrument/1`` envelope containing an exact
    ///     ``"term_loan"`` payload. The UTF-8 input must not exceed 16 MiB.
    ///     Bare payloads and cross-type coercion are rejected.
    ///
    /// Returns
    /// -------
    /// TermLoan
    ///     The validated term loan represented by the exact ``"term_loan"`` payload.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the input exceeds 16 MiB, is malformed, has an unsupported
    ///     envelope schema, carries a type other than ``"term_loan"``, or
    ///     fails term-loan validation.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import TermLoan
    /// >>> TermLoan.from_json(TermLoan.example().to_json()).id
    /// 'TERM-LOAN-USD-5Y'
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        match parse_typed_instrument_json(json)? {
            InstrumentJson::TermLoan(inner) => Ok(Self { inner }),
            _ => Err(value_error(
                "expected instrument type \"term_loan\", got a different instrument type",
            )),
        }
    }

    /// Canonical example term loan (mirrors Rust ``TermLoan::example``).
    ///
    /// Returns a 5-year USD fixed-rate loan (6%, quarterly, Act/360, 2.5%
    /// per-period amortization) useful as a starting point and in tests.
    ///
    /// Returns
    /// -------
    /// TermLoan
    ///     The example loan.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If construction fails (should not occur).
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import TermLoan
    /// >>> TermLoan.example().id
    /// 'TERM-LOAN-USD-5Y'
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::TermLoan::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Example floating-rate leveraged loan with a delayed-draw tranche
    /// (mirrors Rust ``TermLoan::example_floating_with_ddtl``).
    ///
    /// Returns
    /// -------
    /// TermLoan
    ///     A 7-year USD SOFR + 400bp loan with a DDTL commitment and a 0% floor.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If construction fails (should not occur).
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import TermLoan
    /// >>> TermLoan.example_floating_with_ddtl().ddtl is not None
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example_floating_with_ddtl() -> PyResult<Self> {
        finstack_quant_valuations::instruments::TermLoan::example_floating_with_ddtl()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Example callable term loan (mirrors Rust ``TermLoan::example_callable``).
    ///
    /// Returns
    /// -------
    /// TermLoan
    ///     A loan carrying a prepayment (call) schedule.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If construction fails (should not occur).
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import TermLoan
    /// >>> TermLoan.example_callable().call_schedule is not None
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example_callable() -> PyResult<Self> {
        finstack_quant_valuations::instruments::TermLoan::example_callable()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Serialize to a canonical ``finstack_quant.instrument/1`` envelope.
    ///
    /// Returns
    /// -------
    /// str
    ///     Canonical instrument envelope accepted by ``price_instrument`` and
    ///     ``TermLoan.from_json``.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        self.envelope_json()
    }

    /// Serde form of the loan spec as a Python ``dict``.
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    /// Price the loan and return a ``ValuationResult``.
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

    /// Compute one scalar metric (e.g. ``"dv01"``).
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

    /// Loan currency (ISO-4217 code).
    #[getter]
    fn currency(&self) -> String {
        self.inner.currency.to_string()
    }

    /// Committed notional (facility limit).
    #[getter]
    fn notional_limit(&self) -> PyMoney {
        money_to_py(self.inner.notional_limit)
    }

    /// Issue / funding date.
    #[getter]
    fn issue_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.issue_date)
    }

    /// Maturity date.
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    /// Rate specification in serde form (``{"fixed": {"rate_bp": 600}}`` or
    /// ``{"floating": {...}}``).
    #[getter]
    fn rate<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.rate)
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

    /// Business day convention (serde string).
    #[getter]
    fn business_day_convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.business_day_convention)
    }

    /// Holiday calendar identifier, or ``None``.
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id.clone()
    }

    /// Stub rule.
    #[getter]
    fn stub(&self) -> PyStubKind {
        PyStubKind::from_inner(self.inner.stub)
    }

    /// Discount curve identifier.
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    /// Hazard curve identifier, or ``None``.
    #[getter]
    fn credit_curve_id(&self) -> Option<String> {
        self.inner.credit_curve_id.as_ref().map(ToString::to_string)
    }

    /// Amortization specification in serde form (``"none"``,
    /// ``{"percent_per_period": {"bp": 250}}``, ``{"linear": {...}}``, …).
    #[getter]
    fn amortization<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.amortization)
    }

    /// Coupon type (serde string, e.g. ``"cash"``).
    #[getter]
    fn coupon_type(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.coupon_type)
    }

    /// Upfront fee, or ``None``.
    #[getter]
    fn upfront_fee(&self) -> Option<PyMoney> {
        self.inner.upfront_fee.map(money_to_py)
    }

    /// Delayed-draw term loan specification in serde form, or ``None``.
    #[getter]
    fn ddtl<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.ddtl.as_ref())
    }

    /// Covenant event schedule in serde form, or ``None``.
    #[getter]
    fn covenants<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.covenants.as_ref())
    }

    /// OID / effective-interest-rate specification in serde form, or ``None``.
    #[getter]
    fn oid_eir<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.oid_eir.as_ref())
    }

    /// Prepayment (call) schedule in serde form, or ``None``.
    #[getter]
    fn call_schedule<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.call_schedule.as_ref())
    }

    /// Settlement lag in business days.
    #[getter]
    fn settlement_days(&self) -> u32 {
        self.inner.settlement_days
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

    /// Market-data dependencies (curves, fixings) as a dict.
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
            "TermLoan(id={:?}, notional_limit={}, issue_date={}, maturity={}, discount_curve_id={:?})",
            self.inner.id.as_str(),
            money_repr(self.inner.notional_limit),
            self.inner.issue_date,
            self.inner.maturity,
            self.inner.discount_curve_id.as_str(),
        )
    }
}

/// Fluent builder for ``TermLoan``; wraps the Rust `FinancialBuilder`-generated
/// builder (consuming setters).
///
/// Builders are consumed by build(); create a new builder per instrument.
/// Nested specs (``rate`` when floating, ``amortization``, ``ddtl``,
/// ``covenants``, ``oid_eir``, ``call_schedule``) accept a ``dict`` or a JSON
/// ``str`` in the Rust serde shape.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "TermLoanBuilder",
    skip_from_py_object
)]
pub struct PyTermLoanBuilder {
    inner: Option<TermLoanBuilderInner>,
    fields: Vec<(&'static str, String)>,
}

/// Take the wrapped Rust builder or fail if `build()` already consumed it.
fn take_term_loan(b: &mut PyTermLoanBuilder) -> PyResult<TermLoanBuilderInner> {
    b.inner
        .take()
        .ok_or_else(|| value_error("builder already consumed by build()"))
}

#[pymethods]
impl PyTermLoanBuilder {
    /// Set the instrument identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Unique identifier for the loan.
    ///
    /// Returns
    /// -------
    /// TermLoanBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.id(InstrumentId::new(value.to_string())));
        slf.fields.push(("id", format!("{value:?}")));
        Ok(slf)
    }

    /// Set the loan currency.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     ISO-4217 currency code (e.g. ``"USD"``).
    ///
    /// Returns
    /// -------
    /// TermLoanBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized currency code.
    #[pyo3(text_signature = "($self, value)")]
    fn currency<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let ccy = crate::bindings::module_utils::parse_currency(value)?;
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.currency(ccy));
        slf.fields.push(("currency", format!("{value:?}")));
        Ok(slf)
    }

    /// Set the committed notional (facility limit).
    ///
    /// Parameters
    /// ----------
    /// value : Money | float
    ///     Commitment; a bare number needs ``currency``.
    /// currency : str, optional
    ///     ISO-4217 code applied when ``value`` is a bare number.
    ///
    /// Returns
    /// -------
    /// TermLoanBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a bare number is given without ``currency``.
    #[pyo3(signature = (value, currency = None))]
    #[pyo3(text_signature = "($self, value, currency=None)")]
    fn notional_limit<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
        currency: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, currency, "notional_limit")?;
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.notional_limit(money));
        slf.fields.push(("notional_limit", money_repr(money)));
        Ok(slf)
    }

    /// Set the issue / funding date.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Issue date. When omitted, Rust infers ``maturity - 365 days``.
    ///
    /// Returns
    /// -------
    /// TermLoanBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn issue_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = extract_date(value)?;
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.issue_date(date));
        slf.fields.push(("issue_date", date.to_string()));
        Ok(slf)
    }

    /// Set the maturity date.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Maturity date.
    ///
    /// Returns
    /// -------
    /// TermLoanBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = extract_date(value)?;
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.maturity(date));
        slf.fields.push(("maturity", date.to_string()));
        Ok(slf)
    }

    /// Set the interest rate specification.
    ///
    /// Parameters
    /// ----------
    /// value : float | Rate | dict | str
    ///     A bare decimal (``0.06`` = 6%) or ``Rate`` sets a fixed rate
    ///     (mirrors Rust ``RateSpec::fixed_rate``; rounded to whole basis
    ///     points). A ``dict``/JSON ``str`` is the Rust ``RateSpec`` in serde
    ///     form, e.g. ``{"floating": {"index_id": "USD-SOFR-3M", "spread_bp": 400, ...}}``.
    ///
    /// Returns
    /// -------
    /// TermLoanBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a dict/str does not deserialize as a ``RateSpec``.
    /// TypeError
    ///     If ``value`` has an unsupported type.
    #[pyo3(text_signature = "($self, value)")]
    fn rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let (spec, shown) = if value.extract::<&str>().is_ok() || value.cast::<PyDict>().is_ok() {
            let spec: finstack_quant_valuations::instruments::fixed_income::term_loan::RateSpec =
                spec_from_py(py, value, "rate")?;
            (spec, "{...}".to_string())
        } else {
            let rate = rate_from_py(value, "rate")?;
            (
                finstack_quant_valuations::instruments::fixed_income::term_loan::RateSpec::fixed_rate(rate),
                rate.as_decimal().to_string(),
            )
        };
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.rate(spec));
        slf.fields.push(("rate", shown));
        Ok(slf)
    }

    /// Set the payment frequency.
    ///
    /// Parameters
    /// ----------
    /// value : Tenor
    ///     Payment frequency (e.g. ``Tenor.quarterly()``).
    ///
    /// Returns
    /// -------
    /// TermLoanBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyTenor>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let tenor = value.inner;
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.frequency(tenor));
        slf.fields.push(("frequency", tenor.to_string()));
        Ok(slf)
    }

    /// Set the accrual day-count convention.
    ///
    /// Parameters
    /// ----------
    /// value : DayCount
    ///     Day count convention (e.g. ``DayCount.ACT_360``).
    ///
    /// Returns
    /// -------
    /// TermLoanBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyDayCount>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let day_count = value.inner;
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.day_count(day_count));
        slf.fields.push(("day_count", day_count.to_string()));
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
    /// TermLoanBuilder
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
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.business_day_convention(convention));
        slf.fields
            .push(("business_day_convention", format!("{value:?}")));
        Ok(slf)
    }

    /// Set the holiday calendar identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Calendar identifier (e.g. ``"usny"``).
    ///
    /// Returns
    /// -------
    /// TermLoanBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.calendar_id(value.to_string()));
        slf.fields.push(("calendar_id", format!("{value:?}")));
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
    /// TermLoanBuilder
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
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.stub(stub));
        slf.fields.push((
            "stub",
            format!("{:?}", enum_to_py_string(&stub).unwrap_or_default()),
        ));
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
    /// TermLoanBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.discount_curve_id(CurveId::new(value.to_string())));
        slf.fields.push(("discount_curve_id", format!("{value:?}")));
        Ok(slf)
    }

    /// Set the hazard curve identifier for credit-risky pricing.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Hazard curve identifier.
    ///
    /// Returns
    /// -------
    /// TermLoanBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn credit_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.credit_curve_id(CurveId::new(value.to_string())));
        slf.fields.push(("credit_curve_id", format!("{value:?}")));
        Ok(slf)
    }

    /// Set the amortization schedule.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     Rust ``AmortizationSpec`` in serde form: ``"none"``,
    ///     ``{"percent_per_period": {"bp": 250}}``,
    ///     ``{"linear": {"start": "2025-01-01", "end": "2029-01-01"}}``, …
    ///
    /// Returns
    /// -------
    /// TermLoanBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not deserialize as an ``AmortizationSpec``.
    #[pyo3(text_signature = "($self, value)")]
    fn amortization<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::term_loan::AmortizationSpec =
            if let Ok(name) = value.extract::<&str>() {
                if name.trim_start().starts_with('{') || name.trim_start().starts_with('"') {
                    json_field(name, "amortization")?
                } else {
                    enum_from_str(name, "amortization")?
                }
            } else {
                spec_from_py(py, value, "amortization")?
            };
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.amortization(spec));
        slf.fields.push(("amortization", "{...}".to_string()));
        Ok(slf)
    }

    /// Set the coupon type (default ``"cash"``).
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Serde name of the Rust ``CouponType`` (``"cash"``, ``"pik"``, …).
    ///
    /// Returns
    /// -------
    /// TermLoanBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized coupon type.
    #[pyo3(text_signature = "($self, value)")]
    fn coupon_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let coupon_type = enum_from_str(value, "coupon_type")?;
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.coupon_type(coupon_type));
        slf.fields.push(("coupon_type", format!("{value:?}")));
        Ok(slf)
    }

    /// Set the upfront fee.
    ///
    /// Parameters
    /// ----------
    /// value : Money | float
    ///     Fee amount; a bare number needs ``currency``.
    /// currency : str, optional
    ///     ISO-4217 code applied when ``value`` is a bare number.
    ///
    /// Returns
    /// -------
    /// TermLoanBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a bare number is given without ``currency``.
    #[pyo3(signature = (value, currency = None))]
    #[pyo3(text_signature = "($self, value, currency=None)")]
    fn upfront_fee<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
        currency: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, currency, "upfront_fee")?;
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.upfront_fee(money));
        slf.fields.push(("upfront_fee", money_repr(money)));
        Ok(slf)
    }

    /// Set the delayed-draw (DDTL) specification.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     Rust ``DdtlSpec`` in serde form.
    ///
    /// Returns
    /// -------
    /// TermLoanBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not deserialize as a ``DdtlSpec``.
    #[pyo3(text_signature = "($self, value)")]
    fn ddtl<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::term_loan::DdtlSpec =
            spec_from_py(py, value, "ddtl")?;
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.ddtl(spec));
        slf.fields.push(("ddtl", "{...}".to_string()));
        Ok(slf)
    }

    /// Set the covenant event schedule.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     Rust ``TermLoanCovenantEvents`` in serde form.
    ///
    /// Returns
    /// -------
    /// TermLoanBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not deserialize as ``TermLoanCovenantEvents``.
    #[pyo3(text_signature = "($self, value)")]
    fn covenants<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoanCovenantEvents =
            spec_from_py(py, value, "covenants")?;
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.covenants(spec));
        slf.fields.push(("covenants", "{...}".to_string()));
        Ok(slf)
    }

    /// Set the OID / effective-interest-rate specification.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     Rust ``OidEirSpec`` in serde form.
    ///
    /// Returns
    /// -------
    /// TermLoanBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not deserialize as an ``OidEirSpec``.
    #[pyo3(text_signature = "($self, value)")]
    fn oid_eir<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::term_loan::OidEirSpec =
            spec_from_py(py, value, "oid_eir")?;
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.oid_eir(spec));
        slf.fields.push(("oid_eir", "{...}".to_string()));
        Ok(slf)
    }

    /// Set the prepayment (call) schedule.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     Rust ``LoanCallSchedule`` in serde form.
    ///
    /// Returns
    /// -------
    /// TermLoanBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not deserialize as a ``LoanCallSchedule``.
    #[pyo3(text_signature = "($self, value)")]
    fn call_schedule<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: finstack_quant_valuations::instruments::fixed_income::term_loan::LoanCallSchedule =
            spec_from_py(py, value, "call_schedule")?;
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.call_schedule(spec));
        slf.fields.push(("call_schedule", "{...}".to_string()));
        Ok(slf)
    }

    /// Set the settlement lag in business days (default 2).
    ///
    /// Parameters
    /// ----------
    /// value : int
    ///     Settlement lag.
    ///
    /// Returns
    /// -------
    /// TermLoanBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn settlement_days<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: u32,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.settlement_days(value));
        slf.fields.push(("settlement_days", value.to_string()));
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
    /// TermLoanBuilder
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
        let b = take_term_loan(&mut slf)?;
        slf.inner = Some(b.attributes(attrs));
        slf.fields
            .push(("attributes", "Attributes(...)".to_string()));
        Ok(slf)
    }

    /// Build the validated term loan.
    ///
    /// Returns
    /// -------
    /// TermLoan
    ///     The validated loan.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed, a required field is missing
    ///     (the message names the field), or the loan fails validation.
    #[pyo3(text_signature = "($self)")]
    fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<PyTermLoan> {
        let b = take_term_loan(&mut slf)?;
        let inner = b.build().map_err(core_to_py)?;
        Ok(PyTermLoan { inner })
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        builder_repr("TermLoanBuilder", &self.fields)
    }
}

/// Register the typed instrument classes on the instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBond>()?;
    m.add_class::<PyBondBuilder>()?;
    m.add_class::<PyTermLoan>()?;
    m.add_class::<PyTermLoanBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
///
/// Extend this list (sorted) when adding a class or function here; `mod.rs`
/// merges every submodule list so registration stays in one place per file.
pub(crate) const EXPORTS: &[&str] = &["BondBuilder", "TermLoanBuilder"];
