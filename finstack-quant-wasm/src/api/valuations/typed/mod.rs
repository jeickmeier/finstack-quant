//! Member parity for the typed instrument classes and their builders.
//!
//! Every typed instrument class exposes one getter per public Rust field, the
//! `Instrument` trait surface, `toDict`, and a `builder()` whose setters map
//! one for one onto the Rust `FinancialBuilder` setters. The members are
//! emitted by the macros below so each one is a single conversion plus one
//! Rust call:
//!
//! - [`get`] converts a Rust field to its JavaScript value: `Money`,
//!   `Tenor`, `DayCount` and `Currency` become their WASM handles, dates
//!   become ISO-8601 strings, identifiers and enums become their serde
//!   strings, decimals become numbers, and nested specs become plain objects
//!   (the schema-generated TypeScript types).
//! - [`arg`] converts a JavaScript setter argument to the Rust setter type
//!   with the strict `utils::input` checks.

#[macro_use]
mod macros;

pub mod credit;
pub mod data;
pub mod equity;
pub mod fixed_income;
pub mod fx;
pub mod rates;
pub mod structured;

use crate::api::core::currency::JsCurrency;
use crate::api::core::dates::{JsDayCount, JsTenor};
use crate::api::core::money::JsMoney;
use crate::utils::input::{js_string, json_text};
use crate::utils::{date_to_iso, parse_iso_date, to_js_err, to_js_value};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::{Instrument, InstrumentEnvelope, InstrumentJson};
use rust_decimal::Decimal;
use wasm_bindgen::prelude::*;

/// Result of one getter conversion.
type Js = Result<JsValue, JsValue>;

/// Rust field → JavaScript value conversions used by the getter macro.
pub(super) mod get {
    use super::*;

    /// `Money` as a WASM `Money` handle.
    pub(crate) fn money(value: &Money) -> Js {
        Ok(JsMoney { inner: *value }.into())
    }

    /// `Date` as an ISO-8601 string.
    pub(crate) fn date(value: &Date) -> Js {
        Ok(JsValue::from(date_to_iso(*value)))
    }

    /// Optional `Date` as an ISO-8601 string or `null`.
    pub(crate) fn opt_date(value: &Option<Date>) -> Js {
        value.as_ref().map_or(Ok(JsValue::NULL), date)
    }

    /// A list of dates as ISO-8601 strings.
    pub(crate) fn dates(value: &[Date]) -> Js {
        to_js_value(&value.iter().map(|d| date_to_iso(*d)).collect::<Vec<_>>())
    }

    /// Optional dates as ISO-8601 strings, or `null`.
    pub(crate) fn opt_dates(value: &Option<Vec<Date>>) -> Js {
        value.as_deref().map_or(Ok(JsValue::NULL), dates)
    }

    /// `(date, amount)` pairs as `[isoDate, number]` pairs.
    pub(crate) fn dated_nums(value: &[(Date, f64)]) -> Js {
        to_js_value(
            &value
                .iter()
                .map(|(date, amount)| (date_to_iso(*date), *amount))
                .collect::<Vec<_>>(),
        )
    }

    /// An identifier or other `Display` value as a string.
    pub(crate) fn text<T: ToString>(value: &T) -> Js {
        Ok(JsValue::from(value.to_string()))
    }

    /// An optional identifier as a string or `null`.
    pub(crate) fn opt_text<T: ToString>(value: &Option<T>) -> Js {
        value.as_ref().map_or(Ok(JsValue::NULL), text)
    }

    /// A `Decimal` as a JavaScript number.
    pub(crate) fn dec(value: &Decimal) -> Js {
        finstack_quant_core::decimal::decimal_to_f64(*value)
            .map(JsValue::from_f64)
            .map_err(to_js_err)
    }

    /// A `(payment date, amount)` pair as `[isoDate, Money]`, or `null`.
    pub(crate) fn dated_money(value: &Option<(Date, Money)>) -> Js {
        let Some((paid, amount)) = value else {
            return Ok(JsValue::NULL);
        };
        let pair = js_sys::Array::new();
        pair.push(&date(paid)?);
        pair.push(&money(amount)?);
        Ok(pair.into())
    }

    /// `Tenor` as a WASM `Tenor` handle.
    pub(crate) fn tenor(value: &Tenor) -> Js {
        Ok(JsTenor { inner: *value }.into())
    }

    /// `DayCount` as a WASM `DayCount` handle.
    pub(crate) fn day_count(value: &DayCount) -> Js {
        Ok(JsDayCount { inner: *value }.into())
    }

    /// `Currency` as a WASM `Currency` handle.
    pub(crate) fn currency(value: &Currency) -> Js {
        Ok(JsCurrency { inner: *value }.into())
    }

    /// Any serde value as its JSON-shaped plain JavaScript value.
    pub(crate) fn json<T: serde::Serialize>(value: &T) -> Js {
        to_js_value(value)
    }
}

/// JavaScript argument → Rust setter type conversions used by the setter macro.
pub(super) mod arg {
    use super::*;
    use crate::utils::input::{from_js_json, js_bool, js_f64, js_uint};
    use serde::de::DeserializeOwned;

    fn invalid(label: &str, error: impl std::fmt::Display) -> JsValue {
        to_js_err(finstack_quant_core::Error::Validation(format!(
            "{label}: {error}"
        )))
    }

    /// A string identifier (instrument, curve, calendar or price id).
    pub(crate) fn id<T: From<String>>(value: &JsValue, label: &str) -> Result<T, JsValue> {
        js_string(value, label).map(T::from)
    }

    /// A plain string.
    pub(crate) fn text(value: &JsValue, label: &str) -> Result<String, JsValue> {
        js_string(value, label)
    }

    /// A nested spec, or the serde name of one of its unit variants.
    ///
    /// A string that does not start a JSON document (`{`, `[` or `"`) is the
    /// variant name itself, so `"bullet"` and `{"linear": {...}}` both work.
    pub(crate) fn json_or_name<T: DeserializeOwned>(
        value: &JsValue,
        label: &str,
    ) -> Result<T, JsValue> {
        match value.as_string() {
            Some(name) if !name.trim_start().starts_with(['{', '[', '"']) => {
                serde_json::from_value(serde_json::Value::String(name))
                    .map_err(|error| invalid(label, error))
            }
            _ => from_js_json(value, label),
        }
    }

    /// An ISO-8601 date string.
    pub(crate) fn date(value: &JsValue, label: &str) -> Result<Date, JsValue> {
        parse_iso_date(&js_string(value, label)?)
    }

    /// An array of ISO-8601 date strings.
    pub(crate) fn dates(value: &JsValue, label: &str) -> Result<Vec<Date>, JsValue> {
        crate::utils::input::js_string_seq(value, label)?
            .iter()
            .map(|text| parse_iso_date(text))
            .collect()
    }

    /// A JavaScript number.
    pub(crate) fn num(value: &JsValue, label: &str) -> Result<f64, JsValue> {
        js_f64(value, label)
    }

    /// A JavaScript number stored as an exact `Decimal`.
    pub(crate) fn dec(value: &JsValue, label: &str) -> Result<Decimal, JsValue> {
        finstack_quant_core::decimal::f64_to_decimal(js_f64(value, label)?)
            .map_err(|error| invalid(label, error))
    }

    /// A non-negative whole number.
    pub(crate) fn uint<T: TryFrom<u64>>(value: &JsValue, label: &str) -> Result<T, JsValue> {
        js_uint(value, label)
    }

    /// A boolean.
    pub(crate) fn flag(value: &JsValue, label: &str) -> Result<bool, JsValue> {
        js_bool(value, label)
    }

    /// A serde string enum (for example `"pay"` or `"act_360"`).
    pub(crate) fn en<T: DeserializeOwned>(value: &JsValue, label: &str) -> Result<T, JsValue> {
        serde_json::from_value(serde_json::Value::String(js_string(value, label)?))
            .map_err(|error| invalid(label, error))
    }

    /// A nested spec as a plain object or JSON string.
    pub(crate) fn json<T: DeserializeOwned>(value: &JsValue, label: &str) -> Result<T, JsValue> {
        from_js_json(value, label)
    }
}

/// Parse a market-context argument (JSON string or plain object).
pub(super) fn market(market_json: &JsValue) -> Result<MarketContext, JsValue> {
    serde_json::from_str(&json_text(market_json, "marketJson")?).map_err(to_js_err)
}

/// Parse the `asOf` valuation date argument.
pub(super) fn as_of(as_of: &JsValue) -> Result<Date, JsValue> {
    parse_iso_date(&js_string(as_of, "asOf")?)
}

/// The instrument as its compact canonical v1 envelope.
pub(super) fn envelope_json(instrument: impl Into<InstrumentJson>) -> Result<String, JsValue> {
    serde_json::to_string(&InstrumentEnvelope::new(instrument.into())).map_err(to_js_err)
}

/// `Instrument::market_dependencies` as a plain object.
pub(super) fn market_dependencies(instrument: &dyn Instrument) -> Js {
    to_js_value(&instrument.market_dependencies().map_err(to_js_err)?)
}

/// One scalar metric of a typed instrument, through the `metric_value` path
/// shared with `priceInstrument`.
pub(super) fn metric_value(
    envelope: &str,
    market_json: &JsValue,
    as_of: &JsValue,
    model: Option<&JsValue>,
    metric_id: &str,
) -> Result<f64, JsValue> {
    let instrument = super::pricing::parse_pricing_instrument_json(envelope, None)?;
    let market = market(market_json)?;
    let as_of = js_string(as_of, "asOf")?;
    let model = crate::utils::input::js_opt_string(model, "model")?;
    super::pricing::metric_value_with_context(
        &instrument,
        &market,
        &as_of,
        model.as_deref().unwrap_or("default"),
        metric_id,
    )
}

/// Shared, consumable state of a fluent builder.
///
/// Every setter returns a new JavaScript handle on the same staged Rust
/// builder, so both `b.id("X").build()` chains and statement-by-statement
/// use work; `build()` takes the Rust builder and leaves the slot empty.
pub(super) struct Staged<B>(std::rc::Rc<std::cell::RefCell<Option<B>>>);

impl<B> Staged<B> {
    /// Stage a fresh Rust builder.
    pub(super) fn new(builder: B) -> Self {
        Self(std::rc::Rc::new(std::cell::RefCell::new(Some(builder))))
    }

    fn consumed() -> JsValue {
        to_js_err(finstack_quant_core::Error::Validation(
            "builder already consumed by build()".to_string(),
        ))
    }

    /// Apply one consuming Rust setter and return a handle on the same state.
    pub(super) fn apply(&self, set: impl FnOnce(B) -> B) -> Result<Self, JsValue> {
        let mut slot = self.0.borrow_mut();
        let builder = slot.take().ok_or_else(Self::consumed)?;
        *slot = Some(set(builder));
        Ok(Self(std::rc::Rc::clone(&self.0)))
    }

    /// Take the Rust builder for `build()`.
    pub(super) fn take(&self) -> Result<B, JsValue> {
        self.0.borrow_mut().take().ok_or_else(Self::consumed)
    }
}
