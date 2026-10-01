//! WASM bindings for date utilities from [`finstack_quant_core::dates`].

use crate::utils::input::{
    from_js_json, invalid_type, js_epoch_days, js_f64, js_int, js_opt_bool, js_opt_string,
    js_opt_uint, js_string, js_uint,
};
use crate::utils::to_js_err;
use finstack_quant_core::dates::{
    adjust as core_adjust, available_calendars as core_available_calendars,
    fx::resolve_calendar as rust_resolve_calendar, BusinessDayConvention, Date,
    DayCount as RustDayCount, DayCountContext as RustDayCountContext, DayCountContextState,
    HolidayCalendar, Tenor as RustTenor,
};
use wasm_bindgen::prelude::*;

/// Optional context for day-count conventions that need market metadata.
#[wasm_bindgen(js_name = DayCountContext)]
#[derive(Clone, Default)]
pub struct JsDayCountContext {
    /// Serializable state (`DayCountContextState::default()` when empty); the
    /// live calendar is resolved on each use.
    pub(crate) inner: DayCountContextState,
}

impl JsDayCountContext {
    /// Resolve to a runtime context.
    ///
    /// Errors when the calendar id is set but unknown to the global calendar
    /// lookup, instead of silently dropping the calendar. Routes through the
    /// core registry error so unknown codes surface "Did you mean …?"
    /// suggestions and a structured `not_found` kind.
    fn to_rust_ctx(&self) -> Result<RustDayCountContext<'static>, JsValue> {
        self.inner.to_ctx().map_err(to_js_err)
    }
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
    ///   `tenor.toString()` for a `Tenor`), required by Act/Act ICMA and used
    ///   by Act/365L.
    /// * `bus_basis` - Business-day denominator for Bus/252 (an integer in
    ///   `0..=65535`, normally `252`).
    /// * `coupon_period` - Reference coupon period `[startEpochDays,
    ///   endEpochDays]` (days since 1970-01-01) for Act/Act ICMA; the start must
    ///   precede the end.
    /// * `end_is_termination_date` - Whether the accrual end is the
    ///   instrument's termination date (30E/360 ISDA February-end handling);
    ///   omitted means `false`.
    ///
    /// @returns A new `DayCountContext`.
    /// @throws `TypeError` (kind `invalid_type`) for a mistyped argument or a
    /// `couponPeriod` that is not a two-element array of epoch days;
    /// `FinstackError` (kind `validation`) if `frequency` is not a tenor or
    /// the coupon period is inverted or out of range.
    ///
    /// @example
    /// ```javascript
    /// const ctx = new core.DayCountContext("nyse", "3M", 252);
    /// ctx.frequency?.toString();  // "3M"
    /// core.DayCountContext.fromJson(ctx.toJson()).busBasis;  // 252
    /// ```
    #[wasm_bindgen(constructor)]
    pub fn new(
        calendar_id: Option<JsValue>,
        frequency: Option<JsValue>,
        bus_basis: Option<JsValue>,
        coupon_period: Option<JsValue>,
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
            js_opt_bool(end_is_termination_date.as_ref(), "endIsTerminationDate")?
                .unwrap_or_default();
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
        self.inner.calendar_id.clone()
    }

    /// Coupon frequency, or `undefined`.
    #[wasm_bindgen(getter, js_name = frequency)]
    pub fn frequency(&self) -> Option<JsTenor> {
        self.inner.frequency.map(|inner| JsTenor { inner })
    }

    /// Custom business-day denominator, or `undefined`.
    #[wasm_bindgen(getter, js_name = busBasis)]
    pub fn bus_basis(&self) -> Option<u16> {
        self.inner.bus_basis
    }

    /// Reference coupon period as `[startEpochDays, endEpochDays]`, or `undefined`.
    #[wasm_bindgen(getter, js_name = couponPeriod)]
    pub fn coupon_period(&self) -> Option<Box<[i32]>> {
        self.inner.coupon_period.map(|(start, end)| {
            Box::new([
                finstack_quant_core::dates::days_since_epoch(start),
                finstack_quant_core::dates::days_since_epoch(end),
            ]) as Box<[i32]>
        })
    }

    /// Whether the accrual end is the instrument termination date.
    #[wasm_bindgen(getter, js_name = endIsTerminationDate)]
    pub fn end_is_termination_date(&self) -> bool {
        self.inner.end_is_termination_date
    }

    /// Serialize to the canonical JSON wire form shared with Python `DayCountContext.to_json`.
    ///
    /// @returns Compact JSON text (dates in the coupon period are ISO-8601).
    /// @throws If serialization fails (not expected for a valid context).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Deserialize from the canonical JSON wire form produced by `toJson`.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical DayCountContext JSON text or plain object; unknown
    ///   fields are rejected. As in Rust and Python, the coupon period is
    ///   validated when the context is used (`DayCountContextState::to_ctx`).
    ///
    /// @returns The parsed `DayCountContext`.
    /// @throws If `json` is malformed or has unknown or mistyped fields.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsDayCountContext, JsValue> {
        from_js_json::<DayCountContextState>(&json, "json").map(|inner| JsDayCountContext { inner })
    }
}

/// Read a `[startEpochDays, endEpochDays]` pair.
fn epoch_day_pair(value: &JsValue, label: &str) -> Result<(Date, Date), JsValue> {
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

/// Day-count convention for computing year fractions and day counts.
///
/// Dates are represented as **epoch days** (`i32`, days since 1970-01-01).
/// Use `createDate` to convert from a `(year, month, day)` triple.
///
/// Available conventions (canonical names for `DayCount.fromName`) and their factories:
/// - `one_one` → `DayCount.oneOne`
/// - `act_360` → `DayCount.act360`
/// - `act_365f` → `DayCount.act365f`
/// - `act_365l` → `DayCount.act365l`
/// - `nl_365` (No-Leap/365) → `DayCount.nl365`
/// - `30_360` → `DayCount.thirty360`
/// - `30e_360` → `DayCount.thirtyE360`
/// - `30e_360_isda` → `DayCount.thirtyE360Isda`
/// - `act_act` (ISDA) → `DayCount.actAct`
/// - `act_act_isma` (ICMA) → `DayCount.actActIsma`
/// - `act_act_afb` (AFB / Actual/Actual Euro) → `DayCount.actActAfb`
/// - `30_360_it` (Italian) → `DayCount.thirty360It`
/// - `bus_252` → `DayCount.bus252`
///
/// Term-sheet spellings such as `"ACT/360"` or `"30/360 ISDA"` go through the
/// lenient `DayCount.parse`.
///
/// @example
/// ```javascript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const day_count = core.DayCount.act365f();
/// const start = core.createDate(2025, 1, 15);
/// const end   = core.createDate(2025, 7, 15);
/// const yf    = day_count.yearFraction(start, end);
/// // yf ≈ 0.4959 (181 / 365)
/// ```
#[wasm_bindgen(js_name = DayCount)]
pub struct JsDayCount {
    pub(crate) inner: RustDayCount,
}

#[wasm_bindgen(js_class = DayCount)]
impl JsDayCount {
    /// Look up a convention by its canonical name (Rust `DayCount::from_str`,
    /// the twin of Python `DayCount.from_name`).
    ///
    /// # Arguments
    ///
    /// * `name` - Canonical snake_case convention name, for example
    ///   `"act_360"`, `"30_360"` or `"act_act"` (see the class list).
    ///
    /// @returns The matching `DayCount`.
    /// @throws `TypeError` if `name` is not a string; `FinstackError` (kind
    /// `validation`) if it is not a canonical convention name. Use
    /// `DayCount.parse` for term-sheet spellings.
    #[wasm_bindgen(js_name = fromName)]
    pub fn from_name(name: JsValue) -> Result<JsDayCount, JsValue> {
        let name: &str = &js_string(&name, "name")?;
        name.parse::<RustDayCount>()
            .map(|inner| JsDayCount { inner })
            .map_err(to_js_err)
    }

    /// Parse a convention leniently (Rust `DayCount::parse`): case, spaces,
    /// `/` and `-` are normalised, so term-sheet spellings such as
    /// `"ACT/360"`, `"Act/Act ICMA"` or `"30E/360 ISDA"` are accepted.
    ///
    /// # Arguments
    ///
    /// * `s` - Convention text in canonical or term-sheet spelling.
    ///
    /// @returns The matching `DayCount`.
    /// @throws `TypeError` if `s` is not a string; `FinstackError` (kind
    /// `validation`) if no convention matches.
    ///
    /// @example
    /// ```javascript
    /// core.DayCount.parse("Act/Act ICMA").toString();  // "act_act_isma"
    /// ```
    #[wasm_bindgen(js_name = parse)]
    pub fn parse(s: JsValue) -> Result<JsDayCount, JsValue> {
        let s: &str = &js_string(&s, "s")?;
        RustDayCount::parse(s)
            .map(|inner| JsDayCount { inner })
            .map_err(to_js_err)
    }

    /// No-Leap/365: actual days excluding February 29, over 365.
    #[wasm_bindgen(js_name = nl365)]
    pub fn nl365() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::Nl365,
        }
    }

    /// One unit per nonempty contractual accrual period; empty periods return zero.
    /// @returns The 1/1 convention used for annual inflation accrual periods.
    /// @throws This constructor does not throw.
    #[wasm_bindgen(js_name = oneOne)]
    pub fn one_one() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::OneOne,
        }
    }

    /// Actual/360.
    #[wasm_bindgen(js_name = act360)]
    pub fn act360() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::Act360,
        }
    }

    /// Actual/365 Fixed.
    #[wasm_bindgen(js_name = act365f)]
    pub fn act365f() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::Act365F,
        }
    }

    /// Actual/365L (ICMA Rule 251). Annual periods (or periods without
    /// frequency context) use denominator 366 exactly when February 29 falls
    /// in `(start, end]`; non-annual periods use 366 exactly when the end
    /// date's year is a leap year. Otherwise the denominator is 365. This is
    /// not ACT/ACT AFB.
    #[wasm_bindgen(js_name = act365l)]
    pub fn act365l() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::Act365L,
        }
    }

    /// 30/360 US (Bond Basis).
    #[wasm_bindgen(js_name = thirty360)]
    pub fn thirty360() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::Thirty360,
        }
    }

    /// 30E/360 (Eurobond Basis).
    #[wasm_bindgen(js_name = thirtyE360)]
    pub fn thirty_e360() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::ThirtyE360,
        }
    }

    /// 30E/360 ISDA.
    #[wasm_bindgen(js_name = thirtyE360Isda)]
    pub fn thirty_e360_isda() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::ThirtyE360Isda,
        }
    }

    /// Actual/Actual (ISDA).
    #[wasm_bindgen(js_name = actAct)]
    pub fn act_act() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::ActAct,
        }
    }

    /// Actual/Actual (ICMA/ISMA).
    #[wasm_bindgen(js_name = actActIsma)]
    pub fn act_act_isma() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::ActActIsma,
        }
    }

    /// Actual/Actual AFB (Actual/Actual Euro).
    ///
    /// Walks whole years backwards from the end date (QuantLib
    /// `ActualActual::AFB`). A year-step landing on 28 February of a leap
    /// year is bumped to 29 February. The residual uses denominator 366 if
    /// 29 February lies in `[start, residual_end)`, else 365.
    #[wasm_bindgen(js_name = actActAfb)]
    pub fn act_act_afb() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::ActActAfb,
        }
    }

    /// 30/360 Italian.
    ///
    /// Day 31 becomes 30, and any February day after the 27th becomes 30
    /// (QuantLib `Thirty360::Italian`). Distinct from US SIA and 30E/360.
    #[wasm_bindgen(js_name = thirty360It)]
    pub fn thirty360_it() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::Thirty360It,
        }
    }

    /// Business/252.
    #[wasm_bindgen(js_name = bus252)]
    pub fn bus252() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::Bus252,
        }
    }

    /// Compute the year fraction between two dates given as epoch days
    /// (Rust `DayCount::year_fraction(start, end, ctx)`).
    ///
    /// The published facade makes `ctx` optional: an omitted context is the
    /// empty Rust default (`new DayCountContext()`). Act/Act ISMA needs a
    /// context frequency (or coupon period) and Bus/252 a context calendar;
    /// both throw without them.
    ///
    /// @param startEpochDays - Start date as days since 1970-01-01.
    /// @param endEpochDays - End date as days since 1970-01-01; must not be
    /// before the start.
    /// @param ctx - DayCountContext supplying calendar, frequency, coupon-period
    /// and termination metadata; omitted means the empty default context.
    /// @returns Non-negative year fraction in years under the convention and context.
    /// @throws `TypeError` (kind `invalid_type`) if a date is not an integer
    /// epoch-day number; `FinstackError` (kind `validation`) if a date is out
    /// of range, the start is after the end, or the convention's required
    /// context is missing or invalid; kind `not_found` if the context names an
    /// unknown calendar.
    ///
    /// @example
    /// ```javascript
    /// const day_count = core.DayCount.act360();
    /// const start = core.createDate(2025, 1, 15);
    /// const end   = core.createDate(2025, 4, 15);
    /// day_count.yearFraction(start, end); // 90 / 360 = 0.25
    /// ```
    #[wasm_bindgen(js_name = yearFraction)]
    pub fn year_fraction(
        &self,
        start_epoch_days: JsValue,
        end_epoch_days: JsValue,
        ctx: &JsDayCountContext,
    ) -> Result<f64, JsValue> {
        let start = js_epoch_days(&start_epoch_days, "startEpochDays")?;
        let end = js_epoch_days(&end_epoch_days, "endEpochDays")?;
        self.inner
            .year_fraction(start, end, ctx.to_rust_ctx()?)
            .map_err(to_js_err)
    }

    /// Compute a signed year fraction, preserving the start/end orientation
    /// (Rust `DayCount::signed_year_fraction(start, end, ctx)`).
    ///
    /// The published facade makes `ctx` optional, as for `yearFraction`.
    ///
    /// @param start_epoch_days - Start date as days since 1970-01-01.
    /// @param end_epoch_days - End date as days since 1970-01-01; may precede the start.
    /// @param ctx - DayCountContext supplying calendar, frequency, coupon-period
    /// and termination metadata; omitted means the empty default context.
    /// @returns Signed year fraction in years; negative when `end` is before `start`.
    ///
    /// # Errors
    ///
    /// Throws `TypeError` (kind `invalid_type`) if a date is not an integer
    /// epoch-day number; `FinstackError` (kind `validation`) if a date is out of
    /// range or the convention's required context is missing or invalid; kind
    /// `not_found` if the context names an unknown calendar.
    #[wasm_bindgen(js_name = signedYearFraction)]
    pub fn signed_year_fraction(
        &self,
        start_epoch_days: JsValue,
        end_epoch_days: JsValue,
        ctx: &JsDayCountContext,
    ) -> Result<f64, JsValue> {
        let start = js_epoch_days(&start_epoch_days, "startEpochDays")?;
        let end = js_epoch_days(&end_epoch_days, "endEpochDays")?;
        self.inner
            .signed_year_fraction(start, end, ctx.to_rust_ctx()?)
            .map_err(to_js_err)
    }

    /// Count the calendar days between two dates (epoch days), independent of
    /// the convention (Rust associated fn `DayCount::calendar_days`).
    /// @param start_epoch_days - Start date as days since 1970-01-01.
    /// @param end_epoch_days - End date as days since 1970-01-01.
    /// @returns Signed calendar-day count from start to end.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if either epoch-day value is outside the
    /// representable date range.
    #[wasm_bindgen(js_name = calendarDays)]
    pub fn calendar_days(
        start_epoch_days: JsValue,
        end_epoch_days: JsValue,
    ) -> Result<i64, JsValue> {
        let start = js_epoch_days(&start_epoch_days, "startEpochDays")?;
        let end = js_epoch_days(&end_epoch_days, "endEpochDays")?;
        Ok(RustDayCount::calendar_days(start, end))
    }

    /// Convention name.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}

/// A financial tenor such as `3M`, `1Y`, or `2W`.
///
/// Tenors carry a numeric count and a unit (days, weeks, months, years).
/// Parse from strings (`new Tenor(s)` or `Tenor.parse(s)`) or use the
/// named-period factories (`Tenor.daily`, `Tenor.weekly`, `Tenor.biweekly`,
/// `Tenor.monthly`, `Tenor.bimonthly`, `Tenor.quarterly`, `Tenor.semiAnnual`,
/// `Tenor.annual`), `Tenor.fromPaymentsPerYear` or `Tenor.fromYears`.
///
/// @example
/// ```javascript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const t = new core.Tenor("3M");
/// t.toString();        // "3M"
/// t.toYears();   // 0.25
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
    /// Parse a tenor string.
    ///
    /// @param s - Tenor string. Accepted forms include `"3M"`, `"1Y"`,
    /// `"2W"`, `"7D"`, `"6M"`, `"10Y"`. Whitespace is permitted.
    /// @returns The parsed `Tenor`.
    /// @throws If `s` cannot be parsed (unknown unit, missing count).
    #[wasm_bindgen(constructor)]
    pub fn new(s: JsValue) -> Result<JsTenor, JsValue> {
        let s: &str = &js_string(&s, "s")?;
        RustTenor::parse(s)
            .map(|inner| JsTenor { inner })
            .map_err(to_js_err)
    }

    /// Parse a tenor string (Rust `Tenor::parse`; same as `new Tenor(s)`).
    ///
    /// # Arguments
    ///
    /// * `s` - Tenor text such as `"3M"`, `"1Y"`, `"2W"` or `"7D"`.
    ///
    /// @returns The parsed `Tenor`.
    /// @throws If `s` is not a string or cannot be parsed.
    #[wasm_bindgen(js_name = parse)]
    pub fn parse(s: JsValue) -> Result<JsTenor, JsValue> {
        Self::new(s)
    }

    /// Tenor for a year fraction under a day count (Rust `Tenor::from_years`):
    /// a whole number of months when the fraction is one, otherwise days.
    ///
    /// # Arguments
    ///
    /// * `years` - Positive, finite length in years.
    /// * `day_count` - Convention used to interpret `years`.
    ///
    /// @returns The matching `Tenor`.
    /// @throws If `years` is not finite and positive or the tenor is out of range.
    #[wasm_bindgen(js_name = fromYears)]
    pub fn from_years(years: JsValue, day_count: &JsDayCount) -> Result<JsTenor, JsValue> {
        let years = js_f64(&years, "years")?;
        RustTenor::from_years(years, day_count.inner)
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
    #[wasm_bindgen(js_name = fromPaymentsPerYear)]
    pub fn from_payments_per_year(payments: JsValue) -> Result<JsTenor, JsValue> {
        let payments: u32 = js_uint(&payments, "payments")?;
        RustTenor::from_payments_per_year(payments)
            .map(|inner| JsTenor { inner })
            .map_err(to_js_err)
    }

    /// 2-week tenor.
    #[wasm_bindgen(js_name = biweekly)]
    pub fn biweekly() -> JsTenor {
        JsTenor {
            inner: RustTenor::biweekly(),
        }
    }

    /// 2-month tenor.
    #[wasm_bindgen(js_name = bimonthly)]
    pub fn bimonthly() -> JsTenor {
        JsTenor {
            inner: RustTenor::bimonthly(),
        }
    }

    /// 1-day tenor.
    #[wasm_bindgen(js_name = daily)]
    pub fn daily() -> JsTenor {
        JsTenor {
            inner: RustTenor::daily(),
        }
    }

    /// 1-week tenor.
    #[wasm_bindgen(js_name = weekly)]
    pub fn weekly() -> JsTenor {
        JsTenor {
            inner: RustTenor::weekly(),
        }
    }

    /// 1-month tenor.
    #[wasm_bindgen(js_name = monthly)]
    pub fn monthly() -> JsTenor {
        JsTenor {
            inner: RustTenor::monthly(),
        }
    }

    /// 3-month (quarterly) tenor.
    #[wasm_bindgen(js_name = quarterly)]
    pub fn quarterly() -> JsTenor {
        JsTenor {
            inner: RustTenor::quarterly(),
        }
    }

    /// 6-month (semi-annual) tenor.
    #[wasm_bindgen(js_name = semiAnnual)]
    pub fn semi_annual() -> JsTenor {
        JsTenor {
            inner: RustTenor::semi_annual(),
        }
    }

    /// 12-month (annual) tenor.
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
    #[wasm_bindgen(js_name = toYears)]
    pub fn to_years(&self) -> f64 {
        self.inner.to_years()
    }

    /// Coupon payments per year implied by this tenor (`3M` gives `4`, `2Y` gives `0.5`).
    #[wasm_bindgen(js_name = paymentsPerYear)]
    pub fn payments_per_year(&self) -> f64 {
        self.inner.payments_per_year()
    }

    /// Approximate length in calendar days (no calendar).
    ///
    /// @returns Whole days as a number.
    /// @throws Never for a valid tenor (the Rust tenor bounds keep it within range).
    #[wasm_bindgen(js_name = toDaysApprox)]
    pub fn to_days_approx(&self) -> Result<i32, JsValue> {
        i32::try_from(self.inner.to_days_approx()).map_err(|e| to_js_err(e.to_string()))
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
    /// * `convention` - Business-day convention name applied with the
    ///   calendar; omitted uses the Rust default (`"modified_following"`).
    ///
    /// @returns The (optionally adjusted) end date as epoch days.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` if the date
    /// is out of range, the convention is unknown, the calendar is unknown
    /// (kind `not_found`), or no business day is found.
    #[wasm_bindgen(js_name = addToDate)]
    pub fn add_to_date(
        &self,
        epoch_days: JsValue,
        calendar_code: Option<JsValue>,
        convention: Option<JsValue>,
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
    ///   calendar; omitted uses the Rust default (`"modified_following"`).
    ///
    /// @returns Year fraction between the start and the rolled end date.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` if the date
    /// is out of range, the convention or calendar is unknown, or the day count
    /// needs context it cannot get (e.g. Bus/252 without a calendar).
    #[wasm_bindgen(js_name = toYearsWithContext)]
    pub fn to_years_with_context(
        &self,
        as_of_epoch_days: JsValue,
        day_count: &JsDayCount,
        calendar_code: Option<JsValue>,
        convention: Option<JsValue>,
    ) -> Result<f64, JsValue> {
        let as_of = js_epoch_days(&as_of_epoch_days, "asOfEpochDays")?;
        let (calendar, convention) = roll_rule(calendar_code, convention)?;
        self.inner
            .to_years_with_context(as_of, calendar, convention, day_count.inner)
            .map_err(to_js_err)
    }

    /// Tenor string representation.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}

/// Resolve the optional calendar and business-day convention of a tenor roll.
///
/// An omitted convention is `BusinessDayConvention::default()`.
fn roll_rule(
    calendar_code: Option<JsValue>,
    convention: Option<JsValue>,
) -> Result<(Option<&'static dyn HolidayCalendar>, BusinessDayConvention), JsValue> {
    let calendar = js_opt_string(calendar_code.as_ref(), "calendarCode")?
        .map(|code| rust_resolve_calendar(Some(&code)))
        .transpose()
        .map_err(to_js_err)?;
    let convention = match js_opt_string(convention.as_ref(), "convention")? {
        Some(name) => name.parse().map_err(|e: String| to_js_err(e))?,
        None => BusinessDayConvention::default(),
    };
    Ok((calendar, convention))
}

/// Create a date and return it as epoch days (days since 1970-01-01).
/// @param year - Four-digit calendar year component of the supplied date.
/// @param month - Calendar month number from 1 through 12.
/// @param day - Calendar day number within the selected month.
///
/// # Errors
///
/// Throws a JavaScript exception if `month` is outside `1..=12` or the supplied
/// year, month, and day do not form a representable calendar date.
#[wasm_bindgen(js_name = createDate)]
pub fn create_date(year: JsValue, month: JsValue, day: JsValue) -> Result<i32, JsValue> {
    let year: i32 = js_int(&year, "year")?;
    let month: u8 = js_uint(&month, "month")?;
    let day: u8 = js_uint(&day, "day")?;
    let m = time::Month::try_from(month).map_err(to_js_err)?;
    let date = finstack_quant_core::dates::create_date(year, m, day).map_err(to_js_err)?;
    Ok(finstack_quant_core::dates::days_since_epoch(date))
}

/// Convert epoch days back to `[year, month, day]` as a JS array-compatible triple.
/// @param days - Number of days since 1970-01-01 to decompose into year, month, and day.
///
/// # Errors
///
/// Throws a JavaScript exception if `days` is outside the representable date
/// range.
#[wasm_bindgen(js_name = dateFromEpochDays)]
pub fn date_from_epoch_days(days: JsValue) -> Result<Vec<i32>, JsValue> {
    let date = js_epoch_days(&days, "days")?;
    Ok(vec![date.year(), date.month() as i32, date.day() as i32])
}

/// Adjust a date (epoch days) according to a business-day convention and calendar.
///
/// Returns the adjusted date as epoch days.
/// @param epoch_days - Unadjusted date as days since 1970-01-01.
/// @param convention - Business-day adjustment convention string accepted by the date API.
/// @param calendar_code - Registered holiday-calendar identifier used to find business days.
///
/// # Errors
///
/// Throws a JavaScript exception if `epochDays` is outside the representable date
/// range, `convention` is unrecognized, `calendarCode` is unknown, or adjustment
/// cannot produce a representable business date.
#[wasm_bindgen(js_name = adjust)]
pub fn adjust(
    epoch_days: JsValue,
    convention: JsValue,
    calendar_code: JsValue,
) -> Result<i32, JsValue> {
    let convention: &str = &js_string(&convention, "convention")?;
    let calendar_code: &str = &js_string(&calendar_code, "calendarCode")?;
    let date = js_epoch_days(&epoch_days, "epochDays")?;
    let business_day_convention: BusinessDayConvention =
        convention.parse().map_err(|e: String| to_js_err(e))?;
    let cal = rust_resolve_calendar(Some(calendar_code)).map_err(to_js_err)?;
    let adjusted = core_adjust(date, business_day_convention, cal).map_err(to_js_err)?;
    Ok(finstack_quant_core::dates::days_since_epoch(adjusted))
}

/// Return the list of available calendar codes.
#[wasm_bindgen(js_name = availableCalendars)]
pub fn available_calendars() -> Vec<String> {
    core_available_calendars()
        .iter()
        .map(|s| s.to_string())
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;

    // -- JsDayCount -----------------------------------------------------------

    #[test]
    fn daycount_constructors() {
        let day_count = JsDayCount::act360();
        assert_eq!(day_count.to_string(), "act_360");
        let day_count = JsDayCount::act365f();
        assert_eq!(day_count.to_string(), "act_365f");
        let day_count = JsDayCount::thirty360();
        assert_eq!(day_count.to_string(), "30_360");
        let day_count = JsDayCount::thirty_e360();
        assert_eq!(day_count.to_string(), "30e_360");
        let day_count = JsDayCount::thirty_e360_isda();
        assert_eq!(day_count.to_string(), "30e_360_isda");
        let day_count = JsDayCount::act_act();
        assert_eq!(day_count.to_string(), "act_act");
        let day_count = JsDayCount::act_act_isma();
        assert_eq!(day_count.to_string(), "act_act_isma");
        let day_count = JsDayCount::act_act_afb();
        assert_eq!(day_count.to_string(), "act_act_afb");
        let day_count = JsDayCount::thirty360_it();
        assert_eq!(day_count.to_string(), "30_360_it");
        let day_count = JsDayCount::bus252();
        assert_eq!(day_count.to_string(), "bus_252");
    }

    // -- JsTenor --------------------------------------------------------------

    #[test]
    fn tenor_factories() {
        assert_eq!(JsTenor::daily().count(), 1);
        assert_eq!(JsTenor::weekly().count(), 1);
        assert_eq!(JsTenor::monthly().count(), 1);
        assert_eq!(JsTenor::quarterly().count(), 3);
        assert_eq!(JsTenor::semi_annual().count(), 6);
        assert_eq!(JsTenor::annual().count(), 1);
    }

    #[test]
    fn tenor_to_string() {
        let t = JsTenor::quarterly();
        let s = t.to_string();
        assert!(s.contains('M') || s.contains('Q'), "got: {s}");
    }

    // -- Free functions -----------------------------------------------------

    #[test]
    fn available_calendars_not_empty() {
        let cals = available_calendars();
        assert!(!cals.is_empty());
    }

    #[test]
    fn tenor_weekly_to_string() {
        let t = JsTenor::weekly();
        let s = t.to_string();
        assert!(!s.is_empty());
    }

    #[test]
    fn tenor_semi_annual_years() {
        let t = JsTenor::semi_annual();
        assert!((t.to_years() - 0.5).abs() < 0.01);
    }

    #[test]
    fn tenor_annual_years() {
        let t = JsTenor::annual();
        assert!((t.to_years() - 1.0).abs() < 0.01);
    }

    #[test]
    fn tenor_daily_years() {
        let t = JsTenor::daily();
        assert!(t.to_years() < 0.01);
    }

    // -- Boundary tests ------------------------------------------------
    // Error paths through wasm-bindgen create JsValue, which panics on
    // native targets.  Test the underlying Rust types instead.

    #[test]
    fn create_date_invalid_month() {
        assert!(time::Month::try_from(13_u8).is_err());
        assert!(time::Month::try_from(0_u8).is_err());
    }

    #[test]
    fn create_date_invalid_day() {
        assert!(finstack_quant_core::dates::create_date(2024, time::Month::February, 30).is_err());
    }

    #[test]
    fn date_from_epoch_days_extreme() {
        assert!(finstack_quant_core::dates::date_from_epoch_days(i32::MAX).is_none());
        assert!(finstack_quant_core::dates::date_from_epoch_days(i32::MIN).is_none());
    }

    #[test]
    fn daycount_nl365_factory_and_lenient_parse() {
        assert_eq!(JsDayCount::nl365().to_string(), "nl_365");
        assert_eq!(
            RustDayCount::parse("Act/Act ICMA").expect("lenient"),
            RustDayCount::ActActIsma
        );
        assert!("ACT/360".parse::<RustDayCount>().is_err());
    }

    #[test]
    fn tenor_members_pass_through_rust() {
        let t = JsTenor {
            inner: RustTenor::parse("3M").expect("3M"),
        };
        assert_eq!(t.unit(), "M");
        assert_eq!(t.months(), Some(3));
        assert_eq!(t.days(), None);
        assert!((t.payments_per_year() - 4.0).abs() < 1e-12);
        assert_eq!(
            t.to_days_approx().expect("days"),
            i32::try_from(t.inner.to_days_approx()).expect("fits")
        );
        assert_eq!(JsTenor::biweekly().days(), Some(14));
        assert_eq!(JsTenor::bimonthly().months(), Some(2));
    }

    #[test]
    fn day_count_context_getters_read_the_rust_state() {
        let start =
            finstack_quant_core::dates::create_date(2025, time::Month::January, 15).expect("start");
        let end =
            finstack_quant_core::dates::create_date(2025, time::Month::July, 15).expect("end");
        let ctx = JsDayCountContext {
            inner: DayCountContextState::try_new(
                Some("nyse".into()),
                Some(RustTenor::semi_annual()),
                Some(252),
                Some((start, end)),
                true,
            )
            .expect("state"),
        };
        assert_eq!(ctx.calendar_id().as_deref(), Some("nyse"));
        assert_eq!(
            ctx.frequency().map(|t| t.to_string()).as_deref(),
            Some("6M")
        );
        assert_eq!(ctx.bus_basis(), Some(252));
        assert_eq!(
            ctx.coupon_period().as_deref(),
            Some(
                &[
                    finstack_quant_core::dates::days_since_epoch(start),
                    finstack_quant_core::dates::days_since_epoch(end)
                ][..]
            )
        );
        assert!(ctx.end_is_termination_date());
        let back: DayCountContextState =
            serde_json::from_str(&ctx.to_json().expect("json")).expect("parse");
        assert_eq!(back, ctx.inner);
    }

    #[test]
    fn daycount_invalid_string() {
        assert!("not_a_daycount".parse::<RustDayCount>().is_err());
    }

    #[test]
    fn tenor_invalid_string() {
        assert!(RustTenor::parse("").is_err());
        assert!(RustTenor::parse("XYZ").is_err());
    }
}
