//! WASM bindings for `finstack_quant_core::market_data` term structures and FX.

use crate::utils::input::{
    from_js_json, js_f64, js_f64_seq, js_opt_f64_seq, js_opt_string, js_string, js_string_seq,
    js_uint, json_text,
};
use std::sync::Arc;

use crate::api::core::currency::JsCurrency;
use crate::utils::{date_to_iso, parse_iso_date, parse_iso_dates, to_js_err};
use finstack_quant_core::currency::Currency as RustCurrency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::surfaces::{
    FxDeltaVolSurface as RustFxDeltaVolSurface, SabrParameterData, VolCube as RustVolCube,
    VolInterpolationMode,
};
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve as RustDiscountCurve, ForwardCurve as RustForwardCurve,
    HazardCurve as RustHazardCurve, ValidationMode,
};
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_quant_core::money::fx::{
    fx_market_pair as rust_fx_market_pair, fx_pair_convention as rust_fx_pair_convention,
    fx_pip_size as rust_fx_pip_size, invert_fx_rate as rust_invert_fx_rate,
    FxConversionPolicy as RustFxConversionPolicy, FxMatrix as RustFxMatrix,
    FxPairConvention as RustFxPairConvention, FxQuery, FxQuoteConvention as RustFxQuoteConvention,
    FxRateResult as RustFxRateResult, SimpleFxProvider,
};
use finstack_quant_core::wire::{serde_label, serde_parse};
use js_sys::{Array, Float64Array};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

/// Parse a day-count string.
fn parse_day_count(s: &str) -> Result<DayCount, JsValue> {
    s.parse::<DayCount>().map_err(to_js_err)
}

/// Parse an interpolation style string.
fn parse_interp_style(s: &str) -> Result<InterpStyle, JsValue> {
    s.parse::<InterpStyle>().map_err(to_js_err)
}

/// Parse an extrapolation policy string.
fn parse_extrapolation(s: &str) -> Result<ExtrapolationPolicy, JsValue> {
    s.parse::<ExtrapolationPolicy>().map_err(to_js_err)
}

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
/// curve.zero(2.5);        // continuously-compounded zero rate at 2.5y
/// ```
#[wasm_bindgen(js_name = DiscountCurve)]
pub struct JsDiscountCurve {
    pub(crate) inner: Arc<RustDiscountCurve>,
}

/// Named constructor options for [`JsDiscountCurve`]; unknown keys are rejected.
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
    validation_mode: Option<String>,
    #[serde(default)]
    forward_floor: Option<f64>,
}

impl JsDiscountCurve {
    fn build(options: DiscountCurveOptions) -> Result<JsDiscountCurve, JsValue> {
        let base = parse_iso_date(&options.base_date)?;
        if !options.knots.len().is_multiple_of(2) {
            return Err(to_js_err("knots array must have even length (t, df pairs)"));
        }
        let pairs: Vec<(f64, f64)> = options
            .knots
            .chunks_exact(2)
            .map(|c| (c[0], c[1]))
            .collect();

        let mut builder = RustDiscountCurve::builder(options.id)
            .base_date(base)
            .knots(pairs);
        if let Some(interp) = options.interp.as_deref() {
            builder = builder.interp(parse_interp_style(interp)?);
        }
        if let Some(extrapolation) = options.extrapolation.as_deref() {
            builder = builder.extrapolation(parse_extrapolation(extrapolation)?);
        }
        if let Some(day_count) = options.day_count.as_deref() {
            builder = builder.day_count(parse_day_count(day_count)?);
        }
        builder = builder.validation(
            ValidationMode::from_preset(options.validation_mode.as_deref(), options.forward_floor)
                .map_err(to_js_err)?,
        );

        builder
            .build()
            .map(|curve| Self {
                inner: Arc::new(curve),
            })
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
    ///   array, `t` in years, `df` strictly positive, even length); and the
    ///   optional `interp` (`"linear"`, `"log_linear"`, `"monotone_convex"`,
    ///   `"cubic_hermite"`, `"piecewise_quadratic_forward"`), `extrapolation`
    ///   (`"flat_zero"`, `"flat_forward"`, or `"none"`, which returns NaN
    ///   outside the pillar range), `dayCount` (used to convert dates to curve
    ///   time; not inferred from the ID),
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
    /// misplaced or missing `forwardFloor`, or discount factors the curve
    /// validation rejects.
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
        let id: &str = &js_string(&id, "id")?;
        let base_date: &str = &js_string(&base_date, "baseDate")?;
        let curve = RustDiscountCurve::flat(id, parse_iso_date(base_date)?, continuous_rate)
            .map_err(to_js_err)?;
        Ok(Self {
            inner: Arc::new(curve),
        })
    }

    /// Discount factor at year fraction `t`.
    /// @param t - Time from the curve base date in years.
    pub fn df(&self, t: JsValue) -> Result<f64, JsValue> {
        let t = js_f64(&t, "t")?;
        Ok(self.inner.df(t))
    }

    /// Continuously-compounded zero rate at year fraction `t`.
    /// @param t - Time from the curve base date in years.
    pub fn zero(&self, t: JsValue) -> Result<f64, JsValue> {
        let t = js_f64(&t, "t")?;
        Ok(self.inner.zero(t))
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
}

/// Split a flat `[x0, y0, x1, y1, …]` array into `(x, y)` pairs.
///
/// # Errors
///
/// Returns a validation error naming `label` when the array has odd length.
fn flat_pairs(values: &[f64], label: &str) -> Result<Vec<(f64, f64)>, JsValue> {
    if !values.len().is_multiple_of(2) {
        return Err(to_js_err(format!(
            "{label} array must have even length (flat [x0, y0, x1, y1, …] pairs)"
        )));
    }
    Ok(values.chunks_exact(2).map(|c| (c[0], c[1])).collect())
}

/// Flatten `(x, y)` pairs into `[x0, y0, x1, y1, …]`.
fn flatten_pairs(points: impl Iterator<Item = (f64, f64)>) -> Box<[f64]> {
    points.flat_map(|(x, y)| [x, y]).collect()
}

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

/// Named constructor options for [`JsHazardCurve`]; unknown keys are rejected.
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
    interp: Option<String>,
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
    fn wrap(curve: RustHazardCurve) -> Self {
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
            builder = builder.day_count(parse_day_count(day_count)?);
        }
        if let Some(par_spreads) = options.par_spreads.as_deref() {
            builder = builder.par_spreads(flat_pairs(par_spreads, "parSpreads")?);
        }
        if let Some(interp) = options.interp.as_deref() {
            builder = builder.interp(parse_interp_style(interp)?);
        }
        if let Some(par_interp) = options.par_interp.as_deref() {
            builder = builder.par_interp(serde_parse(par_interp).map_err(to_js_err)?);
        }
        if let Some(issuer) = options.issuer {
            builder = builder.issuer(issuer);
        }
        if let Some(seniority) = options.seniority.as_deref() {
            builder = builder.seniority(serde_parse(seniority).map_err(to_js_err)?);
        }
        if let Some(currency) = options.currency.as_deref() {
            builder = builder.currency(currency.parse::<RustCurrency>().map_err(to_js_err)?);
        }
        if let Some(max_hazard_rate) = options.max_hazard_rate {
            builder = builder.max_hazard_rate(max_hazard_rate);
        }
        builder.build().map(Self::wrap).map_err(to_js_err)
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
    ///   basis points, kept for reporting), `interp` (survival interpolation;
    ///   only `"log_linear"` is accepted), `parInterp` (`"linear"` default or
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
        let base_date = js_string(&base_date, "baseDate")?;
        let hazard_rate = js_f64(&hazard_rate, "hazardRate")?;
        let recovery_rate = js_f64(&recovery_rate, "recoveryRate")?;
        RustHazardCurve::flat(id, parse_iso_date(&base_date)?, hazard_rate, recovery_rate)
            .map(Self::wrap)
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
        let base_date = js_string(&base_date, "baseDate")?;
        let points = flat_pairs(&js_f64_seq(&points, "points")?, "points")?;
        let recovery_rate = js_f64(&recovery_rate, "recoveryRate")?;
        RustHazardCurve::from_survival_probs(
            id,
            parse_iso_date(&base_date)?,
            &points,
            recovery_rate,
        )
        .map(Self::wrap)
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
        from_js_json::<RustHazardCurve>(&json, "json").map(Self::wrap)
    }

    /// Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid curve).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&*self.inner).map_err(to_js_err)
    }

    /// Survival probability `S(t)` at year fraction `t`.
    /// @param t - Time from the curve base date in years.
    /// @returns The probability of surviving from the base date through `t`, in `[0, 1]`.
    /// This operation does not throw.
    pub fn sp(&self, t: JsValue) -> Result<f64, JsValue> {
        let t = js_f64(&t, "t")?;
        Ok(self.inner.sp(t))
    }

    /// Instantaneous hazard rate `lambda(t)` at year fraction `t`.
    /// @param t - Time from the curve base date in years.
    /// @returns The annualized default intensity at `t`, expressed as a decimal rate.
    /// This operation does not throw.
    #[wasm_bindgen(js_name = hazardRate)]
    pub fn hazard_rate(&self, t: JsValue) -> Result<f64, JsValue> {
        let t = js_f64(&t, "t")?;
        Ok(self.inner.hazard_rate(t))
    }

    /// Survival probability on a date, measured with the curve day count.
    ///
    /// # Arguments
    ///
    /// * `date` - ISO-8601 target date on or after `baseDate`.
    ///
    /// @returns Survival probability in `(0, 1]`.
    /// @throws If `date` is not an ISO date or the year fraction cannot be computed.
    #[wasm_bindgen(js_name = spOnDate)]
    pub fn sp_on_date(&self, date: JsValue) -> Result<f64, JsValue> {
        let date = js_string(&date, "date")?;
        self.inner
            .sp_on_date(parse_iso_date(&date)?)
            .map_err(to_js_err)
    }

    /// Hazard rate (decimal per year) on a date, measured with the curve day count.
    ///
    /// # Arguments
    ///
    /// * `date` - ISO-8601 target date on or after `baseDate`.
    ///
    /// @returns Annual default intensity as a decimal.
    /// @throws If `date` is not an ISO date or the year fraction cannot be computed.
    #[wasm_bindgen(js_name = hazardRateOnDate)]
    pub fn hazard_rate_on_date(&self, date: JsValue) -> Result<f64, JsValue> {
        let date = js_string(&date, "date")?;
        self.inner
            .hazard_rate_on_date(parse_iso_date(&date)?)
            .map_err(to_js_err)
    }

    /// Survival probabilities on several dates.
    ///
    /// # Arguments
    ///
    /// * `dates` - ISO-8601 target dates on or after `baseDate`.
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
            Some(label) => serde_parse(&label).map_err(to_js_err)?,
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
            .map(Self::wrap)
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
        self.inner.seniority.map(|s| s.to_string())
    }

    /// Par-spread readout interpolation label (`"linear"` or `"log_linear"`).
    #[wasm_bindgen(getter, js_name = parInterp)]
    pub fn par_interp(&self) -> Result<String, JsValue> {
        serde_label(&self.inner.par_interp()).map_err(to_js_err)
    }
}

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

#[wasm_bindgen(js_class = ForwardCurve)]
impl JsForwardCurve {
    fn build(options: ForwardCurveOptions) -> Result<JsForwardCurve, JsValue> {
        let base = parse_iso_date(&options.base_date)?;

        if !options.knots.len().is_multiple_of(2) {
            return Err(to_js_err(
                "knots array must have even length (t, rate pairs)",
            ));
        }
        let pairs = options
            .knots
            .chunks_exact(2)
            .map(|c| (c[0], c[1]))
            .collect::<Vec<_>>();

        let mut builder = RustForwardCurve::builder(options.id, options.tenor)
            .base_date(base)
            .knots(pairs)
            .projection_grid_opt(options.projection_grid);
        if let Some(interp) = options.interp.as_deref() {
            builder = builder.interp(parse_interp_style(interp)?);
        }
        if let Some(extrapolation) = options.extrapolation.as_deref() {
            builder = builder.extrapolation(parse_extrapolation(extrapolation)?);
        }
        if let Some(day_count) = options.day_count.as_deref() {
            builder = builder.day_count(parse_day_count(day_count)?);
        }
        if let Some(reset_lag) = options.reset_lag {
            builder = builder.reset_lag(reset_lag);
        }

        builder
            .build()
            .map(|curve| Self {
                inner: Arc::new(curve),
            })
            .map_err(to_js_err)
    }

    /// Construct a forward curve from named options.
    ///
    /// # Arguments
    ///
    /// * `options` - ForwardCurveOptions object: curve id, tenor in years, ISO baseDate, flat time/decimal-rate knots, and optional dayCount, interp, extrapolation, projectionGrid and resetLag. Omitted policies use the Rust builder defaults; arrays and typed arrays are accepted.
    ///
    /// # Errors
    ///
    /// Throws Error when options cannot be decoded or canonical curve validation rejects dates, conventions, knots, tenor, reset lag, or projection grid.
    #[wasm_bindgen(constructor)]
    pub fn new(options: JsValue) -> Result<JsForwardCurve, JsValue> {
        let options = from_js_json(&options, "options")?;
        Self::build(options)
    }

    /// Forward rate at year fraction `t`.
    /// @param t - Time from the curve base date in years.
    #[wasm_bindgen(js_name = rate)]
    pub fn rate(&self, t: JsValue) -> Result<f64, JsValue> {
        let t = js_f64(&t, "t")?;
        Ok(self.inner.rate(t))
    }

    /// Discount-factor-implied simple forward over `(t1, t2)`.
    /// @param t1 - Earlier curve time in years used as the start of the forward interval.
    /// @param t2 - Later curve time in years used as the end of the forward interval.
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
        let base_date = js_string(&base_date, "baseDate")?;
        let rate = js_f64(&rate, "rate")?;
        RustForwardCurve::flat(id, tenor, parse_iso_date(&base_date)?, rate)
            .map(|curve| Self {
                inner: Arc::new(curve),
            })
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
        from_js_json::<RustForwardCurve>(&json, "json").map(|curve| Self {
            inner: Arc::new(curve),
        })
    }

    /// Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid curve).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&*self.inner).map_err(to_js_err)
    }

    /// Simple forward rate over `[t1, t2]` implied by the curve (Rust `rate_period`).
    ///
    /// # Arguments
    ///
    /// * `t1` - Start of the accrual period in years from `baseDate`.
    /// * `t2` - End of the accrual period in years from `baseDate`.
    ///
    /// @returns The average forward over the period as a decimal.
    #[wasm_bindgen(js_name = ratePeriod)]
    pub fn rate_period(&self, t1: JsValue, t2: JsValue) -> Result<f64, JsValue> {
        let t1 = js_f64(&t1, "t1")?;
        let t2 = js_f64(&t2, "t2")?;
        Ok(self.inner.rate_period(t1, t2))
    }

    /// Projection discount factor implied by the forwards at year fraction `t`.
    ///
    /// # Arguments
    ///
    /// * `t` - Time from `baseDate` in years.
    ///
    /// @returns Projection discount factor.
    /// @throws If the implied discount factor is non-finite or non-positive.
    pub fn df(&self, t: JsValue) -> Result<f64, JsValue> {
        let t = js_f64(&t, "t")?;
        self.inner.df(t).map_err(to_js_err)
    }

    /// Projection discount factor on a date, measured with the curve day count.
    ///
    /// # Arguments
    ///
    /// * `date` - ISO-8601 target date.
    ///
    /// @returns Projection discount factor.
    /// @throws If `date` is not an ISO date, the year fraction cannot be
    /// computed, or the implied discount factor is invalid.
    #[wasm_bindgen(js_name = dfOnDateCurve)]
    pub fn df_on_date_curve(&self, date: JsValue) -> Result<f64, JsValue> {
        let date = js_string(&date, "date")?;
        self.inner
            .df_on_date_curve(parse_iso_date(&date)?)
            .map_err(to_js_err)
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
}

/// Typed FX conversion policy wrapper for WASM callers.
#[wasm_bindgen(js_name = FxConversionPolicy)]
#[derive(Clone, Copy, Debug)]
pub struct JsFxConversionPolicy {
    inner: RustFxConversionPolicy,
}

#[wasm_bindgen(js_class = FxConversionPolicy)]
impl JsFxConversionPolicy {
    /// Use spot/forward on the cashflow date.
    #[wasm_bindgen(js_name = cashflowDate)]
    pub fn cashflow_date() -> Self {
        Self {
            inner: RustFxConversionPolicy::CashflowDate,
        }
    }

    /// Use period end date.
    #[wasm_bindgen(js_name = periodEnd)]
    pub fn period_end() -> Self {
        Self {
            inner: RustFxConversionPolicy::PeriodEnd,
        }
    }

    /// Use an average over the period.
    #[wasm_bindgen(js_name = periodAverage)]
    pub fn period_average() -> Self {
        Self {
            inner: RustFxConversionPolicy::PeriodAverage,
        }
    }

    /// Parse from a string label such as ``\"cashflow_date\"``.
    /// @param name - Policy label: `cashflow_date`, `period_end`, or `period_average`.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception unless `name` is `cashflow_date`,
    /// `period_end`, or `period_average`.
    #[wasm_bindgen(js_name = fromName)]
    pub fn from_name(name: JsValue) -> Result<Self, JsValue> {
        let name: &str = &js_string(&name, "name")?;
        Ok(Self {
            inner: name.parse().map_err(to_js_err)?,
        })
    }

    /// String form of the conversion policy.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}

/// Structured FX lookup result for WASM callers.
#[wasm_bindgen(js_name = FxRateResult)]
pub struct JsFxRateResult {
    inner: RustFxRateResult,
}

#[wasm_bindgen(js_class = FxRateResult)]
impl JsFxRateResult {
    /// The FX conversion rate.
    #[wasm_bindgen(getter, js_name = rate)]
    pub fn rate(&self) -> f64 {
        self.inner.rate
    }

    /// Whether the rate was obtained via triangulation.
    #[wasm_bindgen(getter, js_name = triangulated)]
    pub fn triangulated(&self) -> bool {
        self.inner.triangulated
    }

    /// Serialize to the canonical JSON wire form shared with Python `FxRateResult.to_json`.
    ///
    /// @returns Compact JSON text with `rate` and `triangulated`.
    /// @throws If serialization fails (not expected for a valid result).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Deserialize from the canonical JSON wire form produced by `toJson`.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical FxRateResult JSON text or plain object; unknown fields are rejected.
    ///
    /// @returns The parsed `FxRateResult`.
    /// @throws If `json` is malformed or has unknown or missing fields.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsFxRateResult, JsValue> {
        from_js_json::<RustFxRateResult>(&json, "json").map(|inner| JsFxRateResult { inner })
    }
}

/// USD quotation style for a market FX pair (Direct or Indirect versus USD).
///
/// **Direct** means USD is the quote currency (EURUSD, GBPUSD). **Indirect**
/// means USD is the base (USDJPY, USDCAD). Non-USD crosses inherit the USD
/// quotation of market CCY1 versus USD.
///
/// @example
/// ```javascript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const direct = core.FxQuoteConvention.direct();
/// direct.toString(); // "direct"
/// ```
#[wasm_bindgen(js_name = FxQuoteConvention)]
#[derive(Clone, Copy, Debug)]
pub struct JsFxQuoteConvention {
    inner: RustFxQuoteConvention,
}

#[wasm_bindgen(js_class = FxQuoteConvention)]
impl JsFxQuoteConvention {
    /// USD is the quote currency (units of USD per one unit of CCY1).
    pub fn direct() -> Self {
        Self {
            inner: RustFxQuoteConvention::Direct,
        }
    }

    /// USD is the base currency (units of CCY2 per one USD).
    pub fn indirect() -> Self {
        Self {
            inner: RustFxQuoteConvention::Indirect,
        }
    }

    /// Parse from a string label such as `"direct"` or `"indirect"`.
    /// @param name - Convention label: `direct` or `indirect`.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception unless `name` is `direct` or `indirect`.
    #[wasm_bindgen(js_name = fromName)]
    pub fn from_name(name: JsValue) -> Result<Self, JsValue> {
        let name: &str = &js_string(&name, "name")?;
        Ok(Self {
            inner: name.parse().map_err(to_js_err)?,
        })
    }

    /// String form of the USD quotation style (`"direct"` or `"indirect"`).
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}

/// Market convention for one FX pair after Bloomberg/Reuters CCY1 ordering.
///
/// Instances come from `fxPairConvention`. `base` / `quote` are always market
/// CCY1/CCY2, even when the lookup arguments were inverted.
///
/// @example
/// ```javascript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const conv = core.fxPairConvention("USD", "EUR");
/// conv.base.code;          // "EUR"
/// conv.usdQuotation.toString(); // "direct"
/// conv.pipSize;            // 0.0001
/// conv.settlementDays;     // 2
/// ```
#[wasm_bindgen(js_name = FxPairConvention)]
#[derive(Clone, Copy, Debug)]
pub struct JsFxPairConvention {
    inner: RustFxPairConvention,
}

#[wasm_bindgen(js_class = FxPairConvention)]
impl JsFxPairConvention {
    /// Market CCY1 (one unit of this currency in the screen pair).
    #[wasm_bindgen(getter, js_name = base)]
    pub fn base(&self) -> JsCurrency {
        JsCurrency {
            inner: self.inner.base,
        }
    }

    /// Market CCY2 (units of this currency per one unit of CCY1).
    #[wasm_bindgen(getter, js_name = quote)]
    pub fn quote(&self) -> JsCurrency {
        JsCurrency {
            inner: self.inner.quote,
        }
    }

    /// Direct if the USD leg quotes USD as CCY2; Indirect if USD is CCY1.
    #[wasm_bindgen(getter, js_name = usdQuotation)]
    pub fn usd_quotation(&self) -> JsFxQuoteConvention {
        JsFxQuoteConvention {
            inner: self.inner.usd_quotation,
        }
    }

    /// Pip size in outright-rate units (`0.01` or `0.0001`).
    #[wasm_bindgen(getter, js_name = pipSize)]
    pub fn pip_size(&self) -> f64 {
        self.inner.pip_size
    }

    /// Standard spot lag in business days (T+1 or T+2).
    #[wasm_bindgen(getter, js_name = settlementDays)]
    pub fn settlement_days(&self) -> u32 {
        self.inner.settlement_days
    }
}

/// Order two currencies into the market CCY1/CCY2 pair.
///
/// Priority is EUR > GBP > AUD > NZD > USD > other, with a stable ISO-4217
/// alphabetic tie-break when both sides share the same rank.
/// @param a - First currency ISO code of the unordered pair. Need not be market CCY1.
/// @param b - Second currency ISO code of the unordered pair. Need not be market CCY2.
/// @returns A two-element array `[CCY1, CCY2]` of `Currency` handles in market order.
///
/// # Errors
///
/// Throws a JavaScript exception if either code is not a recognized ISO-4217
/// alphabetic currency.
#[wasm_bindgen(js_name = fxMarketPair)]
pub fn fx_market_pair(a: JsValue, b: JsValue) -> Result<Array, JsValue> {
    let a: &str = &js_string(&a, "a")?;
    let b: &str = &js_string(&b, "b")?;
    let a: RustCurrency = a.parse().map_err(to_js_err)?;
    let b: RustCurrency = b.parse().map_err(to_js_err)?;
    let (base, quote) = rust_fx_market_pair(a, b);
    let out = Array::new();
    out.push(&JsCurrency { inner: base }.into());
    out.push(&JsCurrency { inner: quote }.into());
    Ok(out)
}

/// Market convention for an unordered currency pair.
///
/// Returned `base` / `quote` are always the market CCY1/CCY2, even when the
/// arguments are inverted.
/// @param base - One currency ISO code of the pair. Orientation is ignored.
/// @param quote - The other currency ISO code of the pair. Orientation is ignored.
/// @returns Market CCY1/CCY2, USD quotation, pip size, and standard spot lag.
///
/// # Errors
///
/// Throws a JavaScript exception if either code is not a recognized ISO-4217
/// alphabetic currency.
#[wasm_bindgen(js_name = fxPairConvention)]
pub fn fx_pair_convention(base: JsValue, quote: JsValue) -> Result<JsFxPairConvention, JsValue> {
    let base: &str = &js_string(&base, "base")?;
    let quote: &str = &js_string(&quote, "quote")?;
    let base: RustCurrency = base.parse().map_err(to_js_err)?;
    let quote: RustCurrency = quote.parse().map_err(to_js_err)?;
    Ok(JsFxPairConvention {
        inner: rust_fx_pair_convention(base, quote),
    })
}

/// Pip size in outright-rate units for a currency pair.
///
/// Returns `0.01` when either side is JPY, KRW, or HUF; otherwise `0.0001`.
/// Argument order does not matter.
/// @param base - One currency ISO code of the pair. Order is not significant.
/// @param quote - The other currency ISO code of the pair. Order is not significant.
/// @returns Pip size as a decimal increment of the outright FX rate.
///
/// # Errors
///
/// Throws a JavaScript exception if either code is not a recognized ISO-4217
/// alphabetic currency.
#[wasm_bindgen(js_name = fxPipSize)]
pub fn fx_pip_size(base: JsValue, quote: JsValue) -> Result<f64, JsValue> {
    let base: &str = &js_string(&base, "base")?;
    let quote: &str = &js_string(&quote, "quote")?;
    let base: RustCurrency = base.parse().map_err(to_js_err)?;
    let quote: RustCurrency = quote.parse().map_err(to_js_err)?;
    Ok(rust_fx_pip_size(base, quote))
}

/// Reciprocal of a strictly positive finite FX rate.
/// @param rate - Outright FX rate to invert, in quote-per-base units. Must be
/// finite and strictly positive; the reciprocal must also be a valid FX rate.
/// @returns `1 / rate` when that reciprocal is a valid FX rate.
///
/// # Errors
///
/// Throws a `validation` error if `rate` is non-finite, zero or negative, or
/// its reciprocal overflows.
#[wasm_bindgen(js_name = invertFxRate)]
pub fn invert_fx_rate(rate: JsValue) -> Result<f64, JsValue> {
    let rate = js_f64(&rate, "rate")?;
    rust_invert_fx_rate(rate).map_err(to_js_err)
}

/// Foreign-exchange rate matrix for currency conversion.
#[wasm_bindgen(js_name = FxMatrix)]
pub struct JsFxMatrix {
    inner: Arc<RustFxMatrix>,
}

impl Default for JsFxMatrix {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen(js_class = FxMatrix)]
impl JsFxMatrix {
    /// Create an empty FX matrix.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let matrix = RustFxMatrix::new(Arc::new(SimpleFxProvider::new()));
        Self {
            inner: Arc::new(matrix),
        }
    }

    /// Set an explicit FX quote.
    ///
    /// # Arguments
    /// * `base` - Base (from) currency ISO code.
    /// * `quote` - Quote (to) currency ISO code.
    /// * `rate` - Conversion rate.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if either currency code is invalid or
    /// `rate` is non-finite or not strictly positive.
    #[wasm_bindgen(js_name = setQuote)]
    pub fn set_quote(&self, base: JsValue, quote: JsValue, rate: JsValue) -> Result<(), JsValue> {
        let rate = js_f64(&rate, "rate")?;
        let base: &str = &js_string(&base, "base")?;
        let quote: &str = &js_string(&quote, "quote")?;
        let base_currency: RustCurrency = base.parse().map_err(to_js_err)?;
        let quote_currency: RustCurrency = quote.parse().map_err(to_js_err)?;
        self.inner
            .set_quote(base_currency, quote_currency, rate)
            .map_err(to_js_err)?;
        Ok(())
    }

    /// Set an authoritative quote scoped to one date and conversion policy.
    /// Pair-global quotes in either orientation take priority; pinned quotes
    /// precede provider observations.
    /// @param base - Base currency code of the FX quote, where the rate is quote per base.
    /// @param quote - Quote currency code of the FX rate, expressed per unit of base currency.
    /// @param date - ISO-8601 date used by the calculation or market-data lookup.
    /// @param policy - FX quote-selection policy for resolving direct, inverse, or triangulated rates.
    /// @param rate - Finite positive quote-currency units per one base-currency unit.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if either currency code is invalid, `date`
    /// is not a valid ISO date, or `rate` is non-finite or not strictly positive.
    #[wasm_bindgen(js_name = setQuoteOn)]
    pub fn set_quote_on(
        &self,
        base: JsValue,
        quote: JsValue,
        date: JsValue,
        policy: &JsFxConversionPolicy,
        rate: JsValue,
    ) -> Result<(), JsValue> {
        let rate = js_f64(&rate, "rate")?;
        let base: &str = &js_string(&base, "base")?;
        let quote: &str = &js_string(&quote, "quote")?;
        let date: &str = &js_string(&date, "date")?;
        let base_currency: RustCurrency = base.parse().map_err(to_js_err)?;
        let quote_currency: RustCurrency = quote.parse().map_err(to_js_err)?;
        let d = parse_iso_date(date)?;
        self.inner
            .set_quote_on(base_currency, quote_currency, d, policy.inner, rate)
            .map_err(to_js_err)
    }

    /// Look up an FX rate.
    /// Global quotes precede pinned fixings, then provider observations,
    /// resolving source priority before taking a reciprocal.
    ///
    /// The published facade makes `policy` optional: when omitted it calls
    /// the Rust default query (`FxQuery::new`, cashflow-date policy).
    ///
    /// # Arguments
    /// * `base` - Base (from) currency ISO code.
    /// * `quote` - Quote (to) currency ISO code.
    /// * `date` - ISO date string.
    /// * `policy` - Reusable conversion policy handle; omitted means the Rust
    ///   default (cashflow date).
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if either currency code or `date` is invalid,
    /// no direct, inverse, or triangulated quote is available, or a resolved quote
    /// is non-finite or non-positive.
    pub fn rate(
        &self,
        base: JsValue,
        quote: JsValue,
        date: JsValue,
        policy: &JsFxConversionPolicy,
    ) -> Result<JsFxRateResult, JsValue> {
        let base: &str = &js_string(&base, "base")?;
        let quote: &str = &js_string(&quote, "quote")?;
        let date: &str = &js_string(&date, "date")?;
        let base_currency: RustCurrency = base.parse().map_err(to_js_err)?;
        let quote_currency: RustCurrency = quote.parse().map_err(to_js_err)?;
        let d = parse_iso_date(date)?;
        let query = FxQuery::with_policy(base_currency, quote_currency, d, policy.inner);
        let result = self.inner.rate(query).map_err(to_js_err)?;
        Ok(JsFxRateResult { inner: result })
    }

    /// Look up an FX rate with the Rust default query (`FxQuery::new`).
    ///
    /// Raw-package implementation of `rate(base, quote, date)` without a
    /// policy; the published facade folds it into `rate` and removes this
    /// name from the public prototype.
    /// @param base - Base currency code of the FX quote, where the rate is quote per base.
    /// @param quote - Quote currency code of the FX rate, expressed per unit of base currency.
    /// @param date - ISO-8601 date used by the calculation or market-data lookup.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if either currency code or `date` is invalid,
    /// no direct, inverse, or triangulated quote is available, or a resolved
    /// quote is non-finite or non-positive.
    #[wasm_bindgen(js_name = rateWithDefaultPolicy)]
    pub fn rate_with_default_policy(
        &self,
        base: JsValue,
        quote: JsValue,
        date: JsValue,
    ) -> Result<JsFxRateResult, JsValue> {
        let base: &str = &js_string(&base, "base")?;
        let quote: &str = &js_string(&quote, "quote")?;
        let date: &str = &js_string(&date, "date")?;
        let base_currency: RustCurrency = base.parse().map_err(to_js_err)?;
        let quote_currency: RustCurrency = quote.parse().map_err(to_js_err)?;
        let d = parse_iso_date(date)?;
        let query = FxQuery::new(base_currency, quote_currency, d);
        self.inner
            .rate(query)
            .map(|inner| JsFxRateResult { inner })
            .map_err(to_js_err)
    }
}

/// SABR volatility cube for swaption pricing.
///
/// Stores calibrated SABR parameters on an expiry × tenor grid and evaluates
/// implied volatilities via bilinear parameter interpolation followed by the
/// Hagan (2002) approximation.
#[wasm_bindgen(js_name = VolCube)]
pub struct JsVolCube {
    pub(crate) inner: Arc<RustVolCube>,
}

#[wasm_bindgen(js_class = VolCube)]
impl JsVolCube {
    /// Construct a vol cube from a flat SABR parameter array.
    ///
    /// # Arguments
    /// * `id` - Curve identifier.
    /// * `expiries` - Option expiry axis in years (strictly increasing).
    /// * `tenors` - Swap tenor axis in years (strictly increasing).
    /// * `params_flat` - Row-major flat array of SABR parameters:
    ///   `[alpha0, beta0, rho0, nu0, shift0, alpha1, …]`.
    ///   Length must equal `expiries.len() * tenors.len() * 5`.
    ///   Pass `NaN` for the shift element of a node to omit the shift.
    /// * `forwards` - Row-major forward rates, one per grid node.
    /// @param interpolation_mode - Interpolation across the expiry axis: `"vol"` or
    /// `"total_variance"`; omitted keeps the Rust `VolCube::from_grid` default (`"vol"`).
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if an axis is empty, non-finite,
    /// non-positive, or not strictly increasing; the parameter or forward array
    /// has the wrong length; a forward is non-finite; any SABR node has invalid
    /// alpha, beta, rho, nu, or shift; or `interpolationMode` is neither `vol`
    /// nor `total_variance`.
    #[wasm_bindgen(constructor)]
    pub fn new(
        id: JsValue,
        expiries: JsValue,
        tenors: JsValue,
        params_flat: JsValue,
        forwards: JsValue,
        interpolation_mode: Option<JsValue>,
    ) -> Result<JsVolCube, JsValue> {
        let expiries: &[f64] = &js_f64_seq(&expiries, "expiries")?;
        let tenors: &[f64] = &js_f64_seq(&tenors, "tenors")?;
        let params_flat: &[f64] = &js_f64_seq(&params_flat, "paramsFlat")?;
        let forwards: &[f64] = &js_f64_seq(&forwards, "forwards")?;
        let id: &str = &js_string(&id, "id")?;
        let interpolation_mode = js_opt_string(interpolation_mode.as_ref(), "interpolationMode")?;
        let n_nodes = expiries.len() * tenors.len();
        if params_flat.len() != n_nodes * 5 {
            return Err(to_js_err(format!(
                "params_flat length {} != {} nodes * 5 params",
                params_flat.len(),
                n_nodes
            )));
        }
        let mut sabr_params = Vec::with_capacity(n_nodes);
        for i in 0..n_nodes {
            let base = i * 5;
            let shift = params_flat[base + 4];
            let shift = if shift.is_nan() { None } else { Some(shift) };
            let p = SabrParameterData::new_with_shift(
                params_flat[base],     // alpha
                params_flat[base + 1], // beta
                params_flat[base + 2], // rho
                params_flat[base + 3], // nu
                shift,
            )
            .map_err(to_js_err)?;
            sabr_params.push(p);
        }
        let mut cube = RustVolCube::from_grid(id, expiries, tenors, &sabr_params, forwards)
            .map_err(to_js_err)?;
        if let Some(mode) = interpolation_mode.as_deref() {
            let mode: VolInterpolationMode =
                finstack_quant_core::wire::serde_parse(mode).map_err(to_js_err)?;
            cube = cube.with_interpolation_mode(mode);
        }
        Ok(Self {
            inner: Arc::new(cube),
        })
    }

    /// Deserialize a canonical SABR cube state without flattening parameter nodes.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical VolCube JSON containing id, expiry and tenor axes in years,
    ///   row-major SABR nodes and decimal-rate forwards, and interpolation_mode.
    ///   Missing or null node shifts remain absent; unknown fields are rejected.
    /// @returns A validated VolCube handle owned by the caller; release it with free().
    /// @throws Error - Throws when JSON is malformed, fields are unknown, or native axis, parameter, or forward validation fails.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsVolCube, JsValue> {
        let json: &str = &json_text(&json, "json")?;
        let inner = serde_json::from_str::<RustVolCube>(json).map_err(to_js_err)?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Interpolation contract used across the expiry axis.
    #[wasm_bindgen(getter, js_name = interpolationMode)]
    pub fn interpolation_mode(&self) -> Result<String, JsValue> {
        finstack_quant_core::wire::serde_label(&self.inner.interpolation_mode()).map_err(to_js_err)
    }

    /// Cube identifier.
    #[wasm_bindgen(getter, js_name = id)]
    pub fn id(&self) -> String {
        self.inner.id().as_str().to_string()
    }

    /// Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid cube).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&*self.inner).map_err(to_js_err)
    }

    /// Option expiry axis in years.
    #[wasm_bindgen(getter, js_name = expiries)]
    pub fn expiries(&self) -> Box<[f64]> {
        self.inner.expiries().into()
    }

    /// Underlying swap tenor axis in years.
    #[wasm_bindgen(getter, js_name = tenors)]
    pub fn tenors(&self) -> Box<[f64]> {
        self.inner.tenors().into()
    }

    /// Grid shape as `[nExpiries, nTenors]`.
    #[wasm_bindgen(getter, js_name = gridShape)]
    pub fn grid_shape(&self) -> Result<Box<[u32]>, JsValue> {
        let (n_exp, n_ten) = self.inner.grid_shape();
        let n_exp = u32::try_from(n_exp).map_err(|e| to_js_err(e.to_string()))?;
        let n_ten = u32::try_from(n_ten).map_err(|e| to_js_err(e.to_string()))?;
        Ok(Box::new([n_exp, n_ten]))
    }

    /// Row-major forward rates (decimals), one per grid node.
    #[wasm_bindgen(getter, js_name = forwards)]
    pub fn forwards(&self) -> Box<[f64]> {
        self.inner.forwards().into()
    }

    /// Row-major SABR nodes as plain `{alpha, beta, rho, nu, shift?}` objects.
    ///
    /// @returns One object per grid node in row-major (expiry, tenor) order.
    /// @throws If serialization fails (not expected for a valid cube).
    #[wasm_bindgen(getter, js_name = params)]
    pub fn params(&self) -> Result<JsValue, JsValue> {
        crate::utils::to_js_value(&self.inner.params())
    }

    /// SABR parameters at grid indices, as a plain `{alpha, beta, rho, nu, shift?}` object.
    ///
    /// # Arguments
    ///
    /// * `exp_idx` - Zero-based index into `expiries`.
    /// * `tenor_idx` - Zero-based index into `tenors`.
    ///
    /// @returns The node's SABR parameters.
    /// @throws `TypeError` if an index is not a non-negative integer;
    /// `FinstackError` (kind `validation`) if it lies outside `gridShape`.
    #[wasm_bindgen(js_name = paramsAt)]
    pub fn params_at(&self, exp_idx: JsValue, tenor_idx: JsValue) -> Result<JsValue, JsValue> {
        let exp_idx: usize = js_uint(&exp_idx, "expIdx")?;
        let tenor_idx: usize = js_uint(&tenor_idx, "tenorIdx")?;
        let params = self
            .inner
            .params_at(exp_idx, tenor_idx)
            .map_err(to_js_err)?;
        crate::utils::to_js_value(params)
    }

    /// Forward rate (decimal) at grid indices.
    ///
    /// # Arguments
    ///
    /// * `exp_idx` - Zero-based index into `expiries`.
    /// * `tenor_idx` - Zero-based index into `tenors`.
    ///
    /// @returns The node's forward rate.
    /// @throws `TypeError` if an index is not a non-negative integer;
    /// `FinstackError` (kind `validation`) if it lies outside `gridShape`.
    #[wasm_bindgen(js_name = forwardAt)]
    pub fn forward_at(&self, exp_idx: JsValue, tenor_idx: JsValue) -> Result<f64, JsValue> {
        let exp_idx: usize = js_uint(&exp_idx, "expIdx")?;
        let tenor_idx: usize = js_uint(&tenor_idx, "tenorIdx")?;
        self.inner.forward_at(exp_idx, tenor_idx).map_err(to_js_err)
    }
}

/// FX vol surface quoted in **delta space** (ATM, 25-delta RR/BF, optional
/// 10-delta wings).
///
/// Stores market-standard FX delta quotes (Wystup 2006, Clark 2011). Use
/// `models.volatility` (`getFxDeltaVol`, `getFxDeltaPillarVols`) for
/// evaluation; this type does not convert quotes to strikes itself.
/// The delta convention is **forward delta (premium-unadjusted)**.
#[wasm_bindgen(js_name = FxDeltaVolSurface)]
pub struct JsFxDeltaVolSurface {
    pub(crate) inner: Arc<RustFxDeltaVolSurface>,
}

#[wasm_bindgen(js_class = FxDeltaVolSurface)]
impl JsFxDeltaVolSurface {
    /// Construct an FX delta-quoted vol surface with 25-delta wings.
    ///
    /// Optional `rr10d` / `bf10d` add 10-delta wings for richer wing
    /// interpolation. Omit both (`undefined`/`null`) for a three-point smile;
    /// the Rust constructor rejects one without the other.
    ///
    /// # Arguments
    /// * `id`        - Stable surface identifier.
    /// * `expiries`  - Strictly increasing positive expiry times (years).
    /// * `atm_vols`  - ATM delta-neutral straddle vols per expiry.
    /// * `rr25d`     - 25-delta risk reversal per expiry (call vol − put vol).
    /// * `bf25d`     - 25-delta butterfly per expiry (wing avg − ATM).
    /// * `rr10d`     - Optional 10-delta risk reversal per expiry.
    /// * `bf10d`     - Optional 10-delta butterfly per expiry.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if `rr10d` and `bf10d` are not both present
    /// or both absent; quote arrays are empty or have mismatched lengths;
    /// expiries are not finite, positive, and strictly increasing; ATM vols are
    /// not finite and positive; or any risk reversal or butterfly is non-finite.
    #[wasm_bindgen(constructor)]
    pub fn new(
        id: JsValue,
        expiries: JsValue,
        atm_vols: JsValue,
        rr25d: JsValue,
        bf25d: JsValue,
        rr10d: Option<JsValue>,
        bf10d: Option<JsValue>,
    ) -> Result<JsFxDeltaVolSurface, JsValue> {
        let expiries: &[f64] = &js_f64_seq(&expiries, "expiries")?;
        let atm_vols: &[f64] = &js_f64_seq(&atm_vols, "atmVols")?;
        let rr25d: &[f64] = &js_f64_seq(&rr25d, "rr25d")?;
        let bf25d: &[f64] = &js_f64_seq(&bf25d, "bf25d")?;
        let rr10d = js_opt_f64_seq(rr10d.as_ref(), "rr10d")?;
        let bf10d = js_opt_f64_seq(bf10d.as_ref(), "bf10d")?;
        let id: &str = &js_string(&id, "id")?;
        let surface = RustFxDeltaVolSurface::new(
            id,
            expiries.to_vec(),
            atm_vols.to_vec(),
            rr25d.to_vec(),
            bf25d.to_vec(),
            rr10d,
            bf10d,
        )
        .map_err(to_js_err)?;
        Ok(Self {
            inner: Arc::new(surface),
        })
    }

    /// Deserialize canonical FX delta quotes without reconstructing positional arrays.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical FxDeltaVolSurface JSON with expiries in years and
    ///   annualized decimal ATM, risk-reversal, and butterfly quotes. Optional
    ///   10-delta wings must occur together; unknown fields are rejected.
    /// @returns A validated FxDeltaVolSurface handle owned by the caller; release it with free().
    /// @throws Error - Throws when JSON is malformed, fields are unknown, or native expiry, quote, or wing validation fails.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsFxDeltaVolSurface, JsValue> {
        let json: &str = &json_text(&json, "json")?;
        let inner = serde_json::from_str::<RustFxDeltaVolSurface>(json).map_err(to_js_err)?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Surface identifier.
    #[wasm_bindgen(getter, js_name = id)]
    pub fn id(&self) -> String {
        self.inner.id().as_str().to_string()
    }

    /// Expiry axis in years.
    #[wasm_bindgen(getter, js_name = expiries)]
    pub fn expiries(&self) -> Box<[f64]> {
        self.inner.expiries().into()
    }

    /// Number of expiry pillars.
    #[wasm_bindgen(getter, js_name = numExpiries)]
    pub fn num_expiries(&self) -> usize {
        self.inner.num_expiries()
    }

    /// Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid surface).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&*self.inner).map_err(to_js_err)
    }

    /// ATM delta-neutral straddle vols per expiry (decimals).
    #[wasm_bindgen(getter, js_name = atmVols)]
    pub fn atm_vols(&self) -> Box<[f64]> {
        self.inner.atm_vols().into()
    }

    /// 25-delta risk reversals per expiry (call vol minus put vol, decimals).
    #[wasm_bindgen(getter, js_name = rr25d)]
    pub fn rr_25d(&self) -> Box<[f64]> {
        self.inner.rr_25d().into()
    }

    /// 25-delta butterflies per expiry (wing average minus ATM, decimals).
    #[wasm_bindgen(getter, js_name = bf25d)]
    pub fn bf_25d(&self) -> Box<[f64]> {
        self.inner.bf_25d().into()
    }

    /// 10-delta risk reversals per expiry, or `undefined` without 10-delta wings.
    #[wasm_bindgen(getter, js_name = rr10d)]
    pub fn rr_10d(&self) -> Option<Box<[f64]>> {
        self.inner.rr_10d().map(Into::into)
    }

    /// 10-delta butterflies per expiry, or `undefined` without 10-delta wings.
    #[wasm_bindgen(getter, js_name = bf10d)]
    pub fn bf_10d(&self) -> Option<Box<[f64]>> {
        self.inner.bf_10d().map(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::{DayCount, Month};
    use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};

    #[test]
    fn parse_iso_date_components_and_roundtrip() {
        let d = parse_iso_date("2024-01-15").expect("valid ISO date");
        assert_eq!(d.year(), 2024);
        assert_eq!(d.month(), Month::January);
        assert_eq!(d.day(), 15);
        assert_eq!(date_to_iso(d), "2024-01-15");
    }

    #[test]
    fn date_to_iso_roundtrips_parse() {
        let s = "2024-06-30";
        let d = parse_iso_date(s).expect("valid ISO date");
        assert_eq!(date_to_iso(d), s);
    }

    #[test]
    fn parse_day_count_act_variants() {
        assert_eq!(
            parse_day_count("act_365f").expect("act_365f"),
            DayCount::Act365F
        );
        assert_eq!(
            parse_day_count("act_360").expect("act_360"),
            DayCount::Act360
        );
    }

    #[test]
    fn parse_interp_style_variants() {
        assert_eq!(
            parse_interp_style("linear").expect("linear"),
            InterpStyle::Linear
        );
        assert_eq!(
            parse_interp_style("monotone_convex").expect("monotone_convex"),
            InterpStyle::MonotoneConvex
        );
    }

    #[test]
    fn parse_extrapolation_variants() {
        assert_eq!(
            parse_extrapolation("flat_forward").expect("flat_forward"),
            ExtrapolationPolicy::FlatForward
        );
        assert!("flat".parse::<ExtrapolationPolicy>().is_err());
    }

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

    fn hazard_options() -> HazardCurveOptions {
        HazardCurveOptions {
            id: "ACME-HZD".into(),
            base_date: "2025-01-02".into(),
            knots: vec![1.0, 0.02, 5.0, 0.03],
            recovery_rate: 0.4,
            day_count: None,
            par_spreads: Some(vec![1.0, 120.0, 5.0, 180.0]),
            interp: None,
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

    // JsVolCube tests require a WASM runtime (JsValue) — run via wasm-pack test.
}
