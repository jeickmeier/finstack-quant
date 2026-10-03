//! JSON round trip of the WASM `Performance` class.

use super::performance::JsPerformance;
use crate::utils::input::json_text;
use crate::utils::to_js_err;
use finstack_quant_analytics as fa;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_class = Performance)]
impl JsPerformance {
    /// Serialize the full panel state to compact JSON.
    ///
    /// The JSON holds the dates, return panel, ticker names, benchmark index,
    /// frequency and active date window, so `Performance.fromJson` rebuilds an
    /// equivalent handle. Twin of Python `Performance.to_json`.
    /// # Errors
    ///
    /// Rejects a panel that cannot be serialized.
    /// @returns Compact JSON string of the Rust `Performance` panel.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Rebuild a panel from JSON produced by `toJson`.
    ///
    /// Twin of Python `Performance.from_json`.
    /// # Errors
    ///
    /// Rejects JSON that is malformed or does not describe a valid
    /// `Performance` panel.
    /// @param json - `Performance` JSON string produced by `toJson`, or the equivalent parsed object.
    /// @returns A `Performance` handle equivalent to the serialized one.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsPerformance, JsValue> {
        let json: &str = &json_text(&json, "json")?;
        let inner: fa::Performance = serde_json::from_str(json).map_err(to_js_err)?;
        Ok(JsPerformance { inner })
    }
}
