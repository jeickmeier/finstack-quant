//! WASM bindings for the `finstack-quant-models` crate.
//!
//! Split by model family:
//! - [`analytic`] — closed-form option primitives.
//! - [`fourier`] — COS-method Fourier pricers.
//! - [`volatility`] — volatility models, evaluators, and convention conversion.
//! - [`credit`] — structural-credit model factories.
//! - [`correlation`] — copula, recovery, and joint-probability utilities.
//! - [`monte_carlo`] — stochastic option-pricing convenience functions.
//! - [`liquidity`] — liquidity estimation, risk, and market-impact models.
//! - [`rates`] — interest-rate models and dynamic term-structure engines.

/// Generate the `fromJson` / `toJson` pair for a handle wrapping a serde
/// type in `inner`.
///
/// Both exits use the canonical Rust serde form, so the JSON is the same text
/// Python `to_json` / `from_json` exchange.
macro_rules! json_round_trip {
    ($js:ident, $class:ident) => {
        #[wasm_bindgen::prelude::wasm_bindgen(js_class = $class)]
        impl $js {
            /// Load a handle from its canonical JSON wire form, the same form
            /// Python `from_json` reads.
            ///
            /// Unknown fields are rejected where the Rust type denies them and
            /// the Rust validation rules are applied again.
            /// @param json - Canonical JSON for this type, as JSON text or a plain object.
            /// @returns The validated handle.
            ///
            /// # Errors
            ///
            /// Throws a `TypeError` (`kind: "invalid_type"`) if `json` is neither
            /// a string nor a plain object, and a `validation` error if it is
            /// malformed or fails the type's validation.
            #[wasm_bindgen(js_name = fromJson)]
            pub fn from_json(json: wasm_bindgen::JsValue) -> Result<$js, wasm_bindgen::JsValue> {
                $crate::utils::input::from_js_json(&json, "json").map(|inner| Self { inner })
            }

            /// Serialize to the canonical JSON wire form accepted by `fromJson`
            /// and by Python `from_json`.
            /// @returns Compact canonical JSON text.
            ///
            /// # Errors
            ///
            /// Throws a `validation` error if serialization fails (not expected
            /// for a valid handle).
            #[wasm_bindgen(js_name = toJson)]
            pub fn to_json(&self) -> Result<String, wasm_bindgen::JsValue> {
                serde_json::to_string(&self.inner).map_err($crate::utils::to_js_err)
            }
        }
    };
}

/// Canonical serde tag of an externally tagged enum value (`"constant"`, or
/// the single key of `{"inverse_power": {...}}`), as the Python binding reports
/// it through `kind`.
pub(crate) fn serde_tag<T: serde::Serialize>(value: &T) -> Result<String, wasm_bindgen::JsValue> {
    match serde_json::to_value(value).map_err(crate::utils::to_js_err)? {
        serde_json::Value::String(tag) => Ok(tag),
        serde_json::Value::Object(map) if map.len() == 1 => Ok(map
            .into_iter()
            .next()
            .map(|(tag, _)| tag)
            .unwrap_or_default()),
        other => Err(crate::utils::to_js_err(format!(
            "expected an externally tagged enum value, got {other}"
        ))),
    }
}

pub mod analytic;
pub mod correlation;
pub mod credit;
pub mod factor;
pub mod fourier;
pub mod liability_management;
pub mod liquidity;
pub mod monte_carlo;
pub mod rates;
pub mod volatility;
