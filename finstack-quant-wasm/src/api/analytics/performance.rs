//! WASM `Performance` class — the sole analytics entry point.
//!
//! Mirrors the Python `Performance` API (price- or return-panel construction,
//! every metric exposed as an instance method). Complex result types are
//! serialized to plain JS objects via `serde_wasm_bindgen` rather than
//! exposed as classes, keeping the JS facade simple.

use crate::utils::{date_to_iso, to_js_err};
use finstack_quant_analytics as fa;
use finstack_quant_core::dates::{calendar_by_id, FiscalConfig, HolidayCalendar, PeriodKind};
use js_sys::{Array, Float64Array, Reflect};
use wasm_bindgen::prelude::*;

use super::support::{parse_f64_matrix, parse_f64_vec, parse_iso_date, parse_iso_dates};

const DEFAULT_FISCAL_START_MONTH: u8 = 1;
const DEFAULT_FISCAL_START_DAY: u8 = 1;
/// Default holiday calendar for FYTD fiscal-year-start alignment. Callers
/// with non-US panels should pass an explicit calendar id instead.
const DEFAULT_FISCAL_CALENDAR_ID: &str = "nyse";
const DEFAULT_FREQ: &str = "daily";
const DEFAULT_ROLLING_WINDOW: usize = 63;
const DEFAULT_CONFIDENCE: f64 = 0.95;

struct PanelInputs {
    dates: Vec<time::Date>,
    values: Vec<Vec<f64>>,
    ticker_names: Vec<String>,
    freq: PeriodKind,
}

fn parse_freq(freq: &str) -> Result<PeriodKind, JsValue> {
    freq.parse::<PeriodKind>().map_err(|_| {
        to_js_err(format!(
            "Unknown frequency {freq:?}; expected one of: \
             daily, weekly, monthly, quarterly, semiannual, annual"
        ))
    })
}

fn make_fiscal_config(month: Option<u8>, day: Option<u8>) -> Result<FiscalConfig, JsValue> {
    FiscalConfig::new(
        month.unwrap_or(DEFAULT_FISCAL_START_MONTH),
        day.unwrap_or(DEFAULT_FISCAL_START_DAY),
    )
    .map_err(to_js_err)
}

fn resolve_fiscal_calendar(calendar_id: &str) -> Result<&'static dyn HolidayCalendar, JsValue> {
    calendar_by_id(calendar_id)
        .ok_or_else(|| to_js_err(format!("calendar {calendar_id:?} not found")))
}

fn parse_dates(dates: JsValue) -> Result<Vec<time::Date>, JsValue> {
    let strs: Vec<String> = serde_wasm_bindgen::from_value(dates).map_err(to_js_err)?;
    parse_iso_dates(&strs)
}

fn parse_panel_inputs(
    dates: JsValue,
    values: JsValue,
    ticker_names: JsValue,
    freq: Option<String>,
) -> Result<PanelInputs, JsValue> {
    Ok(PanelInputs {
        dates: parse_dates(dates)?,
        values: parse_f64_matrix(values)?,
        ticker_names: serde_wasm_bindgen::from_value(ticker_names).map_err(to_js_err)?,
        freq: parse_freq(freq.as_deref().unwrap_or(DEFAULT_FREQ))?,
    })
}

fn to_js<T: serde::Serialize>(value: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value).map_err(to_js_err)
}

/// Serialize a `Vec<f64>` as a JavaScript `Float64Array`.
///
/// Used for hot numeric outputs (per-ticker scalars, drawdowns, cumulative
/// returns) so the JS side gets a contiguous typed array instead of a generic
/// `Array<number>` whose `Number` boxing dominates allocation cost on large
/// panels.
fn vec_f64_to_js(values: &[f64]) -> JsValue {
    Float64Array::from(values).into()
}

/// Serialize a `Vec<Vec<f64>>` as a JavaScript `Array<Float64Array>`.
fn matrix_f64_to_js(values: &[Vec<f64>]) -> JsValue {
    let outer = Array::new_with_length(values.len() as u32);
    for (i, row) in values.iter().enumerate() {
        outer.set(i as u32, Float64Array::from(row.as_slice()).into());
    }
    outer.into()
}

fn result_vec_f64_to_js(result: finstack_quant_core::Result<Vec<f64>>) -> Result<JsValue, JsValue> {
    Ok(vec_f64_to_js(&result.map_err(to_js_err)?))
}

fn dates_to_js_array(dates: &[time::Date]) -> Array {
    let date_array = Array::new_with_length(dates.len() as u32);
    for (i, &d) in dates.iter().enumerate() {
        date_array.set(i as u32, JsValue::from_str(&date_to_iso(d)));
    }
    date_array
}

/// Build a plain JS object `{ dates: string[], <numeric_field>: Float64Array, ... }`
/// from a series of (key, JsValue) pairs.
fn obj_from_pairs(pairs: &[(&str, JsValue)]) -> Result<JsValue, JsValue> {
    let obj = js_sys::Object::new();
    for (key, value) in pairs {
        Reflect::set(&obj, &JsValue::from_str(key), value)?;
    }
    Ok(obj.into())
}

/// Serialize a `DatedSeries`-like rolling result with parallel `dates` /
/// numeric vectors as a plain JS object whose numeric vector is a typed array.
fn dated_series_to_js(
    values: &[f64],
    dates: &[time::Date],
    value_key: &str,
) -> Result<JsValue, JsValue> {
    obj_from_pairs(&[
        ("dates", dates_to_js_array(dates).into()),
        (value_key, vec_f64_to_js(values)),
    ])
}

fn rolling_greeks_to_js(rg: &fa::RollingGreeks) -> Result<JsValue, JsValue> {
    obj_from_pairs(&[
        ("dates", dates_to_js_array(&rg.dates).into()),
        ("alphas", vec_f64_to_js(&rg.alphas)),
        ("betas", vec_f64_to_js(&rg.betas)),
    ])
}

/// Stateful performance analytics engine over a panel of ticker price (or return) series.
///
/// Dates are ISO-8601 values in ascending order. Numeric inputs are row-major
/// with one row per date and one column per ticker. Scalar rates and returns
/// use decimal fractions; numeric outputs are Float64Array values in ticker
/// order unless the method documents an object or matrix shape.
///
/// Invalid dates, shapes, frequencies, tickers, and confidence levels are
/// returned as rejected JsValue errors.
#[wasm_bindgen(js_name = Performance)]
pub struct JsPerformance {
    inner: fa::Performance,
}

#[wasm_bindgen(js_class = Performance)]
impl JsPerformance {
    /// Construct from a price matrix. `dates` is an array of ISO date strings,
    /// `prices` is `prices[i]` = column for ticker `i`.
    /// # Errors
    ///
    /// Rejects malformed dates or matrices, invalid prices, unsupported
    /// frequencies, and an unknown benchmark ticker.
    /// @param dates - ISO-8601 observation dates in ascending order, one entry per price row.
    /// @param prices - Row-major matrix where `prices[i][j]` is ticker j on observation i.
    /// @param ticker_names - Ticker labels aligned with the price-matrix columns.
    /// @param benchmark_ticker - Optional ticker label to use as the benchmark return series.
    /// @param freq - Optional observation frequency token; defaults to daily.
    #[wasm_bindgen(constructor)]
    pub fn new(
        dates: JsValue,
        prices: JsValue,
        ticker_names: JsValue,
        benchmark_ticker: Option<String>,
        freq: Option<String>,
    ) -> Result<JsPerformance, JsValue> {
        let panel = parse_panel_inputs(dates, prices, ticker_names, freq)?;
        let inner = fa::Performance::new(
            panel.dates,
            panel.values,
            panel.ticker_names,
            benchmark_ticker.as_deref(),
            panel.freq,
        )
        .map_err(to_js_err)?;
        Ok(JsPerformance { inner })
    }

    /// Construct from a return matrix (one row per `dates` entry per ticker).
    /// # Errors
    ///
    /// Rejects malformed dates or matrices and invalid benchmark or
    /// frequency inputs.
    /// @param dates - ISO-8601 observation dates in ascending order, one entry per return row.
    /// @param returns - Row-major simple decimal return matrix where `returns[i][j]` is ticker j on observation i.
    /// @param ticker_names - Ticker labels aligned with the return-matrix columns.
    /// @param benchmark_ticker - Optional ticker label to use as the benchmark return series.
    /// @param freq - Optional observation frequency token; defaults to daily.
    #[wasm_bindgen(js_name = fromReturns)]
    pub fn from_returns(
        dates: JsValue,
        returns: JsValue,
        ticker_names: JsValue,
        benchmark_ticker: Option<String>,
        freq: Option<String>,
    ) -> Result<JsPerformance, JsValue> {
        let panel = parse_panel_inputs(dates, returns, ticker_names, freq)?;
        let inner = fa::Performance::from_returns(
            panel.dates,
            panel.values,
            panel.ticker_names,
            benchmark_ticker.as_deref(),
            panel.freq,
        )
        .map_err(to_js_err)?;
        Ok(JsPerformance { inner })
    }

    // ── Mutators ──

    /// Restrict subsequent analytics to `[start, end]`.
    /// @param start - Inclusive ISO-8601 start date for the active analysis window.
    /// @param end - Inclusive ISO-8601 end date for the active analysis window.
    #[wasm_bindgen(js_name = resetDateRange)]
    pub fn reset_date_range(&mut self, start: &str, end: &str) -> Result<(), JsValue> {
        self.inner
            .reset_date_range(parse_iso_date(start)?, parse_iso_date(end)?);
        Ok(())
    }

    /// Change the benchmark ticker.
    /// @param ticker - Existing ticker label to use as the benchmark return series.
    #[wasm_bindgen(js_name = resetBenchTicker)]
    pub fn reset_bench_ticker(&mut self, ticker: &str) -> Result<(), JsValue> {
        self.inner.reset_bench_ticker(ticker).map_err(to_js_err)
    }

    // ── Accessors ──

    /// Ticker names in column order.
    #[wasm_bindgen(js_name = tickerNames)]
    pub fn ticker_names(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.ticker_names().to_vec())
    }

    /// Benchmark column index.
    #[wasm_bindgen(js_name = benchmarkIdx)]
    pub fn benchmark_idx(&self) -> usize {
        self.inner.benchmark_idx()
    }

    /// Observation frequency token.
    #[wasm_bindgen(js_name = freq)]
    pub fn freq(&self) -> String {
        self.inner.freq().to_string()
    }

    /// Full return-aligned date grid as ISO date strings (`"YYYY-MM-DD"`),
    /// independent of any active window — matches Rust `Performance::dates`.
    #[wasm_bindgen(js_name = dates)]
    pub fn dates(&self) -> Vec<String> {
        self.inner.dates().iter().map(|&d| date_to_iso(d)).collect()
    }

    /// Date grid of the currently active analysis window as ISO date strings.
    /// Equal to `dates()` until `resetDateRange` narrows the window.
    #[wasm_bindgen(js_name = activeDates)]
    pub fn active_dates(&self) -> Vec<String> {
        self.inner
            .active_dates()
            .iter()
            .map(|&d| date_to_iso(d))
            .collect()
    }

    /// Date grid for one ticker's active return series as ISO date strings.
    /// @param ticker_idx - Zero-based ticker column index in tickerNames order.
    #[wasm_bindgen(js_name = activeDatesForTicker)]
    pub fn active_dates_for_ticker(&self, ticker_idx: usize) -> Result<Vec<String>, JsValue> {
        Ok(self
            .inner
            .active_dates_for_ticker(ticker_idx)
            .map_err(to_js_err)?
            .iter()
            .map(|&d| date_to_iso(d))
            .collect())
    }

    // ── Scalar metrics ──

    /// Compound annual growth rate per asset.
    pub fn cagr(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.cagr())
    }

    /// Mean periodic return per asset (annualized by default).
    /// @param annualize - Whether to annualize by the configured frequency; defaults to true.
    #[wasm_bindgen(js_name = meanReturn)]
    pub fn mean_return(&self, annualize: Option<bool>) -> JsValue {
        vec_f64_to_js(&self.inner.mean_return(annualize.unwrap_or(true)))
    }

    /// Return volatility per asset (annualized by default).
    /// @param annualize - Whether to annualize by the configured frequency; defaults to true.
    pub fn volatility(&self, annualize: Option<bool>) -> JsValue {
        vec_f64_to_js(&self.inner.volatility(annualize.unwrap_or(true)))
    }

    /// Sharpe ratio per asset for the given risk-free rate.
    /// @param risk_free_rate - Annualized decimal risk-free rate; defaults to 0.0.
    pub fn sharpe(&self, risk_free_rate: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.sharpe(risk_free_rate.unwrap_or(0.0)))
    }

    /// Sortino ratio per asset for the given per-period minimum acceptable return.
    /// @param mar - Per-period minimum acceptable return as a decimal; defaults to 0.0.
    pub fn sortino(&self, mar: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.sortino(mar.unwrap_or(0.0)))
    }

    /// Calmar ratio (CAGR over max drawdown) per asset.
    pub fn calmar(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.calmar())
    }

    /// Mean drawdown per asset.
    #[wasm_bindgen(js_name = meanDrawdown)]
    pub fn mean_drawdown(&self) -> JsValue {
        vec_f64_to_js(&self.inner.mean_drawdown())
    }

    /// Maximum drawdown per asset.
    #[wasm_bindgen(js_name = maxDrawdown)]
    pub fn max_drawdown(&self) -> JsValue {
        vec_f64_to_js(&self.inner.max_drawdown())
    }

    /// Historical value-at-risk per asset at the given confidence level.
    /// @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
    #[wasm_bindgen(js_name = valueAtRisk)]
    pub fn value_at_risk(&self, confidence: Option<f64>) -> JsValue {
        vec_f64_to_js(
            &self
                .inner
                .value_at_risk(confidence.unwrap_or(DEFAULT_CONFIDENCE)),
        )
    }

    /// Expected shortfall (CVaR) per asset at the given confidence level.
    /// @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
    #[wasm_bindgen(js_name = expectedShortfall)]
    pub fn expected_shortfall(&self, confidence: Option<f64>) -> JsValue {
        vec_f64_to_js(
            &self
                .inner
                .expected_shortfall(confidence.unwrap_or(DEFAULT_CONFIDENCE)),
        )
    }

    /// Tracking error versus the benchmark per asset.
    #[wasm_bindgen(js_name = trackingError)]
    pub fn tracking_error(&self) -> JsValue {
        vec_f64_to_js(&self.inner.tracking_error())
    }

    /// Information ratio versus the benchmark per asset.
    #[wasm_bindgen(js_name = informationRatio)]
    pub fn information_ratio(&self) -> JsValue {
        vec_f64_to_js(&self.inner.information_ratio())
    }

    /// Return skewness per asset.
    pub fn skewness(&self) -> JsValue {
        vec_f64_to_js(&self.inner.skewness())
    }

    /// Excess kurtosis of returns per asset.
    pub fn kurtosis(&self) -> JsValue {
        vec_f64_to_js(&self.inner.kurtosis())
    }

    /// Geometric mean return per asset.
    #[wasm_bindgen(js_name = geometricMean)]
    pub fn geometric_mean(&self) -> JsValue {
        vec_f64_to_js(&self.inner.geometric_mean())
    }

    /// Per-asset skewness and kurtosis from one moments pass, as
    /// `{ skewness: Float64Array, kurtosis: Float64Array }`.
    #[wasm_bindgen(js_name = skewKurt)]
    pub fn skew_kurt(&self) -> Result<JsValue, JsValue> {
        let (skew, kurt) = self.inner.skew_kurt();
        obj_from_pairs(&[
            ("skewness", vec_f64_to_js(&skew)),
            ("kurtosis", vec_f64_to_js(&kurt)),
        ])
    }

    /// Per-asset historical VaR and expected shortfall from one tail pass, as
    /// `{ value_at_risk: Float64Array, expected_shortfall: Float64Array }`.
    /// @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
    #[wasm_bindgen(js_name = valueAtRiskAndEs)]
    pub fn value_at_risk_and_es(&self, confidence: Option<f64>) -> Result<JsValue, JsValue> {
        let (var, es) = self
            .inner
            .value_at_risk_and_es(confidence.unwrap_or(DEFAULT_CONFIDENCE));
        obj_from_pairs(&[
            ("value_at_risk", vec_f64_to_js(&var)),
            ("expected_shortfall", vec_f64_to_js(&es)),
        ])
    }

    /// Downside deviation per asset below the per-period minimum acceptable return.
    /// @param mar - Per-period minimum acceptable return as a decimal; defaults to 0.0.
    #[wasm_bindgen(js_name = downsideDeviation)]
    pub fn downside_deviation(&self, mar: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.downside_deviation(mar.unwrap_or(0.0)))
    }

    /// Longest drawdown duration (in periods) per asset.
    #[wasm_bindgen(js_name = maxDrawdownDuration)]
    pub fn max_drawdown_duration(&self) -> Result<JsValue, JsValue> {
        // `usize` does not fit a typed array; keep the serde path.
        to_js(&self.inner.max_drawdown_duration())
    }

    /// Empyrical-style annualized geometric up-capture versus the benchmark per asset.
    #[wasm_bindgen(js_name = upCapture)]
    pub fn up_capture(&self) -> JsValue {
        vec_f64_to_js(&self.inner.up_capture())
    }

    /// Empyrical-style annualized geometric down-capture versus the benchmark per asset.
    #[wasm_bindgen(js_name = downCapture)]
    pub fn down_capture(&self) -> JsValue {
        vec_f64_to_js(&self.inner.down_capture())
    }

    /// Empyrical-style annualized geometric up/down capture ratio versus the benchmark per asset.
    #[wasm_bindgen(js_name = captureRatio)]
    pub fn capture_ratio(&self) -> JsValue {
        vec_f64_to_js(&self.inner.capture_ratio())
    }

    /// Omega ratio per asset for the given threshold return.
    /// @param threshold - Per-period threshold return as a decimal; defaults to 0.0.
    #[wasm_bindgen(js_name = omegaRatio)]
    pub fn omega_ratio(&self, threshold: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.omega_ratio(threshold.unwrap_or(0.0)))
    }

    /// Treynor ratio per asset for the given risk-free rate.
    /// @param risk_free_rate - Annualized decimal risk-free rate; defaults to 0.0.
    pub fn treynor(&self, risk_free_rate: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.treynor(risk_free_rate.unwrap_or(0.0)))
    }

    /// Gain-to-pain ratio per asset.
    #[wasm_bindgen(js_name = gainToPain)]
    pub fn gain_to_pain(&self) -> JsValue {
        vec_f64_to_js(&self.inner.gain_to_pain())
    }

    /// Ulcer index per asset.
    #[wasm_bindgen(js_name = ulcerIndex)]
    pub fn ulcer_index(&self) -> JsValue {
        vec_f64_to_js(&self.inner.ulcer_index())
    }

    /// Martin ratio (excess return over ulcer index) per asset.
    #[wasm_bindgen(js_name = martinRatio)]
    pub fn martin_ratio(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.martin_ratio())
    }

    /// Recovery factor (total return over max drawdown) per asset.
    #[wasm_bindgen(js_name = recoveryFactor)]
    pub fn recovery_factor(&self) -> JsValue {
        vec_f64_to_js(&self.inner.recovery_factor())
    }

    /// Pain index (mean drawdown magnitude) per asset.
    #[wasm_bindgen(js_name = painIndex)]
    pub fn pain_index(&self) -> JsValue {
        vec_f64_to_js(&self.inner.pain_index())
    }

    /// Pain ratio (excess return over pain index) per asset.
    /// @param risk_free_rate - Annualized decimal risk-free rate; defaults to 0.0.
    #[wasm_bindgen(js_name = painRatio)]
    pub fn pain_ratio(&self, risk_free_rate: Option<f64>) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.pain_ratio(risk_free_rate.unwrap_or(0.0)))
    }

    /// Tail ratio of upper to lower return quantiles per asset.
    /// @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
    #[wasm_bindgen(js_name = tailRatio)]
    pub fn tail_ratio(&self, confidence: Option<f64>) -> JsValue {
        vec_f64_to_js(
            &self
                .inner
                .tail_ratio(confidence.unwrap_or(DEFAULT_CONFIDENCE)),
        )
    }

    /// R-squared of returns against the benchmark per asset.
    #[wasm_bindgen(js_name = rSquared)]
    pub fn r_squared(&self) -> JsValue {
        vec_f64_to_js(&self.inner.r_squared())
    }

    /// Share of periods beating the benchmark per asset.
    #[wasm_bindgen(js_name = battingAverage)]
    pub fn batting_average(&self) -> JsValue {
        vec_f64_to_js(&self.inner.batting_average())
    }

    /// Parametric (Gaussian) value-at-risk per asset.
    /// @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
    #[wasm_bindgen(js_name = parametricVar)]
    pub fn parametric_var(&self, confidence: Option<f64>) -> JsValue {
        vec_f64_to_js(
            &self
                .inner
                .parametric_var(confidence.unwrap_or(DEFAULT_CONFIDENCE)),
        )
    }

    /// Cornish-Fisher adjusted value-at-risk per asset.
    /// @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
    #[wasm_bindgen(js_name = cornishFisherVar)]
    pub fn cornish_fisher_var(&self, confidence: Option<f64>) -> JsValue {
        vec_f64_to_js(
            &self
                .inner
                .cornish_fisher_var(confidence.unwrap_or(DEFAULT_CONFIDENCE)),
        )
    }

    /// Conditional drawdown-at-risk per asset at the given confidence level.
    /// @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
    pub fn cdar(&self, confidence: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.cdar(confidence.unwrap_or(DEFAULT_CONFIDENCE)))
    }

    /// M-squared (Modigliani) risk-adjusted return per asset.
    /// @param risk_free_rate - Annualized decimal risk-free rate; defaults to 0.0.
    #[wasm_bindgen(js_name = mSquared)]
    pub fn m_squared(&self, risk_free_rate: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.m_squared(risk_free_rate.unwrap_or(0.0)))
    }

    /// Modified Sharpe ratio using Cornish-Fisher VaR per asset.
    /// @param risk_free_rate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
    #[wasm_bindgen(js_name = modifiedSharpe)]
    pub fn modified_sharpe(&self, risk_free_rate: Option<f64>, confidence: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.modified_sharpe(
            risk_free_rate.unwrap_or(0.0),
            confidence.unwrap_or(DEFAULT_CONFIDENCE),
        ))
    }

    /// Sterling ratio over the `n` largest drawdowns per asset.
    /// @param risk_free_rate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @param n - Number of largest drawdowns to include; defaults to 5.
    #[wasm_bindgen(js_name = sterlingRatio)]
    pub fn sterling_ratio(
        &self,
        risk_free_rate: Option<f64>,
        n: Option<usize>,
    ) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(
            self.inner
                .sterling_ratio(risk_free_rate.unwrap_or(0.0), n.unwrap_or(5)),
        )
    }

    /// Burke ratio over the `n` largest drawdowns per asset.
    /// @param risk_free_rate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @param n - Number of largest drawdowns to include; defaults to 5.
    #[wasm_bindgen(js_name = burkeRatio)]
    pub fn burke_ratio(
        &self,
        risk_free_rate: Option<f64>,
        n: Option<usize>,
    ) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(
            self.inner
                .burke_ratio(risk_free_rate.unwrap_or(0.0), n.unwrap_or(5)),
        )
    }

    // ── Vector outputs ──

    /// Cumulative return series per asset.
    #[wasm_bindgen(js_name = cumulativeReturns)]
    pub fn cumulative_returns(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.cumulative_returns())
    }

    /// Drawdown series per asset.
    #[wasm_bindgen(js_name = drawdownSeries)]
    pub fn drawdown_series(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.drawdown_series())
    }

    /// Pairwise return correlation matrix across assets.
    #[wasm_bindgen(js_name = correlationMatrix)]
    pub fn correlation_matrix(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.correlation_matrix())
    }

    /// Cumulative outperformance versus the benchmark per asset.
    #[wasm_bindgen(js_name = cumulativeReturnsOutperformance)]
    pub fn cumulative_returns_outperformance(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.cumulative_returns_outperformance())
    }

    /// Difference between asset and benchmark drawdown series.
    #[wasm_bindgen(js_name = drawdownDifference)]
    pub fn drawdown_difference(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.drawdown_difference())
    }

    /// Excess returns over the supplied risk-free series per asset.
    /// @param rf - Risk-free return series as decimal values aligned with active observations.
    /// @param nperiods - Optional periods per year used to annualize excess returns.
    #[wasm_bindgen(js_name = excessReturns)]
    pub fn excess_returns(&self, rf: JsValue, nperiods: Option<f64>) -> Result<JsValue, JsValue> {
        let rf = parse_f64_vec(rf)?;
        Ok(matrix_f64_to_js(&self.inner.excess_returns(&rf, nperiods)))
    }

    // ── Benchmark ──

    /// OLS beta versus the benchmark per asset, with standard error and 95% CI.
    pub fn beta(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.beta())
    }

    /// Benchmark regression annualized Jensen alpha/beta statistics per asset.
    /// @param risk_free_rate - Annualized decimal risk-free rate; defaults to 0.0.
    pub fn greeks(&self, risk_free_rate: Option<f64>) -> Result<JsValue, JsValue> {
        to_js(&self.inner.greeks(risk_free_rate.unwrap_or(0.0)))
    }

    /// Rolling benchmark annualized Jensen alpha/beta for one asset over a window.
    /// @param ticker_idx - Zero-based ticker column index in tickerNames order.
    /// @param window - Observation window length; defaults to 63 periods.
    /// @param risk_free_rate - Annualized decimal risk-free rate; defaults to 0.0.
    #[wasm_bindgen(js_name = rollingGreeks)]
    pub fn rolling_greeks(
        &self,
        ticker_idx: usize,
        window: Option<usize>,
        risk_free_rate: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        let rg = self
            .inner
            .rolling_greeks(
                ticker_idx,
                window.unwrap_or(DEFAULT_ROLLING_WINDOW),
                risk_free_rate.unwrap_or(0.0),
            )
            .map_err(to_js_err)?;
        rolling_greeks_to_js(&rg)
    }

    /// Rolling volatility series for one asset over a window.
    /// @param ticker_idx - Zero-based ticker column index in tickerNames order.
    /// @param window - Observation window length; defaults to 63 periods.
    #[wasm_bindgen(js_name = rollingVolatility)]
    pub fn rolling_volatility(
        &self,
        ticker_idx: usize,
        window: Option<usize>,
    ) -> Result<JsValue, JsValue> {
        let series = self
            .inner
            .rolling_volatility(ticker_idx, window.unwrap_or(DEFAULT_ROLLING_WINDOW))
            .map_err(to_js_err)?;
        dated_series_to_js(&series.values, &series.dates, "volatility")
    }

    /// Rolling Sortino ratio series for one asset over a window.
    /// @param ticker_idx - Zero-based ticker column index in tickerNames order.
    /// @param window - Observation window length; defaults to 63 periods.
    /// @param mar - Per-period minimum acceptable return as a decimal; defaults to 0.0.
    #[wasm_bindgen(js_name = rollingSortino)]
    pub fn rolling_sortino(
        &self,
        ticker_idx: usize,
        window: Option<usize>,
        mar: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        let series = self
            .inner
            .rolling_sortino(
                ticker_idx,
                window.unwrap_or(DEFAULT_ROLLING_WINDOW),
                mar.unwrap_or(0.0),
            )
            .map_err(to_js_err)?;
        dated_series_to_js(&series.values, &series.dates, "sortino")
    }

    /// Rolling Sharpe ratio series for one asset over a window.
    /// @param ticker_idx - Zero-based ticker column index in tickerNames order.
    /// @param window - Observation window length; defaults to 63 periods.
    /// @param risk_free_rate - Annualized decimal risk-free rate; defaults to 0.0.
    #[wasm_bindgen(js_name = rollingSharpe)]
    pub fn rolling_sharpe(
        &self,
        ticker_idx: usize,
        window: Option<usize>,
        risk_free_rate: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        let series = self
            .inner
            .rolling_sharpe(
                ticker_idx,
                window.unwrap_or(DEFAULT_ROLLING_WINDOW),
                risk_free_rate.unwrap_or(0.0),
            )
            .map_err(to_js_err)?;
        dated_series_to_js(&series.values, &series.dates, "sharpe")
    }

    /// Rolling compounded return series for one asset over a window.
    /// @param ticker_idx - Zero-based ticker column index in tickerNames order.
    /// @param window - Positive number of observations to compound in each window.
    #[wasm_bindgen(js_name = rollingReturns)]
    pub fn rolling_returns(&self, ticker_idx: usize, window: usize) -> Result<JsValue, JsValue> {
        let series = self
            .inner
            .rolling_returns(ticker_idx, window)
            .map_err(to_js_err)?;
        dated_series_to_js(&series.values, &series.dates, "return")
    }

    /// Details of the `n` largest drawdown episodes for one asset.
    /// @param ticker_idx - Zero-based ticker column index in tickerNames order.
    /// @param n - Number of largest drawdown episodes to return; defaults to 5.
    #[wasm_bindgen(js_name = drawdownDetails)]
    pub fn drawdown_details(
        &self,
        ticker_idx: usize,
        n: Option<usize>,
    ) -> Result<JsValue, JsValue> {
        to_js(
            &self
                .inner
                .drawdown_details(ticker_idx, n.unwrap_or(5))
                .map_err(to_js_err)?,
        )
    }

    /// Multi-factor regression statistics for one asset.
    /// @param ticker_idx - Zero-based ticker column index in tickerNames order.
    /// @param factor_returns - Matrix of aligned decimal factor-return series, one row per factor.
    #[wasm_bindgen(js_name = multiFactorGreeks)]
    pub fn multi_factor_greeks(
        &self,
        ticker_idx: usize,
        factor_returns: JsValue,
    ) -> Result<JsValue, JsValue> {
        let factors = parse_f64_matrix(factor_returns)?;
        let refs: Vec<&[f64]> = factors.iter().map(|v| v.as_slice()).collect();
        to_js(
            &self
                .inner
                .multi_factor_greeks(ticker_idx, &refs)
                .map_err(to_js_err)?,
        )
    }

    // ── Lookback & aggregation ──

    /// Standard lookback-window returns (MTD, QTD, YTD, ...) per asset.
    ///
    /// The FYTD window starts at the fiscal-year start
    /// (`fiscalYearStartMonth` / `fiscalYearStartDay`) adjusted to the next
    /// business day on `calendar` (default `"nyse"`); pass the calendar id
    /// matching your market for non-US panels.
    /// @param ref_date - ISO-8601 date on which MTD, QTD, YTD, and FYTD windows end.
    /// @param fiscal_year_start_month - Optional fiscal-year start month from 1 through 12; defaults to January.
    /// @param fiscal_year_start_day - Optional fiscal-year start day; defaults to the first day.
    /// @param calendar - Optional holiday-calendar id for FYTD adjustment; defaults to NYSE.
    #[wasm_bindgen(js_name = lookbackReturns)]
    pub fn lookback_returns(
        &self,
        ref_date: &str,
        fiscal_year_start_month: Option<u8>,
        fiscal_year_start_day: Option<u8>,
        calendar: Option<String>,
    ) -> Result<JsValue, JsValue> {
        let d = parse_iso_date(ref_date)?;
        let fc = make_fiscal_config(fiscal_year_start_month, fiscal_year_start_day)?;
        let cal =
            resolve_fiscal_calendar(calendar.as_deref().unwrap_or(DEFAULT_FISCAL_CALENDAR_ID))?;
        to_js(&self.inner.lookback_returns(d, fc, cal).map_err(to_js_err)?)
    }

    /// Aggregated period statistics for one asset at the given frequency.
    /// @param ticker_idx - Zero-based ticker column index in tickerNames order.
    /// @param agg_freq - Optional aggregation frequency token; defaults to monthly.
    /// @param fiscal_year_start_month - Optional fiscal-year start month from 1 through 12.
    /// @param fiscal_year_start_day - Optional fiscal-year start day within the selected month.
    #[wasm_bindgen(js_name = periodStats)]
    pub fn period_stats(
        &self,
        ticker_idx: usize,
        agg_freq: Option<String>,
        fiscal_year_start_month: Option<u8>,
        fiscal_year_start_day: Option<u8>,
    ) -> Result<JsValue, JsValue> {
        let pk = parse_freq(agg_freq.as_deref().unwrap_or("monthly"))?;
        let fc = make_fiscal_config(fiscal_year_start_month, fiscal_year_start_day)?;
        to_js(
            &self
                .inner
                .period_stats(ticker_idx, pk, Some(fc))
                .map_err(to_js_err)?,
        )
    }
}
