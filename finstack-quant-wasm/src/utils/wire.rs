//! Conversion of wire-shaped JavaScript values into Rust boundary types.
//!
//! A boundary data type (spec, config, result, enum) crosses as the value its
//! serde contract describes: a plain object or array, or a bare string for
//! string-valued types such as a day count (`"act_360"`), a currency (`"USD"`)
//! or a unit enum variant (`"cash"`). [`js_wire`] accepts that value, or the
//! same document as JSON text, and always parses through `serde_json`, so the
//! Rust type's `deny_unknown_fields` and validating `Deserialize` apply.

use finstack_quant_core::dates::Date;
use finstack_quant_core::wire::DecimalWire;
use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::JsValue;

use super::input::{from_js_json, js_string};
use super::to_js_err;

/// A wire value of type `T`: a plain object/array, a bare wire string, or
/// JSON text.
///
/// A JavaScript string that does not start a JSON object, array or string is
/// taken as the wire string itself (`"act_360"`); any other input goes through
/// [`from_js_json`].
///
/// # Arguments
///
/// * `value` - The JavaScript argument.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) as [`from_js_json`] does, and
/// a validation error naming `label` when the value does not match `T`.
pub fn js_wire<T: DeserializeOwned>(value: &JsValue, label: &str) -> Result<T, JsValue> {
    if let Some(text) = value.as_string() {
        if !text.trim_start().starts_with(['{', '[', '"']) {
            return serde_json::from_value(serde_json::Value::String(text)).map_err(|error| {
                to_js_err(finstack_quant_core::Error::Validation(format!(
                    "{label}: {error}"
                )))
            });
        }
    }
    from_js_json(value, label)
}

/// An optional wire value: `undefined` and `null` are `None`.
///
/// # Arguments
///
/// * `value` - The JavaScript argument, if supplied.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws as [`js_wire`] does for a supplied value.
pub fn js_opt_wire<T: DeserializeOwned>(
    value: Option<&JsValue>,
    label: &str,
) -> Result<Option<T>, JsValue> {
    match value {
        Some(value) if !value.is_undefined() && !value.is_null() => js_wire(value, label).map(Some),
        _ => Ok(None),
    }
}

/// A string argument parsed by the Rust type's `FromStr`.
///
/// For types whose text parser accepts more than their serde names (currency
/// codes in any case, day-count aliases such as `"ACT/360"`, rating notches),
/// so [`js_wire`] would narrow the accepted input.
///
/// # Arguments
///
/// * `value` - The JavaScript argument.
/// * `label` - The camelCase argument name, used in the type-error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when `value` is not a string,
/// and the mapped Rust parse error when `FromStr` rejects it.
pub fn js_parse<T>(value: &JsValue, label: &str) -> Result<T, JsValue>
where
    T: std::str::FromStr,
    T::Err: super::IntoJsError,
{
    js_string(value, label)?.parse::<T>().map_err(to_js_err)
}

/// An optional [`js_parse`] argument: `undefined` and `null` are `None`.
///
/// # Arguments
///
/// * `value` - The JavaScript argument, if supplied.
/// * `label` - The camelCase argument name, used in the type-error message.
///
/// # Errors
///
/// Throws as [`js_parse`] does for a supplied value.
pub fn js_opt_parse<T>(value: Option<&JsValue>, label: &str) -> Result<Option<T>, JsValue>
where
    T: std::str::FromStr,
    T::Err: super::IntoJsError,
{
    match value {
        Some(value) if !value.is_undefined() && !value.is_null() => {
            js_parse(value, label).map(Some)
        }
        _ => Ok(None),
    }
}

/// A required ISO-8601 calendar date string (`"YYYY-MM-DD"`).
///
/// # Arguments
///
/// * `value` - The JavaScript argument.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when `value` is not a string
/// and a validation error when it is not a strict ISO date.
pub fn js_date(value: &JsValue, label: &str) -> Result<Date, JsValue> {
    super::parse_iso_date(&js_string(value, label)?)
}

/// An exact decimal carried as a string (`"0.0425"`), as on the wire.
///
/// The decimal is field `0` of the returned [`DecimalWire`].
///
/// # Arguments
///
/// * `value` - The JavaScript argument.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when `value` is not a string
/// and a validation error naming `label` when it is not a decimal number.
pub fn js_decimal(value: &JsValue, label: &str) -> Result<DecimalWire, JsValue> {
    let text = js_string(value, label)?;
    serde_json::from_value(serde_json::Value::String(text)).map_err(|error| {
        to_js_err(finstack_quant_core::Error::Validation(format!(
            "{label}: {error}"
        )))
    })
}

/// Parse a serde wire label (`"total_variance"`) into a unit enum (Rust
/// `wire::serde_parse`).
///
/// # Arguments
///
/// * `label` - The wire label, already read from the JavaScript argument.
///
/// # Errors
///
/// Throws a validation error naming the accepted labels when `label` is not
/// one of them.
pub fn parse_label<T: DeserializeOwned>(label: &str) -> Result<T, JsValue> {
    finstack_quant_core::wire::serde_parse(label).map_err(to_js_err)
}

/// The serde wire label of a unit enum value (Rust `wire::serde_label`).
///
/// # Arguments
///
/// * `value` - The enum value to render.
///
/// # Errors
///
/// Throws when `value` does not serialize to a bare string (not expected for
/// a unit enum).
pub fn label<T: Serialize>(value: &T) -> Result<String, JsValue> {
    finstack_quant_core::wire::serde_label(value).map_err(to_js_err)
}
