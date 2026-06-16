//! WASM bindings for [`finstack_quant_core::currency::Currency`].

use crate::utils::to_js_err;
use finstack_quant_core::currency::Currency as RustCurrency;
use std::str::FromStr;
use wasm_bindgen::prelude::*;

/// ISO-4217 currency code wrapper for JavaScript.
///
/// Currencies parse from three-letter alphabetic codes (case-insensitive).
/// They expose the alphabetic code, the ISO numeric code, and the number of
/// decimal places (minor units) for the currency.
///
/// @example
/// ```javascript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const usd = new core.Currency("USD");
/// usd.code;     // "USD"
/// usd.numeric;  // 840
/// usd.decimals; // 2
/// ```
#[wasm_bindgen(js_name = Currency)]
pub struct Currency {
    #[wasm_bindgen(skip)]
    pub(crate) inner: RustCurrency,
}

#[wasm_bindgen(js_class = Currency)]
impl Currency {
    /// Parse a case-insensitive ISO-4217 alphabetic currency code.
    ///
    /// @param code - Three-letter ISO-4217 code (e.g. `"USD"`, `"eur"`,
    /// `"GBP"`). Leading and trailing whitespace is trimmed.
    /// @returns Constructed `Currency`.
    /// @throws If `code` is not a recognized ISO-4217 alphabetic code.
    ///
    /// @example
    /// ```javascript
    /// const eur = new core.Currency("eur"); // case-insensitive
    /// eur.code; // "EUR"
    /// ```
    #[wasm_bindgen(constructor)]
    pub fn new(code: &str) -> Result<Currency, JsValue> {
        RustCurrency::from_str(code.trim())
            .map(|inner| Currency { inner })
            .map_err(to_js_err)
    }

    /// Three-letter ISO-4217 alphabetic code.
    ///
    /// @returns The uppercase alphabetic code (e.g. `"USD"`).
    #[wasm_bindgen(getter, js_name = code)]
    pub fn code(&self) -> String {
        self.inner.to_string()
    }

    /// ISO-4217 numeric code.
    ///
    /// @returns Numeric code (e.g. `840` for USD, `978` for EUR).
    #[wasm_bindgen(getter, js_name = numeric)]
    pub fn numeric(&self) -> u16 {
        self.inner as u16
    }

    /// Number of decimal places (minor units) for this currency.
    ///
    /// @returns Decimal-place count (e.g. `2` for USD, `0` for JPY).
    #[wasm_bindgen(getter, js_name = decimals)]
    pub fn decimals(&self) -> u8 {
        self.inner.decimals()
    }

    /// Human-readable code (same as `code`).
    ///
    /// @returns The uppercase alphabetic ISO-4217 code.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }

    /// Serialize to a JSON string.
    ///
    /// @returns A JSON string (the ISO-4217 alphabetic code in quotes).
    /// @throws If serialization fails (should not happen for valid `Currency`).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Deserialize from a JSON string produced by `Currency.toJson`.
    ///
    /// @param json - A JSON string containing a quoted ISO-4217 code.
    /// @returns The parsed `Currency`.
    /// @throws If `json` is malformed or contains an unknown code.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str) -> Result<Currency, JsValue> {
        let inner: RustCurrency = serde_json::from_str(json).map_err(to_js_err)?;
        Ok(Currency { inner })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construct_usd() {
        let c = Currency::new("USD").expect("valid");
        assert_eq!(c.code(), "USD");
        assert_eq!(c.to_string(), "USD");
        assert_eq!(c.decimals(), 2);
    }

    #[test]
    fn numeric_code() {
        let c = Currency::new("EUR").expect("valid");
        assert_eq!(c.numeric(), 978);
    }

    #[test]
    fn json_roundtrip() {
        let c = Currency::new("GBP").expect("valid");
        let json = c.to_json().expect("serialize");
        let c2 = Currency::from_json(&json).expect("deserialize");
        assert_eq!(c2.code(), "GBP");
    }

    #[test]
    fn case_insensitive() {
        let c = Currency::new("usd").expect("valid");
        assert_eq!(c.code(), "USD");
    }

    #[test]
    fn multiple_currencies() {
        for code in &["USD", "EUR", "GBP", "JPY", "CHF"] {
            let c = Currency::new(code).expect("valid");
            assert_eq!(c.code(), *code);
            assert_eq!(c.to_string(), *code);
        }
    }

    // -- Boundary tests ------------------------------------------------
    // Error paths through wasm-bindgen create JsValue, which panics on
    // native targets.  Test the underlying Rust types instead.

    #[test]
    fn empty_string_rejected() {
        use std::str::FromStr;
        assert!(RustCurrency::from_str("").is_err());
    }

    #[test]
    fn invalid_code_rejected() {
        use std::str::FromStr;
        assert!(RustCurrency::from_str("XXXX").is_err());
        assert!(RustCurrency::from_str("Z").is_err());
    }

    #[test]
    fn whitespace_trimmed() {
        // Currency::new trims, so "  USD  " should succeed
        use std::str::FromStr;
        assert!(RustCurrency::from_str("USD").is_ok());
    }

    #[test]
    fn from_json_invalid() {
        assert!(serde_json::from_str::<RustCurrency>("not json").is_err());
        assert!(serde_json::from_str::<RustCurrency>("\"ZZZZZ\"").is_err());
    }
}
