//! WASM handle for `finstack_quant_core::market_data::context::MarketContext`.
//!
//! Parse a market context once with `MarketContext.fromJson`, then pass the
//! handle to every `*WithMarket` entry point instead of re-parsing the same
//! JSON on each pricing or sensitivity call.

use crate::utils::input::json_text;
use crate::utils::to_js_err;
use finstack_quant_core::market_data::context::MarketContext;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// Parsed Rust `MarketContext` held across pricing calls.
///
/// Build it with `MarketContext.fromJson`, then pass it to
/// `priceInstrumentWithMarket` and the other `*WithMarket` entry points.
/// This avoids re-parsing the market JSON in bulk-pricing and Greeks-sweep
/// loops. WASM binds only the JSON round trip; the Python class also exposes
/// the insertion and lookup methods.
///
/// @example
/// ```javascript
/// const market = core.MarketContext.fromJson(marketJson);
/// for (const instr of instruments) {
///   const result = valuations.instruments.priceInstrumentWithMarket(instr, market, "2025-06-15");
/// }
/// ```
#[wasm_bindgen(js_name = MarketContext)]
pub struct JsMarketContext {
    inner: Arc<MarketContext>,
}

#[wasm_bindgen(js_class = MarketContext)]
impl JsMarketContext {
    /// Parse a market context from its canonical JSON representation.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical MarketContext JSON (string or plain object), the
    ///   same payload accepted by pricing `marketJson` arguments. Unknown fields
    ///   are rejected.
    /// @returns A `MarketContext` handle that can be reused across pricing calls; release it with free().
    /// @throws Error - Throws with kind `validation` when the JSON is malformed or does not match the MarketContext schema, and a `TypeError` when `json` is neither a string nor a plain object.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsMarketContext, JsValue> {
        let json: &str = &json_text(&json, "json")?;
        let inner: MarketContext = serde_json::from_str(json).map_err(to_js_err)?;
        Ok(JsMarketContext {
            inner: Arc::new(inner),
        })
    }

    /// Serialize the wrapped MarketContext back to canonical JSON.
    ///
    /// @returns Canonical MarketContext JSON accepted by `MarketContext.fromJson` and every `marketJson` argument.
    /// @throws Error - Throws if the market context cannot be serialized to JSON.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Access the inner MarketContext (crate-internal).
    pub(crate) fn inner(&self) -> &MarketContext {
        self.inner.as_ref()
    }
}

impl JsMarketContext {
    /// Wrap a Rust `MarketContext` produced by a binding (crate-internal).
    pub(crate) fn from_inner(inner: MarketContext) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }
}
