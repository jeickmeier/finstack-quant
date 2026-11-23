//! Day-count context and volatility-clock WASM handles and argument parsers.

use super::day_count::JsDayCount;
use super::tenor::JsTenor;
use crate::utils::input::{
    from_js_json, invalid_type, is_plain_object, js_epoch_days, js_opt_bool, js_opt_string,
    js_opt_uint, wasm_handle_string,
};
use crate::utils::to_js_err;
use finstack_quant_core::dates::{
    Date, DayCount as RustDayCount, DayCountContextState, Tenor as RustTenor, VolatilityClock,
};
use wasm_bindgen::prelude::*;

/// Optional context for day-count conventions that need market metadata.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const context = new core.DayCountContext("nyse", "3M", 252);
/// console.log(context.frequency?.toString()); // "3M"
/// ```
#[wasm_bindgen(js_name = DayCountContext)]
#[derive(Clone, Default)]
pub struct JsDayCountContext {
    /// Serializable state (`DayCountContextState::default()` when empty); the
    /// live calendar is resolved on each use.
    pub(crate) inner: DayCountContextState,
}

#[wasm_bindgen(js_class = DayCountContext)]
impl JsDayCountContext {
    /// Create a day-count context (Rust `DayCountContextState::try_new`, the
    /// validation point shared with Python `DayCountContext(...)`).
    ///
    /// Every argument is optional; `new DayCountContext()` is the empty
    /// context `DayCount.yearFraction` / `signedYearFraction` use when none is
    /// passed.
    ///
    /// # Arguments
    ///
    /// * `calendar_id` - Registered holiday-calendar identifier (for example
    ///   `"nyse"`) used by Bus/252; resolved when the context is used.
    /// * `frequency` - Coupon frequency as tenor text (for example `"6M"`; use
    ///   `tenor.toString()` for a `Tenor`), required by Act/Act ICMA.
    /// * `bus_basis` - Business-day denominator for Bus/252 (an integer in
    ///   `1..=65535`, normally `252`).
    /// * `coupon_period` - `[startEpochDays, endEpochDays]` (days since
    ///   1970-01-01): the unadjusted regular reference period for Act/Act ICMA,
    ///   or the full enclosing contractual coupon for Act/365L; the start must
    ///   precede the end. ICMA endpoints must share the contractual nominal
    ///   month grid, rather than adjusted payment dates.
    /// * `end_is_termination_date` - Whether the chronologically later accrual
    ///   endpoint is the contractual termination date (30E/360 ISDA
    ///   February-end handling); omitted or `null` means unspecified, which
    ///   30E/360 ISDA rejects.
    ///
    /// @returns A new `DayCountContext`.
    /// @throws `TypeError` (kind `invalid_type`) for a mistyped argument or a
    /// `couponPeriod` that is not a two-element array of epoch days;
    /// `FinstackError` (kind `validation`) if `frequency` is not a tenor, the
    /// coupon period is inverted or out of range, or `busBasis` is zero.
    /// @param calendarId - Registered holiday-calendar identifier (for example `"nyse"`) used by Bus/252; resolved when the context is used.
    /// @param frequency - Coupon frequency as tenor text (for example `"6M"`; use `tenor.toString()` for a `Tenor`), required by Act/Act ICMA.
    /// @param busBasis - Business-day denominator for Bus/252 (an integer in `1..=65535`, normally `252`).
    /// @param couponPeriod - `[startEpochDays, endEpochDays]` (days since 1970-01-01): the unadjusted regular reference period for Act/Act ICMA, or the full enclosing contractual coupon for Act/365L; the start must precede the end. ICMA endpoints must share the contractual nominal month grid, rather than adjusted payment dates.
    /// @param endIsTerminationDate - Whether the chronologically later accrual endpoint is the contractual termination date (30E/360 ISDA February-end handling); omitted or `null` means unspecified, which 30E/360 ISDA rejects.
    ///
    /// @example
    /// ```javascript
    /// const ctx = new core.DayCountContext("nyse", "3M", 252);
    /// ctx.frequency?.toString();  // "3M"
    /// core.DayCountContext.fromJson(ctx.toJson()).busBasis;  // 252
    /// ```
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(
            js_name = "calendarId",
            unchecked_optional_param_type = "string | null"
        )]
        calendar_id: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "string | null")] frequency: Option<JsValue>,
        #[wasm_bindgen(js_name = "busBasis", unchecked_optional_param_type = "number | null")]
        bus_basis: Option<JsValue>,
        #[wasm_bindgen(
            js_name = "couponPeriod",
            unchecked_optional_param_type = "readonly [number, number] | null"
        )]
        coupon_period: Option<JsValue>,
        #[wasm_bindgen(
            js_name = "endIsTerminationDate",
            unchecked_optional_param_type = "boolean | null"
        )]
        end_is_termination_date: Option<JsValue>,
    ) -> Result<JsDayCountContext, JsValue> {
        let calendar_id = js_opt_string(calendar_id.as_ref(), "calendarId")?;
        let frequency = js_opt_string(frequency.as_ref(), "frequency")?
            .map(|text| RustTenor::parse(&text))
            .transpose()
            .map_err(to_js_err)?;
        let bus_basis: Option<u16> = js_opt_uint(bus_basis.as_ref(), "busBasis")?;
        let coupon_period = match coupon_period {
            Some(value) if !(value.is_null() || value.is_undefined()) => {
                Some(epoch_day_pair(&value, "couponPeriod")?)
            }
            _ => None,
        };
        let end_is_termination_date =
            js_opt_bool(end_is_termination_date.as_ref(), "endIsTerminationDate")?;
        DayCountContextState::try_new(
            calendar_id,
            frequency,
            bus_basis,
            coupon_period,
            end_is_termination_date,
        )
        .map(|inner| JsDayCountContext { inner })
        .map_err(to_js_err)
    }

    /// Holiday-calendar identifier, or `undefined`.
    #[wasm_bindgen(getter, js_name = calendarId)]
    pub fn calendar_id(&self) -> Option<String> {
        self.inner.get_calendar_id().map(str::to_owned)
    }

    /// Coupon frequency, or `undefined`.
    #[wasm_bindgen(getter, js_name = frequency)]
    pub fn frequency(&self) -> Option<JsTenor> {
        self.inner.get_frequency().map(|inner| JsTenor { inner })
    }

    /// Custom business-day denominator, or `undefined`.
    #[wasm_bindgen(getter, js_name = busBasis)]
    pub fn bus_basis(&self) -> Option<u16> {
        self.inner.get_bus_basis()
    }

    /// Reference coupon period as `[startEpochDays, endEpochDays]`, or `undefined`.
    #[wasm_bindgen(getter, js_name = couponPeriod)]
    pub fn coupon_period(&self) -> Option<Box<[i32]>> {
        self.inner.get_coupon_period().map(|(start, end)| {
            Box::new([
                finstack_quant_core::dates::days_since_epoch(start),
                finstack_quant_core::dates::days_since_epoch(end),
            ]) as Box<[i32]>
        })
    }

    /// Whether the later accrual endpoint is the termination date, or `undefined` if unspecified.
    #[wasm_bindgen(getter, js_name = endIsTerminationDate)]
    pub fn end_is_termination_date(&self) -> Option<bool> {
        self.inner.get_end_is_termination_date()
    }
}

json_round_trip!(JsDayCountContext, DayCountContext);

thread_local! {
    static DAY_COUNT_PROTOTYPE: std::cell::OnceCell<js_sys::Object> = const { std::cell::OnceCell::new() };
    static DAY_COUNT_CONTEXT_PROTOTYPE: std::cell::OnceCell<js_sys::Object> = const { std::cell::OnceCell::new() };
    static VOLATILITY_CLOCK_PROTOTYPE: std::cell::OnceCell<js_sys::Object> = const { std::cell::OnceCell::new() };
}

pub(super) fn js_day_count_arg(value: &JsValue, label: &str) -> Result<RustDayCount, JsValue> {
    if let Some(s) = value.as_string() {
        return s.parse::<RustDayCount>().map_err(to_js_err);
    }
    if let Some(name) = wasm_handle_string(
        value,
        label,
        &DAY_COUNT_PROTOTYPE,
        || Ok(JsDayCount::act365f()),
        "toString",
    )? {
        return name.parse::<RustDayCount>().map_err(to_js_err);
    }
    Err(invalid_type(
        label,
        "expected a DayCount handle or canonical day-count string",
    ))
}

pub(super) fn js_opt_day_count_context_state(
    value: Option<&JsValue>,
    label: &str,
) -> Result<DayCountContextState, JsValue> {
    let Some(val) = value else {
        return Ok(DayCountContextState::default());
    };
    if val.is_undefined() || val.is_null() {
        return Ok(DayCountContextState::default());
    }
    if let Some(json) = wasm_handle_string(
        val,
        label,
        &DAY_COUNT_CONTEXT_PROTOTYPE,
        || {
            Ok(JsDayCountContext {
                inner: DayCountContextState::default(),
            })
        },
        "toJson",
    )? {
        return serde_json::from_str::<DayCountContextState>(&json).map_err(to_js_err);
    }
    if is_plain_object(val) {
        return from_js_json::<DayCountContextState>(val, label);
    }
    Err(invalid_type(
        label,
        "expected a DayCountContext handle, DayCountContextValue object, or null",
    ))
}

pub(crate) fn js_opt_day_count_context(
    value: Option<&JsValue>,
    label: &str,
) -> Result<finstack_quant_core::dates::DayCountContext<'static>, JsValue> {
    js_opt_day_count_context_state(value, label)?
        .to_ctx()
        .map_err(to_js_err)
}

/// Borrow a `VolatilityClock` from either a live `core.VolatilityClock` handle,
/// a plain JSON object matching the wire schema, or `null`/`undefined`.
///
/// # Arguments
///
/// * `value` - Optional JavaScript value holding a `VolatilityClock` handle, plain object, or `null`.
/// * `label` - Parameter name used in type/validation error messages.
pub(crate) fn js_opt_volatility_clock(
    value: Option<&JsValue>,
    label: &str,
) -> Result<Option<VolatilityClock>, JsValue> {
    let Some(val) = value else {
        return Ok(None);
    };
    if val.is_undefined() || val.is_null() {
        return Ok(None);
    }
    if let Some(json) = wasm_handle_string(
        val,
        label,
        &VOLATILITY_CLOCK_PROTOTYPE,
        || {
            let ref_date = Date::from_calendar_date(2025, time::Month::January, 1)
                .map_err(|_| invalid_type(label, "failed to create prototype reference date"))?;
            let inner = VolatilityClock::try_new(
                ref_date,
                RustDayCount::Act365F,
                DayCountContextState::default(),
            )
            .map_err(to_js_err)?;
            Ok(JsVolatilityClock { inner })
        },
        "toJson",
    )? {
        return serde_json::from_str::<VolatilityClock>(&json)
            .map(Some)
            .map_err(to_js_err);
    }
    if is_plain_object(val) {
        return from_js_json::<VolatilityClock>(val, label).map(Some);
    }
    Err(invalid_type(
        label,
        "expected a VolatilityClock handle, VolatilityClockValue object, or null",
    ))
}

/// Calendar and day-count clock used to map option expiry dates to year fractions.
///
/// Pairs an explicit valuation reference date, day-count convention, and optional
/// `DayCountContext` so surfaces convert expiry dates into year fractions
/// without implicit clocks or heuristics.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const refDate = core.createDate(2025, 1, 15);
/// const expiry = core.createDate(2026, 1, 15);
/// const clock = new core.VolatilityClock(refDate, core.DayCount.act365f());
/// console.log(clock.expiryTime(expiry));
/// clock.free();
/// ```
#[wasm_bindgen(js_name = VolatilityClock)]
pub struct JsVolatilityClock {
    pub(crate) inner: VolatilityClock,
}

#[wasm_bindgen(js_class = VolatilityClock)]
impl JsVolatilityClock {
    /// Construct a validated volatility clock from a reference date, day-count
    /// convention, and optional day-count context (Rust `VolatilityClock::try_new`).
    ///
    /// # Arguments
    ///
    /// * `reference_epoch_days` - Valuation reference date as days since 1970-01-01.
    /// * `day_count` - Day-count convention as a `DayCount` handle or canonical snake_case name.
    /// * `context` - Optional `DayCountContext` handle or plain object supplying metadata for context-dependent conventions (`bus_252`, `act_act_isma`, `act_365l`); omitted or `null` uses the empty default context.
    ///
    /// @returns A validated `VolatilityClock` handle.
    /// @throws `TypeError` (kind `invalid_type`) if an input has the wrong JavaScript type; `FinstackError` (kind `validation` or `not_found`) if the date is invalid, the convention name is unknown, or the convention requires context metadata that was not supplied.
    /// @param referenceEpochDays - Valuation reference date as days since 1970-01-01.
    /// @param dayCount - Day-count convention as a `DayCount` handle or canonical snake_case name.
    /// @param context - Optional `DayCountContext` handle or plain object; omitted or `null` uses the empty default context.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(js_name = "referenceEpochDays", unchecked_param_type = "number")]
        reference_epoch_days: JsValue,
        #[wasm_bindgen(js_name = "dayCount", unchecked_param_type = "DayCount | string")]
        day_count: JsValue,
        #[wasm_bindgen(
            unchecked_optional_param_type = "DayCountContext | DayCountContextValue | null"
        )]
        context: Option<JsValue>,
    ) -> Result<JsVolatilityClock, JsValue> {
        let reference = js_epoch_days(&reference_epoch_days, "referenceEpochDays")?;
        let convention = js_day_count_arg(&day_count, "dayCount")?;
        let ctx = js_opt_day_count_context_state(context.as_ref(), "context")?;
        VolatilityClock::try_new(reference, convention, ctx)
            .map(|inner| JsVolatilityClock { inner })
            .map_err(to_js_err)
    }

    /// Reference date as epoch days (days since 1970-01-01).
    #[wasm_bindgen(getter, js_name = referenceDate)]
    pub fn reference_date(&self) -> i32 {
        finstack_quant_core::dates::days_since_epoch(self.inner.get_reference_date())
    }

    /// Day-count convention used to map expiry dates to times.
    #[wasm_bindgen(getter, js_name = dayCount)]
    pub fn day_count(&self) -> JsDayCount {
        JsDayCount {
            inner: self.inner.get_day_count(),
        }
    }

    /// Day-count context associated with the clock.
    #[wasm_bindgen(getter, js_name = context)]
    pub fn context(&self) -> JsDayCountContext {
        JsDayCountContext {
            inner: self.inner.get_context().clone(),
        }
    }

    /// Compute the non-negative year fraction from the clock's reference date to
    /// an option expiry date (Rust `VolatilityClock::expiry_time`).
    ///
    /// # Arguments
    ///
    /// * `expiry_epoch_days` - Option expiry date as days since 1970-01-01; must be strictly after `referenceDate`.
    ///
    /// @returns Positive year fraction in years under the clock's day count and context.
    /// @throws `TypeError` (kind `invalid_type`) if `expiryEpochDays` is not an integer epoch-day number; `FinstackError` (kind `validation` or `not_found`) if the date is out of range or not strictly after `referenceDate`.
    /// @param expiryEpochDays - Option expiry date as days since 1970-01-01; must be strictly after `referenceDate`.
    #[wasm_bindgen(js_name = expiryTime)]
    pub fn expiry_time(
        &self,
        #[wasm_bindgen(js_name = "expiryEpochDays", unchecked_param_type = "number")]
        expiry_epoch_days: JsValue,
    ) -> Result<f64, JsValue> {
        let expiry = js_epoch_days(&expiry_epoch_days, "expiryEpochDays")?;
        self.inner.expiry_time(expiry).map_err(to_js_err)
    }
}

json_round_trip!(JsVolatilityClock, VolatilityClock);

/// Read a `[startEpochDays, endEpochDays]` pair.
pub(super) fn epoch_day_pair(value: &JsValue, label: &str) -> Result<(Date, Date), JsValue> {
    if !js_sys::Array::is_array(value) {
        return Err(invalid_type(
            label,
            "expected a [startEpochDays, endEpochDays] array",
        ));
    }
    let pair = js_sys::Array::from(value);
    if pair.length() != 2 {
        return Err(invalid_type(
            label,
            &format!("expected 2 elements, got {}", pair.length()),
        ));
    }
    Ok((
        js_epoch_days(&pair.get(0), &format!("{label}[0]"))?,
        js_epoch_days(&pair.get(1), &format!("{label}[1]"))?,
    ))
}
