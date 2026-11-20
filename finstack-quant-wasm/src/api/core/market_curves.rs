//! WASM bindings for the inflation, price and base-correlation term
//! structures and for credit-index market data
//! (`finstack_quant_core::market_data::term_structures`).
//!
//! Curve dates are ISO-8601 strings (`"YYYY-MM-DD"`), as for `DiscountCurve`.

use crate::api::core::curve_args::TimePoint;
use crate::api::core::curve_args::flat_pairs;
use crate::api::core::curve_args::js_time_point;
use crate::api::core::market_hazard::JsHazardCurve;
use crate::utils::input::{
    from_js_json, from_js_json_exact, js_f64, js_f64_seq, js_string, js_uint,
};
use crate::utils::wire::js_date;
use crate::utils::{date_to_iso, parse_iso_date, to_js_err};
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::term_structures::{
    BaseCorrelationCurve as RustBaseCorrelationCurve, CreditIndexData as RustCreditIndexData,
    InflationCurve as RustInflationCurve, PriceCurve as RustPriceCurve, PriceCurveKind,
};
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use serde::Deserialize;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// Projected CPI curve for inflation-linked valuation.
///
/// Built from `(time, CPI level)` pillars; `time` is a year fraction from
/// `baseDate` and the level is the consumer-price index itself (not a rate).
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const curve = new core.InflationCurve({
///   id: "US-CPI",
///   baseDate: "2025-01-02",
///   knots: [0.0, 300.0, 5.0, 331.2],
///   dayCount: "act_365f",
///   interp: "log_linear",
///   extrapolation: "extend",
/// });
/// curve.cpi(2.5); // projected CPI level at 2.5y
/// curve.inflationRate(0.0, 5.0); // annualised inflation over 5y
/// ```
#[wasm_bindgen(js_name = InflationCurve)]
pub struct JsInflationCurve {
    pub(crate) inner: Arc<RustInflationCurve>,
}

/// Named constructor options for `InflationCurve`; unknown keys are rejected.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InflationCurveOptions {
    id: String,
    base_date: String,
    knots: Vec<f64>,
    day_count: String,
    interp: String,
    extrapolation: String,
    calendar_id: Option<String>,
}

impl JsInflationCurve {
    pub(crate) fn from_inner(inner: Arc<RustInflationCurve>) -> Self {
        Self { inner }
    }

    fn build(options: InflationCurveOptions) -> Result<JsInflationCurve, JsValue> {
        RustInflationCurve::builder(options.id)
            .base_date(parse_iso_date(&options.base_date)?)
            .day_count(options.day_count.parse::<DayCount>().map_err(to_js_err)?)
            .calendar_id_opt(options.calendar_id)
            .knots(flat_pairs(&options.knots, "knots")?)
            .interp(options.interp.parse::<InterpStyle>().map_err(to_js_err)?)
            .extrapolation(
                options
                    .extrapolation
                    .parse::<ExtrapolationPolicy>()
                    .map_err(to_js_err)?,
            )
            .build()
            .map(|curve| Self::from_inner(Arc::new(curve)))
            .map_err(to_js_err)
    }
}

#[wasm_bindgen(js_class = InflationCurve)]
impl JsInflationCurve {
    /// Construct an inflation curve from dated projected CPI levels.
    /// @param id - Market source identifier.
    /// @param baseDate - ISO-8601 date defining zero curve time.
    /// @param points - ISO date and quote pairs. Strictly increasing observation dates with the first observation on the base date. Finite positive unlagged CPI levels in index points.
    /// @param dayCount - Date-to-year-fraction convention such as act_365f.
    /// @param interp - Interpolation label for positive CPI.
    /// @param extrapolation - Outside-grid policy, such as flat.
    /// @param calendarId - Holiday calendar, required for calendar-dependent day counts.
    /// @returns The validated curve on the supplied clock.
    /// @throws TypeError for mistyped fields or FinstackError for invalid curves or clock conventions.
    #[wasm_bindgen(js_name = fromDates)]
    pub fn from_dates(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        #[wasm_bindgen(js_name = "baseDate", unchecked_param_type = "string")] base_date: JsValue,
        #[wasm_bindgen(unchecked_param_type = "readonly (readonly [string, number])[] | string")]
        points: JsValue,
        #[wasm_bindgen(js_name = "dayCount", unchecked_param_type = "string")] day_count: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] interp: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] extrapolation: JsValue,
        #[wasm_bindgen(js_name = "calendarId", unchecked_optional_param_type = "string")]
        calendar_id: Option<JsValue>,
    ) -> Result<JsInflationCurve, JsValue> {
        let points = from_js_json_exact::<Vec<(String, f64)>>(&points, "points")?
            .into_iter()
            .map(|(date, quote)| Ok((parse_iso_date(&date)?, quote)))
            .collect::<Result<Vec<_>, JsValue>>()?;
        let calendar_id = calendar_id
            .as_ref()
            .map(|id| js_string(id, "calendarId"))
            .transpose()?;
        RustInflationCurve::from_dates(
            js_string(&id, "id")?,
            js_date(&base_date, "baseDate")?,
            &points,
            js_string(&day_count, "dayCount")?
                .parse::<DayCount>()
                .map_err(to_js_err)?,
            js_string(&interp, "interp")?
                .parse::<InterpStyle>()
                .map_err(to_js_err)?,
            js_string(&extrapolation, "extrapolation")?
                .parse::<ExtrapolationPolicy>()
                .map_err(to_js_err)?,
            calendar_id.as_deref(),
        )
        .map(|curve| Self::from_inner(Arc::new(curve)))
        .map_err(to_js_err)
    }

    /// Construct an inflation curve from named options.
    ///
    /// # Arguments
    ///
    /// * `options` - InflationCurveOptions object (or its JSON text) with:
    ///   `id` (curve identifier, the `MarketContext` lookup key); `baseDate`
    ///   (ISO-8601; knot times are year fractions from it under `dayCount`);
    ///   `knots` (flat `[0, I0, t1, I1, …]` array, years and positive index levels);
    ///   required `dayCount`, `interp` and `extrapolation`; optional `calendarId`
    ///   (required for Bus/252). Unknown keys are rejected.
    ///
    /// @returns The constructed `InflationCurve`.
    /// @throws `TypeError` (kind `invalid_type`) if `options` is not a JSON
    /// string or plain object; `FinstackError` (kind `validation`) for an
    /// unknown, missing or mistyped key, an odd-length `knots`, a malformed
    /// date, an unknown day-count/interpolation/extrapolation name, or CPI
    /// levels the curve builder rejects.
    /// @param options - Explicit curve identifier, ISO base date, day count, interpolation and extrapolation, and flat knots beginning `[0, I0]` with at least one positive-time pillar. `calendarId` is required for Bus/252; no contractual lag is applied.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "InflationCurveOptions | string")] options: JsValue,
    ) -> Result<JsInflationCurve, JsValue> {
        Self::build(from_js_json(&options, "options")?)
    }

    /// Projected CPI level at year fraction `t` or on an ISO-8601 date, without
    /// indexation lag (Rust `InflationCurve::cpi` / `InflationCurve::cpi_on_date`).
    ///
    /// # Arguments
    ///
    /// * `t` - Time from the curve base date in years (`number`) or an ISO-8601
    ///   target date (`"YYYY-MM-DD"`, converted with the curve day count).
    ///
    /// @returns The interpolated index level.
    /// @throws `TypeError` if `t` is neither a number nor a string;
    /// `FinstackError` (kind `validation`) for a malformed date or a year
    /// fraction that cannot be computed, negative or non-finite time, a query
    /// outside the knots under `"error"`, or non-positive interpolated CPI.
    /// @param t - Time from the curve base date in years (`number`) or an ISO-8601 target date (`"YYYY-MM-DD"`, converted with the curve day count).
    #[wasm_bindgen(js_name = cpi)]
    pub fn cpi(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number | string")] t: JsValue,
    ) -> Result<f64, JsValue> {
        match js_time_point(&t, "t")? {
            TimePoint::Years(t) => self.inner.cpi(t).map_err(to_js_err),
            TimePoint::Date(d) => self.inner.cpi_on_date(d).map_err(to_js_err),
        }
    }

    /// Annualised inflation rate between two times, by the CAGR formula
    /// `(I(t2) / I(t1))^(1 / (t2 - t1)) - 1` (Rust `InflationCurve::inflation_rate`).
    ///
    /// # Arguments
    ///
    /// * `t1` - Start time in years from the curve base date.
    /// * `t2` - End time in years, strictly greater than `t1`.
    ///
    /// @returns The annualised inflation rate as a decimal.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) for an invalid interval or non-positive CPI levels.
    /// @param t1 - Start time in years from the curve base date.
    /// @param t2 - End time in years, strictly greater than `t1`.
    #[wasm_bindgen(js_name = inflationRate)]
    pub fn inflation_rate(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] t1: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] t2: JsValue,
    ) -> Result<f64, JsValue> {
        self.inner
            .inflation_rate(js_f64(&t1, "t1")?, js_f64(&t2, "t2")?)
            .map_err(to_js_err)
    }

    /// Curve identifier (the `MarketContext` lookup key).
    #[wasm_bindgen(getter, js_name = id)]
    pub fn id(&self) -> String {
        self.inner.id().as_str().to_string()
    }

    /// Base date as an ISO-8601 string; knot times are measured from it.
    #[wasm_bindgen(getter, js_name = baseDate)]
    pub fn base_date(&self) -> String {
        date_to_iso(self.inner.base_date())
    }

    /// Day count that converts dates to curve time, such as `"act_365f"`.
    #[wasm_bindgen(getter, js_name = dayCount)]
    pub fn day_count(&self) -> String {
        self.inner.day_count().to_string()
    }

    /// Registered calendar identifier used by the source clock, or `undefined`.
    #[wasm_bindgen(getter, js_name = calendarId)]
    pub fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id().map(str::to_owned)
    }

    /// CPI index level at the base date.
    #[wasm_bindgen(getter, js_name = baseCpi)]
    pub fn base_cpi(&self) -> f64 {
        self.inner.base_cpi()
    }

    /// Pillar times in years from the base date, strictly increasing.
    #[wasm_bindgen(getter, js_name = knots)]
    pub fn knots(&self) -> Box<[f64]> {
        self.inner.knots().into()
    }

    /// CPI index level at each pillar, aligned with `knots`.
    #[wasm_bindgen(getter, js_name = cpiLevels)]
    pub fn cpi_levels(&self) -> Box<[f64]> {
        self.inner.cpi_levels().into()
    }

    /// Interpolation style between pillars, such as `"log_linear"`.
    #[wasm_bindgen(getter, js_name = interpStyle)]
    pub fn interp_style(&self) -> String {
        self.inner.interp_style().to_string()
    }

    /// Extrapolation policy beyond the pillar range, such as `"extend"`.
    #[wasm_bindgen(getter, js_name = extrapolation)]
    pub fn extrapolation(&self) -> String {
        self.inner.extrapolation().to_string()
    }
}

json_round_trip!(JsInflationCurve, InflationCurve, arc);

/// Forward price curve for commodities and other price-based assets, or a
/// volatility-index forward curve.
///
/// Built from `(time, level)` pillars; `time` is a year fraction from
/// `baseDate` and the level is an absolute price (or index points).
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const curve = new core.PriceCurve({
///   id: "WTI",
///   baseDate: "2025-01-02",
///   knots: [0.0, 72.0, 1.0, 70.5],
///   kind: "price", dayCount: "act_365f", interp: "linear", extrapolation: "flat",
/// });
/// curve.price(0.5); // forward price at 6 months
/// curve.kind; // "price"
/// ```
#[wasm_bindgen(js_name = PriceCurve)]
pub struct JsPriceCurve {
    pub(crate) inner: Arc<RustPriceCurve>,
}

/// Named constructor options for `PriceCurve`; unknown keys are rejected.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PriceCurveOptions {
    id: String,
    base_date: String,
    knots: Vec<f64>,
    kind: String,
    extrapolation: String,
    interp: String,
    day_count: String,
    calendar_id: Option<String>,
}

impl JsPriceCurve {
    pub(crate) fn from_inner(inner: Arc<RustPriceCurve>) -> Self {
        Self { inner }
    }

    fn build(options: PriceCurveOptions) -> Result<JsPriceCurve, JsValue> {
        RustPriceCurve::builder(options.id)
            .base_date(parse_iso_date(&options.base_date)?)
            .knots(flat_pairs(&options.knots, "knots")?)
            .kind(options.kind.parse::<PriceCurveKind>().map_err(to_js_err)?)
            .day_count(options.day_count.parse::<DayCount>().map_err(to_js_err)?)
            .calendar_id_opt(options.calendar_id)
            .interp(options.interp.parse::<InterpStyle>().map_err(to_js_err)?)
            .extrapolation(
                options
                    .extrapolation
                    .parse::<ExtrapolationPolicy>()
                    .map_err(to_js_err)?,
            )
            .build()
            .map(|curve| Self::from_inner(Arc::new(curve)))
            .map_err(to_js_err)
    }
}

#[wasm_bindgen(js_class = PriceCurve)]
impl JsPriceCurve {
    /// Construct a curve from dated price or index levels.
    /// @param id - Market identifier used to retrieve the source.
    /// @param baseDate - ISO-8601 date at zero curve time.
    /// @param points - Ordered ISO date and native level pairs on or after the base date.
    /// @param kind - "price" for signed price quotes or "vol_index" for nonnegative index levels.
    /// @param dayCount - Explicit date-to-time convention usable without a holiday calendar.
    /// @param interp - Level interpolation label; log-linear requires positive levels.
    /// @param extrapolation - Outside-grid handling such as "flat".
    /// @returns Curve using the supplied date pillars.
    /// @throws TypeError for mistyped data or FinstackError for invalid dates, policies or levels.
    #[wasm_bindgen(js_name = fromDates)]
    pub fn from_dates(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        #[wasm_bindgen(js_name = "baseDate", unchecked_param_type = "string")] base_date: JsValue,
        #[wasm_bindgen(unchecked_param_type = "readonly (readonly [string, number])[] | string")]
        points: JsValue,
        #[wasm_bindgen(unchecked_param_type = "\"price\" | \"vol_index\"")] kind: JsValue,
        #[wasm_bindgen(js_name = "dayCount", unchecked_param_type = "string")] day_count: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] interp: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] extrapolation: JsValue,
    ) -> Result<JsPriceCurve, JsValue> {
        let points = from_js_json_exact::<Vec<(String, f64)>>(&points, "points")?
            .into_iter()
            .map(|(date, level)| Ok((parse_iso_date(&date)?, level)))
            .collect::<Result<Vec<_>, JsValue>>()?;
        RustPriceCurve::from_dates(
            js_string(&id, "id")?,
            js_date(&base_date, "baseDate")?,
            &points,
            js_string(&kind, "kind")?
                .parse::<PriceCurveKind>()
                .map_err(to_js_err)?,
            js_string(&day_count, "dayCount")?
                .parse::<DayCount>()
                .map_err(to_js_err)?,
            js_string(&interp, "interp")?
                .parse::<InterpStyle>()
                .map_err(to_js_err)?,
            js_string(&extrapolation, "extrapolation")?
                .parse::<ExtrapolationPolicy>()
                .map_err(to_js_err)?,
        )
        .map(|curve| Self::from_inner(Arc::new(curve)))
        .map_err(to_js_err)
    }

    /// Return a copy of the curve with a uniform relative shift to its levels.
    /// @param pct - Signed percentage applied to each knot. A value of 2.5 increases each level by 2.5 percent.
    /// @returns The shocked curve, preserving knot times and curve conventions.
    /// @throws `TypeError` if pct is not numeric or `FinstackError` if it is
    /// non-finite, produces unrepresentable levels, or reverses a volatility index.
    #[wasm_bindgen(js_name = bumpParallelPct)]
    pub fn bump_parallel_pct(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] pct: JsValue,
    ) -> Result<JsPriceCurve, JsValue> {
        self.inner
            .bump_parallel_pct(js_f64(&pct, "pct")?)
            .map(|curve| Self::from_inner(Arc::new(curve)))
            .map_err(to_js_err)
    }

    /// Construct a price curve from named options.
    ///
    /// # Arguments
    ///
    /// * `options` - Source identifier, level family, ISO base date, ordered
    ///   numeric time/level pairs, day-count clock, interpolation and tail policy.
    ///   Bus/252 also requires a registered calendar identifier.
    ///
    /// @returns The constructed `PriceCurve`.
    /// @throws `TypeError` (kind `invalid_type`) if `options` is not a JSON
    /// string or plain object; `FinstackError` (kind `validation`) for an
    /// unknown, missing or mistyped key, an odd-length `knots`, a malformed
    /// date, an unknown kind/day-count/interpolation/extrapolation name, or
    /// levels the curve builder rejects. An unknown calendar raises a
    /// `FinstackError` with kind `not_found`.
    /// @param options - Source ID, kind ("price" or "vol_index"), ISO baseDate, a flat array alternating nonnegative times and native levels, dayCount, interp and extrapolation. Supply calendarId for Bus/252.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "PriceCurveOptions | string")] options: JsValue,
    ) -> Result<JsPriceCurve, JsValue> {
        Self::build(from_js_json(&options, "options")?)
    }

    /// Forward price at year fraction `t` or on an ISO-8601 date (Rust
    /// `PriceCurve::price` / `PriceCurve::price_on_date`).
    ///
    /// # Arguments
    ///
    /// * `t` - Finite nonnegative years from the curve base date, or an ISO-8601
    ///   date on or after it, converted with the source clock.
    ///
    /// @returns The interpolated forward price (or index level).
    /// @throws `TypeError` if `t` is neither a number nor a string;
    /// `FinstackError` (kind `validation`) for an invalid or pre-base time or
    /// date, forbidden extrapolation, a non-finite result or an inadmissible level.
    /// @param t - Finite nonnegative years from baseDate, or an ISO-8601 date on or after baseDate under the source clock.
    #[wasm_bindgen(js_name = price)]
    pub fn price(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number | string")] t: JsValue,
    ) -> Result<f64, JsValue> {
        match js_time_point(&t, "t")? {
            TimePoint::Years(t) => self.inner.price(t).map_err(to_js_err),
            TimePoint::Date(d) => self.inner.price_on_date(d).map_err(to_js_err),
        }
    }

    /// Curve identifier (the `MarketContext` lookup key).
    #[wasm_bindgen(getter, js_name = id)]
    pub fn id(&self) -> String {
        self.inner.id().as_str().to_string()
    }

    /// Base date as an ISO-8601 string; knot times are measured from it.
    #[wasm_bindgen(getter, js_name = baseDate)]
    pub fn base_date(&self) -> String {
        date_to_iso(self.inner.base_date())
    }

    /// Level family of the curve: `"price"` or `"vol_index"`.
    #[wasm_bindgen(getter, js_name = kind)]
    pub fn kind(&self) -> String {
        self.inner.kind().to_string()
    }

    /// Registered calendar identifier used by the source clock, or `undefined`.
    #[wasm_bindgen(getter, js_name = calendarId)]
    pub fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id().map(str::to_owned)
    }

    /// Pillar times in years from the base date, strictly increasing.
    #[wasm_bindgen(getter, js_name = knots)]
    pub fn knots(&self) -> Box<[f64]> {
        self.inner.knots().into()
    }

    /// Forward level at each pillar, aligned with `knots`.
    #[wasm_bindgen(getter, js_name = prices)]
    pub fn prices(&self) -> Box<[f64]> {
        self.inner.prices().into()
    }

    /// Day count that converts dates to curve time, such as `"act_365f"`.
    #[wasm_bindgen(getter, js_name = dayCount)]
    pub fn day_count(&self) -> String {
        self.inner.day_count().to_string()
    }

    /// Interpolation style between pillars, such as `"linear"`.
    #[wasm_bindgen(getter, js_name = interpStyle)]
    pub fn interp_style(&self) -> String {
        self.inner.interp_style().to_string()
    }

    /// Extrapolation policy beyond the pillar range, such as `"flat"`.
    #[wasm_bindgen(getter, js_name = extrapolation)]
    pub fn extrapolation(&self) -> String {
        self.inner.extrapolation().to_string()
    }
}

json_round_trip!(JsPriceCurve, PriceCurve, arc);

/// Base-correlation curve for credit tranche pricing: correlation as a
/// function of the detachment point.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const curve = new core.BaseCorrelationCurve("CDX-IG", [3.0, 0.25, 7.0, 0.45]);
/// curve.getCorrelation(5.0); // 0.35
/// ```
#[wasm_bindgen(js_name = BaseCorrelationCurve)]
pub struct JsBaseCorrelationCurve {
    pub(crate) inner: Arc<RustBaseCorrelationCurve>,
}

impl JsBaseCorrelationCurve {
    pub(crate) fn from_inner(inner: Arc<RustBaseCorrelationCurve>) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen(js_class = BaseCorrelationCurve)]
impl JsBaseCorrelationCurve {
    /// Construct a base-correlation curve (Rust `BaseCorrelationCurve::builder`).
    ///
    /// # Arguments
    ///
    /// * `id` - Curve identifier, the `MarketContext` lookup key.
    /// * `knots` - Flat `[d0, rho0, d1, rho1, …]` array: detachment points in
    ///   percent (`3.0` is 3%), strictly increasing, and base correlations as
    ///   decimals in `[0, 1]`.
    ///
    /// @returns The validated `BaseCorrelationCurve`.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) for an odd-length or too-short `knots`, unsorted
    /// detachment points, or correlations the builder rejects.
    /// @param id - Curve identifier, the `MarketContext` lookup key.
    /// @param knots - Flat `[d0, rho0, d1, rho1, …]` array: detachment points in percent (`3.0` is 3%), strictly increasing, and base correlations as decimals in `[0, 1]`.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] knots: JsValue,
    ) -> Result<JsBaseCorrelationCurve, JsValue> {
        let id = js_string(&id, "id")?;
        let knots = flat_pairs(&js_f64_seq(&knots, "knots")?, "knots")?;
        RustBaseCorrelationCurve::builder(id)
            .knots(knots)
            .build()
            .map(|curve| Self::from_inner(Arc::new(curve)))
            .map_err(to_js_err)
    }

    /// Base correlation at a detachment point (Rust `BaseCorrelationCurve::get_correlation`).
    ///
    /// # Arguments
    ///
    /// * `detachment_pct` - Tranche detachment point in percent (`7.0` is 7%).
    ///
    /// @returns The interpolated correlation as a decimal; flat beyond the
    /// first and last pillar.
    /// @throws `TypeError` for a nonnumeric input, or `FinstackError` (kind
    /// `validation`) for a nonfinite detachment or a value outside `[0, 100]`.
    /// @param detachmentPct - Tranche detachment point in percent (`7.0` is 7%).
    #[wasm_bindgen(js_name = getCorrelation)]
    pub fn get_correlation(
        &self,
        #[wasm_bindgen(js_name = "detachmentPct", unchecked_param_type = "number")] detachment_pct: JsValue,
    ) -> Result<f64, JsValue> {
        self.inner
            .get_correlation(js_f64(&detachment_pct, "detachmentPct")?)
            .map_err(to_js_err)
    }

    /// Curve identifier (the `MarketContext` lookup key).
    #[wasm_bindgen(getter, js_name = id)]
    pub fn id(&self) -> String {
        self.inner.id().as_str().to_string()
    }

    /// Detachment points in percent, strictly increasing.
    #[wasm_bindgen(getter, js_name = detachmentPoints)]
    pub fn detachment_points(&self) -> Box<[f64]> {
        self.inner.detachment_points().into()
    }

    /// Base correlation at each detachment point, aligned with `detachmentPoints`.
    #[wasm_bindgen(getter, js_name = correlations)]
    pub fn correlations(&self) -> Box<[f64]> {
        self.inner.correlations().into()
    }

    /// Interpolation style between detachment points, such as `"linear"`.
    #[wasm_bindgen(getter, js_name = interpStyle)]
    pub fn interp_style(&self) -> String {
        self.inner.interp_style().to_string()
    }

    /// Extrapolation policy beyond the pillar range, such as `"flat"`.
    #[wasm_bindgen(getter, js_name = extrapolation)]
    pub fn extrapolation(&self) -> String {
        self.inner.extrapolation().to_string()
    }
}

json_round_trip!(JsBaseCorrelationCurve, BaseCorrelationCurve, arc);

/// Market data of one credit index: constituent count, recovery, the index
/// hazard curve and its base-correlation curve.
///
/// The constructor builds homogeneous data. Issuer-level curves, recoveries
/// and weights (heterogeneous tranche pricing) arrive through
/// `MarketContext.fromJson` (`issuer_credit_curve_ids`, `issuer_recovery_rates`,
/// `issuer_weights`) and are read with the `getIssuer*` accessors.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const hazard = core.HazardCurve.flat("CDX-IG-HZD", "2025-01-02", 0.01, 0.4, "act_365f");
/// const correlation = new core.BaseCorrelationCurve("CDX-IG", [3.0, 0.25, 7.0, 0.45]);
/// const index = new core.CreditIndexData(125, 0.4, hazard, correlation);
/// index.numConstituents; // 125
/// ```
#[wasm_bindgen(js_name = CreditIndexData)]
pub struct JsCreditIndexData {
    pub(crate) inner: Arc<RustCreditIndexData>,
}

impl JsCreditIndexData {
    pub(crate) fn from_inner(inner: Arc<RustCreditIndexData>) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen(js_class = CreditIndexData)]
impl JsCreditIndexData {
    /// Assemble credit-index market data (Rust `CreditIndexData::builder`).
    ///
    /// # Arguments
    ///
    /// * `num_constituents` - Number of names in the index (for example `125`
    ///   for CDX IG), an integer in `1..=65535`.
    /// * `recovery_rate` - Index-level recovery on default as a decimal in `[0, 1]`.
    /// * `index_credit_curve` - Hazard curve of the index as a whole.
    /// * `base_correlation_curve` - Base-correlation curve used for tranches.
    ///
    /// @returns The validated `CreditIndexData`; the curves are shared, not copied.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) for a zero constituent count or a recovery outside `[0, 1]`.
    /// @param numConstituents - Number of names in the index (for example `125` for CDX IG), an integer in `1..=65535`.
    /// @param recoveryRate - Index-level recovery on default as a decimal in `[0, 1]`.
    /// @param indexCreditCurve - Hazard curve of the index as a whole.
    /// @param baseCorrelationCurve - Base-correlation curve used for tranches.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(js_name = "numConstituents", unchecked_param_type = "number")]
        num_constituents: JsValue,
        #[wasm_bindgen(js_name = "recoveryRate", unchecked_param_type = "number")]
        recovery_rate: JsValue,
        #[wasm_bindgen(js_name = "indexCreditCurve")] index_credit_curve: &JsHazardCurve,
        #[wasm_bindgen(js_name = "baseCorrelationCurve")]
        base_correlation_curve: &JsBaseCorrelationCurve,
    ) -> Result<JsCreditIndexData, JsValue> {
        RustCreditIndexData::builder()
            .num_constituents(js_uint(&num_constituents, "numConstituents")?)
            .recovery_rate(js_f64(&recovery_rate, "recoveryRate")?)
            .index_credit_curve(Arc::clone(&index_credit_curve.inner))
            .base_correlation_curve(Arc::clone(&base_correlation_curve.inner))
            .build()
            .map(|data| Self::from_inner(Arc::new(data)))
            .map_err(to_js_err)
    }

    /// Number of names in the index.
    #[wasm_bindgen(getter, js_name = numConstituents)]
    pub fn num_constituents(&self) -> u16 {
        self.inner.num_constituents()
    }

    /// Index-level recovery rate on default, as a decimal.
    #[wasm_bindgen(getter, js_name = recoveryRate)]
    pub fn recovery_rate(&self) -> f64 {
        self.inner.recovery_rate()
    }

    /// Hazard curve of the index as a whole, as a `HazardCurve`.
    #[wasm_bindgen(getter, js_name = indexCreditCurve)]
    pub fn index_credit_curve(&self) -> JsHazardCurve {
        JsHazardCurve {
            inner: Arc::clone(self.inner.index_credit_curve()),
        }
    }

    /// Base-correlation curve used for tranche pricing, as a `BaseCorrelationCurve`.
    #[wasm_bindgen(getter, js_name = baseCorrelationCurve)]
    pub fn base_correlation_curve(&self) -> JsBaseCorrelationCurve {
        JsBaseCorrelationCurve::from_inner(Arc::clone(self.inner.base_correlation_curve()))
    }

    /// Hazard curve of one issuer (Rust `CreditIndexData::get_issuer_curve`).
    ///
    /// # Arguments
    ///
    /// * `issuer_id` - Issuer identifier as stored in the index data (ticker,
    ///   CUSIP, ...).
    ///
    /// @returns The hazard curve of the named pool member.
    /// @throws `TypeError` if `issuerId` is not a string. A lookup error is thrown when the named roster does not contain the issuer.
    /// @param issuerId - Issuer identifier as stored in the index data (ticker, CUSIP, ...).
    #[wasm_bindgen(js_name = getIssuerCurve)]
    pub fn get_issuer_curve(
        &self,
        #[wasm_bindgen(js_name = "issuerId", unchecked_param_type = "string")] issuer_id: JsValue,
    ) -> Result<JsHazardCurve, JsValue> {
        let issuer_id = js_string(&issuer_id, "issuerId")?;
        Ok(JsHazardCurve {
            inner: Arc::new(
                self.inner
                    .get_issuer_curve(&issuer_id)
                    .map_err(to_js_err)?
                    .clone(),
            ),
        })
    }

    /// Whether issuer-level curves are present (heterogeneous pricing mode).
    ///
    /// @returns `true` when the data carries per-issuer hazard curves.
    #[wasm_bindgen(js_name = hasIssuerCurves)]
    pub fn has_issuer_curves(&self) -> bool {
        self.inner.has_issuer_curves()
    }

    /// Issuer identifiers with their own curve.
    ///
    /// @returns Identifiers in sorted order; empty for homogeneous data.
    #[wasm_bindgen(js_name = issuerIds)]
    pub fn issuer_ids(&self) -> Vec<String> {
        self.inner.issuer_ids()
    }

    /// Recovery rate of one issuer (Rust `CreditIndexData::get_issuer_recovery`).
    ///
    /// # Arguments
    ///
    /// * `issuer_id` - Issuer identifier as stored in the index data.
    ///
    /// @returns The issuer's recovery as a decimal, or `recoveryRate` when
    /// none was supplied for `issuerId`.
    /// @throws `TypeError` if `issuerId` is not a string. A lookup error is thrown when the named roster does not contain the issuer.
    /// @param issuerId - Issuer identifier as stored in the index data.
    #[wasm_bindgen(js_name = getIssuerRecovery)]
    pub fn get_issuer_recovery(
        &self,
        #[wasm_bindgen(js_name = "issuerId", unchecked_param_type = "string")] issuer_id: JsValue,
    ) -> Result<f64, JsValue> {
        let issuer_id = js_string(&issuer_id, "issuerId")?;
        self.inner
            .get_issuer_recovery(&issuer_id)
            .map_err(to_js_err)
    }

    /// Notional weight of one issuer (Rust `CreditIndexData::get_issuer_weight`).
    ///
    /// # Arguments
    ///
    /// * `issuer_id` - Issuer identifier as stored in the index data.
    ///
    /// @returns The issuer's weight as a decimal fraction, or
    /// `1 / numConstituents` when none was supplied for `issuerId`.
    /// @throws `TypeError` if `issuerId` is not a string. A lookup error is thrown when the named roster does not contain the issuer.
    /// @param issuerId - Issuer identifier as stored in the index data.
    #[wasm_bindgen(js_name = getIssuerWeight)]
    pub fn get_issuer_weight(
        &self,
        #[wasm_bindgen(js_name = "issuerId", unchecked_param_type = "string")] issuer_id: JsValue,
    ) -> Result<f64, JsValue> {
        let issuer_id = js_string(&issuer_id, "issuerId")?;
        self.inner.get_issuer_weight(&issuer_id).map_err(to_js_err)
    }
}
