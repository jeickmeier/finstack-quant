//! WASM binding for `finstack_quant_core::market_data::term_structures::ForwardCurve`.

use crate::utils::input::{from_js_json, js_f64, js_f64_seq, js_opt_string, js_string};
use std::sync::Arc;

use crate::utils::wire::js_date;
use crate::utils::{date_to_iso, parse_iso_date, to_js_err};
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::market_data::term_structures::ForwardCurve as RustForwardCurve;
use finstack_quant_core::math::Compounding;
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

use crate::api::core::curve_args::{TimePoint, flat_pairs, js_time_point};
use crate::api::core::market_discount::JsDiscountCurve;

/// Named constructor options for `ForwardCurve`; unknown keys are rejected.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ForwardCurveOptions {
    id: String,
    base_date: String,
    knots: Vec<f64>,
    interp: String,
    extrapolation: String,
    day_count: String,
    #[serde(default)]
    calendar_id: Option<String>,
    #[serde(default)]
    index_tenor: Option<String>,
}

/// Projection-factor curve for one floating-rate index.
///
/// Stores strictly positive projection factors `P_x(t)` at knot times in
/// years from `baseDate`, with `P_x(0) = 1`. The growth `P_x(t1) / P_x(t2)`
/// over an index period drives its projected fixing; `rateBetween` converts
/// it to a simple rate on an explicit contractual accrual fraction. All curve
/// conventions (`interp`, `extrapolation`, and `dayCount`) must be specified
/// explicitly.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const curve = new core.ForwardCurve({
///   id: "USD-SOFR-3M",
///   baseDate: "2026-01-02",
///   knots: [0, 1, 1, Math.exp(-0.04)],
///   interp: "log_linear",
///   extrapolation: "extend",
///   dayCount: "act_360",
///   indexTenor: "3M",
/// });
/// console.log(curve.rateBetween(0, 0.25, 0.25));
/// ```
#[wasm_bindgen(js_name = ForwardCurve)]
pub struct JsForwardCurve {
    pub(crate) inner: Arc<RustForwardCurve>,
}

impl JsForwardCurve {
    pub(crate) fn from_inner(curve: RustForwardCurve) -> Self {
        Self {
            inner: Arc::new(curve),
        }
    }

    fn build(options: ForwardCurveOptions) -> Result<JsForwardCurve, JsValue> {
        let base = parse_iso_date(&options.base_date)?;
        let pairs = flat_pairs(&options.knots, "knots")?;
        let interp = options.interp.parse::<InterpStyle>().map_err(to_js_err)?;
        let extrapolation = options
            .extrapolation
            .parse::<ExtrapolationPolicy>()
            .map_err(to_js_err)?;
        let day_count = options.day_count.parse::<DayCount>().map_err(to_js_err)?;
        let index_tenor = options
            .index_tenor
            .as_deref()
            .map(Tenor::parse)
            .transpose()
            .map_err(to_js_err)?;

        RustForwardCurve::builder(options.id)
            .base_date(base)
            .knots(pairs)
            .calendar_id_opt(options.calendar_id)
            .index_tenor_opt(index_tenor)
            .interp(interp)
            .extrapolation(extrapolation)
            .day_count(day_count)
            .build()
            .map(Self::from_inner)
            .map_err(to_js_err)
    }
}

#[wasm_bindgen(js_class = ForwardCurve)]
impl JsForwardCurve {
    /// Construct a forward curve from named options.
    ///
    /// # Arguments
    ///
    /// * `options` - ForwardCurveOptions object (or its JSON text) with: `id`
    ///   (curve identifier, the `MarketContext` lookup key); `baseDate`
    ///   (ISO-8601 `"YYYY-MM-DD"`; knot times are year fractions from it under
    ///   `dayCount`); `knots` (flat `[t0, p0, t1, p1, …]` array or typed array
    ///   of strictly ascending times in years and strictly positive projection
    ///   factors; `(0, 1)` is prepended when absent and a supplied `t = 0`
    ///   factor must be `1`); `interp` (`"log_linear"`, `"linear"`,
    ///   `"monotone_convex"`, `"cubic_hermite"`,
    ///   `"piecewise_quadratic_forward"`); `extrapolation` (`"extend"`,
    ///   `"flat"`, or `"error"`, which rejects queries outside the pillar
    ///   range); `dayCount` (converts dates to curve time; not inferred from
    ///   the ID); optional `calendarId` (registered calendar ID required for
    ///   `"bus_252"`); and optional `indexTenor` (accrual tenor of the projected
    ///   index such as `"3M"`, required by term-rate consumers). Unknown keys
    ///   are rejected.
    ///
    /// @returns The constructed `ForwardCurve`.
    /// @throws `TypeError` (kind `invalid_type`) if `options` is not a JSON
    /// string or plain object; `FinstackError` (kind `validation`) for an
    /// unknown, missing, or mistyped key, an odd `knots` length, a malformed
    /// date or tenor, an unknown interpolation/extrapolation/day-count name,
    /// or non-positive, unsorted or duplicated knots.
    /// @param options - ForwardCurveOptions object (or its JSON text) with `id`, `baseDate`, `knots` (flat time/projection-factor pairs), `interp`, `extrapolation`, `dayCount`, optional `calendarId`, and optional `indexTenor`. Unknown keys are rejected.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "ForwardCurveOptions | string")] options: JsValue,
    ) -> Result<JsForwardCurve, JsValue> {
        Self::build(from_js_json(&options, "options")?)
    }

    /// Construct a flat continuously-compounded projection curve (Rust
    /// `ForwardCurve::flat`).
    ///
    /// # Arguments
    ///
    /// * `id` - Curve identifier stored on the curve.
    /// * `base_date` - ISO-8601 valuation date anchoring `t = 0`.
    /// * `continuous_rate` - Continuously-compounded projection zero rate as a
    ///   decimal (`0.04` is 4%).
    /// * `day_count` - Explicit curve day-count convention label (for example
    ///   `"act_360"`).
    ///
    /// @returns A log-linear `ForwardCurve` with `df(t) == exp(-continuousRate * t)`, `"extend"` tails and no `indexTenor`.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) if `baseDate` is not an ISO date, `dayCount` is unknown or
    /// requires a calendar, or the rate is non-finite or its one-year factor
    /// overflows or underflows.
    /// @param id - Curve identifier stored on the curve.
    /// @param baseDate - ISO-8601 valuation date anchoring `t = 0`.
    /// @param continuousRate - Continuously-compounded projection zero rate as a decimal (`0.04` is 4%).
    /// @param dayCount - Explicit curve day-count convention label (for example `"act_360"`).
    #[wasm_bindgen(js_name = flat)]
    pub fn flat(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        #[wasm_bindgen(js_name = "baseDate", unchecked_param_type = "string")] base_date: JsValue,
        #[wasm_bindgen(js_name = "continuousRate", unchecked_param_type = "number")]
        continuous_rate: JsValue,
        #[wasm_bindgen(js_name = "dayCount", unchecked_param_type = "string")] day_count: JsValue,
    ) -> Result<JsForwardCurve, JsValue> {
        let id = js_string(&id, "id")?;
        let base_date = js_date(&base_date, "baseDate")?;
        let continuous_rate = js_f64(&continuous_rate, "continuousRate")?;
        let day_count = js_string(&day_count, "dayCount")?
            .parse::<DayCount>()
            .map_err(to_js_err)?;
        RustForwardCurve::flat(id, base_date, continuous_rate, day_count)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Promote a discount curve into a projection curve without refitting
    /// (Rust `ForwardCurve::from_discount_curve`).
    ///
    /// The forward curve reuses the discount curve's factors, interpolator,
    /// day count, calendar, tail policy and any retained shock, so `df(t)`
    /// matches `discount.df(t)` everywhere.
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier of the resulting forward curve.
    /// * `discount` - Source `DiscountCurve` whose factor state is copied; its
    ///   calibration recipe is dropped.
    /// * `index_tenor` - Optional accrual tenor of the projected index (for
    ///   example `"3M"`); omitted leaves the curve without index-period metadata.
    ///
    /// @returns A `ForwardCurve` sharing `discount`'s factor function.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) for a malformed `indexTenor`.
    /// @param id - Identifier of the resulting forward curve.
    /// @param discount - Source `DiscountCurve` whose factor state is copied; its calibration recipe is dropped.
    /// @param indexTenor - Optional accrual tenor of the projected index (for example `"3M"`); omitted leaves the curve without index-period metadata.
    #[wasm_bindgen(js_name = fromDiscountCurve)]
    pub fn from_discount_curve(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        discount: &JsDiscountCurve,
        #[wasm_bindgen(
            js_name = "indexTenor",
            unchecked_optional_param_type = "string | null"
        )]
        index_tenor: Option<JsValue>,
    ) -> Result<JsForwardCurve, JsValue> {
        let id = js_string(&id, "id")?;
        let index_tenor = js_opt_string(index_tenor.as_ref(), "indexTenor")?
            .as_deref()
            .map(Tenor::parse)
            .transpose()
            .map_err(to_js_err)?;
        Ok(Self::from_inner(RustForwardCurve::from_discount_curve(
            id,
            &discount.inner,
            index_tenor,
        )))
    }

    /// Construct a forward curve from zero rates (Rust `ForwardCurve::from_zero_rates`).
    ///
    /// # Arguments
    ///
    /// * `id` - Curve identifier stored on the curve.
    /// * `base_date` - ISO-8601 valuation date anchoring `t = 0`.
    /// * `points` - Flat `[t0, z0, t1, z1, …]` array: times in years and
    ///   zero rates as decimals (`0.04` is 4%), with strictly increasing times.
    /// * `compounding` - Compounding of the zero rates: `"continuous"`,
    ///   `"simple"`, `"periodic(1)"`, `"semi_annual"`, `"quarterly"` or `"monthly"`.
    /// * `day_count` - Explicit curve day-count convention label.
    /// * `interp` - Explicit interpolation style label.
    /// * `extrapolation` - Explicit tail extrapolation policy (`"flat"`, `"extend"`, or `"error"`).
    ///
    /// @returns Forward curve whose projection factors reproduce every zero rate.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind `validation`)
    /// for empty/odd/unsorted `points`, a malformed date, an unknown label, or invalid zero rates.
    /// @param id - Curve identifier stored on the curve.
    /// @param baseDate - ISO-8601 valuation date anchoring `t = 0`.
    /// @param points - Flat `[t0, z0, t1, z1, …]` array: times in years and decimal zero rates.
    /// @param compounding - Compounding convention label for the zero rates.
    /// @param dayCount - Explicit curve day-count convention label.
    /// @param interp - Explicit interpolation style label.
    /// @param extrapolation - Explicit tail extrapolation policy label.
    #[wasm_bindgen(js_name = fromZeroRates)]
    pub fn from_zero_rates(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        #[wasm_bindgen(js_name = "baseDate", unchecked_param_type = "string")] base_date: JsValue,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] points: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] compounding: JsValue,
        #[wasm_bindgen(js_name = "dayCount", unchecked_param_type = "string")] day_count: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] interp: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] extrapolation: JsValue,
    ) -> Result<JsForwardCurve, JsValue> {
        let id = js_string(&id, "id")?;
        let base_date = js_date(&base_date, "baseDate")?;
        let points = flat_pairs(&js_f64_seq(&points, "points")?, "points")?;
        let compounding = js_string(&compounding, "compounding")?
            .parse::<Compounding>()
            .map_err(to_js_err)?;
        let day_count = js_string(&day_count, "dayCount")?
            .parse::<DayCount>()
            .map_err(to_js_err)?;
        let interp = js_string(&interp, "interp")?
            .parse::<InterpStyle>()
            .map_err(to_js_err)?;
        let extrapolation = js_string(&extrapolation, "extrapolation")?
            .parse::<ExtrapolationPolicy>()
            .map_err(to_js_err)?;
        RustForwardCurve::from_zero_rates(
            id,
            base_date,
            &points,
            compounding,
            day_count,
            interp,
            extrapolation,
        )
        .map(Self::from_inner)
        .map_err(to_js_err)
    }

    /// Construct a forward curve from dated projection factors (Rust `ForwardCurve::from_dates`).
    ///
    /// # Arguments
    ///
    /// * `id` - Curve identifier stored on the curve.
    /// * `base_date` - ISO-8601 valuation date anchoring `t = 0`.
    /// * `points` - Array of `[isoDate, factor]` pairs with strictly increasing dates on or after `baseDate` and strictly positive projection factors.
    /// * `day_count` - Explicit day count that converts each date to curve time.
    /// * `interp` - Explicit interpolation style label.
    /// * `extrapolation` - Explicit tail extrapolation policy label.
    ///
    /// @returns Curve with one pillar per dated point.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind `validation`) for empty `points`, a malformed date, an unknown label, or invalid projection factors.
    /// @param id - Curve identifier stored on the curve.
    /// @param baseDate - ISO-8601 valuation date anchoring `t = 0`.
    /// @param points - Array of `[isoDate, factor]` pairs with strictly increasing dates and positive factors.
    /// @param dayCount - Explicit day count converting dates to curve time.
    /// @param interp - Explicit interpolation style label.
    /// @param extrapolation - Explicit tail extrapolation policy label.
    #[wasm_bindgen(js_name = fromDates)]
    pub fn from_dates(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        #[wasm_bindgen(js_name = "baseDate", unchecked_param_type = "string")] base_date: JsValue,
        #[wasm_bindgen(unchecked_param_type = "readonly (readonly [string, number])[] | string")]
        points: JsValue,
        #[wasm_bindgen(js_name = "dayCount", unchecked_param_type = "string")] day_count: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] interp: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] extrapolation: JsValue,
    ) -> Result<JsForwardCurve, JsValue> {
        let id = js_string(&id, "id")?;
        let base_date = js_date(&base_date, "baseDate")?;
        let points = from_js_json::<Vec<(String, f64)>>(&points, "points")?
            .into_iter()
            .map(|(date, factor)| Ok((parse_iso_date(&date)?, factor)))
            .collect::<Result<Vec<_>, JsValue>>()?;
        let day_count = js_string(&day_count, "dayCount")?
            .parse::<DayCount>()
            .map_err(to_js_err)?;
        let interp = js_string(&interp, "interp")?
            .parse::<InterpStyle>()
            .map_err(to_js_err)?;
        let extrapolation = js_string(&extrapolation, "extrapolation")?
            .parse::<ExtrapolationPolicy>()
            .map_err(to_js_err)?;
        RustForwardCurve::from_dates(id, base_date, &points, day_count, interp, extrapolation)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Projection factor `P_x(t)` at year fraction `t` or on an ISO-8601 date.
    ///
    /// # Arguments
    ///
    /// * `t` - Time from `baseDate` in years (`number`) or an ISO-8601 target
    ///   date (`"YYYY-MM-DD"`, converted with the curve day count); times
    ///   before the base date follow the tail policy.
    ///
    /// @returns Unitless, strictly positive projection factor (`1` at `t = 0`).
    /// @throws `TypeError` if `t` is neither a number nor a string;
    /// `FinstackError` (kind `validation`) for a malformed date, a non-finite
    /// time, an out-of-range `"error"` tail query, or a year fraction that
    /// cannot be computed.
    /// @param t - Time from `baseDate` in years (`number`) or an ISO-8601 target date (`"YYYY-MM-DD"`, converted with the curve day count); times before the base date follow the tail policy.
    pub fn df(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number | string")] t: JsValue,
    ) -> Result<f64, JsValue> {
        match js_time_point(&t, "t")? {
            TimePoint::Years(t) => self.inner.df(t).map_err(to_js_err),
            TimePoint::Date(d) => self.inner.df_on_date(d).map_err(to_js_err),
        }
    }

    /// Relative projection factor between two dates, `P_x(toDate) / P_x(fromDate)`
    /// (Rust `ForwardCurve::df_between_dates`).
    ///
    /// # Arguments
    ///
    /// * `from_date` - ISO-8601 start date of the projection interval.
    /// * `to_date` - ISO-8601 end date of the projection interval.
    ///
    /// @returns Unitless relative projection factor (`1` when equal).
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind `validation`)
    /// for a malformed date or factor admission failure.
    /// @param fromDate - ISO-8601 start date of the projection interval.
    /// @param toDate - ISO-8601 end date of the projection interval.
    #[wasm_bindgen(js_name = dfBetweenDates)]
    pub fn df_between_dates(
        &self,
        #[wasm_bindgen(js_name = "fromDate", unchecked_param_type = "string")] from_date: JsValue,
        #[wasm_bindgen(js_name = "toDate", unchecked_param_type = "string")] to_date: JsValue,
    ) -> Result<f64, JsValue> {
        let from_date = js_date(&from_date, "fromDate")?;
        let to_date = js_date(&to_date, "toDate")?;
        self.inner
            .df_between_dates(from_date, to_date)
            .map_err(to_js_err)
    }

    /// Simple forward rate over `[t1, t2]` on a contractual accrual fraction.
    ///
    /// Evaluates `(P_x(t1) / P_x(t2) - 1) / accrualYearFraction`. The interval
    /// endpoints are curve times; the accrual fraction is the index's own
    /// day-count fraction for the same dated period, which may differ from
    /// `t2 - t1` when the index and curve conventions differ.
    ///
    /// # Arguments
    ///
    /// * `t1` - Start year fraction from `baseDate` on the curve day count.
    /// * `t2` - End year fraction on the same basis; finite and greater than `t1`.
    /// * `accrual_year_fraction` - Strictly positive accrual year fraction of
    ///   the index period under its contractual day count (for example `91 / 360`).
    ///
    /// @returns Simple forward rate as a decimal, annualized on `accrualYearFraction`.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) if the interval is empty, reversed or non-finite, the
    /// accrual fraction is not finite and positive, or a factor or the rate
    /// cannot be computed.
    /// @param t1 - Start year fraction from `baseDate` on the curve day count.
    /// @param t2 - End year fraction on the same basis; finite and greater than `t1`.
    /// @param accrualYearFraction - Strictly positive accrual year fraction of the index period under its contractual day count (for example `91 / 360`).
    #[wasm_bindgen(js_name = rateBetween)]
    pub fn rate_between(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] t1: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] t2: JsValue,
        #[wasm_bindgen(js_name = "accrualYearFraction", unchecked_param_type = "number")]
        accrual_year_fraction: JsValue,
    ) -> Result<f64, JsValue> {
        let t1 = js_f64(&t1, "t1")?;
        let t2 = js_f64(&t2, "t2")?;
        let accrual_year_fraction = js_f64(&accrual_year_fraction, "accrualYearFraction")?;
        self.inner
            .rate_between(t1, t2, accrual_year_fraction)
            .map_err(to_js_err)
    }

    /// Simple forward rate over date interval `[start, end]` on a contractual accrual fraction
    /// (Rust `ForwardCurve::rate_between_dates`).
    ///
    /// # Arguments
    ///
    /// * `start` - ISO-8601 period start date (`start < end`).
    /// * `end` - ISO-8601 exclusive period end date strictly after `start`.
    /// * `accrual_year_fraction` - Strictly positive accrual year fraction of the index period under its contractual day count.
    ///
    /// @returns Simple forward rate as a decimal, annualized on `accrualYearFraction`.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind `validation`)
    /// if `end <= start`, mapped coordinates reverse order, `accrualYearFraction` is not finite and positive, or factors/rate fail admission.
    /// @param start - ISO-8601 period start date.
    /// @param end - ISO-8601 exclusive period end date strictly after `start`.
    /// @param accrualYearFraction - Strictly positive accrual year fraction under contractual convention.
    #[wasm_bindgen(js_name = rateBetweenDates)]
    pub fn rate_between_dates(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] start: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] end: JsValue,
        #[wasm_bindgen(js_name = "accrualYearFraction", unchecked_param_type = "number")]
        accrual_year_fraction: JsValue,
    ) -> Result<f64, JsValue> {
        let start = js_date(&start, "start")?;
        let end = js_date(&end, "end")?;
        let accrual_year_fraction = js_f64(&accrual_year_fraction, "accrualYearFraction")?;
        self.inner
            .rate_between_dates(start, end, accrual_year_fraction)
            .map_err(to_js_err)
    }

    /// Instantaneous projection forward `-P_x'(t) / P_x(t)` at year fraction `t`.
    ///
    /// # Arguments
    ///
    /// * `t` - Curve time in years from `baseDate`; at a knot a one-sided
    ///   derivative of the interpolator is used.
    ///
    /// @returns Continuously-compounded instantaneous forward rate as a decimal.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) if `t` is non-finite, the factor at `t` cannot be
    /// evaluated, or the derivative overflows.
    /// @param t - Curve time in years from `baseDate`; at a knot a one-sided derivative of the interpolator is used.
    #[wasm_bindgen(js_name = instantaneousForward)]
    pub fn instantaneous_forward(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] t: JsValue,
    ) -> Result<f64, JsValue> {
        let t = js_f64(&t, "t")?;
        self.inner.instantaneous_forward(t).map_err(to_js_err)
    }

    /// Curve identifier.
    #[wasm_bindgen(getter, js_name = id)]
    pub fn id(&self) -> String {
        self.inner.id().as_str().to_string()
    }

    /// Base date as ISO string.
    #[wasm_bindgen(getter, js_name = baseDate)]
    pub fn base_date(&self) -> String {
        date_to_iso(self.inner.base_date())
    }

    /// Accrual tenor of the projected index (for example `"3M"`), or `undefined` when not recorded.
    #[wasm_bindgen(getter, js_name = indexTenor)]
    pub fn index_tenor(&self) -> Option<String> {
        self.inner.index_tenor().map(|tenor| tenor.to_string())
    }

    /// Knot times in years from `baseDate`, ascending and starting at `0`.
    #[wasm_bindgen(getter, js_name = knots)]
    pub fn knots(&self) -> Box<[f64]> {
        self.inner.knots().into()
    }

    /// Projection factors at the knots, aligned with `knots` (`1` at `t = 0`).
    #[wasm_bindgen(getter, js_name = factors)]
    pub fn factors(&self) -> Box<[f64]> {
        self.inner.factors().into()
    }

    /// Day-count convention label (e.g. `"act_360"`).
    #[wasm_bindgen(getter, js_name = dayCount)]
    pub fn day_count(&self) -> String {
        self.inner.day_count().to_string()
    }

    /// Registered holiday calendar of the curve's time axis (for example
    /// `"brbd"` on a `"bus_252"` curve), or `undefined` when none is set.
    #[wasm_bindgen(getter, js_name = calendarId)]
    pub fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id().map(str::to_owned)
    }

    /// Interpolation style label (e.g. `"log_linear"`).
    #[wasm_bindgen(getter, js_name = interpStyle)]
    pub fn interp_style(&self) -> String {
        self.inner.interp_style().to_string()
    }

    /// Extrapolation policy label (e.g. `"extend"`).
    #[wasm_bindgen(getter, js_name = extrapolation)]
    pub fn extrapolation(&self) -> String {
        self.inner.extrapolation().to_string()
    }

    /// Calibration recipe stamped on this curve by the calibration engine.
    ///
    /// The same object found under `rate_calibration` in `toJson()`: currency,
    /// method, curve day count, OIS compounding, curve role and the complete
    /// typed quote set (rates and spreads as decimals, futures as prices).
    ///
    /// @returns The `RateCalibrationRecipe` object, or `undefined` for a curve that was built directly rather than produced by a calibration step.
    /// @throws FinstackError - If the recipe cannot be converted to a JavaScript object.
    #[wasm_bindgen(getter, js_name = rateCalibration, unchecked_return_type = "RateCalibrationRecipe | undefined")]
    pub fn rate_calibration(&self) -> Result<JsValue, JsValue> {
        match self.inner.rate_calibration() {
            Some(recipe) => crate::utils::to_js_value(recipe),
            None => Ok(JsValue::UNDEFINED),
        }
    }
}

json_round_trip!(JsForwardCurve, ForwardCurve, arc);

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> ForwardCurveOptions {
        ForwardCurveOptions {
            id: "USD-3M".into(),
            base_date: "2024-01-15".into(),
            knots: vec![0.5, (-0.02_f64).exp(), 1.0, (-0.045_f64).exp()],
            interp: "log_linear".into(),
            extrapolation: "extend".into(),
            day_count: "act_360".into(),
            calendar_id: None,
            index_tenor: Some("3M".into()),
        }
    }

    #[test]
    fn forward_curve_new_and_accessors() {
        let curve = JsForwardCurve::build(options()).expect("forward curve");
        assert_eq!(curve.id(), "USD-3M");
        assert_eq!(curve.base_date(), "2024-01-15");
        assert_eq!(curve.index_tenor().as_deref(), Some("3M"));
        assert!((curve.inner.df(1.0).expect("df") - (-0.045_f64).exp()).abs() < 1e-15);
    }

    #[test]
    fn forward_curve_accessors_and_json() {
        let curve = JsForwardCurve::build(options()).expect("forward curve");
        assert_eq!(&*curve.knots(), &[0.0, 0.5, 1.0]);
        assert_eq!(curve.factors()[0], 1.0);
        let back: RustForwardCurve =
            serde_json::from_str(&curve.to_json().expect("json")).expect("parse");
        assert_eq!(back.knots(), curve.inner.knots());
        assert_eq!(back.factors(), curve.inner.factors());
        assert_eq!(back.index_tenor(), curve.inner.index_tenor());
    }
}
