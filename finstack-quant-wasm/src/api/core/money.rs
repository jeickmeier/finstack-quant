//! WASM bindings for [`finstack_quant_core::money::Money`].

use crate::api::core::currency::JsCurrency;
use crate::utils::input::{
    invalid_type, js_f64, js_opt_bool, js_opt_string, js_opt_uint, js_string, json_text,
};
use crate::utils::to_js_err;
use finstack_quant_core::config::RoundingMode;
use finstack_quant_core::money::{FormatOpts, Money as RustMoney};
use wasm_bindgen::prelude::*;

/// Currency-tagged monetary amount.
///
/// Money values pin a numeric amount to a [`JsCurrency`]. The arithmetic
/// methods carry the Rust names: `checkedAdd` / `checkedSub` refuse to mix
/// currencies; `checkedMulF64` / `checkedDivF64` scale by a number and keep
/// the currency; `checkedNeg` negates exactly.
///
/// @example
/// ```javascript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const usd = new core.Currency("USD");
/// const total = new core.Money(1_000_000, usd);
/// const fee   = new core.Money(50, usd);
/// const net   = total.checkedSub(fee);          // Money { amount: 999950, currency: USD }
/// const tax   = net.checkedMulF64(0.07);        // 7% of net
/// console.log(net.toString(), tax.toString());  // "USD 999950.00", "USD 69996.50"
/// ```
#[wasm_bindgen(js_name = Money)]
pub struct JsMoney {
    pub(crate) inner: RustMoney,
}

#[wasm_bindgen(js_class = Money)]
impl JsMoney {
    /// Creates a new money value without implicit currency-minor-unit rounding.
    ///
    /// WASM accepts a JavaScript `number` only. Its finite numeric value is
    /// converted to Rust `Decimal` and stored without currency-minor-unit
    /// rounding; precision already absent from the input `number` cannot be
    /// recovered. Formatting does not mutate the stored amount.
    ///
    /// @param amount - Numeric amount in major units (must be finite).
    /// @param currency - ISO-4217 Currency object that tags the amount and controls arithmetic compatibility.
    /// @returns The constructed `Money`.
    /// @throws If `amount` is non-finite (NaN, ±∞) or cannot be represented as a `Decimal`.
    ///
    /// @example
    /// ```javascript
    /// const usd = new core.Currency("USD");
    /// const m = new core.Money(1234.56, usd);
    /// m.amount;          // 1234.56
    /// m.currency.code;   // "USD"
    /// ```
    #[wasm_bindgen(constructor)]
    pub fn new(amount: JsValue, currency: &JsCurrency) -> Result<JsMoney, JsValue> {
        let amount = js_f64(&amount, "amount")?;
        RustMoney::new(amount, currency.inner)
            .map(|inner| JsMoney { inner })
            .map_err(to_js_err)
    }

    /// Numeric amount in major units as `f64`.
    ///
    /// The Rust core stores money as `Decimal`; this getter exposes the finite
    /// JavaScript number view for interop.
    ///
    /// @returns Amount in major units (e.g. dollars, not cents).
    #[wasm_bindgen(getter, js_name = amount)]
    pub fn amount(&self) -> f64 {
        self.inner.amount()
    }

    /// Lossless amount as a decimal string (e.g. `"1234.56"`).
    ///
    /// Renders the internal Rust `Decimal` directly, so no `f64` round-trip
    /// occurs. Parse with a JavaScript decimal library for exact arithmetic.
    ///
    /// @returns The exact decimal amount as a string.
    #[wasm_bindgen(getter, js_name = amountDecimal)]
    pub fn amount_decimal(&self) -> String {
        self.inner.amount_decimal().to_string()
    }

    /// Currency of this amount.
    ///
    /// @returns The [`JsCurrency`] this amount is tagged with.
    #[wasm_bindgen(getter, js_name = currency)]
    pub fn currency(&self) -> JsCurrency {
        JsCurrency {
            inner: self.inner.currency(),
        }
    }

    /// Convert using an already-resolved positive FX rate.
    /// @param target - Target Currency for the converted monetary amount.
    /// @param rate - FX conversion rate expressed as target-currency units per source-currency unit.
    ///
    /// # Errors
    ///
    /// For a different target currency, throws a JavaScript exception if `rate`
    /// is non-finite or not strictly positive, or if the converted amount cannot
    /// be represented as a decimal.
    #[wasm_bindgen(js_name = convertAtRate)]
    pub fn convert_at_rate(&self, target: &JsCurrency, rate: JsValue) -> Result<JsMoney, JsValue> {
        let rate = js_f64(&rate, "rate")?;
        self.inner
            .convert_at_rate(target.inner, rate)
            .map(|inner| JsMoney { inner })
            .map_err(to_js_err)
    }

    /// Add two amounts (Rust `Money::checked_add`).
    ///
    /// @param other - Another `Money` value.
    /// @returns Sum, in the same currency.
    /// @throws If `other.currency` differs from `this.currency`, or the
    /// operation is not representable as a `Decimal`.
    ///
    /// @example
    /// ```javascript
    /// const usd = new core.Currency("USD");
    /// const a = new core.Money(10, usd);
    /// const b = new core.Money(5, usd);
    /// a.checkedAdd(b).amount;  // 15
    /// ```
    #[wasm_bindgen(js_name = checkedAdd)]
    pub fn checked_add(&self, other: &JsMoney) -> Result<JsMoney, JsValue> {
        self.inner
            .checked_add(other.inner)
            .map(|inner| JsMoney { inner })
            .map_err(to_js_err)
    }

    /// Subtract two amounts (Rust `Money::checked_sub`).
    ///
    /// @param other - Another `Money` value.
    /// @returns Difference, in the same currency.
    /// @throws If `other.currency` differs from `this.currency`, or the
    /// operation is not representable as a `Decimal`.
    #[wasm_bindgen(js_name = checkedSub)]
    pub fn checked_sub(&self, other: &JsMoney) -> Result<JsMoney, JsValue> {
        self.inner
            .checked_sub(other.inner)
            .map(|inner| JsMoney { inner })
            .map_err(to_js_err)
    }

    /// Multiply by a number (Rust `Money::checked_mul_f64`).
    ///
    /// @param factor - Dimensionless multiplier (must be finite).
    /// @returns Scaled amount, in the same currency.
    /// @throws If `factor` is non-finite or the result is not representable.
    #[wasm_bindgen(js_name = checkedMulF64)]
    pub fn checked_mul_f64(&self, factor: JsValue) -> Result<JsMoney, JsValue> {
        let factor = js_f64(&factor, "factor")?;
        self.inner
            .checked_mul_f64(factor)
            .map(|inner| JsMoney { inner })
            .map_err(to_js_err)
    }

    /// Divide by a number (Rust `Money::checked_div_f64`).
    ///
    /// @param divisor - Dimensionless divisor (must be finite and non-zero).
    /// @returns Scaled amount, in the same currency.
    /// @throws If `divisor` is zero, non-finite, or the result is not representable.
    #[wasm_bindgen(js_name = checkedDivF64)]
    pub fn checked_div_f64(&self, divisor: JsValue) -> Result<JsMoney, JsValue> {
        let divisor = js_f64(&divisor, "divisor")?;
        self.inner
            .checked_div_f64(divisor)
            .map(|inner| JsMoney { inner })
            .map_err(to_js_err)
    }

    /// Negate the monetary amount (Rust `Money::checked_neg`).
    ///
    /// Negation is exact on the stored `Decimal`, keeps its scale and never
    /// passes through `f64`, so it cannot fail.
    ///
    /// @returns Negated amount in the same currency.
    #[wasm_bindgen(js_name = checkedNeg)]
    pub fn checked_neg(&self) -> JsMoney {
        JsMoney {
            inner: self.inner.checked_neg(),
        }
    }

    /// Format the amount with explicit display options.
    ///
    /// `decimals` accepts `null`/`undefined` (currency ISO minor units) or a
    /// non-negative integer up to 1,000,000. `group` is an optional
    /// single-character thousands separator (e.g. `","`); `null`/`undefined`
    /// disables grouping. `rounding` accepts the canonical mode names
    /// (`"bankers"`, `"away_from_zero"`, `"toward_zero"`, `"floor"`,
    /// `"ceil"`; case-sensitive); `null`/`undefined` selects the Rust default
    /// (`RoundingMode::default()`, bankers). Formatting never mutates the
    /// stored amount.
    ///
    /// # Arguments
    ///
    /// * `decimals` - JavaScript number of fractional digits, an integer in
    ///   `0..=1_000_000`; null or undefined selects ISO minor units.
    /// * `show_currency` - Whether to prepend the ISO code; omitted means true.
    /// * `group` - Optional single-character thousands separator; omitted means no grouping.
    /// * `rounding` - Canonical lowercase rounding mode; omitted selects the
    ///   Rust default (bankers).
    ///
    /// @returns Formatted amount such as `"USD 1,234.57"`.
    /// @throws If `decimals` is not a non-negative integer or exceeds 1,000,000, if `group` is not a single character, or if `rounding` is not a recognised mode name.
    ///
    /// @example
    /// ```javascript
    /// const usd = new core.Currency("USD");
    /// const m = core.Money.fromDecimalStr("1234.567", usd);
    /// try {
    ///   m.formatWith(2, true, ",", "bankers");  // "USD 1,234.57"
    /// } finally {
    ///   m.free();
    ///   usd.free();
    /// }
    /// ```
    #[wasm_bindgen(js_name = formatWith)]
    pub fn format_with(
        &self,
        decimals: JsValue,
        show_currency: Option<JsValue>,
        group: Option<JsValue>,
        rounding: Option<JsValue>,
    ) -> Result<String, JsValue> {
        let show_currency = js_opt_bool(show_currency.as_ref(), "showCurrency")?;
        let group = js_opt_string(group.as_ref(), "group")?;
        let rounding = js_opt_string(rounding.as_ref(), "rounding")?;
        let decimals: Option<usize> = js_opt_uint(Some(&decimals), "decimals")?;
        let group = match group {
            None => None,
            Some(sep) => {
                let mut chars = sep.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => Some(c),
                    _ => return Err(to_js_err("group must be a single character such as ','")),
                }
            }
        };
        let rounding = match rounding {
            Some(mode) => mode.parse::<RoundingMode>().map_err(to_js_err)?,
            None => RoundingMode::default(),
        };
        let opts = FormatOpts::new(decimals, show_currency.unwrap_or(true), group, rounding)
            .map_err(to_js_err)?;
        Ok(self.inner.format_with(opts))
    }

    /// Default string representation (e.g. `"USD 10.00"`).
    ///
    /// @returns Formatted amount with currency code.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }

    /// Serialize to a JSON string using the canonical Rust serde schema.
    ///
    /// @returns A JSON string carrying the exact decimal amount and the
    /// ISO-4217 currency code.
    /// @throws If serialization fails (should not happen for valid `Money`).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Deserialize from a JSON string produced by `Money.toJson`.
    ///
    /// @param json - A JSON string in the canonical Rust `Money` schema.
    /// @returns The parsed `Money`.
    /// @throws If `json` is malformed or fails strict schema validation.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsMoney, JsValue> {
        let json: &str = &json_text(&json, "json")?;
        let inner: RustMoney = serde_json::from_str(json).map_err(to_js_err)?;
        Ok(JsMoney { inner })
    }

    /// Construct from exact decimal text, rejecting inexact amounts.
    ///
    /// `amount` accepts fixed-point (`"1234.56"`) or scientific (`"1.2345e3"`)
    /// text and must be exactly representable as a Rust `Decimal` (96-bit
    /// mantissa, up to 28 fractional digits; the scientific mantissa must
    /// itself fit exactly). Inexact or underflowing amounts throw rather than
    /// being silently rounded, and the text never passes through `f64`. The
    /// currency is a tag only — no FX conversion is performed.
    ///
    /// # Arguments
    ///
    /// * `amount` - Exact fixed-point or scientific decimal text in major currency units.
    /// * `currency` - ISO-4217 `Currency` object that tags the amount, as for
    ///   the `Money` constructor (build one with `new Currency(code)`).
    ///
    /// @returns The constructed `Money`.
    /// @throws If `amount` is malformed, non-finite, needs more precision than `Decimal` can hold, or underflows its supported scale; or if `currency` is not a `Currency` object.
    ///
    /// @example
    /// ```javascript
    /// const usd = new core.Currency("USD");
    /// const m = core.Money.fromDecimalStr("1.245", usd);
    /// try {
    ///   m.amountDecimal;  // "1.245"
    /// } finally {
    ///   m.free();
    ///   usd.free();
    /// }
    /// ```
    #[wasm_bindgen(js_name = fromDecimalStr)]
    pub fn from_decimal_str(amount: JsValue, currency: &JsCurrency) -> Result<JsMoney, JsValue> {
        let amount: &str = &js_string(&amount, "amount")?;
        RustMoney::from_decimal_str(amount, currency.inner)
            .map(|inner| JsMoney { inner })
            .map_err(to_js_err)
    }

    /// A zero amount in a currency.
    ///
    /// # Arguments
    ///
    /// * `currency` - ISO-4217 `Currency` object that tags the amount.
    ///
    /// @returns `Money` with amount `0` in `currency`.
    /// @throws If the amount cannot be constructed (not expected for zero).
    #[wasm_bindgen(js_name = zero)]
    pub fn zero(currency: &JsCurrency) -> Result<JsMoney, JsValue> {
        RustMoney::new(0.0, currency.inner)
            .map(|inner| JsMoney { inner })
            .map_err(to_js_err)
    }

    /// Construct from an `[amount, currencyCode]` pair, the inverse of `toTuple`.
    ///
    /// # Arguments
    ///
    /// * `tup` - Two-element array: the amount in major units (a finite
    ///   `number`) and the ISO-4217 alphabetic currency code (a string).
    ///
    /// @returns The constructed `Money`.
    /// @throws `TypeError` (kind `invalid_type`) if `tup` is not a two-element
    /// array of a number and a string; `FinstackError` (kind `validation`) for
    /// a non-finite amount or an unknown currency code.
    #[wasm_bindgen(js_name = fromTuple)]
    pub fn from_tuple(tup: JsValue) -> Result<JsMoney, JsValue> {
        if !js_sys::Array::is_array(&tup) {
            return Err(invalid_type("tup", "expected an [amount, currency] array"));
        }
        let pair = js_sys::Array::from(&tup);
        if pair.length() != 2 {
            return Err(invalid_type(
                "tup",
                &format!("expected 2 elements, got {}", pair.length()),
            ));
        }
        let amount = js_f64(&pair.get(0), "tup[0]")?;
        let currency = js_string(&pair.get(1), "tup[1]")?
            .parse()
            .map_err(to_js_err)?;
        RustMoney::new(amount, currency)
            .map(|inner| JsMoney { inner })
            .map_err(to_js_err)
    }

    /// The amount and currency code as an `[amount, currencyCode]` pair.
    ///
    /// @returns A two-element array: the amount in major units as a `number`
    /// (the `f64` view of the exact decimal) and the ISO-4217 code.
    #[wasm_bindgen(js_name = toTuple)]
    pub fn to_tuple(&self) -> js_sys::Array {
        js_sys::Array::of2(
            &JsValue::from_f64(self.inner.amount()),
            &JsValue::from(self.inner.currency().as_ref()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // mul_scalar error-path tests live in tests/wasm_*.rs (requires wasm32)
    // because Err(JsValue) panics on native targets.

    #[test]
    fn sub_different_via_inner() {
        let a = RustMoney::new(10.0, finstack_quant_core::currency::Currency::USD).expect("ok");
        let b = RustMoney::new(5.0, finstack_quant_core::currency::Currency::EUR).expect("ok");
        assert!(a.checked_sub(b).is_err());
    }

    // Error paths through wasm-bindgen create JsValue, which panics on
    // native targets.  Test the underlying Rust types instead.

    #[test]
    fn new_rejects_nan() {
        assert!(RustMoney::new(f64::NAN, finstack_quant_core::currency::Currency::USD).is_err());
    }

    #[test]
    fn new_rejects_infinity() {
        assert!(
            RustMoney::new(f64::INFINITY, finstack_quant_core::currency::Currency::USD).is_err()
        );
    }

    #[test]
    fn div_scalar_rejects_zero() {
        let m = RustMoney::new(10.0, finstack_quant_core::currency::Currency::USD).expect("valid");
        assert!(m.checked_div_f64(0.0).is_err());
    }

    #[test]
    fn checked_neg_is_the_rust_checked_neg() {
        for inner in [
            RustMoney::new(0.0, finstack_quant_core::currency::Currency::USD).expect("zero"),
            RustMoney::from_decimal_str("0.00", finstack_quant_core::currency::Currency::USD)
                .expect("scaled zero"),
            RustMoney::from_decimal_str("1e-27", finstack_quant_core::currency::Currency::USD)
                .expect("tiny"),
        ] {
            let neg = JsMoney { inner }.checked_neg();
            assert_eq!(
                neg.to_json().expect("json"),
                serde_json::to_string(&inner.checked_neg()).expect("json")
            );
        }
    }
}
