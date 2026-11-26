//! Shared getter/setter conversions, pricing dispatch, and staged builder state.

use crate::api::core::currency::JsCurrency;
use crate::api::core::dates::{JsDayCount, JsTenor};
use crate::api::core::money::JsMoney;
use crate::utils::{date_to_iso, parse_iso_date, to_js_err, to_js_value};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::json_loader::{InstrumentEnvelope, InstrumentJson};
use rust_decimal::Decimal;
use wasm_bindgen::prelude::*;

/// Result of one getter conversion.
type Js = Result<JsValue, JsValue>;

/// Rust field → JavaScript value conversions used by the getter macro.
pub(crate) mod get {
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
        Ok(JsValue::from_f64(value.as_f64()))
    }

    /// An optional `Decimal` as a JavaScript number or `null`.
    pub(crate) fn opt_dec(value: &Option<Decimal>) -> Js {
        value.as_ref().map_or(Ok(JsValue::NULL), dec)
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
///
/// The macro dispatches `arg::$conv`, so each conversion has a one-word name;
/// most are the `utils` input helpers themselves. Code outside the macros
/// calls `utils::input` / `utils::wire` directly.
pub(crate) mod arg {
    use super::*;
    pub(crate) use crate::utils::input::{
        from_js_json, js_bool, js_currency, js_f64, js_int, js_string, js_uint,
    };
    pub(crate) use crate::utils::wire::{js_date, js_parse, js_wire};

    /// An optional JavaScript number; `null` and `undefined` yield `None`.
    pub(crate) fn opt_f64(value: &JsValue, label: &str) -> Result<Option<f64>, JsValue> {
        crate::utils::input::js_opt_f64(Some(value), label)
    }

    /// An optional JavaScript boolean; `null` and `undefined` yield `None`.
    pub(crate) fn opt_bool(value: &JsValue, label: &str) -> Result<Option<bool>, JsValue> {
        crate::utils::input::js_opt_bool(Some(value), label)
    }

    /// An optional signed whole number; `null` and `undefined` yield `None`.
    pub(crate) fn opt_int<T: TryFrom<i64>>(
        value: &JsValue,
        label: &str,
    ) -> Result<Option<T>, JsValue> {
        crate::utils::input::js_opt_int(Some(value), label)
    }

    /// An optional non-negative whole number; `null` and `undefined` yield `None`.
    pub(crate) fn opt_uint<T: TryFrom<u64>>(
        value: &JsValue,
        label: &str,
    ) -> Result<Option<T>, JsValue> {
        crate::utils::input::js_opt_uint(Some(value), label)
    }

    /// An optional ISO-8601 date string; `null` and `undefined` yield `None`.
    pub(crate) fn opt_date(value: &JsValue, label: &str) -> Result<Option<Date>, JsValue> {
        crate::utils::wire::js_opt_date(Some(value), label)
    }

    /// An optional wire value; `null` and `undefined` yield `None`.
    pub(crate) fn opt_wire<T: serde::de::DeserializeOwned>(
        value: &JsValue,
        label: &str,
    ) -> Result<Option<T>, JsValue> {
        crate::utils::wire::js_opt_wire(Some(value), label)
    }

    /// An optional structured input defaulting on `null` or `undefined`.
    pub(crate) fn opt_or_default<T: serde::de::DeserializeOwned + Default>(
        value: &JsValue,
        label: &str,
    ) -> Result<T, JsValue> {
        Ok(crate::utils::input::opt_from_js_json(Some(value), label)?.unwrap_or_default())
    }

    /// A string identifier (instrument, curve, calendar or price id).
    pub(crate) fn id<T: From<String>>(value: &JsValue, label: &str) -> Result<T, JsValue> {
        js_string(value, label).map(T::from)
    }

    /// An optional identifier that preserves every supplied string literally.
    pub(crate) fn optional_id<T: From<String>>(
        value: &JsValue,
        label: &str,
    ) -> Result<Option<T>, JsValue> {
        if value.is_null() || value.is_undefined() {
            return Ok(None);
        }
        id(value, label).map(Some)
    }

    /// An array of ISO-8601 date strings.
    pub(crate) fn dates(value: &JsValue, label: &str) -> Result<Vec<Date>, JsValue> {
        crate::utils::input::js_string_seq(value, label)?
            .iter()
            .map(|text| parse_iso_date(text))
            .collect()
    }

    /// Contractual exact decimal, encoded as a string.
    pub(crate) fn exact_decimal(value: &JsValue, label: &str) -> Result<Decimal, JsValue> {
        crate::utils::wire::js_decimal(value, label).map(|value| value.0)
    }

    /// A JavaScript number stored as an exact `Decimal`.
    pub(crate) fn dec(value: &JsValue, label: &str) -> Result<Decimal, JsValue> {
        finstack_quant_core::decimal::f64_to_decimal(crate::utils::input::js_f64(value, label)?)
            .map_err(|error| {
                to_js_err(finstack_quant_core::Error::Validation(format!(
                    "{label}: {error}"
                )))
            })
    }

    /// `TermOutSpec` as a month count (`24`) or `{"months": 24}` object/JSON string.
    pub(crate) fn term_out(
        value: &JsValue,
        label: &str,
    ) -> Result<
        finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::TermOutSpec,
        JsValue,
    > {
        if value.as_f64().is_some() {
            Ok(
                finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::TermOutSpec {
                    months: js_uint(value, label)?,
                },
            )
        } else {
            from_js_json(value, label)
        }
    }
}

/// The instrument as its compact canonical v1 envelope.
pub(crate) fn envelope_json(instrument: impl Into<InstrumentJson>) -> Result<String, JsValue> {
    serde_json::to_string(&InstrumentEnvelope::new(instrument.into())).map_err(to_js_err)
}

/// `Instrument::market_dependencies` as a plain object.
pub(crate) fn market_dependencies(instrument: &dyn Instrument) -> Js {
    to_js_value(&instrument.market_dependencies().map_err(to_js_err)?)
}

/// Every standard option Greek that applies to a typed instrument, as a JS
/// object whose keys keep the Rust `STANDARD_OPTION_GREEKS` order.
///
/// Shared by the `greeks` methods of the option classes; non-finite Greeks are
/// rejected by Rust rather than serialized.
pub(crate) fn option_greeks(
    instrument: Box<dyn Instrument>,
    market_json: &str,
    as_of: &str,
    model: Option<&str>,
    extras: Option<&JsValue>,
) -> Js {
    let extras: finstack_quant_valuations::pricer::PriceExtras =
        crate::utils::input::opt_from_js_json(extras, "extras")?.unwrap_or_default();
    let instrument = finstack_quant_valuations::pricer::ParsedInstrument::from_instrument(
        instrument,
        extras.metric_pricing_overrides_json().as_deref(),
        extras.instrument_pricing_overrides.as_ref(),
    )
    .map_err(to_js_err)?;
    let market = super::super::pricing::parse_market_text(market_json)?;
    let pairs = finstack_quant_valuations::pricer::present_standard_option_greeks(
        &instrument,
        &market,
        as_of,
        model.unwrap_or("default"),
        extras.pricing_options(finstack_quant_calibration::recalibration::pricing_options()),
    )
    .map_err(to_js_err)?;
    let ordered: indexmap::IndexMap<&'static str, f64> = pairs.into_iter().collect();
    to_js_value(&ordered)
}

/// Shared, consumable state of a fluent builder.
///
/// Every setter returns a new JavaScript handle on the same staged Rust
/// builder, so both `b.id("X").build()` chains and statement-by-statement
/// use work; `build()` takes the Rust builder and leaves the slot empty.
pub(crate) struct Staged<B>(std::rc::Rc<std::cell::RefCell<Option<B>>>);

impl<B> Staged<B> {
    /// Stage a fresh Rust builder.
    pub(crate) fn new(builder: B) -> Self {
        Self(std::rc::Rc::new(std::cell::RefCell::new(Some(builder))))
    }

    fn consumed() -> JsValue {
        to_js_err(finstack_quant_core::Error::Validation(
            "builder already consumed by build()".to_string(),
        ))
    }

    /// Apply one consuming Rust setter and return a handle on the same state.
    pub(crate) fn apply(&self, set: impl FnOnce(B) -> B) -> Result<Self, JsValue> {
        let mut slot = self.0.borrow_mut();
        let builder = slot.take().ok_or_else(Self::consumed)?;
        *slot = Some(set(builder));
        Ok(Self(std::rc::Rc::clone(&self.0)))
    }

    /// Take the Rust builder for `build()`.
    pub(crate) fn take(&self) -> Result<B, JsValue> {
        self.0.borrow_mut().take().ok_or_else(Self::consumed)
    }
}
