//! WASM bindings for [`finstack_quant_core::types`] rate helpers (`Rate`, `Bps`, `Percentage`).

use crate::utils::input::{from_js_json, js_f64, js_string};
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
    pub fn new(decimal: JsValue) -> Result<JsRate, JsValue> {
        let decimal = js_f64(&decimal, "decimal")?;
        RustRate::from_decimal(decimal)
            .map(|inner| JsRate { inner })
            .map_err(to_js_err)
    }

    /// Create a rate from a percent figure.
    ///
    /// @param percent - Percent value (e.g. `5.0` for 5%).
    /// @returns The constructed `Rate`.
    /// @throws If `percent` is non-finite.
    ///
    /// @example
    /// ```javascript
    /// const r = core.Rate.fromPercent(5.0);
    /// r.asDecimal;  // 0.05
    /// ```
    #[wasm_bindgen(js_name = fromPercent)]
    pub fn from_percent(percent: JsValue) -> Result<JsRate, JsValue> {
        let percent = js_f64(&percent, "percent")?;
        RustRate::from_percent(percent)
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
    pub fn from_bp(bp: JsValue) -> Result<JsRate, JsValue> {
        let bp = js_f64(&bp, "bp")?;
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

    /// Parse a rate quote through Rust `Rate::from_str` (the twin of Python `Rate(text)`).
    ///
    /// # Arguments
    ///
    /// * `text` - Quote text: a decimal (`"0.05"`), a percent (`"5%"`) or
    ///   basis points (`"25bp"`, fractional bp allowed).
    ///
    /// @returns The parsed `Rate`.
    /// @throws `TypeError` if `text` is not a string; `FinstackError` (kind
    /// `validation`) if it is not a recognised rate quote.
    ///
    /// @example
    /// ```javascript
    /// core.Rate.parse("12.5bp").asDecimal;  // 0.00125
    /// ```
    #[wasm_bindgen(js_name = parse)]
    pub fn parse(text: JsValue) -> Result<JsRate, JsValue> {
        let text = js_string(&text, "text")?;
        text.parse::<RustRate>()
            .map(|inner| JsRate { inner })
            .map_err(to_js_err)
    }

    /// The rate as `Bps`, rounded to the nearest whole basis point.
    #[wasm_bindgen(getter, js_name = asBasisPoints)]
    pub fn as_basis_points(&self) -> JsBps {
        JsBps {
            inner: RustBps::from(self.inner),
        }
    }

    /// The rate as a `Percentage`.
    #[wasm_bindgen(getter, js_name = asPercentage)]
    pub fn as_percentage(&self) -> JsPercentage {
        JsPercentage {
            inner: RustPercentage::from(self.inner),
        }
    }

    /// Absolute value (Rust `Rate::abs`).
    ///
    /// @returns A new `Rate` with the sign removed.
    #[wasm_bindgen(js_name = abs)]
    pub fn abs(&self) -> JsRate {
        JsRate {
            inner: self.inner.abs(),
        }
    }

    /// Whether the value is exactly zero.
    ///
    /// @returns `true` when the rate is zero.
    #[wasm_bindgen(js_name = isZero)]
    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// Whether the value is strictly positive.
    ///
    /// @returns `true` when the rate is above zero.
    #[wasm_bindgen(js_name = isPositive)]
    pub fn is_positive(&self) -> bool {
        self.inner.is_positive()
    }

    /// Whether the value is strictly negative.
    ///
    /// @returns `true` when the rate is below zero.
    #[wasm_bindgen(js_name = isNegative)]
    pub fn is_negative(&self) -> bool {
        self.inner.is_negative()
    }

    /// Serialize to the canonical JSON wire form shared with Python `Rate.to_json`.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid value).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Deserialize from the canonical JSON wire form produced by `toJson`.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical `Rate` JSON text (or the equivalent plain value).
    ///
    /// @returns The parsed `Rate`.
    /// @throws If `json` is malformed or holds an invalid value.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsRate, JsValue> {
        from_js_json::<RustRate>(&json, "json").map(|inner| JsRate { inner })
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
/// spread.asDecimal;  // 0.0125
/// spread.asBp;       // 125
/// spread.asRate.asPercent;  // 1.25
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
    /// @param bp - Value in whole basis points (e.g. `25` for 25 bp).
    /// @returns The constructed `Bps`.
    /// @throws If `bp` is non-finite or not a whole number of basis
    /// points. Sub-bp spreads must use the JSON instrument path (which
    /// preserves fractional values) or a decimal `Rate`.
    #[wasm_bindgen(constructor)]
    pub fn new(bp: JsValue) -> Result<JsBps, JsValue> {
        let bp = js_f64(&bp, "bp")?;
        RustBps::try_new(bp)
            .map(|inner| JsBps { inner })
            .map_err(to_js_err)
    }

    /// Value as a decimal (e.g. 25 bp → 0.0025).
    ///
    /// @returns Decimal equivalent.
    #[wasm_bindgen(getter, js_name = asDecimal)]
    pub fn as_decimal(&self) -> f64 {
        self.inner.as_decimal()
    }

    /// Value in whole basis points.
    ///
    /// @returns Integer bp.
    #[wasm_bindgen(getter, js_name = asBp)]
    pub fn as_bp(&self) -> i32 {
        self.inner.as_bp()
    }

    /// Value in percent (e.g. 25 bp → 0.25).
    #[wasm_bindgen(getter, js_name = asPercent)]
    pub fn as_percent(&self) -> f64 {
        self.inner.as_percent()
    }

    /// Value as a decimal `Rate`.
    #[wasm_bindgen(getter, js_name = asRate)]
    pub fn as_rate(&self) -> JsRate {
        JsRate {
            inner: self.inner.as_rate(),
        }
    }

    /// Value as a `Percentage`.
    #[wasm_bindgen(getter, js_name = asPercentage)]
    pub fn as_percentage(&self) -> JsPercentage {
        JsPercentage {
            inner: RustPercentage::from(self.inner),
        }
    }

    /// Absolute value (Rust `Bps::abs`).
    ///
    /// @returns A new `Bps` with the sign removed.
    #[wasm_bindgen(js_name = abs)]
    pub fn abs(&self) -> JsBps {
        JsBps {
            inner: self.inner.abs(),
        }
    }

    /// Whether the value is exactly zero.
    ///
    /// @returns `true` when the basis-point value is zero.
    #[wasm_bindgen(js_name = isZero)]
    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// Whether the value is strictly positive.
    ///
    /// @returns `true` when the basis-point value is above zero.
    #[wasm_bindgen(js_name = isPositive)]
    pub fn is_positive(&self) -> bool {
        self.inner.is_positive()
    }

    /// Whether the value is strictly negative.
    ///
    /// @returns `true` when the basis-point value is below zero.
    #[wasm_bindgen(js_name = isNegative)]
    pub fn is_negative(&self) -> bool {
        self.inner.is_negative()
    }

    /// Serialize to the canonical JSON wire form shared with Python `Bps.to_json`.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid value).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Deserialize from the canonical JSON wire form produced by `toJson`.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical `Bps` JSON text (or the equivalent plain value).
    ///
    /// @returns The parsed `Bps`.
    /// @throws If `json` is malformed or holds an invalid value.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsBps, JsValue> {
        from_js_json::<RustBps>(&json, "json").map(|inner| JsBps { inner })
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
/// p.asDecimal;  // 0.05
/// p.asPercent;  // 5
/// p.asBp;       // 500
/// ```
#[wasm_bindgen(js_name = Percentage)]
pub struct JsPercentage {
    pub(crate) inner: RustPercentage,
}

#[wasm_bindgen(js_class = Percentage)]
impl JsPercentage {
    /// Create a percentage.
    ///
    /// @param percent - Value in percent (e.g. `5.0` for 5%).
    /// @returns The constructed `Percentage`.
    /// @throws If `percent` is non-finite.
    #[wasm_bindgen(constructor)]
    pub fn new(percent: JsValue) -> Result<JsPercentage, JsValue> {
        let percent = js_f64(&percent, "percent")?;
        RustPercentage::new(percent)
            .map(|inner| JsPercentage { inner })
            .map_err(to_js_err)
    }

    /// Value as a decimal (5% → 0.05).
    ///
    /// @returns Decimal equivalent.
    #[wasm_bindgen(getter, js_name = asDecimal)]
    pub fn as_decimal(&self) -> f64 {
        self.inner.as_decimal()
    }

    /// Value in percent points.
    ///
    /// @returns Percent value.
    #[wasm_bindgen(getter, js_name = asPercent)]
    pub fn as_percent(&self) -> f64 {
        self.inner.as_percent()
    }

    /// Value in basis points, rounded to the nearest integer (e.g. 17.5% → 1750).
    #[wasm_bindgen(getter, js_name = asBp)]
    pub fn as_bp(&self) -> i32 {
        self.inner.as_bp()
    }

    /// Value as a decimal `Rate`.
    #[wasm_bindgen(getter, js_name = asRate)]
    pub fn as_rate(&self) -> JsRate {
        JsRate {
            inner: self.inner.as_rate(),
        }
    }

    /// Value as `Bps`, rounded to the nearest whole basis point.
    #[wasm_bindgen(getter, js_name = asBasisPoints)]
    pub fn as_basis_points(&self) -> JsBps {
        JsBps {
            inner: RustBps::from(self.inner),
        }
    }

    /// Absolute value (Rust `Percentage::abs`).
    ///
    /// @returns A new `Percentage` with the sign removed.
    #[wasm_bindgen(js_name = abs)]
    pub fn abs(&self) -> JsPercentage {
        JsPercentage {
            inner: self.inner.abs(),
        }
    }

    /// Whether the value is exactly zero.
    ///
    /// @returns `true` when the percentage is zero.
    #[wasm_bindgen(js_name = isZero)]
    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// Whether the value is strictly positive.
    ///
    /// @returns `true` when the percentage is above zero.
    #[wasm_bindgen(js_name = isPositive)]
    pub fn is_positive(&self) -> bool {
        self.inner.is_positive()
    }

    /// Whether the value is strictly negative.
    ///
    /// @returns `true` when the percentage is below zero.
    #[wasm_bindgen(js_name = isNegative)]
    pub fn is_negative(&self) -> bool {
        self.inner.is_negative()
    }

    /// Serialize to the canonical JSON wire form shared with Python `Percentage.to_json`.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid value).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Deserialize from the canonical JSON wire form produced by `toJson`.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical `Percentage` JSON text (or the equivalent plain value).
    ///
    /// @returns The parsed `Percentage`.
    /// @throws If `json` is malformed or holds an invalid value.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsPercentage, JsValue> {
        from_js_json::<RustPercentage>(&json, "json").map(|inner| JsPercentage { inner })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn cross_conversions_use_the_rust_from_impls() {
        let pct = JsPercentage {
            inner: RustPercentage::new(0.175).expect("pct"),
        };
        assert_eq!(pct.as_bp(), 18);
        assert_eq!(pct.as_basis_points().as_bp(), 18);
        let bps = JsBps {
            inner: RustBps::try_new(-1995.0).expect("bps"),
        };
        assert!((bps.as_percent() + 19.95).abs() < 1e-12);
        assert!((bps.as_rate().as_decimal() + 0.1995).abs() < 1e-12);
        assert!(bps.is_negative() && bps.abs().is_positive());
        let rate = JsRate {
            inner: "12.5bp".parse::<RustRate>().expect("fractional bp quote"),
        };
        assert!((rate.as_decimal() - 0.00125).abs() < 1e-15);
        assert!((rate.as_percentage().as_percent() - 0.125).abs() < 1e-12);
    }

    #[test]
    fn json_round_trips_through_rust_serde() {
        let rate = JsRate {
            inner: RustRate::from_decimal(0.0525).expect("rate"),
        };
        let back: RustRate = serde_json::from_str(&rate.to_json().expect("json")).expect("parse");
        assert_eq!(back, rate.inner);
    }
}
