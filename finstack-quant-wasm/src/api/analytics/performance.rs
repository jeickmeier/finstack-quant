//! WASM `Performance` class — the analytics panel entry point.
//!
//! Mirrors the Python `Performance` API (price- or return-panel construction,
//! every metric exposed as an instance method). Complex results are plain JS
//! objects rather than classes: serde-shaped results go through the
//! `crate::utils` serializers, while `DatedSeries`, `RollingGreeks` and the
//! `correlationMatrix` result are assembled by hand so their numeric columns
//! arrive as `Float64Array`. Omitted arguments resolve to the Rust-owned
//! `finstack_quant_analytics::DEFAULT_*` constants.

use crate::api::core::periods::JsFiscalConfig;
use crate::utils::input::{
    from_js_json, js_f64_matrix, js_f64_seq, js_opt_bool, js_opt_f64, js_opt_string, js_opt_uint,
    js_string, js_uint, json_text,
};
use crate::utils::{
    date_to_iso, parse_iso_date, to_js_err, to_js_rows_numeric, to_js_value, to_js_value_numeric,
};
use finstack_quant_analytics::{
    CagrDayCount, DEFAULT_ANNUALIZE, DEFAULT_CONFIDENCE, DEFAULT_DRAWDOWN_COUNT, DEFAULT_MAR,
    DEFAULT_PERIODIC_FREQUENCY, DEFAULT_RISK_FREE_RATE, DEFAULT_ROLLING_WINDOW, DatedSeries,
    Performance, PerformancePanel, ReturnKind, RollingGreeks,
};
use finstack_quant_core::dates::{HolidayCalendar, PeriodKind, calendar_by_id_strict};
use js_sys::{Array, Float64Array, Reflect};
use wasm_bindgen::prelude::*;

/// Parse a canonical frequency label, or use the Rust default when omitted.
fn parse_frequency(frequency: Option<&str>, default: PeriodKind) -> Result<PeriodKind, JsValue> {
    frequency
        .map(str::parse::<PeriodKind>)
        .transpose()
        .map_err(to_js_err)
        .map(|kind| kind.unwrap_or(default))
}

/// Body shared by the two `periodStats` exports; the only difference between
/// them is whether a fiscal calendar is supplied.
fn period_stats_js(
    perf: &Performance,
    ticker_idx: JsValue,
    aggregation_frequency: Option<JsValue>,
    fiscal_config: Option<&JsFiscalConfig>,
) -> Result<JsValue, JsValue> {
    let aggregation_frequency =
        js_opt_string(aggregation_frequency.as_ref(), "aggregationFrequency")?;
    let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
    let pk = parse_frequency(aggregation_frequency.as_deref(), DEFAULT_PERIODIC_FREQUENCY)?;
    let stats = perf
        .period_stats(ticker_idx, pk, fiscal_config.map(|config| config.inner))
        .map_err(to_js_err)?;
    to_js_value_numeric(&stats)
}

fn parse_cagr_day_count(day_count: Option<&str>) -> Result<CagrDayCount, JsValue> {
    day_count
        .map(str::parse::<CagrDayCount>)
        .transpose()
        .map_err(to_js_err)
        .map(Option::unwrap_or_default)
}

fn resolve_optional_calendar(
    calendar_id: Option<&str>,
) -> Result<Option<&'static dyn HolidayCalendar>, JsValue> {
    calendar_id
        .map(calendar_by_id_strict)
        .transpose()
        .map_err(to_js_err)
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

fn dates_to_js_array(dates: &[finstack_quant_core::dates::Date]) -> Array {
    let date_array = Array::new_with_length(dates.len() as u32);
    for (i, &d) in dates.iter().enumerate() {
        date_array.set(i as u32, JsValue::from(&date_to_iso(d)));
    }
    date_array
}

/// Build a plain JS object `{ dates: string[], <numeric_field>: Float64Array, ... }`
/// from a series of (key, JsValue) pairs.
fn obj_from_pairs(pairs: &[(&str, JsValue)]) -> Result<JsValue, JsValue> {
    let obj = js_sys::Object::new();
    for (key, value) in pairs {
        Reflect::set(&obj, &JsValue::from(*key), value)?;
    }
    Ok(obj.into())
}

/// Serialize a rolling [`DatedSeries`] with its Rust field names:
/// `{ values: Float64Array, dates: string[], value_column: string }`, where
/// `value_column` is the Rust `RollingMetric` name of the series.
fn dated_series_to_js(series: &DatedSeries) -> Result<JsValue, JsValue> {
    obj_from_pairs(&[
        ("values", vec_f64_to_js(&series.values)),
        ("dates", dates_to_js_array(&series.dates).into()),
        ("value_column", JsValue::from(series.value_column.as_str())),
    ])
}

fn rolling_greeks_to_js(rg: &RollingGreeks) -> Result<JsValue, JsValue> {
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
    inner: Performance,
}

#[wasm_bindgen(js_class = Performance)]
impl JsPerformance {
    /// Construct from a dated price or return panel (the Rust
    /// `PerformancePanel`).
    ///
    /// # Arguments
    ///
    /// * `panel` - `PerformancePanel` object or JSON text: strictly ascending
    ///   ISO-8601 `dates`, ticker-major `series` (`series[i][t]` is ticker `i`
    ///   at `dates[t]`; a leading or trailing gap is `null`), `kind`
    ///   (`"prices"` for strictly positive prices or `"returns"` for simple
    ///   decimal returns), unique `ticker_names`, an optional
    ///   `benchmark_ticker` (the first ticker when omitted) and an optional
    ///   `frequency` token (daily when omitted).
    ///
    /// # Errors
    ///
    /// Rejects a malformed panel or unknown field (kind `validation`), a
    /// non-finite number (kind `invalid_type`), invalid series or dates
    /// (kind `validation`) and an unknown benchmark ticker (kind `not_found`).
    /// @param panel - `PerformancePanel` object or JSON text: strictly ascending ISO-8601 `dates`, ticker-major `series` (`series[i][t]` is ticker `i` at `dates[t]`; a leading or trailing gap is `null`), `kind` (`"prices"` for strictly positive prices or `"returns"` for simple decimal returns), unique `ticker_names`, an optional `benchmark_ticker` (the first ticker when omitted) and an optional `frequency` token (daily when omitted).
    ///
    /// @example
    /// ```typescript
    /// import init, { analytics } from "finstack-quant-wasm";
    /// await init();
    /// const perf = new analytics.Performance({
    ///   dates: ["2024-01-01", "2024-01-02", "2024-01-03"],
    ///   series: [[100, 101, 103]],
    ///   kind: "prices",
    ///   ticker_names: ["FUND"],
    /// });
    /// console.log(perf.getTickerNames()); // ["FUND"]
    /// perf.free();
    /// ```
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "PerformancePanel | string")] panel: JsValue,
    ) -> Result<JsPerformance, JsValue> {
        let panel: PerformancePanel = from_js_json(&panel, "panel")?;
        let inner = Performance::new(&panel).map_err(to_js_err)?;
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
    pub fn reset_date_range(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string")] start: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] end: JsValue,
    ) -> Result<(), JsValue> {
        let start: &str = &js_string(&start, "start")?;
        let end: &str = &js_string(&end, "end")?;
        self.inner
            .reset_date_range(parse_iso_date(start)?, parse_iso_date(end)?);
        Ok(())
    }

    /// Change the benchmark ticker.
    ///
    /// # Errors
    ///
    /// Rejects a non-string `ticker` (kind `invalid_type`) or one that does not
    /// match a loaded ticker name (kind `not_found`).
    /// @param ticker - Existing ticker label to use as the benchmark return series.
    #[wasm_bindgen(js_name = resetBenchTicker)]
    pub fn reset_bench_ticker(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string")] ticker: JsValue,
    ) -> Result<(), JsValue> {
        let ticker: &str = &js_string(&ticker, "ticker")?;
        self.inner.reset_bench_ticker(ticker).map_err(to_js_err)
    }

    /// Ticker names in column order.
    ///
    /// # Errors
    ///
    /// Rejects if the ticker-name vector cannot be serialized to JavaScript.
    /// @returns Ticker labels in column order as a JavaScript string array.
    #[wasm_bindgen(js_name = getTickerNames, unchecked_return_type = "string[]")]
    pub fn get_ticker_names(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.get_ticker_names())
    }

    /// Resolve an exact ticker name to its zero-based column index.
    ///
    /// # Arguments
    ///
    /// * `ticker` - Case-sensitive ticker label supplied when the panel was constructed.
    ///
    /// # Errors
    ///
    /// Rejects a non-string value (kind `invalid_type`) or a name absent from the
    /// panel (kind `not_found`).
    /// @returns Zero-based column index for the loaded ticker.
    /// @param ticker - Case-sensitive ticker label supplied when the panel was constructed.
    #[wasm_bindgen(js_name = getTickerIndex)]
    pub fn get_ticker_index(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] ticker: JsValue,
    ) -> Result<usize, JsValue> {
        let ticker = js_string(&ticker, "ticker")?;
        self.inner.get_ticker_index(&ticker).map_err(to_js_err)
    }

    /// Benchmark column index.
    /// @returns Zero-based index of the benchmark ticker in `getTickerNames()`.
    #[wasm_bindgen(js_name = getBenchmarkIdx)]
    pub fn get_benchmark_idx(&self) -> usize {
        self.inner.get_benchmark_idx()
    }

    /// Observation frequency token.
    /// @returns Frequency string such as `"daily"` or `"monthly"`.
    #[wasm_bindgen(js_name = getFrequency)]
    pub fn get_frequency(&self) -> String {
        self.inner.get_frequency().to_string()
    }

    /// Full return-aligned date grid as ISO date strings (`"YYYY-MM-DD"`),
    /// independent of any active window — matches Rust `Performance::get_dates`.
    /// @returns Full panel dates as ISO-8601 strings, ignoring any `resetDateRange` window.
    #[wasm_bindgen(js_name = getDates)]
    pub fn get_dates(&self) -> Vec<String> {
        self.inner
            .get_dates()
            .iter()
            .map(|&d| date_to_iso(d))
            .collect()
    }

    /// Date grid of the currently active analysis window as ISO date strings.
    /// Equal to `getDates()` until `resetDateRange` narrows the window.
    /// @returns ISO-8601 dates of the active analysis window, in chronological order.
    #[wasm_bindgen(js_name = getActiveDates)]
    pub fn get_active_dates(&self) -> Vec<String> {
        self.inner
            .get_active_dates()
            .iter()
            .map(|&d| date_to_iso(d))
            .collect()
    }

    /// Date grid for one ticker's active return series as ISO date strings.
    ///
    /// # Errors
    ///
    /// Rejects when `ticker_idx` is outside the loaded ticker columns.
    /// @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @returns ISO-8601 dates for that ticker's active return series, in chronological order.
    #[wasm_bindgen(js_name = getActiveDatesForTicker)]
    pub fn get_active_dates_for_ticker(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
    ) -> Result<Vec<String>, JsValue> {
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        Ok(self
            .inner
            .get_active_dates_for_ticker(ticker_idx)
            .map_err(to_js_err)?
            .iter()
            .map(|&d| date_to_iso(d))
            .collect())
    }

    /// Compound annual growth rate per asset.
    ///
    /// `dayCount` omitted or `"act365_25"` uses Act/365.25 (the Rust
    /// `CagrDayCount` default). Other values are core DayCount names such as
    /// `"act_365f"` or `"bus_252"`. `bus_252` requires `calendarId`. Labels
    /// only: to reuse a `core.DayCount` handle, pass `dayCount.toString()`
    /// (Python additionally accepts the `DayCount` object itself).
    ///
    /// # Errors
    ///
    /// Rejects an unknown day-count (kind `validation`), an unknown calendar
    /// id (kind `not_found`, with suggestions), a missing calendar when
    /// `bus_252` is requested, or a ticker whose active range has no
    /// positive holding period.
    /// @param dayCount - Optional day-count: `"act365_25"` or a core name such as `"act_365f"`; defaults to Act/365.25.
    /// @param calendarId - Optional holiday-calendar id, or `+`-joined ids for a union calendar (`"nyse+gblo"`); required for `bus_252`.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(unchecked_return_type = "Float64Array")]
    pub fn cagr(
        &self,
        #[wasm_bindgen(js_name = "dayCount", unchecked_optional_param_type = "string")]
        day_count: Option<JsValue>,
        #[wasm_bindgen(js_name = "calendarId", unchecked_optional_param_type = "string")]
        calendar_id: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let day_count = js_opt_string(day_count.as_ref(), "dayCount")?;
        let calendar_id = js_opt_string(calendar_id.as_ref(), "calendarId")?;
        let day_count = parse_cagr_day_count(day_count.as_deref())?;
        let calendar = resolve_optional_calendar(calendar_id.as_deref())?;
        result_vec_f64_to_js(self.inner.cagr(day_count, calendar))
    }

    /// Mean periodic return per asset (annualized by default).
    /// @param annualize - Whether to annualize by the configured frequency; defaults to true.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = meanReturn, unchecked_return_type = "Float64Array")]
    pub fn mean_return(
        &self,
        #[wasm_bindgen(unchecked_optional_param_type = "boolean")] annualize: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let annualize = js_opt_bool(annualize.as_ref(), "annualize")?;
        Ok(vec_f64_to_js(
            &self
                .inner
                .mean_return(annualize.unwrap_or(DEFAULT_ANNUALIZE)),
        ))
    }

    /// Return volatility per asset (annualized by default).
    /// @param annualize - Whether to annualize by the configured frequency; defaults to true.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(unchecked_return_type = "Float64Array")]
    pub fn volatility(
        &self,
        #[wasm_bindgen(unchecked_optional_param_type = "boolean")] annualize: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let annualize = js_opt_bool(annualize.as_ref(), "annualize")?;
        Ok(vec_f64_to_js(
            &self
                .inner
                .volatility(annualize.unwrap_or(DEFAULT_ANNUALIZE)),
        ))
    }

    /// Sharpe ratio per asset for the given risk-free rate.
    /// @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(unchecked_return_type = "Float64Array")]
    pub fn sharpe(
        &self,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_optional_param_type = "number")]
        risk_free_rate: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_opt_f64(risk_free_rate.as_ref(), "riskFreeRate")?;
        Ok(vec_f64_to_js(
            &self
                .inner
                .sharpe(risk_free_rate.unwrap_or(DEFAULT_RISK_FREE_RATE)),
        ))
    }

    /// Sortino ratio per asset for the given per-period minimum acceptable return.
    /// @param mar - Per-period minimum acceptable return as a decimal; defaults to 0.0.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(unchecked_return_type = "Float64Array")]
    pub fn sortino(
        &self,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] mar: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let mar = js_opt_f64(mar.as_ref(), "mar")?;
        Ok(vec_f64_to_js(
            &self.inner.sortino(mar.unwrap_or(DEFAULT_MAR)),
        ))
    }

    /// Calmar ratio (CAGR / |max drawdown|) over the active window, not
    /// Young's 36-month CTA definition.
    /// With zero drawdown, nonzero CAGR gives signed infinity and zero
    /// CAGR gives `0`.
    ///
    /// # Errors
    ///
    /// Rejects when any ticker's active range has no positive holding period
    /// and therefore cannot produce CAGR.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(unchecked_return_type = "Float64Array")]
    pub fn calmar(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.calmar())
    }

    /// Mean drawdown per asset.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = meanDrawdown, unchecked_return_type = "Float64Array")]
    pub fn mean_drawdown(&self) -> JsValue {
        vec_f64_to_js(&self.inner.mean_drawdown())
    }

    /// Maximum drawdown per asset.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = maxDrawdown, unchecked_return_type = "Float64Array")]
    pub fn max_drawdown(&self) -> JsValue {
        vec_f64_to_js(&self.inner.max_drawdown())
    }

    /// Historical value-at-risk per asset at the given confidence level.
    /// @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    /// @throws Error - Rejects a `confidence` outside the open interval (0, 1).
    #[wasm_bindgen(js_name = valueAtRisk, unchecked_return_type = "Float64Array")]
    pub fn value_at_risk(
        &self,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] confidence: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let confidence = js_opt_f64(confidence.as_ref(), "confidence")?;
        result_vec_f64_to_js(
            self.inner
                .value_at_risk(confidence.unwrap_or(DEFAULT_CONFIDENCE)),
        )
    }

    /// Expected shortfall per asset: mean of exactly the worst `1-confidence`
    /// empirical probability mass, including a fractional boundary observation.
    /// @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    /// @throws Error - Rejects a `confidence` outside the open interval (0, 1).
    #[wasm_bindgen(js_name = expectedShortfall, unchecked_return_type = "Float64Array")]
    pub fn expected_shortfall(
        &self,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] confidence: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let confidence = js_opt_f64(confidence.as_ref(), "confidence")?;
        result_vec_f64_to_js(
            self.inner
                .expected_shortfall(confidence.unwrap_or(DEFAULT_CONFIDENCE)),
        )
    }

    /// Tracking error versus the benchmark per asset.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = trackingError, unchecked_return_type = "Float64Array")]
    pub fn tracking_error(&self) -> JsValue {
        vec_f64_to_js(&self.inner.tracking_error())
    }

    /// Information ratio versus the benchmark per asset.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = informationRatio, unchecked_return_type = "Float64Array")]
    pub fn information_ratio(&self) -> JsValue {
        vec_f64_to_js(&self.inner.information_ratio())
    }

    /// Return skewness per asset.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(unchecked_return_type = "Float64Array")]
    pub fn skewness(&self) -> JsValue {
        vec_f64_to_js(&self.inner.skewness())
    }

    /// Excess kurtosis of returns per asset.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(unchecked_return_type = "Float64Array")]
    pub fn kurtosis(&self) -> JsValue {
        vec_f64_to_js(&self.inner.kurtosis())
    }

    /// Geometric mean return per asset.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = geometricMean, unchecked_return_type = "Float64Array")]
    pub fn geometric_mean(&self) -> JsValue {
        vec_f64_to_js(&self.inner.geometric_mean())
    }

    /// Downside deviation per asset below the per-period minimum acceptable return.
    /// @param mar - Per-period minimum acceptable return as a decimal; defaults to 0.0.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = downsideDeviation, unchecked_return_type = "Float64Array")]
    pub fn downside_deviation(
        &self,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] mar: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let mar = js_opt_f64(mar.as_ref(), "mar")?;
        Ok(vec_f64_to_js(
            &self.inner.downside_deviation(mar.unwrap_or(DEFAULT_MAR)),
        ))
    }

    /// Longest drawdown duration in calendar days per asset.
    ///
    /// # Errors
    ///
    /// Rejects if the duration vector cannot be serialized to JavaScript.
    /// @returns Per-ticker longest drawdown duration in calendar days, as a JavaScript number array.
    #[wasm_bindgen(js_name = maxDrawdownDuration, unchecked_return_type = "number[]")]
    pub fn max_drawdown_duration(&self) -> Result<JsValue, JsValue> {
        // `i64` day counts do not fit a `Float64Array` contract; they cross as a
        // plain JS number array through the serde path.
        to_js_value(&self.inner.max_drawdown_duration())
    }

    /// Empyrical-style annualized geometric up-capture versus the benchmark per asset.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = upCapture, unchecked_return_type = "Float64Array")]
    pub fn up_capture(&self) -> JsValue {
        vec_f64_to_js(&self.inner.up_capture())
    }

    /// Empyrical-style annualized geometric down-capture versus the benchmark per asset.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = downCapture, unchecked_return_type = "Float64Array")]
    pub fn down_capture(&self) -> JsValue {
        vec_f64_to_js(&self.inner.down_capture())
    }

    /// Empyrical-style annualized geometric up/down capture ratio versus the benchmark per asset.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = captureRatio, unchecked_return_type = "Float64Array")]
    pub fn capture_ratio(&self) -> JsValue {
        vec_f64_to_js(&self.inner.capture_ratio())
    }

    /// Omega ratio per asset for the given threshold return.
    /// @param threshold - Per-period threshold return as a decimal; defaults to 0.0.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = omegaRatio, unchecked_return_type = "Float64Array")]
    pub fn omega_ratio(
        &self,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] threshold: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let threshold = js_opt_f64(threshold.as_ref(), "threshold")?;
        Ok(vec_f64_to_js(
            &self.inner.omega_ratio(threshold.unwrap_or(DEFAULT_MAR)),
        ))
    }

    /// Treynor ratio per asset for the given risk-free rate.
    /// A non-finite risk-free rate gives `NaN`. With absolute beta below
    /// `1e-10`, nonzero finite excess return gives signed infinity and zero
    /// excess return gives `0`.
    /// @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(unchecked_return_type = "Float64Array")]
    pub fn treynor(
        &self,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_optional_param_type = "number")]
        risk_free_rate: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_opt_f64(risk_free_rate.as_ref(), "riskFreeRate")?;
        Ok(vec_f64_to_js(
            &self
                .inner
                .treynor(risk_free_rate.unwrap_or(DEFAULT_RISK_FREE_RATE)),
        ))
    }

    /// Gain-to-pain ratio per asset.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = gainToPain, unchecked_return_type = "Float64Array")]
    pub fn gain_to_pain(&self) -> JsValue {
        vec_f64_to_js(&self.inner.gain_to_pain())
    }

    /// Ulcer index per asset.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = ulcerIndex, unchecked_return_type = "Float64Array")]
    pub fn ulcer_index(&self) -> JsValue {
        vec_f64_to_js(&self.inner.ulcer_index())
    }

    /// Martin ratio (CAGR divided by Ulcer Index) per asset.
    /// With zero Ulcer Index, nonzero CAGR gives signed infinity and zero
    /// CAGR gives `0`.
    ///
    /// # Errors
    ///
    /// Rejects when any ticker's active range has no positive holding period
    /// and therefore cannot produce CAGR.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = martinRatio, unchecked_return_type = "Float64Array")]
    pub fn martin_ratio(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.martin_ratio())
    }

    /// Recovery factor (total return over max drawdown) per asset.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = recoveryFactor, unchecked_return_type = "Float64Array")]
    pub fn recovery_factor(&self) -> JsValue {
        vec_f64_to_js(&self.inner.recovery_factor())
    }

    /// Pain index (mean drawdown magnitude) per asset.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = painIndex, unchecked_return_type = "Float64Array")]
    pub fn pain_index(&self) -> JsValue {
        vec_f64_to_js(&self.inner.pain_index())
    }

    /// Pain ratio ((CAGR - annual risk-free rate) / pain index) per asset.
    /// Non-finite risk-free rates give `NaN`, including when the pain index
    /// is zero.
    ///
    /// # Errors
    ///
    /// Rejects when any ticker's active range has no positive holding period
    /// and therefore cannot produce CAGR.
    /// @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = painRatio, unchecked_return_type = "Float64Array")]
    pub fn pain_ratio(
        &self,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_optional_param_type = "number")]
        risk_free_rate: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_opt_f64(risk_free_rate.as_ref(), "riskFreeRate")?;
        result_vec_f64_to_js(
            self.inner
                .pain_ratio(risk_free_rate.unwrap_or(DEFAULT_RISK_FREE_RATE)),
        )
    }

    /// Tail ratio of upper to lower return quantiles per asset.
    /// @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    /// @throws Error - Rejects a `confidence` outside the open interval (0, 1).
    #[wasm_bindgen(js_name = tailRatio, unchecked_return_type = "Float64Array")]
    pub fn tail_ratio(
        &self,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] confidence: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let confidence = js_opt_f64(confidence.as_ref(), "confidence")?;
        result_vec_f64_to_js(
            self.inner
                .tail_ratio(confidence.unwrap_or(DEFAULT_CONFIDENCE)),
        )
    }

    /// R-squared of returns against the benchmark per asset.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = rSquared, unchecked_return_type = "Float64Array")]
    pub fn r_squared(&self) -> JsValue {
        vec_f64_to_js(&self.inner.r_squared())
    }

    /// Share of periods beating the benchmark per asset.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = battingAverage, unchecked_return_type = "Float64Array")]
    pub fn batting_average(&self) -> JsValue {
        vec_f64_to_js(&self.inner.batting_average())
    }

    /// Equal-weight Gaussian value-at-risk per asset.
    ///
    /// `horizonPeriods` omitted is one-period VaR. A positive `h` scales
    /// mean by `h` and volatility by `√h`.
    /// @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
    /// @param horizonPeriods - Optional horizon in observation periods; omitted is one-period VaR.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    /// @throws Error - Rejects a `confidence` outside the open interval (0, 1).
    #[wasm_bindgen(js_name = parametricVar, unchecked_return_type = "Float64Array")]
    pub fn parametric_var(
        &self,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] confidence: Option<JsValue>,
        #[wasm_bindgen(js_name = "horizonPeriods", unchecked_optional_param_type = "number")]
        horizon_periods: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let confidence = js_opt_f64(confidence.as_ref(), "confidence")?;
        let horizon_periods = js_opt_f64(horizon_periods.as_ref(), "horizonPeriods")?;
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
    /// @param horizonPeriods - Optional horizon in observation periods; omitted is one-period VaR.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    /// @throws Error - Rejects a `confidence` outside the open interval (0, 1).
    #[wasm_bindgen(js_name = cornishFisherVar, unchecked_return_type = "Float64Array")]
    pub fn cornish_fisher_var(
        &self,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] confidence: Option<JsValue>,
        #[wasm_bindgen(js_name = "horizonPeriods", unchecked_optional_param_type = "number")]
        horizon_periods: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let confidence = js_opt_f64(confidence.as_ref(), "confidence")?;
        let horizon_periods = js_opt_f64(horizon_periods.as_ref(), "horizonPeriods")?;
        result_vec_f64_to_js(
            self.inner
                .cornish_fisher_var(confidence.unwrap_or(DEFAULT_CONFIDENCE), horizon_periods),
        )
    }

    /// Conditional drawdown-at-risk per asset: mean of exactly the worst
    /// `1-confidence` drawdown mass, including fractional boundary weighting.
    /// @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    /// @throws Error - Rejects a `confidence` outside the open interval (0, 1).
    #[wasm_bindgen(unchecked_return_type = "Float64Array")]
    pub fn cdar(
        &self,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] confidence: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let confidence = js_opt_f64(confidence.as_ref(), "confidence")?;
        result_vec_f64_to_js(self.inner.cdar(confidence.unwrap_or(DEFAULT_CONFIDENCE)))
    }

    /// Linearly annualized M-squared per asset. Cash subtraction and addition
    /// both use the decompounded period cash rate multiplied by periods per year.
    /// @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = mSquared, unchecked_return_type = "Float64Array")]
    pub fn m_squared(
        &self,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_optional_param_type = "number")]
        risk_free_rate: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_opt_f64(risk_free_rate.as_ref(), "riskFreeRate")?;
        Ok(vec_f64_to_js(&self.inner.m_squared(
            risk_free_rate.unwrap_or(DEFAULT_RISK_FREE_RATE),
        )))
    }

    /// Modified Sharpe ratio using annualized excess return and
    /// corresponding-annual-horizon Cornish-Fisher VaR per asset.
    ///
    /// The panel frequency supplies the periods-per-year scaling for both
    /// terms, including the horizon decay of skewness and excess kurtosis;
    /// the denominator is not one-period VaR.
    /// @param riskFreeRate - Annualized decimal risk-free rate, decompounded
    /// to the panel frequency before constructing annualized excess return;
    /// defaults to 0.0.
    /// @param confidence - Annual-horizon tail confidence as a decimal
    /// probability; defaults to 0.95.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    /// @throws Error - Rejects a `confidence` outside the open interval (0, 1).
    #[wasm_bindgen(js_name = modifiedSharpe, unchecked_return_type = "Float64Array")]
    pub fn modified_sharpe(
        &self,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_optional_param_type = "number")]
        risk_free_rate: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] confidence: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_opt_f64(risk_free_rate.as_ref(), "riskFreeRate")?;
        let confidence = js_opt_f64(confidence.as_ref(), "confidence")?;
        result_vec_f64_to_js(self.inner.modified_sharpe(
            risk_free_rate.unwrap_or(DEFAULT_RISK_FREE_RATE),
            confidence.unwrap_or(DEFAULT_CONFIDENCE),
        ))
    }

    /// Sterling ratio over the `n` largest drawdowns per asset.
    /// Non-finite risk-free rates give `NaN`, including when no drawdowns
    /// are observed.
    ///
    /// # Errors
    ///
    /// Rejects when any ticker's active range has no positive holding period
    /// and therefore cannot produce CAGR.
    /// @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @param n - Finite non-negative integer count of largest drawdowns; defaults to 5. Invalid numeric values are rejected.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = sterlingRatio, unchecked_return_type = "Float64Array")]
    pub fn sterling_ratio(
        &self,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_optional_param_type = "number")]
        risk_free_rate: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] n: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_opt_f64(risk_free_rate.as_ref(), "riskFreeRate")?;
        let n = js_opt_uint(n.as_ref(), "n")?;
        result_vec_f64_to_js(self.inner.sterling_ratio(
            risk_free_rate.unwrap_or(DEFAULT_RISK_FREE_RATE),
            n.unwrap_or(DEFAULT_DRAWDOWN_COUNT),
        ))
    }

    /// Burke ratio over the `n` largest drawdowns per asset.
    /// Non-finite risk-free rates give `NaN`, including when no drawdowns
    /// are observed.
    ///
    /// # Errors
    ///
    /// Rejects when any ticker's active range has no positive holding period
    /// and therefore cannot produce CAGR.
    /// @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @param n - Finite non-negative integer count of largest drawdowns; defaults to 5. Invalid numeric values are rejected.
    /// @returns Per-ticker values as a Float64Array in `getTickerNames()` order.
    #[wasm_bindgen(js_name = burkeRatio, unchecked_return_type = "Float64Array")]
    pub fn burke_ratio(
        &self,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_optional_param_type = "number")]
        risk_free_rate: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] n: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_opt_f64(risk_free_rate.as_ref(), "riskFreeRate")?;
        let n = js_opt_uint(n.as_ref(), "n")?;
        result_vec_f64_to_js(self.inner.burke_ratio(
            risk_free_rate.unwrap_or(DEFAULT_RISK_FREE_RATE),
            n.unwrap_or(DEFAULT_DRAWDOWN_COUNT),
        ))
    }

    /// Per-period simple return series per asset, as decimal fractions.
    ///
    /// Canonical accessor for the raw return panel over the active window;
    /// prefer it over `excessReturns` with an all-zero risk-free series or
    /// un-compounding `cumulativeReturns`. Series are span-aware and therefore
    /// ragged across assets on edge-ragged panels.
    /// @returns One Float64Array of simple decimal returns per ticker in `getTickerNames()` order.
    #[wasm_bindgen(js_name = getReturns, unchecked_return_type = "Float64Array[]")]
    pub fn get_returns(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.get_returns())
    }

    /// Per-period simple return series for one asset, as decimal fractions.
    ///
    /// # Errors
    ///
    /// Rejects when `ticker_idx` is outside the loaded ticker columns.
    /// @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @returns Simple decimal returns for the selected ticker, in date order.
    #[wasm_bindgen(js_name = getReturnsForTicker, unchecked_return_type = "Float64Array")]
    pub fn get_returns_for_ticker(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
    ) -> Result<JsValue, JsValue> {
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        let series = self
            .inner
            .get_returns_for_ticker(ticker_idx)
            .map_err(to_js_err)?;
        Ok(vec_f64_to_js(&series))
    }

    /// Cumulative return series per asset.
    /// @returns One Float64Array per ticker in `getTickerNames()` order.
    #[wasm_bindgen(js_name = cumulativeReturns, unchecked_return_type = "Float64Array[]")]
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
    /// `Performance.to_dataframe`.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the panel is too short to annualize
    /// (`cagr` / `calmar`) or the table cannot be converted.
    /// @param riskFreeRate - Annualized risk-free rate as a decimal (0.02 = 2%); affects only `sharpe`. Defaults to 0.0.
    /// @param confidence - Tail confidence as a decimal probability applied to VaR, ES and tail ratio; defaults to 0.95.
    /// @returns Column-oriented table envelope of the summary metrics.
    #[wasm_bindgen(js_name = summary, unchecked_return_type = "TableEnvelope")]
    pub fn summary(
        &self,
        #[wasm_bindgen(
            js_name = "riskFreeRate",
            unchecked_optional_param_type = "number | null"
        )]
        risk_free_rate: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "number | null")] confidence: Option<
            JsValue,
        >,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_opt_f64(risk_free_rate.as_ref(), "riskFreeRate")?;
        let confidence = js_opt_f64(confidence.as_ref(), "confidence")?;
        let table = self
            .inner
            .summary(
                risk_free_rate.unwrap_or(DEFAULT_RISK_FREE_RATE),
                confidence.unwrap_or(DEFAULT_CONFIDENCE),
            )
            .map_err(to_js_err)?;
        to_js_value(&table)
    }

    /// Calendar-bucketed compounded returns per ticker.
    ///
    /// The outer array is ticker-major in `getTickerNames()` order. Each inner
    /// array contains chronological `{ date, value }` points, where `date` is
    /// the bucket's ISO-8601 period-end date and `value` is a simple decimal
    /// return (`0.01` means 1%). Chaining one ticker's values reconciles with
    /// its final `cumulativeReturns()` value.
    ///
    /// # Arguments
    ///
    /// * `frequency` - Optional calendar frequency token: `"daily"`,
    ///   `"weekly"`, `"monthly"`, `"quarterly"`, `"semi_annual"`, or
    ///   `"annual"`; defaults to `"monthly"`.
    ///
    /// # Errors
    ///
    /// Rejects an unsupported frequency, an unrepresentable period boundary,
    /// or a panel that cannot be serialized to JavaScript.
    /// @returns Ticker-major nested arrays of chronological Rust `PeriodicReturn` `{ date, value }` points with simple decimal returns.
    /// @param frequency - Optional calendar frequency token: `"daily"`, `"weekly"`, `"monthly"`, `"quarterly"`, `"semi_annual"`, or `"annual"`; defaults to `"monthly"`.
    ///
    /// @example
    /// ```typescript
    /// import init, { analytics } from "finstack-quant-wasm";
    /// await init();
    /// const perf = new analytics.Performance({
    ///   dates: ["2024-01-01", "2024-01-02"],
    ///   series: [[0.01, 0.02]],
    ///   kind: "returns",
    ///   ticker_names: ["FUND"],
    /// });
    /// const [[point]] = perf.periodicReturns();
    /// console.log(point.date, point.value); // "2024-01-02", 0.0302
    /// ```
    #[wasm_bindgen(js_name = periodicReturns, unchecked_return_type = "PeriodicReturn[][]")]
    pub fn periodic_returns(
        &self,
        #[wasm_bindgen(unchecked_optional_param_type = "string")] frequency: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let frequency = js_opt_string(frequency.as_ref(), "frequency")?;
        let kind = parse_frequency(frequency.as_deref(), DEFAULT_PERIODIC_FREQUENCY)?;
        to_js_value(&self.inner.periodic_returns(kind).map_err(to_js_err)?)
    }

    /// Drawdown series per asset.
    /// @returns One Float64Array per ticker in `getTickerNames()` order.
    #[wasm_bindgen(js_name = getDrawdownSeries, unchecked_return_type = "Float64Array[]")]
    pub fn get_drawdown_series(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.get_drawdown_series())
    }

    /// Return the correlation matrix across assets and whether it was repaired.
    ///
    /// Uses the complete-case common window when every ticker has at least
    /// two overlapping points; otherwise pairwise intersecting spans. A raw
    /// estimate that is not a valid correlation matrix (ragged panels can
    /// yield one that is not positive semi-definite) is Higham-repaired to
    /// the nearest valid correlation matrix.
    ///
    /// # Errors
    ///
    /// Rejects a degenerate pair or a matrix that cannot be repaired to a
    /// valid correlation matrix.
    /// @returns `{ matrix, repaired }`: the square correlation matrix as nested Float64Array rows in `getTickerNames()` order, with a diagonal of exactly 1 and every coefficient inside [-1, 1], and `true` when Higham repair was applied.
    #[wasm_bindgen(js_name = correlationMatrix, unchecked_return_type = "{ matrix: Float64Array[]; repaired: boolean }")]
    pub fn correlation_matrix(&self) -> Result<JsValue, JsValue> {
        let (matrix, repaired) = self.inner.correlation_matrix().map_err(to_js_err)?;
        obj_from_pairs(&[
            ("matrix", matrix_f64_to_js(&matrix)),
            ("repaired", JsValue::from(repaired)),
        ])
    }

    /// Cumulative outperformance versus the benchmark per asset.
    /// @returns One Float64Array per ticker in `getTickerNames()` order.
    #[wasm_bindgen(js_name = cumulativeReturnsOutperformance, unchecked_return_type = "Float64Array[]")]
    pub fn cumulative_returns_outperformance(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.cumulative_returns_outperformance())
    }

    /// Difference between asset and benchmark drawdown series.
    /// @returns One Float64Array per ticker in `getTickerNames()` order.
    #[wasm_bindgen(js_name = drawdownDifference, unchecked_return_type = "Float64Array[]")]
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
    /// @returns One Float64Array per ticker in `getTickerNames()` order.
    #[wasm_bindgen(js_name = excessReturns, unchecked_return_type = "Float64Array[]")]
    pub fn excess_returns(
        &self,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] rf: JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] nperiods: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let nperiods = js_opt_f64(nperiods.as_ref(), "nperiods")?;
        let rf = js_f64_seq(&rf, "rf")?;
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
    /// @returns Per-ticker `{ beta, std_err, ci_lower, ci_upper, n_obs, confidence_level }` objects in `getTickerNames()` order; `n_obs` is the paired-observation count and `confidence_level` the decimal level (0.95) of the bounds.
    #[wasm_bindgen(unchecked_return_type = "BetaResult[]")]
    pub fn beta(&self) -> Result<JsValue, JsValue> {
        to_js_rows_numeric(&self.inner.beta())
    }

    /// Benchmark regression annualized Jensen alpha/beta statistics per asset.
    ///
    /// # Errors
    ///
    /// Rejects if the regression results cannot be serialized to JavaScript.
    /// @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @returns Per-ticker `{ alpha, beta, r_squared, adjusted_r_squared, n_obs }` objects in `getTickerNames()` order; `n_obs` is the paired-observation count.
    #[wasm_bindgen(unchecked_return_type = "GreeksResult[]")]
    pub fn greeks(
        &self,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_optional_param_type = "number")]
        risk_free_rate: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_opt_f64(risk_free_rate.as_ref(), "riskFreeRate")?;
        to_js_rows_numeric(
            &self
                .inner
                .greeks(risk_free_rate.unwrap_or(DEFAULT_RISK_FREE_RATE)),
        )
    }

    /// Rolling benchmark annualized Jensen alpha/beta for one asset over a window.
    ///
    /// # Errors
    ///
    /// Rejects when `ticker_idx` is outside the loaded ticker columns or the
    /// JavaScript result object's properties cannot be created.
    /// @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @param window - Finite positive integer observation count; defaults to 63 periods. Invalid numeric values are rejected.
    /// @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @returns `{ dates, alphas, betas }` series for the selected ticker.
    #[wasm_bindgen(js_name = rollingGreeks, unchecked_return_type = "RollingGreeks")]
    pub fn rolling_greeks(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] window: Option<JsValue>,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_optional_param_type = "number")]
        risk_free_rate: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_opt_f64(risk_free_rate.as_ref(), "riskFreeRate")?;
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        let window = js_opt_uint(window.as_ref(), "window")?;
        let rg = self
            .inner
            .rolling_greeks(
                ticker_idx,
                window.unwrap_or(DEFAULT_ROLLING_WINDOW),
                risk_free_rate.unwrap_or(DEFAULT_RISK_FREE_RATE),
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
    /// @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @param window - Finite positive integer observation count; defaults to 63 periods. Invalid numeric values are rejected.
    /// @returns `{ values, dates, value_column }` series for the selected ticker; `value_column` is `"volatility"`.
    #[wasm_bindgen(js_name = rollingVolatility, unchecked_return_type = "DatedSeries")]
    pub fn rolling_volatility(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] window: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        let window = js_opt_uint(window.as_ref(), "window")?;
        let series = self
            .inner
            .rolling_volatility(ticker_idx, window.unwrap_or(DEFAULT_ROLLING_WINDOW))
            .map_err(to_js_err)?;
        dated_series_to_js(&series)
    }

    /// Rolling Sortino ratio series for one asset over a window.
    ///
    /// # Errors
    ///
    /// Rejects when `ticker_idx` is outside the loaded ticker columns or the
    /// JavaScript result object's properties cannot be created.
    /// @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @param window - Finite positive integer observation count; defaults to 63 periods. Invalid numeric values are rejected.
    /// @param mar - Per-period minimum acceptable return as a decimal; defaults to 0.0.
    /// @returns `{ values, dates, value_column }` series for the selected ticker; `value_column` is `"sortino"`.
    #[wasm_bindgen(js_name = rollingSortino, unchecked_return_type = "DatedSeries")]
    pub fn rolling_sortino(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] window: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] mar: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let mar = js_opt_f64(mar.as_ref(), "mar")?;
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        let window = js_opt_uint(window.as_ref(), "window")?;
        let series = self
            .inner
            .rolling_sortino(
                ticker_idx,
                window.unwrap_or(DEFAULT_ROLLING_WINDOW),
                mar.unwrap_or(DEFAULT_MAR),
            )
            .map_err(to_js_err)?;
        dated_series_to_js(&series)
    }

    /// Rolling Sharpe ratio series for one asset over a window.
    ///
    /// # Errors
    ///
    /// Rejects when `ticker_idx` is outside the loaded ticker columns or the
    /// JavaScript result object's properties cannot be created.
    /// @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @param window - Finite positive integer observation count; defaults to 63 periods. Invalid numeric values are rejected.
    /// @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
    /// @returns `{ values, dates, value_column }` series for the selected ticker; `value_column` is `"sharpe"`.
    #[wasm_bindgen(js_name = rollingSharpe, unchecked_return_type = "DatedSeries")]
    pub fn rolling_sharpe(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] window: Option<JsValue>,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_optional_param_type = "number")]
        risk_free_rate: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_opt_f64(risk_free_rate.as_ref(), "riskFreeRate")?;
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        let window = js_opt_uint(window.as_ref(), "window")?;
        let series = self
            .inner
            .rolling_sharpe(
                ticker_idx,
                window.unwrap_or(DEFAULT_ROLLING_WINDOW),
                risk_free_rate.unwrap_or(DEFAULT_RISK_FREE_RATE),
            )
            .map_err(to_js_err)?;
        dated_series_to_js(&series)
    }

    /// Rolling compounded return series for one asset over a window.
    ///
    /// # Errors
    ///
    /// Rejects when `ticker_idx` is outside the loaded ticker columns or the
    /// JavaScript result object's properties cannot be created. An
    /// overlong `window` returns an empty series; a zero window is rejected.
    /// @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @param window - Finite positive integer observation count. Zero, fractional, non-finite, and out-of-range values are rejected.
    /// @returns `{ values, dates, value_column }` series for the selected ticker; `value_column` is `"return"`.
    #[wasm_bindgen(js_name = rollingReturns, unchecked_return_type = "DatedSeries")]
    pub fn rolling_returns(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] window: JsValue,
    ) -> Result<JsValue, JsValue> {
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        let window = js_uint(&window, "window")?;
        let series = self
            .inner
            .rolling_returns(ticker_idx, window)
            .map_err(to_js_err)?;
        dated_series_to_js(&series)
    }

    /// Details of the `n` largest drawdown episodes for one asset.
    ///
    /// # Errors
    ///
    /// Rejects when `ticker_idx` is outside the loaded ticker columns or the
    /// drawdown details cannot be serialized to JavaScript.
    /// @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @param n - Finite non-negative integer count of episodes; defaults to 5. Invalid numeric values are rejected.
    /// @returns Drawdown episode objects for the selected ticker, largest first.
    #[wasm_bindgen(js_name = drawdownDetails, unchecked_return_type = "DrawdownEpisode[]")]
    pub fn drawdown_details(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "number")] n: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        let n = js_opt_uint(n.as_ref(), "n")?;
        to_js_value(
            &self
                .inner
                .drawdown_details(ticker_idx, n.unwrap_or(DEFAULT_DRAWDOWN_COUNT))
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
    /// `returnKind`, a non-zero `riskFreeRate` with `"excess"`, an out-of-range `ticker_idx`, no factors, too few
    /// observations, non-finite or length-mismatched inputs, a singular
    /// factor design, a fitted coefficient, annualized intercept, or residual
    /// volatility that cannot be represented as a finite value, or a result
    /// that cannot be serialized to JavaScript. Constant responses retain
    /// undefined `NaN` R-squared statistics.
    /// @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @param factorReturns - Matrix of aligned already-excess decimal factor-return series, one row per factor.
    /// @param returnKind - `"excess"` or `"total"`; defaults to `"excess"`.
    /// @param riskFreeRate - Annualized decimal risk-free rate, used only when `returnKind` is `"total"`; defaults to 0.0 and must be 0.0 for `"excess"`.
    /// @returns `{ alpha, betas, r_squared, adjusted_r_squared, residual_vol, n_obs }` for the selected ticker; `n_obs` is the observation count.
    #[wasm_bindgen(js_name = multiFactorGreeks, unchecked_return_type = "MultiFactorResult")]
    pub fn multi_factor_greeks(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
        #[wasm_bindgen(js_name = "factorReturns", unchecked_param_type = "NumericMatrix")]
        factor_returns: JsValue,
        #[wasm_bindgen(js_name = "returnKind", unchecked_optional_param_type = "string")]
        return_kind: Option<JsValue>,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_optional_param_type = "number")]
        risk_free_rate: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_opt_f64(risk_free_rate.as_ref(), "riskFreeRate")?;
        let return_kind = js_opt_string(return_kind.as_ref(), "returnKind")?;
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        let factors = js_f64_matrix(&factor_returns, "factorReturns")?;
        let refs: Vec<&[f64]> = factors.iter().map(|v| v.as_slice()).collect();
        let kind = ReturnKind::from_label(
            return_kind.as_deref(),
            risk_free_rate.unwrap_or(DEFAULT_RISK_FREE_RATE),
        )
        .map_err(to_js_err)?;
        let result = self
            .inner
            .multi_factor_greeks(ticker_idx, &refs, kind)
            .map_err(to_js_err)?;
        to_js_value_numeric(&result)
    }

    /// Standard lookback-window returns (MTD, QTD, YTD, ...) per asset.
    ///
    /// MTD, QTD and YTD start on the first calendar day of `ref_date`'s month,
    /// calendar quarter and year. FYTD is the first observation on or after the
    /// fiscal calendar start of `fiscalConfig` through `ref_date`; the calendar
    /// year applies when it is omitted. Holidays are not skipped. The first
    /// included simple return still spans the prior close. The serialized result
    /// always contains `{ mtd, qtd, ytd, fytd }`, with `fytd` as a numeric array.
    ///
    /// # Arguments
    ///
    /// * `ref_date` - ISO-8601 date on which MTD, QTD, YTD, and FYTD windows end.
    ///   Omitted or `null` is the last active date; the result's `ref_date` states
    ///   the date used.
    /// * `fiscal_config` - Optional `FiscalConfig` handle for FYTD. Omitted or
    ///   `null` is the calendar year (`FiscalConfig::default`).
    ///
    /// # Errors
    ///
    /// Rejects an invalid ISO `ref_date`, an omitted one while no dates are active,
    /// or a result that cannot be serialized to JavaScript.
    /// @returns The window end `ref_date` and per-ticker `{ mtd, qtd, ytd, fytd }` numeric arrays of lookback returns as decimal fractions; `fytd` is never null.
    /// @param refDate - ISO-8601 date on which MTD, QTD, YTD, and FYTD windows end. Omitted or `null` is the last active date; the result's `ref_date` states the date used.
    #[wasm_bindgen(js_name = lookbackReturns, unchecked_return_type = "LookbackReturns")]
    pub fn lookback_returns(
        &self,
        #[wasm_bindgen(js_name = "refDate", unchecked_optional_param_type = "string | null")]
        ref_date: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let ref_date = crate::utils::wire::js_opt_date(ref_date.as_ref(), "refDate")?;
        to_js_value(
            &self
                .inner
                .lookback_returns(ref_date, None)
                .map_err(to_js_err)?,
        )
    }

    /// Lookback returns for an explicit fiscal calendar.
    ///
    /// The published `lookbackReturns(refDate, fiscalConfig)` calls this export
    /// when a handle is supplied.
    ///
    /// # Errors
    ///
    /// Rejects an invalid ISO `ref_date`, an omitted one while no dates are active,
    /// or a result that cannot be serialized to JavaScript.
    /// @param ref_date - ISO-8601 date on which MTD, QTD, YTD, and FYTD windows end; omitted or `null` is the last active date.
    /// @param fiscalConfig - FiscalConfig handle whose start sets the FYTD window.
    /// @returns The window end `ref_date` and per-ticker `{ mtd, qtd, ytd, fytd }` numeric arrays of lookback returns as decimal fractions.
    #[wasm_bindgen(
        skip_typescript,
        js_name = lookbackReturnsWithFiscalConfig,
        unchecked_return_type = "LookbackReturns"
    )]
    pub fn lookback_returns_with_fiscal_config(
        &self,
        #[wasm_bindgen(
            js_name = "refDate",
            unchecked_param_type = "string | null | undefined"
        )]
        ref_date: Option<JsValue>,
        #[wasm_bindgen(js_name = "fiscalConfig")] fiscal_config: &JsFiscalConfig,
    ) -> Result<JsValue, JsValue> {
        let ref_date = crate::utils::wire::js_opt_date(ref_date.as_ref(), "refDate")?;
        to_js_value(
            &self
                .inner
                .lookback_returns(ref_date, Some(fiscal_config.inner))
                .map_err(to_js_err)?,
        )
    }

    /// Aggregated period statistics for one asset at the given frequency.
    /// Without a fiscal calendar, Gregorian periods are bucketed with ISO weeks.
    ///
    /// # Errors
    ///
    /// Rejects an unsupported `aggregation_frequency`, an out-of-range
    /// `ticker_idx`, an unrepresentable period boundary, or period statistics
    /// that cannot be serialized to JavaScript.
    /// @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @param aggregationFrequency - Optional aggregation frequency token; defaults to monthly.
    /// @param fiscalConfig - Optional FiscalConfig handle; omitted or null buckets Gregorian periods with ISO weeks.
    /// @returns Period statistics object for the selected ticker at the requested frequency.
    #[wasm_bindgen(js_name = periodStats, unchecked_return_type = "PeriodStats")]
    pub fn period_stats(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
        #[wasm_bindgen(
            js_name = "aggregationFrequency",
            unchecked_optional_param_type = "string"
        )]
        aggregation_frequency: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        period_stats_js(&self.inner, ticker_idx, aggregation_frequency, None)
    }

    /// Period statistics for an explicit fiscal calendar. A supplied fiscal
    /// calendar applies fiscal day, week, month, quarter, half-year and year
    /// boundaries, including clamped month ends.
    ///
    /// The published `periodStats(tickerIdx, aggregationFrequency, fiscalConfig)`
    /// calls this export when a handle is supplied.
    ///
    /// # Errors
    ///
    /// Rejects an unsupported `aggregation_frequency`, an out-of-range
    /// `ticker_idx`, an unrepresentable period boundary, or period statistics
    /// that cannot be serialized to JavaScript.
    /// @param ticker_idx - Finite non-negative integer column index in tickerNames order.
    /// @param aggregation_frequency - Optional aggregation frequency token; defaults to monthly.
    /// @param fiscalConfig - FiscalConfig handle whose start sets the period boundaries.
    /// @returns Period statistics object for the selected ticker at the requested frequency.
    #[wasm_bindgen(
        skip_typescript,
        js_name = periodStatsWithFiscalConfig,
        unchecked_return_type = "PeriodStats"
    )]
    pub fn period_stats_with_fiscal_config(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
        #[wasm_bindgen(
            js_name = "aggregationFrequency",
            unchecked_param_type = "string | null | undefined"
        )]
        aggregation_frequency: Option<JsValue>,
        #[wasm_bindgen(js_name = "fiscalConfig")] fiscal_config: &JsFiscalConfig,
    ) -> Result<JsValue, JsValue> {
        period_stats_js(
            &self.inner,
            ticker_idx,
            aggregation_frequency,
            Some(fiscal_config),
        )
    }

    /// Serialize the full panel state to compact JSON.
    ///
    /// The JSON holds the dates, return panel, ticker names, benchmark index,
    /// frequency and active date window, so `Performance.fromJson` rebuilds an
    /// equivalent handle. Twin of Python `Performance.to_json`.
    /// # Errors
    ///
    /// Rejects a panel that cannot be serialized.
    /// @returns Compact JSON string of the Rust `Performance` panel.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Rebuild a panel from JSON produced by `toJson`.
    ///
    /// Twin of Python `Performance.from_json`.
    /// # Errors
    ///
    /// Rejects JSON that is malformed or does not describe a valid
    /// `Performance` panel.
    /// @param json - `Performance` JSON string produced by `toJson`, or the equivalent parsed object.
    /// @returns A `Performance` handle equivalent to the serialized one.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(
        #[wasm_bindgen(unchecked_param_type = "string | Record<string, unknown>")] json: JsValue,
    ) -> Result<JsPerformance, JsValue> {
        let json: &str = &json_text(&json, "json")?;
        let inner: Performance = serde_json::from_str(json).map_err(to_js_err)?;
        Ok(JsPerformance { inner })
    }
}
