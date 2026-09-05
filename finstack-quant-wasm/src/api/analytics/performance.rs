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

const DEFAULT_FREQ: &str = "daily";
const DEFAULT_ROLLING_WINDOW: usize = 63;
const DEFAULT_CONFIDENCE: f64 = 0.95;

struct PanelInputs {
    dates: Vec<time::Date>,
    values: Vec<Vec<f64>>,
    ticker_names: Vec<String>,
    frequency: PeriodKind,
}

/// Parse a frequency token (`daily`, `weekly`, `monthly`, `quarterly`,
/// `semi_annual`, `annual` or a pandas offset alias `D`/`B`, `W`, `M`, `Q`,
/// `A`/`Y`); the descriptive error comes from core.
fn parse_frequency(frequency: &str) -> Result<PeriodKind, JsValue> {
    frequency.parse::<PeriodKind>().map_err(to_js_err)
}

/// Validate JavaScript numbers before the WASM ABI can truncate or wrap them.
fn parse_usize(value: f64, name: &str) -> Result<usize, JsValue> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > f64::from(u32::MAX) {
        return Err(to_js_err(format!(
            "{name} must be a finite non-negative integer no greater than 4294967295"
        )));
    }
    Ok(value as usize)
}

/// `None` when both parts are omitted so the Rust default (calendar year)
/// applies; a partial start fills the other half with `1`.
fn make_fiscal_config(
    month: Option<f64>,
    day: Option<f64>,
) -> Result<Option<FiscalConfig>, JsValue> {
    if month.is_none() && day.is_none() {
        return Ok(None);
    }
    let month = parse_usize(month.unwrap_or(1.0), "fiscalYearStartMonth")?;
    let day = parse_usize(day.unwrap_or(1.0), "fiscalYearStartDay")?;
    FiscalConfig::new(
        u8::try_from(month).map_err(|_| to_js_err("fiscalYearStartMonth exceeds 255"))?,
        u8::try_from(day).map_err(|_| to_js_err("fiscalYearStartDay exceeds 255"))?,
    )
    .map(Some)
    .map_err(to_js_err)
}

/// Lookback returns always need a fiscal config: default January 1.
fn lookback_fiscal_config(month: Option<f64>, day: Option<f64>) -> Result<FiscalConfig, JsValue> {
    match make_fiscal_config(month, day)? {
        Some(config) => Ok(config),
        None => FiscalConfig::new(1, 1).map_err(to_js_err),
    }
}

fn resolve_fiscal_calendar(calendar_id: &str) -> Result<&'static dyn HolidayCalendar, JsValue> {
    calendar_by_id(calendar_id)
        .ok_or_else(|| to_js_err(format!("calendar {calendar_id:?} not found")))
}

fn parse_cagr_day_count(day_count: Option<&str>) -> Result<fa::CagrDayCount, JsValue> {
    match day_count {
        None => Ok(fa::CagrDayCount::Act365_25),
        Some(label) => label.parse::<fa::CagrDayCount>().map_err(to_js_err),
    }
}

fn resolve_optional_calendar(
    calendar_id: Option<&str>,
) -> Result<Option<&'static dyn HolidayCalendar>, JsValue> {
    calendar_id.map(resolve_fiscal_calendar).transpose()
}

fn parse_return_kind(
    return_kind: Option<&str>,
    risk_free_rate: Option<f64>,
) -> Result<fa::ReturnKind, JsValue> {
    return_kind
        .unwrap_or("excess")
        .parse::<fa::ReturnKind>()
        .map(|kind| kind.with_risk_free_rate(risk_free_rate.unwrap_or(0.0)))
        .map_err(to_js_err)
}

fn parse_dates(dates: JsValue) -> Result<Vec<time::Date>, JsValue> {
    let strs: Vec<String> = serde_wasm_bindgen::from_value(dates).map_err(to_js_err)?;
    parse_iso_dates(&strs)
}

fn parse_panel_inputs(
    dates: JsValue,
    values: JsValue,
    ticker_names: JsValue,
    frequency: Option<String>,
) -> Result<PanelInputs, JsValue> {
    Ok(PanelInputs {
        dates: parse_dates(dates)?,
        values: parse_f64_matrix(values)?,
        ticker_names: serde_wasm_bindgen::from_value(ticker_names).map_err(to_js_err)?,
        frequency: parse_frequency(frequency.as_deref().unwrap_or(DEFAULT_FREQ))?,
    })
}

fn to_js<T: serde::Serialize>(value: &T) -> Result<JsValue, JsValue> {
    crate::utils::to_js_value(value)
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

/// Convert a ticker-major periodic-return panel to nested JavaScript arrays.
fn periodic_panel_to_js(panel: Vec<Vec<(time::Date, f64)>>) -> Result<JsValue, JsValue> {
    let outer = Array::new_with_length(panel.len() as u32);
    for (ticker_idx, series) in panel.into_iter().enumerate() {
        let points = Array::new_with_length(series.len() as u32);
        for (point_idx, (date, value)) in series.into_iter().enumerate() {
            let point = js_sys::Object::new();
            Reflect::set(
                &point,
                &JsValue::from_str("date"),
                &JsValue::from_str(&date_to_iso(date)),
            )?;
            Reflect::set(
                &point,
                &JsValue::from_str("value"),
                &JsValue::from_f64(value),
            )?;
            points.set(point_idx as u32, point.into());
        }
        outer.set(ticker_idx as u32, points.into());
    }
    Ok(outer.into())
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

fn beta_results_to_js(results: Vec<fa::BetaResult>) -> Result<JsValue, JsValue> {
    let array = Array::new_with_length(results.len() as u32);
    for (index, result) in results.into_iter().enumerate() {
        let value = obj_from_pairs(&[
            ("beta", JsValue::from_f64(result.beta)),
            ("std_err", JsValue::from_f64(result.std_err)),
            ("ci_lower", JsValue::from_f64(result.ci_lower)),
            ("ci_upper", JsValue::from_f64(result.ci_upper)),
        ])?;
        array.set(index as u32, value);
    }
    Ok(array.into())
}

fn greeks_results_to_js(results: Vec<fa::GreeksResult>) -> Result<JsValue, JsValue> {
    let array = Array::new_with_length(results.len() as u32);
    for (index, result) in results.into_iter().enumerate() {
        let value = obj_from_pairs(&[
            ("alpha", JsValue::from_f64(result.alpha)),
            ("beta", JsValue::from_f64(result.beta)),
            ("r_squared", JsValue::from_f64(result.r_squared)),
            (
                "adjusted_r_squared",
                JsValue::from_f64(result.adjusted_r_squared),
            ),
        ])?;
        array.set(index as u32, value);
    }
    Ok(array.into())
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
/// Dates are ISO-8601 values in ascending order. The `prices` and `returns`
/// supplied to the panel constructors are ticker-major and column-oriented;
/// matrix inputs to other methods follow their parameter documentation. Scalar
/// rates and returns use decimal fractions; numeric outputs are Float64Array
/// values in ticker order unless the method documents an object or matrix shape.
///
/// Invalid dates, shapes, frequencies, tickers, and confidence levels are
/// returned as rejected JsValue errors.
#[wasm_bindgen(js_name = Performance)]
pub struct JsPerformance {
    inner: fa::Performance,
}

#[wasm_bindgen(js_class = Performance)]
impl JsPerformance {
    /// Construct from a ticker-major, column-oriented price matrix. The outer
    /// element selects a ticker, and each inner series is aligned to `dates`.
    /// # Errors
    ///
    /// Rejects malformed dates or matrices, invalid prices, unsupported
    /// frequencies, and an unknown benchmark ticker.
    /// @param dates - ISO-8601 observation dates in ascending order, with one entry per value in each inner price series.
    /// @param prices - Ticker-major, column-oriented matrix where `prices[tickerIdx][dateIdx]` is the price for `tickerIdx` at `dates[dateIdx]`.
    /// @param ticker_names - Ticker labels aligned with the outer elements of `prices`.
    /// @param benchmark_ticker - Optional ticker label to use as the benchmark return series.
    /// @param frequency - Optional observation frequency token; defaults to daily.
    #[wasm_bindgen(constructor)]
    pub fn new(
        dates: JsValue,
        prices: JsValue,
        ticker_names: JsValue,
        benchmark_ticker: Option<String>,
        frequency: Option<String>,
    ) -> Result<JsPerformance, JsValue> {
        let panel = parse_panel_inputs(dates, prices, ticker_names, frequency)?;
        let inner = fa::Performance::new(
            panel.dates,
            panel.values,
            panel.ticker_names,
            benchmark_ticker.as_deref(),
            panel.frequency,
        )
        .map_err(to_js_err)?;
        Ok(JsPerformance { inner })
    }

    /// Construct from a ticker-major, column-oriented return matrix. The outer
    /// element selects a ticker, and each inner series is aligned to `dates`.
    /// # Errors
    ///
    /// Rejects malformed dates or matrices and invalid benchmark or
    /// frequency inputs.
    /// @param dates - ISO-8601 observation dates in ascending order, with one entry per value in each inner return series.
    /// @param returns - Ticker-major, column-oriented simple decimal return matrix where `returns[tickerIdx][dateIdx]` is the return for `tickerIdx` at `dates[dateIdx]`.
    /// @param ticker_names - Ticker labels aligned with the outer elements of `returns`.
    /// @param benchmark_ticker - Optional ticker label to use as the benchmark return series.
    /// @param frequency - Optional observation frequency token; defaults to daily.
    /// @returns A `Performance` handle over the supplied return panel.
    #[wasm_bindgen(js_name = fromReturns)]
    pub fn from_returns(
        dates: JsValue,
        returns: JsValue,
        ticker_names: JsValue,
        benchmark_ticker: Option<String>,
        frequency: Option<String>,
    ) -> Result<JsPerformance, JsValue> {
        let panel = parse_panel_inputs(dates, returns, ticker_names, frequency)?;
        let inner = fa::Performance::from_returns(
            panel.dates,
            panel.values,
            panel.ticker_names,
            benchmark_ticker.as_deref(),
            panel.frequency,
        )
        .map_err(to_js_err)?;
        Ok(JsPerformance { inner })
    }

    /// Restrict subsequent analytics to `[start, end]`.
    ///
    /// # Errors
    ///
    /// Rejects `start` or `end` when it is not a valid ISO-8601 calendar date.
    /// @param start - Inclusive ISO-8601 start date for the active analysis window.
    /// @param end - Inclusive ISO-8601 end date for the active analysis window.
    #[wasm_bindgen(js_name = resetDateRange)]
    pub fn reset_date_range(&mut self, start: &str, end: &str) -> Result<(), JsValue> {
        self.inner
            .reset_date_range(parse_iso_date(start)?, parse_iso_date(end)?);
        Ok(())
    }

    /// Change the benchmark ticker.
    ///
    /// # Errors
    ///
    /// Rejects `ticker` when it does not match a loaded ticker name.
    /// @param ticker - Existing ticker label to use as the benchmark return series.
    #[wasm_bindgen(js_name = resetBenchTicker)]
    pub fn reset_bench_ticker(&mut self, ticker: &str) -> Result<(), JsValue> {
        self.inner.reset_bench_ticker(ticker).map_err(to_js_err)
    }

    /// Ticker names in column order.
    ///
    /// # Errors
    ///
    /// Rejects if the ticker-name vector cannot be serialized to JavaScript.
    /// @returns Ticker labels in column order as a JavaScript string array.
    #[wasm_bindgen(js_name = tickerNames)]
    pub fn ticker_names(&self) -> Result<JsValue, JsValue> {
        to_js(&self.inner.ticker_names().to_vec())
    }

    /// Benchmark column index.
    /// @returns Zero-based index of the benchmark ticker in `tickerNames()`.
    #[wasm_bindgen(js_name = benchmarkIdx)]
    pub fn benchmark_idx(&self) -> usize {
        self.inner.benchmark_idx()
    }

    /// Observation frequency token.
    /// @returns Frequency string such as `"daily"` or `"monthly"`.
    #[wasm_bindgen(js_name = frequency)]
    pub fn frequency(&self) -> String {
        self.inner.frequency().to_string()
    }

    /// Full return-aligned date grid as ISO date strings (`"YYYY-MM-DD"`),
    /// independent of any active window — matches Rust `Performance::dates`.
    /// @returns Full panel dates as ISO-8601 strings, ignoring any `resetDateRange` window.
    #[wasm_bindgen(js_name = dates)]
    pub fn dates(&self) -> Vec<String> {
        self.inner.dates().iter().map(|&d| date_to_iso(d)).collect()
    }

    /// Date grid of the currently active analysis window as ISO date strings.
    /// Equal to `dates()` until `resetDateRange` narrows the window.
    /// @returns ISO-8601 dates of the active analysis window, in chronological order.
    #[wasm_bindgen(js_name = activeDates)]
    pub fn active_dates(&self) -> Vec<String> {
        self.inner
            .active_dates()
            .iter()
            .map(|&d| date_to_iso(d))
            .collect()
    }

    /// Date grid for one ticker's active return series as ISO date strings.
    ///
    /// # Errors
    ///
    /// Rejects when `ticker_idx` is outside the loaded ticker columns.
    /// @param ticker_idx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @returns ISO-8601 dates for that ticker's active return series, in chronological order.
    #[wasm_bindgen(js_name = activeDatesForTicker)]
    pub fn active_dates_for_ticker(&self, ticker_idx: f64) -> Result<Vec<String>, JsValue> {
        let ticker_idx = parse_usize(ticker_idx, "tickerIdx")?;
        Ok(self
            .inner
            .active_dates_for_ticker(ticker_idx)
            .map_err(to_js_err)?
            .iter()
            .map(|&d| date_to_iso(d))
            .collect())
    }

    /// Compound annual growth rate per asset.
    ///
    /// `dayCount` omitted or `"act365_25"` uses Act/365.25. Other values are
    /// core DayCount names such as `"act_365f"` or `"bus_252"`. `bus_252`
    /// requires `calendarId`.
    ///
    /// # Errors
    ///
    /// Rejects an unknown day-count or calendar id, a missing calendar when
    /// `bus_252` is requested, or a ticker whose active range has no
    /// positive holding period.
    /// @param day_count - Optional day-count: `"act365_25"` or a core name such as `"act_365f"`; defaults to Act/365.25.
    /// @param calendar_id - Optional holiday-calendar id; required for `bus_252`.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    pub fn cagr(
        &self,
        day_count: Option<String>,
        calendar_id: Option<String>,
    ) -> Result<JsValue, JsValue> {
        let day_count = parse_cagr_day_count(day_count.as_deref())?;
        let calendar = resolve_optional_calendar(calendar_id.as_deref())?;
        result_vec_f64_to_js(self.inner.cagr(day_count, calendar))
    }

    /// Mean periodic return per asset (annualized by default).
    /// @param annualize - Whether to annualize by the configured frequency; defaults to true.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = meanReturn)]
    pub fn mean_return(&self, annualize: Option<bool>) -> JsValue {
        vec_f64_to_js(&self.inner.mean_return(annualize.unwrap_or(true)))
    }

    /// Return volatility per asset (annualized by default).
    /// @param annualize - Whether to annualize by the configured frequency; defaults to true.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    pub fn volatility(&self, annualize: Option<bool>) -> JsValue {
        vec_f64_to_js(&self.inner.volatility(annualize.unwrap_or(true)))
    }

    /// Sharpe ratio per asset for the given risk-free rate.
    /// @param risk_free_rate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    pub fn sharpe(&self, risk_free_rate: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.sharpe(risk_free_rate.unwrap_or(0.0)))
    }

    /// Sortino ratio per asset for the given per-period minimum acceptable return.
    /// @param mar - Per-period minimum acceptable return as a decimal; defaults to 0.0.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    pub fn sortino(&self, mar: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.sortino(mar.unwrap_or(0.0)))
    }

    /// Calmar ratio (CAGR / |max drawdown|) over the active window, not
    /// Young's 36-month CTA definition.
    ///
    /// # Errors
    ///
    /// Rejects when any ticker's active range has no positive holding period
    /// and therefore cannot produce CAGR.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    pub fn calmar(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.calmar())
    }

    /// Mean drawdown per asset.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = meanDrawdown)]
    pub fn mean_drawdown(&self) -> JsValue {
        vec_f64_to_js(&self.inner.mean_drawdown())
    }

    /// Maximum drawdown per asset.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = maxDrawdown)]
    pub fn max_drawdown(&self) -> JsValue {
        vec_f64_to_js(&self.inner.max_drawdown())
    }

    /// Historical value-at-risk per asset at the given confidence level.
    /// @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    /// @throws Error - Rejects a `confidence` outside the open interval (0, 1).
    #[wasm_bindgen(js_name = valueAtRisk)]
    pub fn value_at_risk(&self, confidence: Option<f64>) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(
            self.inner
                .value_at_risk(confidence.unwrap_or(DEFAULT_CONFIDENCE)),
        )
    }

    /// Expected shortfall per asset: mean of exactly the worst `1-confidence`
    /// empirical probability mass, including a fractional boundary observation.
    /// @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    /// @throws Error - Rejects a `confidence` outside the open interval (0, 1).
    #[wasm_bindgen(js_name = expectedShortfall)]
    pub fn expected_shortfall(&self, confidence: Option<f64>) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(
            self.inner
                .expected_shortfall(confidence.unwrap_or(DEFAULT_CONFIDENCE)),
        )
    }

    /// Tracking error versus the benchmark per asset.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = trackingError)]
    pub fn tracking_error(&self) -> JsValue {
        vec_f64_to_js(&self.inner.tracking_error())
    }

    /// Information ratio versus the benchmark per asset.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = informationRatio)]
    pub fn information_ratio(&self) -> JsValue {
        vec_f64_to_js(&self.inner.information_ratio())
    }

    /// Return skewness per asset.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    pub fn skewness(&self) -> JsValue {
        vec_f64_to_js(&self.inner.skewness())
    }

    /// Excess kurtosis of returns per asset.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    pub fn kurtosis(&self) -> JsValue {
        vec_f64_to_js(&self.inner.kurtosis())
    }

    /// Geometric mean return per asset.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = geometricMean)]
    pub fn geometric_mean(&self) -> JsValue {
        vec_f64_to_js(&self.inner.geometric_mean())
    }

    /// Downside deviation per asset below the per-period minimum acceptable return.
    /// @param mar - Per-period minimum acceptable return as a decimal; defaults to 0.0.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = downsideDeviation)]
    pub fn downside_deviation(&self, mar: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.downside_deviation(mar.unwrap_or(0.0)))
    }

    /// Longest drawdown duration in calendar days per asset.
    ///
    /// # Errors
    ///
    /// Rejects if the duration vector cannot be serialized to JavaScript.
    /// @returns Per-ticker longest drawdown duration in calendar days, as a JavaScript number array.
    #[wasm_bindgen(js_name = maxDrawdownDuration)]
    pub fn max_drawdown_duration(&self) -> Result<JsValue, JsValue> {
        // `usize` does not fit a typed array; keep the serde path.
        to_js(&self.inner.max_drawdown_duration())
    }

    /// Empyrical-style annualized geometric up-capture versus the benchmark per asset.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = upCapture)]
    pub fn up_capture(&self) -> JsValue {
        vec_f64_to_js(&self.inner.up_capture())
    }

    /// Empyrical-style annualized geometric down-capture versus the benchmark per asset.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = downCapture)]
    pub fn down_capture(&self) -> JsValue {
        vec_f64_to_js(&self.inner.down_capture())
    }

    /// Empyrical-style annualized geometric up/down capture ratio versus the benchmark per asset.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = captureRatio)]
    pub fn capture_ratio(&self) -> JsValue {
        vec_f64_to_js(&self.inner.capture_ratio())
    }

    /// Omega ratio per asset for the given threshold return.
    /// @param threshold - Per-period threshold return as a decimal; defaults to 0.0.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = omegaRatio)]
    pub fn omega_ratio(&self, threshold: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.omega_ratio(threshold.unwrap_or(0.0)))
    }

    /// Treynor ratio per asset for the given risk-free rate.
    /// @param risk_free_rate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    pub fn treynor(&self, risk_free_rate: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.treynor(risk_free_rate.unwrap_or(0.0)))
    }

    /// Gain-to-pain ratio per asset.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = gainToPain)]
    pub fn gain_to_pain(&self) -> JsValue {
        vec_f64_to_js(&self.inner.gain_to_pain())
    }

    /// Ulcer index per asset.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = ulcerIndex)]
    pub fn ulcer_index(&self) -> JsValue {
        vec_f64_to_js(&self.inner.ulcer_index())
    }

    /// Martin ratio (excess return over ulcer index) per asset.
    ///
    /// # Errors
    ///
    /// Rejects when any ticker's active range has no positive holding period
    /// and therefore cannot produce CAGR.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = martinRatio)]
    pub fn martin_ratio(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.martin_ratio())
    }

    /// Recovery factor (total return over max drawdown) per asset.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = recoveryFactor)]
    pub fn recovery_factor(&self) -> JsValue {
        vec_f64_to_js(&self.inner.recovery_factor())
    }

    /// Pain index (mean drawdown magnitude) per asset.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = painIndex)]
    pub fn pain_index(&self) -> JsValue {
        vec_f64_to_js(&self.inner.pain_index())
    }

    /// Pain ratio (excess return over pain index) per asset.
    ///
    /// # Errors
    ///
    /// Rejects when any ticker's active range has no positive holding period
    /// and therefore cannot produce CAGR.
    /// @param risk_free_rate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = painRatio)]
    pub fn pain_ratio(&self, risk_free_rate: Option<f64>) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.pain_ratio(risk_free_rate.unwrap_or(0.0)))
    }

    /// Tail ratio of upper to lower return quantiles per asset.
    /// @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    /// @throws Error - Rejects a `confidence` outside the open interval (0, 1).
    #[wasm_bindgen(js_name = tailRatio)]
    pub fn tail_ratio(&self, confidence: Option<f64>) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(
            self.inner
                .tail_ratio(confidence.unwrap_or(DEFAULT_CONFIDENCE)),
        )
    }

    /// R-squared of returns against the benchmark per asset.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = rSquared)]
    pub fn r_squared(&self) -> JsValue {
        vec_f64_to_js(&self.inner.r_squared())
    }

    /// Share of periods beating the benchmark per asset.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = battingAverage)]
    pub fn batting_average(&self) -> JsValue {
        vec_f64_to_js(&self.inner.batting_average())
    }

    /// Equal-weight Gaussian value-at-risk per asset.
    ///
    /// `horizonPeriods` omitted is one-period VaR. A positive `h` scales
    /// mean by `h` and volatility by `√h`.
    /// @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
    /// @param horizon_periods - Optional horizon in observation periods; omitted is one-period VaR.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    /// @throws Error - Rejects a `confidence` outside the open interval (0, 1).
    #[wasm_bindgen(js_name = parametricVar)]
    pub fn parametric_var(
        &self,
        confidence: Option<f64>,
        horizon_periods: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(
            self.inner
                .parametric_var(confidence.unwrap_or(DEFAULT_CONFIDENCE), horizon_periods),
        )
    }

    /// Cornish-Fisher adjusted value-at-risk per asset.
    ///
    /// `horizonPeriods` omitted is one-period VaR. A positive `h` scales
    /// Cornish–Fisher moments to that horizon.
    /// @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
    /// @param horizon_periods - Optional horizon in observation periods; omitted is one-period VaR.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    /// @throws Error - Rejects a `confidence` outside the open interval (0, 1).
    #[wasm_bindgen(js_name = cornishFisherVar)]
    pub fn cornish_fisher_var(
        &self,
        confidence: Option<f64>,
        horizon_periods: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(
            self.inner
                .cornish_fisher_var(confidence.unwrap_or(DEFAULT_CONFIDENCE), horizon_periods),
        )
    }

    /// Conditional drawdown-at-risk per asset: mean of exactly the worst
    /// `1-confidence` drawdown mass, including fractional boundary weighting.
    /// @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    /// @throws Error - Rejects a `confidence` outside the open interval (0, 1).
    pub fn cdar(&self, confidence: Option<f64>) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.cdar(confidence.unwrap_or(DEFAULT_CONFIDENCE)))
    }

    /// Linearly annualized M-squared per asset. Cash subtraction and addition
    /// both use the decompounded period cash rate multiplied by periods per year.
    /// @param risk_free_rate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = mSquared)]
    pub fn m_squared(&self, risk_free_rate: Option<f64>) -> JsValue {
        vec_f64_to_js(&self.inner.m_squared(risk_free_rate.unwrap_or(0.0)))
    }

    /// Modified Sharpe ratio using annualized excess return and
    /// corresponding-annual-horizon Cornish-Fisher VaR per asset.
    ///
    /// The panel frequency supplies the periods-per-year scaling for both
    /// terms, including the horizon decay of skewness and excess kurtosis;
    /// the denominator is not one-period VaR.
    /// @param risk_free_rate - Annualized decimal risk-free rate, decompounded
    /// to the panel frequency before constructing annualized excess return;
    /// defaults to 0.0.
    /// @param confidence - Annual-horizon tail confidence as a decimal
    /// probability; defaults to 0.95.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    /// @throws Error - Rejects a `confidence` outside the open interval (0, 1).
    #[wasm_bindgen(js_name = modifiedSharpe)]
    pub fn modified_sharpe(
        &self,
        risk_free_rate: Option<f64>,
        confidence: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.modified_sharpe(
            risk_free_rate.unwrap_or(0.0),
            confidence.unwrap_or(DEFAULT_CONFIDENCE),
        ))
    }

    /// Sterling ratio over the `n` largest drawdowns per asset.
    ///
    /// # Errors
    ///
    /// Rejects when any ticker's active range has no positive holding period
    /// and therefore cannot produce CAGR.
    /// @param risk_free_rate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @param n - Finite non-negative integer count of largest drawdowns; defaults to 5. Invalid numeric values are rejected.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = sterlingRatio)]
    pub fn sterling_ratio(
        &self,
        risk_free_rate: Option<f64>,
        n: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        let n = n.map(|value| parse_usize(value, "n")).transpose()?;
        result_vec_f64_to_js(
            self.inner
                .sterling_ratio(risk_free_rate.unwrap_or(0.0), n.unwrap_or(5)),
        )
    }

    /// Burke ratio over the `n` largest drawdowns per asset.
    ///
    /// # Errors
    ///
    /// Rejects when any ticker's active range has no positive holding period
    /// and therefore cannot produce CAGR.
    /// @param risk_free_rate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @param n - Finite non-negative integer count of largest drawdowns; defaults to 5. Invalid numeric values are rejected.
    /// @returns Per-ticker values as a Float64Array in `tickerNames()` order.
    #[wasm_bindgen(js_name = burkeRatio)]
    pub fn burke_ratio(
        &self,
        risk_free_rate: Option<f64>,
        n: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        let n = n.map(|value| parse_usize(value, "n")).transpose()?;
        result_vec_f64_to_js(
            self.inner
                .burke_ratio(risk_free_rate.unwrap_or(0.0), n.unwrap_or(5)),
        )
    }

    /// Per-period simple return series per asset, as decimal fractions.
    ///
    /// Canonical accessor for the raw return panel over the active window;
    /// prefer it over `excessReturns` with an all-zero risk-free series or
    /// un-compounding `cumulativeReturns`. Series are span-aware and therefore
    /// ragged across assets on edge-ragged panels.
    /// @returns One Float64Array of simple decimal returns per ticker in `tickerNames()` order.
    #[wasm_bindgen(js_name = returns)]
    pub fn returns(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.returns())
    }

    /// Per-period simple return series for one asset, as decimal fractions.
    ///
    /// # Errors
    ///
    /// Rejects when `ticker_idx` is outside the loaded ticker columns.
    /// @param ticker_idx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @returns Simple decimal returns for the selected ticker, in date order.
    #[wasm_bindgen(js_name = returnsForTicker)]
    pub fn returns_for_ticker(&self, ticker_idx: f64) -> Result<JsValue, JsValue> {
        let ticker_idx = parse_usize(ticker_idx, "tickerIdx")?;
        let series = self
            .inner
            .returns_for_ticker(ticker_idx)
            .map_err(to_js_err)?;
        Ok(vec_f64_to_js(&series))
    }

    /// Cumulative return series per asset.
    /// @returns One Float64Array per ticker in `tickerNames()` order.
    #[wasm_bindgen(js_name = cumulativeReturns)]
    pub fn cumulative_returns(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.cumulative_returns())
    }

    /// The standard per-ticker summary as a table envelope.
    ///
    /// One row per ticker with 22 metric columns (`cagr`, `mean_return`,
    /// `volatility`, `sharpe`, `sortino`, `calmar`, `max_drawdown`,
    /// `value_at_risk`, `expected_shortfall`, `tracking_error`,
    /// `information_ratio`, `skewness`, `kurtosis`, `geometric_mean`,
    /// `downside_deviation`, `omega_ratio`, `gain_to_pain`, `ulcer_index`,
    /// `pain_index`, `recovery_factor`, `tail_ratio`, `r_squared`) plus a
    /// leading `ticker` dimension column. Same rows as the Python
    /// `Performance.to_summary_dataframe`.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the panel is too short to annualize
    /// (`cagr` / `calmar`) or the table cannot be converted.
    /// @param risk_free_rate - Annualized risk-free rate as a decimal (0.02 = 2%); affects only `sharpe`. Defaults to 0.0.
    /// @param confidence - Tail confidence as a decimal probability applied to VaR, ES and tail ratio; defaults to 0.95.
    #[wasm_bindgen(js_name = summary)]
    pub fn summary(
        &self,
        risk_free_rate: Option<f64>,
        confidence: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        let table = self
            .inner
            .summary(
                risk_free_rate.unwrap_or(0.0),
                confidence.unwrap_or(DEFAULT_CONFIDENCE),
            )
            .map_err(to_js_err)?;
        to_js(&table)
    }

    /// Calendar-bucketed compounded returns per ticker.
    ///
    /// The outer array is ticker-major in `tickerNames()` order. Each inner
    /// array contains chronological `{ date, value }` points, where `date` is
    /// the bucket's ISO-8601 period-end date and `value` is a simple decimal
    /// return (`0.01` means 1%). Chaining one ticker's values reconciles with
    /// its final `cumulativeReturns()` value.
    ///
    /// # Arguments
    ///
    /// * `frequency` - Optional calendar frequency token: `"daily"`,
    ///   `"weekly"`, `"monthly"`, `"quarterly"`, `"semi_annual"`, or
    ///   `"annual"` (pandas offset aliases `D`/`B`, `W`, `M`, `Q`, `A`/`Y` are accepted too); defaults to `"monthly"`.
    ///
    /// # Errors
    ///
    /// Rejects an unsupported frequency or a failure to create a point
    /// property on the JavaScript result object.
    /// @returns Ticker-major nested arrays of chronological period-end points with simple decimal returns.
    #[wasm_bindgen(js_name = periodicReturns)]
    pub fn periodic_returns(&self, frequency: Option<String>) -> Result<JsValue, JsValue> {
        let kind = parse_frequency(frequency.as_deref().unwrap_or("monthly"))?;
        periodic_panel_to_js(self.inner.periodic_returns(kind))
    }

    /// Drawdown series per asset.
    /// @returns One Float64Array per ticker in `tickerNames()` order.
    #[wasm_bindgen(js_name = drawdownSeries)]
    pub fn drawdown_series(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.drawdown_series())
    }

    /// Return correlation matrix across assets.
    ///
    /// Uses the complete-case common window when every ticker has at least
    /// two overlapping points; otherwise pairwise intersecting spans, then
    /// Higham repair.
    ///
    /// # Errors
    ///
    /// Rejects a degenerate pair or a matrix that cannot be repaired to a
    /// valid correlation matrix.
    /// @returns Square correlation matrix as nested Float64Array rows in `tickerNames()` order.
    #[wasm_bindgen(js_name = correlationMatrix)]
    pub fn correlation_matrix(&self) -> Result<JsValue, JsValue> {
        let matrix = self.inner.correlation_matrix().map_err(to_js_err)?;
        Ok(matrix_f64_to_js(&matrix))
    }

    /// `true` when `correlationMatrix()` had to be Higham-repaired to the
    /// nearest valid correlation matrix (ragged panels can yield a raw
    /// pairwise estimate that is not positive semi-definite).
    ///
    /// # Errors
    ///
    /// Rejects the same degenerate-pair conditions as `correlationMatrix`.
    /// @returns `true` when the estimate was projected to the nearest correlation matrix.
    /// @throws Error - Rejects when a ticker pair is degenerate or Higham repair fails.
    #[wasm_bindgen(js_name = correlationMatrixRepaired)]
    pub fn correlation_matrix_repaired(&self) -> Result<bool, JsValue> {
        self.inner
            .correlation_matrix_with_repair_flag()
            .map(|(_, repaired)| repaired)
            .map_err(to_js_err)
    }

    /// Cumulative outperformance versus the benchmark per asset.
    /// @returns One Float64Array per ticker in `tickerNames()` order.
    #[wasm_bindgen(js_name = cumulativeReturnsOutperformance)]
    pub fn cumulative_returns_outperformance(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.cumulative_returns_outperformance())
    }

    /// Difference between asset and benchmark drawdown series.
    /// @returns One Float64Array per ticker in `tickerNames()` order.
    #[wasm_bindgen(js_name = drawdownDifference)]
    pub fn drawdown_difference(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.drawdown_difference())
    }

    /// Excess returns over the supplied risk-free series per asset.
    ///
    /// `rf` must have one value per active panel date. `nperiods` omitted
    /// geometrically decompounds an annual series using the engine frequency;
    /// pass `1.0` when `rf` is already periodic.
    ///
    /// # Errors
    ///
    /// Rejects when `rf` is neither a numeric JavaScript array nor a
    /// `Float64Array`, or when its length differs from the active panel.
    /// @param rf - Risk-free return series as decimal values aligned with active panel dates.
    /// @param nperiods - Optional periods per year used to decompound annual `rf`; omit to use the engine frequency, or pass `1` for already-periodic `rf`.
    /// @returns One Float64Array per ticker in `tickerNames()` order.
    #[wasm_bindgen(js_name = excessReturns)]
    pub fn excess_returns(&self, rf: JsValue, nperiods: Option<f64>) -> Result<JsValue, JsValue> {
        let rf = parse_f64_vec(rf)?;
        let excess = self
            .inner
            .excess_returns(&rf, nperiods)
            .map_err(to_js_err)?;
        Ok(matrix_f64_to_js(&excess))
    }

    /// OLS beta versus the benchmark per asset, with standard error and 95% CI.
    ///
    /// # Errors
    ///
    /// Rejects if the beta results cannot be serialized to JavaScript.
    /// @returns Per-ticker `{ beta, std_err, ci_lower, ci_upper }` objects in `tickerNames()` order.
    pub fn beta(&self) -> Result<JsValue, JsValue> {
        beta_results_to_js(self.inner.beta())
    }

    /// Benchmark regression annualized Jensen alpha/beta statistics per asset.
    ///
    /// # Errors
    ///
    /// Rejects if the regression results cannot be serialized to JavaScript.
    /// @param risk_free_rate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @returns Per-ticker `{ alpha, beta, r_squared, adjusted_r_squared }` objects in `tickerNames()` order.
    pub fn greeks(&self, risk_free_rate: Option<f64>) -> Result<JsValue, JsValue> {
        greeks_results_to_js(self.inner.greeks(risk_free_rate.unwrap_or(0.0)))
    }

    /// Rolling benchmark annualized Jensen alpha/beta for one asset over a window.
    ///
    /// # Errors
    ///
    /// Rejects when `ticker_idx` is outside the loaded ticker columns or the
    /// JavaScript result object's properties cannot be created.
    /// @param ticker_idx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @param window - Finite positive integer observation count; defaults to 63 periods. Invalid numeric values are rejected.
    /// @param risk_free_rate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @returns `{ dates, alphas, betas }` series for the selected ticker.
    #[wasm_bindgen(js_name = rollingGreeks)]
    pub fn rolling_greeks(
        &self,
        ticker_idx: f64,
        window: Option<f64>,
        risk_free_rate: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        let ticker_idx = parse_usize(ticker_idx, "tickerIdx")?;
        let window = window
            .map(|value| parse_usize(value, "window"))
            .transpose()?;
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
    ///
    /// # Errors
    ///
    /// Rejects when `ticker_idx` is outside the loaded ticker columns or the
    /// JavaScript result object's properties cannot be created.
    /// @param ticker_idx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @param window - Finite positive integer observation count; defaults to 63 periods. Invalid numeric values are rejected.
    /// @returns `{ dates, volatility }` series for the selected ticker.
    #[wasm_bindgen(js_name = rollingVolatility)]
    pub fn rolling_volatility(
        &self,
        ticker_idx: f64,
        window: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        let ticker_idx = parse_usize(ticker_idx, "tickerIdx")?;
        let window = window
            .map(|value| parse_usize(value, "window"))
            .transpose()?;
        let series = self
            .inner
            .rolling_volatility(ticker_idx, window.unwrap_or(DEFAULT_ROLLING_WINDOW))
            .map_err(to_js_err)?;
        dated_series_to_js(&series.values, &series.dates, "volatility")
    }

    /// Rolling Sortino ratio series for one asset over a window.
    ///
    /// # Errors
    ///
    /// Rejects when `ticker_idx` is outside the loaded ticker columns or the
    /// JavaScript result object's properties cannot be created.
    /// @param ticker_idx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @param window - Finite positive integer observation count; defaults to 63 periods. Invalid numeric values are rejected.
    /// @param mar - Per-period minimum acceptable return as a decimal; defaults to 0.0.
    /// @returns `{ dates, sortino }` series for the selected ticker.
    #[wasm_bindgen(js_name = rollingSortino)]
    pub fn rolling_sortino(
        &self,
        ticker_idx: f64,
        window: Option<f64>,
        mar: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        let ticker_idx = parse_usize(ticker_idx, "tickerIdx")?;
        let window = window
            .map(|value| parse_usize(value, "window"))
            .transpose()?;
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
    ///
    /// # Errors
    ///
    /// Rejects when `ticker_idx` is outside the loaded ticker columns or the
    /// JavaScript result object's properties cannot be created.
    /// @param ticker_idx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @param window - Finite positive integer observation count; defaults to 63 periods. Invalid numeric values are rejected.
    /// @param risk_free_rate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @returns `{ dates, sharpe }` series for the selected ticker.
    #[wasm_bindgen(js_name = rollingSharpe)]
    pub fn rolling_sharpe(
        &self,
        ticker_idx: f64,
        window: Option<f64>,
        risk_free_rate: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        let ticker_idx = parse_usize(ticker_idx, "tickerIdx")?;
        let window = window
            .map(|value| parse_usize(value, "window"))
            .transpose()?;
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
    ///
    /// # Errors
    ///
    /// Rejects when `ticker_idx` is outside the loaded ticker columns or the
    /// JavaScript result object's properties cannot be created. An
    /// overlong `window` returns an empty series; a zero window is rejected.
    /// @param ticker_idx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @param window - Finite positive integer observation count. Zero, fractional, non-finite, and out-of-range values are rejected.
    /// @returns `{ dates, return }` series for the selected ticker.
    #[wasm_bindgen(js_name = rollingReturns)]
    pub fn rolling_returns(&self, ticker_idx: f64, window: f64) -> Result<JsValue, JsValue> {
        let ticker_idx = parse_usize(ticker_idx, "tickerIdx")?;
        let window = parse_usize(window, "window")?;
        let series = self
            .inner
            .rolling_returns(ticker_idx, window)
            .map_err(to_js_err)?;
        dated_series_to_js(&series.values, &series.dates, "return")
    }

    /// Details of the `n` largest drawdown episodes for one asset.
    ///
    /// # Errors
    ///
    /// Rejects when `ticker_idx` is outside the loaded ticker columns or the
    /// drawdown details cannot be serialized to JavaScript.
    /// @param ticker_idx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @param n - Finite non-negative integer count of episodes; defaults to 5. Invalid numeric values are rejected.
    /// @returns Drawdown episode objects for the selected ticker, largest first.
    #[wasm_bindgen(js_name = drawdownDetails)]
    pub fn drawdown_details(&self, ticker_idx: f64, n: Option<f64>) -> Result<JsValue, JsValue> {
        let ticker_idx = parse_usize(ticker_idx, "tickerIdx")?;
        let n = n.map(|value| parse_usize(value, "n")).transpose()?;
        to_js(
            &self
                .inner
                .drawdown_details(ticker_idx, n.unwrap_or(5))
                .map_err(to_js_err)?,
        )
    }

    /// Multi-factor regression statistics for one asset.
    ///
    /// Factor series are already-excess. `returnKind` `"excess"` leaves the
    /// ticker series unchanged; `"total"` subtracts the geometrically
    /// decompounded period risk-free rate from the ticker series only.
    ///
    /// # Errors
    ///
    /// Rejects a non-numeric `factor_returns` matrix, an unknown
    /// `returnKind`, an out-of-range `ticker_idx`, no factors, too few
    /// observations, non-finite or length-mismatched inputs, a singular
    /// factor design, or a result that cannot be serialized to JavaScript.
    /// @param ticker_idx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @param factor_returns - Matrix of aligned already-excess decimal factor-return series, one row per factor.
    /// @param return_kind - `"excess"` or `"total"`; defaults to `"excess"`.
    /// @param risk_free_rate - Annualized decimal risk-free rate used when `returnKind` is `"total"`; defaults to 0.0.
    /// @returns `{ alpha, betas, r_squared, adjusted_r_squared, residual_vol }` for the selected ticker.
    #[wasm_bindgen(js_name = multiFactorGreeks)]
    pub fn multi_factor_greeks(
        &self,
        ticker_idx: f64,
        factor_returns: JsValue,
        return_kind: Option<String>,
        risk_free_rate: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        let ticker_idx = parse_usize(ticker_idx, "tickerIdx")?;
        let factors = parse_f64_matrix(factor_returns)?;
        let refs: Vec<&[f64]> = factors.iter().map(|v| v.as_slice()).collect();
        let kind = parse_return_kind(return_kind.as_deref(), risk_free_rate)?;
        let result = self
            .inner
            .multi_factor_greeks(ticker_idx, &refs, kind)
            .map_err(to_js_err)?;
        let js = to_js(&result)?;
        // JSON uses explicit non-finite sentinels; the JS API stays numeric.
        Reflect::set(
            &js,
            &"r_squared".into(),
            &JsValue::from_f64(result.r_squared),
        )?;
        Reflect::set(
            &js,
            &"adjusted_r_squared".into(),
            &JsValue::from_f64(result.adjusted_r_squared),
        )?;
        Ok(js)
    }

    /// Standard lookback-window returns (MTD, QTD, YTD, ...) per asset.
    ///
    /// FYTD is the first observation on or after the fiscal calendar start
    /// through `ref_date`. Holidays are not skipped. The first included
    /// simple return still spans the prior close. The serialized result always
    /// contains `{ mtd, qtd, ytd, fytd }`, with `fytd` as a numeric array.
    ///
    /// # Arguments
    ///
    /// * `ref_date` - ISO-8601 date on which MTD, QTD, YTD, and FYTD windows end.
    /// * `fiscal_year_start_month` - Optional fiscal-year start month from 1
    ///   through 12; defaults to January.
    /// * `fiscal_year_start_day` - Optional fiscal-year start day; defaults to
    ///   the first day of the month.
    ///
    /// # Errors
    ///
    /// Rejects an invalid ISO `ref_date`, a fiscal month outside `1..=12`, a
    /// fiscal day outside `1..=31`, or a result that cannot be serialized to
    /// JavaScript.
    /// @param ref_date - ISO-8601 date on which MTD, QTD, YTD, and FYTD windows end.
    /// @param fiscal_year_start_month - Optional fiscal-year start month from 1 through 12; defaults to January.
    /// @param fiscal_year_start_day - Optional fiscal-year start day; defaults to the first day.
    /// @returns Per-ticker `{ mtd, qtd, ytd, fytd }` numeric arrays of lookback returns as decimal fractions; `fytd` is never null.
    #[wasm_bindgen(js_name = lookbackReturns)]
    pub fn lookback_returns(
        &self,
        ref_date: &str,
        fiscal_year_start_month: Option<f64>,
        fiscal_year_start_day: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        let d = parse_iso_date(ref_date)?;
        let fc = lookback_fiscal_config(fiscal_year_start_month, fiscal_year_start_day)?;
        to_js(&self.inner.lookback_returns(d, fc))
    }

    /// Aggregated period statistics for one asset at the given frequency.
    ///
    /// # Errors
    ///
    /// Rejects an unsupported `aggregation_frequency`, a fiscal month outside
    /// `1..=12`, a fiscal day outside `1..=31`, an out-of-range `ticker_idx`,
    /// or period statistics that cannot be serialized to JavaScript.
    /// @param ticker_idx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @param aggregation_frequency - Optional aggregation frequency token; defaults to monthly.
    /// @param fiscal_year_start_month - Optional fiscal-year start month from 1 through 12.
    /// @param fiscal_year_start_day - Optional fiscal-year start day within the selected month.
    /// @returns Period statistics object for the selected ticker at the requested frequency.
    #[wasm_bindgen(js_name = periodStats)]
    pub fn period_stats(
        &self,
        ticker_idx: f64,
        aggregation_frequency: Option<String>,
        fiscal_year_start_month: Option<f64>,
        fiscal_year_start_day: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        let ticker_idx = parse_usize(ticker_idx, "tickerIdx")?;
        let pk = parse_frequency(aggregation_frequency.as_deref().unwrap_or("monthly"))?;
        let fc = make_fiscal_config(fiscal_year_start_month, fiscal_year_start_day)?;
        let stats = self
            .inner
            .period_stats(ticker_idx, pk, fc)
            .map_err(to_js_err)?;
        let js = to_js(&stats)?;
        restore_non_finite_ratios(&js, &stats)?;
        Ok(js)
    }
}

/// Restore `PeriodStats` ratios that serde encoded as sentinel strings.
///
/// The four ratio fields carry `#[serde(with = "core::wire::non_finite_f64")]`
/// so the JSON wire form can round-trip `+∞`: `serde_json` writes a bare
/// `f64::INFINITY` as `null` and then refuses to read `null` back as an `f64`.
/// JavaScript numbers have no such limitation — `Infinity` is an ordinary
/// number — but `serde_wasm_bindgen` still runs that adapter and hands JS the
/// string `"inf"`, which breaks consumers silently (`"inf" > 2` is `false`
/// where `Infinity > 2` is `true`). Overwrite those four keys with the real
/// `f64`s; finite values are unchanged by the round trip.
///
/// # Arguments
///
/// * `js` - Serialized `PeriodStats` object to patch in place.
/// * `stats` - Source statistics holding the unencoded `f64` ratios.
///
/// # Errors
///
/// Propagates any failure from `Reflect::set`.
fn restore_non_finite_ratios(js: &JsValue, stats: &fa::PeriodStats) -> Result<(), JsValue> {
    for (key, value) in [
        ("payoff_ratio", stats.payoff_ratio),
        ("profit_factor", stats.profit_factor),
        ("cpc_ratio", stats.cpc_ratio),
        ("kelly_criterion", stats.kelly_criterion),
    ] {
        Reflect::set(js, &JsValue::from_str(key), &JsValue::from_f64(value))?;
    }
    Ok(())
}
