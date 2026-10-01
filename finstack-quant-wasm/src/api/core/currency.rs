//! WASM bindings for [`finstack_quant_core::currency::Currency`].

use crate::utils::input::json_text;
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
pub struct JsCurrency {
    pub(crate) inner: RustCurrency,
}

#[wasm_bindgen(js_class = Currency)]
impl JsCurrency {
    /// Parse a case-insensitive ISO-4217 alphabetic currency code.
    ///
    /// @param code - Three-letter ISO-4217 code (e.g. `"USD"`, `"eur"`,
    /// `"GBP"`). Case-insensitive; surrounding whitespace is not trimmed.
    /// @returns Constructed `Currency`.
    /// @throws `TypeError` (kind `invalid_type`) if `code` is not a string;
    /// `FinstackError` (kind `validation`) naming the rejected text if it is not
    /// a supported ISO-4217 alphabetic code (e.g. `" USD "`).
    ///
    /// @example
    /// ```javascript
    /// const eur = new core.Currency("eur"); // case-insensitive
    /// eur.code; // "EUR"
    /// ```
    #[wasm_bindgen(constructor)]
    pub fn new(code: JsValue) -> Result<JsCurrency, JsValue> {
        let code = crate::utils::input::js_string(&code, "code")?;
        RustCurrency::from_str(&code)
            .map(|inner| JsCurrency { inner })
            .map_err(to_js_err)
    }

    /// Look up a currency by its ISO-4217 numeric code (Rust `Currency::try_from`).
    ///
    /// # Arguments
    ///
    /// * `code` - ISO-4217 numeric code, such as `840` for USD or `978` for EUR.
    ///
    /// @returns The matching `Currency`.
    /// @throws `TypeError` (kind `invalid_type`) if `code` is not an integer in
    /// `0..=65535`; `FinstackError` (kind `validation`) if no supported
    /// currency has that numeric code.
    #[wasm_bindgen(js_name = fromNumeric)]
    pub fn from_numeric(code: JsValue) -> Result<JsCurrency, JsValue> {
        let code: u16 = crate::utils::input::js_uint(&code, "code")?;
        RustCurrency::try_from(code)
            .map(|inner| JsCurrency { inner })
            .map_err(|error| to_js_err(error.to_string()))
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
        self.inner.numeric()
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
    pub fn from_json(json: JsValue) -> Result<JsCurrency, JsValue> {
        let json: &str = &json_text(&json, "json")?;
        let inner: RustCurrency = serde_json::from_str(json).map_err(to_js_err)?;
        Ok(JsCurrency { inner })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Native tests cannot build a `JsValue`, so fixtures wrap the Rust
    /// currency directly; argument conversion is covered by the facade tests.
    fn currency(code: &str) -> JsCurrency {
        JsCurrency {
            inner: code.parse().expect("valid currency code"),
        }
    }

    #[test]
    fn construct_usd() {
        let c = currency("USD");
        assert_eq!(c.code(), "USD");
        assert_eq!(c.to_string(), "USD");
        assert_eq!(c.decimals(), 2);
    }

    #[test]
    fn numeric_code() {
        let c = currency("EUR");
        assert_eq!(c.numeric(), 978);
    }

    #[test]
    fn case_insensitive() {
        let c = currency("usd");
        assert_eq!(c.code(), "USD");
    }

    #[test]
    fn multiple_currencies() {
        for code in &["USD", "EUR", "GBP", "JPY", "CHF"] {
            let c = currency(code);
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
    fn whitespace_is_not_trimmed() {
        // `Currency` construction is the Rust parser: untrimmed text is rejected.
        use std::str::FromStr;
        assert!(RustCurrency::from_str("  USD  ").is_err());
    }

    #[test]
    fn from_json_invalid() {
        assert!(serde_json::from_str::<RustCurrency>("not json").is_err());
        assert!(serde_json::from_str::<RustCurrency>("\"ZZZZZ\"").is_err());
    }
}
