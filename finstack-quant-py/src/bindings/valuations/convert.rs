//! Shared Python → Rust value coercions for the valuations bindings.
//!
//! Every typed instrument, leg spec and builder in this domain accepts the
//! same loose input forms, so the rules live here rather than being restated
//! per class:
//!
//! - rates: `float | int | Rate | Bps` (decimal, `0.05` = 5%) and spreads:
//!   `float | int | Bps | Rate` (basis points), through the crate-wide
//!   `extract::{extract_rate_decimal, extract_basis_points}`
//! - money: `Money | float | int` (a bare number needs a currency from the caller)
//! - enums: serde string names, resolved through `enum_from_str`
//! - attributes: `Attributes | dict | None`, a `dict` being the serde form of
//!   `Attributes` (`{"tags": [...], "meta": {...}}`, as `to_dict()` returns it)
//!
//! Outbound conversions (`enum_to_py_string`, `money_to_py`) are the exact
//! inverses so getters render what the constructor accepted.

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::bindings::core::dates::schedule::PyStubKind;
use crate::bindings::core::money::PyMoney;
use crate::bindings::core::types::PyAttributes;
use crate::bindings::date_utils::date_to_py;
use crate::bindings::extract::{extract_basis_points, extract_rate_decimal};
use crate::bindings::pandas_utils::serde_to_py;
use crate::errors::{core_to_py, serde_json_to_py, value_error};
use finstack_quant_valuations::instruments::json_loader::{InstrumentEnvelope, InstrumentJson};
use finstack_quant_valuations::instruments::Instrument;

/// Coerce `Money | float | int` to [`Money`](finstack_quant_core::money::Money).
///
/// # Arguments
///
/// * `obj` - A `finstack_quant.core.money.Money`, or a bare amount that is
///   tagged with `currency`.
/// * `currency` - ISO-4217 code applied when `obj` is a bare number; ignored
///   for a `Money` object (its own currency wins).
/// * `what` - Parameter name used in error messages.
pub(crate) fn money_from_py(
    obj: &Bound<'_, PyAny>,
    currency: Option<&str>,
    what: &str,
) -> PyResult<finstack_quant_core::money::Money> {
    if let Ok(money) = obj.cast::<PyMoney>() {
        return Ok(money.borrow().inner);
    }
    if let Ok(amount) = obj.extract::<f64>() {
        let Some(code) = currency else {
            return Err(value_error(format!(
                "{what}: a bare number needs a currency; pass Money(amount, \"USD\") or supply currency="
            )));
        };
        let ccy = crate::bindings::module_utils::parse_currency(code)?;
        return finstack_quant_core::money::Money::new(amount, ccy)
            .map_err(crate::errors::core_to_py);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(format!(
        "{what}: expected finstack_quant.core.money.Money or a float amount, got {}",
        obj.get_type().name()?
    )))
}

/// Wrap a Rust [`Money`](finstack_quant_core::money::Money) for Python.
pub(crate) fn money_to_py(value: finstack_quant_core::money::Money) -> PyMoney {
    PyMoney { inner: value }
}

/// Render a serde-string enum (or any `Serialize` value that serializes as a
/// JSON string) as the bare Python string the matching setter accepts.
///
/// # Arguments
///
/// * `value` - Enum value whose serde representation is a JSON string
///   (`"pay"`, `"modified_following"`, `"isda_na"`, …).
pub(crate) fn enum_to_py_string<T: serde::Serialize>(value: &T) -> PyResult<String> {
    let json = serde_json::to_value(value).map_err(crate::errors::display_to_py)?;
    match json {
        serde_json::Value::String(s) => Ok(s),
        other => Ok(other.to_string()),
    }
}

/// Coerce `Attributes | dict | None` to a Rust attribute bag.
///
/// A `dict` is deserialized as the serde form of
/// [`finstack_quant_core::types::Attributes`]: optional `"tags"` (list of
/// strings) and `"meta"` (string-to-string mapping), exactly what `to_dict()`
/// returns. Any other key or a non-string value is rejected, never coerced.
///
/// # Arguments
///
/// * `obj` - `finstack_quant.core.types.Attributes`, the serde `dict` form, or
///   `None` (an empty bag).
///
/// # Errors
///
/// `TypeError` for any other Python type; `ValueError` when the `dict` does not
/// match the `Attributes` serde shape (unknown key, non-string tag or value).
pub(crate) fn attributes_from_py(
    obj: &Bound<'_, PyAny>,
) -> PyResult<finstack_quant_core::types::Attributes> {
    if obj.is_none() {
        return Ok(finstack_quant_core::types::Attributes::new());
    }
    if let Ok(attrs) = obj.cast::<PyAttributes>() {
        return Ok(attrs.borrow().inner.clone());
    }
    if obj.cast::<PyDict>().is_err() {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "attributes: expected finstack_quant.core.types.Attributes or a dict with \"tags\" / \"meta\", got {}",
            obj.get_type().name()?
        )));
    }
    crate::bindings::module_utils::py_to_serde(obj.py(), obj, "attributes")
}

/// Wrap a Rust attribute bag for Python.
pub(crate) fn attributes_to_py(value: &finstack_quant_core::types::Attributes) -> PyAttributes {
    PyAttributes::from_inner(value.clone())
}

/// Render an `Option<T: Debug-ish>` the Python way (`None` instead of `None`/`Some(..)`).
///
/// # Arguments
///
/// * `value` - Optional display value rendered with `Display`.
pub(crate) fn opt_repr<T: std::fmt::Display>(value: Option<T>) -> String {
    match value {
        Some(v) => v.to_string(),
        None => "None".to_string(),
    }
}

/// Render a `bool` the Python way (`True`/`False`).
pub(crate) fn bool_repr(value: bool) -> &'static str {
    if value {
        "True"
    } else {
        "False"
    }
}

/// Parse a business-day-convention name with the Rust `FromStr` (the parser
/// `adjust` uses), so every Python BDC string argument accepts the same
/// canonical serde names, ignoring surrounding whitespace and ASCII case.
///
/// # Arguments
///
/// * `value` - Convention name supplied by the caller.
/// * `what` - Parameter name used in the error message.
pub(crate) fn bdc_from_str(
    value: &str,
    what: &str,
) -> PyResult<finstack_quant_core::dates::BusinessDayConvention> {
    value
        .parse::<finstack_quant_core::dates::BusinessDayConvention>()
        .map_err(|e| crate::errors::value_error(format!("invalid {what}: {e}")))
}

/// Coerce `BusinessDayConvention | str` to a Rust business-day convention.
///
/// # Arguments
///
/// * `obj` - A `finstack_quant.core.dates.BusinessDayConvention` or a name
///   accepted by the Rust `BusinessDayConvention` `FromStr` (the parser
///   `adjust` uses): `"modified_following"`, `"following"`, `"preceding"`, …
///   with surrounding whitespace and ASCII case ignored.
/// * `what` - Parameter name used in the error message.
pub(crate) fn bdc_from_py(
    obj: &Bound<'_, PyAny>,
    what: &str,
) -> PyResult<finstack_quant_core::dates::BusinessDayConvention> {
    if let Ok(convention) =
        obj.cast::<crate::bindings::core::dates::calendar::PyBusinessDayConvention>()
    {
        return Ok(convention.borrow().inner);
    }
    if let Ok(text) = obj.extract::<std::borrow::Cow<'_, str>>() {
        return bdc_from_str(&text, what);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(format!(
        "{what}: expected finstack_quant.core.dates.BusinessDayConvention or its string name, got {}",
        obj.get_type().name()?
    )))
}

/// Coerce `DayCount | str` to a Rust day-count convention.
///
/// # Arguments
///
/// * `obj` - A `finstack_quant.core.dates.DayCount` or its serde name
///   (`"act_360"`, `"act_365f"`, `"30_360"`, …).
/// * `what` - Parameter name used in the error message.
pub(crate) fn day_count_from_py(
    obj: &Bound<'_, PyAny>,
    what: &str,
) -> PyResult<finstack_quant_core::dates::DayCount> {
    if let Ok(day_count) = obj.cast::<crate::bindings::core::dates::daycount::PyDayCount>() {
        return Ok(day_count.borrow().inner);
    }
    if let Ok(text) = obj.extract::<std::borrow::Cow<'_, str>>() {
        return enum_from_str(&text, what);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(format!(
        "{what}: expected finstack_quant.core.dates.DayCount or its string name, got {}",
        obj.get_type().name()?
    )))
}

/// Coerce a `(date, Money)` pair (an upfront payment quote) to Rust.
///
/// # Arguments
///
/// * `obj` - A two-element tuple/list `(payment_date, amount)` where the
///   date is any value accepted by `py_to_date` and the amount is a
///   `Money` (or a bare number when `currency` is given).
/// * `currency` - ISO-4217 code applied when the amount is a bare number.
/// * `what` - Parameter name used in error messages.
pub(crate) fn dated_money_from_py(
    obj: &Bound<'_, PyAny>,
    currency: Option<&str>,
    what: &str,
) -> PyResult<(
    finstack_quant_core::dates::Date,
    finstack_quant_core::money::Money,
)> {
    let type_name = obj.get_type().name()?.to_string();
    let (date_obj, money_obj): (Bound<'_, PyAny>, Bound<'_, PyAny>) =
        obj.extract().map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err(format!(
                "{what}: expected a (date, Money) pair, got {type_name}"
            ))
        })?;
    let date = crate::bindings::date_utils::py_to_date(&date_obj)?;
    let money = money_from_py(&money_obj, currency, what)?;
    Ok((date, money))
}

/// Render a Rust date as the Python `datetime.date(y, m, d)` literal.
///
/// # Arguments
///
/// * `date` - Calendar date to render.
pub(crate) fn date_repr(date: finstack_quant_core::dates::Date) -> String {
    format!(
        "datetime.date({}, {}, {})",
        date.year(),
        date.month() as u8,
        date.day()
    )
}

/// Render `Money` the way `Money.__repr__` does (`Money(100.0, 'USD')`).
///
/// # Arguments
///
/// * `value` - Amount to render; the currency code is single-quoted.
pub(crate) fn money_repr(value: finstack_quant_core::money::Money) -> String {
    format!("Money({}, '{}')", value.amount_decimal(), value.currency())
}

/// Render an `f64` the way Python's `repr(float)` does for integral values
/// (`1.0`, not `1`).
///
/// # Arguments
///
/// * `value` - Number to render.
pub(crate) fn float_repr(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e16 {
        format!("{value:.1}")
    } else {
        format!("{value}")
    }
}

/// Render `name(field=value, ...)` for a builder `__repr__`.
///
/// # Arguments
///
/// * `name` - Python class name of the builder.
/// * `fields` - `(field, python_repr)` pairs in the order they were set.
pub(crate) fn builder_repr(name: &str, fields: &[(&'static str, String)]) -> String {
    let body = fields
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}({body})")
}

/// Consume a wrapped builder, returning ValueError when build already consumed it.
///
/// # Arguments
///
/// * `inner` - Stored builder removed on success; an empty wrapper remains consumed.
pub(crate) fn take_builder<B>(inner: &mut Option<B>) -> PyResult<B> {
    inner
        .take()
        .ok_or_else(|| value_error("builder already consumed by build()"))
}

/// Coerce `Sequence[P | dict] | JSON str` to a list of Rust rows.
///
/// A `str` is parsed as a JSON array; any other iterable is walked item by
/// item, a typed `P` handing over its Rust row through `inner` and any other
/// item going through the serde round trip of
/// [`py_to_serde`](crate::bindings::module_utils::py_to_serde). Typed rows and
/// dicts may be mixed.
///
/// # Arguments
///
/// * `py` - GIL token used for the serde round trip.
/// * `value` - The Python argument.
/// * `label` - Parameter name used in error messages.
/// * `inner` - Extracts the Rust row from a typed `P` wrapper.
///
/// # Errors
///
/// `ValueError` when `value` is neither a string nor iterable, or the JSON
/// text or an item does not match the row shape.
pub(crate) fn typed_list_from_py<P, T>(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
    label: &str,
    inner: impl Fn(&P) -> T,
) -> PyResult<Vec<T>>
where
    P: pyo3::PyClass,
    T: serde::de::DeserializeOwned + Send,
{
    if let Ok(json) = value.extract::<&str>() {
        return serde_json::from_str(json)
            .map_err(|err| crate::errors::serde_json_to_py(err, &format!("invalid {label} JSON")));
    }
    let items = value.try_iter().map_err(|_| {
        let class = P::type_object(py)
            .name()
            .map_or_else(|_| "typed".to_string(), |name| name.to_string());
        let got = value
            .get_type()
            .name()
            .map_or_else(|_| "object".to_string(), |name| name.to_string());
        value_error(format!(
            "{label}: expected a sequence of {class} objects or dicts, or a JSON array string, got {got}"
        ))
    })?;
    items
        .map(|item| {
            let item = item?;
            match item.cast::<P>() {
                Ok(typed) => Ok(inner(&typed.borrow())),
                Err(_) => crate::bindings::module_utils::py_to_serde(py, &item, label),
            }
        })
        .collect()
}

/// Parse a canonical typed-instrument envelope through the shared Rust path.
pub(crate) fn parse_typed_instrument_json<T>(json: &str) -> PyResult<T>
where
    T: TryFrom<InstrumentJson, Error = finstack_quant_core::Error>,
{
    finstack_quant_valuations::pricer::parse_typed_instrument_json(json).map_err(core_to_py)
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
    finstack_quant_core::decimal::f64_to_decimal(value)
        .map_err(|err| value_error(format!("invalid {what}: {err}")))
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

/// Coerce `float | int | Rate | Bps` to a core `Rate` (decimal units).
pub(crate) fn rate_from_py(
    obj: &Bound<'_, PyAny>,
    what: &str,
) -> PyResult<finstack_quant_core::types::Rate> {
    extract_rate_decimal(obj, what).and_then(|value| {
        finstack_quant_core::types::Rate::from_decimal(value).map_err(crate::errors::core_to_py)
    })
}

/// Coerce `float | int | Bps | Rate` to a core `Bps`.
///
/// The basis points go through the Rust `Bps::try_new`, which rejects
/// non-finite, fractional and out-of-range basis points (`ValueError`),
/// exactly as WASM `Bps` does; a `Bps` object round-trips unchanged.
pub(crate) fn bps_value_from_py(
    obj: &Bound<'_, PyAny>,
    what: &str,
) -> PyResult<finstack_quant_core::types::Bps> {
    extract_basis_points(obj, what)
        .and_then(|bp| finstack_quant_core::types::Bps::try_new(bp).map_err(core_to_py))
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
