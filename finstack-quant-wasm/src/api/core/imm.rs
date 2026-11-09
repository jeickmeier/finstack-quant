//! WASM bindings for market date rules from [`finstack_quant_core::dates`]:
//! IMM and CDS roll dates, third-Friday lattice dates and published SIFMA settlement dates.
//!
//! Dates are **epoch days** (`i32`, days since 1970-01-01).

use crate::utils::input::{js_epoch_days, js_int, js_string, js_uint};
use crate::utils::to_js_err;
use finstack_quant_core::dates::{
    self as rust_dates, Date, SifmaSettlementClass, TenorUnit, days_since_epoch,
};
use time::Month;
use wasm_bindgen::prelude::*;

/// Read a `(month, year)` argument pair; the Rust date rule validates the year.
fn month_year(month: &JsValue, year: &JsValue) -> Result<(Month, i32), JsValue> {
    let month: u8 = js_uint(month, "month")?;
    let year: i32 = js_int(year, "year")?;
    let month = Month::try_from(month).map_err(to_js_err)?;
    Ok((month, year))
}

/// Apply a date-to-date rule to an epoch-day argument.
fn map_date(
    date: &JsValue,
    rule: fn(Date) -> finstack_quant_core::Result<Date>,
) -> Result<i32, JsValue> {
    rule(js_epoch_days(date, "date")?)
        .map(days_since_epoch)
        .map_err(to_js_err)
}

/// Third Wednesday of a month, the standard IMM date (Rust `third_wednesday`).
///
/// # Arguments
///
/// * `month` - Calendar month number, `1` through `12`.
/// * `year` - Four-digit calendar year.
///
/// @returns The third Wednesday of that month as epoch days.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if `month` is outside `1..=12` or `year` is outside the
/// supported date range.
/// @param month - Calendar month number, `1` through `12`.
/// @param year - Four-digit calendar year.
#[wasm_bindgen(js_name = thirdWednesday)]
pub fn third_wednesday(
    #[wasm_bindgen(unchecked_param_type = "number")] month: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] year: JsValue,
) -> Result<i32, JsValue> {
    let (month, year) = month_year(&month, &year)?;
    rust_dates::third_wednesday(month, year)
        .map(days_since_epoch)
        .map_err(to_js_err)
}

/// Third Friday of a month, the standard equity-option expiry (Rust `third_friday`).
///
/// # Arguments
///
/// * `month` - Calendar month number, `1` through `12`.
/// * `year` - Four-digit calendar year.
///
/// @returns The third Friday of that month as epoch days.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if `month` is outside `1..=12` or `year` is outside the
/// supported date range.
/// @param month - Calendar month number, `1` through `12`.
/// @param year - Four-digit calendar year.
#[wasm_bindgen(js_name = thirdFriday)]
pub fn third_friday(
    #[wasm_bindgen(unchecked_param_type = "number")] month: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] year: JsValue,
) -> Result<i32, JsValue> {
    let (month, year) = month_year(&month, &year)?;
    rust_dates::third_friday(month, year)
        .map(days_since_epoch)
        .map_err(to_js_err)
}

/// Next quarterly IMM date strictly after a date (Rust `next_imm`).
///
/// # Arguments
///
/// * `date` - Reference date as days since 1970-01-01.
///
/// @returns The next third Wednesday of March, June, September or December,
/// as epoch days.
/// @throws `TypeError` if `date` is not an integer; `FinstackError` (kind
/// `validation`) if it is outside the supported date range.
/// @param date - Reference date as days since 1970-01-01.
#[wasm_bindgen(js_name = nextImm)]
pub fn next_imm(
    #[wasm_bindgen(unchecked_param_type = "number")] date: JsValue,
) -> Result<i32, JsValue> {
    map_date(&date, rust_dates::next_imm)
}

/// Whether a date is a quarterly IMM date (Rust `is_imm_date`).
///
/// # Arguments
///
/// * `date` - Date as days since 1970-01-01.
///
/// @returns `true` on the third Wednesday of March, June, September or December.
/// @throws `TypeError` if `date` is not an integer; `FinstackError` (kind
/// `validation`) if it is outside the supported date range.
/// @param date - Date as days since 1970-01-01.
#[wasm_bindgen(js_name = isImmDate)]
pub fn is_imm_date(
    #[wasm_bindgen(unchecked_param_type = "number")] date: JsValue,
) -> Result<bool, JsValue> {
    Ok(rust_dates::is_imm_date(js_epoch_days(&date, "date")?))
}

/// Whether a date is a quarterly CDS roll date (Rust `is_cds_date`).
///
/// # Arguments
///
/// * `date` - Date as days since 1970-01-01.
///
/// @returns `true` on the 20th of March, June, September or December.
/// @throws `TypeError` if `date` is not an integer; `FinstackError` (kind
/// `validation`) if it is outside the supported date range.
/// @param date - Date as days since 1970-01-01.
#[wasm_bindgen(js_name = isCdsDate)]
pub fn is_cds_date(
    #[wasm_bindgen(unchecked_param_type = "number")] date: JsValue,
) -> Result<bool, JsValue> {
    Ok(rust_dates::is_cds_date(js_epoch_days(&date, "date")?))
}

/// Next quarterly CDS date strictly after a date (Rust `next_cds_date`).
///
/// # Arguments
///
/// * `date` - Reference date as days since 1970-01-01.
///
/// @returns The next 20th of March, June, September or December, as epoch days.
/// @throws `TypeError` if `date` is not an integer; `FinstackError` (kind
/// `validation`) if it is outside the supported date range.
/// @param date - Reference date as days since 1970-01-01.
#[wasm_bindgen(js_name = nextCdsDate)]
pub fn next_cds_date(
    #[wasm_bindgen(unchecked_param_type = "number")] date: JsValue,
) -> Result<i32, JsValue> {
    map_date(&date, rust_dates::next_cds_date)
}

/// Previous quarterly CDS date strictly before a date (Rust `prev_cds_date`).
///
/// # Arguments
///
/// * `date` - Reference date as days since 1970-01-01.
///
/// @returns The latest 20th of March, June, September or December before
/// `date`, as epoch days; a roll date returns the preceding roll.
/// @throws `TypeError` if `date` is not an integer; `FinstackError` (kind
/// `validation`) if it is outside the supported date range.
/// @param date - Reference date as days since 1970-01-01.
#[wasm_bindgen(js_name = prevCdsDate)]
pub fn prev_cds_date(
    #[wasm_bindgen(unchecked_param_type = "number")] date: JsValue,
) -> Result<i32, JsValue> {
    map_date(&date, rust_dates::prev_cds_date)
}

/// Most recent semi-annual CDS roll on or before a date (Rust
/// `prev_cds_semiannual_roll`).
///
/// # Arguments
///
/// * `date` - Reference date as days since 1970-01-01.
///
/// @returns The latest 20 March or 20 September that is not after `date`, as
/// epoch days.
/// @throws `TypeError` if `date` is not an integer; `FinstackError` (kind
/// `validation`) if it is outside the supported date range.
/// @param date - Reference date as days since 1970-01-01.
#[wasm_bindgen(js_name = prevCdsSemiannualRoll)]
pub fn prev_cds_semiannual_roll(
    #[wasm_bindgen(unchecked_param_type = "number")] date: JsValue,
) -> Result<i32, JsValue> {
    map_date(&date, rust_dates::prev_cds_semiannual_roll)
}

/// Standard semi-annual CDS maturity on or after a date (Rust
/// `next_semiannual_cds_maturity`).
///
/// # Arguments
///
/// * `date` - Unadjusted candidate maturity (roll date plus tenor) as days
///   since 1970-01-01.
///
/// @returns The first 20 June or 20 December on or after `date`, as epoch
/// days; a date already on that grid is returned unchanged.
/// @throws `TypeError` if `date` is not an integer; `FinstackError` (kind
/// `validation`) if it is outside the supported date range.
/// @param date - Unadjusted candidate maturity (roll date plus tenor) as days since 1970-01-01.
#[wasm_bindgen(js_name = nextSemiannualCdsMaturity)]
pub fn next_semiannual_cds_maturity(
    #[wasm_bindgen(unchecked_param_type = "number")] date: JsValue,
) -> Result<i32, JsValue> {
    map_date(&date, rust_dates::next_semiannual_cds_maturity)
}

/// Next third Friday of a month strictly after a date, the monthly
/// equity-option expiry (Rust `next_third_friday`).
///
/// # Arguments
///
/// * `date` - Reference date as days since 1970-01-01.
///
/// @returns The next third Friday of a month, as epoch days.
/// @throws `TypeError` if `date` is not an integer; `FinstackError` (kind
/// `validation`) if it is outside the supported date range.
/// @param date - Reference date as days since 1970-01-01.
#[wasm_bindgen(js_name = nextThirdFriday)]
pub fn next_third_friday(
    #[wasm_bindgen(unchecked_param_type = "number")] date: JsValue,
) -> Result<i32, JsValue> {
    map_date(&date, rust_dates::next_third_friday)
}

/// SIFMA agency-MBS settlement class (A through D).
///
/// SIFMA publishes a separate TBA settlement date per class each month.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const cls = core.SifmaSettlementClass.b();
/// cls.toString(); // "b"
/// core.sifmaSettlementDate(1, 2026, cls); // epoch days; throws when not maintained
/// ```
#[wasm_bindgen(js_name = SifmaSettlementClass)]
#[derive(Clone, Copy, Debug)]
pub struct JsSifmaSettlementClass {
    pub(crate) inner: SifmaSettlementClass,
}

impl JsSifmaSettlementClass {
    const fn from_inner(inner: SifmaSettlementClass) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen(js_class = SifmaSettlementClass)]
impl JsSifmaSettlementClass {
    /// Class A: conventional 30-year pools (FNMA/FHLMC UMBS).
    ///
    /// @returns Settlement class A (the Rust default).
    #[wasm_bindgen(js_name = a)]
    pub fn a() -> Self {
        Self::from_inner(SifmaSettlementClass::A)
    }

    /// Class B: fixed-rate 15-year agency pools.
    ///
    /// @returns Settlement class B.
    #[wasm_bindgen(js_name = b)]
    pub fn b() -> Self {
        Self::from_inner(SifmaSettlementClass::B)
    }

    /// Class C: GNMA single-family 30-year pools.
    ///
    /// @returns Settlement class C.
    #[wasm_bindgen(js_name = c)]
    pub fn c() -> Self {
        Self::from_inner(SifmaSettlementClass::C)
    }

    /// Class D: balloons, ARMs, multifamily and other non-standard products.
    ///
    /// @returns Settlement class D.
    #[wasm_bindgen(js_name = d)]
    pub fn d() -> Self {
        Self::from_inner(SifmaSettlementClass::D)
    }

    /// Lower-case class letter (`"a"` through `"d"`), the JSON wire label.
    ///
    /// @returns The class label.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}

/// Published SIFMA settlement date of a month for one class (Rust
/// `sifma_settlement_date`).
///
/// The class is required: there is no default class.
///
/// # Arguments
///
/// * `month` - Settlement month number, `1` through `12`.
/// * `year` - Settlement year.
/// * `settlement_class` - Agency-MBS settlement class whose date is requested.
///
/// @param month - Settlement month number, `1` through `12`.
/// @param year - Settlement year.
/// @param settlementClass - Agency-MBS settlement class whose date is requested.
/// @returns The maintained published settlement date as epoch days.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `not_found`) when the maintained SIFMA data has no date for that month,
/// year and class; `FinstackError` (kind `validation`) if `month` is outside
/// `1..=12` or the stored date is invalid.
#[wasm_bindgen(js_name = sifmaSettlementDate)]
pub fn sifma_settlement_date(
    #[wasm_bindgen(unchecked_param_type = "number")] month: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] year: JsValue,
    #[wasm_bindgen(js_name = "settlementClass")] settlement_class: &JsSifmaSettlementClass,
) -> Result<i32, JsValue> {
    let (month, year) = month_year(&month, &year)?;
    rust_dates::sifma_settlement_date(month, year, settlement_class.inner)
        .map(days_since_epoch)
        .map_err(to_js_err)
}

/// Next published SIFMA settlement for one class strictly after a date (Rust
/// `next_sifma_settlement`).
///
/// # Arguments
///
/// * `date` - Reference date as days since 1970-01-01; a settlement on this
///   date is not returned.
/// * `settlement_class` - Agency-MBS settlement class whose published dates
///   are searched.
///
/// @param date - Reference date as days since 1970-01-01; a settlement on this date is not returned.
/// @param settlementClass - Agency-MBS settlement class whose published dates are searched.
/// @returns The current month's published date when it is later than `date`,
/// otherwise the following month's, as epoch days.
/// @throws `TypeError` if `date` is not an integer; `FinstackError` (kind
/// `not_found`) when a required current- or following-month date is not in
/// the maintained data (gaps are never skipped); `FinstackError` (kind
/// `validation`) if `date` is outside the supported range or has no later month.
#[wasm_bindgen(js_name = nextSifmaSettlement)]
pub fn next_sifma_settlement(
    #[wasm_bindgen(unchecked_param_type = "number")] date: JsValue,
    #[wasm_bindgen(js_name = "settlementClass")] settlement_class: &JsSifmaSettlementClass,
) -> Result<i32, JsValue> {
    rust_dates::next_sifma_settlement(js_epoch_days(&date, "date")?, settlement_class.inner)
        .map(days_since_epoch)
        .map_err(to_js_err)
}

/// Unit of a tenor: days, weeks, months or years.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// core.TenorUnit.months().toString(); // "M"
/// core.TenorUnit.fromName("y").toString(); // "Y"
/// ```
#[wasm_bindgen(js_name = TenorUnit)]
#[derive(Clone, Copy, Debug)]
pub struct JsTenorUnit {
    pub(crate) inner: TenorUnit,
}

impl JsTenorUnit {
    const fn from_inner(inner: TenorUnit) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen(js_class = TenorUnit)]
impl JsTenorUnit {
    /// Calendar days (`D`).
    ///
    /// @returns The days unit.
    #[wasm_bindgen(js_name = days)]
    pub fn days() -> Self {
        Self::from_inner(TenorUnit::Days)
    }

    /// Calendar weeks (`W`).
    ///
    /// @returns The weeks unit.
    #[wasm_bindgen(js_name = weeks)]
    pub fn weeks() -> Self {
        Self::from_inner(TenorUnit::Weeks)
    }

    /// Calendar months (`M`).
    ///
    /// @returns The months unit.
    #[wasm_bindgen(js_name = months)]
    pub fn months() -> Self {
        Self::from_inner(TenorUnit::Months)
    }

    /// Calendar years (`Y`).
    ///
    /// @returns The years unit.
    #[wasm_bindgen(js_name = years)]
    pub fn years() -> Self {
        Self::from_inner(TenorUnit::Years)
    }

    /// Parse a one-letter unit code (Rust `TenorUnit::from_str`).
    ///
    /// # Arguments
    ///
    /// * `name` - Exactly one character: `D`, `W`, `M` or `Y`, in either case.
    ///
    /// @returns The matching `TenorUnit`.
    /// @throws `TypeError` (kind `invalid_type`) if `name` is not a string;
    /// `FinstackError` (kind `validation`) if it is not exactly one of the
    /// four unit letters.
    /// @param name - Exactly one character: `D`, `W`, `M` or `Y`, in either case.
    #[wasm_bindgen(js_name = fromName)]
    pub fn from_name(
        #[wasm_bindgen(unchecked_param_type = "string")] name: JsValue,
    ) -> Result<JsTenorUnit, JsValue> {
        let text = js_string(&name, "name")?;
        text.parse::<TenorUnit>()
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// One-letter unit code: `"D"`, `"W"`, `"M"` or `"Y"`.
    ///
    /// @returns The designator accepted by `fromName` and used in tenor text.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.designator().to_string()
    }
}
