//! WASM bindings for reporting periods from [`finstack_quant_core::dates`]:
//! `PeriodKind`, `PeriodId`, `FiscalConfig` and the period-plan builders.
//!
//! `Period` and `PeriodPlan` are plain data: they cross the boundary as plain
//! objects typed by the schema-generated `Period` / `PeriodPlan` types.

use crate::utils::input::{js_epoch_days, js_int, js_opt_string, js_string, js_uint};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::dates::{
    FiscalConfig, PeriodId, PeriodKind, build_fiscal_periods as rust_build_fiscal_periods,
    build_periods as rust_build_periods, days_since_epoch,
};
use wasm_bindgen::prelude::*;

/// Reporting-period frequency (Rust `PeriodKind`).
///
/// Each static factory is one frequency; `toString()` is the canonical
/// snake_case name shared with the JSON wire format.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const kind = core.PeriodKind.quarterly();
/// kind.periodsPerYear; // 4
/// kind.priorObservationDate(core.createDate(2025, 3, 31)); // 2024-12-31 as epoch days
/// ```
#[wasm_bindgen(js_name = PeriodKind)]
#[derive(Clone, Copy, Debug)]
pub struct JsPeriodKind {
    pub(crate) inner: PeriodKind,
}

impl JsPeriodKind {
    const fn from_inner(inner: PeriodKind) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen(js_class = PeriodKind)]
impl JsPeriodKind {
    /// Daily periods (252 trading days per year).
    ///
    /// @returns The `daily` frequency.
    #[wasm_bindgen(js_name = daily)]
    pub fn daily() -> Self {
        Self::from_inner(PeriodKind::Daily)
    }

    /// ISO-week periods (52 per year).
    ///
    /// @returns The `weekly` frequency.
    #[wasm_bindgen(js_name = weekly)]
    pub fn weekly() -> Self {
        Self::from_inner(PeriodKind::Weekly)
    }

    /// Calendar-month periods (12 per year).
    ///
    /// @returns The `monthly` frequency.
    #[wasm_bindgen(js_name = monthly)]
    pub fn monthly() -> Self {
        Self::from_inner(PeriodKind::Monthly)
    }

    /// Calendar-quarter periods (4 per year).
    ///
    /// @returns The `quarterly` frequency.
    #[wasm_bindgen(js_name = quarterly)]
    pub fn quarterly() -> Self {
        Self::from_inner(PeriodKind::Quarterly)
    }

    /// Half-year periods (2 per year).
    ///
    /// @returns The `semi_annual` frequency.
    #[wasm_bindgen(js_name = semiAnnual)]
    pub fn semi_annual() -> Self {
        Self::from_inner(PeriodKind::SemiAnnual)
    }

    /// Full-year periods (1 per year).
    ///
    /// @returns The `annual` frequency.
    #[wasm_bindgen(js_name = annual)]
    pub fn annual() -> Self {
        Self::from_inner(PeriodKind::Annual)
    }

    /// Parse a frequency name (Rust `PeriodKind::from_str`).
    ///
    /// # Arguments
    ///
    /// * `name` - Frequency name such as `"daily"`, `"monthly"`,
    ///   `"quarterly"`, `"semi_annual"` or `"annual"`.
    ///
    /// @returns The matching `PeriodKind`.
    /// @throws `TypeError` (kind `invalid_type`) if `name` is not a string;
    /// `FinstackError` (kind `validation`) if no frequency matches.
    /// @param name - Frequency name such as `"daily"`, `"monthly"`, `"quarterly"`, `"semi_annual"` or `"annual"`.
    #[wasm_bindgen(js_name = fromName)]
    pub fn from_name(
        #[wasm_bindgen(unchecked_param_type = "string")] name: JsValue,
    ) -> Result<JsPeriodKind, JsValue> {
        js_string(&name, "name")?
            .parse::<PeriodKind>()
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Number of periods per year (daily uses the 252 trading-day convention).
    #[wasm_bindgen(getter, js_name = periodsPerYear)]
    pub fn periods_per_year(&self) -> u16 {
        self.inner.periods_per_year()
    }

    /// Observation date one frequency step before `first` (Rust
    /// `PeriodKind::prior_observation_date`).
    ///
    /// # Arguments
    ///
    /// * `first` - First return-aligned observation date, as days since
    ///   1970-01-01.
    ///
    /// @returns The prior observation date as epoch days: one or seven calendar
    /// days back for daily/weekly, otherwise 1/3/6/12 months back clamped to the
    /// last valid day of the target month.
    /// @throws `TypeError` if `first` is not an integer; `FinstackError` (kind
    /// `validation`) if it is outside the supported date range.
    /// @param first - First return-aligned observation date, as days since 1970-01-01.
    #[wasm_bindgen(js_name = priorObservationDate)]
    pub fn prior_observation_date(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] first: JsValue,
    ) -> Result<i32, JsValue> {
        let first = js_epoch_days(&first, "first")?;
        self.inner
            .prior_observation_date(first)
            .map(days_since_epoch)
            .map_err(to_js_err)
    }

    /// Canonical snake_case name of the frequency.
    ///
    /// @returns The name accepted by `fromName` and the JSON wire format.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}

/// Identifier of one reporting period, such as `2025Q1`, `2025M03` or `FY2025Q2`.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const q1 = core.PeriodId.quarter(2025, 1);
/// q1.toString(); // "2025Q1"
/// q1.shift(1).toString(); // "2025Q2"
/// core.PeriodId.parse("2025M12").shift(1).toString(); // "2026M01"
/// ```
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const q1 = core.PeriodId.quarter(2025, 1);
/// q1.toString(); // "2025Q1"
/// q1.shift(1).toString(); // "2025Q2"
/// core.PeriodId.parse("2025M12").shift(1).toString(); // "2026M01"
/// ```
#[wasm_bindgen(js_name = PeriodId)]
#[derive(Clone, Copy, Debug)]
pub struct JsPeriodId {
    pub(crate) inner: PeriodId,
}

impl JsPeriodId {
    fn from_inner(inner: PeriodId) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen(js_class = PeriodId)]
impl JsPeriodId {
    /// Parse a period code (Rust `PeriodId::from_str`).
    ///
    /// # Arguments
    ///
    /// * `code` - Period code such as `"2025Q1"`, `"2025M03"`, `"2025H1"`,
    ///   `"2025W05"`, `"2025D032"`, `"2025"`, or a fiscal code such as
    ///   `"FY2025Q1"`.
    ///
    /// @returns The parsed `PeriodId`.
    /// @throws `TypeError` (kind `invalid_type`) if `code` is not a string;
    /// `FinstackError` (kind `validation`) if it is not a period code.
    /// @param code - Period code such as `"2025Q1"`, `"2025M03"`, `"2025H1"`, `"2025W05"`, `"2025D032"`, `"2025"`, or a fiscal code such as `"FY2025Q1"`.
    #[wasm_bindgen(js_name = parse)]
    pub fn parse(
        #[wasm_bindgen(unchecked_param_type = "string")] code: JsValue,
    ) -> Result<JsPeriodId, JsValue> {
        js_string(&code, "code")?
            .parse::<PeriodId>()
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Build a monthly identifier.
    ///
    /// # Arguments
    ///
    /// * `year` - Calendar year of the period.
    /// * `month` - Month number, `1` through `12`.
    ///
    /// @returns The monthly `PeriodId`.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) if `month` is outside `1..=12`.
    /// @param year - Calendar year of the period.
    /// @param month - Month number, `1` through `12`.
    #[wasm_bindgen(js_name = month)]
    pub fn month(
        #[wasm_bindgen(unchecked_param_type = "number")] year: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] month: JsValue,
    ) -> Result<JsPeriodId, JsValue> {
        PeriodId::month(js_int(&year, "year")?, js_uint(&month, "month")?)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Build a quarterly identifier.
    ///
    /// # Arguments
    ///
    /// * `year` - Calendar year of the period.
    /// * `quarter` - Quarter number, `1` through `4`.
    ///
    /// @returns The quarterly `PeriodId`.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) if `quarter` is outside `1..=4`.
    /// @param year - Calendar year of the period.
    /// @param quarter - Quarter number, `1` through `4`.
    #[wasm_bindgen(js_name = quarter)]
    pub fn quarter(
        #[wasm_bindgen(unchecked_param_type = "number")] year: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] quarter: JsValue,
    ) -> Result<JsPeriodId, JsValue> {
        PeriodId::quarter(js_int(&year, "year")?, js_uint(&quarter, "quarter")?)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Build an annual identifier.
    ///
    /// # Arguments
    ///
    /// * `year` - Calendar year of the period.
    ///
    /// @returns The annual `PeriodId`.
    /// @throws `TypeError` if `year` is not an integer; `FinstackError` (kind
    /// `validation`) when the year's bounds are outside the supported date range.
    /// @param year - Calendar year of the period.
    #[wasm_bindgen(js_name = annual)]
    pub fn annual(
        #[wasm_bindgen(unchecked_param_type = "number")] year: JsValue,
    ) -> Result<JsPeriodId, JsValue> {
        PeriodId::annual(js_int(&year, "year")?)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Build a half-year identifier.
    ///
    /// # Arguments
    ///
    /// * `year` - Calendar year of the period.
    /// * `half` - Half number, `1` or `2`.
    ///
    /// @returns The semi-annual `PeriodId`.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) if `half` is not `1` or `2`.
    /// @param year - Calendar year of the period.
    /// @param half - Half number, `1` or `2`.
    #[wasm_bindgen(js_name = half)]
    pub fn half(
        #[wasm_bindgen(unchecked_param_type = "number")] year: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] half: JsValue,
    ) -> Result<JsPeriodId, JsValue> {
        PeriodId::half(js_int(&year, "year")?, js_uint(&half, "half")?)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Build an ISO-week identifier.
    ///
    /// # Arguments
    ///
    /// * `year` - ISO week-year of the period.
    /// * `week` - ISO week number, `1` through `52` (or `53` in long years).
    ///
    /// @returns The weekly `PeriodId`.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) if `week` is not valid for `year`.
    /// @param year - ISO week-year of the period.
    /// @param week - ISO week number, `1` through `52` (or `53` in long years).
    #[wasm_bindgen(js_name = week)]
    pub fn week(
        #[wasm_bindgen(unchecked_param_type = "number")] year: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] week: JsValue,
    ) -> Result<JsPeriodId, JsValue> {
        PeriodId::week(js_int(&year, "year")?, js_uint(&week, "week")?)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Build a daily identifier from a day-of-year ordinal.
    ///
    /// # Arguments
    ///
    /// * `year` - Calendar year of the period.
    /// * `ordinal` - One-based day of the year, `1` through `365` (or `366`
    ///   in leap years).
    ///
    /// @returns The daily `PeriodId`.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) if `ordinal` is not valid for `year`.
    /// @param year - Calendar year of the period.
    /// @param ordinal - One-based day of the year, `1` through `365` (or `366` in leap years).
    #[wasm_bindgen(js_name = day)]
    pub fn day(
        #[wasm_bindgen(unchecked_param_type = "number")] year: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] ordinal: JsValue,
    ) -> Result<JsPeriodId, JsValue> {
        PeriodId::day(js_int(&year, "year")?, js_uint(&ordinal, "ordinal")?)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Calendar (or fiscal) year of the period.
    #[wasm_bindgen(getter, js_name = year)]
    pub fn year(&self) -> i32 {
        self.inner.year()
    }

    /// One-based index of the period within its year (quarter, month, week or day number).
    #[wasm_bindgen(getter, js_name = index)]
    pub fn index(&self) -> u16 {
        self.inner.index()
    }

    /// Frequency of the period as a `PeriodKind`.
    #[wasm_bindgen(getter, js_name = kind)]
    pub fn kind(&self) -> JsPeriodKind {
        JsPeriodKind::from_inner(self.inner.kind())
    }

    /// Whether the identifier uses fiscal-year (`FY…`) semantics.
    #[wasm_bindgen(getter, js_name = isFiscal)]
    pub fn is_fiscal(&self) -> bool {
        self.inner.is_fiscal()
    }

    /// Move by a whole number of periods of the same kind in `calendar`
    /// (Rust `PeriodId::shift`).
    ///
    /// # Arguments
    ///
    /// * `periods` - Whole number of periods to move; negative moves back and
    ///   `0` returns the identifier after validating it.
    /// * `calendar` - The identifier's calendar context: `"gregorian"` for an
    ///   unprefixed identifier, `{ fiscal: { start_month, start_day } }` for an
    ///   `FY` identifier.
    ///
    /// @returns The `PeriodId` `periods` steps away in `calendar`.
    /// @throws `TypeError` (kind `invalid_type`) when `periods` is not a whole
    /// number; `FinstackError` (kind `validation`) when `calendar` does not
    /// match the identifier, the identifier is not a period of `calendar`, or
    /// the result leaves the supported range.
    /// @param periods - Whole number of periods to move; negative moves back and `0` returns the identifier after validating it.
    /// @param calendar - The identifier's calendar context: `"gregorian"` for an unprefixed identifier, `{ fiscal: { start_month, start_day } }` for an `FY` identifier.
    #[wasm_bindgen(js_name = shift)]
    pub fn shift(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] periods: JsValue,
        #[wasm_bindgen(unchecked_param_type = "PeriodPlan['calendar']")] calendar: JsValue,
    ) -> Result<JsPeriodId, JsValue> {
        let periods: i32 = crate::utils::input::js_int(&periods, "periods")?;
        let calendar: finstack_quant_core::dates::PeriodCalendar =
            serde_json::from_value(crate::utils::input::js_json_value(&calendar, "calendar")?)
                .map_err(|error| {
                    to_js_err(finstack_quant_core::Error::Validation(format!(
                        "calendar: {error}"
                    )))
                })?;
        self.inner
            .shift(periods, calendar)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Period code, such as `"2025Q1"`.
    ///
    /// @returns The text accepted by `PeriodId.parse`.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}

/// Fiscal-year start (month and day) used to map fiscal periods onto calendar dates.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const fiscal = new core.FiscalConfig(10, 1); // opens 1 October
/// core.fiscalYear(core.createDate(2024, 11, 15), fiscal); // 2025
/// new core.FiscalConfig(4, 6).startDay; // 6
/// ```
#[wasm_bindgen(js_name = FiscalConfig)]
#[derive(Clone, Copy, Debug)]
pub struct JsFiscalConfig {
    pub(crate) inner: FiscalConfig,
}

impl JsFiscalConfig {
    const fn from_inner(inner: FiscalConfig) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen(js_class = FiscalConfig)]
impl JsFiscalConfig {
    /// Create a fiscal configuration (Rust `FiscalConfig::new`).
    ///
    /// # Arguments
    ///
    /// * `start_month` - Month the fiscal year starts in, `1` (January)
    ///   through `12`.
    /// * `start_day` - Day of that month the fiscal year starts on, `1`
    ///   through `31`; validity for a specific year (for example 30 February)
    ///   is checked when the configuration is applied.
    ///
    /// @returns The validated `FiscalConfig`.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) if `startMonth` is outside `1..=12` or `startDay` is
    /// outside `1..=31`.
    /// @param startMonth - Month the fiscal year starts in, `1` (January) through `12`.
    /// @param startDay - Day of that month the fiscal year starts on, `1` through `31`; validity for a specific year (for example 30 February) is checked when the configuration is applied.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(js_name = "startMonth", unchecked_param_type = "number")]
        start_month: JsValue,
        #[wasm_bindgen(js_name = "startDay", unchecked_param_type = "number")] start_day: JsValue,
    ) -> Result<JsFiscalConfig, JsValue> {
        FiscalConfig::new(
            js_uint(&start_month, "startMonth")?,
            js_uint(&start_day, "startDay")?,
        )
        .map(Self::from_inner)
        .map_err(to_js_err)
    }

    /// Calendar-year fiscal configuration (1 January).
    ///
    /// @returns The calendar-year `FiscalConfig`.
    #[wasm_bindgen(js_name = calendarYear)]
    pub fn calendar_year() -> Self {
        Self::from_inner(FiscalConfig::calendar_year())
    }

    /// Month the fiscal year starts in (1 = January).
    #[wasm_bindgen(getter, js_name = startMonth)]
    pub fn start_month(&self) -> u8 {
        self.inner.start_month()
    }

    /// Day of the start month the fiscal year starts on.
    #[wasm_bindgen(getter, js_name = startDay)]
    pub fn start_day(&self) -> u8 {
        self.inner.start_day()
    }
}

/// Build a plan of calendar periods from a range expression (Rust `build_periods`).
///
/// # Arguments
///
/// * `spec` - Period range such as `"2025Q1..Q4"` or `"2025M01..2025M12"`;
///   both ends must use the same frequency.
/// * `actuals_cutoff` - Inclusive period code up to which periods are marked
///   actual (`is_actual: true`); omitted marks every period as forecast.
///
/// @returns A plain `PeriodPlan` object: `periods` in ascending order, each
/// with `id`, ISO `start`/`end` dates (end exclusive) and `is_actual`.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if the range or the cutoff cannot be parsed or the two ends
/// are incompatible.
/// @param spec - Period range such as `"2025Q1..Q4"` or `"2025M01..2025M12"`; both ends must use the same frequency.
/// @param actualsCutoff - Inclusive period code up to which periods are marked actual (`is_actual: true`); omitted marks every period as forecast.
#[wasm_bindgen(js_name = buildPeriods, unchecked_return_type = "PeriodPlan")]
pub fn build_periods(
    #[wasm_bindgen(unchecked_param_type = "string")] spec: JsValue,
    #[wasm_bindgen(
        js_name = "actualsCutoff",
        unchecked_optional_param_type = "string | null"
    )]
    actuals_cutoff: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let spec = js_string(&spec, "spec")?;
    let cutoff = js_opt_string(actuals_cutoff.as_ref(), "actualsCutoff")?;
    let plan = rust_build_periods(&spec, cutoff.as_deref()).map_err(to_js_err)?;
    to_js_value(&plan)
}

/// Build a plan of fiscal periods mapped onto calendar dates (Rust
/// `build_fiscal_periods`).
///
/// # Arguments
///
/// * `spec` - Fiscal period range such as `"FY2025Q1..Q4"`.
/// * `fiscal_config` - Fiscal-year start that maps fiscal periods to
///   calendar dates.
/// * `actuals_cutoff` - Inclusive fiscal period code up to which periods are
///   marked actual; omitted marks every period as forecast.
///
/// @returns A plain `PeriodPlan` object with fiscal identifiers and calendar
/// `start`/`end` dates.
/// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
/// `validation`) if a fiscal identifier cannot be parsed or the configuration
/// produces invalid calendar boundaries.
/// @param spec - Fiscal period range such as `"FY2025Q1..Q4"`.
/// @param fiscalConfig - Fiscal-year start that maps fiscal periods to calendar dates.
/// @param actualsCutoff - Inclusive fiscal period code up to which periods are marked actual; omitted marks every period as forecast.
#[wasm_bindgen(js_name = buildFiscalPeriods, unchecked_return_type = "PeriodPlan")]
pub fn build_fiscal_periods(
    #[wasm_bindgen(unchecked_param_type = "string")] spec: JsValue,
    #[wasm_bindgen(js_name = "fiscalConfig")] fiscal_config: &JsFiscalConfig,
    #[wasm_bindgen(
        js_name = "actualsCutoff",
        unchecked_optional_param_type = "string | null"
    )]
    actuals_cutoff: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let spec = js_string(&spec, "spec")?;
    let cutoff = js_opt_string(actuals_cutoff.as_ref(), "actualsCutoff")?;
    let plan = rust_build_fiscal_periods(&spec, fiscal_config.inner, cutoff.as_deref())
        .map_err(to_js_err)?;
    to_js_value(&plan)
}
