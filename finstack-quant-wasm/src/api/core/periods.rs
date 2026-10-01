//! WASM bindings for reporting periods from [`finstack_quant_core::dates`]:
//! `PeriodKind`, `PeriodId`, `FiscalConfig` and the period-plan builders.
//!
//! `Period` and `PeriodPlan` are plain data: they cross the boundary as plain
//! objects typed by the schema-generated `Period` / `PeriodPlan` types.

use crate::utils::input::{js_epoch_days, js_int, js_opt_string, js_string, js_uint};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::dates::{
    build_fiscal_periods as rust_build_fiscal_periods, build_periods as rust_build_periods,
    days_since_epoch, FiscalConfig, PeriodId, PeriodKind,
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
    const fn wrap(inner: PeriodKind) -> Self {
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
        Self::wrap(PeriodKind::Daily)
    }

    /// ISO-week periods (52 per year).
    ///
    /// @returns The `weekly` frequency.
    #[wasm_bindgen(js_name = weekly)]
    pub fn weekly() -> Self {
        Self::wrap(PeriodKind::Weekly)
    }

    /// Calendar-month periods (12 per year).
    ///
    /// @returns The `monthly` frequency.
    #[wasm_bindgen(js_name = monthly)]
    pub fn monthly() -> Self {
        Self::wrap(PeriodKind::Monthly)
    }

    /// Calendar-quarter periods (4 per year).
    ///
    /// @returns The `quarterly` frequency.
    #[wasm_bindgen(js_name = quarterly)]
    pub fn quarterly() -> Self {
        Self::wrap(PeriodKind::Quarterly)
    }

    /// Half-year periods (2 per year).
    ///
    /// @returns The `semi_annual` frequency.
    #[wasm_bindgen(js_name = semiAnnual)]
    pub fn semi_annual() -> Self {
        Self::wrap(PeriodKind::SemiAnnual)
    }

    /// Full-year periods (1 per year).
    ///
    /// @returns The `annual` frequency.
    #[wasm_bindgen(js_name = annual)]
    pub fn annual() -> Self {
        Self::wrap(PeriodKind::Annual)
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
    #[wasm_bindgen(js_name = fromName)]
    pub fn from_name(name: JsValue) -> Result<JsPeriodKind, JsValue> {
        js_string(&name, "name")?
            .parse::<PeriodKind>()
            .map(Self::wrap)
            .map_err(to_js_err)
    }

    /// Number of periods per year (daily uses the 252 trading-day convention).
    #[wasm_bindgen(getter, js_name = periodsPerYear)]
    pub fn periods_per_year(&self) -> u16 {
        self.inner.periods_per_year()
    }

    /// Factor that scales per-period statistics to annual ones (`periodsPerYear` as a float).
    #[wasm_bindgen(getter, js_name = annualizationFactor)]
    pub fn annualization_factor(&self) -> f64 {
        self.inner.annualization_factor()
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
    #[wasm_bindgen(js_name = priorObservationDate)]
    pub fn prior_observation_date(&self, first: JsValue) -> Result<i32, JsValue> {
        let first = js_epoch_days(&first, "first")?;
        Ok(days_since_epoch(self.inner.prior_observation_date(first)))
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
/// q1.code; // "2025Q1"
/// q1.next().code; // "2025Q2"
/// core.PeriodId.parse("2025M12").next().code; // "2026M01"
/// ```
#[wasm_bindgen(js_name = PeriodId)]
#[derive(Clone, Copy, Debug)]
pub struct JsPeriodId {
    pub(crate) inner: PeriodId,
}

impl JsPeriodId {
    fn wrap(inner: PeriodId) -> Self {
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
    #[wasm_bindgen(js_name = parse)]
    pub fn parse(code: JsValue) -> Result<JsPeriodId, JsValue> {
        js_string(&code, "code")?
            .parse::<PeriodId>()
            .map(Self::wrap)
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
    #[wasm_bindgen(js_name = month)]
    pub fn month(year: JsValue, month: JsValue) -> Result<JsPeriodId, JsValue> {
        PeriodId::month(js_int(&year, "year")?, js_uint(&month, "month")?)
            .map(Self::wrap)
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
    #[wasm_bindgen(js_name = quarter)]
    pub fn quarter(year: JsValue, quarter: JsValue) -> Result<JsPeriodId, JsValue> {
        PeriodId::quarter(js_int(&year, "year")?, js_uint(&quarter, "quarter")?)
            .map(Self::wrap)
            .map_err(to_js_err)
    }

    /// Build an annual identifier.
    ///
    /// # Arguments
    ///
    /// * `year` - Calendar year of the period.
    ///
    /// @returns The annual `PeriodId`.
    /// @throws `TypeError` if `year` is not an integer.
    #[wasm_bindgen(js_name = annual)]
    pub fn annual(year: JsValue) -> Result<JsPeriodId, JsValue> {
        Ok(Self::wrap(PeriodId::annual(js_int(&year, "year")?)))
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
    #[wasm_bindgen(js_name = half)]
    pub fn half(year: JsValue, half: JsValue) -> Result<JsPeriodId, JsValue> {
        PeriodId::half(js_int(&year, "year")?, js_uint(&half, "half")?)
            .map(Self::wrap)
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
    #[wasm_bindgen(js_name = week)]
    pub fn week(year: JsValue, week: JsValue) -> Result<JsPeriodId, JsValue> {
        PeriodId::week(js_int(&year, "year")?, js_uint(&week, "week")?)
            .map(Self::wrap)
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
    #[wasm_bindgen(js_name = day)]
    pub fn day(year: JsValue, ordinal: JsValue) -> Result<JsPeriodId, JsValue> {
        PeriodId::day(js_int(&year, "year")?, js_uint(&ordinal, "ordinal")?)
            .map(Self::wrap)
            .map_err(to_js_err)
    }

    /// Period code, such as `"2025Q1"` (the text `parse` accepts).
    #[wasm_bindgen(getter, js_name = code)]
    pub fn code(&self) -> String {
        self.inner.to_string()
    }

    /// Calendar (or fiscal) year of the period.
    #[wasm_bindgen(getter, js_name = year)]
    pub fn year(&self) -> i32 {
        self.inner.year
    }

    /// One-based index of the period within its year (quarter, month, week or day number).
    #[wasm_bindgen(getter, js_name = index)]
    pub fn index(&self) -> u16 {
        self.inner.index
    }

    /// Frequency of the period as a `PeriodKind`.
    #[wasm_bindgen(getter, js_name = kind)]
    pub fn kind(&self) -> JsPeriodKind {
        JsPeriodKind::wrap(self.inner.kind())
    }

    /// Whether the identifier uses fiscal-year (`FY…`) semantics.
    #[wasm_bindgen(getter, js_name = isFiscal)]
    pub fn is_fiscal(&self) -> bool {
        self.inner.is_fiscal()
    }

    /// Number of periods per year at this identifier's frequency.
    #[wasm_bindgen(getter, js_name = periodsPerYear)]
    pub fn periods_per_year(&self) -> u16 {
        self.inner.periods_per_year()
    }

    /// Step forward to the next period (Rust `PeriodId::next`).
    ///
    /// @returns The following `PeriodId`, rolling the year where needed.
    /// @throws `FinstackError` (kind `validation`) for a fiscal identifier
    /// (use `nextFiscal`) or when the year would overflow.
    #[wasm_bindgen(js_name = next)]
    pub fn next(&self) -> Result<JsPeriodId, JsValue> {
        self.inner.next().map(Self::wrap).map_err(to_js_err)
    }

    /// Step back to the previous period (Rust `PeriodId::prev`).
    ///
    /// @returns The preceding `PeriodId`, rolling the year where needed.
    /// @throws `FinstackError` (kind `validation`) for a fiscal identifier
    /// (use `prevFiscal`) or when the year would overflow.
    #[wasm_bindgen(js_name = prev)]
    pub fn prev(&self) -> Result<JsPeriodId, JsValue> {
        self.inner.prev().map(Self::wrap).map_err(to_js_err)
    }

    /// Step forward to the next fiscal period (Rust `PeriodId::next_fiscal`).
    ///
    /// # Arguments
    ///
    /// * `fiscal_config` - Fiscal-year start used to size fiscal weeks and days.
    ///
    /// @returns The following `PeriodId`, marked fiscal.
    /// @throws `FinstackError` (kind `validation`) if the configuration has an
    /// invalid fiscal start date for the year or the fiscal-year boundary is
    /// outside the supported date range.
    #[wasm_bindgen(js_name = nextFiscal)]
    pub fn next_fiscal(&self, fiscal_config: &JsFiscalConfig) -> Result<JsPeriodId, JsValue> {
        self.inner
            .next_fiscal(fiscal_config.inner)
            .map(Self::wrap)
            .map_err(to_js_err)
    }

    /// Step back to the previous fiscal period (Rust `PeriodId::prev_fiscal`).
    ///
    /// # Arguments
    ///
    /// * `fiscal_config` - Fiscal-year start used to size fiscal weeks and days.
    ///
    /// @returns The preceding `PeriodId`, marked fiscal.
    /// @throws `FinstackError` (kind `validation`) if the configuration has an
    /// invalid fiscal start date for the year or the fiscal-year boundary is
    /// outside the supported date range.
    #[wasm_bindgen(js_name = prevFiscal)]
    pub fn prev_fiscal(&self, fiscal_config: &JsFiscalConfig) -> Result<JsPeriodId, JsValue> {
        self.inner
            .prev_fiscal(fiscal_config.inner)
            .map(Self::wrap)
            .map_err(to_js_err)
    }

    /// Period code (same as `code`).
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
/// const fiscal = core.FiscalConfig.usFederal(); // 1 October
/// core.fiscalYear(core.createDate(2024, 11, 15), fiscal); // 2025
/// new core.FiscalConfig(4, 6).startDay; // 6
/// ```
#[wasm_bindgen(js_name = FiscalConfig)]
#[derive(Clone, Copy, Debug)]
pub struct JsFiscalConfig {
    pub(crate) inner: FiscalConfig,
}

impl JsFiscalConfig {
    const fn wrap(inner: FiscalConfig) -> Self {
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
    #[wasm_bindgen(constructor)]
    pub fn new(start_month: JsValue, start_day: JsValue) -> Result<JsFiscalConfig, JsValue> {
        FiscalConfig::new(
            js_uint(&start_month, "startMonth")?,
            js_uint(&start_day, "startDay")?,
        )
        .map(Self::wrap)
        .map_err(to_js_err)
    }

    /// Calendar-year fiscal configuration (1 January).
    ///
    /// @returns The calendar-year `FiscalConfig`.
    #[wasm_bindgen(js_name = calendarYear)]
    pub fn calendar_year() -> Self {
        Self::wrap(FiscalConfig::calendar_year())
    }

    /// US federal government fiscal year (1 October).
    ///
    /// @returns The US federal `FiscalConfig`.
    #[wasm_bindgen(js_name = usFederal)]
    pub fn us_federal() -> Self {
        Self::wrap(FiscalConfig::us_federal())
    }

    /// UK government fiscal year (6 April).
    ///
    /// @returns The UK `FiscalConfig`.
    #[wasm_bindgen(js_name = uk)]
    pub fn uk() -> Self {
        Self::wrap(FiscalConfig::uk())
    }

    /// Japanese government fiscal year (1 April).
    ///
    /// @returns The Japanese `FiscalConfig`.
    #[wasm_bindgen(js_name = japan)]
    pub fn japan() -> Self {
        Self::wrap(FiscalConfig::japan())
    }

    /// Australian government fiscal year (1 July).
    ///
    /// @returns The Australian `FiscalConfig`.
    #[wasm_bindgen(js_name = australia)]
    pub fn australia() -> Self {
        Self::wrap(FiscalConfig::australia())
    }

    /// Month the fiscal year starts in (1 = January).
    #[wasm_bindgen(getter, js_name = startMonth)]
    pub fn start_month(&self) -> u8 {
        self.inner.start_month
    }

    /// Day of the start month the fiscal year starts on.
    #[wasm_bindgen(getter, js_name = startDay)]
    pub fn start_day(&self) -> u8 {
        self.inner.start_day
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
#[wasm_bindgen(js_name = buildPeriods)]
pub fn build_periods(spec: JsValue, actuals_cutoff: Option<JsValue>) -> Result<JsValue, JsValue> {
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
#[wasm_bindgen(js_name = buildFiscalPeriods)]
pub fn build_fiscal_periods(
    spec: JsValue,
    fiscal_config: &JsFiscalConfig,
    actuals_cutoff: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let spec = js_string(&spec, "spec")?;
    let cutoff = js_opt_string(actuals_cutoff.as_ref(), "actualsCutoff")?;
    let plan = rust_build_fiscal_periods(&spec, fiscal_config.inner, cutoff.as_deref())
        .map_err(to_js_err)?;
    to_js_value(&plan)
}
