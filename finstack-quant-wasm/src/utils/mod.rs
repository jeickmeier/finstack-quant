//! Shared conversion helpers for WASM bindings.
//!
//! Utilities for error mapping, JSON serialization, and decimal conversion
//! used across all domain binding modules.

pub mod date;

pub use date::{date_to_iso, parse_iso_date, parse_iso_dates};

use wasm_bindgen::JsValue;

/// Anything the bindings can turn into a structured JS error.
///
/// The `kind` property is decided by the error's *type*, never by sniffing
/// its message: typed core errors classify by variant through
/// [`finstack_quant_core::Error::kind`], crate errors that wrap a core error
/// defer to it, and plain strings / parse errors are input validation.
pub trait IntoJsError {
    /// Structured `kind` for the JS error (`"not_found"`, `"validation"`,
    /// `"computation"`).
    fn js_kind(&self) -> &'static str;
    /// Full message, including the source chain where one exists.
    fn js_message(&self) -> String;
}

/// JS `kind` label for a core [`ErrorKind`](finstack_quant_core::error::ErrorKind).
fn kind_label(kind: finstack_quant_core::error::ErrorKind) -> &'static str {
    use finstack_quant_core::error::ErrorKind;
    match kind {
        ErrorKind::NotFound => "not_found",
        ErrorKind::Validation => "validation",
        ErrorKind::Computation => "computation",
    }
}

impl IntoJsError for finstack_quant_core::Error {
    fn js_kind(&self) -> &'static str {
        kind_label(self.kind())
    }
    fn js_message(&self) -> String {
        format_error_chain(self)
    }
}

impl IntoJsError for finstack_quant_valuations::Error {
    fn js_kind(&self) -> &'static str {
        match self {
            finstack_quant_valuations::Error::Core(e) => e.js_kind(),
            _ => "computation",
        }
    }
    fn js_message(&self) -> String {
        format_error_chain(self)
    }
}

impl IntoJsError for finstack_quant_statements::error::Error {
    fn js_kind(&self) -> &'static str {
        use finstack_quant_statements::error::Error;
        match self {
            Error::Core(e) => e.js_kind(),
            Error::NodeNotFound(_) | Error::RegistryNotFound(_) => "not_found",
            Error::Eval(_) | Error::CircularDependency(_) => "computation",
            _ => "validation",
        }
    }
    fn js_message(&self) -> String {
        format_error_chain(self)
    }
}

impl IntoJsError for finstack_quant_portfolio::Error {
    fn js_kind(&self) -> &'static str {
        use finstack_quant_portfolio::Error;
        match self {
            Error::Core(e) => e.js_kind(),
            Error::UnknownEntity { .. } | Error::MissingMarketData(_) => "not_found",
            Error::ValuationError { .. } | Error::FxConversionFailed { .. } => "computation",
            _ => "validation",
        }
    }
    fn js_message(&self) -> String {
        format_error_chain(self)
    }
}

impl IntoJsError for finstack_quant_scenarios::Error {
    fn js_kind(&self) -> &'static str {
        use finstack_quant_scenarios::Error;
        match self {
            Error::Core(e) => e.js_kind(),
            Error::Statements(e) => e.js_kind(),
            Error::Valuations(e) => e.js_kind(),
            Error::MarketDataNotFound { .. }
            | Error::NodeNotFound { .. }
            | Error::TenorNotFound { .. }
            | Error::InstrumentNotFound(_) => "not_found",
            Error::Internal(_) => "computation",
            _ => "validation",
        }
    }
    fn js_message(&self) -> String {
        format_error_chain(self)
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
    strum::ParseError,
    finstack_quant_core::math::linalg::CorrelationError,
    finstack_quant_core::math::linalg::CholeskyError,
    finstack_quant_models::correlation::Error,
    finstack_quant_models::factor::credit::decomposition::DecompositionError,
);

/// Numerical failures inside a model are computation errors.
macro_rules! computation_js_error {
    ($($ty:ty),* $(,)?) => {$(
        impl IntoJsError for $ty {
            fn js_kind(&self) -> &'static str {
                "computation"
            }
            fn js_message(&self) -> String {
                self.to_string()
            }
        }
    )*};
}

computation_js_error!(finstack_quant_models::fourier::FourierError);

impl<T: IntoJsError + ?Sized> IntoJsError for &T {
    fn js_kind(&self) -> &'static str {
        (**self).js_kind()
    }
    fn js_message(&self) -> String {
        (**self).js_message()
    }
}

impl<T: IntoJsError + ?Sized> IntoJsError for Box<T> {
    fn js_kind(&self) -> &'static str {
        (**self).js_kind()
    }
    fn js_message(&self) -> String {
        (**self).js_message()
    }
}

/// Convert a binding error into a structured `JsValue` error.
///
/// Returns a plain JS `Error` object whose `message` is the error's text
/// (with the source chain for typed errors) and whose `name` is
/// `"FinstackError"`. The `kind` property comes from [`IntoJsError`], i.e.
/// from the error's type — never from its message text, so an identifier
/// that happens to contain "not found" cannot change the classification.
pub fn to_js_err(e: impl IntoJsError) -> JsValue {
    structured_js_error("FinstackError", &e.js_message(), Some(e.js_kind()), None)
}

/// Convert a typed `finstack_quant_core::Error` into a structured `JsValue`
/// error. Identical to [`to_js_err`]; kept for call sites that hold a
/// reference.
pub fn to_js_err_core(e: &finstack_quant_core::Error) -> JsValue {
    to_js_err(e)
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

/// Serialize a value to a JSON-compatible `JsValue` while preserving Rust
/// 64-bit integers as JavaScript `BigInt` values.
///
/// This is reserved for structured host results whose full-width integer
/// fields, such as Monte Carlo seeds, must remain lossless.
pub(crate) fn to_js_value_with_bigints<T: serde::Serialize>(value: &T) -> Result<JsValue, JsValue> {
    let serializer = serde_wasm_bindgen::Serializer::json_compatible()
        .serialize_large_number_types_as_bigints(true);
    value.serialize(&serializer).map_err(to_js_err)
}

pub(crate) fn to_js_value_with_kind<T: serde::Serialize>(
    value: &T,
    kind: &'static str,
) -> Result<JsValue, JsValue> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|error| structured_js_error("FinstackError", &error.to_string(), Some(kind), None))
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

/// Convert a typed persisted-contract failure to a structured JavaScript error.
///
/// The public ``kind`` value comes directly from the Rust enum variant. Report
/// failures additionally expose the serialized [`ValidationReport`] as
/// ``error.report``.
///
/// # Arguments
///
/// * `error` - Typed contract failure to map without inspecting its message.
pub fn contract_to_js_error(error: finstack_quant_core::contract::ContractError) -> JsValue {
    use finstack_quant_core::contract::ContractError;

    let kind = contract_error_kind(&error);
    match error {
        ContractError::Report(report) => validation_report_js_error(&report),
        other => contract_error_without_report(&other.to_string(), kind),
    }
}

fn contract_error_kind(error: &finstack_quant_core::contract::ContractError) -> &'static str {
    use finstack_quant_core::contract::ContractError;

    match error {
        ContractError::UnsupportedVersion { .. } => "unsupported_version",
        ContractError::MissingVersion { .. } => "missing_version",
        ContractError::MalformedSchema { .. } => "malformed_schema",
        ContractError::LimitExceeded { .. } => "limit_exceeded",
        ContractError::Report(_) => "report",
        ContractError::Core(_) => "core",
        #[allow(unreachable_patterns)]
        _ => "contract",
    }
}

/// Convert a portfolio materialization failure without message classification.
///
/// # Arguments
///
/// * `error` - Typed portfolio error returned by the materialization API.
pub fn materialization_to_js_error(error: finstack_quant_portfolio::Error) -> JsValue {
    let kind = materialization_error_kind(&error);
    match error {
        finstack_quant_portfolio::Error::MaterializationFailed(report) => {
            validation_report_js_error(&report)
        }
        error @ finstack_quant_portfolio::Error::ContractLimitExceeded { .. } => {
            contract_error_without_report(&error.to_string(), kind)
        }
        other => structured_js_error("FinstackError", &other.to_string(), Some(kind), None),
    }
}

fn materialization_error_kind(error: &finstack_quant_portfolio::Error) -> &'static str {
    use finstack_quant_portfolio::Error;

    match error {
        Error::UnknownEntity { .. } => "unknown_entity",
        Error::ValidationFailed(_) => "validation",
        Error::FxConversionFailed { .. } => "fx_conversion",
        Error::ValuationError { .. } => "valuation",
        Error::ScenarioError(_) => "scenario",
        Error::MissingMarketData(_) => "missing_market_data",
        Error::Core(_) => "core",
        Error::InvalidInput(_) => "invalid_input",
        Error::ContractLimitExceeded { .. } => "limit_exceeded",
        Error::MaterializationFailed(_) => "report",
        #[allow(unreachable_patterns)]
        _ => "portfolio",
    }
}

fn validation_report_js_error(report: &finstack_quant_core::contract::ValidationReport) -> JsValue {
    let error = structured_js_error(
        "ContractValidationError",
        &format!(
            "validation failed with {} structured diagnostic(s)",
            report.diagnostics.len()
        ),
        Some("report"),
        None,
    );
    #[cfg(target_arch = "wasm32")]
    {
        if let Ok(value) = to_js_value_with_kind(report, "serialization") {
            let _ = js_sys::Reflect::set(&error, &JsValue::from_str("report"), &value);
        }
    }
    error
}

fn contract_error_without_report(message: &str, kind: &str) -> JsValue {
    structured_js_error("ContractValidationError", message, Some(kind), None)
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

    #[test]
    fn contract_error_kind_is_selected_from_variant_not_message() {
        use finstack_quant_core::contract::{
            ContractError, Diagnostic, LoadPhase, Severity, ValidationReport,
        };

        let mut report = ValidationReport::default();
        report.diagnostics.push(Diagnostic::new(
            "test/misleading",
            LoadPhase::Build,
            Severity::Error,
            "missing curve but this is still a report variant",
        ));
        let cases = [
            (
                ContractError::UnsupportedVersion {
                    contract: "missing curve".to_string(),
                    found: 3,
                    min: 1,
                    max: 2,
                },
                "unsupported_version",
            ),
            (
                ContractError::MissingVersion {
                    contract: "malformed validation".to_string(),
                },
                "missing_version",
            ),
            (
                ContractError::MalformedSchema {
                    value: "missing curve".to_string(),
                    expected: "not found".to_string(),
                },
                "malformed_schema",
            ),
            (
                ContractError::LimitExceeded {
                    what: "malformed validation",
                    found: 2,
                    limit: 1,
                },
                "limit_exceeded",
            ),
            (ContractError::Report(Box::new(report)), "report"),
            (
                ContractError::Core(finstack_quant_core::Error::Internal(
                    "missing curve malformed validation".to_string(),
                )),
                "core",
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(contract_error_kind(&error), expected);
        }
    }

    #[test]
    fn materialization_error_kind_is_selected_from_variant_not_message() {
        use finstack_quant_portfolio::Error;

        let cases = [
            (
                Error::UnknownEntity {
                    position_id: "position".into(),
                    entity_id: "entity".into(),
                },
                "unknown_entity",
            ),
            (
                Error::ValidationFailed("missing curve".to_string()),
                "validation",
            ),
            (
                Error::FxConversionFailed {
                    from: finstack_quant_core::currency::Currency::USD,
                    to: finstack_quant_core::currency::Currency::EUR,
                },
                "fx_conversion",
            ),
            (
                Error::ValuationError {
                    position_id: "position".into(),
                    message: "malformed validation".to_string(),
                },
                "valuation",
            ),
            (
                Error::ScenarioError("missing curve".to_string()),
                "scenario",
            ),
            (
                Error::MissingMarketData("malformed validation".to_string()),
                "missing_market_data",
            ),
            (
                Error::Core(finstack_quant_core::Error::Internal(
                    "missing curve malformed validation".to_string(),
                )),
                "core",
            ),
            (
                Error::InvalidInput("not found malformed validation".to_string()),
                "invalid_input",
            ),
            (
                Error::ContractLimitExceeded {
                    what: "missing curve malformed validation".to_string(),
                    found: 2,
                    limit: 1,
                },
                "limit_exceeded",
            ),
            (Error::MaterializationFailed(Box::default()), "report"),
        ];

        for (error, expected) in cases {
            assert_eq!(materialization_error_kind(&error), expected);
        }
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
