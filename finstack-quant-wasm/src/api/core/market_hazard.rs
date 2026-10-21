//! WASM binding for `finstack_quant_core::market_data::term_structures::HazardCurve`.

use crate::utils::input::{
    from_js_json, js_f64, js_f64_seq, js_opt_string, js_string, js_string_seq,
};
use std::sync::Arc;

use crate::api::core::currency::JsCurrency;
use crate::utils::wire::{js_date, label, parse_label};
use crate::utils::{date_to_iso, parse_iso_date, parse_iso_dates, to_js_err};
use finstack_quant_core::currency::Currency as RustCurrency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::term_structures::HazardCurve as RustHazardCurve;
use serde::Deserialize;
use wasm_bindgen::prelude::*;

use crate::api::core::curve_args::{flat_pairs, flatten_pairs, js_time_point, TimePoint};

/// Credit hazard-rate curve for default-probability modelling.
///
/// Built from `(time, hazard_rate)` pillars where `time` is a year fraction
/// from `baseDate` and `hazard_rate` is the instantaneous default intensity
/// `λ(t)`. Survival is `S(t) = exp(-∫₀ᵗ λ(u) du)`.
///
/// @example
/// ```javascript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// // Flat 200bp hazard rate, 40% recovery.
/// const hz = new core.HazardCurve({
///   id: "ACME-HZD",
///   baseDate: "2025-01-02",
///   knots: [0.0, 0.02, 30.0, 0.02],
///   recoveryRate: 0.4,
/// });
/// hz.sp(5.0);          // survival probability at 5y
/// hz.hazardRate(5.0);  // instantaneous hazard rate at 5y
/// const copy = core.HazardCurve.fromJson(hz.toJson());
/// ```
#[wasm_bindgen(js_name = HazardCurve)]
pub struct JsHazardCurve {
    pub(crate) inner: Arc<RustHazardCurve>,
}

/// Named constructor options for `HazardCurve`; unknown keys are rejected.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HazardCurveOptions {
    id: String,
    base_date: String,
    knots: Vec<f64>,
    recovery_rate: f64,
    #[serde(default)]
    day_count: Option<String>,
    #[serde(default)]
    par_spreads: Option<Vec<f64>>,
    #[serde(default)]
    par_interp: Option<String>,
    #[serde(default)]
    issuer: Option<String>,
    #[serde(default)]
    seniority: Option<String>,
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    max_hazard_rate: Option<f64>,
}

impl JsHazardCurve {
    fn from_inner(curve: RustHazardCurve) -> Self {
        Self {
            inner: Arc::new(curve),
        }
    }

    fn build(options: HazardCurveOptions) -> Result<JsHazardCurve, JsValue> {
        let mut builder = RustHazardCurve::builder(options.id)
            .base_date(parse_iso_date(&options.base_date)?)
            .knots(flat_pairs(&options.knots, "knots")?)
            .recovery_rate(options.recovery_rate);
        if let Some(day_count) = options.day_count.as_deref() {
            builder = builder.day_count(day_count.parse::<DayCount>().map_err(to_js_err)?);
        }
        if let Some(par_spreads) = options.par_spreads.as_deref() {
            builder = builder.par_spreads(flat_pairs(par_spreads, "parSpreads")?);
        }
        if let Some(par_interp) = options.par_interp.as_deref() {
            builder = builder.par_interp(parse_label(par_interp)?);
        }
        if let Some(issuer) = options.issuer {
            builder = builder.issuer(issuer);
        }
        if let Some(seniority) = options.seniority.as_deref() {
            builder = builder.seniority(parse_label(seniority)?);
        }
        if let Some(currency) = options.currency.as_deref() {
            builder = builder.currency(currency.parse::<RustCurrency>().map_err(to_js_err)?);
        }
        if let Some(max_hazard_rate) = options.max_hazard_rate {
            builder = builder.max_hazard_rate(max_hazard_rate);
        }
        builder.build().map(Self::from_inner).map_err(to_js_err)
    }
}

#[wasm_bindgen(js_class = HazardCurve)]
impl JsHazardCurve {
    /// Construct a hazard curve from named options.
    ///
    /// # Arguments
    ///
    /// * `options` - HazardCurveOptions object (or its JSON text) with: `id`
    ///   (curve identifier, the `MarketContext` lookup key); `baseDate`
    ///   (ISO-8601 `"YYYY-MM-DD"`; knot times are year fractions from it under
    ///   `dayCount`); `knots` (flat `[t0, lambda0, t1, lambda1, …]` array or
    ///   typed array, `t` in years, `lambda` a non-negative annual default
    ///   intensity as a decimal); `recoveryRate` (required recovery on default,
    ///   decimal in `[0, 1]`); and the optional `dayCount` (default
    ///   `"act_365f"`), `parSpreads` (flat `[t0, bp0, …]` par CDS quotes in
    ///   basis points, kept for reporting), `parInterp` (`"linear"` default or
    ///   `"log_linear"`), `issuer`, `seniority` (`"senior_secured"`,
    ///   `"senior"`, `"subordinated"`, `"junior"`), `currency` (ISO-4217 code of
    ///   the protection leg) and `maxHazardRate` (sanity ceiling on any knot,
    ///   default `10.0`). Omitted options use the Rust builder defaults.
    ///   Unknown keys are rejected.
    ///
    /// @returns The constructed `HazardCurve`.
    /// @throws `TypeError` (kind `invalid_type`) if `options` is not a JSON
    /// string or plain object, or holds a non-finite number; `FinstackError`
    /// (kind `validation`) for an unknown, missing or mistyped key, an
    /// odd-length `knots`/`parSpreads`, a malformed date, an unknown label, a
    /// knot the curve builder rejects, or `recoveryRate` outside `[0, 1]`.
    #[wasm_bindgen(constructor)]
    pub fn new(options: JsValue) -> Result<JsHazardCurve, JsValue> {
        Self::build(from_js_json(&options, "options")?)
    }

    /// Construct a flat (constant-intensity) hazard curve (Rust `HazardCurve::flat`).
    ///
    /// # Arguments
    ///
    /// * `id` - Curve identifier stored on the curve.
    /// * `base_date` - ISO-8601 valuation date anchoring `t = 0`.
    /// * `hazard_rate` - Constant annual default intensity as a decimal (`0.02` is 2%).
    /// * `recovery_rate` - Recovery on default as a decimal fraction in `[0, 1]`.
    ///
    /// @returns Curve with `sp(t) === Math.exp(-hazardRate * t)`.
    /// @throws If `baseDate` is not an ISO date, `hazardRate` is non-finite or
    /// negative, or `recoveryRate` is outside `[0, 1]`.
    #[wasm_bindgen(js_name = flat)]
    pub fn flat(
        id: JsValue,
        base_date: JsValue,
        hazard_rate: JsValue,
        recovery_rate: JsValue,
    ) -> Result<JsHazardCurve, JsValue> {
        let id = js_string(&id, "id")?;
        let base_date = js_date(&base_date, "baseDate")?;
        let hazard_rate = js_f64(&hazard_rate, "hazardRate")?;
        let recovery_rate = js_f64(&recovery_rate, "recoveryRate")?;
        RustHazardCurve::flat(id, base_date, hazard_rate, recovery_rate)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Construct a hazard curve from survival-probability pillars (Rust
    /// `HazardCurve::from_survival_probs`).
    ///
    /// # Arguments
    ///
    /// * `id` - Curve identifier stored on the curve.
    /// * `base_date` - ISO-8601 valuation date anchoring `t = 0`.
    /// * `points` - Flat `[t0, s0, t1, s1, …]` array: times in years and
    ///   survival probabilities in `(0, 1]`, non-increasing in time; a `t = 0`
    ///   pillar must be `1.0`.
    /// * `recovery_rate` - Recovery on default as a decimal fraction in `[0, 1]`.
    ///
    /// @returns Piecewise-constant hazard curve reproducing every pillar.
    /// @throws If `points` is empty or odd-length, a probability is outside
    /// `(0, 1]` or increases with time, or `recoveryRate` is outside `[0, 1]`.
    #[wasm_bindgen(js_name = fromSurvivalProbs)]
    pub fn from_survival_probs(
        id: JsValue,
        base_date: JsValue,
        points: JsValue,
        recovery_rate: JsValue,
    ) -> Result<JsHazardCurve, JsValue> {
        let id = js_string(&id, "id")?;
        let base_date = js_date(&base_date, "baseDate")?;
        let points = flat_pairs(&js_f64_seq(&points, "points")?, "points")?;
        let recovery_rate = js_f64(&recovery_rate, "recoveryRate")?;
        RustHazardCurve::from_survival_probs(id, base_date, &points, recovery_rate)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Deserialize a hazard curve from its canonical JSON wire form (the
    /// Rust serde schema shared with Python `HazardCurve.to_json`).
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical HazardCurve JSON text or plain object, such as
    ///   `HazardCurve.toJson()` output (for example of the curve that
    ///   `models.credit.MertonModel.toHazardCurve` returns). Unknown fields
    ///   are rejected and the curve is re-validated.
    ///
    /// @returns The validated `HazardCurve`.
    /// @throws If `json` is malformed, has unknown fields, or fails curve validation.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsHazardCurve, JsValue> {
        from_js_json::<RustHazardCurve>(&json, "json").map(Self::from_inner)
    }

    /// Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid curve).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&*self.inner).map_err(to_js_err)
    }

    /// Survival probability `S(t)` at year fraction `t` or on an ISO-8601 date.
    ///
    /// # Arguments
    ///
    /// * `t` - Time from the curve base date in years (`number`) or an ISO-8601
    ///   target date (`"YYYY-MM-DD"`, converted with the curve day count).
    ///
    /// @returns The probability of surviving from the base date through `t`, in `[0, 1]`.
    /// @throws `TypeError` if `t` is neither a number nor a string;
    /// `FinstackError` (kind `validation`) for a malformed date or a year
    /// fraction that cannot be computed.
    pub fn sp(&self, t: JsValue) -> Result<f64, JsValue> {
        match js_time_point(&t, "t")? {
            TimePoint::Years(t) => Ok(self.inner.sp(t)),
            TimePoint::Date(d) => self.inner.sp_on_date(d).map_err(to_js_err),
        }
    }

    /// Instantaneous hazard rate `lambda(t)` at year fraction `t` or on an ISO-8601 date.
    ///
    /// # Arguments
    ///
    /// * `t` - Time from the curve base date in years (`number`) or an ISO-8601
    ///   target date (`"YYYY-MM-DD"`, converted with the curve day count).
    ///
    /// @returns The annualized default intensity at `t`, expressed as a decimal rate.
    /// @throws `TypeError` if `t` is neither a number nor a string;
    /// `FinstackError` (kind `validation`) for a malformed date or a year
    /// fraction that cannot be computed.
    #[wasm_bindgen(js_name = hazardRate)]
    pub fn hazard_rate(&self, t: JsValue) -> Result<f64, JsValue> {
        match js_time_point(&t, "t")? {
            TimePoint::Years(t) => Ok(self.inner.hazard_rate(t)),
            TimePoint::Date(d) => self.inner.hazard_rate_on_date(d).map_err(to_js_err),
        }
    }

    /// Survival probabilities on several dates.
    ///
    /// # Arguments
    ///
    /// * `dates` - ISO-8601 target dates; a date on or before `baseDate` yields `1`.
    ///
    /// @returns One survival probability per input date, in order.
    /// @throws If a date is not an ISO date or a year fraction cannot be computed.
    #[wasm_bindgen(js_name = survivalAtDates)]
    pub fn survival_at_dates(&self, dates: JsValue) -> Result<Box<[f64]>, JsValue> {
        let dates = parse_iso_dates(&js_string_seq(&dates, "dates")?)?;
        self.inner
            .survival_at_dates(&dates)
            .map(Vec::into_boxed_slice)
            .map_err(to_js_err)
    }

    /// Probability of default in `[t1, t2]`: `sp(t1) - sp(t2)`.
    ///
    /// # Arguments
    ///
    /// * `t1` - Start year fraction from `baseDate`.
    /// * `t2` - End year fraction; must not precede `t1`.
    ///
    /// @returns Default probability in `[0, 1]`.
    /// @throws If `t2 < t1`.
    #[wasm_bindgen(js_name = defaultProb)]
    pub fn default_prob(&self, t1: JsValue, t2: JsValue) -> Result<f64, JsValue> {
        let t1 = js_f64(&t1, "t1")?;
        let t2 = js_f64(&t2, "t2")?;
        self.inner.default_prob(t1, t2).map_err(to_js_err)
    }

    /// Interpolated par CDS spread in basis points at year fraction `t`.
    ///
    /// Uses the stored `parSpreads` quotes; with fewer than two quotes it
    /// falls back to a hazard-based approximation.
    ///
    /// # Arguments
    ///
    /// * `t` - Year fraction from `baseDate`.
    /// * `method` - `"linear"` or `"log_linear"`; omitted uses the curve's `parInterp`.
    ///
    /// @returns Par spread in basis points.
    /// @throws If `method` is not a recognised label.
    #[wasm_bindgen(js_name = cdsQuoteBp)]
    pub fn cds_quote_bp(&self, t: JsValue, method: Option<JsValue>) -> Result<f64, JsValue> {
        let t = js_f64(&t, "t")?;
        let method = match js_opt_string(method.as_ref(), "method")? {
            Some(label) => parse_label(&label)?,
            None => self.inner.par_interp(),
        };
        Ok(self.inner.cds_quote_bp(t, method))
    }

    /// Copy of this curve with a different recovery rate (survival unchanged).
    ///
    /// # Arguments
    ///
    /// * `recovery_rate` - New recovery as a decimal fraction in `[0, 1]`.
    ///
    /// @returns A new `HazardCurve`.
    /// @throws If `recoveryRate` is outside `[0, 1]`.
    #[wasm_bindgen(js_name = withRecoveryRate)]
    pub fn with_recovery_rate(&self, recovery_rate: JsValue) -> Result<JsHazardCurve, JsValue> {
        let recovery_rate = js_f64(&recovery_rate, "recoveryRate")?;
        self.inner
            .with_recovery_rate(recovery_rate)
            .map(Self::from_inner)
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

    /// Recovery rate assumed on default.
    #[wasm_bindgen(getter, js_name = recoveryRate)]
    pub fn recovery_rate(&self) -> f64 {
        self.inner.recovery_rate()
    }

    /// Knots as a flat `[t0, lambda0, t1, lambda1, …]` array (years, decimal intensities).
    #[wasm_bindgen(getter, js_name = knotPoints)]
    pub fn knot_points(&self) -> Box<[f64]> {
        flatten_pairs(self.inner.knot_points())
    }

    /// Par CDS quotes as a flat `[t0, bp0, …]` array in basis points (may be empty).
    #[wasm_bindgen(getter, js_name = parSpreadPoints)]
    pub fn par_spread_points(&self) -> Box<[f64]> {
        flatten_pairs(self.inner.par_spread_points())
    }

    /// Day-count convention label (e.g. `"act_365f"`).
    #[wasm_bindgen(getter, js_name = dayCount)]
    pub fn day_count(&self) -> String {
        self.inner.day_count().to_string()
    }

    /// Currency of the protection leg, or `undefined`.
    #[wasm_bindgen(getter, js_name = currency)]
    pub fn currency(&self) -> Option<JsCurrency> {
        self.inner.currency().map(|inner| JsCurrency { inner })
    }

    /// Issuer name metadata, or `undefined`.
    #[wasm_bindgen(getter, js_name = issuer)]
    pub fn issuer(&self) -> Option<String> {
        self.inner.issuer().map(str::to_owned)
    }

    /// Debt seniority label (`"senior_secured"`, `"senior"`, `"subordinated"`,
    /// `"junior"`), or `undefined`.
    #[wasm_bindgen(getter, js_name = seniority)]
    pub fn seniority(&self) -> Option<String> {
        self.inner.seniority().map(|s| s.to_string())
    }

    /// Par-spread readout interpolation label (`"linear"` or `"log_linear"`).
    #[wasm_bindgen(getter, js_name = parInterp)]
    pub fn par_interp(&self) -> Result<String, JsValue> {
        label(&self.inner.par_interp())
    }

    /// Calibration recipe stamped on this curve by the calibration engine.
    ///
    /// The same object found under `hazard_calibration` in `toJson()`: the
    /// serialized hazard-curve parameters, the calibration and spread-risk
    /// quote inputs with their pillar dates and times, and the calibration
    /// configuration used for the original solve.
    ///
    /// @returns The `HazardCalibrationRecipe` object, or `undefined` for a curve that was built directly rather than produced by a calibration step.
    /// @throws FinstackError - If the recipe cannot be converted to a JavaScript object.
    #[wasm_bindgen(getter, js_name = hazardCalibration)]
    pub fn hazard_calibration(&self) -> Result<JsValue, JsValue> {
        match self.inner.hazard_calibration() {
            Some(recipe) => crate::utils::to_js_value(recipe),
            None => Ok(JsValue::UNDEFINED),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hazard_options() -> HazardCurveOptions {
        HazardCurveOptions {
            id: "ACME-HZD".into(),
            base_date: "2025-01-02".into(),
            knots: vec![1.0, 0.02, 5.0, 0.03],
            recovery_rate: 0.4,
            day_count: None,
            par_spreads: Some(vec![1.0, 120.0, 5.0, 180.0]),
            par_interp: Some("log_linear".into()),
            issuer: Some("ACME".into()),
            seniority: Some("senior".into()),
            currency: Some("USD".into()),
            max_hazard_rate: None,
        }
    }

    #[test]
    fn hazard_curve_options_reach_the_rust_builder() {
        let curve = JsHazardCurve::build(hazard_options()).expect("hazard curve");
        assert_eq!(&*curve.knot_points(), &[1.0, 0.02, 5.0, 0.03]);
        assert_eq!(&*curve.par_spread_points(), &[1.0, 120.0, 5.0, 180.0]);
        assert_eq!(curve.day_count(), "act_365f");
        assert_eq!(curve.issuer().as_deref(), Some("ACME"));
        assert_eq!(curve.seniority().as_deref(), Some("senior"));
        assert_eq!(
            curve.currency().map(|c| c.inner.to_string()).as_deref(),
            Some("USD")
        );
        assert_eq!(curve.par_interp().expect("label"), "log_linear");
    }

    #[test]
    fn hazard_curve_json_round_trips() {
        let curve = JsHazardCurve::build(hazard_options()).expect("hazard curve");
        let json = curve.to_json().expect("json");
        let back: RustHazardCurve = serde_json::from_str(&json).expect("parse");
        assert_eq!(serde_json::to_string(&back).expect("json"), json);
        assert!((back.sp(3.0) - curve.inner.sp(3.0)).abs() < 1e-15);
    }
}
