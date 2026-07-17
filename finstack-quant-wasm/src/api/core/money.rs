//! WASM bindings for [`finstack_quant_core::money::Money`].

use crate::api::core::currency::Currency;
use crate::utils::to_js_err;
use finstack_quant_core::money::Money as RustMoney;
use wasm_bindgen::prelude::*;

/// Currency-tagged monetary amount.
///
/// Money values pin a numeric amount to a [`Currency`]. Arithmetic
/// (`add`, `sub`) refuses to mix currencies; scalar multiplication and
/// division preserve the currency.
///
/// @example
/// ```javascript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const usd = new core.Currency("USD");
/// const total = new core.Money(1_000_000, usd);
/// const fee   = new core.Money(50, usd);
/// const net   = total.sub(fee);                 // Money { amount: 999950, currency: USD }
/// const tax   = net.mulScalar(0.07);            // 7% of net
/// console.log(net.toString(), tax.toString());  // "USD 999950.00", "USD 69996.50"
/// ```
#[wasm_bindgen(js_name = Money)]
pub struct Money {
    #[wasm_bindgen(skip)]
    pub(crate) inner: RustMoney,
}

#[wasm_bindgen(js_class = Money)]
impl Money {
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
    pub fn new(amount: f64, currency: &Currency) -> Result<Money, JsValue> {
        RustMoney::try_new(amount, currency.inner)
            .map(|inner| Money { inner })
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
    #[wasm_bindgen(js_name = amountDecimal)]
    pub fn amount_decimal(&self) -> String {
        self.inner.amount_decimal().to_string()
    }

    /// Currency of this amount.
    ///
    /// @returns The [`Currency`] this amount is tagged with.
    #[wasm_bindgen(getter, js_name = currency)]
    pub fn currency(&self) -> Currency {
        Currency {
            inner: self.inner.currency(),
        }
    }

    /// Convert using an already-resolved positive FX rate.
    /// @param target - Target Currency for the converted monetary amount.
    /// @param rate - FX conversion rate expressed as target-currency units per source-currency unit.
    #[wasm_bindgen(js_name = convertAtRate)]
    pub fn convert_at_rate(&self, target: &Currency, rate: f64) -> Result<Money, JsValue> {
        self.inner
            .convert_at_rate(target.inner, rate)
            .map(|inner| Money { inner })
            .map_err(to_js_err)
    }

    /// Add two amounts.
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
    /// a.add(b).amount;  // 15
    /// ```
    #[wasm_bindgen(js_name = add)]
    pub fn add(&self, other: &Money) -> Result<Money, JsValue> {
        self.inner
            .checked_add(other.inner)
            .map(|inner| Money { inner })
            .map_err(to_js_err)
    }

    /// Subtract two amounts.
    ///
    /// @param other - Another `Money` value.
    /// @returns Difference, in the same currency.
    /// @throws If `other.currency` differs from `this.currency`, or the
    /// operation is not representable as a `Decimal`.
    #[wasm_bindgen(js_name = sub)]
    pub fn sub(&self, other: &Money) -> Result<Money, JsValue> {
        self.inner
            .checked_sub(other.inner)
            .map(|inner| Money { inner })
            .map_err(to_js_err)
    }

    /// Multiply by a scalar.
    ///
    /// @param factor - Dimensionless multiplier (must be finite).
    /// @returns Scaled amount, in the same currency.
    /// @throws If `factor` is non-finite or the result is not representable.
    #[wasm_bindgen(js_name = mulScalar)]
    pub fn mul_scalar(&self, factor: f64) -> Result<Money, JsValue> {
        self.inner
            .checked_mul_f64(factor)
            .map(|inner| Money { inner })
            .map_err(to_js_err)
    }

    /// Divide by a scalar.
    ///
    /// @param divisor - Dimensionless divisor (must be finite and non-zero).
    /// @returns Scaled amount, in the same currency.
    /// @throws If `divisor` is zero, non-finite, or the result is not representable.
    #[wasm_bindgen(js_name = divScalar)]
    pub fn div_scalar(&self, divisor: f64) -> Result<Money, JsValue> {
        self.inner
            .checked_div_f64(divisor)
            .map(|inner| Money { inner })
            .map_err(to_js_err)
    }

    /// Negate the monetary amount.
    ///
    /// @returns Negated amount in the same currency.
    /// @throws If the negation is not representable as a `Decimal`.
    #[wasm_bindgen(js_name = negate)]
    pub fn negate(&self) -> Result<Money, JsValue> {
        self.inner
            .checked_mul_f64(-1.0)
            .map(|inner| Money { inner })
            .map_err(to_js_err)
    }

    /// Default string representation (e.g. `"USD 10.00"`).
    ///
    /// @returns Formatted amount with currency code.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usd() -> Currency {
        Currency::new("USD").expect("USD")
    }

    #[test]
    fn construct_and_getters() {
        let m = Money::new(10.0, &usd()).expect("valid");
        assert!((m.amount() - 10.0).abs() < 1e-10);
        assert_eq!(m.currency().code(), "USD");
    }

    #[test]
    fn add_same_currency() {
        let a = Money::new(10.0, &usd()).expect("valid");
        let b = Money::new(5.0, &usd()).expect("valid");
        let c = a.add(&b).expect("add");
        assert!((c.amount() - 15.0).abs() < 1e-10);
    }

    #[test]
    fn sub_same_currency() {
        let a = Money::new(10.0, &usd()).expect("valid");
        let b = Money::new(3.0, &usd()).expect("valid");
        let c = a.sub(&b).expect("sub");
        assert!((c.amount() - 7.0).abs() < 1e-10);
    }

    #[test]
    fn mul_scalar() {
        let m = Money::new(10.0, &usd()).expect("valid");
        let scaled = m.mul_scalar(2.5).expect("finite factor");
        assert!((scaled.amount() - 25.0).abs() < 1e-10);
    }

    // mul_scalar error-path tests live in tests/wasm_*.rs (requires wasm32)
    // because Err(JsValue) panics on native targets.

    #[test]
    fn div_scalar() {
        let m = Money::new(10.0, &usd()).expect("valid");
        let half = m.div_scalar(2.0).expect("div");
        assert!((half.amount() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn negate() {
        let m = Money::new(10.0, &usd()).expect("valid");
        let neg = m.negate().expect("negate");
        assert!((neg.amount() + 10.0).abs() < 1e-10);
    }

    #[test]
    fn to_string_format() {
        let m = Money::new(10.0, &usd()).expect("valid");
        let s = m.to_string();
        assert!(s.contains("USD"), "expected USD in: {s}");
        assert!(s.contains("10"), "expected 10 in: {s}");
    }

    #[test]
    fn sub_different_via_inner() {
        let a = RustMoney::try_new(10.0, finstack_quant_core::currency::Currency::USD).expect("ok");
        let b = RustMoney::try_new(5.0, finstack_quant_core::currency::Currency::EUR).expect("ok");
        assert!(a.checked_sub(b).is_err());
    }

    // Error paths through wasm-bindgen create JsValue, which panics on
    // native targets.  Test the underlying Rust types instead.

    #[test]
    fn new_rejects_nan() {
        assert!(
            RustMoney::try_new(f64::NAN, finstack_quant_core::currency::Currency::USD).is_err()
        );
    }

    #[test]
    fn new_rejects_infinity() {
        assert!(
            RustMoney::try_new(f64::INFINITY, finstack_quant_core::currency::Currency::USD)
                .is_err()
        );
    }

    #[test]
    fn div_scalar_rejects_zero() {
        let m =
            RustMoney::try_new(10.0, finstack_quant_core::currency::Currency::USD).expect("valid");
        assert!(m.checked_div_f64(0.0).is_err());
    }

    #[test]
    fn negate_zero() {
        let m = Money::new(0.0, &usd()).expect("valid");
        let neg = m.negate().expect("negate");
        assert!(neg.amount().abs() < 1e-12);
    }
}
