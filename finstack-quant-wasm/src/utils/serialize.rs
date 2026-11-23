//! Plain-object JSON/JS serialization and non-finite value restoration.

use super::error::to_js_err;
use finstack_quant_core::wire::{NonFiniteFields, non_finite_f64};
use wasm_bindgen::JsValue;

/// Serialize a value to a `JsValue` using JSON-compatible conventions.
///
/// Unlike `serde_wasm_bindgen::to_value`, which serializes Rust maps (and
/// `serde_json::Value::Object`) as ES2015 `Map`s, this helper uses
/// [`serde_wasm_bindgen::Serializer::json_compatible`] so maps become plain
/// JS objects — matching the declared TypeScript shapes and the dict
/// shapes returned by the Python bindings.
///
/// Key order: JavaScript enumerates map keys that are canonical array indices
/// (decimal integer strings `"0"`..`"4294967294"`, e.g. position or entity ids
/// like `"10"`) in ascending numeric order before all other keys. Iteration
/// order of such a map can therefore differ from the Rust `IndexMap`
/// insertion order that Python dicts preserve; keyed lookup is unaffected.
/// The object is not byte-identical to the Rust wire JSON either: integral
/// floats print as `1`, not `1.0`, under `JSON.stringify`.
///
/// # Errors
///
/// Returns a structured `JsValue` error if serialization fails.
pub fn to_js_value<T: serde::Serialize>(value: &T) -> Result<JsValue, JsValue> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(to_js_err)
}

/// Serialize a validated table while keeping nonfinite float64 results numeric.
///
/// # Arguments
///
/// * `table` - Typed table whose float64 columns may contain intentional
///   unavailable or zero-risk values; string columns retain their exact text.
///
/// # Errors
///
/// Returns a structured JavaScript error for invalid shape or failed property access.
pub(crate) fn to_js_table_numeric(
    table: &finstack_quant_core::table::TableEnvelope,
) -> Result<JsValue, JsValue> {
    use finstack_quant_core::table::TableColumnData;

    table.validate().map_err(to_js_err)?;
    let js = to_js_value(table)?;
    let columns = js_sys::Reflect::get(&js, &JsValue::from_str("columns"))?;
    let columns = js_sys::Array::from(&columns);
    for (index, column) in table.columns.iter().enumerate() {
        if let TableColumnData::Float64(values) = &column.data {
            let host_column = columns.get(index as u32);
            let data = js_sys::Reflect::get(&host_column, &JsValue::from_str("data"))?;
            let host_values = js_sys::Array::new_with_length(values.len() as u32);
            for (row, value) in values.iter().enumerate() {
                host_values.set(row as u32, JsValue::from_f64(*value));
            }
            js_sys::Reflect::set(&data, &JsValue::from_str("values"), &host_values)?;
        }
    }
    Ok(js)
}

/// Serialize a result whose Rust type names its non-finite fields, returning
/// those fields as JavaScript numbers.
///
/// The fields listed by [`NonFiniteFields`] serialize through
/// `core::wire::non_finite_f64`, which writes `±∞`/`NaN` as sentinel strings
/// so JSON can round-trip them. JavaScript numbers hold those values natively,
/// so the sentinels are decoded back into numbers here.
///
/// # Errors
///
/// Returns a structured `JsValue` error if serialization or property access
/// fails.
pub(crate) fn to_js_value_numeric<T: serde::Serialize + NonFiniteFields>(
    value: &T,
) -> Result<JsValue, JsValue> {
    let js = to_js_value(value)?;
    restore_non_finite::<T>(&js)?;
    Ok(js)
}

/// Serialize a slice of results as a JavaScript array, returning each row's
/// non-finite fields as numbers (see [`to_js_value_numeric`]).
///
/// # Errors
///
/// Returns a structured `JsValue` error if serialization or property access
/// fails.
pub(crate) fn to_js_rows_numeric<T: serde::Serialize + NonFiniteFields>(
    rows: &[T],
) -> Result<JsValue, JsValue> {
    let js = to_js_value(&rows)?;
    restore_non_finite_rows::<T>(&js)?;
    Ok(js)
}

/// Decode the sentinel strings in each element of an already serialized
/// array of `T` rows (for a result that nests `T` rows under one key).
///
/// # Errors
///
/// Returns the `Reflect` failure if a property cannot be read or written.
pub(crate) fn restore_non_finite_rows<T: NonFiniteFields>(rows: &JsValue) -> Result<(), JsValue> {
    let rows = js_sys::Array::from(rows);
    for row in rows.iter() {
        restore_non_finite::<T>(&row)?;
    }
    Ok(())
}

/// Replace each `T::NON_FINITE_FIELDS` sentinel string on `object` with the
/// number it encodes, using the Rust-owned sentinel vocabulary.
fn restore_non_finite<T: NonFiniteFields>(object: &JsValue) -> Result<(), JsValue> {
    for &field in T::NON_FINITE_FIELDS {
        let key = JsValue::from_str(field);
        let value = js_sys::Reflect::get(object, &key)?;
        if let Some(number) = value
            .as_string()
            .and_then(|text| non_finite_f64::parse_sentinel(&text))
        {
            js_sys::Reflect::set(object, &key, &JsValue::from_f64(number))?;
        }
    }
    Ok(())
}

/// Serialize a value to a JSON-compatible `JsValue` while preserving Rust
/// 64-bit integers as JavaScript `BigInt` values.
///
/// This is reserved for structured host results whose full-width integer
/// fields, such as Monte Carlo seeds, must remain lossless.
/// Map key order follows the same JavaScript rule as [`to_js_value`].
pub(crate) fn to_js_value_with_bigints<T: serde::Serialize>(value: &T) -> Result<JsValue, JsValue> {
    let serializer = serde_wasm_bindgen::Serializer::json_compatible()
        .serialize_large_number_types_as_bigints(true);
    value.serialize(&serializer).map_err(to_js_err)
}
