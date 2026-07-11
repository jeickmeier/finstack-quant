//! Shared conversion helpers for WASM bindings.
//!
//! Utilities for error mapping, JSON serialization, and decimal conversion
//! used across all domain binding modules.

pub mod date;

pub use date::{date_to_iso, parse_iso_date, parse_iso_dates};

use wasm_bindgen::JsValue;

/// Convert any `Display`-able error into a structured `JsValue` error.
///
/// Returns a plain JS `Error` object whose `message` is the error's
/// `Display` text and whose `name` is `"FinstackError"`. Structured
/// errors let JS clients pattern-match on `err.name` and reliably read
/// `err.message` rather than parsing ad-hoc strings.
pub fn to_js_err(e: impl std::fmt::Display) -> JsValue {
    let message = e.to_string();
    let kind = classify_error_message(&message);
    structured_js_error("FinstackError", &message, Some(kind), None)
}

/// Convert an error with a `source()` chain into a structured `JsValue` error.
pub fn to_js_error(e: &dyn std::error::Error) -> JsValue {
    js_value_from_message(format_error_chain(e))
}

/// Serialize a value to a `JsValue` using JSON-compatible conventions.
///
/// Unlike `serde_wasm_bindgen::to_value`, which serializes Rust maps (and
/// `serde_json::Value::Object`) as ES2015 `Map`s, this helper uses
/// [`serde_wasm_bindgen::Serializer::json_compatible`] so maps become plain
/// JS objects — matching the shapes declared in `index.d.ts` and the dict
/// shapes returned by the Python bindings.
///
/// # Errors
///
/// Returns a structured `JsValue` error if serialization fails.
pub fn to_js_value<T: serde::Serialize>(value: &T) -> Result<JsValue, JsValue> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(to_js_err)
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

fn js_value_from_message(msg: String) -> JsValue {
    #[cfg(target_arch = "wasm32")]
    {
        let err = js_sys::Error::new(&msg);
        err.set_name("FinstackError");
        err.into()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = msg;
        JsValue::NULL
    }
}

fn classify_error_message(message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("not found")
        || lower.contains("missing curve")
        || lower.contains("missing fixing")
        || lower.contains("required fixing")
        || lower.contains("requires fixings")
    {
        "not_found"
    } else if lower.contains("validation")
        || lower.contains("invalid")
        || lower.contains("malformed")
        || lower.contains("must ")
    {
        "validation"
    } else {
        "computation"
    }
}

/// Largest integer a JavaScript `number` (IEEE-754 double) can represent
/// exactly: `2^53 - 1` (`Number.MAX_SAFE_INTEGER`).
///
/// `usize` counts that cross the wasm boundary are marshaled as `f64`. A value
/// above this bound cannot survive the round trip without silent rounding, so
/// callers should reject counts larger than this rather than accept a
/// mis-represented value.
pub const MAX_SAFE_JS_INTEGER: u64 = 9_007_199_254_740_991;

/// Validate that a `usize` count is exactly representable as a JavaScript
/// `number` before it crosses the wasm boundary.
///
/// `wasm-bindgen` marshals `usize` as an IEEE-754 double; on a 64-bit host a
/// `usize` above `2^53 - 1` would round silently. This guard converts that
/// silent precision loss into an explicit, catchable JS error. `label` names
/// the offending count in the message (e.g. `"nested_paths"`).
///
/// # Errors
///
/// Returns a structured `JsValue` error when `count` exceeds
/// [`MAX_SAFE_JS_INTEGER`].
pub fn check_js_safe_count(count: usize, label: &str) -> Result<(), JsValue> {
    if count as u64 > MAX_SAFE_JS_INTEGER {
        return Err(to_js_err(format!(
            "{label} ({count}) exceeds the maximum JavaScript-safe integer \
             ({MAX_SAFE_JS_INTEGER}); counts above 2^53-1 cannot cross the \
             wasm boundary without silent rounding"
        )));
    }
    Ok(())
}

fn format_error_chain(err: &dyn std::error::Error) -> String {
    let mut out = err.to_string();
    let mut src = err.source();
    while let Some(cause) = src {
        let msg = cause.to_string();
        if !out.ends_with(&msg) {
            out.push_str(": ");
            out.push_str(&msg);
        }
        src = cause.source();
    }
    out
}

// Native unit tests for `to_js_err` are limited because `js_sys::Error` only
// behaves normally under wasm32. The function is exercised indirectly by
// error-path wasm-bindgen tests.

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::fmt;

    #[derive(Debug)]
    struct Wrapper(Box<dyn Error + Send + Sync>);

    impl fmt::Display for Wrapper {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "calibration failed")
        }
    }

    impl Error for Wrapper {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&*self.0)
        }
    }

    #[derive(Debug)]
    struct Leaf;

    impl fmt::Display for Leaf {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "solver diverged after 1000 iterations")
        }
    }

    impl Error for Leaf {}

    #[test]
    fn format_error_chain_flattens_error_sources() {
        let err = Wrapper(Box::new(Leaf));

        assert_eq!(
            format_error_chain(&err),
            "calibration failed: solver diverged after 1000 iterations"
        );
    }

    #[test]
    fn check_js_safe_count_accepts_values_up_to_the_bound() {
        // Zero, a typical count, and exactly MAX_SAFE_JS_INTEGER must pass.
        assert!(check_js_safe_count(0, "n").is_ok());
        assert!(check_js_safe_count(1_000_000, "n").is_ok());
        // `usize` on the 64-bit host can hold MAX_SAFE_JS_INTEGER exactly.
        let at_bound = MAX_SAFE_JS_INTEGER as usize;
        assert!(
            check_js_safe_count(at_bound, "n").is_ok(),
            "the boundary value itself is JS-safe and must be accepted"
        );
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn check_js_safe_count_rejects_values_above_the_bound() {
        // One past the bound cannot survive the f64 round trip; must error.
        let over = MAX_SAFE_JS_INTEGER as usize + 1;
        let result = check_js_safe_count(over, "nested_paths");
        assert!(
            result.is_err(),
            "a count above 2^53-1 must be rejected, not silently rounded"
        );
    }
}
