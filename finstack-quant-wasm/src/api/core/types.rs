//! WASM bindings for [`finstack_quant_core::types`] rate helpers (`Rate`, `Bps`, `Percentage`).

use crate::utils::to_js_err;
use finstack_quant_core::types::{Bps as RustBps, Percentage as RustPercentage, Rate as RustRate};
use wasm_bindgen::prelude::*;

/// Interest or discount rate stored as a decimal (e.g. `0.05` is 5%).
///
/// Conventions:
/// - **Decimal**: `0.05` represents 5%.
/// - **Percent**: `5.0` represents 5%.
/// - **Basis points**: `500` represents 5% (1 bp = 0.01%).
///
/// Use the `fromPercent` or `fromBp` factories to avoid scaling errors
/// when working with quoted rates.
///
/// @example
/// ```javascript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const r = core.Rate.fromBp(250);     // 2.5% as 250 bp
/// r.asDecimal;  // 0.025
/// r.asPercent;  // 2.5
/// r.asBp;      // 250
/// ```
#[wasm_bindgen(js_name = Rate)]
pub struct JsRate {
    pub(crate) inner: RustRate,
}

#[wasm_bindgen(js_class = Rate)]
impl JsRate {
    /// Create a rate from a decimal value.
    ///
    /// @param decimal - Rate as a decimal (e.g. `0.05` for 5%).
    /// @returns The constructed `Rate`.
    /// @throws If `decimal` is non-finite (NaN, ±∞).
    ///
    /// @example
    /// ```javascript
    /// const r = new core.Rate(0.05);  // 5%
    /// r.asPercent;  // 5
    /// ```
    #[wasm_bindgen(constructor)]
    pub fn new(decimal: f64) -> Result<JsRate, JsValue> {
        RustRate::from_decimal(decimal)
            .map(|inner| JsRate { inner })
            .map_err(to_js_err)
    }

    /// Create a rate from a percent figure.
    ///
    /// @param pct - Percent value (e.g. `5.0` for 5%).
    /// @returns The constructed `Rate`.
    /// @throws If `pct` is non-finite.
    ///
    /// @example
    /// ```javascript
    /// const r = core.Rate.fromPercent(5.0);
    /// r.asDecimal;  // 0.05
    /// ```
    #[wasm_bindgen(js_name = fromPercent)]
    pub fn from_percent(pct: f64) -> Result<JsRate, JsValue> {
        RustRate::from_percent(pct)
            .map(|inner| JsRate { inner })
            .map_err(to_js_err)
    }

    /// Create a rate from a whole number of basis points.
    ///
    /// The canonical Rust `Bps::try_new` is integer-backed and **rejects
    /// fractional input** rather than silently rounding it: a sub-bp rate
    /// quietly rounded to whole bp is a pricing bug, not a convenience.
    /// Use `new Rate(decimal)` or `Rate.fromPercent` for sub-bp rates.
    ///
    /// @param bp - Rate in whole basis points (e.g. `500` for 5%).
    /// @returns The constructed `Rate`.
    /// @throws If `bp` is non-finite or not a whole number of basis points.
    ///
    /// @example
    /// ```javascript
    /// const r = core.Rate.fromBp(250);  // 2.5%
    /// r.asDecimal;  // 0.025
    /// ```
    #[wasm_bindgen(js_name = fromBp)]
    pub fn from_bp(bp: f64) -> Result<JsRate, JsValue> {
        let b = RustBps::try_new(bp).map_err(to_js_err)?;
        Ok(JsRate { inner: b.as_rate() })
    }

    /// Rate as a decimal (e.g. `0.05` for 5%).
    ///
    /// @returns Decimal rate.
    #[wasm_bindgen(getter, js_name = asDecimal)]
    pub fn as_decimal(&self) -> f64 {
        self.inner.as_decimal()
    }

    /// Rate as a percent (e.g. `5.0` for 5%).
    ///
    /// @returns Percent rate.
    #[wasm_bindgen(getter, js_name = asPercent)]
    pub fn as_percent(&self) -> f64 {
        self.inner.as_percent()
    }

    /// Rate in basis points, rounded to the nearest integer (e.g. `500` for 5%).
    ///
    /// @returns Rate in bp.
    #[wasm_bindgen(getter, js_name = asBp)]
    pub fn as_bp(&self) -> i32 {
        self.inner.as_bp()
    }
}

/// Basis points (1 bp = 0.01%, 10_000 bp = 100%).
///
/// Stored as integer bp internally; constructors reject fractional input.
///
/// @example
/// ```javascript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const spread = new core.Bps(125);
/// spread.asDecimal();  // 0.0125
/// spread.asBp();      // 125
/// ```
#[wasm_bindgen(js_name = Bps)]
pub struct JsBps {
    pub(crate) inner: RustBps,
}

#[wasm_bindgen(js_class = Bps)]
impl JsBps {
    /// Create basis points from a whole-number value.
    ///
    /// Delegates to the canonical Rust `Bps::try_new`, which rejects
    /// fractional basis points.
    ///
    /// @param value - Value in whole basis points (e.g. `25` for 25 bp).
    /// @returns The constructed `Bps`.
    /// @throws If `value` is non-finite or not a whole number of basis
    /// points. Sub-bp spreads must use the JSON instrument path (which
    /// preserves fractional values) or a decimal `Rate`.
    #[wasm_bindgen(constructor)]
    pub fn new(value: f64) -> Result<JsBps, JsValue> {
        RustBps::try_new(value)
            .map(|inner| JsBps { inner })
            .map_err(to_js_err)
    }

    /// Value as a decimal (e.g. 25 bp → 0.0025).
    ///
    /// @returns Decimal equivalent.
    #[wasm_bindgen(js_name = asDecimal)]
    pub fn as_decimal(&self) -> f64 {
        self.inner.as_decimal()
    }

    /// Value in whole basis points.
    ///
    /// @returns Integer bp.
    #[wasm_bindgen(js_name = asBp)]
    pub fn as_bp(&self) -> i32 {
        self.inner.as_bp()
    }
}

/// Percentage stored in percent points (`5.0` means 5%).
///
/// Use this when you want the API to be explicit that the value is in
/// percent (rather than decimal). Equivalent to `Rate` for arithmetic.
///
/// @example
/// ```javascript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const p = new core.Percentage(5.0);
/// p.asDecimal();  // 0.05
/// p.asPercent();  // 5
/// ```
#[wasm_bindgen(js_name = Percentage)]
pub struct JsPercentage {
    pub(crate) inner: RustPercentage,
}

#[wasm_bindgen(js_class = Percentage)]
impl JsPercentage {
    /// Create a percentage.
    ///
    /// @param value - Value in percent (e.g. `5.0` for 5%).
    /// @returns The constructed `Percentage`.
    /// @throws If `value` is non-finite.
    #[wasm_bindgen(constructor)]
    pub fn new(value: f64) -> Result<JsPercentage, JsValue> {
        RustPercentage::new(value)
            .map(|inner| JsPercentage { inner })
            .map_err(to_js_err)
    }

    /// Value as a decimal (5% → 0.05).
    ///
    /// @returns Decimal equivalent.
    #[wasm_bindgen(js_name = asDecimal)]
    pub fn as_decimal(&self) -> f64 {
        self.inner.as_decimal()
    }

    /// Value in percent points.
    ///
    /// @returns Percent value.
    #[wasm_bindgen(js_name = asPercent)]
    pub fn as_percent(&self) -> f64 {
        self.inner.as_percent()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_new_roundtrip() {
        let r = JsRate::new(0.05).expect("valid");
        assert!((r.as_decimal() - 0.05).abs() < 1e-12);
        assert!((r.as_percent() - 5.0).abs() < 1e-10);
        assert_eq!(r.as_bp(), 500);
    }

    #[test]
    fn rate_from_percent() {
        let r = JsRate::from_percent(5.0).expect("valid");
        assert!((r.as_decimal() - 0.05).abs() < 1e-12);
    }

    #[test]
    fn rate_from_bp() {
        let r = JsRate::from_bp(250.0).expect("valid");
        assert!((r.as_decimal() - 0.025).abs() < 1e-10);
        assert_eq!(r.as_bp(), 250);
    }

    #[test]
    fn bp_roundtrip() {
        let b = JsBps::new(25.0).expect("valid");
        assert!((b.as_decimal() - 0.0025).abs() < 1e-10);
        assert_eq!(b.as_bp(), 25);
    }

    #[test]
    fn percentage_roundtrip() {
        let p = JsPercentage::new(5.0).expect("valid");
        assert!((p.as_decimal() - 0.05).abs() < 1e-12);
        assert!((p.as_percent() - 5.0).abs() < 1e-12);
    }

    #[test]
    fn rate_zero() {
        let r = JsRate::new(0.0).expect("valid");
        assert_eq!(r.as_decimal(), 0.0);
        assert_eq!(r.as_percent(), 0.0);
        assert_eq!(r.as_bp(), 0);
    }

    #[test]
    fn bp_large_value() {
        let b = JsBps::new(10_000.0).expect("valid");
        assert!((b.as_decimal() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn percentage_zero() {
        let p = JsPercentage::new(0.0).expect("valid");
        assert_eq!(p.as_decimal(), 0.0);
    }

    #[test]
    fn rate_negative() {
        let r = JsRate::new(-0.01).expect("valid");
        assert!((r.as_decimal() - (-0.01)).abs() < 1e-12);
    }

    // -- Boundary tests ------------------------------------------------
    // Error paths through wasm-bindgen create JsValue, which panics on
    // native targets.  Test the underlying Rust types instead.

    #[test]
    fn rate_rejects_nan() {
        assert!(RustRate::from_decimal(f64::NAN).is_err());
    }

    #[test]
    fn rate_rejects_infinity() {
        assert!(RustRate::from_decimal(f64::INFINITY).is_err());
    }

    #[test]
    fn bp_rejects_nan() {
        assert!(RustBps::try_new(f64::NAN).is_err());
    }

    #[test]
    fn bp_rejects_fractional() {
        assert!(RustBps::try_new(62.5).is_err());
    }

    #[test]
    fn percentage_rejects_nan() {
        assert!(RustPercentage::new(f64::NAN).is_err());
    }
}
