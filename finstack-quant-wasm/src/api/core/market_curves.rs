//! WASM bindings for the inflation, price and base-correlation term
//! structures and for credit-index market data
//! (`finstack_quant_core::market_data::term_structures`).
//!
//! Curve dates are ISO-8601 strings (`"YYYY-MM-DD"`), as for `DiscountCurve`.

use crate::api::core::market_data::{
    flat_pairs, parse_day_count, parse_extrapolation, parse_interp_style, JsHazardCurve,
};
use crate::utils::input::{from_js_json, js_f64, js_f64_seq, js_string, js_uint};
use crate::utils::{date_to_iso, parse_iso_date, to_js_err};
use finstack_quant_core::market_data::term_structures::{
    BaseCorrelationCurve as RustBaseCorrelationCurve, CreditIndexData as RustCreditIndexData,
    InflationCurve as RustInflationCurve, PriceCurve as RustPriceCurve, PriceCurveKind,
};
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
///   baseCpi: 300.0,
///   knots: [0.0, 300.0, 5.0, 331.2],
/// });
/// curve.cpi(2.5); // projected CPI level at 2.5y
/// curve.inflationRate(0.0, 5.0); // annualised inflation over 5y
/// ```
#[wasm_bindgen(js_name = InflationCurve)]
pub struct JsInflationCurve {
    pub(crate) inner: Arc<RustInflationCurve>,
}

/// Named constructor options for [`JsInflationCurve`]; unknown keys are rejected.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InflationCurveOptions {
    id: String,
    base_date: String,
    base_cpi: f64,
    knots: Vec<f64>,
    #[serde(default)]
    day_count: Option<String>,
    #[serde(default)]
    indexation_lag_months: Option<u32>,
    #[serde(default)]
    interp: Option<String>,
    #[serde(default)]
    extrapolation: Option<String>,
}

impl JsInflationCurve {
    pub(crate) fn from_inner(inner: Arc<RustInflationCurve>) -> Self {
        Self { inner }
    }

    fn build(options: InflationCurveOptions) -> Result<JsInflationCurve, JsValue> {
        let mut builder = RustInflationCurve::builder(options.id)
            .base_date(parse_iso_date(&options.base_date)?)
            .base_cpi(options.base_cpi)
            .knots(flat_pairs(&options.knots, "knots")?);
        if let Some(day_count) = options.day_count.as_deref() {
            builder = builder.day_count(parse_day_count(day_count)?);
        }
        if let Some(months) = options.indexation_lag_months {
            builder = builder.indexation_lag_months(months);
        }
        if let Some(interp) = options.interp.as_deref() {
            builder = builder.interp(parse_interp_style(interp)?);
        }
        if let Some(extrapolation) = options.extrapolation.as_deref() {
            builder = builder.extrapolation(parse_extrapolation(extrapolation)?);
        }
        builder
            .build()
            .map(|curve| Self::from_inner(Arc::new(curve)))
            .map_err(to_js_err)
    }
}

#[wasm_bindgen(js_class = InflationCurve)]
impl JsInflationCurve {
    /// Construct an inflation curve from named options.
    ///
    /// # Arguments
    ///
    /// * `options` - InflationCurveOptions object (or its JSON text) with:
    ///   `id` (curve identifier, the `MarketContext` lookup key); `baseDate`
    ///   (ISO-8601; knot times are year fractions from it under `dayCount`);
    ///   `baseCpi` (strictly positive index level at the base date); `knots`
    ///   (flat `[t0, cpi0, t1, cpi1, …]` array, `t` in years, `cpi` the
    ///   projected index level); and the optional `dayCount`,
    ///   `indexationLagMonths` (publication lag in whole months),
    ///   `interp` and `extrapolation`. Omitted options use the Rust builder
    ///   defaults. Unknown keys are rejected.
    ///
    /// @returns The constructed `InflationCurve`.
    /// @throws `TypeError` (kind `invalid_type`) if `options` is not a JSON
    /// string or plain object; `FinstackError` (kind `validation`) for an
    /// unknown, missing or mistyped key, an odd-length `knots`, a malformed
    /// date, an unknown day-count/interpolation/extrapolation name, or CPI
    /// levels the curve builder rejects.
    #[wasm_bindgen(constructor)]
    pub fn new(options: JsValue) -> Result<JsInflationCurve, JsValue> {
        Self::build(from_js_json(&options, "options")?)
    }

    /// Deserialize from the canonical JSON wire form shared with Python `InflationCurve.to_json`.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical InflationCurve JSON text or plain object; unknown
    ///   fields are rejected and the curve is re-validated.
    ///
    /// @returns The validated `InflationCurve`.
    /// @throws `TypeError` if `json` is not a JSON string or plain object;
    /// `FinstackError` (kind `validation`) if it does not match the schema or
    /// fails curve validation.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsInflationCurve, JsValue> {
        from_js_json::<RustInflationCurve>(&json, "json")
            .map(|curve| Self::from_inner(Arc::new(curve)))
    }

    /// Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid curve).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&*self.inner).map_err(to_js_err)
    }

    /// Projected CPI level at year fraction `t`, without indexation lag (Rust
    /// `InflationCurve::cpi`).
    ///
    /// # Arguments
    ///
    /// * `t` - Time from the curve base date in years.
    ///
    /// @returns The interpolated index level.
    /// @throws `TypeError` if `t` is not a number.
    #[wasm_bindgen(js_name = cpi)]
    pub fn cpi(&self, t: JsValue) -> Result<f64, JsValue> {
        Ok(self.inner.cpi(js_f64(&t, "t")?))
    }

    /// Projected CPI level on a date, measured with the curve day count (Rust
    /// `InflationCurve::cpi_on_date`).
    ///
    /// # Arguments
    ///
    /// * `date` - ISO-8601 target date.
    ///
    /// @returns The interpolated index level.
    /// @throws `TypeError` if `date` is not a string; `FinstackError` (kind
    /// `validation`) for a malformed date or a year fraction that cannot be
    /// computed.
    #[wasm_bindgen(js_name = cpiOnDate)]
    pub fn cpi_on_date(&self, date: JsValue) -> Result<f64, JsValue> {
        let date = parse_iso_date(&js_string(&date, "date")?)?;
        self.inner.cpi_on_date(date).map_err(to_js_err)
    }

    /// CPI level at `t` shifted back by the indexation lag (Rust
    /// `InflationCurve::cpi_with_lag`).
    ///
    /// # Arguments
    ///
    /// * `t` - Settlement time from the curve base date in years; the curve is
    ///   evaluated at `t - indexationLagMonths / 12`.
    ///
    /// @returns The lagged index level (a continuous shift, with no
    /// seasonality adjustment).
    /// @throws `TypeError` if `t` is not a number.
    #[wasm_bindgen(js_name = cpiWithLag)]
    pub fn cpi_with_lag(&self, t: JsValue) -> Result<f64, JsValue> {
        Ok(self.inner.cpi_with_lag(js_f64(&t, "t")?))
    }

    /// Principal indexation ratio `cpiWithLag(t) / baseCpi` (Rust
    /// `InflationCurve::index_ratio`).
    ///
    /// # Arguments
    ///
    /// * `t` - Settlement time from the curve base date in years.
    ///
    /// @returns The uplift factor of an inflation-linked notional: `1.08` is
    /// 108% of face; no deflation floor is applied.
    /// @throws `TypeError` if `t` is not a number; `FinstackError` (kind
    /// `validation`) if the ratio is not finite and positive.
    #[wasm_bindgen(js_name = indexRatio)]
    pub fn index_ratio(&self, t: JsValue) -> Result<f64, JsValue> {
        self.inner.index_ratio(js_f64(&t, "t")?).map_err(to_js_err)
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
    #[wasm_bindgen(js_name = inflationRate)]
    pub fn inflation_rate(&self, t1: JsValue, t2: JsValue) -> Result<f64, JsValue> {
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

    /// Indexation (publication) lag in whole months.
    #[wasm_bindgen(getter, js_name = indexationLagMonths)]
    pub fn indexation_lag_months(&self) -> u32 {
        self.inner.indexation_lag_months()
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

    /// Extrapolation policy beyond the pillar range, such as `"flat_forward"`.
    #[wasm_bindgen(getter, js_name = extrapolation)]
    pub fn extrapolation(&self) -> String {
        self.inner.extrapolation().to_string()
    }
}

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
///   spotPrice: 72.0,
/// });
/// curve.price(0.5); // forward price at 6 months
/// curve.kind; // "price"
/// ```
#[wasm_bindgen(js_name = PriceCurve)]
pub struct JsPriceCurve {
    pub(crate) inner: Arc<RustPriceCurve>,
}

/// Named constructor options for [`JsPriceCurve`]; unknown keys are rejected.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PriceCurveOptions {
    id: String,
    base_date: String,
    knots: Vec<f64>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    spot_price: Option<f64>,
    #[serde(default)]
    extrapolation: Option<String>,
    #[serde(default)]
    interp: Option<String>,
    #[serde(default)]
    day_count: Option<String>,
}

impl JsPriceCurve {
    pub(crate) fn from_inner(inner: Arc<RustPriceCurve>) -> Self {
        Self { inner }
    }

    fn build(options: PriceCurveOptions) -> Result<JsPriceCurve, JsValue> {
        let mut builder = RustPriceCurve::builder(options.id)
            .base_date(parse_iso_date(&options.base_date)?)
            .knots(flat_pairs(&options.knots, "knots")?);
        if let Some(kind) = options.kind.as_deref() {
            builder = builder.kind(kind.parse::<PriceCurveKind>().map_err(to_js_err)?);
        }
        if let Some(spot_price) = options.spot_price {
            builder = builder.spot_price(spot_price);
        }
        if let Some(extrapolation) = options.extrapolation.as_deref() {
            builder = builder.extrapolation(parse_extrapolation(extrapolation)?);
        }
        if let Some(interp) = options.interp.as_deref() {
            builder = builder.interp(parse_interp_style(interp)?);
        }
        if let Some(day_count) = options.day_count.as_deref() {
            builder = builder.day_count(parse_day_count(day_count)?);
        }
        builder
            .build()
            .map(|curve| Self::from_inner(Arc::new(curve)))
            .map_err(to_js_err)
    }
}

#[wasm_bindgen(js_class = PriceCurve)]
impl JsPriceCurve {
    /// Construct a price curve from named options.
    ///
    /// # Arguments
    ///
    /// * `options` - PriceCurveOptions object (or its JSON text) with: `id`
    ///   (curve identifier, the `MarketContext` lookup key); `baseDate`
    ///   (ISO-8601; knot times are year fractions from it under `dayCount`);
    ///   `knots` (flat `[t0, p0, t1, p1, …]` array, `t` in years, `p` the
    ///   forward price or index level); and the optional `kind` (`"price"`,
    ///   the default, accepts any finite level; `"vol_index"` requires
    ///   non-negative levels), `spotPrice` (level at `t = 0`),
    ///   `extrapolation`, `interp` and `dayCount`. Omitted options use the
    ///   Rust builder defaults. Unknown keys are rejected.
    ///
    /// @returns The constructed `PriceCurve`.
    /// @throws `TypeError` (kind `invalid_type`) if `options` is not a JSON
    /// string or plain object; `FinstackError` (kind `validation`) for an
    /// unknown, missing or mistyped key, an odd-length `knots`, a malformed
    /// date, an unknown kind/day-count/interpolation/extrapolation name, or
    /// levels the curve builder rejects.
    #[wasm_bindgen(constructor)]
    pub fn new(options: JsValue) -> Result<JsPriceCurve, JsValue> {
        Self::build(from_js_json(&options, "options")?)
    }

    /// Deserialize from the canonical JSON wire form shared with Python `PriceCurve.to_json`.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical PriceCurve JSON text or plain object; unknown
    ///   fields are rejected and the curve is re-validated.
    ///
    /// @returns The validated `PriceCurve`.
    /// @throws `TypeError` if `json` is not a JSON string or plain object;
    /// `FinstackError` (kind `validation`) if it does not match the schema or
    /// fails curve validation.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsPriceCurve, JsValue> {
        from_js_json::<RustPriceCurve>(&json, "json").map(|curve| Self::from_inner(Arc::new(curve)))
    }

    /// Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid curve).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&*self.inner).map_err(to_js_err)
    }

    /// Forward price at year fraction `t` (Rust `PriceCurve::price`).
    ///
    /// # Arguments
    ///
    /// * `t` - Time from the curve base date in years.
    ///
    /// @returns The interpolated forward price (or index level).
    /// @throws `TypeError` if `t` is not a number.
    #[wasm_bindgen(js_name = price)]
    pub fn price(&self, t: JsValue) -> Result<f64, JsValue> {
        Ok(self.inner.price(js_f64(&t, "t")?))
    }

    /// Forward price on a date, measured with the curve day count (Rust
    /// `PriceCurve::price_on_date`).
    ///
    /// # Arguments
    ///
    /// * `date` - ISO-8601 target date.
    ///
    /// @returns The interpolated forward price (or index level).
    /// @throws `TypeError` if `date` is not a string; `FinstackError` (kind
    /// `validation`) for a malformed date or a year fraction that cannot be
    /// computed.
    #[wasm_bindgen(js_name = priceOnDate)]
    pub fn price_on_date(&self, date: JsValue) -> Result<f64, JsValue> {
        let date = parse_iso_date(&js_string(&date, "date")?)?;
        self.inner.price_on_date(date).map_err(to_js_err)
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

    /// Spot level at `t = 0` (a price, or index points for a vol-index curve).
    #[wasm_bindgen(getter, js_name = spotPrice)]
    pub fn spot_price(&self) -> f64 {
        self.inner.spot_price()
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

    /// Extrapolation policy beyond the pillar range, such as `"flat_zero"`.
    #[wasm_bindgen(getter, js_name = extrapolation)]
    pub fn extrapolation(&self) -> String {
        self.inner.extrapolation().to_string()
    }
}

/// Base-correlation curve for credit tranche pricing: correlation as a
/// function of the detachment point.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const curve = new core.BaseCorrelationCurve("CDX-IG", [3.0, 0.25, 7.0, 0.45]);
/// curve.correlation(5.0); // 0.35
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
    #[wasm_bindgen(constructor)]
    pub fn new(id: JsValue, knots: JsValue) -> Result<JsBaseCorrelationCurve, JsValue> {
        let id = js_string(&id, "id")?;
        let knots = flat_pairs(&js_f64_seq(&knots, "knots")?, "knots")?;
        RustBaseCorrelationCurve::builder(id)
            .knots(knots)
            .build()
            .map(|curve| Self::from_inner(Arc::new(curve)))
            .map_err(to_js_err)
    }

    /// Deserialize from the canonical JSON wire form shared with Python
    /// `BaseCorrelationCurve.to_json`.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical BaseCorrelationCurve JSON text or plain object;
    ///   unknown fields are rejected and the curve is re-validated.
    ///
    /// @returns The validated `BaseCorrelationCurve`.
    /// @throws `TypeError` if `json` is not a JSON string or plain object;
    /// `FinstackError` (kind `validation`) if it does not match the schema or
    /// fails curve validation.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsBaseCorrelationCurve, JsValue> {
        from_js_json::<RustBaseCorrelationCurve>(&json, "json")
            .map(|curve| Self::from_inner(Arc::new(curve)))
    }

    /// Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid curve).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&*self.inner).map_err(to_js_err)
    }

    /// Base correlation at a detachment point (Rust `BaseCorrelationCurve::correlation`).
    ///
    /// # Arguments
    ///
    /// * `detachment_pct` - Tranche detachment point in percent (`7.0` is 7%).
    ///
    /// @returns The interpolated correlation as a decimal; flat beyond the
    /// first and last pillar.
    /// @throws `TypeError` if `detachmentPct` is not a number.
    #[wasm_bindgen(js_name = correlation)]
    pub fn correlation(&self, detachment_pct: JsValue) -> Result<f64, JsValue> {
        Ok(self
            .inner
            .correlation(js_f64(&detachment_pct, "detachmentPct")?))
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

    /// Extrapolation policy beyond the pillar range, such as `"flat_zero"`.
    #[wasm_bindgen(getter, js_name = extrapolation)]
    pub fn extrapolation(&self) -> String {
        self.inner.extrapolation().to_string()
    }
}

/// Market data of one credit index: constituent count, recovery, the index
/// hazard curve and its base-correlation curve.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const hazard = core.HazardCurve.flat("CDX-IG-HZD", "2025-01-02", 0.01, 0.4);
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
    #[wasm_bindgen(constructor)]
    pub fn new(
        num_constituents: JsValue,
        recovery_rate: JsValue,
        index_credit_curve: &JsHazardCurve,
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
        self.inner.num_constituents
    }

    /// Index-level recovery rate on default, as a decimal.
    #[wasm_bindgen(getter, js_name = recoveryRate)]
    pub fn recovery_rate(&self) -> f64 {
        self.inner.recovery_rate
    }

    /// Hazard curve of the index as a whole, as a `HazardCurve`.
    #[wasm_bindgen(getter, js_name = indexCreditCurve)]
    pub fn index_credit_curve(&self) -> JsHazardCurve {
        JsHazardCurve {
            inner: Arc::clone(&self.inner.index_credit_curve),
        }
    }

    /// Base-correlation curve used for tranche pricing, as a `BaseCorrelationCurve`.
    #[wasm_bindgen(getter, js_name = baseCorrelationCurve)]
    pub fn base_correlation_curve(&self) -> JsBaseCorrelationCurve {
        JsBaseCorrelationCurve::from_inner(Arc::clone(&self.inner.base_correlation_curve))
    }
}
