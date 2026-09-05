//! Shared Python → Rust value coercions for the valuations bindings.
//!
//! Every typed instrument, leg spec and builder in this domain accepts the
//! same loose input forms, so the rules live here rather than being restated
//! per class:
//!
//! - rates: `float | int | Rate` (decimal, `0.05` = 5%)
//! - spreads: `float | int | Bps` (basis points)
//! - money: `Money | float | int` (a bare number needs a currency from the caller)
//! - enums: serde string names, resolved through `enum_from_str`
//! - attributes: `Attributes | dict[str, str] | None`
//!
//! Outbound conversions (`enum_to_py_string`, `money_to_py`) are the exact
//! inverses so getters render what the constructor accepted.

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::bindings::core::money::PyMoney;
use crate::bindings::core::types::{PyAttributes, PyBps, PyRate};
use crate::errors::value_error;

/// Coerce `float | int | Rate` to a decimal rate (`0.05` = 5%).
///
/// # Arguments
///
/// * `obj` - A Python `float`/`int` already in decimal form, or a
///   `finstack_quant.core.types.Rate`.
/// * `what` - Parameter name used in the `TypeError` message.
pub(crate) fn rate_decimal_from_py(obj: &Bound<'_, PyAny>, what: &str) -> PyResult<f64> {
    if let Ok(rate) = obj.cast::<PyRate>() {
        return Ok(rate.borrow().inner.as_decimal());
    }
    if let Ok(value) = obj.extract::<f64>() {
        return Ok(value);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(format!(
        "{what}: expected a decimal float (0.05 = 5%) or finstack_quant.core.types.Rate, got {}",
        obj.get_type().name()?
    )))
}

/// Coerce `float | int | Bps` to a basis-point value (`25.0` = 25bp).
///
/// # Arguments
///
/// * `obj` - A Python `float`/`int` already in basis points, or a
///   `finstack_quant.core.types.Bps`.
/// * `what` - Parameter name used in the `TypeError` message.
pub(crate) fn bps_from_py(obj: &Bound<'_, PyAny>, what: &str) -> PyResult<f64> {
    if let Ok(bps) = obj.cast::<PyBps>() {
        return Ok(bps.borrow().inner.as_decimal() * 10_000.0);
    }
    if let Ok(value) = obj.extract::<f64>() {
        return Ok(value);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(format!(
        "{what}: expected basis points as float or finstack_quant.core.types.Bps, got {}",
        obj.get_type().name()?
    )))
}

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

/// Coerce `Attributes | dict[str, str] | None` to a Rust attribute bag.
///
/// A `dict` populates `meta` (string keys and values); an optional `"tags"`
/// entry holding a list of strings populates `tags`.
///
/// # Arguments
///
/// * `obj` - `finstack_quant.core.types.Attributes`, a `dict`, or `None`.
pub(crate) fn attributes_from_py(
    obj: &Bound<'_, PyAny>,
) -> PyResult<finstack_quant_core::types::Attributes> {
    if obj.is_none() {
        return Ok(finstack_quant_core::types::Attributes::new());
    }
    if let Ok(attrs) = obj.cast::<PyAttributes>() {
        return Ok(attrs.borrow().inner.clone());
    }
    let Ok(dict) = obj.cast::<PyDict>() else {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "attributes: expected finstack_quant.core.types.Attributes or dict[str, str], got {}",
            obj.get_type().name()?
        )));
    };
    let mut attrs = finstack_quant_core::types::Attributes::new();
    for (key, value) in dict.iter() {
        let key: String = key.extract()?;
        if key == "tags" {
            let tags: Vec<String> = value.extract()?;
            for tag in tags {
                attrs.tags.insert(tag);
            }
            continue;
        }
        let value: String = value.str()?.extract()?;
        attrs.set_meta(&key, &value);
    }
    Ok(attrs)
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

/// Coerce `Currency | str` to a [`Currency`](finstack_quant_core::currency::Currency).
///
/// # Arguments
///
/// * `obj` - A `finstack_quant.core.currency.Currency` or an ISO-4217 code
///   such as `"USD"`.
/// * `what` - Parameter name used in the `TypeError` message.
pub(crate) fn currency_from_py(
    obj: &Bound<'_, PyAny>,
    what: &str,
) -> PyResult<finstack_quant_core::currency::Currency> {
    if let Ok(ccy) = obj.cast::<crate::bindings::core::currency::PyCurrency>() {
        return Ok(ccy.borrow().inner);
    }
    if let Ok(code) = obj.extract::<std::borrow::Cow<'_, str>>() {
        return crate::bindings::module_utils::parse_currency(&code);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(format!(
        "{what}: expected finstack_quant.core.currency.Currency or an ISO-4217 code, got {}",
        obj.get_type().name()?
    )))
}

/// Coerce `Tenor | str` to a [`Tenor`](finstack_quant_core::dates::Tenor).
///
/// # Arguments
///
/// * `obj` - A `finstack_quant.core.dates.Tenor` or a tenor string such as
///   `"3M"` / `"6M"` / `"1Y"`.
/// * `what` - Parameter name used in the error message.
pub(crate) fn tenor_from_py(
    obj: &Bound<'_, PyAny>,
    what: &str,
) -> PyResult<finstack_quant_core::dates::Tenor> {
    if let Ok(tenor) = obj.cast::<crate::bindings::core::dates::tenor::PyTenor>() {
        return Ok(tenor.borrow().inner);
    }
    if let Ok(text) = obj.extract::<std::borrow::Cow<'_, str>>() {
        return finstack_quant_core::dates::Tenor::parse(&text).map_err(crate::errors::core_to_py);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(format!(
        "{what}: expected finstack_quant.core.dates.Tenor or a tenor string such as \"3M\", got {}",
        obj.get_type().name()?
    )))
}

/// Coerce `BusinessDayConvention | str` to a Rust business-day convention.
///
/// # Arguments
///
/// * `obj` - A `finstack_quant.core.dates.BusinessDayConvention` or its
///   serde name (`"modified_following"`, `"following"`, `"preceding"`, …).
/// * `what` - Parameter name used in the error message.
pub(crate) fn bdc_from_py(
    obj: &Bound<'_, PyAny>,
    what: &str,
) -> PyResult<finstack_quant_core::dates::BusinessDayConvention> {
    if let Ok(convention) = obj.cast::<crate::bindings::core::dates::calendar::PyBusinessDayConvention>() {
        return Ok(convention.borrow().inner);
    }
    if let Ok(text) = obj.extract::<std::borrow::Cow<'_, str>>() {
        return crate::bindings::valuations::instruments::enum_from_str(&text, what);
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
        return crate::bindings::valuations::instruments::enum_from_str(&text, what);
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
///   date is any value accepted by `extract_date` and the amount is a
///   `Money` (or a bare number when `currency` is given).
/// * `currency` - ISO-4217 code applied when the amount is a bare number.
/// * `what` - Parameter name used in error messages.
pub(crate) fn dated_money_from_py(
    obj: &Bound<'_, PyAny>,
    currency: Option<&str>,
    what: &str,
) -> PyResult<(time::Date, finstack_quant_core::money::Money)> {
    let type_name = obj.get_type().name()?.to_string();
    let (date_obj, money_obj): (Bound<'_, PyAny>, Bound<'_, PyAny>) =
        obj.extract().map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err(format!(
                "{what}: expected a (date, Money) pair, got {type_name}"
            ))
        })?;
    let date = crate::bindings::date_utils::extract_date(&date_obj)?;
    let money = money_from_py(&money_obj, currency, what)?;
    Ok((date, money))
}

/// Render a Rust date as the Python `datetime.date(y, m, d)` literal.
///
/// # Arguments
///
/// * `date` - Calendar date to render.
pub(crate) fn date_repr(date: time::Date) -> String {
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
