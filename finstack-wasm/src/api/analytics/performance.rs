//! WASM `Performance` class — the sole analytics entry point.
//!
//! Mirrors the Python `Performance` API (price- or return-panel construction,
//! every metric exposed as an instance method). Complex result types are
//! serialized to plain JS objects via `serde_wasm_bindgen` rather than
//! exposed as classes, keeping the JS facade simple.

use crate::utils::{date_to_iso, to_js_err};
use finstack_analytics as fa;
use finstack_core::dates::{CalendarRegistry, FiscalConfig, HolidayCalendar, PeriodKind};
use js_sys::{Array, Float64Array, Reflect};
use wasm_bindgen::prelude::*;

use super::support::{parse_f64_matrix, parse_f64_vec, parse_iso_date, parse_iso_dates};

const DEFAULT_FISCAL_START_MONTH: u8 = 1;
const DEFAULT_FISCAL_START_DAY: u8 = 1;
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

fn make_fiscal_config(month: Option<u8>) -> Result<FiscalConfig, JsValue> {
    FiscalConfig::new(
        month.unwrap_or(DEFAULT_FISCAL_START_MONTH),
        DEFAULT_FISCAL_START_DAY,
    )
    .map_err(to_js_err)
}

fn resolve_fiscal_calendar() -> Result<&'static dyn HolidayCalendar, JsValue> {
    CalendarRegistry::global()
        .resolve_str(DEFAULT_FISCAL_CALENDAR_ID)
        .ok_or_else(|| to_js_err(format!("calendar {DEFAULT_FISCAL_CALENDAR_ID:?} not found")))
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

fn result_vec_f64_to_js(result: finstack_core::Result<Vec<f64>>) -> Result<JsValue, JsValue> {
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
#[wasm_bindgen(js_name = Performance)]
pub struct WasmPerformance {
    inner: fa::Performance,
}

#[wasm_bindgen(js_class = Performance)]
impl WasmPerformance {
    /// Construct from a price matrix. `dates` is an array of ISO date strings,
    /// `prices` is `prices[i]` = column for ticker `i`.
    #[wasm_bindgen(constructor)]
    pub fn new(
        dates: JsValue,
        prices: JsValue,
        ticker_names: JsValue,
        benchmark_ticker: Option<String>,
        freq: Option<String>,
    ) -> Result<WasmPerformance, JsValue> {
        let panel = parse_panel_inputs(dates, prices, ticker_names, freq)?;
        let inner = fa::Performance::new(
            panel.dates,
            panel.values,
            panel.ticker_names,
            benchmark_ticker.as_deref(),
            panel.freq,
        )
        .map_err(to_js_err)?;
        Ok(WasmPerformance { inner })
    }

    /// Construct from a return matrix (one row per `dates` entry per ticker).
    #[wasm_bindgen(js_name = fromReturns)]
    pub fn from_returns(
        dates: JsValue,
        returns: JsValue,
        ticker_names: JsValue,
        benchmark_ticker: Option<String>,
        freq: Option<String>,
    ) -> Result<WasmPerformance, JsValue> {
        let panel = parse_panel_inputs(dates, returns, ticker_names, freq)?;
        let inner = fa::Performance::from_returns(
            panel.dates,
            panel.values,
            panel.ticker_names,
            benchmark_ticker.as_deref(),
            panel.freq,
        )
        .map_err(to_js_err)?;
        Ok(WasmPerformance { inner })
    }

    // ── Mutators ──

    /// Restrict subsequent analytics to `[start, end]`.
    #[wasm_bindgen(js_name = resetDateRange)]
    pub fn reset_date_range(&mut self, start: &str, end: &str) -> Result<(), JsValue> {
        self.inner
            .reset_date_range(parse_iso_date(start)?, parse_iso_date(end)?);
        Ok(())
    }

    /// Change the benchmark ticker.
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

    /// Active date grid as ISO date strings (`"YYYY-MM-DD"`).
    #[wasm_bindgen(js_name = dates)]
    pub fn dates(&self) -> Vec<String> {
        self.inner
            .active_dates()
            .iter()
            .map(|&d| date_to_iso(d))
            .collect()
    }

    // ── Scalar metrics ──

    /// Compound annual growth rate per asset.
    pub fn cagr(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.cagr())
    }

    #[wasm_bindgen(js_name = meanReturn)]
    /// Mean periodic return per asset (annualized by default).
    pub fn mean_return(&self, annualize: Option<bool>) -> JsValue {
        vec_f64_to_js(&self.inner.mean_return(annualize.unwrap_or(true)))
    }

    /// Return volatility per asset (annualized by default).
    pub fn volatility(&self, annualize: Option<bool>) -> JsValue {
        vec_f64_to_js(&self.inner.volatility(annualize.unwrap_or(true)))
    }

    /// Sharpe ratio per asset for the given risk-free rate.
    pub fn sharpe(&self, risk_free_rate: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.sharpe(risk_free_rate.unwrap_or(0.0)))
    }

    /// Sortino ratio per asset for the given minimum acceptable return.
    pub fn sortino(&self, mar: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.sortino(mar.unwrap_or(0.0)))
    }

    /// Calmar ratio (CAGR over max drawdown) per asset.
    pub fn calmar(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.calmar())
    }

    #[wasm_bindgen(js_name = meanDrawdown)]
    /// Mean drawdown per asset.
    pub fn mean_drawdown(&self) -> JsValue {
        vec_f64_to_js(&self.inner.mean_drawdown())
    }

    #[wasm_bindgen(js_name = maxDrawdown)]
    /// Maximum drawdown per asset.
    pub fn max_drawdown(&self) -> JsValue {
        vec_f64_to_js(&self.inner.max_drawdown())
    }

    #[wasm_bindgen(js_name = valueAtRisk)]
    /// Historical value-at-risk per asset at the given confidence level.
    pub fn value_at_risk(&self, confidence: Option<f64>) -> JsValue {
        vec_f64_to_js(
            &self
                .inner
                .value_at_risk(confidence.unwrap_or(DEFAULT_CONFIDENCE)),
        )
    }

    #[wasm_bindgen(js_name = expectedShortfall)]
    /// Expected shortfall (CVaR) per asset at the given confidence level.
    pub fn expected_shortfall(&self, confidence: Option<f64>) -> JsValue {
        vec_f64_to_js(
            &self
                .inner
                .expected_shortfall(confidence.unwrap_or(DEFAULT_CONFIDENCE)),
        )
    }

    #[wasm_bindgen(js_name = trackingError)]
    /// Tracking error versus the benchmark per asset.
    pub fn tracking_error(&self) -> JsValue {
        vec_f64_to_js(&self.inner.tracking_error())
    }

    #[wasm_bindgen(js_name = informationRatio)]
    /// Information ratio versus the benchmark per asset.
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

    #[wasm_bindgen(js_name = geometricMean)]
    /// Geometric mean return per asset.
    pub fn geometric_mean(&self) -> JsValue {
        vec_f64_to_js(&self.inner.geometric_mean())
    }

    #[wasm_bindgen(js_name = downsideDeviation)]
    /// Downside deviation per asset below the minimum acceptable return.
    pub fn downside_deviation(&self, mar: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.downside_deviation(mar.unwrap_or(0.0)))
    }

    #[wasm_bindgen(js_name = maxDrawdownDuration)]
    /// Longest drawdown duration (in periods) per asset.
    pub fn max_drawdown_duration(&self) -> Result<JsValue, JsValue> {
        // `usize` does not fit a typed array; keep the serde path.
        to_js(&self.inner.max_drawdown_duration())
    }

    #[wasm_bindgen(js_name = upCapture)]
    /// Upside capture ratio versus the benchmark per asset.
    pub fn up_capture(&self) -> JsValue {
        vec_f64_to_js(&self.inner.up_capture())
    }

    #[wasm_bindgen(js_name = downCapture)]
    /// Downside capture ratio versus the benchmark per asset.
    pub fn down_capture(&self) -> JsValue {
        vec_f64_to_js(&self.inner.down_capture())
    }

    #[wasm_bindgen(js_name = captureRatio)]
    /// Up/down capture ratio versus the benchmark per asset.
    pub fn capture_ratio(&self) -> JsValue {
        vec_f64_to_js(&self.inner.capture_ratio())
    }

    #[wasm_bindgen(js_name = omegaRatio)]
    /// Omega ratio per asset for the given threshold return.
    pub fn omega_ratio(&self, threshold: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.omega_ratio(threshold.unwrap_or(0.0)))
    }

    /// Treynor ratio per asset for the given risk-free rate.
    pub fn treynor(&self, risk_free_rate: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.treynor(risk_free_rate.unwrap_or(0.0)))
    }

    #[wasm_bindgen(js_name = gainToPain)]
    /// Gain-to-pain ratio per asset.
    pub fn gain_to_pain(&self) -> JsValue {
        vec_f64_to_js(&self.inner.gain_to_pain())
    }

    #[wasm_bindgen(js_name = ulcerIndex)]
    /// Ulcer index per asset.
    pub fn ulcer_index(&self) -> JsValue {
        vec_f64_to_js(&self.inner.ulcer_index())
    }

    #[wasm_bindgen(js_name = martinRatio)]
    /// Martin ratio (excess return over ulcer index) per asset.
    pub fn martin_ratio(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.martin_ratio())
    }

    #[wasm_bindgen(js_name = recoveryFactor)]
    /// Recovery factor (total return over max drawdown) per asset.
    pub fn recovery_factor(&self) -> JsValue {
        vec_f64_to_js(&self.inner.recovery_factor())
    }

    #[wasm_bindgen(js_name = painIndex)]
    /// Pain index (mean drawdown magnitude) per asset.
    pub fn pain_index(&self) -> JsValue {
        vec_f64_to_js(&self.inner.pain_index())
    }

    #[wasm_bindgen(js_name = painRatio)]
    /// Pain ratio (excess return over pain index) per asset.
    pub fn pain_ratio(&self, risk_free_rate: Option<f64>) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.pain_ratio(risk_free_rate.unwrap_or(0.0)))
    }

    #[wasm_bindgen(js_name = tailRatio)]
    /// Tail ratio of upper to lower return quantiles per asset.
    pub fn tail_ratio(&self, confidence: Option<f64>) -> JsValue {
        vec_f64_to_js(
            &self
                .inner
                .tail_ratio(confidence.unwrap_or(DEFAULT_CONFIDENCE)),
        )
    }

    #[wasm_bindgen(js_name = rSquared)]
    /// R-squared of returns against the benchmark per asset.
    pub fn r_squared(&self) -> JsValue {
        vec_f64_to_js(&self.inner.r_squared())
    }

    #[wasm_bindgen(js_name = battingAverage)]
    /// Share of periods beating the benchmark per asset.
    pub fn batting_average(&self) -> JsValue {
        vec_f64_to_js(&self.inner.batting_average())
    }

    #[wasm_bindgen(js_name = parametricVar)]
    /// Parametric (Gaussian) value-at-risk per asset.
    pub fn parametric_var(&self, confidence: Option<f64>) -> JsValue {
        vec_f64_to_js(
            &self
                .inner
                .parametric_var(confidence.unwrap_or(DEFAULT_CONFIDENCE)),
        )
    }

    #[wasm_bindgen(js_name = cornishFisherVar)]
    /// Cornish-Fisher adjusted value-at-risk per asset.
    pub fn cornish_fisher_var(&self, confidence: Option<f64>) -> JsValue {
        vec_f64_to_js(
            &self
                .inner
                .cornish_fisher_var(confidence.unwrap_or(DEFAULT_CONFIDENCE)),
        )
    }

    /// Conditional drawdown-at-risk per asset at the given confidence level.
    pub fn cdar(&self, confidence: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.cdar(confidence.unwrap_or(DEFAULT_CONFIDENCE)))
    }

    #[wasm_bindgen(js_name = mSquared)]
    /// M-squared (Modigliani) risk-adjusted return per asset.
    pub fn m_squared(&self, risk_free_rate: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.m_squared(risk_free_rate.unwrap_or(0.0)))
    }

    #[wasm_bindgen(js_name = modifiedSharpe)]
    /// Modified Sharpe ratio using Cornish-Fisher VaR per asset.
    pub fn modified_sharpe(&self, risk_free_rate: Option<f64>, confidence: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.modified_sharpe(
            risk_free_rate.unwrap_or(0.0),
            confidence.unwrap_or(DEFAULT_CONFIDENCE),
        ))
    }

    #[wasm_bindgen(js_name = sterlingRatio)]
    /// Sterling ratio over the `n` largest drawdowns per asset.
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

    #[wasm_bindgen(js_name = burkeRatio)]
    /// Burke ratio over the `n` largest drawdowns per asset.
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

    #[wasm_bindgen(js_name = cumulativeReturns)]
    /// Cumulative return series per asset.
    pub fn cumulative_returns(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.cumulative_returns())
    }

    #[wasm_bindgen(js_name = drawdownSeries)]
    /// Drawdown series per asset.
    pub fn drawdown_series(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.drawdown_series())
    }

    #[wasm_bindgen(js_name = correlationMatrix)]
    /// Pairwise return correlation matrix across assets.
    pub fn correlation_matrix(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.correlation_matrix())
    }

    #[wasm_bindgen(js_name = cumulativeReturnsOutperformance)]
    /// Cumulative outperformance versus the benchmark per asset.
    pub fn cumulative_returns_outperformance(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.cumulative_returns_outperformance())
    }

    #[wasm_bindgen(js_name = drawdownDifference)]
    /// Difference between asset and benchmark drawdown series.
    pub fn drawdown_difference(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.drawdown_difference())
    }

    #[wasm_bindgen(js_name = excessReturns)]
    /// Excess returns over the supplied risk-free series per asset.
    pub fn excess_returns(&self, rf: JsValue, nperiods: Option<f64>) -> Result<JsValue, JsValue> {
        let rf = parse_f64_vec(rf)?;
        Ok(matrix_f64_to_js(&self.inner.excess_returns(&rf, nperiods)))
    }

    // ── Benchmark ──

    /// SABR `beta` (backbone exponent).
    pub fn beta(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.beta())
    }

    /// Benchmark regression alpha/beta statistics per asset.
    pub fn greeks(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.greeks())
    }

    #[wasm_bindgen(js_name = rollingGreeks)]
    /// Rolling benchmark alpha/beta for one asset over a window.
    pub fn rolling_greeks(
        &self,
        ticker_idx: usize,
        window: Option<usize>,
    ) -> Result<JsValue, JsValue> {
        let rg = self
            .inner
            .rolling_greeks(ticker_idx, window.unwrap_or(DEFAULT_ROLLING_WINDOW))
            .map_err(to_js_err)?;
        rolling_greeks_to_js(&rg)
    }

    #[wasm_bindgen(js_name = rollingVolatility)]
    /// Rolling volatility series for one asset over a window.
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

    #[wasm_bindgen(js_name = rollingSortino)]
    /// Rolling Sortino ratio series for one asset over a window.
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

    #[wasm_bindgen(js_name = rollingSharpe)]
    /// Rolling Sharpe ratio series for one asset over a window.
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

    #[wasm_bindgen(js_name = rollingReturns)]
    /// Rolling compounded return series for one asset over a window.
    pub fn rolling_returns(&self, ticker_idx: usize, window: usize) -> Result<JsValue, JsValue> {
        let series = self
            .inner
            .rolling_returns(ticker_idx, window)
            .map_err(to_js_err)?;
        dated_series_to_js(&series.values, &series.dates, "return")
    }

    #[wasm_bindgen(js_name = drawdownDetails)]
    /// Details of the `n` largest drawdown episodes for one asset.
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

    #[wasm_bindgen(js_name = multiFactorGreeks)]
    /// Multi-factor regression statistics for one asset.
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

    #[wasm_bindgen(js_name = lookbackReturns)]
    /// Standard lookback-window returns (MTD, QTD, YTD, ...) per asset.
    pub fn lookback_returns(
        &self,
        ref_date: &str,
        fiscal_year_start_month: Option<u8>,
    ) -> Result<JsValue, JsValue> {
        let d = parse_iso_date(ref_date)?;
        let fc = make_fiscal_config(fiscal_year_start_month)?;
        let cal = resolve_fiscal_calendar()?;
        to_js(&self.inner.lookback_returns(d, fc, cal).map_err(to_js_err)?)
    }

    #[wasm_bindgen(js_name = periodStats)]
    /// Aggregated period statistics for one asset at the given frequency.
    pub fn period_stats(
        &self,
        ticker_idx: usize,
        agg_freq: Option<String>,
        fiscal_year_start_month: Option<u8>,
    ) -> Result<JsValue, JsValue> {
        let pk = parse_freq(agg_freq.as_deref().unwrap_or("monthly"))?;
        let fc = make_fiscal_config(fiscal_year_start_month)?;
        to_js(
            &self
                .inner
                .period_stats(ticker_idx, pk, Some(fc))
                .map_err(to_js_err)?,
        )
    }
}
