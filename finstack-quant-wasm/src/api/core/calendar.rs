//! WASM bindings for holiday calendars, business-day conventions and the
//! `DateExt` helpers from [`finstack_quant_core::dates`].
//!
//! Dates are **epoch days** (`i32`, days since 1970-01-01), the convention of
//! every `core` date utility; `daysSinceEpoch` converts an ISO string.

use crate::api::core::periods::JsFiscalConfig;
use crate::utils::input::{js_epoch_days, js_int, js_string};
use crate::utils::{parse_iso_date, to_js_err, to_js_value};
use finstack_quant_core::dates::{
    canonical_calendar_id, days_since_epoch as rust_days_since_epoch,
    fx::resolve_calendar as rust_resolve_calendar, BusinessDayConvention, DateExt, HolidayCalendar,
};
use wasm_bindgen::prelude::*;

/// Business-day adjustment convention (ISDA 2006 Definitions, Section 4.12).
///
/// Each static factory is one Rust `BusinessDayConvention` variant;
/// `toString()` is the canonical snake_case name that `adjust`,
/// `ScheduleBuilder.adjustWith` and the JSON wire format accept.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const convention = core.BusinessDayConvention.modifiedFollowing();
/// convention.toString(); // "modified_following"
/// core.BusinessDayConvention.fromName("following").toString(); // "following"
/// ```
#[wasm_bindgen(js_name = BusinessDayConvention)]
#[derive(Clone, Copy, Debug)]
pub struct JsBusinessDayConvention {
    pub(crate) inner: BusinessDayConvention,
}

impl JsBusinessDayConvention {
    const fn wrap(inner: BusinessDayConvention) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen(js_class = BusinessDayConvention)]
impl JsBusinessDayConvention {
    /// Leave the date unchanged even when it is not a business day.
    ///
    /// @returns The `unadjusted` convention.
    #[wasm_bindgen(js_name = unadjusted)]
    pub fn unadjusted() -> Self {
        Self::wrap(BusinessDayConvention::Unadjusted)
    }

    /// Roll forward to the next business day.
    ///
    /// @returns The `following` convention.
    #[wasm_bindgen(js_name = following)]
    pub fn following() -> Self {
        Self::wrap(BusinessDayConvention::Following)
    }

    /// Roll forward unless that crosses a month end, then roll backward.
    ///
    /// @returns The `modified_following` convention (the Rust default).
    #[wasm_bindgen(js_name = modifiedFollowing)]
    pub fn modified_following() -> Self {
        Self::wrap(BusinessDayConvention::ModifiedFollowing)
    }

    /// Roll backward to the previous business day.
    ///
    /// @returns The `preceding` convention.
    #[wasm_bindgen(js_name = preceding)]
    pub fn preceding() -> Self {
        Self::wrap(BusinessDayConvention::Preceding)
    }

    /// Roll backward unless that crosses a month start, then roll forward.
    ///
    /// @returns The `modified_preceding` convention.
    #[wasm_bindgen(js_name = modifiedPreceding)]
    pub fn modified_preceding() -> Self {
        Self::wrap(BusinessDayConvention::ModifiedPreceding)
    }

    /// Roll to the nearest business day (forward on a tie).
    ///
    /// @returns The `nearest` convention.
    #[wasm_bindgen(js_name = nearest)]
    pub fn nearest() -> Self {
        Self::wrap(BusinessDayConvention::Nearest)
    }

    /// Parse a convention name (Rust `BusinessDayConvention::from_str`).
    ///
    /// # Arguments
    ///
    /// * `name` - Convention name such as `"following"`,
    ///   `"modified_following"` or `"preceding"`; case and `-`/space separators
    ///   are normalised by the Rust parser.
    ///
    /// @returns The matching `BusinessDayConvention`.
    /// @throws `TypeError` (kind `invalid_type`) if `name` is not a string;
    /// `FinstackError` (kind `validation`) if no convention matches.
    #[wasm_bindgen(js_name = fromName)]
    pub fn from_name(name: JsValue) -> Result<JsBusinessDayConvention, JsValue> {
        js_string(&name, "name")?
            .parse::<BusinessDayConvention>()
            .map(Self::wrap)
            .map_err(to_js_err)
    }

    /// Canonical snake_case name of the convention.
    ///
    /// @returns The name accepted by `fromName` and the JSON wire format.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}

/// Holiday calendar resolved from the built-in registry.
///
/// A calendar classifies dates as holidays or business days. `+`-joined
/// codes such as `"nyse+gblo"` resolve to the union calendar, on which a day
/// is a business day only when every member market is open.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const calendar = new core.HolidayCalendar("nyse");
/// calendar.isHoliday(core.createDate(2025, 1, 1)); // true
/// calendar.isBusinessDay(core.createDate(2025, 1, 6)); // true
/// calendar.code; // "nyse"
/// ```
#[wasm_bindgen(js_name = HolidayCalendar)]
#[derive(Clone)]
pub struct JsHolidayCalendar {
    /// Resolved registry calendar (built-in or interned union).
    calendar: &'static dyn HolidayCalendar,
    /// Canonical id (Rust `canonical_calendar_id`).
    code: String,
}

#[wasm_bindgen(js_class = HolidayCalendar)]
impl JsHolidayCalendar {
    /// Resolve a calendar by its registry id.
    ///
    /// # Arguments
    ///
    /// * `code` - Built-in calendar id in any case (for example `"target2"`
    ///   or `"nyse"`; see `availableCalendars()`), or `+`-joined ids for a
    ///   union calendar (`"nyse+gblo"`).
    ///
    /// @returns The resolved `HolidayCalendar`.
    /// @throws `TypeError` (kind `invalid_type`) if `code` is not a string;
    /// `FinstackError` (kind `not_found`) naming close matches if `code` or a
    /// `+` member is not a registered calendar.
    #[wasm_bindgen(constructor)]
    pub fn new(code: JsValue) -> Result<JsHolidayCalendar, JsValue> {
        let code = js_string(&code, "code")?;
        let calendar = rust_resolve_calendar(Some(&code)).map_err(to_js_err)?;
        let code = canonical_calendar_id(&code).map_err(to_js_err)?;
        Ok(Self { calendar, code })
    }

    /// Whether a date is a holiday on this calendar.
    ///
    /// # Arguments
    ///
    /// * `date` - Date as days since 1970-01-01.
    ///
    /// @returns `true` for a holiday; weekends follow the calendar's weekend rule.
    /// @throws `TypeError` if `date` is not an integer; `FinstackError` (kind
    /// `validation`) if it is outside the supported date range.
    #[wasm_bindgen(js_name = isHoliday)]
    pub fn is_holiday(&self, date: JsValue) -> Result<bool, JsValue> {
        Ok(self.calendar.is_holiday(js_epoch_days(&date, "date")?))
    }

    /// Whether a date is a business day (neither a weekend nor a holiday).
    ///
    /// # Arguments
    ///
    /// * `date` - Date as days since 1970-01-01.
    ///
    /// @returns `true` when the market is open on `date`.
    /// @throws `TypeError` if `date` is not an integer; `FinstackError` (kind
    /// `validation`) if it is outside the supported date range.
    #[wasm_bindgen(js_name = isBusinessDay)]
    pub fn is_business_day(&self, date: JsValue) -> Result<bool, JsValue> {
        Ok(self.calendar.is_business_day(js_epoch_days(&date, "date")?))
    }

    /// Count business days in the half-open interval `[start, end)`.
    ///
    /// # Arguments
    ///
    /// * `start` - First date counted, as days since 1970-01-01.
    /// * `end` - Exclusive end date, as days since 1970-01-01.
    ///
    /// @returns Number of business days; `0` when `start` is on or after `end`.
    /// @throws `TypeError` if a date is not an integer; `FinstackError` (kind
    /// `validation`) if a date is outside the supported range.
    #[wasm_bindgen(js_name = countBusinessDays)]
    pub fn count_business_days(&self, start: JsValue, end: JsValue) -> Result<i32, JsValue> {
        Ok(self
            .calendar
            .count_business_days(js_epoch_days(&start, "start")?, js_epoch_days(&end, "end")?))
    }

    /// Calendar metadata, or `undefined` for union calendars.
    ///
    /// A plain `CalendarMetadata` object with `id`, `name`,
    /// `ignore_weekends` and `weekend_rule`.
    #[wasm_bindgen(getter, js_name = metadata)]
    pub fn metadata(&self) -> Result<JsValue, JsValue> {
        match self.calendar.metadata() {
            Some(metadata) => to_js_value(&metadata),
            None => Ok(JsValue::UNDEFINED),
        }
    }

    /// Canonical calendar id: the registry id, or the sorted `a+b` union form.
    #[wasm_bindgen(getter, js_name = code)]
    pub fn code(&self) -> String {
        self.code.clone()
    }

    /// Canonical calendar id (same as `code`).
    ///
    /// @returns The id accepted by `new HolidayCalendar(code)`.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.code.clone()
    }
}

/// Resolve a calendar code argument.
fn calendar_arg(value: &JsValue, label: &str) -> Result<&'static dyn HolidayCalendar, JsValue> {
    let code = js_string(value, label)?;
    rust_resolve_calendar(Some(&code)).map_err(to_js_err)
}

/// Convert an ISO-8601 date to epoch days (Rust `days_since_epoch`).
///
/// This is the bridge from the ISO strings used by curves and series to the
/// epoch-day numbers used by the `core` date utilities.
///
/// # Arguments
///
/// * `date` - Calendar date as strict ISO-8601 text (`"YYYY-MM-DD"`).
///
/// @returns Days since 1970-01-01 (negative before the epoch).
/// @throws `TypeError` (kind `invalid_type`) if `date` is not a string;
/// `FinstackError` (kind `validation`) if it is not a valid ISO date.
#[wasm_bindgen(js_name = daysSinceEpoch)]
pub fn days_since_epoch(date: JsValue) -> Result<i32, JsValue> {
    let date = parse_iso_date(&js_string(&date, "date")?)?;
    Ok(rust_days_since_epoch(date))
}

/// Add business days on a holiday calendar (Rust `DateExt::add_business_days`).
///
/// # Arguments
///
/// * `date` - Start date as days since 1970-01-01.
/// * `n` - Signed number of business days to move; negative moves backward
///   and `0` returns `date` unchanged, even on a holiday.
/// * `calendar` - Registered holiday-calendar id (for example `"nyse"`, or a
///   `+`-joined union).
///
/// @returns The shifted date as epoch days.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `not_found`) for an unknown calendar, or kind `validation` if no business
/// day is found within the bounded search window.
#[wasm_bindgen(js_name = addBusinessDays)]
pub fn add_business_days(date: JsValue, n: JsValue, calendar: JsValue) -> Result<i32, JsValue> {
    let start = js_epoch_days(&date, "date")?;
    let n: i32 = js_int(&n, "n")?;
    let calendar = calendar_arg(&calendar, "calendar")?;
    start
        .add_business_days(n, calendar)
        .map(rust_days_since_epoch)
        .map_err(to_js_err)
}

/// Add weekdays, skipping Saturdays and Sundays only (Rust `DateExt::add_weekdays`).
///
/// # Arguments
///
/// * `date` - Start date as days since 1970-01-01.
/// * `n` - Signed number of weekdays to move; holidays are not considered.
///
/// @returns The shifted date as epoch days.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if `date` is outside the supported range.
#[wasm_bindgen(js_name = addWeekdays)]
pub fn add_weekdays(date: JsValue, n: JsValue) -> Result<i32, JsValue> {
    let start = js_epoch_days(&date, "date")?;
    let n: i32 = js_int(&n, "n")?;
    Ok(rust_days_since_epoch(start.add_weekdays(n)))
}

/// Add calendar months, clamping to the last day of the target month (Rust
/// `DateExt::add_months`).
///
/// # Arguments
///
/// * `date` - Start date as days since 1970-01-01.
/// * `months` - Signed number of months to move (`Jan 31 + 1` gives the last
///   day of February).
///
/// @returns The shifted date as epoch days.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if `date` is outside the supported range.
#[wasm_bindgen(js_name = addMonths)]
pub fn add_months(date: JsValue, months: JsValue) -> Result<i32, JsValue> {
    let start = js_epoch_days(&date, "date")?;
    let months: i32 = js_int(&months, "months")?;
    Ok(rust_days_since_epoch(start.add_months(months)))
}

/// Last calendar day of the date's month (Rust `DateExt::end_of_month`).
///
/// # Arguments
///
/// * `date` - Any date in the month, as days since 1970-01-01.
///
/// @returns The month-end date as epoch days.
/// @throws `TypeError` if `date` is not an integer; `FinstackError` (kind
/// `validation`) if it is outside the supported range.
#[wasm_bindgen(js_name = endOfMonth)]
pub fn end_of_month(date: JsValue) -> Result<i32, JsValue> {
    Ok(rust_days_since_epoch(
        js_epoch_days(&date, "date")?.end_of_month(),
    ))
}

/// Whether a date falls on a Saturday or Sunday (Rust `DateExt::is_weekend`).
///
/// # Arguments
///
/// * `date` - Date as days since 1970-01-01.
///
/// @returns `true` on Saturday or Sunday.
/// @throws `TypeError` if `date` is not an integer; `FinstackError` (kind
/// `validation`) if it is outside the supported range.
#[wasm_bindgen(js_name = isWeekend)]
pub fn is_weekend(date: JsValue) -> Result<bool, JsValue> {
    Ok(js_epoch_days(&date, "date")?.is_weekend())
}

/// Calendar quarter of a date (Rust `DateExt::quarter`).
///
/// # Arguments
///
/// * `date` - Date as days since 1970-01-01.
///
/// @returns The quarter number, `1` through `4`.
/// @throws `TypeError` if `date` is not an integer; `FinstackError` (kind
/// `validation`) if it is outside the supported range.
#[wasm_bindgen(js_name = quarter)]
pub fn quarter(date: JsValue) -> Result<u8, JsValue> {
    Ok(js_epoch_days(&date, "date")?.quarter())
}

/// Fiscal year containing a date (Rust `DateExt::fiscal_year`).
///
/// # Arguments
///
/// * `date` - Date as days since 1970-01-01.
/// * `config` - Fiscal-year start month and day that decide which fiscal
///   year the date belongs to (for example `FiscalConfig.usFederal()`).
///
/// @returns The fiscal year the date falls in.
/// @throws `TypeError` if `date` is not an integer; `FinstackError` (kind
/// `validation`) if it is outside the supported range.
#[wasm_bindgen(js_name = fiscalYear)]
pub fn fiscal_year(date: JsValue, config: &JsFiscalConfig) -> Result<i32, JsValue> {
    Ok(js_epoch_days(&date, "date")?.fiscal_year(config.inner))
}

/// Whole months from `date` to `other` (Rust `DateExt::months_until`).
///
/// # Arguments
///
/// * `date` - Start date as days since 1970-01-01.
/// * `other` - End date as days since 1970-01-01.
///
/// @returns `(other.year - date.year) * 12 + (other.month - date.month)`, ignoring
/// the day of month; `0` when `other` is before `date`.
/// @throws `TypeError` if a date is not an integer; `FinstackError` (kind
/// `validation`) if a date is outside the supported range.
#[wasm_bindgen(js_name = monthsUntil)]
pub fn months_until(date: JsValue, other: JsValue) -> Result<u32, JsValue> {
    Ok(js_epoch_days(&date, "date")?.months_until(js_epoch_days(&other, "other")?))
}
