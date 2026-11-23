//! Date creation, adjustment, and holiday-calendar WASM entry points.

use crate::api::core::calendar::calendar_arg;
use crate::utils::input::{js_epoch_days, js_int, js_opt_string, js_string, js_uint};
use crate::utils::to_js_err;
use finstack_quant_core::dates::{
    BusinessDayConvention, HolidayCalendar, adjust as core_adjust,
    available_calendars as core_available_calendars, calendar_by_id_strict,
};
use wasm_bindgen::prelude::*;

/// Resolve the optional calendar and business-day convention of a tenor roll.
///
/// The convention is required: there is no default convention.
pub(super) fn roll_rule(
    calendar_code: Option<JsValue>,
    convention: Option<JsValue>,
) -> Result<(Option<&'static dyn HolidayCalendar>, BusinessDayConvention), JsValue> {
    let calendar = js_opt_string(calendar_code.as_ref(), "calendarCode")?
        .map(|code| calendar_by_id_strict(&code))
        .transpose()
        .map_err(to_js_err)?;
    let convention = match js_opt_string(convention.as_ref(), "convention")? {
        Some(name) => name.parse().map_err(to_js_err)?,
        None => {
            return Err(to_js_err(finstack_quant_core::Error::Validation(
                "a business-day convention is required: there is no default convention".to_string(),
            )));
        }
    };
    Ok((calendar, convention))
}

/// Create a date and return it as epoch days (days since 1970-01-01).
/// # Arguments
///
/// * `year` - Integral calendar year supported by the Rust date type.
/// * `month` - Integral calendar month number from 1 through 12.
/// * `day` - Integral calendar day number within the selected month, from 1 through 31.
///
/// # Errors
///
/// Throws a JavaScript exception if any component is non-finite or fractional,
/// `month` is outside `1..=12`, or the supplied year, month, and day do not form a representable calendar date.
/// @param year - Integral calendar year supported by the Rust date type.
/// @param month - Integral calendar month number from 1 through 12.
/// @param day - Integral calendar day number within the selected month, from 1 through 31.
/// @returns Days since 1970-01-01 (Unix epoch).
#[wasm_bindgen(js_name = createDate)]
pub fn create_date(
    #[wasm_bindgen(unchecked_param_type = "number")] year: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] month: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] day: JsValue,
) -> Result<i32, JsValue> {
    let year: i32 = js_int(&year, "year")?;
    let month: u8 = js_uint(&month, "month")?;
    let day: u8 = js_uint(&day, "day")?;
    let m = time::Month::try_from(month).map_err(to_js_err)?;
    let date = finstack_quant_core::dates::create_date(year, m, day).map_err(to_js_err)?;
    Ok(finstack_quant_core::dates::days_since_epoch(date))
}

/// Convert epoch days back to `[year, month, day]` as a JS array-compatible triple.
/// # Arguments
///
/// * `days` - Integral number of days since 1970-01-01 to decompose into year, month, and day.
///
/// # Errors
///
/// Throws a JavaScript exception if `days` is non-finite, fractional, or outside the representable date range.
/// @param days - Integral number of days since 1970-01-01 to decompose into year, month, and day.
/// @returns `[year, month, day]` as an `Int32Array`, with month in `1..=12`.
#[wasm_bindgen(js_name = dateFromEpochDays)]
pub fn date_from_epoch_days(
    #[wasm_bindgen(unchecked_param_type = "number")] days: JsValue,
) -> Result<Vec<i32>, JsValue> {
    let date = js_epoch_days(&days, "days")?;
    Ok(vec![date.year(), date.month() as i32, date.day() as i32])
}

/// Adjust a date (epoch days) according to a business-day convention and calendar.
///
/// Returns the adjusted date as epoch days.
///
/// # Arguments
///
/// * `epoch_days` - Integral unadjusted date as days since 1970-01-01.
/// * `convention` - Canonical business-day convention name, such as following or modified_following.
/// * `calendar_code` - Registered holiday-calendar identifier used to select business days; unknown identifiers are rejected.
///
/// # Errors
///
/// Throws a JavaScript exception if `epochDays` is non-finite, fractional, or outside the representable date
/// range, `convention` is unrecognized, `calendarCode` is unknown, or adjustment
/// cannot produce a representable business date.
/// @param epochDays - Integral unadjusted date as days since 1970-01-01.
/// @param convention - Canonical business-day convention name, such as following or modified_following.
/// @param calendarCode - Registered holiday-calendar identifier used to select business days; unknown identifiers are rejected.
/// @returns Adjusted date as days since 1970-01-01.
#[wasm_bindgen(js_name = adjust)]
pub fn adjust(
    #[wasm_bindgen(js_name = "epochDays", unchecked_param_type = "number")] epoch_days: JsValue,
    #[wasm_bindgen(unchecked_param_type = "string")] convention: JsValue,
    #[wasm_bindgen(js_name = "calendarCode", unchecked_param_type = "string")]
    calendar_code: JsValue,
) -> Result<i32, JsValue> {
    let business_day_convention: BusinessDayConvention = js_string(&convention, "convention")?
        .parse()
        .map_err(to_js_err)?;
    let cal = calendar_arg(&calendar_code, "calendarCode")?;
    let date = js_epoch_days(&epoch_days, "epochDays")?;
    let adjusted = core_adjust(date, business_day_convention, cal).map_err(to_js_err)?;
    Ok(finstack_quant_core::dates::days_since_epoch(adjusted))
}

/// Return the list of available calendar codes.
/// @returns Registered holiday-calendar identifiers, sorted alphabetically.
#[wasm_bindgen(js_name = availableCalendars)]
pub fn available_calendars() -> Vec<String> {
    core_available_calendars()
        .iter()
        .map(|s| s.to_string())
        .collect()
}
