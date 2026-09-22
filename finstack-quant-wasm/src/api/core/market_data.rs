//! WASM bindings for `finstack_quant_core::market_data` term structures and FX.

use std::sync::Arc;

use crate::api::core::currency::JsCurrency;
use crate::utils::{date_to_iso, parse_iso_date, to_js_err};
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
/// const curve = new core.DiscountCurve(
///   "USD-OIS",
///   "2025-01-02",
///   [0.0, 1.0, 1.0, 0.95, 5.0, 0.78],
///   "monotone_convex",
///   "flat_forward",
///   "act_365f",
/// );
/// curve.df(2.5);          // discount factor at 2.5y
/// curve.zero(2.5);        // continuously-compounded zero rate at 2.5y
/// ```
#[wasm_bindgen(js_name = DiscountCurve)]
pub struct JsDiscountCurve {
    pub(crate) inner: Arc<RustDiscountCurve>,
}

#[wasm_bindgen(js_class = DiscountCurve)]
impl JsDiscountCurve {
    /// Construct from an array of `[time, df]` pairs.
    ///
    /// @param id - Curve identifier (e.g. `"USD-OIS"`). Used as the lookup
    /// key inside a `MarketContext`.
    /// @param baseDate - ISO-8601 date string (`"YYYY-MM-DD"`). All `time`
    /// values are interpreted as year fractions from this date under
    /// `dayCount`.
    /// @param knots - Flat `[t0, df0, t1, df1, …]` array. `t` in years,
    /// `df` strictly positive. Length must be even.
    /// @param interp - Interpolation style. When omitted, the Rust builder
    /// default (`"monotone_convex"`) applies. One of `"linear"`,
    /// `"log_linear"`, `"monotone_convex"`, `"cubic_hermite"`,
    /// `"piecewise_quadratic_forward"`.
    /// @param extrapolation - Extrapolation policy. When omitted, the Rust
    /// builder default (`"flat_forward"`) applies. One of `"flat_zero"`,
    /// `"flat_forward"`, `"nan"`.
    /// @param dayCount - Day-count convention (defaults to curve-ID inference).
    /// @param validationMode - Rust validation preset: `"market_standard"`
    /// (default) or `"negative_rate_friendly"`.
    /// @param forwardFloor - Required minimum implied forward when using
    /// `"negative_rate_friendly"`.
    /// @returns The constructed `DiscountCurve`.
    /// @throws If `knots` length is odd, the date is malformed, the
    /// interpolation style is unknown, or any `df` is non-positive.
    #[wasm_bindgen(constructor)]
    #[expect(
        clippy::too_many_arguments,
        reason = "preserves existing positional constructor arguments and appends validation options compatibly"
    )]
    pub fn new(
        id: &str,
        base_date: &str,
        knots: &[f64],
        interp: Option<String>,
        extrapolation: Option<String>,
        day_count: Option<String>,
        validation_mode: Option<String>,
        forward_floor: Option<f64>,
    ) -> Result<JsDiscountCurve, JsValue> {
        let base = parse_iso_date(base_date)?;
        if !knots.len().is_multiple_of(2) {
            return Err(to_js_err("knots array must have even length (t, df pairs)"));
        }
        let pairs: Vec<(f64, f64)> = knots.chunks_exact(2).map(|c| (c[0], c[1])).collect();

        let mut builder = RustDiscountCurve::builder(id).base_date(base).knots(pairs);
        if let Some(ref s) = interp {
            builder = builder.interp(parse_interp_style(s)?);
        }
        if let Some(ref s) = extrapolation {
            builder = builder.extrapolation(parse_extrapolation(s)?);
        }
        if let Some(ref s) = day_count {
            builder = builder.day_count(parse_day_count(s)?);
        }
        builder = builder.validation(
            ValidationMode::from_preset(
                validation_mode.as_deref().unwrap_or("market_standard"),
                forward_floor,
            )
            .map_err(to_js_err)?,
        );

        let curve = builder.build().map_err(to_js_err)?;

        Ok(Self {
            inner: Arc::new(curve),
        })
    }

    /// Construct a flat continuously-compounded discount curve.
    /// @param id - Curve identifier stored on the constructed discount curve.
    /// @param base_date - ISO-8601 curve base date from which time coordinates are measured.
    /// @param continuous_rate - Flat continuously compounded zero rate expressed as a decimal.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if `baseDate` is not a valid ISO date,
    /// `continuousRate` is non-finite, or the implied discount factors are not
    /// finite and strictly positive.
    #[wasm_bindgen(js_name = flat)]
    pub fn flat(
        id: &str,
        base_date: &str,
        continuous_rate: f64,
    ) -> Result<JsDiscountCurve, JsValue> {
        let curve = RustDiscountCurve::flat(id, parse_iso_date(base_date)?, continuous_rate)
            .map_err(to_js_err)?;
        Ok(Self {
            inner: Arc::new(curve),
        })
    }

    /// Discount factor at year fraction `t`.
    /// @param t - Time from the curve base date in years.
    pub fn df(&self, t: f64) -> f64 {
        self.inner.df(t)
    }

    /// Continuously-compounded zero rate at year fraction `t`.
    /// @param t - Time from the curve base date in years.
    pub fn zero(&self, t: f64) -> f64 {
        self.inner.zero(t)
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
    pub fn forward(&self, t1: f64, t2: f64) -> Result<f64, JsValue> {
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
/// const hz = new core.HazardCurve(
///   "ACME-HZD",
///   "2025-01-02",
///   [0.0, 0.02, 30.0, 0.02],
///   0.4,
/// );
/// hz.sp(5.0);          // survival probability at 5y
/// hz.hazardRate(5.0);  // instantaneous hazard rate at 5y
/// ```
#[wasm_bindgen(js_name = HazardCurve)]
pub struct JsHazardCurve {
    pub(crate) inner: Arc<RustHazardCurve>,
}

#[wasm_bindgen(js_class = HazardCurve)]
impl JsHazardCurve {
    /// Construct from an array of `[time, hazardRate]` pairs.
    ///
    /// @param id - Curve identifier (e.g. `"ACME-HZD"`).
    /// @param baseDate - ISO-8601 date string (`"YYYY-MM-DD"`). All `time`
    /// values are year fractions from this date under `dayCount`.
    /// @param knots - Flat `[t0, lambda0, t1, lambda1, …]` array. `t` in
    /// years, `lambda` a non-negative intensity. Length must be even.
    /// @param recoveryRate - Required recovery on default as a decimal fraction in `[0, 1]`.
    /// @param dayCount - Day-count convention (default `"act_365f"`).
    /// @returns The constructed `HazardCurve`.
    /// @throws If `recoveryRate` is missing, non-finite, or outside `[0, 1]`,
    /// `knots` length is odd, the date is malformed, the day-count is unknown,
    /// or the curve otherwise fails validation.
    #[wasm_bindgen(constructor)]
    pub fn new(
        id: &str,
        base_date: &str,
        knots: &[f64],
        recovery_rate: f64,
        day_count: Option<String>,
    ) -> Result<JsHazardCurve, JsValue> {
        let base = parse_iso_date(base_date)?;
        if !knots.len().is_multiple_of(2) {
            return Err(to_js_err(
                "knots array must have even length (t, hazardRate pairs)",
            ));
        }
        if !recovery_rate.is_finite() || !(0.0..=1.0).contains(&recovery_rate) {
            return Err(to_js_err(
                "recoveryRate is required and must be a finite decimal in [0, 1]",
            ));
        }
        let pairs: Vec<(f64, f64)> = knots.chunks_exact(2).map(|c| (c[0], c[1])).collect();
        let mut builder = RustHazardCurve::builder(id)
            .base_date(base)
            .knots(pairs)
            .recovery_rate(recovery_rate);
        if let Some(ref day_count) = day_count {
            builder = builder.day_count(parse_day_count(day_count)?);
        }
        let curve = builder.build().map_err(to_js_err)?;

        Ok(Self {
            inner: Arc::new(curve),
        })
    }

    /// Survival probability `S(t)` at year fraction `t`.
    /// @param t - Time from the curve base date in years.
    /// @returns The probability of surviving from the base date through `t`, in `[0, 1]`.
    /// This operation does not throw.
    pub fn sp(&self, t: f64) -> f64 {
        self.inner.sp(t)
    }

    /// Instantaneous hazard rate `lambda(t)` at year fraction `t`.
    /// @param t - Time from the curve base date in years.
    /// @returns The annualized default intensity at `t`, expressed as a decimal rate.
    /// This operation does not throw.
    #[wasm_bindgen(js_name = hazardRate)]
    pub fn hazard_rate(&self, t: f64) -> f64 {
        self.inner.hazard_rate(t)
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
        let options = serde_wasm_bindgen::from_value(options).map_err(to_js_err)?;
        Self::build(options)
    }

    /// Forward rate at year fraction `t`.
    /// @param t - Time from the curve base date in years.
    #[wasm_bindgen(js_name = rate)]
    pub fn rate(&self, t: f64) -> f64 {
        self.inner.rate(t)
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
    pub fn rate_between(&self, t1: f64, t2: f64) -> Result<f64, JsValue> {
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
    pub fn from_name(name: &str) -> Result<Self, JsValue> {
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
    pub fn from_name(name: &str) -> Result<Self, JsValue> {
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
/// conv.spotLagDays;        // 2
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
    #[wasm_bindgen(getter, js_name = spotLagDays)]
    pub fn spot_lag_days(&self) -> u32 {
        self.inner.spot_lag_days
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
pub fn fx_market_pair(a: &str, b: &str) -> Result<Array, JsValue> {
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
pub fn fx_pair_convention(base: &str, quote: &str) -> Result<JsFxPairConvention, JsValue> {
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
pub fn fx_pip_size(base: &str, quote: &str) -> Result<f64, JsValue> {
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
/// Throws a JavaScript exception if `rate` is non-finite, non-positive, or
/// when `1 / rate` is not a usable FX rate (overflow to infinity, zero, or a
/// negative value).
#[wasm_bindgen(js_name = invertFxRate)]
pub fn invert_fx_rate(rate: f64) -> Result<f64, JsValue> {
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
    pub fn set_quote(&self, base: &str, quote: &str, rate: f64) -> Result<(), JsValue> {
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
        base: &str,
        quote: &str,
        date: &str,
        policy: &JsFxConversionPolicy,
        rate: f64,
    ) -> Result<(), JsValue> {
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
    /// # Arguments
    /// * `base` - Base (from) currency ISO code.
    /// * `quote` - Quote (to) currency ISO code.
    /// * `date` - ISO date string.
    /// * `policy` - Reusable conversion policy handle.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if either currency code or `date` is invalid,
    /// no direct, inverse, or triangulated quote is available, or a resolved quote
    /// is non-finite or non-positive.
    pub fn rate(
        &self,
        base: &str,
        quote: &str,
        date: &str,
        policy: &JsFxConversionPolicy,
    ) -> Result<JsFxRateResult, JsValue> {
        let base_currency: RustCurrency = base.parse().map_err(to_js_err)?;
        let quote_currency: RustCurrency = quote.parse().map_err(to_js_err)?;
        let d = parse_iso_date(date)?;
        let query = FxQuery::with_policy(base_currency, quote_currency, d, policy.inner);
        let result = self.inner.rate(query).map_err(to_js_err)?;
        Ok(JsFxRateResult { inner: result })
    }

    /// Look up an FX rate using cashflow-date conversion semantics.
    /// @param base - Base currency code of the FX quote, where the rate is quote per base.
    /// @param quote - Quote currency code of the FX rate, expressed per unit of base currency.
    /// @param date - ISO-8601 date used by the calculation or market-data lookup.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if either currency code or `date` is invalid,
    /// no direct, inverse, or triangulated cashflow-date quote is available, or a
    /// resolved quote is non-finite or non-positive.
    #[wasm_bindgen(js_name = rateDefault)]
    pub fn rate_default(
        &self,
        base: &str,
        quote: &str,
        date: &str,
    ) -> Result<JsFxRateResult, JsValue> {
        let base_currency: RustCurrency = base.parse().map_err(to_js_err)?;
        let quote_currency: RustCurrency = quote.parse().map_err(to_js_err)?;
        let d = parse_iso_date(date)?;
        let query = FxQuery::with_policy(
            base_currency,
            quote_currency,
            d,
            RustFxConversionPolicy::CashflowDate,
        );
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
    /// @param interpolation_mode - Volatility-surface interpolation mode used between quoted points.
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
        id: &str,
        expiries: &[f64],
        tenors: &[f64],
        params_flat: &[f64],
        forwards: &[f64],
        interpolation_mode: Option<String>,
    ) -> Result<JsVolCube, JsValue> {
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
        let mode: VolInterpolationMode =
            finstack_quant_core::wire::serde_parse(interpolation_mode.as_deref().unwrap_or("vol"))
                .map_err(to_js_err)?;
        let cube = RustVolCube::from_grid(id, expiries, tenors, &sabr_params, forwards)
            .map_err(to_js_err)?
            .with_interpolation_mode(mode);
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
    pub fn from_json(json: &str) -> Result<JsVolCube, JsValue> {
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
    /// interpolation. Pass an empty array for both to omit; if one is
    /// provided, the other must be too.
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
        id: &str,
        expiries: &[f64],
        atm_vols: &[f64],
        rr25d: &[f64],
        bf25d: &[f64],
        rr10d: Option<Vec<f64>>,
        bf10d: Option<Vec<f64>>,
    ) -> Result<JsFxDeltaVolSurface, JsValue> {
        let wings_10d = match (rr10d, bf10d) {
            (Some(rr), Some(bf)) => Some((rr, bf)),
            (None, None) => None,
            _ => {
                return Err(to_js_err(
                    "rr10d and bf10d must both be provided or both omitted",
                ));
            }
        };
        let surface = RustFxDeltaVolSurface::new(
            id,
            expiries.to_vec(),
            atm_vols.to_vec(),
            rr25d.to_vec(),
            bf25d.to_vec(),
            wings_10d,
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
    pub fn from_json(json: &str) -> Result<JsFxDeltaVolSurface, JsValue> {
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
    fn discount_curve_new_and_accessors() {
        let curve = JsDiscountCurve::new(
            "USD-OIS",
            "2024-01-15",
            &[0.5, 0.99, 1.0, 0.98, 2.0, 0.96],
            None,
            None,
            None,
            None,
            None,
        )
        .expect("discount curve");
        assert_eq!(curve.id(), "USD-OIS");
        assert_eq!(curve.base_date(), "2024-01-15");
        assert!((curve.df(0.5) - 0.99).abs() < 1e-6);
        assert!((curve.df(1.0) - 0.98).abs() < 1e-6);
        assert!(curve.zero(1.0) > 0.0);
        let f = curve.forward(0.5, 1.0).expect("forward rate");
        assert!(f > 0.0);
    }

    #[test]
    fn discount_curve_flat_uses_continuous_compounding() {
        let curve =
            JsDiscountCurve::flat("USD-OIS", "2024-01-15", 0.04).expect("flat discount curve");

        for t in [0.0_f64, 0.25, 1.0, 5.0, 30.0] {
            assert!((curve.df(t) - (-0.04 * t).exp()).abs() < 1e-12);
        }
        assert!((curve.forward(2.0, 9.0).expect("flat forward") - 0.04).abs() < 1e-12);
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
        assert!((curve.rate(1.0) - 0.045).abs() < 1e-6);
    }

    #[test]
    fn fx_matrix_quote_and_rate() {
        let m = JsFxMatrix::new();
        m.set_quote("USD", "EUR", 0.92).expect("set quote");
        let r = m.rate_default("USD", "EUR", "2024-01-15").expect("fx rate");
        assert!((r.rate() - 0.92).abs() < 1e-9);
        assert!(!r.triangulated());
    }

    #[test]
    fn fx_pair_convention_helpers() {
        let conv = fx_pair_convention("USD", "JPY").expect("USDJPY convention");
        assert_eq!(conv.base().code(), "USD");
        assert_eq!(conv.quote().code(), "JPY");
        assert_eq!(conv.usd_quotation().to_string(), "indirect");
        assert!((conv.pip_size() - 0.01).abs() < 1e-12);
        assert_eq!(conv.spot_lag_days(), 2);
        assert!((fx_pip_size("EUR", "USD").expect("EURUSD pip") - 0.0001).abs() < 1e-12);
        let inverted = invert_fx_rate(1.10).expect("positive rate");
        assert!((inverted - 1.0 / 1.10).abs() < 1e-12);
        assert_eq!(
            JsFxQuoteConvention::from_name("direct")
                .expect("direct")
                .to_string(),
            "direct"
        );
    }

    // JsVolCube tests require a WASM runtime (JsValue) — run via wasm-pack test.
}
