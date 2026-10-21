//! WASM bindings for the `finstack-quant-models` crate.
//!
//! Split by model family:
//! - [`closed_form`] — closed-form option primitives.
//! - [`fourier`] — COS-method Fourier pricers.
//! - [`volatility`] — volatility models, evaluators, and convention conversion.
//! - [`credit`] — structural-credit, LGD, PD, migration, scoring, recovery-waterfall
//!   and liability-management models.
//! - [`factor`] — credit factor hierarchy and factor-risk decomposition.
//! - [`correlation`] — copula, recovery, and joint-probability utilities.
//! - [`monte_carlo`] — stochastic option-pricing convenience functions.
//! - [`liquidity`] — liquidity estimation, risk, and market-impact models.
//! - [`rates`] — interest-rate models and dynamic term-structure engines.

/// Generate the `fromJson` / `toJson` pair for a handle wrapping a serde
/// type in `inner`.
///
/// Both exits use the canonical Rust serde form, so the JSON is the same text
/// Python `to_json` / `from_json` exchange. The arms mirror Python's
/// `wire_methods!`: append `validate` when the Rust type checks its invariants
/// in an inherent `validate()` rather than in `Deserialize`, and `rust_json`
/// when it owns its wire form through inherent `from_json(&str)` /
/// `to_json()` methods.
macro_rules! json_round_trip {
    ($js:ident, $class:ident) => {
        json_round_trip!(
            @impl $js, $class,
            |json| $crate::utils::input::from_js_json(&json, "json")?,
            |inner| serde_json::to_string(inner).map_err($crate::utils::to_js_err)
        );
    };
    ($js:ident, $class:ident, validate) => {
        json_round_trip!(
            @impl $js, $class,
            |json| {
                let loaded = Self {
                    inner: $crate::utils::input::from_js_json(&json, "json")?,
                };
                loaded.inner.validate().map_err($crate::utils::to_js_err)?;
                loaded.inner
            },
            |inner| serde_json::to_string(inner).map_err($crate::utils::to_js_err)
        );
    };
    ($js:ident, $class:ident, rust_json $rust:ty) => {
        json_round_trip!(
            @impl $js, $class,
            |json| <$rust>::from_json(&$crate::utils::input::json_text(&json, "json")?)
                .map_err($crate::utils::to_js_err)?,
            |inner| inner.to_json().map_err($crate::utils::to_js_err)
        );
    };
    (@impl $js:ident, $class:ident, |$json:ident| $load:expr, |$inner:ident| $dump:expr) => {
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
                let $json = json;
                Ok(Self { inner: $load })
            }

            /// Serialize to the canonical JSON wire form accepted by `fromJson`
            /// and by Python `from_json`.
            /// @returns Compact canonical JSON text.
            ///
            /// # Errors
            ///
            /// Throws a `validation` error if serialization fails, for example
            /// on a non-finite value the type's wire form rejects.
            #[wasm_bindgen(js_name = toJson)]
            pub fn to_json(&self) -> Result<String, wasm_bindgen::JsValue> {
                let $inner = &self.inner;
                $dump
            }
        }
    };
}

pub mod closed_form;
pub mod correlation;
pub mod credit;
pub mod factor;
pub mod fourier;
pub mod liquidity;
pub mod monte_carlo;
pub mod rates;
pub mod volatility;
