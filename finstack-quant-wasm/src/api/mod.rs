//! WASM API modules mirroring the Rust umbrella crate structure.
//!
//! Each submodule corresponds to one Rust crate domain.
//!
//! Native tests cannot inspect `JsValue` errors because `js_sys::Error` and
//! other JS constructors only work under `wasm32`. When a binding has
//! meaningful validation or diagnostics, keep the Rust work in a private
//! `*_inner` helper that returns the domain error, and make the
//! `#[wasm_bindgen]` function a thin wrapper that converts that error at the
//! boundary. This keeps native tests precise while preserving JS-facing errors.

/// Generate the `fromJson` / `toJson` pair for a handle wrapping a serde
/// type (or `Arc<T>`) in `inner`.
///
/// Both exits use the canonical Rust serde form, so the JSON is the same text
/// Python `to_json` / `from_json` exchange. The arms mirror Python's
/// `wire_methods!`:
/// - Default: accepts `JsonInput` (JSON text or plain object).
/// - `string`: restricts `fromJson`'s TypeScript input type to `string` (for
///   quoted-string enums/ratings).
/// - `arc`: wraps `inner` in `Arc<T>` on load and dereferences `&*self.inner`
///   on save.
/// - `validate`: runs inherent `validate()` after deserializing rather than
///   relying on `Deserialize`.
/// - `rust_json <RustType>`: owns its wire form through inherent
///   `from_json(&str)` / `to_json()` methods.
macro_rules! json_round_trip {
    ($js:ident, $class:ident) => {
        json_round_trip!(
            @impl $js, $class, "JsonInput",
            |json| $crate::utils::input::from_js_json(&json, "json")?,
            |inner| serde_json::to_string(inner).map_err($crate::utils::to_js_err)
        );
    };
    ($js:ident, $class:ident, string) => {
        json_round_trip!(
            @impl $js, $class, "string",
            |json| $crate::utils::input::from_js_json(&json, "json")?,
            |inner| serde_json::to_string(inner).map_err($crate::utils::to_js_err)
        );
    };
    ($js:ident, $class:ident, arc) => {
        json_round_trip!(
            @impl $js, $class, "JsonInput",
            |json| ::std::sync::Arc::new($crate::utils::input::from_js_json(&json, "json")?),
            |inner| serde_json::to_string(&**inner).map_err($crate::utils::to_js_err)
        );
    };
    ($js:ident, $class:ident, validate) => {
        json_round_trip!(
            @impl $js, $class, "JsonInput",
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
            @impl $js, $class, "JsonInput",
            |json| <$rust>::from_json(&$crate::utils::input::json_text(&json, "json")?)
                .map_err($crate::utils::to_js_err)?,
            |inner| inner.to_json().map_err($crate::utils::to_js_err)
        );
    };
    (@impl $js:ident, $class:ident, $param_ty:literal, |$json:ident| $load:expr_2021, |$inner:ident| $dump:expr_2021) => {
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
            pub fn from_json(
                #[wasm_bindgen(unchecked_param_type = $param_ty)] json: wasm_bindgen::JsValue,
            ) -> Result<$js, wasm_bindgen::JsValue> {
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

/// Generate read-only `#[wasm_bindgen(getter)]` methods on a handle.
macro_rules! wasm_getters {
    (
        $js:ident, $class:ident |$s:ident| {
            $(
                $(#[$doc:meta])*
                $name:ident $(as $js_name:ident)? $(returns $ts:literal)? -> $ret:ty = $body:expr_2021;
            )+
        }
    ) => {
        #[finstack_quant_wasm_macros::plain_docs]
        #[wasm_bindgen::prelude::wasm_bindgen(js_class = $class)]
        impl $js {
            $(
                $(#[$doc])*
                #[wasm_bindgen(getter $(, js_name = $js_name)? $(, unchecked_return_type = $ts)?)]
                pub fn $name(&self) -> $ret {
                    let $s = self;
                    $body
                }
            )+
        }
    };
}

pub mod analytics;
pub mod attribution;
pub mod calibration;
pub mod cashflows;
pub mod core;
pub mod covenants;
pub mod features;
pub mod margin;
pub mod models;
pub mod portfolio;
pub mod scenarios;
pub mod schema;
pub mod statements;
pub mod statements_analytics;
pub mod valuations;
