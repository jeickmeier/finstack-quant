//! WASM binding for `finstack_quant_core::market_data::term_structures::ForwardCurve`.

use crate::utils::input::{from_js_json, js_f64, js_string};
use std::sync::Arc;

use crate::utils::wire::js_date;
use crate::utils::{date_to_iso, parse_iso_date, to_js_err};
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::term_structures::ForwardCurve as RustForwardCurve;
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use js_sys::Float64Array;
use serde::Deserialize;
use wasm_bindgen::prelude::*;

use crate::api::core::curve_args::{flat_pairs, js_time_point, TimePoint};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ForwardCurveOptions {
    id: String,
    tenor: f64,
    base_date: String,
    knots: Vec<f64>,
    #[serde(default)]
    day_count: Option<String>,
    #[serde(default)]
    interp: Option<String>,
    #[serde(default)]
    extrapolation: Option<String>,
    #[serde(default)]
    projection_grid: Option<Vec<f64>>,
    #[serde(default)]
    reset_lag: Option<i32>,
}

/// Forward rate curve for a floating-rate index with a fixed tenor.
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
}

#[wasm_bindgen(js_class = ForwardCurve)]
impl JsForwardCurve {
    fn build(options: ForwardCurveOptions) -> Result<JsForwardCurve, JsValue> {
        let base = parse_iso_date(&options.base_date)?;
        let pairs = flat_pairs(&options.knots, "knots")?;

        let mut builder = RustForwardCurve::builder(options.id, options.tenor)
            .base_date(base)
            .knots(pairs)
            .projection_grid_opt(options.projection_grid);
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
        if let Some(reset_lag) = options.reset_lag {
            builder = builder.reset_lag(reset_lag);
        }

        builder.build().map(Self::from_inner).map_err(to_js_err)
    }

    /// Construct a forward curve from named options.
    ///
    /// # Arguments
    ///
    /// * `options` - ForwardCurveOptions object: curve id, tenor in years, ISO baseDate, flat time/decimal-rate knots, and optional dayCount, interp, extrapolation, projectionGrid and resetLag. Omitted policies use the Rust builder defaults; arrays and typed arrays are accepted.
    ///
    /// # Errors
    ///
    /// Throws Error when options contain unknown fields, cannot be decoded, or canonical curve validation rejects dates, conventions, knots, tenor, reset lag, or projection grid.
    #[wasm_bindgen(constructor)]
    pub fn new(options: JsValue) -> Result<JsForwardCurve, JsValue> {
        let options = from_js_json(&options, "options")?;
        Self::build(options)
    }

    /// Forward rate at year fraction `t` or on an ISO-8601 date.
    ///
    /// The stored rate is annualized on the curve's day-count basis. Convert
    /// its accrual growth to the contractual index basis before calculating a
    /// coupon with a different accrual convention; Rust instrument pricing
    /// performs this conversion from the contractual schedule.
    ///
    /// # Arguments
    ///
    /// * `t` - Time from the curve base date in years (`number`) or an ISO-8601
    ///   target date (`"YYYY-MM-DD"`, converted with the curve day count; a date
    ///   on or before the base date reads the rate at curve time zero).
    ///
    /// @returns Simple forward rate as a decimal, annualized on the curve day count.
    /// @throws `TypeError` if `t` is neither a number nor a string;
    /// `FinstackError` (kind `validation`) for a malformed date or a year
    /// fraction that cannot be computed.
    #[wasm_bindgen(js_name = rate)]
    pub fn rate(&self, t: JsValue) -> Result<f64, JsValue> {
        match js_time_point(&t, "t")? {
            TimePoint::Years(t) => Ok(self.inner.rate(t)),
            TimePoint::Date(d) => self.inner.rate_on_date(d).map_err(to_js_err),
        }
    }

    /// Discount-factor-implied simple forward over `(t1, t2)`.
    ///
    /// The denominator is the curve-time span `t2 - t1`. To express the
    /// implied growth on another contractual index basis, multiply by that
    /// span and divide by the index accrual fraction for the same dated period.
    /// @param t1 - Earlier curve time in years used as the start of the forward interval.
    /// @param t2 - Later curve time in years used as the end of the forward interval.
    /// @returns Simple forward rate as a decimal, annualized on the curve day count.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if either time is non-finite, `t2` is not
    /// later than `t1`, a projection discount factor cannot be computed, or the
    /// implied rate is non-finite.
    #[wasm_bindgen(js_name = rateBetween)]
    pub fn rate_between(&self, t1: JsValue, t2: JsValue) -> Result<f64, JsValue> {
        let t1 = js_f64(&t1, "t1")?;
        let t2 = js_f64(&t2, "t2")?;
        self.inner.rate_between(t1, t2).map_err(to_js_err)
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

    /// Contractual projection boundaries, or `undefined` for legacy tenor stepping.
    #[wasm_bindgen(getter, js_name = projectionGrid)]
    pub fn projection_grid(&self) -> JsValue {
        self.inner
            .projection_grid()
            .map_or(JsValue::UNDEFINED, |grid| Float64Array::from(grid).into())
    }

    /// Business days from fixing to spot.
    #[wasm_bindgen(getter, js_name = resetLag)]
    pub fn reset_lag(&self) -> i32 {
        self.inner.reset_lag()
    }

    /// Construct a flat forward curve (Rust `ForwardCurve::flat`).
    ///
    /// # Arguments
    ///
    /// * `id` - Curve identifier stored on the curve.
    /// * `tenor` - Index tenor in years (e.g. `0.25` for a 3M index).
    /// * `base_date` - ISO-8601 valuation date anchoring `t = 0`.
    /// * `rate` - Constant forward rate as a decimal.
    ///
    /// @returns A `ForwardCurve` with the Rust builder defaults.
    /// @throws If `baseDate` is not an ISO date, or `tenor` or `rate` is invalid.
    #[wasm_bindgen(js_name = flat)]
    pub fn flat(
        id: JsValue,
        tenor: JsValue,
        base_date: JsValue,
        rate: JsValue,
    ) -> Result<JsForwardCurve, JsValue> {
        let id = js_string(&id, "id")?;
        let tenor = js_f64(&tenor, "tenor")?;
        let base_date = js_date(&base_date, "baseDate")?;
        let rate = js_f64(&rate, "rate")?;
        RustForwardCurve::flat(id, tenor, base_date, rate)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Deserialize a forward curve from its canonical JSON wire form (the
    /// Rust serde schema shared with Python `ForwardCurve.to_json`).
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical ForwardCurve JSON text or plain object; unknown
    ///   fields are rejected and the curve is re-validated.
    ///
    /// @returns The validated `ForwardCurve`.
    /// @throws If `json` is malformed, has unknown fields, or fails curve validation.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsForwardCurve, JsValue> {
        from_js_json::<RustForwardCurve>(&json, "json").map(Self::from_inner)
    }

    /// Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid curve).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&*self.inner).map_err(to_js_err)
    }

    /// Integral-average forward rate over `[t1, t2]` on the curve day-count basis.
    ///
    /// # Arguments
    ///
    /// * `t1` - Start of the accrual period in years from `baseDate`.
    /// * `t2` - End of the accrual period in years from `baseDate`.
    ///
    /// @returns The finite period-average rate as a decimal; equal or very short intervals return the start rate.
    /// @throws If endpoints are non-finite or reversed, or the interval or result is non-finite.
    #[wasm_bindgen(js_name = ratePeriod)]
    pub fn rate_period(&self, t1: JsValue, t2: JsValue) -> Result<f64, JsValue> {
        let t1 = js_f64(&t1, "t1")?;
        let t2 = js_f64(&t2, "t2")?;
        self.inner.rate_period(t1, t2).map_err(to_js_err)
    }

    /// Projection discount factor implied by the forwards at year fraction `t`
    /// or on an ISO-8601 date.
    ///
    /// # Arguments
    ///
    /// * `t` - Time from `baseDate` in years (`number`) or an ISO-8601 target
    ///   date (`"YYYY-MM-DD"`, converted with the curve day count).
    ///
    /// @returns Projection discount factor.
    /// @throws `TypeError` if `t` is neither a number nor a string;
    /// `FinstackError` (kind `validation`) for a malformed date, a year
    /// fraction that cannot be computed, or an invalid implied discount factor.
    pub fn df(&self, t: JsValue) -> Result<f64, JsValue> {
        match js_time_point(&t, "t")? {
            TimePoint::Years(t) => self.inner.df(t).map_err(to_js_err),
            TimePoint::Date(d) => self.inner.df_on_date(d).map_err(to_js_err),
        }
    }

    /// Index tenor in years.
    #[wasm_bindgen(getter, js_name = tenor)]
    pub fn tenor(&self) -> f64 {
        self.inner.tenor()
    }

    /// Knot times in years.
    #[wasm_bindgen(getter, js_name = knots)]
    pub fn knots(&self) -> Box<[f64]> {
        self.inner.knots().into()
    }

    /// Forward rates at the knots, as decimals.
    #[wasm_bindgen(getter, js_name = forwards)]
    pub fn forwards(&self) -> Box<[f64]> {
        self.inner.forwards().into()
    }

    /// Day-count convention label (e.g. `"act_360"`).
    #[wasm_bindgen(getter, js_name = dayCount)]
    pub fn day_count(&self) -> String {
        self.inner.day_count().to_string()
    }

    /// Interpolation style label (e.g. `"linear"`).
    #[wasm_bindgen(getter, js_name = interpStyle)]
    pub fn interp_style(&self) -> String {
        self.inner.interp_style().to_string()
    }

    /// Extrapolation policy label (e.g. `"flat_forward"`).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_curve_new_and_accessors() {
        let curve = JsForwardCurve::build(ForwardCurveOptions {
            id: "USD-3M".into(),
            tenor: 0.25,
            base_date: "2024-01-15".into(),
            knots: vec![0.5, 0.04, 1.0, 0.045, 2.0, 0.05],
            day_count: None,
            interp: None,
            extrapolation: None,
            projection_grid: None,
            reset_lag: None,
        })
        .expect("forward curve");
        assert_eq!(curve.id(), "USD-3M");
        assert_eq!(curve.base_date(), "2024-01-15");
        assert!((curve.inner.rate(1.0) - 0.045).abs() < 1e-6);
    }

    #[test]
    fn forward_curve_accessors_and_json() {
        let curve = JsForwardCurve::build(ForwardCurveOptions {
            id: "USD-3M".into(),
            tenor: 0.25,
            base_date: "2024-01-15".into(),
            knots: vec![0.5, 0.04, 1.0, 0.045, 2.0, 0.05],
            day_count: None,
            interp: None,
            extrapolation: None,
            projection_grid: None,
            reset_lag: None,
        })
        .expect("forward curve");
        assert_eq!(&*curve.knots(), &[0.5, 1.0, 2.0]);
        assert_eq!(&*curve.forwards(), &[0.04, 0.045, 0.05]);
        assert!((curve.tenor() - 0.25).abs() < 1e-15);
        let back: RustForwardCurve =
            serde_json::from_str(&curve.to_json().expect("json")).expect("parse");
        assert_eq!(back.knots(), curve.inner.knots());
    }
}
