//! Tenor WASM handle.

use super::calendar::roll_rule;
use super::context::JsDayCountContext;
use super::day_count::JsDayCount;
use crate::api::core::imm::JsTenorUnit;
use crate::utils::input::{js_epoch_days, js_string, js_uint};
use crate::utils::to_js_err;
use finstack_quant_core::dates::Tenor as RustTenor;
use wasm_bindgen::prelude::*;

/// A financial tenor such as `3M`, `1Y`, or `2W`.
///
/// Tenors carry a numeric count and a unit (days, weeks, months, years).
/// Construct from a count and `TenorUnit` (`new Tenor(count, unit)`), parse
/// from strings (`Tenor.parse(s)`), or use the named-period factories
/// (`Tenor.daily`, `Tenor.weekly`, `Tenor.biweekly`, `Tenor.monthly`,
/// `Tenor.bimonthly`, `Tenor.quarterly`, `Tenor.semiAnnual`, `Tenor.annual`), or
/// `Tenor.fromPaymentsPerYear`.
///
/// @example
/// ```javascript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const t = new core.Tenor(3, core.TenorUnit.months());
/// t.toString();        // "3M"
/// t.toYearsApprox();   // 0.25
///
/// const annual = core.Tenor.annual();
/// annual.toString();   // "1Y"
/// ```
#[wasm_bindgen(js_name = Tenor)]
pub struct JsTenor {
    pub(crate) inner: RustTenor,
}

#[wasm_bindgen(js_class = Tenor)]
impl JsTenor {
    /// Construct a tenor from a unit count and unit (Rust `Tenor::new`).
    ///
    /// # Arguments
    ///
    /// * `count` - Positive integer count within the unit's 200-year ceiling.
    /// * `unit` - Calendar unit (`TenorUnit`).
    ///
    /// @returns The constructed `Tenor`.
    /// @throws `TypeError` (kind `invalid_type`) if `count` is not a
    /// non-negative integer; `FinstackError` (kind `validation`) if `count` is
    /// zero or exceeds the supported range for `unit`.
    /// @param count - Positive integer count within the unit's 200-year ceiling.
    /// @param unit - Calendar unit (`TenorUnit`).
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "number")] count: JsValue,
        unit: &JsTenorUnit,
    ) -> Result<JsTenor, JsValue> {
        let count: u32 = js_uint(&count, "count")?;
        RustTenor::new(count, unit.inner)
            .map(|inner| JsTenor { inner })
            .map_err(to_js_err)
    }

    /// Parse a tenor string (Rust `Tenor::parse`).
    ///
    /// # Arguments
    ///
    /// * `s` - Tenor text such as `"3M"`, `"1Y"`, `"2W"` or `"7D"`.
    ///
    /// @returns The parsed `Tenor`.
    /// @throws If `s` is not a string or cannot be parsed.
    /// @param s - Tenor text such as `"3M"`, `"1Y"`, `"2W"` or `"7D"`.
    #[wasm_bindgen(js_name = parse)]
    pub fn parse(
        #[wasm_bindgen(unchecked_param_type = "string")] s: JsValue,
    ) -> Result<JsTenor, JsValue> {
        let s: &str = &js_string(&s, "s")?;
        RustTenor::parse(s)
            .map(|inner| JsTenor { inner })
            .map_err(to_js_err)
    }

    /// Tenor for a coupon frequency (Rust `Tenor::from_payments_per_year`;
    /// `4` gives `3M`).
    ///
    /// # Arguments
    ///
    /// * `payments` - Coupon payments per year; must be positive and divide 12
    ///   (`12` monthly, `4` quarterly, `2` semi-annual, `1` annual).
    ///
    /// @returns The matching `Tenor`.
    /// @throws `TypeError` if `payments` is not a non-negative integer;
    /// `FinstackError` (kind `validation`) if no tenor matches.
    /// @param payments - Coupon payments per year; must be positive and divide 12 (`12` monthly, `4` quarterly, `2` semi-annual, `1` annual).
    #[wasm_bindgen(js_name = fromPaymentsPerYear)]
    pub fn from_payments_per_year(
        #[wasm_bindgen(unchecked_param_type = "number")] payments: JsValue,
    ) -> Result<JsTenor, JsValue> {
        let payments: u32 = js_uint(&payments, "payments")?;
        RustTenor::from_payments_per_year(payments)
            .map(|inner| JsTenor { inner })
            .map_err(to_js_err)
    }

    /// Two-week tenor (`2W`), the biweekly period.
    /// @returns A `Tenor` handle for this named period.
    #[wasm_bindgen(js_name = biweekly)]
    pub fn biweekly() -> JsTenor {
        JsTenor {
            inner: RustTenor::biweekly(),
        }
    }

    /// Two-month tenor (`2M`), the bimonthly period.
    /// @returns A `Tenor` handle for this named period.
    #[wasm_bindgen(js_name = bimonthly)]
    pub fn bimonthly() -> JsTenor {
        JsTenor {
            inner: RustTenor::bimonthly(),
        }
    }

    /// One-day tenor (`1D`), the daily period.
    /// @returns A `Tenor` handle for this named period.
    #[wasm_bindgen(js_name = daily)]
    pub fn daily() -> JsTenor {
        JsTenor {
            inner: RustTenor::daily(),
        }
    }

    /// One-week tenor (`1W`), the weekly period.
    /// @returns A `Tenor` handle for this named period.
    #[wasm_bindgen(js_name = weekly)]
    pub fn weekly() -> JsTenor {
        JsTenor {
            inner: RustTenor::weekly(),
        }
    }

    /// One-month tenor (`1M`), the monthly period.
    /// @returns A `Tenor` handle for this named period.
    #[wasm_bindgen(js_name = monthly)]
    pub fn monthly() -> JsTenor {
        JsTenor {
            inner: RustTenor::monthly(),
        }
    }

    /// 3-month (quarterly) tenor.
    /// @returns A `Tenor` handle for this named period.
    #[wasm_bindgen(js_name = quarterly)]
    pub fn quarterly() -> JsTenor {
        JsTenor {
            inner: RustTenor::quarterly(),
        }
    }

    /// 6-month (semi-annual) tenor.
    /// @returns A `Tenor` handle for this named period.
    #[wasm_bindgen(js_name = semiAnnual)]
    pub fn semi_annual() -> JsTenor {
        JsTenor {
            inner: RustTenor::semi_annual(),
        }
    }

    /// 12-month (annual) tenor.
    /// @returns A `Tenor` handle for this named period.
    #[wasm_bindgen(js_name = annual)]
    pub fn annual() -> JsTenor {
        JsTenor {
            inner: RustTenor::annual(),
        }
    }

    /// Unit count of this tenor, such as `3` for `"3M"`.
    #[wasm_bindgen(getter, js_name = count)]
    pub fn count(&self) -> u32 {
        self.inner.count()
    }

    /// Unit designator: `"D"`, `"W"`, `"M"` or `"Y"` (Rust `TenorUnit::designator`).
    #[wasm_bindgen(getter, js_name = unit)]
    pub fn unit(&self) -> String {
        self.inner.unit().designator().to_string()
    }

    /// Equivalent whole months, or `undefined` for day/week tenors.
    #[wasm_bindgen(getter, js_name = months)]
    pub fn months(&self) -> Option<u32> {
        self.inner.months()
    }

    /// Equivalent whole days, or `undefined` for month/year tenors.
    #[wasm_bindgen(getter, js_name = days)]
    pub fn days(&self) -> Option<u32> {
        self.inner.days()
    }

    /// Approximate length in years (simple estimate, no calendar).
    /// @returns Approximate tenor length in years, such as `0.25` for `"3M"`.
    #[wasm_bindgen(js_name = toYearsApprox)]
    pub fn to_years_approx(&self) -> f64 {
        self.inner.to_years_approx()
    }

    /// Nominal payments per year of a month or year tenor (`3M` gives `4`, `2Y` gives `0.5`).
    /// @returns Payments per year.
    /// @throws `FinstackError` (kind `validation`) for a day or week tenor, which
    /// has no nominal payment ratio.
    #[wasm_bindgen(js_name = paymentsPerYear)]
    pub fn payments_per_year(&self) -> Result<f64, JsValue> {
        self.inner.payments_per_year().map_err(to_js_err)
    }

    /// Approximate length in whole calendar days (no calendar): `nD` → `n`,
    /// `nW` → `7n`, `nM` → `floor((365n + 6) / 12)`, `nY` → `365n`.
    ///
    /// @returns Whole days as a number (exact: every admitted tenor stays
    /// below 2^53).
    #[wasm_bindgen(js_name = toDaysApprox)]
    pub fn to_days_approx(&self) -> f64 {
        // Admitted tenors give at most 365 * (i32::MAX / 12) days, far below
        // 2^53, so the conversion is exact.
        self.inner.to_days_approx() as f64
    }

    /// Add this tenor to a date (Rust `Tenor::add_to_date`).
    ///
    /// Month and year tenors clamp to the last valid day of the target month.
    ///
    /// # Arguments
    ///
    /// * `epoch_days` - Anchor date as days since 1970-01-01.
    /// * `calendar_code` - Registered holiday-calendar identifier used to roll
    ///   the result; omitted skips adjustment.
    /// * `convention` - Business-day convention name applied whenever
    ///   `calendar_code` is provided; required (there is no default convention).
    ///
    /// @returns The (optionally adjusted) end date as epoch days.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` if the date
    /// is out of range, `calendarCode` is supplied without `convention`, the
    /// convention is unknown, the calendar is unknown (kind `not_found`), or no
    /// business day is found.
    /// @param epochDays - Anchor date as days since 1970-01-01.
    /// @param calendarCode - Registered holiday-calendar identifier used to roll the result; omitted skips adjustment.
    /// @param convention - Business-day convention name applied with `calendarCode`; required (omitting it when `calendarCode` is given throws), there is no default convention.
    #[wasm_bindgen(js_name = addToDate)]
    pub fn add_to_date(
        &self,
        #[wasm_bindgen(js_name = "epochDays", unchecked_param_type = "number")] epoch_days: JsValue,
        #[wasm_bindgen(
            js_name = "calendarCode",
            unchecked_optional_param_type = "string | null"
        )]
        calendar_code: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "string | null")] convention: Option<
            JsValue,
        >,
    ) -> Result<i32, JsValue> {
        let date = js_epoch_days(&epoch_days, "epochDays")?;
        let (calendar, convention) = roll_rule(calendar_code, convention)?;
        let end = self
            .inner
            .add_to_date(date, calendar, convention)
            .map_err(to_js_err)?;
        Ok(finstack_quant_core::dates::days_since_epoch(end))
    }

    /// Exact year fraction of this tenor from a date under a day count
    /// (Rust `Tenor::to_years_with_context`).
    ///
    /// # Arguments
    ///
    /// * `as_of_epoch_days` - Start date as days since 1970-01-01.
    /// * `day_count` - Convention used to measure the span.
    /// * `calendar_code` - Registered holiday-calendar identifier used to roll
    ///   the end date; omitted skips adjustment.
    /// * `convention` - Business-day convention name applied with the
    ///   calendar; required, there is no default convention.
    /// * `ctx` - Caller's `DayCountContext` for conventions that need one; the
    ///   published facade makes it optional (omitted is the empty context). No
    ///   frequency or coupon period is invented from the tenor.
    ///
    /// @returns Year fraction between the start and the rolled end date.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` if the date
    /// is out of range, the convention or calendar is unknown, the day count
    /// needs context `ctx` lacks (e.g. Bus/252 without a calendar), or the
    /// rolled end precedes the start.
    /// @param asOfEpochDays - Start date as days since 1970-01-01.
    /// @param dayCount - Convention used to measure the span.
    /// @param calendarCode - Registered holiday-calendar identifier used to roll the end date; omitted skips adjustment.
    /// @param convention - Business-day convention name; required (omitting it throws), there is no default convention.
    /// @param ctx - Caller's `DayCountContext` for conventions that need one; omitted is the empty context.
    #[wasm_bindgen(js_name = toYearsWithContext)]
    pub fn to_years_with_context(
        &self,
        #[wasm_bindgen(js_name = "asOfEpochDays", unchecked_param_type = "number")]
        as_of_epoch_days: JsValue,
        #[wasm_bindgen(js_name = "dayCount")] day_count: &JsDayCount,
        #[wasm_bindgen(
            js_name = "calendarCode",
            unchecked_optional_param_type = "string | null"
        )]
        calendar_code: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "string | null")] convention: Option<
            JsValue,
        >,
        #[wasm_bindgen(unchecked_optional_param_type = "DayCountContext")] ctx: &JsDayCountContext,
    ) -> Result<f64, JsValue> {
        let as_of = js_epoch_days(&as_of_epoch_days, "asOfEpochDays")?;
        let (calendar, convention) = roll_rule(calendar_code, convention)?;
        let context = ctx.inner.to_ctx().map_err(to_js_err)?;
        self.inner
            .to_years_with_context(as_of, calendar, convention, day_count.inner, context)
            .map_err(to_js_err)
    }

    /// Tenor string representation.
    /// @returns Human-readable string form of this value.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}
