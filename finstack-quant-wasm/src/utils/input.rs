//! Strict conversion of JavaScript arguments at the WASM boundary.
//!
//! wasm-bindgen's primitive parameter glue checks nothing at runtime: a `&str`
//! parameter traps the instance (and leaks) when handed a non-string, integer
//! parameters go through `ToInt32` (so `"2025-01-15"`, `NaN` and `1.5` become
//! `0` or `1`), `bool` goes through truthiness, `f64` through `ToNumber` (so
//! `null` becomes `0` and `"5"` becomes `5`), and `&[f64]`/`Vec<f64>` through
//! typed-array glue (a string becomes `NaN`s, `{}` becomes `[]`). Bindings
//! therefore take `JsValue` and convert here, so a wrong argument throws a
//! `TypeError` with `kind: "invalid_type"` instead.
//!
//! Structured JSON inputs go through [`json_text`]: a JSON string is used as
//! is, and a plain object or array is written to canonical JSON text by a
//! strict walker, then parsed by `serde_json` like any other JSON input, so
//! every `#[serde(deny_unknown_fields)]` in the Rust types applies. The walker
//! keeps object key order, writes integral numbers without a fraction,
//! writes `BigInt` values as exact digits, drops `undefined` object
//! properties (as `JSON.stringify` does) and rejects everything JSON cannot
//! represent exactly: non-finite numbers, array holes, functions, symbols and
//! non-plain objects such as `Map`, `Date` or wasm-bindgen handles.

use js_sys::{Array, Object};
use serde::de::DeserializeOwned;
use time::Date;
use wasm_bindgen::{JsCast, JsValue};

/// Largest integer a JavaScript `number` represents exactly (`2^53 - 1`).
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// Nesting depth beyond which a structured input is rejected.
const MAX_DEPTH: usize = 256;

/// Build the `TypeError` (with `kind: "invalid_type"`) thrown for a wrong
/// argument type or shape.
///
/// # Arguments
///
/// * `label` - The camelCase argument name as documented in `index.d.ts`.
/// * `problem` - What was expected and what was found.
pub fn invalid_type(label: &str, problem: &str) -> JsValue {
    let message = format!("{label}: {problem}");
    #[cfg(target_arch = "wasm32")]
    {
        let error = js_sys::TypeError::new(&message);
        let _ = js_sys::Reflect::set(
            &error,
            &JsValue::from("kind"),
            &JsValue::from("invalid_type"),
        );
        error.into()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = message;
        JsValue::NULL
    }
}

/// A required string argument.
///
/// # Arguments
///
/// * `value` - The JavaScript argument.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when `value` is not a string.
pub fn js_string(value: &JsValue, label: &str) -> Result<String, JsValue> {
    value.as_string().ok_or_else(|| {
        invalid_type(
            label,
            &format!("expected a string, got {}", type_name(value)),
        )
    })
}

/// An optional string argument; `null` and `undefined` mean absent.
///
/// # Arguments
///
/// * `value` - The JavaScript argument.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when `value` is present but
/// not a string.
pub fn js_opt_string(value: Option<&JsValue>, label: &str) -> Result<Option<String>, JsValue> {
    match value {
        Some(v) if !(v.is_null() || v.is_undefined()) => js_string(v, label).map(Some),
        _ => Ok(None),
    }
}

/// A required boolean argument (no truthiness coercion).
///
/// # Arguments
///
/// * `value` - The JavaScript argument.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when `value` is not a boolean.
pub fn js_bool(value: &JsValue, label: &str) -> Result<bool, JsValue> {
    value.as_bool().ok_or_else(|| {
        invalid_type(
            label,
            &format!("expected a boolean, got {}", type_name(value)),
        )
    })
}

/// An optional boolean argument; `null` and `undefined` mean absent.
///
/// # Arguments
///
/// * `value` - The JavaScript argument.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when `value` is present but
/// not a boolean.
pub fn js_opt_bool(value: Option<&JsValue>, label: &str) -> Result<Option<bool>, JsValue> {
    match value {
        Some(v) if !(v.is_null() || v.is_undefined()) => js_bool(v, label).map(Some),
        _ => Ok(None),
    }
}

/// A required `number` argument (no `ToNumber` coercion).
///
/// `NaN` and `±Infinity` are numbers and pass through unchanged: the Rust
/// function being called owns finiteness validation, as it does for Python
/// callers passing `float('nan')`.
///
/// # Arguments
///
/// * `value` - The JavaScript argument.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when `value` is not a
/// `number` (`null`, `undefined`, strings, booleans, `BigInt`, arrays and
/// objects all throw).
pub fn js_f64(value: &JsValue, label: &str) -> Result<f64, JsValue> {
    value.as_f64().ok_or_else(|| {
        invalid_type(
            label,
            &format!("expected a number, got {}", type_name(value)),
        )
    })
}

/// An optional `number` argument; `null` and `undefined` mean absent.
///
/// # Arguments
///
/// * `value` - The JavaScript argument, if supplied.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// As [`js_f64`], for a present value.
pub fn js_opt_f64(value: Option<&JsValue>, label: &str) -> Result<Option<f64>, JsValue> {
    match value {
        Some(v) if !(v.is_null() || v.is_undefined()) => js_f64(v, label).map(Some),
        _ => Ok(None),
    }
}

/// A non-negative integer argument.
///
/// # Arguments
///
/// * `value` - The JavaScript argument; must be a `number`.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when `value` is not a finite
/// whole `number` in `0..=2^53 - 1` or does not fit `T`.
pub fn js_uint<T: TryFrom<u64>>(value: &JsValue, label: &str) -> Result<T, JsValue> {
    let number = js_f64(value, label)?;
    if !number.is_finite() || number.fract() != 0.0 || !(0.0..=MAX_SAFE_INTEGER).contains(&number) {
        return Err(invalid_type(
            label,
            &format!("expected a non-negative whole number, got {number}"),
        ));
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let whole = number as u64;
    T::try_from(whole).map_err(|_| invalid_type(label, &format!("{number} is out of range")))
}

/// An optional non-negative integer argument; `null` and `undefined` mean absent.
///
/// # Arguments
///
/// * `value` - The JavaScript argument, if supplied.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// As [`js_uint`], for a present value.
pub fn js_opt_uint<T: TryFrom<u64>>(
    value: Option<&JsValue>,
    label: &str,
) -> Result<Option<T>, JsValue> {
    match value {
        Some(v) if !(v.is_null() || v.is_undefined()) => js_uint(v, label).map(Some),
        _ => Ok(None),
    }
}

/// A signed integer argument.
///
/// # Arguments
///
/// * `value` - The JavaScript argument; must be a `number`.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when `value` is not a finite
/// whole `number` within `±(2^53 - 1)` or does not fit `T`.
pub fn js_int<T: TryFrom<i64>>(value: &JsValue, label: &str) -> Result<T, JsValue> {
    let number = js_f64(value, label)?;
    if !number.is_finite() || number.fract() != 0.0 || number.abs() > MAX_SAFE_INTEGER {
        return Err(invalid_type(
            label,
            &format!("expected a whole number, got {number}"),
        ));
    }
    #[allow(clippy::cast_possible_truncation)]
    let whole = number as i64;
    T::try_from(whole).map_err(|_| invalid_type(label, &format!("{number} is out of range")))
}

/// An optional signed integer argument; `null` and `undefined` mean absent.
///
/// # Arguments
///
/// * `value` - The JavaScript argument, if supplied.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// As [`js_int`], for a present value.
pub fn js_opt_int<T: TryFrom<i64>>(
    value: Option<&JsValue>,
    label: &str,
) -> Result<Option<T>, JsValue> {
    match value {
        Some(v) if !(v.is_null() || v.is_undefined()) => js_int(v, label).map(Some),
        _ => Ok(None),
    }
}

/// An unsigned 64-bit integer argument: a safe-integer `number` or a `BigInt`.
///
/// # Arguments
///
/// * `value` - The JavaScript argument.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when `value` is neither a
/// non-negative safe integer nor a `BigInt` in `0..=2^64 - 1`.
pub fn js_u64(value: &JsValue, label: &str) -> Result<u64, JsValue> {
    if value.as_f64().is_some() {
        return js_uint(value, label);
    }
    if value.is_bigint() {
        let digits = bigint_digits(value).unwrap_or_default();
        return digits.parse::<u64>().map_err(|_| {
            invalid_type(
                label,
                &format!("{digits} is not an unsigned 64-bit integer"),
            )
        });
    }
    Err(invalid_type(
        label,
        &format!("expected a number or BigInt, got {}", type_name(value)),
    ))
}

/// An optional unsigned 64-bit integer argument; `null` and `undefined` mean absent.
///
/// # Arguments
///
/// * `value` - The JavaScript argument, if supplied.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// As [`js_u64`], for a present value.
pub fn js_opt_u64(value: Option<&JsValue>, label: &str) -> Result<Option<u64>, JsValue> {
    match value {
        Some(v) if !(v.is_null() || v.is_undefined()) => js_u64(v, label).map(Some),
        _ => Ok(None),
    }
}

/// A calendar date given as integer epoch days (days since 1970-01-01).
///
/// # Arguments
///
/// * `value` - The JavaScript argument; must be a whole `number`.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when `value` is not a whole
/// `number` (an ISO string, `NaN` or a fraction), and a validation error when
/// it is outside the supported date range.
pub fn js_epoch_days(value: &JsValue, label: &str) -> Result<Date, JsValue> {
    let days: i32 = js_int(value, label)?;
    finstack_quant_core::dates::date_from_epoch_days(days).ok_or_else(|| {
        super::to_js_err(format!(
            "{label}: {days} epoch days is outside the supported date range"
        ))
    })
}

/// An optional array of strings; `null` and `undefined` mean absent.
///
/// # Arguments
///
/// * `value` - The JavaScript argument, if supplied.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when a present value is not
/// an array or holds a non-string item.
pub fn js_opt_string_seq(
    value: Option<&JsValue>,
    label: &str,
) -> Result<Option<Vec<String>>, JsValue> {
    match value {
        Some(v) if !(v.is_null() || v.is_undefined()) => js_string_seq(v, label).map(Some),
        _ => Ok(None),
    }
}

/// A required array of strings.
///
/// # Arguments
///
/// * `value` - The JavaScript argument.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when `value` is not an array
/// or holds a non-string item.
pub fn js_string_seq(value: &JsValue, label: &str) -> Result<Vec<String>, JsValue> {
    sequence(value, label, "strings")?
        .iter()
        .enumerate()
        .map(|(index, item)| js_string(&item, &format!("{label}[{index}]")))
        .collect()
}

/// JSON text for a structured argument: a JSON string as is, or a plain
/// object/array written to JSON by the strict walker described in the module
/// docs.
///
/// # Arguments
///
/// * `value` - The JavaScript argument.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when `value` is neither a
/// string nor a plain object/array, or when it contains a value JSON cannot
/// represent exactly.
pub fn json_text(value: &JsValue, label: &str) -> Result<String, JsValue> {
    if let Some(text) = value.as_string() {
        return Ok(text);
    }
    if !value.is_object() {
        return Err(invalid_type(
            label,
            &format!("expected a JSON string or object, got {}", type_name(value)),
        ));
    }
    let mut out = String::new();
    write_json(value, &mut out, label, 0).map_err(|problem| invalid_type(label, &problem))?;
    Ok(out)
}

/// Any JSON-representable JavaScript value as a `serde_json::Value`.
///
/// Unlike [`json_text`], a string argument is a JSON *string value*, not JSON
/// text to parse; use this where the argument is itself the value to store.
///
/// # Arguments
///
/// * `value` - The JavaScript argument: `null`, a boolean, finite number,
///   string, BigInt, array, typed array or plain object.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when `value` (or anything
/// nested in it) is `undefined`, non-finite, or not JSON-representable.
pub fn js_json_value(value: &JsValue, label: &str) -> Result<serde_json::Value, JsValue> {
    let mut out = String::new();
    write_json(value, &mut out, label, 0).map_err(|problem| invalid_type(label, &problem))?;
    serde_json::from_str(&out).map_err(|error| {
        super::to_js_err(finstack_quant_core::Error::Validation(format!(
            "{label}: {error}"
        )))
    })
}

/// Deserialize a structured argument (JSON string or plain object/array)
/// through `serde_json`, so the target type's `deny_unknown_fields` and
/// validating `Deserialize` impls apply exactly as for JSON text.
///
/// # Arguments
///
/// * `value` - The JavaScript argument.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) as [`json_text`] does, and a
/// validation error naming `label` when the JSON does not match `T`.
pub fn from_js_json<T: DeserializeOwned>(value: &JsValue, label: &str) -> Result<T, JsValue> {
    let text = json_text(value, label)?;
    serde_json::from_str(&text).map_err(|error| {
        super::to_js_err(finstack_quant_core::Error::Validation(format!(
            "{label}: {error}"
        )))
    })
}

/// A sequence of numbers: an array of `number`s or a numeric typed array.
///
/// A plain array's items are type-checked, not `ToNumber`-coerced. A
/// `Float64Array` is copied in bulk without per-item checks and is the fast
/// path for large inputs; a plain array of finite numbers is checked with one
/// `Array.prototype.every(Number.isFinite)` call and copied in bulk, and any
/// other array is checked item by item.
///
/// # Arguments
///
/// * `value` - The JavaScript argument.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when `value` is not an array
/// or typed array, or holds a non-number item. `NaN` items are numbers and
/// pass through for Rust to validate.
pub fn js_f64_seq(value: &JsValue, label: &str) -> Result<Vec<f64>, JsValue> {
    if let Some(typed) = value.dyn_ref::<js_sys::Float64Array>() {
        return Ok(typed.to_vec());
    }
    if let Some(array) = value.dyn_ref::<Array>() {
        if all_finite_numbers(array) {
            let values = js_sys::Float64Array::new(array).to_vec();
            // `every` skips holes, which the copy turns into NaN; those arrays
            // take the per-item path and throw there.
            if !values.iter().any(|v| v.is_nan()) {
                return Ok(values);
            }
        }
    }
    sequence(value, label, "numbers")?
        .iter()
        .enumerate()
        .map(|(index, item)| {
            item.as_f64().ok_or_else(|| {
                invalid_type(
                    &format!("{label}[{index}]"),
                    &format!("expected a number, got {}", type_name(&item)),
                )
            })
        })
        .collect()
}

/// An optional numeric sequence argument; `null` and `undefined` mean absent.
///
/// # Arguments
///
/// * `value` - The JavaScript argument, if supplied; an array of numbers or a
///   `Float64Array`.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// As [`js_f64_seq`], for a present value.
pub fn js_opt_f64_seq(value: Option<&JsValue>, label: &str) -> Result<Option<Vec<f64>>, JsValue> {
    match value {
        Some(v) if !(v.is_null() || v.is_undefined()) => js_f64_seq(v, label).map(Some),
        _ => Ok(None),
    }
}

/// A sequence of optional numbers: `null` and `undefined` items are missing
/// values (`None`); every other item must be a `number`.
///
/// # Arguments
///
/// * `value` - The JavaScript argument; an array or numeric typed array.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when `value` is not an array
/// or typed array, or holds an item that is neither a number nor missing.
pub fn js_nullable_f64_seq(value: &JsValue, label: &str) -> Result<Vec<Option<f64>>, JsValue> {
    if let Some(typed) = value.dyn_ref::<js_sys::Float64Array>() {
        return Ok(typed.to_vec().into_iter().map(Some).collect());
    }
    sequence(value, label, "numbers or nulls")?
        .iter()
        .enumerate()
        .map(|(index, item)| {
            if item.is_null() || item.is_undefined() {
                return Ok(None);
            }
            item.as_f64().map(Some).ok_or_else(|| {
                invalid_type(
                    &format!("{label}[{index}]"),
                    &format!("expected a number or null, got {}", type_name(&item)),
                )
            })
        })
        .collect()
}

/// A matrix of numbers: an array of rows, each accepted by [`js_f64_seq`].
///
/// # Arguments
///
/// * `value` - The JavaScript argument.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when `value` or a row is not
/// an array or typed array, or a row holds a non-number item.
pub fn js_f64_matrix(value: &JsValue, label: &str) -> Result<Vec<Vec<f64>>, JsValue> {
    sequence(value, label, "rows")?
        .iter()
        .enumerate()
        .map(|(index, row)| js_f64_seq(&row, &format!("{label}[{index}]")))
        .collect()
}

/// A matrix of optional numbers: an array of rows, each accepted by
/// [`js_nullable_f64_seq`].
///
/// # Arguments
///
/// * `value` - The JavaScript argument.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) when `value` or a row is not
/// an array or typed array, or a row holds an item that is neither a number
/// nor missing.
pub fn js_nullable_f64_matrix(
    value: &JsValue,
    label: &str,
) -> Result<Vec<Vec<Option<f64>>>, JsValue> {
    sequence(value, label, "rows")?
        .iter()
        .enumerate()
        .map(|(index, row)| js_nullable_f64_seq(&row, &format!("{label}[{index}]")))
        .collect()
}

/// Optional JSON text; `null` and `undefined` mean absent.
///
/// # Arguments
///
/// * `value` - The JavaScript argument.
/// * `label` - The camelCase argument name, used in the error message.
///
/// # Errors
///
/// As [`json_text`], for a present value.
pub fn opt_json_text(value: Option<&JsValue>, label: &str) -> Result<Option<String>, JsValue> {
    match value {
        Some(v) if !(v.is_null() || v.is_undefined()) => json_text(v, label).map(Some),
        _ => Ok(None),
    }
}

/// The items of an array or typed-array argument.
fn sequence(value: &JsValue, label: &str, items: &str) -> Result<Array, JsValue> {
    if Array::is_array(value) || js_sys::ArrayBuffer::is_view(value) {
        return Ok(Array::from(value));
    }
    Err(invalid_type(
        label,
        &format!("expected an array of {items}, got {}", type_name(value)),
    ))
}

/// Whether every item of `array` is a finite `number`, checked in one call to
/// `Array.prototype.every` with `Number.isFinite` (which never coerces).
///
/// This is the fast path of [`js_f64_seq`] for plain arrays: when it holds,
/// the items are copied in bulk through a `Float64Array`, whose `ToNumber`
/// conversion is then the identity. Arrays with `NaN`, `±Infinity` or a
/// non-number item take the per-item path, which accepts the non-finite
/// numbers and names the first bad item.
fn all_finite_numbers(array: &Array) -> bool {
    thread_local! {
        static EVERY_FINITE: Option<(js_sys::Function, js_sys::Function)> = {
            let every = js_sys::Reflect::get(&Array::new(), &JsValue::from("every")).ok();
            let number = js_sys::Reflect::get(&js_sys::global(), &JsValue::from("Number")).ok();
            let is_finite =
                number.and_then(|n| js_sys::Reflect::get(&n, &JsValue::from("isFinite")).ok());
            match (every, is_finite) {
                (Some(every), Some(is_finite)) => Some((every.unchecked_into(), is_finite.unchecked_into())),
                _ => None,
            }
        };
    }
    EVERY_FINITE.with(|functions| {
        functions.as_ref().is_some_and(|(every, is_finite)| {
            every
                .call1(array, is_finite)
                .is_ok_and(|result| result.as_bool() == Some(true))
        })
    })
}

fn bigint_digits(value: &JsValue) -> Option<String> {
    js_sys::BigInt::from(value.clone())
        .to_string(10)
        .ok()
        .and_then(|s| s.as_string())
}

fn type_name(value: &JsValue) -> String {
    value
        .js_typeof()
        .as_string()
        .unwrap_or_else(|| "unknown".to_string())
}

fn is_plain_object(value: &JsValue) -> bool {
    let Some(object) = value.dyn_ref::<Object>() else {
        return false;
    };
    let proto = Object::get_prototype_of(object);
    proto.is_null() || proto == Object::get_prototype_of(&Object::new())
}

fn write_number(number: f64, path: &str, out: &mut String) -> Result<(), String> {
    if !number.is_finite() {
        return Err(format!("{path} is {number}, which JSON cannot represent"));
    }
    if number.fract() == 0.0 && number.abs() <= MAX_SAFE_INTEGER {
        #[allow(clippy::cast_possible_truncation)]
        out.push_str(&(number as i64).to_string());
    } else {
        let json = serde_json::Number::from_f64(number)
            .ok_or_else(|| format!("{path} is not a finite number"))?;
        out.push_str(&json.to_string());
    }
    Ok(())
}

fn write_json(value: &JsValue, out: &mut String, path: &str, depth: usize) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!("{path} nests deeper than {MAX_DEPTH} levels"));
    }
    if value.is_null() {
        out.push_str("null");
    } else if let Some(flag) = value.as_bool() {
        out.push_str(if flag { "true" } else { "false" });
    } else if let Some(number) = value.as_f64() {
        write_number(number, path, out)?;
    } else if let Some(text) = value.as_string() {
        out.push_str(&serde_json::to_string(&text).map_err(|e| e.to_string())?);
    } else if value.is_bigint() {
        let digits = bigint_digits(value)
            .ok_or_else(|| format!("{path} is a BigInt that could not be formatted"))?;
        out.push_str(&digits);
    } else if Array::is_array(value) || js_sys::ArrayBuffer::is_view(value) {
        let items = Array::from(value);
        out.push('[');
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            let item_path = format!("{path}[{index}]");
            if item.is_undefined() {
                return Err(format!(
                    "{item_path} is undefined (array holes are not JSON)"
                ));
            }
            write_json(&item, out, &item_path, depth + 1)?;
        }
        out.push(']');
    } else if is_plain_object(value) {
        let object = Object::from(value.clone());
        out.push('{');
        let mut first = true;
        for key in Object::keys(&object).iter() {
            let name = key.as_string().unwrap_or_default();
            let item = js_sys::Reflect::get(&object, &key)
                .map_err(|_| format!("{path}.{name} could not be read"))?;
            if item.is_undefined() {
                continue;
            }
            if !first {
                out.push(',');
            }
            first = false;
            out.push_str(&serde_json::to_string(&name).map_err(|e| e.to_string())?);
            out.push(':');
            write_json(&item, out, &format!("{path}.{name}"), depth + 1)?;
        }
        out.push('}');
    } else {
        return Err(format!(
            "{path} is a {} that JSON cannot represent (expected a plain object, array, string, number, boolean, BigInt or null)",
            type_name(value)
        ));
    }
    Ok(())
}
