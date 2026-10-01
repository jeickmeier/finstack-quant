//! WASM bindings for market date rules from [`finstack_quant_core::dates`]:
//! IMM and CDS roll dates, listed-option expiries, SIFMA settlement dates and
//! the 30/360 day counts.
//!
//! Dates are **epoch days** (`i32`, days since 1970-01-01).

use crate::utils::input::{js_bool, js_epoch_days, js_int, js_string, js_uint};
use crate::utils::to_js_err;
use finstack_quant_core::dates::{
    self as rust_dates, create_date, days_since_epoch, Date, SifmaSettlementClass, TenorUnit,
    Thirty360Convention,
};
use finstack_quant_core::wire::{serde_label, serde_parse};
use time::Month;
use wasm_bindgen::prelude::*;

/// Read a `(month, year)` argument pair, rejecting anything that is not a
/// representable calendar month before a date rule runs on it.
fn month_year(month: &JsValue, year: &JsValue) -> Result<(Month, i32), JsValue> {
    let month: u8 = js_uint(month, "month")?;
    let year: i32 = js_int(year, "year")?;
    let month = Month::try_from(month).map_err(to_js_err)?;
    create_date(year, month, 1).map_err(to_js_err)?;
    Ok((month, year))
}

/// Apply a date-to-date rule to an epoch-day argument.
fn map_date(date: &JsValue, rule: fn(Date) -> Date) -> Result<i32, JsValue> {
    Ok(days_since_epoch(rule(js_epoch_days(date, "date")?)))
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
#[wasm_bindgen(js_name = thirdWednesday)]
pub fn third_wednesday(month: JsValue, year: JsValue) -> Result<i32, JsValue> {
    let (month, year) = month_year(&month, &year)?;
    Ok(days_since_epoch(rust_dates::third_wednesday(month, year)))
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
#[wasm_bindgen(js_name = thirdFriday)]
pub fn third_friday(month: JsValue, year: JsValue) -> Result<i32, JsValue> {
    let (month, year) = month_year(&month, &year)?;
    Ok(days_since_epoch(rust_dates::third_friday(month, year)))
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
#[wasm_bindgen(js_name = nextImm)]
pub fn next_imm(date: JsValue) -> Result<i32, JsValue> {
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
#[wasm_bindgen(js_name = isImmDate)]
pub fn is_imm_date(date: JsValue) -> Result<bool, JsValue> {
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
#[wasm_bindgen(js_name = isCdsDate)]
pub fn is_cds_date(date: JsValue) -> Result<bool, JsValue> {
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
#[wasm_bindgen(js_name = nextCdsDate)]
pub fn next_cds_date(date: JsValue) -> Result<i32, JsValue> {
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
#[wasm_bindgen(js_name = prevCdsDate)]
pub fn prev_cds_date(date: JsValue) -> Result<i32, JsValue> {
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
#[wasm_bindgen(js_name = prevCdsSemiannualRoll)]
pub fn prev_cds_semiannual_roll(date: JsValue) -> Result<i32, JsValue> {
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
#[wasm_bindgen(js_name = nextSemiannualCdsMaturity)]
pub fn next_semiannual_cds_maturity(date: JsValue) -> Result<i32, JsValue> {
    map_date(&date, rust_dates::next_semiannual_cds_maturity)
}

/// IMM option expiry of a month: the Friday before the third Wednesday (Rust
/// `imm_option_expiry`).
///
/// # Arguments
///
/// * `month` - Calendar month number, `1` through `12`.
/// * `year` - Four-digit calendar year.
///
/// @returns The option expiry date as epoch days.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if `month` is outside `1..=12` or `year` is outside the
/// supported date range.
#[wasm_bindgen(js_name = immOptionExpiry)]
pub fn imm_option_expiry(month: JsValue, year: JsValue) -> Result<i32, JsValue> {
    let (month, year) = month_year(&month, &year)?;
    Ok(days_since_epoch(rust_dates::imm_option_expiry(month, year)))
}

/// Next quarterly IMM option expiry strictly after a date (Rust
/// `next_imm_option_expiry`).
///
/// # Arguments
///
/// * `date` - Reference date as days since 1970-01-01.
///
/// @returns The next March/June/September/December IMM option expiry, as epoch days.
/// @throws `TypeError` if `date` is not an integer; `FinstackError` (kind
/// `validation`) if it is outside the supported date range.
#[wasm_bindgen(js_name = nextImmOptionExpiry)]
pub fn next_imm_option_expiry(date: JsValue) -> Result<i32, JsValue> {
    map_date(&date, rust_dates::next_imm_option_expiry)
}

/// Next monthly equity-option expiry strictly after a date (Rust
/// `next_equity_option_expiry`).
///
/// # Arguments
///
/// * `date` - Reference date as days since 1970-01-01.
///
/// @returns The next third Friday of a month, as epoch days.
/// @throws `TypeError` if `date` is not an integer; `FinstackError` (kind
/// `validation`) if it is outside the supported date range.
#[wasm_bindgen(js_name = nextEquityOptionExpiry)]
pub fn next_equity_option_expiry(date: JsValue) -> Result<i32, JsValue> {
    map_date(&date, rust_dates::next_equity_option_expiry)
}

/// SIFMA agency-MBS settlement class (A through D).
///
/// SIFMA publishes a separate TBA settlement date per class each month.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const cls = core.SifmaSettlementClass.fromAgencyTerm("FNMA", 15);
/// cls.toString(); // "b"
/// core.sifmaSettlementDateForClass(1, 2026, cls); // epoch days, or undefined
/// ```
#[wasm_bindgen(js_name = SifmaSettlementClass)]
#[derive(Clone, Copy, Debug)]
pub struct JsSifmaSettlementClass {
    pub(crate) inner: SifmaSettlementClass,
}

impl JsSifmaSettlementClass {
    const fn wrap(inner: SifmaSettlementClass) -> Self {
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
        Self::wrap(SifmaSettlementClass::A)
    }

    /// Class B: fixed-rate 15-year agency pools.
    ///
    /// @returns Settlement class B.
    #[wasm_bindgen(js_name = b)]
    pub fn b() -> Self {
        Self::wrap(SifmaSettlementClass::B)
    }

    /// Class C: GNMA single-family 30-year pools.
    ///
    /// @returns Settlement class C.
    #[wasm_bindgen(js_name = c)]
    pub fn c() -> Self {
        Self::wrap(SifmaSettlementClass::C)
    }

    /// Class D: balloons, ARMs, multifamily and other non-standard products.
    ///
    /// @returns Settlement class D.
    #[wasm_bindgen(js_name = d)]
    pub fn d() -> Self {
        Self::wrap(SifmaSettlementClass::D)
    }

    /// Infer the settlement class from the agency program and original term
    /// (Rust `SifmaSettlementClass::from_agency_term`).
    ///
    /// # Arguments
    ///
    /// * `agency` - Agency program label; a value containing `GNMA` or `GN`
    ///   (any case) is treated as Ginnie Mae.
    /// * `term_years` - Original mortgage term in whole years: 15-year pools
    ///   are class B, conventional 30-year pools class A, GNMA 30-year pools
    ///   class C, anything else class D.
    ///
    /// @returns The inferred `SifmaSettlementClass`.
    /// @throws `TypeError` if `agency` is not a string or `termYears` is not a
    /// non-negative integer.
    #[wasm_bindgen(js_name = fromAgencyTerm)]
    pub fn from_agency_term(
        agency: JsValue,
        term_years: JsValue,
    ) -> Result<JsSifmaSettlementClass, JsValue> {
        Ok(Self::wrap(SifmaSettlementClass::from_agency_term(
            &js_string(&agency, "agency")?,
            js_uint(&term_years, "termYears")?,
        )))
    }

    /// Lower-case class letter (`"a"` through `"d"`), the JSON wire label.
    ///
    /// @returns The class label.
    /// @throws If the label cannot be produced (not expected).
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string(&self) -> Result<String, JsValue> {
        serde_label(&self.inner).map_err(to_js_err)
    }
}

/// Published SIFMA class A settlement date of a month (Rust `sifma_settlement_date`).
///
/// # Arguments
///
/// * `month` - Settlement month number, `1` through `12`.
/// * `year` - Settlement year.
///
/// @returns The published date as epoch days, or `undefined` when the month
/// is outside the embedded SIFMA calendar (dates are never approximated).
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if `month` is outside `1..=12` or `year` is outside the
/// supported date range.
#[wasm_bindgen(js_name = sifmaSettlementDate)]
pub fn sifma_settlement_date(month: JsValue, year: JsValue) -> Result<Option<i32>, JsValue> {
    let (month, year) = month_year(&month, &year)?;
    Ok(rust_dates::sifma_settlement_date(month, year).map(days_since_epoch))
}

/// Published SIFMA settlement date of a month for one class (Rust
/// `sifma_settlement_date_for_class`).
///
/// # Arguments
///
/// * `month` - Settlement month number, `1` through `12`.
/// * `year` - Settlement year.
/// * `settlement_class` - Agency-MBS settlement class whose date is requested.
///
/// @returns The published date as epoch days, or `undefined` when that month
/// and class are outside the embedded SIFMA calendar.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if `month` is outside `1..=12` or `year` is outside the
/// supported date range.
#[wasm_bindgen(js_name = sifmaSettlementDateForClass)]
pub fn sifma_settlement_date_for_class(
    month: JsValue,
    year: JsValue,
    settlement_class: &JsSifmaSettlementClass,
) -> Result<Option<i32>, JsValue> {
    let (month, year) = month_year(&month, &year)?;
    Ok(
        rust_dates::sifma_settlement_date_for_class(month, year, settlement_class.inner)
            .map(days_since_epoch),
    )
}

/// Projection-only estimate of a SIFMA settlement date (Rust
/// `estimated_sifma_settlement_date_for_class`).
///
/// The estimate counts business days of the month on the SIFMA calendar; use
/// `sifmaSettlementDateForClass` for published dates.
///
/// # Arguments
///
/// * `month` - Settlement month number, `1` through `12`.
/// * `year` - Settlement year.
/// * `settlement_class` - Agency-MBS settlement class whose business-day
///   anchor is used.
///
/// @returns The estimated settlement date as epoch days.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if `month` is outside `1..=12` or `year` is outside the
/// supported date range.
#[wasm_bindgen(js_name = estimatedSifmaSettlementDateForClass)]
pub fn estimated_sifma_settlement_date_for_class(
    month: JsValue,
    year: JsValue,
    settlement_class: &JsSifmaSettlementClass,
) -> Result<i32, JsValue> {
    let (month, year) = month_year(&month, &year)?;
    Ok(days_since_epoch(
        rust_dates::estimated_sifma_settlement_date_for_class(month, year, settlement_class.inner),
    ))
}

/// Next published SIFMA class A settlement strictly after a date (Rust
/// `next_sifma_settlement`).
///
/// # Arguments
///
/// * `date` - Reference date as days since 1970-01-01; a settlement on this
///   date is not returned.
///
/// @returns The next settlement date as epoch days, or `undefined` when a
/// required month is outside the embedded SIFMA calendar.
/// @throws `TypeError` if `date` is not an integer; `FinstackError` (kind
/// `validation`) if it is outside the supported date range.
#[wasm_bindgen(js_name = nextSifmaSettlement)]
pub fn next_sifma_settlement(date: JsValue) -> Result<Option<i32>, JsValue> {
    Ok(rust_dates::next_sifma_settlement(js_epoch_days(&date, "date")?).map(days_since_epoch))
}

/// 30/360 day-count variant used by `days30360`.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const isda = core.Thirty360Convention.isda();
/// core.days30360(core.createDate(2025, 1, 31), core.createDate(2025, 3, 31), isda.toString()); // 60
/// ```
#[wasm_bindgen(js_name = Thirty360Convention)]
#[derive(Clone, Copy, Debug)]
pub struct JsThirty360Convention {
    pub(crate) inner: Thirty360Convention,
}

impl JsThirty360Convention {
    const fn wrap(inner: Thirty360Convention) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen(js_class = Thirty360Convention)]
impl JsThirty360Convention {
    /// 30U/360 (US SIA bond basis).
    ///
    /// @returns The `us_sia` variant.
    #[wasm_bindgen(js_name = usSia)]
    pub fn us_sia() -> Self {
        Self::wrap(Thirty360Convention::UsSia)
    }

    /// 30/360 ISDA bond basis (ISDA 2006 Section 4.16(f); no February month-end rule).
    ///
    /// @returns The `isda` variant.
    #[wasm_bindgen(js_name = isda)]
    pub fn isda() -> Self {
        Self::wrap(Thirty360Convention::Isda)
    }

    /// 30E/360 (European): day 31 becomes 30 on both dates.
    ///
    /// @returns The `european` variant.
    #[wasm_bindgen(js_name = european)]
    pub fn european() -> Self {
        Self::wrap(Thirty360Convention::European)
    }

    /// 30/360 Italian: day 31 and any February day after the 27th become 30.
    ///
    /// @returns The `italian` variant.
    #[wasm_bindgen(js_name = italian)]
    pub fn italian() -> Self {
        Self::wrap(Thirty360Convention::Italian)
    }

    /// Parse a variant name (the Rust serde label).
    ///
    /// # Arguments
    ///
    /// * `name` - Variant name: `"us_sia"`, `"isda"`, `"european"` or
    ///   `"italian"`.
    ///
    /// @returns The matching `Thirty360Convention`.
    /// @throws `TypeError` (kind `invalid_type`) if `name` is not a string;
    /// `FinstackError` (kind `validation`) if no variant matches.
    #[wasm_bindgen(js_name = fromName)]
    pub fn from_name(name: JsValue) -> Result<JsThirty360Convention, JsValue> {
        serde_parse(&js_string(&name, "name")?)
            .map(Self::wrap)
            .map_err(to_js_err)
    }

    /// Canonical snake_case name of the variant.
    ///
    /// @returns The name accepted by `fromName` and `days30360`.
    /// @throws If the label cannot be produced (not expected).
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string(&self) -> Result<String, JsValue> {
        serde_label(&self.inner).map_err(to_js_err)
    }
}

/// Day count between two dates under a 30/360 variant (Rust `days_30_360`).
///
/// # Arguments
///
/// * `start` - Inclusive accrual start as days since 1970-01-01.
/// * `end` - Exclusive accrual end as days since 1970-01-01; an earlier end
///   gives a negative count rather than an error.
/// * `convention` - Variant name: `"us_sia"`, `"isda"`, `"european"` or
///   `"italian"`.
///
/// @returns The 30/360 day count.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) for an unknown variant or a date outside the supported range.
#[wasm_bindgen(js_name = days30360)]
pub fn days_30_360(start: JsValue, end: JsValue, convention: JsValue) -> Result<i32, JsValue> {
    let convention: Thirty360Convention =
        serde_parse(&js_string(&convention, "convention")?).map_err(to_js_err)?;
    Ok(rust_dates::days_30_360(
        js_epoch_days(&start, "start")?,
        js_epoch_days(&end, "end")?,
        convention,
    ))
}

/// Day count under 30E/360 ISDA (Rust `days_30e_360_isda`).
///
/// # Arguments
///
/// * `start` - Inclusive accrual start as days since 1970-01-01.
/// * `end` - Exclusive accrual end as days since 1970-01-01; an earlier end
///   gives a negative count rather than an error.
/// * `end_is_termination_date` - Whether `end` is the instrument's final
///   maturity, which keeps a February month-end day unadjusted.
///
/// @returns The 30E/360 ISDA day count.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) for a date outside the supported range.
#[wasm_bindgen(js_name = days30e360Isda)]
pub fn days_30e_360_isda(
    start: JsValue,
    end: JsValue,
    end_is_termination_date: JsValue,
) -> Result<i32, JsValue> {
    Ok(rust_dates::days_30e_360_isda(
        js_epoch_days(&start, "start")?,
        js_epoch_days(&end, "end")?,
        js_bool(&end_is_termination_date, "endIsTerminationDate")?,
    ))
}

/// Unit of a tenor: days, weeks, months or years.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// core.TenorUnit.months().toString(); // "M"
/// core.TenorUnit.fromChar("y").toString(); // "Y"
/// ```
#[wasm_bindgen(js_name = TenorUnit)]
#[derive(Clone, Copy, Debug)]
pub struct JsTenorUnit {
    pub(crate) inner: TenorUnit,
}

impl JsTenorUnit {
    const fn wrap(inner: TenorUnit) -> Self {
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
        Self::wrap(TenorUnit::Days)
    }

    /// Calendar weeks (`W`).
    ///
    /// @returns The weeks unit.
    #[wasm_bindgen(js_name = weeks)]
    pub fn weeks() -> Self {
        Self::wrap(TenorUnit::Weeks)
    }

    /// Calendar months (`M`).
    ///
    /// @returns The months unit.
    #[wasm_bindgen(js_name = months)]
    pub fn months() -> Self {
        Self::wrap(TenorUnit::Months)
    }

    /// Calendar years (`Y`).
    ///
    /// @returns The years unit.
    #[wasm_bindgen(js_name = years)]
    pub fn years() -> Self {
        Self::wrap(TenorUnit::Years)
    }

    /// Parse a one-letter unit code (Rust `TenorUnit::from_char`).
    ///
    /// # Arguments
    ///
    /// * `ch` - Exactly one character: `D`, `W`, `M` or `Y`, in either case.
    ///
    /// @returns The matching `TenorUnit`.
    /// @throws `TypeError` (kind `invalid_type`) if `ch` is not a string;
    /// `FinstackError` (kind `validation`) if it is not exactly one of the
    /// four unit letters.
    #[wasm_bindgen(js_name = fromChar)]
    pub fn from_char(ch: JsValue) -> Result<JsTenorUnit, JsValue> {
        let text = js_string(&ch, "ch")?;
        let mut chars = text.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => TenorUnit::from_char(c).map(Self::wrap).map_err(to_js_err),
            _ => Err(to_js_err(format!(
                "expected a single unit character D, W, M or Y, got {text:?}"
            ))),
        }
    }

    /// One-letter unit code: `"D"`, `"W"`, `"M"` or `"Y"`.
    ///
    /// @returns The designator accepted by `fromChar` and used in tenor text.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.designator().to_string()
    }
}
