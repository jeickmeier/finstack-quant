//! WASM binding for `finstack_quant_core::market_data::term_structures::DiscountCurve`.

use crate::utils::input::{from_js_json, js_f64, js_f64_seq, js_string};
use finstack_quant_core::market_data::traits::Discounting;
use std::sync::Arc;

use crate::utils::wire::js_date;
use crate::utils::{date_to_iso, parse_iso_date, to_js_err};
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::term_structures::DiscountCurve as RustDiscountCurve;
use finstack_quant_core::math::Compounding;
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

use crate::api::core::curve_args::{TimePoint, flat_pairs, js_time_point};

/// Discount factor curve for present-value calculations.
///
/// Built from `(time, discount_factor)` pillars where `time` is a year
/// fraction from `baseDate` and `df` is the positive price today of $1 paid at
/// that time. All curve conventions (`interp`, `extrapolation`, and `dayCount`)
/// must be specified explicitly.
///
/// @example
/// ```javascript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// // OIS-style USD curve, base-date 2025-01-02, three pillars.
/// const curve = new core.DiscountCurve({
///   id: "USD-OIS",
///   baseDate: "2025-01-02",
///   knots: [0.0, 1.0, 1.0, 0.95, 5.0, 0.78],
///   interp: "monotone_convex",
///   extrapolation: "extend",
///   dayCount: "act_365f",
/// });
/// curve.df(2.5);                  // discount factor at 2.5y
/// curve.zeroRate(2.5, "continuous"); // continuously-compounded zero rate at 2.5y
/// ```
#[wasm_bindgen(js_name = DiscountCurve)]
pub struct JsDiscountCurve {
    pub(crate) inner: Arc<RustDiscountCurve>,
}

/// Named constructor options for `DiscountCurve`; unknown keys are rejected.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DiscountCurveOptions {
    id: String,
    base_date: String,
    knots: Vec<f64>,
    interp: String,
    extrapolation: String,
    day_count: String,
    #[serde(default)]
    calendar_id: Option<String>,
}

impl JsDiscountCurve {
    fn from_inner(curve: RustDiscountCurve) -> Self {
        Self {
            inner: Arc::new(curve),
        }
    }

    fn build(options: DiscountCurveOptions) -> Result<JsDiscountCurve, JsValue> {
        let base = parse_iso_date(&options.base_date)?;
        let pairs = flat_pairs(&options.knots, "knots")?;
        let interp = options.interp.parse::<InterpStyle>().map_err(to_js_err)?;
        let extrapolation = options
            .extrapolation
            .parse::<ExtrapolationPolicy>()
            .map_err(to_js_err)?;
        let day_count = options.day_count.parse::<DayCount>().map_err(to_js_err)?;

        RustDiscountCurve::builder(options.id)
            .base_date(base)
            .knots(pairs)
            .calendar_id_opt(options.calendar_id)
            .interp(interp)
            .extrapolation(extrapolation)
            .day_count(day_count)
            .build()
            .map(Self::from_inner)
            .map_err(to_js_err)
    }
}

#[wasm_bindgen(js_class = DiscountCurve)]
impl JsDiscountCurve {
    /// Construct a discount curve from named options.
    ///
    /// # Arguments
    ///
    /// * `options` - DiscountCurveOptions object (or its JSON text) with:
    ///   `id` (curve identifier, the `MarketContext` lookup key); `baseDate`
    ///   (ISO-8601 `"YYYY-MM-DD"`; knot times are year fractions from it under
    ///   `dayCount`); `knots` (flat `[t0, df0, t1, df1, …]` array or typed
    ///   array, `t` in years, `df` strictly positive, even length); `interp`
    ///   (`"linear"`, `"log_linear"`, `"monotone_convex"`, `"cubic_hermite"`,
    ///   `"piecewise_quadratic_forward"`); `extrapolation` (`"flat"`,
    ///   `"extend"`, or `"error"`, which rejects queries outside the pillar
    ///   range); `dayCount` (used to convert dates to curve time; not inferred
    ///   from the ID); and optional `calendarId` (registered holiday calendar
    ///   of the curve's time axis, e.g. `"brbd"`; required with `"bus_252"`).
    ///   Unknown keys are rejected.
    ///
    /// @returns The constructed `DiscountCurve`.
    /// @throws `TypeError` (kind `invalid_type`) if `options` is not a JSON
    /// string or plain object; `FinstackError` (kind `validation`) for an
    /// unknown, missing, or mistyped key, an odd `knots` length, a malformed
    /// date, an unknown interpolation/extrapolation/day-count name, a
    /// `"bus_252"` curve without `calendarId`, or invalid discount factors;
    /// `FinstackError` (kind `not_found`) for an unknown `calendarId`.
    /// @param options - DiscountCurveOptions object (or its JSON text) with `id`, `baseDate`, `knots`, `interp`, `extrapolation`, `dayCount`, and optional `calendarId`. Unknown keys are rejected.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "DiscountCurveOptions | string")] options: JsValue,
    ) -> Result<JsDiscountCurve, JsValue> {
        Self::build(from_js_json(&options, "options")?)
    }

    /// Construct a flat continuously-compounded discount curve.
    /// @param id - Curve identifier stored on the constructed discount curve.
    /// @param baseDate - ISO-8601 curve base date from which time coordinates are measured.
    /// @param continuousRate - Flat continuously compounded zero rate expressed as a decimal.
    /// @param dayCount - Explicit curve day-count convention label (for example `"act_365f"` or `"act_360"`).
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if `baseDate` is not a valid ISO date,
    /// `dayCount` is unsupported or unknown, `continuousRate` is non-finite or
    /// `|continuousRate| > 1` (rates are decimals: `0.05` is 5%), or the implied
    /// discount factors are not finite and strictly positive.
    /// @returns A `DiscountCurve` handle.
    #[wasm_bindgen(js_name = flat)]
    pub fn flat(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        #[wasm_bindgen(js_name = "baseDate", unchecked_param_type = "string")] base_date: JsValue,
        #[wasm_bindgen(js_name = "continuousRate", unchecked_param_type = "number")]
        continuous_rate: JsValue,
        #[wasm_bindgen(js_name = "dayCount", unchecked_param_type = "string")] day_count: JsValue,
    ) -> Result<JsDiscountCurve, JsValue> {
        let continuous_rate = js_f64(&continuous_rate, "continuousRate")?;
        let id = js_string(&id, "id")?;
        let day_count = js_string(&day_count, "dayCount")?
            .parse::<DayCount>()
            .map_err(to_js_err)?;
        RustDiscountCurve::flat(
            id,
            js_date(&base_date, "baseDate")?,
            continuous_rate,
            day_count,
        )
        .map(Self::from_inner)
        .map_err(to_js_err)
    }

    /// Discount factor at year fraction `t` or on an ISO-8601 date.
    ///
    /// # Arguments
    ///
    /// * `t` - Time from the curve base date in years (`number`, must be non-negative)
    ///   or an ISO-8601 target date (`"YYYY-MM-DD"`, converted with the curve day count).
    ///
    /// @returns The discount factor from the base date to `t`.
    /// @throws `TypeError` if `t` is neither a number nor a string;
    /// `FinstackError` (kind `validation`) for a malformed date, negative time,
    /// out-of-range `"error"` tail query, or a year fraction that cannot be computed.
    /// @param t - Time from the curve base date in years (`number`) or an ISO-8601 target date (`"YYYY-MM-DD"`, converted with the curve day count).
    pub fn df(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number | string")] t: JsValue,
    ) -> Result<f64, JsValue> {
        match js_time_point(&t, "t")? {
            TimePoint::Years(t) => self.inner.df(t).map_err(to_js_err),
            TimePoint::Date(d) => self.inner.df_on_date(d).map_err(to_js_err),
        }
    }

    /// Continuously-compounded forward rate between `t1` and `t2`.
    /// @param t1 - Earlier curve time in years used as the start of the forward interval.
    /// @param t2 - Later curve time in years used as the end of the forward interval.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if either time is non-finite, `t2` is not
    /// later than `t1`, the interval is shorter than the curve's minimum forward
    /// tenor, or either endpoint discount factor is non-finite or non-positive.
    /// @returns Continuously compounded forward rate as a decimal over `(t1, t2)`.
    #[wasm_bindgen(js_name = forward)]
    pub fn forward(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] t1: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] t2: JsValue,
    ) -> Result<f64, JsValue> {
        let t1 = js_f64(&t1, "t1")?;
        let t2 = js_f64(&t2, "t2")?;
        self.inner.forward(t1, t2).map_err(to_js_err)
    }

    /// Construct a curve from zero rates (Rust `DiscountCurve::from_zero_rates`).
    ///
    /// # Arguments
    ///
    /// * `id` - Curve identifier stored on the curve.
    /// * `base_date` - ISO-8601 valuation date anchoring `t = 0`.
    /// * `points` - Flat `[t0, z0, t1, z1, …]` array: times in years and
    ///   zero rates as decimals (`0.05` is 5%), with strictly increasing times.
    /// * `compounding` - Compounding of the zero rates: `"continuous"`,
    ///   `"simple"`, `"periodic(1)"`, `"semi_annual"`, `"quarterly"` or `"monthly"`.
    /// * `day_count` - Day-count convention label (`"act_365f"`, `"act_360"`, etc.).
    /// * `interp` - Interpolation style (`"linear"`, `"log_linear"`,
    ///   `"monotone_convex"`, `"cubic_hermite"`, `"piecewise_quadratic_forward"`).
    /// * `extrapolation` - Tail extrapolation policy (`"flat"`, `"extend"`, or `"error"`).
    ///
    /// @returns Curve whose discount factors reproduce every zero rate.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) for an empty or odd-length `points`, a malformed date, an
    /// unknown compounding/day-count/interpolation/extrapolation label, or
    /// invalid zero rates.
    /// @param id - Curve identifier stored on the curve.
    /// @param baseDate - ISO-8601 valuation date anchoring `t = 0`.
    /// @param points - Flat `[t0, z0, t1, z1, …]` array: times in years and zero rates as decimals (`0.05` is 5%), with strictly increasing times.
    /// @param compounding - Compounding of the zero rates: `"continuous"`, `"simple"`, `"periodic(1)"`, `"semi_annual"`, `"quarterly"` or `"monthly"`.
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
    ) -> Result<JsDiscountCurve, JsValue> {
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
        RustDiscountCurve::from_zero_rates(
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

    /// Construct a curve from dated discount factors (Rust `DiscountCurve::from_dates`).
    ///
    /// # Arguments
    ///
    /// * `id` - Curve identifier stored on the curve.
    /// * `base_date` - ISO-8601 valuation date anchoring `t = 0`.
    /// * `points` - Array of `[isoDate, discountFactor]` pairs with strictly
    ///   increasing dates on or after `baseDate` and strictly positive
    ///   discount factors.
    /// * `day_count` - Day count that converts each date to curve time.
    /// * `interp` - Explicit interpolation style label.
    /// * `extrapolation` - Explicit tail extrapolation policy label.
    ///
    /// @returns Curve with one pillar per dated point.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) for empty `points`, a malformed date, an unknown
    /// day-count/interpolation/extrapolation label, or invalid discount factors.
    /// @param id - Curve identifier stored on the curve.
    /// @param baseDate - ISO-8601 valuation date anchoring `t = 0`.
    /// @param points - Array of `[isoDate, discountFactor]` pairs with strictly increasing dates on or after `baseDate` and strictly positive discount factors.
    /// @param dayCount - Explicit day count that converts each date to curve time.
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
    ) -> Result<JsDiscountCurve, JsValue> {
        let id = js_string(&id, "id")?;
        let base_date = js_date(&base_date, "baseDate")?;
        let points = from_js_json::<Vec<(String, f64)>>(&points, "points")?
            .into_iter()
            .map(|(date, df)| Ok((parse_iso_date(&date)?, df)))
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
        RustDiscountCurve::from_dates(id, base_date, &points, day_count, interp, extrapolation)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Deserialize from the canonical JSON wire form shared with Python `DiscountCurve.to_json`.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical DiscountCurve JSON text or plain object; unknown
    ///   fields are rejected and the curve is re-validated.
    ///
    /// @returns The validated `DiscountCurve`.
    /// @throws `TypeError` if `json` is not a JSON string or plain object;
    /// `FinstackError` (kind `validation`) if it does not match the schema or
    /// fails curve validation.
    /// @param json - Canonical DiscountCurve JSON text or plain object; unknown fields are rejected and the curve is re-validated.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(
        #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
    ) -> Result<JsDiscountCurve, JsValue> {
        from_js_json::<RustDiscountCurve>(&json, "json").map(Self::from_inner)
    }

    /// Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid curve).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&*self.inner).map_err(to_js_err)
    }

    /// Zero rate at year fraction `t` or on an ISO-8601 date under an explicit
    /// compounding convention (Rust `DiscountCurve::zero_rate` / `DiscountCurve::zero_rate_on_date`).
    ///
    /// # Arguments
    ///
    /// * `t` - Strictly positive time from the curve base date in years (`number`)
    ///   or an ISO-8601 target date strictly after `baseDate` (`"YYYY-MM-DD"`,
    ///   measured as the difference between the target and base coordinates
    ///   on the original source clock).
    /// * `compounding` - `"continuous"`, `"simple"`, `"periodic(1)"`,
    ///   `"semi_annual"`, `"quarterly"` or `"monthly"`.
    ///
    /// @returns The zero rate as a decimal.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) for `t <= 0`, a date on or before `baseDate`, a malformed
    /// date, an unknown compounding, or a year fraction that cannot be computed.
    /// @param t - Strictly positive time from the curve base date in years (`number`) or an ISO-8601 target date strictly after `baseDate`. Dated horizons subtract the base coordinate from the target coordinate on the original source clock.
    /// @param compounding - `"continuous"`, `"simple"`, `"periodic(1)"`, `"semi_annual"`, `"quarterly"` or `"monthly"`.
    #[wasm_bindgen(js_name = zeroRate)]
    pub fn zero_rate(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number | string")] t: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] compounding: JsValue,
    ) -> Result<f64, JsValue> {
        let comp = js_string(&compounding, "compounding")?
            .parse::<Compounding>()
            .map_err(to_js_err)?;
        match js_time_point(&t, "t")? {
            TimePoint::Years(t) => self.inner.zero_rate(t, comp).map_err(to_js_err),
            TimePoint::Date(d) => self.inner.zero_rate_on_date(d, comp).map_err(to_js_err),
        }
    }

    /// Forward discount factor between two dates, `df(toDate) / df(fromDate)`
    /// (Rust `DiscountCurve::df_between_dates`).
    ///
    /// # Arguments
    ///
    /// * `from_date` - ISO-8601 start date of the discounting interval.
    /// * `to_date` - ISO-8601 end date of the discounting interval.
    ///
    /// @returns The discount factor that brings a cashflow on `toDate` back to
    /// `fromDate`.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) for a malformed date, a year fraction that cannot be
    /// computed, or a non-finite or non-positive discount factor.
    /// @param fromDate - ISO-8601 start date of the discounting interval.
    /// @param toDate - ISO-8601 end date of the discounting interval.
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

    /// Pillar times in years from the base date, strictly increasing.
    #[wasm_bindgen(getter, js_name = knots)]
    pub fn knots(&self) -> Box<[f64]> {
        self.inner.knots().into()
    }

    /// Discount factor at each pillar, aligned with `knots`.
    #[wasm_bindgen(getter, js_name = dfs)]
    pub fn dfs(&self) -> Box<[f64]> {
        self.inner.dfs().into()
    }

    /// Day count that converts dates to curve time, such as `"act_365f"`.
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

    /// Interpolation style between pillars, such as `"monotone_convex"`.
    #[wasm_bindgen(getter, js_name = interpStyle)]
    pub fn interp_style(&self) -> String {
        self.inner.interp_style().to_string()
    }

    /// Extrapolation policy beyond the last pillar, such as `"extend"`.
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
