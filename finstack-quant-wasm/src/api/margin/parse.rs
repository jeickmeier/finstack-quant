//! Argument-parsing helpers shared by margin WASM bindings.

use crate::utils::input::{js_f64, js_string};
use crate::utils::to_js_err;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use wasm_bindgen::prelude::*;

/// Parse a string enum/label argument using its `FromStr` implementation.
pub(super) fn parse_label<T>(value: &JsValue, label: &str) -> Result<T, JsValue>
where
    T: std::str::FromStr,
    T::Err: crate::utils::IntoJsError,
{
    js_string(value, label)?.parse::<T>().map_err(to_js_err)
}

/// A numeric amount argument in major units of `currency`.
pub(super) fn js_money(value: &JsValue, label: &str, currency: Currency) -> Result<Money, JsValue> {
    Money::new(js_f64(value, label)?, currency).map_err(to_js_err)
}

/// An amount in major units of a CSA's base currency.
pub(super) fn base_money(
    csa: &finstack_quant_margin::types::CsaSpec,
    amount: f64,
) -> Result<Money, JsValue> {
    Money::new(amount, csa.base_currency).map_err(to_js_err)
}
