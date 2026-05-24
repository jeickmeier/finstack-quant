//! WASM market handle — parse MarketContext once, reuse across pricing calls.
//!
//! Avoids repeated `serde_json::from_str` on the full MarketContext JSON
//! in bulk-pricing and sensitivity-sweep workloads.

use crate::utils::to_js_err;
use finstack_core::market_data::context::MarketContext;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// Opaque handle wrapping a parsed [`MarketContext`].
///
/// Construct once from JSON, then pass to `priceInstrumentWithMarket`,
/// `priceInstrumentWithMetricsAndMarket`, etc.  Eliminates the per-call
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
pub struct Market {
    inner: Arc<MarketContext>,
}

#[wasm_bindgen(js_class = Market)]
impl Market {
    /// Parse a MarketContext from its JSON representation.
    ///
    /// @param json - MarketContext JSON string.
    /// @returns A `Market` handle that can be reused across pricing calls.
    /// @throws If the JSON is invalid.
    #[wasm_bindgen(constructor)]
    pub fn new(json: &str) -> Result<Market, JsValue> {
        let inner: MarketContext = serde_json::from_str(json).map_err(to_js_err)?;
        Ok(Market {
            inner: Arc::new(inner),
        })
    }

    /// Serialize the wrapped MarketContext back to JSON.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Access the inner MarketContext (crate-internal).
    pub(crate) fn inner(&self) -> &MarketContext {
        self.inner.as_ref()
    }
}
