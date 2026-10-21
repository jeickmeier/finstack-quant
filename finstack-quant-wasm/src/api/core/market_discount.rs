//! WASM binding for `finstack_quant_core::market_data::term_structures::DiscountCurve`.

use crate::utils::input::{from_js_json, js_f64, js_f64_seq, js_opt_string, js_string};
use finstack_quant_core::market_data::traits::Discounting;
use std::sync::Arc;

use crate::utils::wire::js_date;
use crate::utils::{date_to_iso, parse_iso_date, to_js_err};
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve as RustDiscountCurve, ValidationMode,
};
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_quant_core::math::Compounding;
use serde::Deserialize;
use wasm_bindgen::prelude::*;

use crate::api::core::curve_args::{flat_pairs, js_time_point, TimePoint};
use crate::api::core::market_forward::JsForwardCurve;

/// Discount factor curve for present-value calculations.
///
/// Built from `(time, discount_factor)` pillars where `time` is a year
/// fraction from `baseDate` and `df` is the price today of $1 paid at that
/// time. Defaults reflect the most common practitioner convention
/// (Hagan-West monotone-convex interpolation, flat-forward extrapolation,
/// Act/365 fixed day-count).
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
///   extrapolation: "flat_forward",
///   dayCount: "act_365f",
/// });
/// curve.df(2.5);          // discount factor at 2.5y
/// curve.zeroRate(2.5);    // continuously-compounded zero rate at 2.5y
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
    #[serde(default)]
    interp: Option<String>,
    #[serde(default)]
    extrapolation: Option<String>,
    #[serde(default)]
    day_count: Option<String>,
    #[serde(default)]
    calendar_id: Option<String>,
    #[serde(default)]
    validation_mode: Option<String>,
    #[serde(default)]
    forward_floor: Option<f64>,
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

        let mut builder = RustDiscountCurve::builder(options.id)
            .base_date(base)
            .knots(pairs)
            .calendar_id_opt(options.calendar_id);
        if let Some(interp) = options.interp.as_deref() {
            builder = builder.interp(interp.parse::<InterpStyle>().map_err(to_js_err)?);
        }
        if let Some(extrapolation) = options.extrapolation.as_deref() {
            builder = builder.extrapolation(
                extrapolation
                    .parse::<ExtrapolationPolicy>()
                    .map_err(to_js_err)?,
            );
        }
        if let Some(day_count) = options.day_count.as_deref() {
            builder = builder.day_count(day_count.parse::<DayCount>().map_err(to_js_err)?);
        }
        builder = builder.validation(
            ValidationMode::from_preset(options.validation_mode.as_deref(), options.forward_floor)
                .map_err(to_js_err)?,
        );

        builder.build().map(Self::from_inner).map_err(to_js_err)
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
    ///   array, `t` in years, `df` strictly positive, even length); and the
    ///   optional `interp` (`"linear"`, `"log_linear"`, `"monotone_convex"`,
    ///   `"cubic_hermite"`, `"piecewise_quadratic_forward"`), `extrapolation`
    ///   (`"flat_zero"`, `"flat_forward"`, or `"none"`, which returns NaN
    ///   outside the pillar range), `dayCount` (used to convert dates to curve
    ///   time; not inferred from the ID), `calendarId` (registered holiday
    ///   calendar of the curve's time axis, e.g. `"brbd"`; required with
    ///   `"bus_252"`),
    ///   `validationMode` (`"market_standard"` or `"negative_rate_friendly"`)
    ///   and `forwardFloor` (decimal minimum implied forward, required with
    ///   `"negative_rate_friendly"` and rejected otherwise). Omitted options use
    ///   the Rust builder defaults: `monotone_convex`, `flat_forward`,
    ///   `act_365f` and `market_standard`. Unknown keys are rejected.
    ///
    /// @returns The constructed `DiscountCurve`.
    /// @throws `TypeError` (kind `invalid_type`) if `options` is not a JSON
    /// string or plain object; `FinstackError` (kind `validation`) for an
    /// unknown or mistyped key, an odd `knots` length, a malformed date, an
    /// unknown interpolation/extrapolation/day-count/validation name, a
    /// misplaced or missing `forwardFloor`, a `"bus_252"` curve without
    /// `calendarId`, or discount factors the curve validation rejects;
    /// `FinstackError` (kind `not_found`) for an unknown `calendarId`.
    #[wasm_bindgen(constructor)]
    pub fn new(options: JsValue) -> Result<JsDiscountCurve, JsValue> {
        Self::build(from_js_json(&options, "options")?)
    }

    /// Construct a flat continuously-compounded discount curve.
    /// @param id - Curve identifier stored on the constructed discount curve.
    /// @param base_date - ISO-8601 curve base date from which time coordinates are measured.
    /// @param continuous_rate - Flat continuously compounded zero rate expressed as a decimal.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if `baseDate` is not a valid ISO date,
    /// `continuousRate` is non-finite or `|continuousRate| > 1` (rates are
    /// decimals: `0.05` is 5%), or the implied discount factors are not finite
    /// and strictly positive.
    #[wasm_bindgen(js_name = flat)]
    pub fn flat(
        id: JsValue,
        base_date: JsValue,
        continuous_rate: JsValue,
    ) -> Result<JsDiscountCurve, JsValue> {
        let continuous_rate = js_f64(&continuous_rate, "continuousRate")?;
        let id = js_string(&id, "id")?;
        RustDiscountCurve::flat(id, js_date(&base_date, "baseDate")?, continuous_rate)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Discount factor at year fraction `t` or on an ISO-8601 date.
    ///
    /// # Arguments
    ///
    /// * `t` - Time from the curve base date in years (`number`) or an ISO-8601
    ///   target date (`"YYYY-MM-DD"`, converted with the curve day count).
    ///
    /// @returns The discount factor from the base date to `t`.
    /// @throws `TypeError` if `t` is neither a number nor a string;
    /// `FinstackError` (kind `validation`) for a malformed date or a year
    /// fraction that cannot be computed.
    pub fn df(&self, t: JsValue) -> Result<f64, JsValue> {
        match js_time_point(&t, "t")? {
            TimePoint::Years(t) => Ok(self.inner.df(t)),
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
    #[wasm_bindgen(js_name = forward)]
    pub fn forward(&self, t1: JsValue, t2: JsValue) -> Result<f64, JsValue> {
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
    ///   `"simple"`, `"annual"`, `"semi_annual"`, `"quarterly"` or `"monthly"`;
    ///   omitted means `"continuous"`.
    ///
    /// @returns Curve whose discount factors reproduce every zero rate.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) for an empty or odd-length `points`, a malformed date, an
    /// unknown compounding, or discount factors the curve validation rejects.
    #[wasm_bindgen(js_name = fromZeroRates)]
    pub fn from_zero_rates(
        id: JsValue,
        base_date: JsValue,
        points: JsValue,
        compounding: Option<JsValue>,
    ) -> Result<JsDiscountCurve, JsValue> {
        let id = js_string(&id, "id")?;
        let base_date = js_date(&base_date, "baseDate")?;
        let points = flat_pairs(&js_f64_seq(&points, "points")?, "points")?;
        let compounding = compounding_arg(compounding.as_ref())?;
        RustDiscountCurve::from_zero_rates(id, base_date, &points, compounding)
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
    /// * `day_count` - Day count that converts each date to curve time;
    ///   omitted uses the Rust default (`"act_365f"`).
    ///
    /// @returns Curve with one pillar per dated point.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) for empty `points`, a malformed date, an unknown day
    /// count, or discount factors the curve validation rejects.
    #[wasm_bindgen(js_name = fromDates)]
    pub fn from_dates(
        id: JsValue,
        base_date: JsValue,
        points: JsValue,
        day_count: Option<JsValue>,
    ) -> Result<JsDiscountCurve, JsValue> {
        let id = js_string(&id, "id")?;
        let base_date = js_date(&base_date, "baseDate")?;
        let points = from_js_json::<Vec<(String, f64)>>(&points, "points")?
            .into_iter()
            .map(|(date, df)| Ok((parse_iso_date(&date)?, df)))
            .collect::<Result<Vec<_>, JsValue>>()?;
        let day_count = js_opt_string(day_count.as_ref(), "dayCount")?
            .map(|name| name.parse::<DayCount>())
            .transpose()
            .map_err(to_js_err)?;
        RustDiscountCurve::from_dates(id, base_date, &points, day_count)
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
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsDiscountCurve, JsValue> {
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

    /// Zero rate at year fraction `t` or on an ISO-8601 date under a compounding
    /// convention (Rust `DiscountCurve::zero_rate` / `DiscountCurve::zero_rate_on_date`).
    ///
    /// # Arguments
    ///
    /// * `t` - Time from the curve base date in years (`number`) or an ISO-8601
    ///   target date (`"YYYY-MM-DD"`, converted with the curve day count).
    /// * `compounding` - `"continuous"`, `"simple"`, `"annual"`,
    ///   `"semi_annual"`, `"quarterly"` or `"monthly"`; omitted means
    ///   `"continuous"`.
    ///
    /// @returns The zero rate as a decimal; `0` at `t = 0`.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) for a malformed date, an unknown compounding, or a year
    /// fraction that cannot be computed.
    #[wasm_bindgen(js_name = zeroRate)]
    pub fn zero_rate(&self, t: JsValue, compounding: Option<JsValue>) -> Result<f64, JsValue> {
        let comp = compounding_arg(compounding.as_ref())?;
        match js_time_point(&t, "t")? {
            TimePoint::Years(t) => Ok(self.inner.zero_rate(t, comp)),
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
    #[wasm_bindgen(js_name = dfBetweenDates)]
    pub fn df_between_dates(&self, from_date: JsValue, to_date: JsValue) -> Result<f64, JsValue> {
        let from_date = js_date(&from_date, "fromDate")?;
        let to_date = js_date(&to_date, "toDate")?;
        self.inner
            .df_between_dates(from_date, to_date)
            .map_err(to_js_err)
    }

    /// Derive a single-curve forward curve from this discount curve (Rust
    /// `DiscountCurve::to_forward_curve`).
    ///
    /// # Arguments
    ///
    /// * `forward_id` - Identifier of the new forward curve.
    /// * `tenor` - Index tenor in years (for example `0.25` for 3M); finite
    ///   and strictly positive.
    /// * `interp` - Interpolation style of the forward curve (for example
    ///   `"linear"`); omitted uses the Rust default.
    ///
    /// @returns A `ForwardCurve` of simple forward rates implied by this curve.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) for a non-positive tenor, an unknown interpolation style,
    /// or forwards the forward-curve builder rejects.
    #[wasm_bindgen(js_name = toForwardCurve)]
    pub fn to_forward_curve(
        &self,
        forward_id: JsValue,
        tenor: JsValue,
        interp: Option<JsValue>,
    ) -> Result<JsForwardCurve, JsValue> {
        let forward_id = js_string(&forward_id, "forwardId")?;
        let tenor = js_f64(&tenor, "tenor")?;
        let interp = js_opt_string(interp.as_ref(), "interp")?
            .map(|name| name.parse::<InterpStyle>())
            .transpose()
            .map_err(to_js_err)?;
        self.inner
            .to_forward_curve(forward_id, tenor, interp)
            .map(JsForwardCurve::from_inner)
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

    /// Extrapolation policy beyond the last pillar, such as `"flat_forward"`.
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
    #[wasm_bindgen(getter, js_name = rateCalibration)]
    pub fn rate_calibration(&self) -> Result<JsValue, JsValue> {
        match self.inner.rate_calibration() {
            Some(recipe) => crate::utils::to_js_value(recipe),
            None => Ok(JsValue::UNDEFINED),
        }
    }
}

/// Parse an optional compounding name; omitted means continuous.
fn compounding_arg(value: Option<&JsValue>) -> Result<Compounding, JsValue> {
    match js_opt_string(value, "compounding")? {
        Some(name) => name.parse::<Compounding>().map_err(to_js_err),
        None => Ok(Compounding::default()),
    }
}
