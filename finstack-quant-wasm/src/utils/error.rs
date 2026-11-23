//! Structured Rust-to-JavaScript error mapping for WASM bindings.

#[cfg(target_arch = "wasm32")]
use super::serialize::to_js_value;
use wasm_bindgen::JsValue;

/// Anything the bindings can turn into a structured JS error.
///
/// The `kind` property is decided by the error's *type*, never by sniffing
/// its message: every typed Rust error reports its Rust-owned
/// [`ErrorKind`](finstack_quant_core::ErrorKind) (`kind()` on the error,
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
                finstack_quant_core::format_chain(self)
            }
        }
    )*};
}

kinded_js_error!(
    finstack_quant_core::Error,
    finstack_quant_core::math::linalg::CholeskyError,
    finstack_quant_statements::Error,
    finstack_quant_factor::credit::decomposition::DecompositionError,
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
    finstack_quant_valuations::pricer::PricingError,
    finstack_quant_models::correlation::Error,
    finstack_quant_core::math::linalg::CorrelationError,
);

impl IntoJsError for finstack_quant_portfolio::Error {
    fn js_kind(&self) -> &'static str {
        self.kind().as_str()
    }
    fn js_message(&self) -> String {
        finstack_quant_core::format_chain(self)
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
            other => finstack_quant_core::format_chain(other),
        }
    }
}

/// A strict persisted-contract load failure: malformed, unversioned, oversized
/// or semantically invalid input is validation; a wrapped core error keeps its
/// own Rust kind. `code` names the contract failure mode, as the portfolio
/// materialization errors do.
impl IntoJsError for finstack_quant_core::contract::ContractError {
    fn js_kind(&self) -> &'static str {
        match self {
            Self::Core(error) => error.js_kind(),
            _ => "validation",
        }
    }
    fn js_message(&self) -> String {
        match self {
            Self::Core(error) => error.js_message(),
            other => other.to_string(),
        }
    }
    fn js_code(&self) -> Option<&'static str> {
        match self {
            Self::UnsupportedVersion { .. } => Some("unsupported_version"),
            Self::MissingVersion { .. } => Some("missing_version"),
            Self::MalformedSchema { .. } => Some("malformed_schema"),
            Self::LimitExceeded { .. } => Some("limit_exceeded"),
            Self::Report(_) => Some("report"),
            _ => None,
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
            return named_js_error("ContractValidationError", &error);
        }
        _ => return to_js_err(error),
    };
    #[cfg(target_arch = "wasm32")]
    if let Some(code) = error.js_code() {
        let _ = js_sys::Reflect::set(&js, &JsValue::from("code"), &JsValue::from(code));
    }
    js
}

/// Convert a strict persisted-contract load failure.
///
/// Contract failures are `ContractValidationError`s with `kind: "validation"`
/// and the failure-mode `code`; a validation report is also attached as
/// `error.report`. A wrapped core error is an ordinary [`to_js_err`] error.
///
/// # Arguments
///
/// * `error` - Typed contract error returned by a strict `from_*_slice` loader.
pub fn contract_to_js_err(error: finstack_quant_core::contract::ContractError) -> JsValue {
    use finstack_quant_core::contract::ContractError;
    if matches!(error, ContractError::Core(_)) {
        return to_js_err(error);
    }
    let js = named_js_error("ContractValidationError", &error);
    #[cfg(target_arch = "wasm32")]
    if let ContractError::Report(report) = &error {
        if let Ok(value) = to_js_value(report.as_ref()) {
            let _ = js_sys::Reflect::set(&js, &JsValue::from("report"), &value);
        }
    }
    js
}

// Native unit tests for `to_js_err` are limited because `js_sys::Error` only
// behaves normally under wasm32. The function is exercised indirectly by
// error-path wasm-bindgen tests.
