//! Array-named constructors and the JSON round trip of the WASM `Performance` class.
//!
//! Python's `Performance(...)` and `Performance.from_returns(...)` take pandas
//! objects, with the Rust-shaped constructors published as `from_arrays` and
//! `from_returns_arrays`. The WASM constructor and `fromReturns` already take
//! arrays, so `fromArrays` and `fromReturnsArrays` are the same calls under the
//! Python names. `toJson` / `fromJson` are the serde round trip of the Rust
//! `Performance` panel.

use super::performance::JsPerformance;
use crate::utils::input::json_text;
use crate::utils::to_js_err;
use finstack_quant_analytics as fa;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_class = Performance)]
impl JsPerformance {
    /// Construct from a ticker-major, column-oriented price matrix.
    ///
    /// Same call as `new Performance(...)`, under the name of Python
    /// `Performance.from_arrays` (Rust `Performance::new`).
    /// # Errors
    ///
    /// Rejects malformed dates or matrices, invalid prices, unsupported
    /// frequencies, and an unknown benchmark ticker.
    /// @param dates - ISO-8601 observation dates in ascending order, with one entry per value in each inner price series.
    /// @param prices - Ticker-major, column-oriented matrix where `prices[tickerIdx][dateIdx]` is the price for `tickerIdx` at `dates[dateIdx]`.
    /// @param ticker_names - Ticker labels aligned with the outer elements of `prices`.
    /// @param benchmark_ticker - Optional ticker label to use as the benchmark return series.
    /// @param frequency - Optional observation frequency token; defaults to daily.
    /// @returns A `Performance` handle over the returns derived from the price panel.
    #[wasm_bindgen(js_name = fromArrays)]
    pub fn from_arrays(
        dates: JsValue,
        prices: JsValue,
        ticker_names: JsValue,
        benchmark_ticker: Option<JsValue>,
        frequency: Option<JsValue>,
    ) -> Result<JsPerformance, JsValue> {
        Self::new(dates, prices, ticker_names, benchmark_ticker, frequency)
    }

    /// Construct from a ticker-major, column-oriented return matrix.
    ///
    /// Same call as `Performance.fromReturns(...)`, under the name of Python
    /// `Performance.from_returns_arrays` (Rust `Performance::from_returns`).
    /// # Errors
    ///
    /// Rejects malformed dates or matrices and invalid benchmark or
    /// frequency inputs.
    /// @param dates - ISO-8601 observation dates in ascending order, with one entry per value in each inner return series.
    /// @param returns - Ticker-major, column-oriented simple decimal return matrix where `returns[tickerIdx][dateIdx]` is the return for `tickerIdx` at `dates[dateIdx]`.
    /// @param ticker_names - Ticker labels aligned with the outer elements of `returns`.
    /// @param benchmark_ticker - Optional ticker label to use as the benchmark return series.
    /// @param frequency - Optional observation frequency token; defaults to daily.
    /// @returns A `Performance` handle over the supplied return panel.
    #[wasm_bindgen(js_name = fromReturnsArrays)]
    pub fn from_returns_arrays(
        dates: JsValue,
        returns: JsValue,
        ticker_names: JsValue,
        benchmark_ticker: Option<JsValue>,
        frequency: Option<JsValue>,
    ) -> Result<JsPerformance, JsValue> {
        Self::from_returns(dates, returns, ticker_names, benchmark_ticker, frequency)
    }

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
