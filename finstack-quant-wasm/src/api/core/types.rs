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
/// Use the `fromPercent` or `fromBps` factories to avoid scaling errors
/// when working with quoted rates.
///
/// @example
/// ```javascript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const r = core.Rate.fromBps(250);     // 2.5% as 250 bps
/// r.asDecimal;  // 0.025
/// r.asPercent;  // 2.5
/// r.asBps;      // 250
/// ```
#[wasm_bindgen(js_name = Rate)]
pub struct Rate {
    #[wasm_bindgen(skip)]
    pub(crate) inner: RustRate,
}

#[wasm_bindgen(js_class = Rate)]
impl Rate {
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
    pub fn new(decimal: f64) -> Result<Rate, JsValue> {
        RustRate::try_from_decimal(decimal)
            .map(|inner| Rate { inner })
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
    pub fn from_percent(pct: f64) -> Result<Rate, JsValue> {
        if !pct.is_finite() {
            return Err(to_js_err("percent must be finite"));
        }
        Ok(Rate {
            inner: RustRate::from_percent(pct),
        })
    }

    /// Create a rate from basis points.
    ///
    /// The canonical Rust `Rate::from_bps` takes an integer (`i32`) number
    /// of basis points. Because JavaScript numbers are `f64`, this binding
    /// accepts a float and rounds it to the nearest integer basis point
    /// before delegating (banker-free half-away rounding via `Bps`).
    /// Fractional inputs therefore lose sub-bp precision; use
    /// `new Rate(decimal)` or `Rate.fromPercent` for sub-bp rates.
    ///
    /// @param bps - Rate in basis points (e.g. `500` for 5%). Rounded to the
    /// nearest integer bp.
    /// @returns The constructed `Rate`.
    /// @throws If `bps` is non-finite.
    ///
    /// @example
    /// ```javascript
    /// const r = core.Rate.fromBps(250);  // 2.5%
    /// r.asDecimal;  // 0.025
    /// ```
    #[wasm_bindgen(js_name = fromBps)]
    pub fn from_bps(bps: f64) -> Result<Rate, JsValue> {
        let b = RustBps::try_new(bps).map_err(to_js_err)?;
        Ok(Rate { inner: b.as_rate() })
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
    /// @returns Rate in bps.
    #[wasm_bindgen(getter, js_name = asBps)]
    pub fn as_bps(&self) -> i32 {
        self.inner.as_bps()
    }
}

/// Basis points (1 bp = 0.01%, 10_000 bps = 100%).
///
/// Stored as integer bps internally; constructors round to the nearest bp.
///
/// @example
/// ```javascript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const spread = new core.Bps(125);
/// spread.asDecimal();  // 0.0125
/// spread.asBps();      // 125
/// ```
#[wasm_bindgen(js_name = Bps)]
pub struct Bps {
    #[wasm_bindgen(skip)]
    pub(crate) inner: RustBps,
}

#[wasm_bindgen(js_class = Bps)]
impl Bps {
    /// Create basis points from a floating value.
    ///
    /// @param value - Value in basis points (e.g. `25` for 25 bps). Rounded
    /// to the nearest integer bp.
    /// @returns The constructed `Bps`.
    /// @throws If `value` is non-finite.
    #[wasm_bindgen(constructor)]
    pub fn new(value: f64) -> Result<Bps, JsValue> {
        RustBps::try_new(value)
            .map(|inner| Bps { inner })
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
    /// @returns Integer bps.
    #[wasm_bindgen(js_name = asBps)]
    pub fn as_bps(&self) -> i32 {
        self.inner.as_bps()
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
pub struct Percentage {
    #[wasm_bindgen(skip)]
    pub(crate) inner: RustPercentage,
}

#[wasm_bindgen(js_class = Percentage)]
impl Percentage {
    /// Create a percentage.
    ///
    /// @param value - Value in percent (e.g. `5.0` for 5%).
    /// @returns The constructed `Percentage`.
    /// @throws If `value` is non-finite.
    #[wasm_bindgen(constructor)]
    pub fn new(value: f64) -> Result<Percentage, JsValue> {
        RustPercentage::try_new(value)
            .map(|inner| Percentage { inner })
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
        let r = Rate::new(0.05).expect("valid");
        assert!((r.as_decimal() - 0.05).abs() < 1e-12);
        assert!((r.as_percent() - 5.0).abs() < 1e-10);
        assert_eq!(r.as_bps(), 500);
    }

    #[test]
    fn rate_from_percent() {
        let r = Rate::from_percent(5.0).expect("valid");
        assert!((r.as_decimal() - 0.05).abs() < 1e-12);
    }

    #[test]
    fn rate_from_bps() {
        let r = Rate::from_bps(250.0).expect("valid");
        assert!((r.as_decimal() - 0.025).abs() < 1e-10);
        assert_eq!(r.as_bps(), 250);
    }

    #[test]
    fn bps_roundtrip() {
        let b = Bps::new(25.0).expect("valid");
        assert!((b.as_decimal() - 0.0025).abs() < 1e-10);
        assert_eq!(b.as_bps(), 25);
    }

    #[test]
    fn percentage_roundtrip() {
        let p = Percentage::new(5.0).expect("valid");
        assert!((p.as_decimal() - 0.05).abs() < 1e-12);
        assert!((p.as_percent() - 5.0).abs() < 1e-12);
    }

    #[test]
    fn rate_zero() {
        let r = Rate::new(0.0).expect("valid");
        assert_eq!(r.as_decimal(), 0.0);
        assert_eq!(r.as_percent(), 0.0);
        assert_eq!(r.as_bps(), 0);
    }

    #[test]
    fn bps_large_value() {
        let b = Bps::new(10_000.0).expect("valid");
        assert!((b.as_decimal() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn percentage_zero() {
        let p = Percentage::new(0.0).expect("valid");
        assert_eq!(p.as_decimal(), 0.0);
    }

    #[test]
    fn rate_negative() {
        let r = Rate::new(-0.01).expect("valid");
        assert!((r.as_decimal() - (-0.01)).abs() < 1e-12);
    }

    // -- Boundary tests ------------------------------------------------
    // Error paths through wasm-bindgen create JsValue, which panics on
    // native targets.  Test the underlying Rust types instead.

    #[test]
    fn rate_rejects_nan() {
        assert!(RustRate::try_from_decimal(f64::NAN).is_err());
    }

    #[test]
    fn rate_rejects_infinity() {
        assert!(RustRate::try_from_decimal(f64::INFINITY).is_err());
    }

    #[test]
    fn bps_rejects_nan() {
        assert!(RustBps::try_new(f64::NAN).is_err());
    }

    #[test]
    fn percentage_rejects_nan() {
        assert!(RustPercentage::try_new(f64::NAN).is_err());
    }
}
