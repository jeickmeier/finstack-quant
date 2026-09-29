//! WASM market handle — parse MarketContext once, reuse across pricing calls.
//!
//! Avoids repeated `serde_json::from_str` on the full MarketContext JSON
//! in bulk-pricing and sensitivity-sweep workloads.

use crate::utils::input::json_text;
use crate::utils::to_js_err;
use finstack_quant_core::market_data::context::MarketContext;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// Opaque handle wrapping a parsed [`MarketContext`].
///
/// Construct once from JSON, then pass to `priceInstrumentWithMarket` and
/// other `*WithMarket` pricing entry points. Eliminates the per-call
/// market-parse overhead in bulk-pricing and Greeks-sweep loops.
///
/// @example
/// ```javascript
/// const market = new valuations.Market(marketJson);
/// for (const instr of instruments) {
///   const result = valuations.instruments.priceInstrumentWithMarket(instr, market, "2025-06-15", "default");
/// }
/// ```
#[wasm_bindgen(js_name = Market)]
pub struct JsMarket {
    inner: Arc<MarketContext>,
}

#[wasm_bindgen(js_class = Market)]
impl JsMarket {
    /// Parse a MarketContext from its JSON representation.
    ///
    /// @param json - Canonical MarketContext JSON, the same payload accepted by
    /// pricing `marketJson` arguments.
    /// @returns A `Market` handle that can be reused across pricing calls.
    /// @throws If the JSON is malformed or does not match the MarketContext schema.
    #[wasm_bindgen(constructor)]
    pub fn new(json: JsValue) -> Result<JsMarket, JsValue> {
        let json: &str = &json_text(&json, "json")?;
        let inner: MarketContext = serde_json::from_str(json).map_err(to_js_err)?;
        Ok(JsMarket {
            inner: Arc::new(inner),
        })
    }

    /// Serialize the wrapped MarketContext back to JSON.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the market context cannot be
    /// serialized to JSON.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Access the inner MarketContext (crate-internal).
    pub(crate) fn inner(&self) -> &MarketContext {
        self.inner.as_ref()
    }
}
