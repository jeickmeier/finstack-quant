//! Shared conversion helpers for WASM bindings.
//!
//! Utilities for error mapping, JSON serialization, and decimal conversion
//! used across all domain binding modules.

pub mod date;
pub mod input;

pub use date::{date_to_iso, parse_iso_date, parse_iso_dates};

use finstack_quant_core::wire::{non_finite_f64, NonFiniteFields};
use wasm_bindgen::JsValue;

/// Anything the bindings can turn into a structured JS error.
///
/// The `kind` property is decided by the error's *type*, never by sniffing
/// its message: every typed Rust error reports its Rust-owned
/// [`ErrorKind`](finstack_quant_core::error::ErrorKind) (`kind()` on the error,
/// or the kind of its fold into [`finstack_quant_core::Error`]), and plain
/// strings / parse errors are input validation.
pub trait IntoJsError {
    /// Structured `kind` for the JS error (`"not_found"`, `"validation"`,
    /// `"computation"`).
    fn js_kind(&self) -> &'static str;
    /// Full message, including the source chain where one exists.
    fn js_message(&self) -> String;
    /// Stable machine-readable `code` refining `kind`, when the Rust error
    /// defines one.
    fn js_code(&self) -> Option<&'static str> {
        None
    }
}

/// Errors with a Rust-owned `kind()`: the kind is used as is and the message
/// is the full source chain.
macro_rules! kinded_js_error {
    ($($ty:ty),* $(,)?) => {$(
        impl IntoJsError for $ty {
            fn js_kind(&self) -> &'static str {
                self.kind().as_str()
            }
            fn js_message(&self) -> String {
                finstack_quant_core::error::format_chain(self)
            }
        }
    )*};
}

kinded_js_error!(
    finstack_quant_core::Error,
    finstack_quant_statements::error::Error,
    finstack_quant_models::factor::credit::decomposition::DecompositionError,
    finstack_quant_models::credit::migration::MigrationError,
    finstack_quant_models::credit::pd::PdCalibrationError,
    finstack_quant_models::credit::scoring::CreditScoringError,
);

/// Errors reported through their fold into [`finstack_quant_core::Error`]:
/// kind and message both come from the folded core error, exactly as the
/// Python binding's `core_to_py(error.into())` reports them.
macro_rules! core_folded_js_error {
    ($($ty:ty),* $(,)?) => {$(
        impl IntoJsError for $ty {
            fn js_kind(&self) -> &'static str {
                finstack_quant_core::Error::from(self.clone()).js_kind()
            }
            fn js_message(&self) -> String {
                finstack_quant_core::Error::from(self.clone()).js_message()
            }
        }
    )*};
}

core_folded_js_error!(
    finstack_quant_valuations::Error,
    finstack_quant_models::correlation::Error,
    finstack_quant_core::math::linalg::CorrelationError,
    finstack_quant_models::fourier::FourierError,
);

impl IntoJsError for finstack_quant_portfolio::Error {
    fn js_kind(&self) -> &'static str {
        self.kind().as_str()
    }
    fn js_message(&self) -> String {
        finstack_quant_core::error::format_chain(self)
    }
    fn js_code(&self) -> Option<&'static str> {
        self.code()
    }
}

impl IntoJsError for finstack_quant_scenarios::Error {
    fn js_kind(&self) -> &'static str {
        self.kind().as_str()
    }
    fn js_message(&self) -> String {
        use finstack_quant_scenarios::Error;
        match self {
            Error::Core(error) => error.js_message(),
            Error::Statements(error) => error.js_message(),
            Error::Valuations(error) => error.js_message(),
            other => finstack_quant_core::error::format_chain(other),
        }
    }
}

/// Plain messages and parse/decode failures are input validation.
macro_rules! validation_js_error {
    ($($ty:ty),* $(,)?) => {$(
        impl IntoJsError for $ty {
            fn js_kind(&self) -> &'static str {
                "validation"
            }
            fn js_message(&self) -> String {
                self.to_string()
            }
        }
    )*};
}

validation_js_error!(
    String,
    &str,
    serde_json::Error,
    serde_wasm_bindgen::Error,
    time::error::Parse,
    time::error::ComponentRange,
    std::num::ParseFloatError,
    std::num::ParseIntError,
    std::fmt::Error,
    finstack_quant_core::math::linalg::CholeskyError,
);

impl<T: IntoJsError + ?Sized> IntoJsError for &T {
    fn js_kind(&self) -> &'static str {
        (**self).js_kind()
    }
    fn js_message(&self) -> String {
        (**self).js_message()
    }
    fn js_code(&self) -> Option<&'static str> {
        (**self).js_code()
    }
}

impl<T: IntoJsError + ?Sized> IntoJsError for Box<T> {
    fn js_kind(&self) -> &'static str {
        (**self).js_kind()
    }
    fn js_message(&self) -> String {
        (**self).js_message()
    }
    fn js_code(&self) -> Option<&'static str> {
        (**self).js_code()
    }
}

/// Convert a binding error into a structured `JsValue` error.
///
/// Returns a plain JS `Error` object whose `message` is the error's text
/// (with the source chain for typed errors) and whose `name` is
/// `"FinstackError"`. The `kind` property comes from [`IntoJsError`], i.e.
/// from the error's type — never from its message text, so an identifier
/// that happens to contain "not found" cannot change the classification.
/// A Rust error that defines a `code()` also sets `error.code`.
pub fn to_js_err(e: impl IntoJsError) -> JsValue {
    named_js_error("FinstackError", &e)
}

/// A structured error named `name` carrying `e`'s message, `kind` and, when
/// defined, `code`.
fn named_js_error(name: &str, e: &impl IntoJsError) -> JsValue {
    let error = structured_js_error(name, &e.js_message(), Some(e.js_kind()), None);
    #[cfg(target_arch = "wasm32")]
    if let Some(code) = e.js_code() {
        let _ = js_sys::Reflect::set(&error, &JsValue::from("code"), &JsValue::from(code));
    }
    error
}

/// Serialize a value to a `JsValue` using JSON-compatible conventions.
///
/// Unlike `serde_wasm_bindgen::to_value`, which serializes Rust maps (and
/// `serde_json::Value::Object`) as ES2015 `Map`s, this helper uses
/// [`serde_wasm_bindgen::Serializer::json_compatible`] so maps become plain
/// JS objects — matching the shapes declared in `index.d.ts` and the dict
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

/// Build a named JS `Error` with optional structured `kind` and `cause`
/// properties.
pub fn structured_js_error(
    name: &str,
    message: &str,
    kind: Option<&str>,
    cause_json: Option<&str>,
) -> JsValue {
    #[cfg(target_arch = "wasm32")]
    {
        let err = js_sys::Error::new(message);
        err.set_name(name);
        if let Some(kind) = kind {
            let _ =
                js_sys::Reflect::set(&err, &JsValue::from_str("kind"), &JsValue::from_str(kind));
        }
        if let Some(cause_json) = cause_json {
            let cause_value =
                js_sys::JSON::parse(cause_json).unwrap_or_else(|_| JsValue::from_str(cause_json));
            let _ = js_sys::Reflect::set(&err, &JsValue::from_str("cause"), &cause_value);
        }
        err.into()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (name, message, kind, cause_json);
        JsValue::NULL
    }
}

/// Convert a portfolio materialization failure.
///
/// Contract failures (a validation report or an exceeded resource limit) are
/// `ContractValidationError`s with `kind: "validation"`, the Rust `code`
/// (`"report"` / `"limit_exceeded"`) and, for a report, the serialized
/// [`ValidationReport`](finstack_quant_core::contract::ValidationReport) as
/// `error.report`. Every other failure is an ordinary [`to_js_err`] error.
///
/// # Arguments
///
/// * `error` - Typed portfolio error returned by the materialization API.
pub fn materialization_to_js_error(error: finstack_quant_portfolio::Error) -> JsValue {
    use finstack_quant_portfolio::Error;
    let js = match &error {
        Error::MaterializationFailed(report) => {
            let js = structured_js_error(
                "ContractValidationError",
                &report.summary(),
                Some(error.js_kind()),
                None,
            );
            #[cfg(target_arch = "wasm32")]
            if let Ok(value) = to_js_value(report.as_ref()) {
                let _ = js_sys::Reflect::set(&js, &JsValue::from("report"), &value);
            }
            js
        }
        Error::ContractLimitExceeded { .. } => {
            return named_js_error("ContractValidationError", &error)
        }
        _ => return to_js_err(error),
    };
    #[cfg(target_arch = "wasm32")]
    if let Some(code) = error.js_code() {
        let _ = js_sys::Reflect::set(&js, &JsValue::from("code"), &JsValue::from(code));
    }
    js
}

// Native unit tests for `to_js_err` are limited because `js_sys::Error` only
// behaves normally under wasm32. The function is exercised indirectly by
// error-path wasm-bindgen tests.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_error_kind_is_selected_from_variant_not_message() {
        use finstack_quant_core::error::InputError;
        use finstack_quant_core::Error;

        let cases = [
            // A curve literally named "validation" must still classify as a
            // lookup miss — variant, not message sniffing.
            (
                Error::Input(InputError::MissingCurve {
                    requested: "validation".to_string(),
                    suggestions: vec![],
                }),
                "not_found",
            ),
            (
                Error::Input(InputError::CalendarNotFound {
                    requested: "must be valid".to_string(),
                    suggestions: vec![],
                }),
                "not_found",
            ),
            (
                Error::Input(InputError::NotFound {
                    id: "invalid id containing 'not found'".to_string(),
                }),
                "not_found",
            ),
            (
                Error::Input(InputError::SolverConvergenceFailed {
                    iterations: 1,
                    residual: 1.0,
                    last_x: 0.0,
                    reason: "x".to_string(),
                }),
                "computation",
            ),
            (
                Error::CircularDependency {
                    path: vec!["a".to_string()],
                },
                "computation",
            ),
            // Calibration stays "computation" even though its message contains
            // every keyword a message sniffer would match.
            (
                Error::Calibration {
                    message: "not found: invalid value must be positive".to_string(),
                    category: String::new(),
                },
                "computation",
            ),
            // Genuine validation keeps its kind even when an embedded
            // identifier contains "not found".
            (
                Error::Validation("curve 'not found quotes' rejected".to_string()),
                "validation",
            ),
            (Error::Input(InputError::Invalid), "validation"),
            (
                Error::Input(InputError::VolatilityConversionFailed {
                    tolerance: 1e-8,
                    residual: 1e-3,
                }),
                "computation",
            ),
            (
                Error::Input(InputError::TooLarge {
                    what: "arena".to_string(),
                    requested_bytes: 8,
                    limit_bytes: 4,
                }),
                "computation",
            ),
            (
                Error::metric_calculation_failed(
                    "dv01",
                    Error::Calibration {
                        message: "solver stalled".to_string(),
                        category: String::new(),
                    },
                ),
                "computation",
            ),
            (
                Error::metric_calculation_failed(
                    "dv01",
                    Error::Input(InputError::SolverConvergenceFailed {
                        iterations: 1,
                        residual: 1.0,
                        last_x: 0.0,
                        reason: "x".to_string(),
                    }),
                ),
                "computation",
            ),
            (
                Error::metric_calculation_failed("ytm", Error::Validation("bad quote".to_string())),
                "validation",
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(error.js_kind(), expected);
        }
    }
}
