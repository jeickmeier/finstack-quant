//! WASM bindings for `finstack_quant_core::market_data` term structures and FX.

use std::sync::Arc;

use crate::utils::{date_to_iso, parse_iso_date, to_js_err};
use finstack_quant_core::currency::Currency as RustCurrency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::surfaces::{
    FxDeltaVolSurface as RustFxDeltaVolSurface, VolCube as RustVolCube, VolInterpolationMode,
};
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve as RustDiscountCurve, ForwardCurve as RustForwardCurve,
};
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_quant_core::math::volatility::sabr::SabrParams;
use finstack_quant_core::money::fx::{
    FxConversionPolicy as RustFxConversionPolicy, FxMatrix as RustFxMatrix, FxQuery,
    FxRateResult as RustFxRateResult, SimpleFxProvider,
};
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// DiscountCurve
// ---------------------------------------------------------------------------

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
pub struct DiscountCurve {
    #[wasm_bindgen(skip)]
    pub(crate) inner: Arc<RustDiscountCurve>,
}

#[wasm_bindgen(js_class = DiscountCurve)]
impl DiscountCurve {
    /// Construct from an array of `[time, df]` pairs.
    ///
    /// @param id - Curve identifier (e.g. `"USD-OIS"`). Used as the lookup
    /// key inside a `MarketContext`.
    /// @param baseDate - ISO-8601 date string (`"YYYY-MM-DD"`). All `time`
    /// values are interpreted as year fractions from this date under
    /// `dayCount`.
    /// @param knots - Flat `[t0, df0, t1, df1, …]` array. `t` in years,
    /// `df` strictly positive. Length must be even.
    /// @param interp - Interpolation style (default `"monotone_convex"`).
    /// One of `"linear"`, `"log_linear"`, `"monotone_convex"`,
    /// `"cubic_hermite"`, `"piecewise_quadratic_forward"`.
    /// @param extrapolation - Extrapolation policy (default
    /// `"flat_forward"`). One of `"flat_zero"`, `"flat_forward"`, `"nan"`.
    /// @param dayCount - Day-count convention (defaults to curve-ID inference).
    /// @returns The constructed `DiscountCurve`.
    /// @throws If `knots` length is odd, the date is malformed, the
    /// interpolation style is unknown, or any `df` is non-positive.
    #[wasm_bindgen(constructor)]
    pub fn new(
        id: &str,
        base_date: &str,
        knots: &[f64],
        interp: Option<String>,
        extrapolation: Option<String>,
        day_count: Option<String>,
    ) -> Result<DiscountCurve, JsValue> {
        let base = parse_iso_date(base_date)?;
        let style = match interp {
            Some(ref s) => parse_interp_style(s)?,
            None => InterpStyle::MonotoneConvex,
        };
        let extrap = match extrapolation {
            Some(ref s) => parse_extrapolation(s)?,
            None => ExtrapolationPolicy::FlatForward,
        };
        if !knots.len().is_multiple_of(2) {
            return Err(to_js_err("knots array must have even length (t, df pairs)"));
        }
        let pairs: Vec<(f64, f64)> = knots.chunks_exact(2).map(|c| (c[0], c[1])).collect();

        let mut builder = RustDiscountCurve::builder(id)
            .base_date(base)
            .knots(pairs)
            .interp(style)
            .extrapolation(extrap);
        if let Some(ref s) = day_count {
            builder = builder.day_count(parse_day_count(s)?);
        }

        let curve = builder.build().map_err(to_js_err)?;

        Ok(Self {
            inner: Arc::new(curve),
        })
    }

    /// Discount factor at year fraction `t`.
    pub fn df(&self, t: f64) -> f64 {
        self.inner.df(t)
    }

    /// Continuously-compounded zero rate at year fraction `t`.
    pub fn zero(&self, t: f64) -> f64 {
        self.inner.zero(t)
    }

    /// Continuously-compounded forward rate between `t1` and `t2`.
    #[wasm_bindgen(js_name = forwardRate)]
    pub fn forward_rate(&self, t1: f64, t2: f64) -> Result<f64, JsValue> {
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

// ---------------------------------------------------------------------------
// ForwardCurve
// ---------------------------------------------------------------------------

/// Forward rate curve for a floating-rate index with a fixed tenor.
#[wasm_bindgen(js_name = ForwardCurve)]
pub struct ForwardCurve {
    #[wasm_bindgen(skip)]
    pub(crate) inner: Arc<RustForwardCurve>,
}

#[wasm_bindgen(js_class = ForwardCurve)]
impl ForwardCurve {
    /// Construct from an array of `[time, rate]` pairs.
    ///
    /// # Arguments
    /// * `id` - Curve identifier.
    /// * `tenor` - Index tenor in years.
    /// * `baseDate` - ISO date string.
    /// * `knots` - Flat `[t0, rate0, t1, rate1, …]` array.
    /// * `dayCount` - Day-count convention (defaults to curve-ID inference).
    /// * `interp` - Interpolation style (default ``"linear"``).
    /// * `extrapolation` - Extrapolation policy (default ``"flat_forward"``).
    #[wasm_bindgen(constructor)]
    pub fn new(
        id: &str,
        tenor: f64,
        base_date: &str,
        knots: &[f64],
        day_count: Option<String>,
        interp: Option<String>,
        extrapolation: Option<String>,
    ) -> Result<ForwardCurve, JsValue> {
        let base = parse_iso_date(base_date)?;
        let style = match interp {
            Some(ref s) => parse_interp_style(s)?,
            None => InterpStyle::Linear,
        };
        let extrap = match extrapolation {
            Some(ref s) => parse_extrapolation(s)?,
            None => ExtrapolationPolicy::FlatForward,
        };

        if !knots.len().is_multiple_of(2) {
            return Err(to_js_err(
                "knots array must have even length (t, rate pairs)",
            ));
        }
        let pairs: Vec<(f64, f64)> = knots.chunks_exact(2).map(|c| (c[0], c[1])).collect();

        let mut builder = RustForwardCurve::builder(id, tenor)
            .base_date(base)
            .knots(pairs)
            .interp(style)
            .extrapolation(extrap);
        if let Some(ref s) = day_count {
            builder = builder.day_count(parse_day_count(s)?);
        }

        let curve = builder.build().map_err(to_js_err)?;

        Ok(Self {
            inner: Arc::new(curve),
        })
    }

    /// Forward rate at year fraction `t`.
    pub fn rate(&self, t: f64) -> f64 {
        self.inner.rate(t)
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

// ---------------------------------------------------------------------------
// FxConversionPolicy / FxRateResult
// ---------------------------------------------------------------------------

/// Typed FX conversion policy wrapper for WASM callers.
#[wasm_bindgen(js_name = FxConversionPolicy)]
#[derive(Clone, Copy, Debug)]
pub struct FxConversionPolicy {
    inner: RustFxConversionPolicy,
}

#[wasm_bindgen(js_class = FxConversionPolicy)]
impl FxConversionPolicy {
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

    /// Use a custom provider-defined strategy.
    #[wasm_bindgen(js_name = custom)]
    pub fn custom() -> Self {
        Self {
            inner: RustFxConversionPolicy::Custom,
        }
    }

    /// Parse from a string label such as ``\"cashflow_date\"``.
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
pub struct FxRateResult {
    inner: RustFxRateResult,
}

#[wasm_bindgen(js_class = FxRateResult)]
impl FxRateResult {
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

// ---------------------------------------------------------------------------
// FxMatrix
// ---------------------------------------------------------------------------

/// Foreign-exchange rate matrix for currency conversion.
#[wasm_bindgen(js_name = FxMatrix)]
pub struct FxMatrix {
    inner: Arc<RustFxMatrix>,
}

impl Default for FxMatrix {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen(js_class = FxMatrix)]
impl FxMatrix {
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
    #[wasm_bindgen(js_name = setQuote)]
    pub fn set_quote(&self, base: &str, quote: &str, rate: f64) -> Result<(), JsValue> {
        let base_ccy: RustCurrency = base.parse().map_err(to_js_err)?;
        let quote_ccy: RustCurrency = quote.parse().map_err(to_js_err)?;
        self.inner
            .set_quote(base_ccy, quote_ccy, rate)
            .map_err(to_js_err)?;
        Ok(())
    }

    /// Set an authoritative quote scoped to one date and conversion policy.
    #[wasm_bindgen(js_name = setQuoteOn)]
    pub fn set_quote_on(
        &self,
        base: &str,
        quote: &str,
        date: &str,
        policy: &FxConversionPolicy,
        rate: f64,
    ) -> Result<(), JsValue> {
        let base_ccy: RustCurrency = base.parse().map_err(to_js_err)?;
        let quote_ccy: RustCurrency = quote.parse().map_err(to_js_err)?;
        let d = parse_iso_date(date)?;
        self.inner
            .set_quote_on(base_ccy, quote_ccy, d, policy.inner, rate)
            .map_err(to_js_err)
    }

    /// Look up an FX rate.
    ///
    /// # Arguments
    /// * `base` - Base (from) currency ISO code.
    /// * `quote` - Quote (to) currency ISO code.
    /// * `date` - ISO date string.
    /// * `policy` - Reusable conversion policy handle.
    pub fn rate(
        &self,
        base: &str,
        quote: &str,
        date: &str,
        policy: &FxConversionPolicy,
    ) -> Result<FxRateResult, JsValue> {
        let base_ccy: RustCurrency = base.parse().map_err(to_js_err)?;
        let quote_ccy: RustCurrency = quote.parse().map_err(to_js_err)?;
        let d = parse_iso_date(date)?;
        let query = FxQuery::with_policy(base_ccy, quote_ccy, d, policy.inner);
        let result = self.inner.rate(query).map_err(to_js_err)?;
        Ok(FxRateResult { inner: result })
    }

    /// Look up an FX rate using cashflow-date conversion semantics.
    #[wasm_bindgen(js_name = rateDefault)]
    pub fn rate_default(
        &self,
        base: &str,
        quote: &str,
        date: &str,
    ) -> Result<FxRateResult, JsValue> {
        let base_ccy: RustCurrency = base.parse().map_err(to_js_err)?;
        let quote_ccy: RustCurrency = quote.parse().map_err(to_js_err)?;
        let d = parse_iso_date(date)?;
        let query =
            FxQuery::with_policy(base_ccy, quote_ccy, d, RustFxConversionPolicy::CashflowDate);
        self.inner
            .rate(query)
            .map(|inner| FxRateResult { inner })
            .map_err(to_js_err)
    }
}

// ---------------------------------------------------------------------------
// VolCube
// ---------------------------------------------------------------------------

/// SABR volatility cube for swaption pricing.
///
/// Stores calibrated SABR parameters on an expiry × tenor grid and evaluates
/// implied volatilities via bilinear parameter interpolation followed by the
/// Hagan (2002) approximation.
#[wasm_bindgen(js_name = VolCube)]
pub struct VolCube {
    #[wasm_bindgen(skip)]
    pub(crate) inner: Arc<RustVolCube>,
}

#[wasm_bindgen(js_class = VolCube)]
impl VolCube {
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
    #[wasm_bindgen(constructor)]
    pub fn new(
        id: &str,
        expiries: &[f64],
        tenors: &[f64],
        params_flat: &[f64],
        forwards: &[f64],
        interpolation_mode: Option<String>,
    ) -> Result<VolCube, JsValue> {
        let n_nodes = expiries.len() * tenors.len();
        if params_flat.len() != n_nodes * 5 {
            return Err(JsValue::from_str(&format!(
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
            let p = SabrParams::new_with_shift(
                params_flat[base],     // alpha
                params_flat[base + 1], // beta
                params_flat[base + 2], // rho
                params_flat[base + 3], // nu
                shift,
            )
            .map_err(to_js_err)?;
            sabr_params.push(p);
        }
        let mode = match interpolation_mode.as_deref().unwrap_or("vol") {
            "vol" => VolInterpolationMode::Vol,
            "total_variance" => VolInterpolationMode::TotalVariance,
            other => {
                return Err(JsValue::from_str(&format!(
                    "invalid volatility interpolation mode {other:?}; expected 'vol' or 'total_variance'"
                )))
            }
        };
        let cube = RustVolCube::from_grid(id, expiries, tenors, &sabr_params, forwards)
            .map_err(to_js_err)?
            .with_interpolation_mode(mode);
        Ok(Self {
            inner: Arc::new(cube),
        })
    }

    /// Implied volatility at `(expiry, tenor, strike)`.
    ///
    /// Returns `Err` if `expiry` or `tenor` falls outside the grid.
    pub fn vol(&self, expiry: f64, tenor: f64, strike: f64) -> Result<f64, JsValue> {
        self.inner.vol(expiry, tenor, strike).map_err(to_js_err)
    }

    /// Implied volatility with clamped extrapolation.
    ///
    /// Clamps finite `expiry` and `tenor` values to the grid edges before
    /// interpolation. Non-finite inputs return `NaN`.
    pub fn vol_clamped(&self, expiry: f64, tenor: f64, strike: f64) -> f64 {
        self.inner.vol_clamped(expiry, tenor, strike)
    }

    /// Interpolation contract used across the expiry axis.
    #[wasm_bindgen(getter, js_name = interpolationMode)]
    pub fn interpolation_mode(&self) -> String {
        match self.inner.interpolation_mode() {
            VolInterpolationMode::Vol => "vol",
            VolInterpolationMode::TotalVariance => "total_variance",
        }
        .to_string()
    }

    /// Normal (Bachelier) implied volatility at `(expiry, tenor, strike)`.
    ///
    /// The returned vol is in absolute rate units (e.g. `0.008` = 80 bp/yr
    /// normal vol), the swaption market quoting convention.
    ///
    /// Returns `Err` if `expiry` or `tenor` falls outside the grid, if the
    /// expansion yields a non-finite volatility, or for cross-zero quotes
    /// (`(F+s)(K+s) <= 0`) with `beta > 0`, which require an explicit shift.
    pub fn vol_normal(&self, expiry: f64, tenor: f64, strike: f64) -> Result<f64, JsValue> {
        self.inner
            .vol_normal(expiry, tenor, strike)
            .map_err(to_js_err)
    }

    /// Normal (Bachelier) implied volatility with clamped extrapolation.
    ///
    /// Clamps finite `expiry` and `tenor` values to the grid edges; a
    /// degenerate finite expansion is floored to a small positive normal vol
    /// (absolute rate units). Non-finite inputs return `NaN`.
    pub fn vol_normal_clamped(&self, expiry: f64, tenor: f64, strike: f64) -> f64 {
        self.inner.vol_normal_clamped(expiry, tenor, strike)
    }

    /// Cube identifier.
    #[wasm_bindgen(getter, js_name = id)]
    pub fn id(&self) -> String {
        self.inner.id().as_str().to_string()
    }
}

// ---------------------------------------------------------------------------
// FxDeltaVolSurface
// ---------------------------------------------------------------------------

/// FX vol surface quoted in **delta space** (ATM, 25-delta RR/BF, optional
/// 10-delta wings).
///
/// Stores market-standard FX delta quotes (Wystup 2006, Clark 2011) and
/// converts to a strike-axis [`VolSurface`] on demand via Garman-Kohlhagen.
/// The delta convention is **forward delta (premium-unadjusted)**.
#[wasm_bindgen(js_name = FxDeltaVolSurface)]
pub struct FxDeltaVolSurface {
    #[wasm_bindgen(skip)]
    pub(crate) inner: Arc<RustFxDeltaVolSurface>,
}

#[wasm_bindgen(js_class = FxDeltaVolSurface)]
impl FxDeltaVolSurface {
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
    #[wasm_bindgen(constructor)]
    pub fn new(
        id: &str,
        expiries: &[f64],
        atm_vols: &[f64],
        rr25d: &[f64],
        bf25d: &[f64],
        rr10d: Option<Vec<f64>>,
        bf10d: Option<Vec<f64>>,
    ) -> Result<FxDeltaVolSurface, JsValue> {
        let surface = match (rr10d, bf10d) {
            (Some(rr), Some(bf)) => RustFxDeltaVolSurface::with_10d(
                id,
                expiries.to_vec(),
                atm_vols.to_vec(),
                rr25d.to_vec(),
                bf25d.to_vec(),
                rr,
                bf,
            )
            .map_err(to_js_err)?,
            (None, None) => RustFxDeltaVolSurface::new(
                id,
                expiries.to_vec(),
                atm_vols.to_vec(),
                rr25d.to_vec(),
                bf25d.to_vec(),
            )
            .map_err(to_js_err)?,
            _ => {
                return Err(JsValue::from_str(
                    "rr10d and bf10d must both be provided or both omitted",
                ));
            }
        };
        Ok(Self {
            inner: Arc::new(surface),
        })
    }

    /// Surface identifier.
    #[wasm_bindgen(getter, js_name = id)]
    pub fn id(&self) -> String {
        self.inner.id().as_str().to_string()
    }

    /// Expiry axis in years.
    #[wasm_bindgen(getter, js_name = expiries)]
    pub fn expiries(&self) -> Vec<f64> {
        self.inner.expiries().to_vec()
    }

    /// Number of expiry pillars.
    #[wasm_bindgen(getter, js_name = numExpiries)]
    pub fn num_expiries(&self) -> usize {
        self.inner.num_expiries()
    }

    /// Pillar vols at the given expiry index as `[atm, put25d_vol, call25d_vol]`.
    #[wasm_bindgen(js_name = pillarVols)]
    pub fn pillar_vols(&self, expiry_idx: usize) -> Result<Vec<f64>, JsValue> {
        if expiry_idx >= self.inner.num_expiries() {
            return Err(JsValue::from_str(&format!(
                "expiry_idx {} out of range (num_expiries={})",
                expiry_idx,
                self.inner.num_expiries()
            )));
        }
        let (atm, p, c) = self.inner.pillar_vols(expiry_idx);
        Ok(vec![atm, p, c])
    }

    /// Implied vol at `(expiry, strike)` for the supplied forward + rates.
    #[wasm_bindgen(js_name = impliedVol)]
    pub fn implied_vol(
        &self,
        expiry: f64,
        strike: f64,
        forward: f64,
        r_d: f64,
        r_f: f64,
    ) -> Result<f64, JsValue> {
        self.inner
            .implied_vol(expiry, strike, forward, r_d, r_f)
            .map_err(to_js_err)
    }

    /// Convert a forward delta to a strike (Garman-Kohlhagen, premium-unadjusted).
    #[wasm_bindgen(js_name = deltaToStrike)]
    pub fn delta_to_strike(delta: f64, forward: f64, vol: f64, expiry: f64, r_f: f64) -> f64 {
        RustFxDeltaVolSurface::delta_to_strike(delta, forward, vol, expiry, r_f)
    }

    /// Convert a strike to forward delta (Garman-Kohlhagen call delta).
    #[wasm_bindgen(js_name = strikeToDelta)]
    pub fn strike_to_delta(strike: f64, forward: f64, vol: f64, expiry: f64, r_f: f64) -> f64 {
        RustFxDeltaVolSurface::strike_to_delta(strike, forward, vol, expiry, r_f)
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
        assert_eq!(
            parse_extrapolation("flat").expect("flat"),
            ExtrapolationPolicy::FlatZero
        );
    }

    #[test]
    fn discount_curve_new_and_accessors() {
        let curve = DiscountCurve::new(
            "USD-OIS",
            "2024-01-15",
            &[0.5, 0.99, 1.0, 0.98, 2.0, 0.96],
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
        let f = curve.forward_rate(0.5, 1.0).expect("forward rate");
        assert!(f > 0.0);
    }

    #[test]
    fn forward_curve_new_and_accessors() {
        let curve = ForwardCurve::new(
            "USD-3M",
            0.25,
            "2024-01-15",
            &[0.5, 0.04, 1.0, 0.045, 2.0, 0.05],
            None,
            None,
            None,
        )
        .expect("forward curve");
        assert_eq!(curve.id(), "USD-3M");
        assert_eq!(curve.base_date(), "2024-01-15");
        assert!((curve.rate(1.0) - 0.045).abs() < 1e-6);
    }

    #[test]
    fn fx_matrix_quote_and_rate() {
        let m = FxMatrix::new();
        m.set_quote("USD", "EUR", 0.92).expect("set quote");
        let r = m.rate_default("USD", "EUR", "2024-01-15").expect("fx rate");
        assert!((r.rate() - 0.92).abs() < 1e-9);
        assert!(!r.triangulated());
    }

    // VolCube tests require a WASM runtime (JsValue) — run via wasm-pack test.
}
