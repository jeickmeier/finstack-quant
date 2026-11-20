//! WASM performance analytics on an actual valuation grid.

use crate::api::core::dates::js_opt_day_count_context;
use crate::api::core::periods::js_opt_fiscal_config;
use crate::utils::input::{
    from_js_json, js_bool, js_f64, js_f64_matrix, js_f64_seq, js_opt_uint, js_string, js_uint,
};
use crate::utils::wire::{js_date, js_opt_date, js_opt_parse, js_parse};
use crate::utils::{
    date_to_iso, to_js_err, to_js_rows_numeric, to_js_table_numeric, to_js_value,
    to_js_value_numeric,
};
use finstack_quant_analytics::{
    CagrDayCount, DatedSeries, Performance, PerformancePanel, RollingGreeks,
};
use finstack_quant_core::dates::PeriodKind;
use js_sys::{Array, Float64Array, Reflect};
use wasm_bindgen::prelude::*;

fn dates_to_iso(dates: &[finstack_quant_core::dates::Date]) -> Vec<String> {
    dates.iter().map(|&d| date_to_iso(d)).collect()
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

fn lookback_to_js(result: &finstack_quant_analytics::LookbackReturns) -> Result<JsValue, JsValue> {
    let js = to_js_value(result)?;
    for (field, values) in [
        ("mtd", &result.mtd),
        ("qtd", &result.qtd),
        ("ytd", &result.ytd),
        ("fytd", &result.fytd),
    ] {
        let array = Array::new_with_length(values.len() as u32);
        for (row, value) in values.iter().enumerate() {
            array.set(row as u32, JsValue::from_f64(*value));
        }
        Reflect::set(&js, &JsValue::from(field), &array)?;
    }
    Ok(js)
}

/// Stateful analytics on ticker-major prices or simple investment returns.
///
/// Panels supply actual ascending valuation dates. A return panel has one
/// fewer value per ticker than valuation dates; prices have one per date.
/// Frequency and benchmark are optional metadata and required only by methods
/// that use them. Rates and returns are decimal fractions. Unavailable numeric
/// outputs are NaN and genuine zero-risk ratios can be signed infinities.
/// @example
/// ```typescript
/// import init, { analytics } from "finstack-quant-wasm";
/// await init();
/// const p = new analytics.Performance({ dates: ["2025-01-01", "2025-01-02"], series: [[0.01]], kind: "returns", ticker_names: ["A"] });
/// console.log(p.meanReturn(false)[0]); // 0.01
/// p.free();
/// ```
#[wasm_bindgen(js_name = Performance)]
pub struct JsPerformance {
    inner: Performance,
}

#[wasm_bindgen(js_class = Performance)]
impl JsPerformance {
    /// Construct from a validated dated price or return panel.
    /// @param panel - Actual valuation grid, ticker-major series, series kind, unique ticker labels, optional explicit frequency and benchmark. Edge missing values are null; terminal -1 returns or actual final zero prices are allowed.
    /// @returns New Performance handle.
    /// @throws Error - Rejects malformed dates/kind/shape, duplicate or blank identities, unknown fields, interior missingness, infinity, nonterminal loss events, invalid frequency labels, unknown explicit benchmarks, or unrepresentable price-to-return conversion.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "PerformancePanel | string")] panel: JsValue,
    ) -> Result<JsPerformance, JsValue> {
        let panel: PerformancePanel = from_js_json(&panel, "panel")?;
        let inner = Performance::new(&panel).map_err(to_js_err)?;
        Ok(Self { inner })
    }

    /// Atomically select inclusive return-ending date bounds on the original grid.
    /// @param start - Inclusive ISO-8601 return-ending lower bound; may be outside support.
    /// @param end - Inclusive ISO-8601 upper bound, at or after start; reversed requests leave state unchanged.
    /// @throws Error - Rejects non-string or invalid ISO dates, reversed bounds, or an unrepresentable rebased cache; failure leaves the previous active selection unchanged.
    #[wasm_bindgen(js_name = resetDateRange)]
    pub fn reset_date_range(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string")] start: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] end: JsValue,
    ) -> Result<(), JsValue> {
        let start = js_date(&start, "start")?;
        let end = js_date(&end, "end")?;
        self.inner.reset_date_range(start, end).map_err(to_js_err)
    }

    /// Select an existing exact ticker label as benchmark.
    /// @param ticker - Loaded case-sensitive ticker label; deliberate self-benchmarking is allowed.
    /// @throws Error - Rejects non-string values and ticker names not loaded in the panel; failure preserves the prior benchmark.
    #[wasm_bindgen(js_name = resetBenchTicker)]
    pub fn reset_bench_ticker(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string")] ticker: JsValue,
    ) -> Result<(), JsValue> {
        self.inner
            .reset_bench_ticker(&js_string(&ticker, "ticker")?)
            .map_err(to_js_err)
    }

    /// Inspect ticker identities in supplied order.
    /// @returns Ticker labels in column order.
    /// @throws Error - Rejects serialization failure while constructing the JavaScript ticker array.
    #[wasm_bindgen(js_name = getTickerNames, unchecked_return_type = "string[]")]
    pub fn get_ticker_names(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.get_ticker_names())
    }

    /// Resolve an exact ticker identity.
    /// @param ticker - Loaded case-sensitive ticker label.
    /// @returns Zero-based loaded ticker index.
    /// @throws Error - Rejects a non-string name or an exact ticker label absent from the loaded panel.
    #[wasm_bindgen(js_name = getTickerIndex)]
    pub fn get_ticker_index(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] ticker: JsValue,
    ) -> Result<usize, JsValue> {
        self.inner
            .get_ticker_index(&js_string(&ticker, "ticker")?)
            .map_err(to_js_err)
    }

    /// Inspect optional explicit benchmark selection.
    /// @returns Selected zero-based benchmark index, or undefined when absent.
    #[wasm_bindgen(js_name = getBenchmarkIdx)]
    pub fn get_benchmark_idx(&self) -> Option<usize> {
        self.inner.get_benchmark_idx()
    }

    /// Inspect optional declared observation frequency.
    /// @returns Declared frequency label, or undefined when absent; no inference from dates.
    #[wasm_bindgen(js_name = getFrequency)]
    pub fn get_frequency(&self) -> Option<String> {
        self.inner.get_frequency().map(|kind| kind.to_string())
    }

    /// Full return-ending grid, excluding the initial valuation.
    /// @returns Ascending ISO-8601 return-ending dates.
    #[wasm_bindgen(js_name = getDates)]
    pub fn get_dates(&self) -> Vec<String> {
        dates_to_iso(self.inner.get_dates())
    }

    /// Currently selected return-ending grid.
    /// @returns Ascending ISO-8601 return-ending dates.
    #[wasm_bindgen(js_name = getActiveDates)]
    pub fn get_active_dates(&self) -> Vec<String> {
        dates_to_iso(self.inner.get_active_dates())
    }

    /// Inspect one ticker's active return-ending dates.
    /// @param tickerIdx - Zero-based ticker column index; must be a finite non-negative integer in range.
    /// @returns Ascending ISO-8601 dates aligned to the corresponding compact return series.
    /// @throws Error - Rejects a fractional, negative, nonfinite or out-of-range ticker index.
    #[wasm_bindgen(js_name = getActiveDatesForTicker)]
    pub fn get_active_dates_for_ticker(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
    ) -> Result<Vec<String>, JsValue> {
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        self.inner
            .get_active_dates_for_ticker(ticker_idx)
            .map(dates_to_iso)
            .map_err(to_js_err)
    }

    /// Inspect exact active ticker/benchmark paired return-ending dates.
    /// @param tickerIdx - Zero-based ticker column index; must be a finite non-negative integer in range.
    /// @returns Ascending ISO-8601 dates aligned to the corresponding compact return series.
    /// @throws Error - Rejects an invalid ticker index or missing explicit benchmark; an empty valid overlap returns an empty array.
    #[wasm_bindgen(js_name = getPairedDates)]
    pub fn get_paired_dates(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
    ) -> Result<Vec<String>, JsValue> {
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        self.inner
            .get_paired_dates(ticker_idx)
            .map(dates_to_iso)
            .map_err(to_js_err)
    }

    /// Actual-date compound annual growth using ACT/365F unless another clock is selected.
    /// @param dayCount - Optional CagrDayCount label, default act_365f; act365_25 selects explicit actual-days/365.25.
    /// @param ctx - Optional validated DayCountContext; omission or null supplies the empty context. Required calendar/coupon inputs must be explicitly populated for clocks that use them.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects an unknown clock, missing/incompatible day-count context, invalid actual holding chronology, or unrepresentable growth. Empty history and a valid zero year fraction remain NaN per ticker.
    #[wasm_bindgen(unchecked_return_type = "Float64Array")]
    pub fn cagr(
        &self,
        #[wasm_bindgen(js_name = "dayCount", unchecked_optional_param_type = "string | null")]
        day_count: Option<JsValue>,
        #[wasm_bindgen(unchecked_optional_param_type = "DayCountContext | null")] ctx: Option<
            JsValue,
        >,
    ) -> Result<JsValue, JsValue> {
        let day_count =
            js_opt_parse::<CagrDayCount>(day_count.as_ref(), "dayCount")?.unwrap_or_default();
        let ctx = js_opt_day_count_context(ctx.as_ref(), "ctx")?;
        result_vec_f64_to_js(self.inner.cagr(day_count, ctx))
    }

    /// Annualized all-observation downside deviation.
    /// Longest observed drawdown duration in calendar days.
    /// @returns One calendar-day count per ticker, or null for an empty active history.
    /// @throws Error - Rejects an invalid derived drawdown path or serialization failure; empty ticker histories produce null durations.
    #[wasm_bindgen(js_name = maxDrawdownDuration, unchecked_return_type = "(number | null)[]")]
    pub fn max_drawdown_duration(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.max_drawdown_duration().map_err(to_js_err)?)
    }

    /// Inspect own-ticker active compact simple-return histories.
    /// @returns One Float64Array per ticker aligned with getActiveDatesForTicker.
    #[wasm_bindgen(js_name = getReturns, unchecked_return_type = "Float64Array[]")]
    pub fn get_returns(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.get_returns())
    }

    /// Inspect one ticker active compact return history.
    /// @param tickerIdx - Zero-based ticker column index; must be a finite non-negative integer in range.
    /// @returns Float64Array aligned with getActiveDatesForTicker(tickerIdx).
    /// @throws Error - Rejects a fractional, negative, nonfinite or out-of-range ticker index.
    #[wasm_bindgen(js_name = getReturnsForTicker, unchecked_return_type = "Float64Array")]
    pub fn get_returns_for_ticker(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
    ) -> Result<JsValue, JsValue> {
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        Ok(vec_f64_to_js(
            &self
                .inner
                .get_returns_for_ticker(ticker_idx)
                .map_err(to_js_err)?,
        ))
    }

    /// Compound each ticker from the actual opening of its selected history.
    /// @returns One Float64Array per ticker in supplied order; empty histories produce empty rows.
    /// @throws Error - Rejects requested cumulative wealth outputs that overflow or lose a representable nonzero result; empty selected histories return empty rows.
    #[wasm_bindgen(js_name = cumulativeReturns, unchecked_return_type = "Float64Array[]")]
    pub fn cumulative_returns(&self) -> Result<JsValue, JsValue> {
        Ok(matrix_f64_to_js(
            &self.inner.cumulative_returns().map_err(to_js_err)?,
        ))
    }

    /// Geometric drawdown path rebased at the actual selected opening valuation.
    /// @returns One Float64Array per ticker in supplied order; empty histories produce empty rows.
    #[wasm_bindgen(js_name = getDrawdownSeries, unchecked_return_type = "Float64Array[]")]
    pub fn get_drawdown_series(&self) -> JsValue {
        matrix_f64_to_js(&self.inner.get_drawdown_series())
    }

    /// Exact paired wealth quotient minus one; benchmark wipeout is NaN.
    /// @returns One Float64Array per ticker in supplied order; empty histories produce empty rows.
    /// @throws Error - Rejects a missing explicit benchmark or unrepresentable positive-numeraire relative wealth. Benchmark terminal zero gives NaN at that endpoint; empty overlaps give empty rows.
    #[wasm_bindgen(js_name = cumulativeReturnsOutperformance, unchecked_return_type = "Float64Array[]")]
    pub fn cumulative_returns_outperformance(&self) -> Result<JsValue, JsValue> {
        Ok(matrix_f64_to_js(
            &self
                .inner
                .cumulative_returns_outperformance()
                .map_err(to_js_err)?,
        ))
    }

    /// Difference of independently rebased ticker and benchmark drawdown paths on their exact paired sample.
    /// @returns One Float64Array per ticker in supplied order; empty histories produce empty rows.
    /// @throws Error - Rejects a missing explicit benchmark or unrepresentable paired drawdown paths. Empty valid overlaps give empty rows.
    #[wasm_bindgen(js_name = drawdownDifference, unchecked_return_type = "Float64Array[]")]
    pub fn drawdown_difference(&self) -> Result<JsValue, JsValue> {
        Ok(matrix_f64_to_js(
            &self.inner.drawdown_difference().map_err(to_js_err)?,
        ))
    }

    /// Fixed 22-metric report using owner calculations, historical VaR/ES and explicit policy inputs.
    /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
    /// @param mar - Required finite per-observation minimum acceptable return, in the same units and periodicity as the returns.
    /// @param omegaThreshold - Required finite per-observation Omega hurdle, independent of MAR and cash.
    /// @param confidence - Required finite tail confidence strictly between 0.5 and 1.
    /// @returns Column-oriented TableEnvelope with numeric NaN/infinity values and metadata describing inputs and row samples.
    /// @throws Error - Rejects missing explicit frequency/benchmark, invalid cash/MAR/Omega/confidence inputs, invalid report structure/provenance or any metric calculation failure. Field-specific unavailable estimates remain native numeric NaN.
    #[wasm_bindgen(unchecked_return_type = "TableEnvelope")]
    pub fn summary(
        &self,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
        risk_free_rate: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] mar: JsValue,
        #[wasm_bindgen(js_name = "omegaThreshold", unchecked_param_type = "number")]
        omega_threshold: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] confidence: JsValue,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_f64(&risk_free_rate, "riskFreeRate")?;
        let mar = js_f64(&mar, "mar")?;
        let omega_threshold = js_f64(&omega_threshold, "omegaThreshold")?;
        let confidence = js_f64(&confidence, "confidence")?;
        to_js_table_numeric(
            &self
                .inner
                .summary(risk_free_rate, mar, omega_threshold, confidence)
                .map_err(to_js_err)?,
        )
    }

    /// Compound each occupied calendar bucket without inserting missing periods.
    /// @param frequency - Required explicit calendar frequency: daily, weekly, monthly, quarterly, semi_annual or annual.
    /// @returns Ticker-major chronological PeriodicReturn points labeled by each bucket last observed return date.
    /// @throws Error - Rejects an absent/unknown frequency label, unrepresentable containing period boundary or compounded bucket result.
    #[wasm_bindgen(js_name = periodicReturns, unchecked_return_type = "PeriodicReturn[][]")]
    pub fn periodic_returns(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] frequency: JsValue,
    ) -> Result<JsValue, JsValue> {
        let frequency: PeriodKind = js_parse(&frequency, "frequency")?;
        to_js_value(&self.inner.periodic_returns(frequency).map_err(to_js_err)?)
    }

    /// Common-sample Pearson correlation without pairwise fallback or automatic repair.
    /// @returns CorrelationResult containing ticker identities, matrix, common observation count and return-ending bounds.
    /// @throws Error - Rejects fewer than two all-ticker common observations, any constant common-sample column, materially invalid normalized coefficients/PSD admission, or unrepresentable calculations. There is no pairwise fallback or repair.
    #[wasm_bindgen(js_name = correlationMatrix, unchecked_return_type = "CorrelationResult")]
    pub fn correlation_matrix(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.correlation_matrix().map_err(to_js_err)?)
    }

    /// Subtract an aligned annual-effective cash series after geometric period conversion.
    /// @param rf - Annual-effective decimal cash rates greater than -1, aligned to global active return-ending dates.
    /// @returns One Float64Array per ticker aligned with its active dates.
    /// @throws Error - Rejects absent explicit frequency, wrong global active-grid length, any nonfinite cash or cash at/below -1, or unrepresentable differential return results.
    #[wasm_bindgen(js_name = excessReturns, unchecked_return_type = "Float64Array[]")]
    pub fn excess_returns(
        &self,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] rf: JsValue,
    ) -> Result<JsValue, JsValue> {
        let rf = js_f64_seq(&rf, "rf")?;
        Ok(matrix_f64_to_js(
            &self.inner.excess_returns(&rf).map_err(to_js_err)?,
        ))
    }

    /// Paired intercept OLS beta with fixed 95% Student-t uncertainty.
    /// @returns BetaResult per ticker with paired sample counts; point estimates need two pairs and uncertainty three.
    /// @throws Error - Rejects a missing explicit benchmark or failed numerical OLS evaluation. Fewer than two paired observations or constant benchmark produce field-specific NaNs; uncertainty needs three pairs.
    #[wasm_bindgen(unchecked_return_type = "BetaResult[]")]
    pub fn beta(&self) -> Result<JsValue, JsValue> {
        to_js_rows_numeric(&self.inner.beta().map_err(to_js_err)?)
    }

    /// Paired intercept OLS beta, annualized Jensen alpha and fit statistics.
    /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
    /// @returns GreeksResult objects in ticker order with exact paired sample counts.
    /// @throws Error - Rejects missing benchmark/frequency, absent/invalid annual-effective cash or unrepresentable OLS/Jensen calculations. Insufficient/constant benchmark samples retain field-specific NaNs.
    #[wasm_bindgen(unchecked_return_type = "GreeksResult[]")]
    pub fn greeks(
        &self,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
        risk_free_rate: JsValue,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_f64(&risk_free_rate, "riskFreeRate")?;
        to_js_rows_numeric(&self.inner.greeks(risk_free_rate).map_err(to_js_err)?)
    }

    /// Rolling paired OLS beta and annualized Jensen alpha.
    /// @param tickerIdx - Zero-based ticker column index; must be a finite non-negative integer in range.
    /// @param window - Required positive integer observation count; overlong windows produce empty compact series.
    /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
    /// @returns Compact right-labeled RollingGreeks with numeric NaNs in unavailable windows.
    /// @throws Error - Rejects an invalid ticker/window, missing explicit observation frequency, missing explicit benchmark, missing/invalid cash, or unrepresentable window calculations. Overlong windows return empty carriers; estimator-specific unavailable windows remain NaN.
    #[wasm_bindgen(js_name = rollingGreeks, unchecked_return_type = "RollingGreeks")]
    pub fn rolling_greeks(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] window: JsValue,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
        risk_free_rate: JsValue,
    ) -> Result<JsValue, JsValue> {
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        let window = js_uint(&window, "window")?;
        let risk_free_rate = js_f64(&risk_free_rate, "riskFreeRate")?;
        let series = self
            .inner
            .rolling_greeks(ticker_idx, window, risk_free_rate)
            .map_err(to_js_err)?;
        rolling_greeks_to_js(&series)
    }

    /// Select largest strict-recovery drawdown episodes with actual dates.
    /// @param tickerIdx - Zero-based ticker column index; must be a finite non-negative integer in range.
    /// @param n - Required nonnegative display count; zero returns an empty array and fewer observed episodes returns all available.
    /// @returns DrawdownEpisode objects ordered by magnitude; no approximate recovery metadata.
    /// @throws Error - Rejects an invalid ticker index, missing/negative/fractional display count, invalid path chronology or unrepresentable episode results.
    #[wasm_bindgen(js_name = drawdownDetails, unchecked_return_type = "DrawdownEpisode[]")]
    pub fn drawdown_details(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] n: JsValue,
    ) -> Result<JsValue, JsValue> {
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        let n = js_uint(&n, "n")?;
        to_js_value(
            &self
                .inner
                .drawdown_details(ticker_idx, n)
                .map_err(to_js_err)?,
        )
    }

    /// Total-investment panel response regression on caller-aligned already-excess factor series.
    /// @param tickerIdx - Zero-based ticker column index; must be a finite non-negative integer in range.
    /// @param factorReturns - One already-excess factor row per factor, each exactly aligned to the selected ticker active return dates.
    /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
    /// @returns MultiFactorResult with annualized cash-adjusted alpha, raw factor slopes, fit statistics and residual sample risk.
    /// @throws Error - Rejects invalid ticker/cash, absent frequency, missing/misaligned/nonfinite factors, insufficient rows for an intercept fit, deficient normalized rank or unrepresentable coefficients/risk. Field-specific unavailable residual statistics remain NaN.
    #[wasm_bindgen(js_name = multiFactorGreeks, unchecked_return_type = "MultiFactorResult")]
    pub fn multi_factor_greeks(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
        #[wasm_bindgen(js_name = "factorReturns", unchecked_param_type = "NumericMatrix")]
        factor_returns: JsValue,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
        risk_free_rate: JsValue,
    ) -> Result<JsValue, JsValue> {
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        let factors = js_f64_matrix(&factor_returns, "factorReturns")?;
        let refs: Vec<&[f64]> = factors.iter().map(Vec::as_slice).collect();
        let risk_free_rate = js_f64(&risk_free_rate, "riskFreeRate")?;
        to_js_value_numeric(
            &self
                .inner
                .multi_factor_greeks(ticker_idx, &refs, risk_free_rate)
                .map_err(to_js_err)?,
        )
    }

    /// Period-to-date lookback returns.
    ///
    /// MTD, QTD and YTD start on the first calendar day of `refDate`'s month,
    /// calendar quarter and year. FYTD is the first observation on or after the
    /// fiscal calendar start of `fiscalConfig` through `refDate`. Holidays are not
    /// skipped. The first included simple return still spans the prior close.
    /// @param refDate - ISO-8601 date on which MTD, QTD, YTD, and FYTD windows end. Omitted or `null` is the last active date; the result's `ref_date` states the date used.
    /// @param fiscalConfig - Optional `FiscalConfig` handle for FYTD. Omitted or `null` selects the explicit January 1 rule (`FiscalConfig.calendarYear()`, ANALYTICS-003).
    /// @returns LookbackReturns with return estimates, fiscal context and per-ticker counts and actual holding bounds for each selected window; unavailable numeric results remain NaN.
    /// @throws Error - Rejects an invalid ISO `ref_date`, an omitted one while no dates are active, an invalid `fiscalConfig` handle, or a result that cannot be serialized to JavaScript.
    #[wasm_bindgen(js_name = lookbackReturns, unchecked_return_type = "LookbackReturns")]
    pub fn lookback_returns(
        &self,
        #[wasm_bindgen(js_name = "refDate", unchecked_optional_param_type = "string | null")]
        ref_date: Option<JsValue>,
        #[wasm_bindgen(
            js_name = "fiscalConfig",
            unchecked_optional_param_type = "FiscalConfig | null"
        )]
        fiscal_config: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let ref_date = js_opt_date(ref_date.as_ref(), "refDate")?;
        let fiscal_config = js_opt_fiscal_config(fiscal_config.as_ref(), "fiscalConfig")?;
        let result = self
            .inner
            .lookback_returns(ref_date, fiscal_config)
            .map_err(to_js_err)?;
        lookback_to_js(&result)
    }

    /// Aggregated period statistics for one asset at the given frequency. Without
    /// a fiscal calendar, Gregorian periods are bucketed with ISO weeks.
    /// @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
    /// @param aggregationFrequency - Required explicit grouping frequency token; no inference from observation frequency.
    /// @param fiscalConfig - Optional FiscalConfig handle; omitted or null buckets Gregorian periods with ISO weeks.
    /// @returns Period statistics object for the selected ticker at the requested frequency.
    /// @throws Error - Rejects an unsupported `aggregation_frequency`, an out-of-range `ticker_idx`, an invalid `fiscalConfig` handle, an unrepresentable period boundary, or period statistics that cannot be serialized to JavaScript.
    #[wasm_bindgen(js_name = periodStats, unchecked_return_type = "PeriodStats")]
    pub fn period_stats(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
        #[wasm_bindgen(js_name = "aggregationFrequency", unchecked_param_type = "string")]
        aggregation_frequency: JsValue,
        #[wasm_bindgen(
            js_name = "fiscalConfig",
            unchecked_optional_param_type = "FiscalConfig | null"
        )]
        fiscal_config: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        let frequency: PeriodKind = js_parse(&aggregation_frequency, "aggregationFrequency")?;
        let fiscal_config = js_opt_fiscal_config(fiscal_config.as_ref(), "fiscalConfig")?;
        to_js_value_numeric(
            &self
                .inner
                .period_stats(ticker_idx, frequency, fiscal_config)
                .map_err(to_js_err)?,
        )
    }
}

json_round_trip!(JsPerformance, Performance);

macro_rules! perf_f64_methods {
    (
        zero { $( $(#[$zdoc:meta])* $zmethod:ident as $zjs:ident ),* $(,)? }
        one { $( $(#[$odoc:meta])* $omethod:ident as $ojs:ident ($oarg:ident : $ojs_arg:literal => $oconv:ident : $ots:literal) ),* $(,)? }
        two { $( $(#[$tdoc:meta])* $tmethod:ident as $tjs:ident ($ta:ident : $ta_js:literal => $ta_conv:ident : $ta_ts:literal, $tb:ident : $tb_js:literal => $tb_conv:ident : $tb_ts:literal) ),* $(,)? }
        two_opt_uint { $( $(#[$pdoc:meta])* $pmethod:ident as $pjs:ident ($pa:ident : $pa_js:literal, $pb:ident : $pb_js:literal) ),* $(,)? }
    ) => {
        #[finstack_quant_wasm_macros::plain_docs]
        #[wasm_bindgen(js_class = Performance)]
        impl JsPerformance {
            $(
                $(#[$zdoc])*
                #[wasm_bindgen(js_name = $zjs, unchecked_return_type = "Float64Array")]
                pub fn $zmethod(&self) -> Result<JsValue, JsValue> {
                    result_vec_f64_to_js(self.inner.$zmethod())
                }
            )*
            $(
                $(#[$odoc])*
                #[wasm_bindgen(js_name = $ojs, unchecked_return_type = "Float64Array")]
                pub fn $omethod(
                    &self,
                    #[wasm_bindgen(js_name = $ojs_arg, unchecked_param_type = $ots)]
                    $oarg: JsValue,
                ) -> Result<JsValue, JsValue> {
                    let $oarg = $oconv(&$oarg, $ojs_arg)?;
                    result_vec_f64_to_js(self.inner.$omethod($oarg))
                }
            )*
            $(
                $(#[$tdoc])*
                #[wasm_bindgen(js_name = $tjs, unchecked_return_type = "Float64Array")]
                pub fn $tmethod(
                    &self,
                    #[wasm_bindgen(js_name = $ta_js, unchecked_param_type = $ta_ts)]
                    $ta: JsValue,
                    #[wasm_bindgen(js_name = $tb_js, unchecked_param_type = $tb_ts)]
                    $tb: JsValue,
                ) -> Result<JsValue, JsValue> {
                    let $ta = $ta_conv(&$ta, $ta_js)?;
                    let $tb = $tb_conv(&$tb, $tb_js)?;
                    result_vec_f64_to_js(self.inner.$tmethod($ta, $tb))
                }
            )*
            $(
                $(#[$pdoc])*
                #[wasm_bindgen(js_name = $pjs, unchecked_return_type = "Float64Array")]
                pub fn $pmethod(
                    &self,
                    #[wasm_bindgen(js_name = $pa_js, unchecked_param_type = "number")]
                    $pa: JsValue,
                    #[wasm_bindgen(js_name = $pb_js, unchecked_optional_param_type = "number")]
                    $pb: Option<JsValue>,
                ) -> Result<JsValue, JsValue> {
                    let $pa = js_f64(&$pa, $pa_js)?;
                    let $pb: Option<u64> = js_opt_uint($pb.as_ref(), $pb_js)?;
                    result_vec_f64_to_js(self.inner.$pmethod($pa, $pb))
                }
            )*
        }
    };
}

macro_rules! perf_rolling_methods {
    (
        two { $( $(#[$rdoc2:meta])* $rmethod2:ident as $rjs2:ident ),* $(,)? }
        three { $( $(#[$rdoc3:meta])* $rmethod3:ident as $rjs3:ident ($rarg:ident : $rjs_arg:literal) ),* $(,)? }
    ) => {
        #[finstack_quant_wasm_macros::plain_docs]
        #[wasm_bindgen(js_class = Performance)]
        impl JsPerformance {
            $(
                $(#[$rdoc2])*
                #[wasm_bindgen(js_name = $rjs2, unchecked_return_type = "DatedSeries")]
                pub fn $rmethod2(
                    &self,
                    #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")]
                    ticker_idx: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "number")] window: JsValue,
                ) -> Result<JsValue, JsValue> {
                    let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
                    let window = js_uint(&window, "window")?;
                    let series = self.inner.$rmethod2(ticker_idx, window).map_err(to_js_err)?;
                    dated_series_to_js(&series)
                }
            )*
            $(
                $(#[$rdoc3])*
                #[wasm_bindgen(js_name = $rjs3, unchecked_return_type = "DatedSeries")]
                pub fn $rmethod3(
                    &self,
                    #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")]
                    ticker_idx: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "number")] window: JsValue,
                    #[wasm_bindgen(js_name = $rjs_arg, unchecked_param_type = "number")]
                    $rarg: JsValue,
                ) -> Result<JsValue, JsValue> {
                    let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
                    let window = js_uint(&window, "window")?;
                    let $rarg = js_f64(&$rarg, $rjs_arg)?;
                    let series = self.inner.$rmethod3(ticker_idx, window, $rarg).map_err(to_js_err)?;
                    dated_series_to_js(&series)
                }
            )*
        }
    };
}

perf_f64_methods!(
    zero {
        /// Actual-date CAGR divided by maximum drawdown magnitude.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects an unrepresentable CAGR, drawdown denominator or finite-denominator ratio. Empty history and zero reward over zero drawdown remain NaN.
        calmar as calmar,
        /// Observation-weighted signed mean drawdown.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects an unrepresentable signed mean drawdown; empty history remains NaN.
        mean_drawdown as meanDrawdown,
        /// Maximum geometric peak-to-trough decline as a nonpositive fraction.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects an unrepresentable requested drawdown; empty history remains NaN.
        max_drawdown as maxDrawdown,
        /// Annualized sample standard deviation of exact paired active returns.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects a missing explicit benchmark or observation frequency, or unrepresentable exact-paired statistics. Empty/insufficient populations and zero over zero remain NaN; no near-zero denominator cutoff is used.
        tracking_error as trackingError,
        /// Annualized arithmetic active mean divided by paired tracking error.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects a missing explicit benchmark or observation frequency, or unrepresentable exact-paired statistics. Empty/insufficient populations and zero over zero remain NaN; no near-zero denominator cutoff is used.
        information_ratio as informationRatio,
        /// Fisher-adjusted Type 2 sample skewness; unavailable with fewer than three observations or zero variance.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects unrepresentable requested shape statistics. Fewer than three observations or a constant history remain NaN.
        skewness as skewness,
        /// Fisher-adjusted Type 2 excess kurtosis; unavailable with fewer than four observations or zero variance.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects unrepresentable requested shape statistics. Fewer than four observations or a constant history remain NaN.
        kurtosis as kurtosis,
        /// Observation-period geometric mean investment return.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects an unrepresentable geometric mean. Empty history remains NaN; exact terminal loss yields -1.
        geometric_mean as geometricMean,
        /// Signed geometric capture on strictly positive benchmark observations.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects a missing explicit benchmark or observation frequency, or unrepresentable exact-paired statistics. Empty/insufficient populations and zero over zero remain NaN; no near-zero denominator cutoff is used.
        up_capture as upCapture,
        /// Signed geometric capture on strictly negative benchmark observations.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects a missing explicit benchmark or observation frequency, or unrepresentable exact-paired statistics. Empty/insufficient populations and zero over zero remain NaN; no near-zero denominator cutoff is used.
        down_capture as downCapture,
        /// Up capture divided by down capture, preserving signed zero-risk outcomes.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects a missing explicit benchmark or observation frequency, or unrepresentable exact-paired statistics. Empty/insufficient populations and zero over zero remain NaN; no near-zero denominator cutoff is used.
        capture_ratio as captureRatio,
        /// Arithmetic return sum divided by negative-return magnitudes.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects an unrepresentable requested arithmetic sum ratio. Empty or all-neutral history remains NaN; observed gains without losses yield infinity.
        gain_to_pain as gainToPain,
        /// Root mean squared percentage drawdown across return-ending observations.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects an unrepresentable endpoint root-mean-square drawdown. Empty history remains NaN.
        ulcer_index as ulcerIndex,
        /// Total compounded return divided by maximum drawdown magnitude.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects unrepresentable total growth or a finite-denominator recovery ratio. Empty history and zero over zero remain NaN.
        recovery_factor as recoveryFactor,
        /// Mean absolute percentage drawdown across return-ending observations.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects an unrepresentable mean endpoint drawdown magnitude. Empty history remains NaN.
        pain_index as painIndex,
        /// Equal-weight paired intercept OLS squared correlation.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects a missing explicit benchmark or failed exact-paired numerical calculation. Empty or estimator-specific unavailable populations remain NaN.
        r_squared as rSquared,
        /// Share of exact paired observations with strictly larger ticker return.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects a missing explicit benchmark or failed exact-paired numerical calculation. Empty or estimator-specific unavailable populations remain NaN.
        batting_average as battingAverage,
    }
    one {
        /// Equal-weight arithmetic mean return.
        /// @param annualize - Required unit selection: true uses explicit stored frequency; false returns the observation-period statistic.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects a non-boolean unit selection, absent observation frequency when annualize is true, or an unrepresentable requested mean. Empty history remains NaN.
        mean_return as meanReturn (annualize: "annualize" => js_bool : "boolean"),
        /// Sample standard deviation with n-1 denominator.
        /// @param annualize - Required unit selection: true uses explicit stored frequency; false returns the observation-period statistic.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects a non-boolean unit selection, absent observation frequency when annualize is true, or unrepresentable requested sample risk. Fewer than two observations remain NaN.
        volatility as volatility (annualize: "annualize" => js_bool : "boolean"),
        /// Annualized arithmetic excess mean over sample volatility.
        /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects missing/invalid annual-effective cash, absent explicit observation frequency, or unrepresentable requested statistics. Unavailable or zero-over-zero estimates remain NaN; genuine nonzero-over-zero risk retains signed infinity.
        sharpe as sharpe (risk_free_rate: "riskFreeRate" => js_f64 : "number"),
        /// Annualized arithmetic excess mean divided by paired beta.
        /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects missing/invalid annual-effective cash, absent explicit observation frequency, missing selected benchmark, or unrepresentable requested statistics. Unavailable or zero-over-zero estimates remain NaN; genuine nonzero-over-zero risk retains signed infinity.
        treynor as treynor (risk_free_rate: "riskFreeRate" => js_f64 : "number"),
        /// Cash-adjusted actual-date CAGR divided by Ulcer Index.
        /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects missing/nonfinite annual-effective cash or cash at/below -1, unrepresentable actual-date CAGR/path denominators, or a finite-denominator result that cannot be represented. Empty histories and zero over zero remain NaN.
        martin_ratio as martinRatio (risk_free_rate: "riskFreeRate" => js_f64 : "number"),
        /// Cash-adjusted actual-date CAGR divided by Pain Index.
        /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects missing/nonfinite annual-effective cash or cash at/below -1, unrepresentable actual-date CAGR/path denominators, or a finite-denominator result that cannot be represented. Empty histories and zero over zero remain NaN.
        pain_ratio as painRatio (risk_free_rate: "riskFreeRate" => js_f64 : "number"),
        /// Absolute benchmark-volatility adjusted return including cash.
        /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects missing/invalid annual-effective cash, absent explicit observation frequency, missing selected benchmark, or unrepresentable requested statistics. Unavailable or zero-over-zero estimates remain NaN; genuine nonzero-over-zero risk retains signed infinity.
        m_squared as mSquared (risk_free_rate: "riskFreeRate" => js_f64 : "number"),
        /// Modified Burke: cash-adjusted CAGR over the square root of all episode-depth squares divided by drawdown-path observation count.
        /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects missing/nonfinite annual-effective cash or cash at/below -1, unrepresentable actual-date CAGR/path denominators, or a finite-denominator result that cannot be represented. Empty histories and zero over zero remain NaN.
        burke_ratio as burkeRatio (risk_free_rate: "riskFreeRate" => js_f64 : "number"),
        /// Frequency-scaled arithmetic excess over all-observation downside deviation.
        /// @param mar - Required finite per-observation minimum acceptable return, in the same units and periodicity as the returns.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects missing/nonfinite MAR, absent explicit frequency, or unrepresentable reward/downside calculations. Fewer than two observations or zero over zero remain NaN.
        sortino as sortino (mar: "mar" => js_f64 : "number"),
        /// Annualized all-observation downside deviation.
        /// @param mar - Required finite per-observation minimum acceptable return, in the same units and periodicity as the returns.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects missing/nonfinite MAR, absent explicit frequency, or unrepresentable requested downside deviation. Empty history remains NaN.
        downside_deviation as downsideDeviation (mar: "mar" => js_f64 : "number"),
        /// Unannualized empirical gains divided by shortfalls around an explicit hurdle.
        /// @param threshold - Required finite per-observation decimal hurdle; no reuse of cash or MAR.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects a missing/nonfinite threshold or an unrepresentable two-sided gain/shortfall ratio. Empty/all-at-threshold histories remain NaN.
        omega_ratio as omegaRatio (threshold: "threshold" => js_f64 : "number"),
        /// Historical signed R-7 return quantile.
        /// @param confidence - Required finite tail confidence strictly between 0.5 and 1.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects missing/nonfinite confidence or confidence outside 0.5 < c < 1, and any unrepresentable requested tail statistic. Empty valid histories remain NaN; no tail-count cutoff is imposed.
        value_at_risk as valueAtRisk (confidence: "confidence" => js_f64 : "number"),
        /// Historical signed mean of exactly the worst 1-confidence empirical mass with fractional boundary weight.
        /// @param confidence - Required finite tail confidence strictly between 0.5 and 1.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects missing/nonfinite confidence or confidence outside 0.5 < c < 1, and any unrepresentable requested tail statistic. Empty valid histories remain NaN; no tail-count cutoff is imposed.
        expected_shortfall as expectedShortfall (confidence: "confidence" => js_f64 : "number"),
        /// Magnitude ratio of upper and lower R-7 return quantiles.
        /// @param confidence - Required finite tail confidence strictly between 0.5 and 1.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects missing/nonfinite confidence or confidence outside 0.5 < c < 1, and any unrepresentable requested tail statistic. Empty valid histories remain NaN; no tail-count cutoff is imposed.
        tail_ratio as tailRatio (confidence: "confidence" => js_f64 : "number"),
        /// Nonpositive average of the worst percentage-drawdown empirical mass.
        /// @param confidence - Required finite tail confidence strictly between 0.5 and 1.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects missing/nonfinite confidence or confidence outside 0.5 < c < 1, and any unrepresentable requested tail statistic. Empty valid histories remain NaN; no tail-count cutoff is imposed.
        cdar as cdar (confidence: "confidence" => js_f64 : "number"),
    }
    two {
        /// Observation-period arithmetic cash excess divided by the negated total-return Cornish-Fisher quantile; no annualization.
        /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
        /// @param confidence - Required finite tail confidence strictly between 0.5 and 1.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects missing/invalid cash or confidence, absent explicit frequency, a globally unordered Cornish-Fisher polynomial, or unrepresentable requested results. Unavailable CF samples or nonpositive CF loss denominators remain NaN.
        modified_sharpe as modifiedSharpe (risk_free_rate: "riskFreeRate" => js_f64 : "number", confidence: "confidence" => js_f64 : "number"),
        /// Cash-adjusted actual-date CAGR divided by mean magnitude of the selected largest episode depths.
        /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
        /// @param n - Required positive count of largest episodes; fewer observed episodes uses all available episodes.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects missing/invalid cash, a nonpositive/fractional episode count, or unrepresentable reward/episode-denominator calculations. Empty history and zero over zero remain NaN.
        sterling_ratio as sterlingRatio (risk_free_rate: "riskFreeRate" => js_f64 : "number", n: "n" => js_uint : "number"),
    }
    two_opt_uint {
        /// Additive i.i.d. Gaussian signed return quantile; outputs may extend below -1.
        /// @param confidence - Required finite tail confidence strictly between 0.5 and 1.
        /// @param horizonPeriods - Optional whole observation-period horizon in 1..=9007199254740991; omitted is one. No rounding or fractional horizons.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects missing/invalid confidence, horizons outside integral 1..=9007199254740991, or unrepresentable requested model estimates. Estimator-specific insufficient histories remain NaN; no fallback or output floor is applied.
        parametric_var as parametricVar (confidence: "confidence", horizon_periods: "horizonPeriods"),
        /// Classical Type 2 plug-in Cornish-Fisher signed quantile with global ordering validation and explicit rejection of invalid polynomials.
        /// @param confidence - Required finite tail confidence strictly between 0.5 and 1.
        /// @param horizonPeriods - Optional whole observation-period horizon in 1..=9007199254740991; omitted is one. No rounding or fractional horizons.
        /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
        /// @throws Error - Rejects missing/invalid confidence, horizons outside integral 1..=9007199254740991, a globally unordered Cornish-Fisher polynomial, or unrepresentable requested model estimates. Estimator-specific insufficient histories remain NaN; no fallback or output floor is applied.
        cornish_fisher_var as cornishFisherVar (confidence: "confidence", horizon_periods: "horizonPeriods"),
    }
);

perf_rolling_methods!(
    two {
        /// Rolling annualized sample standard deviation.
        /// @param tickerIdx - Zero-based ticker column index; must be a finite non-negative integer in range.
        /// @param window - Required positive integer observation count; overlong windows produce empty compact series.
        /// @returns Compact DatedSeries with right-labeled dates and numeric unavailable/zero-risk values.
        /// @throws Error - Rejects an invalid ticker/window, missing explicit observation frequency, or unrepresentable window calculations. Overlong windows return empty carriers; estimator-specific unavailable windows remain NaN.
        rolling_volatility as rollingVolatility,
        /// Rolling geometric simple return.
        /// @param tickerIdx - Zero-based ticker column index; must be a finite non-negative integer in range.
        /// @param window - Required positive integer observation count; overlong windows produce empty compact series.
        /// @returns Compact DatedSeries with right-labeled dates and numeric unavailable/zero-risk values.
        /// @throws Error - Rejects an invalid ticker/window, or unrepresentable window calculations. Overlong windows return empty carriers; estimator-specific unavailable windows remain NaN.
        rolling_returns as rollingReturns,
    }
    three {
        /// Rolling annualized all-observation downside ratio.
        /// @param tickerIdx - Zero-based ticker column index; must be a finite non-negative integer in range.
        /// @param window - Required positive integer observation count; overlong windows produce empty compact series.
        /// @param mar - Required finite per-observation minimum acceptable return, in the same units and periodicity as the returns.
        /// @returns Compact DatedSeries with right-labeled dates and numeric unavailable/zero-risk values.
        /// @throws Error - Rejects an invalid ticker/window, missing explicit observation frequency, missing/nonfinite MAR, or unrepresentable window calculations. Overlong windows return empty carriers; estimator-specific unavailable windows remain NaN.
        rolling_sortino as rollingSortino (mar: "mar"),
        /// Rolling annualized arithmetic cash-excess ratio.
        /// @param tickerIdx - Zero-based ticker column index; must be a finite non-negative integer in range.
        /// @param window - Required positive integer observation count; overlong windows produce empty compact series.
        /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
        /// @returns Compact DatedSeries with right-labeled dates and numeric unavailable/zero-risk values.
        /// @throws Error - Rejects an invalid ticker/window, missing explicit observation frequency, missing/invalid cash, or unrepresentable window calculations. Overlong windows return empty carriers; estimator-specific unavailable windows remain NaN.
        rolling_sharpe as rollingSharpe (risk_free_rate: "riskFreeRate"),
    }
);
