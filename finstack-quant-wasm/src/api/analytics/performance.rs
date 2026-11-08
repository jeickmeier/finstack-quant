//! WASM performance analytics on an actual valuation grid.

use crate::api::core::dates::JsDayCountContext;
use crate::api::core::periods::JsFiscalConfig;
use crate::utils::input::{
    from_js_json, js_bool, js_f64, js_f64_matrix, js_f64_seq, js_opt_string, js_opt_uint,
    js_string, js_uint, json_text,
};
use crate::utils::{
    date_to_iso, parse_iso_date, to_js_err, to_js_rows_numeric, to_js_table_numeric, to_js_value,
    to_js_value_numeric,
};
use finstack_quant_analytics::{
    CagrDayCount, DatedSeries, Performance, PerformancePanel, RollingGreeks,
};
use finstack_quant_core::dates::PeriodKind;
use js_sys::{Array, Float64Array, Reflect};
use wasm_bindgen::prelude::*;

fn parse_frequency(value: &JsValue, label: &str) -> Result<PeriodKind, JsValue> {
    js_string(value, label)?.parse().map_err(to_js_err)
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
        let start = parse_iso_date(&js_string(&start, "start")?)?;
        let end = parse_iso_date(&js_string(&end, "end")?)?;
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
        self.inner
            .get_dates()
            .iter()
            .map(|&d| date_to_iso(d))
            .collect()
    }

    /// Currently selected return-ending grid.
    /// @returns Ascending ISO-8601 return-ending dates.
    #[wasm_bindgen(js_name = getActiveDates)]
    pub fn get_active_dates(&self) -> Vec<String> {
        self.inner
            .get_active_dates()
            .iter()
            .map(|&d| date_to_iso(d))
            .collect()
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
        Ok(self
            .inner
            .get_active_dates_for_ticker(ticker_idx)
            .map_err(to_js_err)?
            .iter()
            .map(|&d| date_to_iso(d))
            .collect())
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
        Ok(self
            .inner
            .get_paired_dates(ticker_idx)
            .map_err(to_js_err)?
            .iter()
            .map(|&d| date_to_iso(d))
            .collect())
    }

    /// Actual-date compound annual growth using ACT/365F unless another clock is selected.
    /// @param dayCount - Optional CagrDayCount label, default act_365f; act365_25 selects explicit actual-days/365.25.
    /// @param ctx - Validated DayCountContext supplying calendar, reference periods or termination inputs needed by the selected clock. Empty context suffices for context-free clocks.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects an unknown clock, missing/incompatible day-count context, invalid actual holding chronology, or unrepresentable growth. Empty history and a valid zero year fraction remain NaN per ticker.
    #[wasm_bindgen(unchecked_return_type = "Float64Array")]
    pub fn cagr(
        &self,
        #[wasm_bindgen(js_name = "dayCount", unchecked_param_type = "string | undefined")]
        day_count: Option<JsValue>,
        #[wasm_bindgen(unchecked_param_type = "DayCountContext")] ctx: &JsDayCountContext,
    ) -> Result<JsValue, JsValue> {
        let label = js_opt_string(day_count.as_ref(), "dayCount")?;
        let day_count = label
            .as_deref()
            .map(str::parse::<CagrDayCount>)
            .transpose()
            .map_err(to_js_err)?
            .unwrap_or_default();
        result_vec_f64_to_js(
            self.inner
                .cagr(day_count, ctx.inner.to_ctx().map_err(to_js_err)?),
        )
    }

    /// Equal-weight arithmetic mean return.
    /// @param annualize - Required unit selection: true uses explicit stored frequency; false returns the observation-period statistic.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects a non-boolean unit selection, absent observation frequency when annualize is true, or an unrepresentable requested mean. Empty history remains NaN.
    #[wasm_bindgen(js_name = meanReturn, unchecked_return_type = "Float64Array")]
    pub fn mean_return(
        &self,
        #[wasm_bindgen(unchecked_param_type = "boolean")] annualize: JsValue,
    ) -> Result<JsValue, JsValue> {
        let annualize = js_bool(&annualize, "annualize")?;
        result_vec_f64_to_js(self.inner.mean_return(annualize))
    }

    /// Sample standard deviation with n-1 denominator.
    /// @param annualize - Required unit selection: true uses explicit stored frequency; false returns the observation-period statistic.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects a non-boolean unit selection, absent observation frequency when annualize is true, or unrepresentable requested sample risk. Fewer than two observations remain NaN.
    #[wasm_bindgen(unchecked_return_type = "Float64Array")]
    pub fn volatility(
        &self,
        #[wasm_bindgen(unchecked_param_type = "boolean")] annualize: JsValue,
    ) -> Result<JsValue, JsValue> {
        let annualize = js_bool(&annualize, "annualize")?;
        result_vec_f64_to_js(self.inner.volatility(annualize))
    }

    /// Actual-date CAGR divided by maximum drawdown magnitude.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects an unrepresentable CAGR, drawdown denominator or finite-denominator ratio. Empty history and zero reward over zero drawdown remain NaN.
    #[wasm_bindgen(unchecked_return_type = "Float64Array")]
    pub fn calmar(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.calmar())
    }

    /// Observation-weighted signed mean drawdown.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects an unrepresentable signed mean drawdown; empty history remains NaN.
    #[wasm_bindgen(js_name = meanDrawdown, unchecked_return_type = "Float64Array")]
    pub fn mean_drawdown(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.mean_drawdown())
    }

    /// Maximum geometric peak-to-trough decline as a nonpositive fraction.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects an unrepresentable requested drawdown; empty history remains NaN.
    #[wasm_bindgen(js_name = maxDrawdown, unchecked_return_type = "Float64Array")]
    pub fn max_drawdown(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.max_drawdown())
    }

    /// Annualized sample standard deviation of exact paired active returns.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects a missing explicit benchmark or observation frequency, or unrepresentable exact-paired statistics. Empty/insufficient populations and zero over zero remain NaN; no near-zero denominator cutoff is used.
    #[wasm_bindgen(js_name = trackingError, unchecked_return_type = "Float64Array")]
    pub fn tracking_error(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.tracking_error())
    }

    /// Annualized arithmetic active mean divided by paired tracking error.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects a missing explicit benchmark or observation frequency, or unrepresentable exact-paired statistics. Empty/insufficient populations and zero over zero remain NaN; no near-zero denominator cutoff is used.
    #[wasm_bindgen(js_name = informationRatio, unchecked_return_type = "Float64Array")]
    pub fn information_ratio(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.information_ratio())
    }

    /// Fisher-adjusted Type 2 sample skewness; unavailable with fewer than three observations or zero variance.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects unrepresentable requested shape statistics. Fewer than three observations or a constant history remain NaN.
    #[wasm_bindgen(unchecked_return_type = "Float64Array")]
    pub fn skewness(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.skewness())
    }

    /// Fisher-adjusted Type 2 excess kurtosis; unavailable with fewer than four observations or zero variance.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects unrepresentable requested shape statistics. Fewer than four observations or a constant history remain NaN.
    #[wasm_bindgen(unchecked_return_type = "Float64Array")]
    pub fn kurtosis(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.kurtosis())
    }

    /// Observation-period geometric mean investment return.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects an unrepresentable geometric mean. Empty history remains NaN; exact terminal loss yields -1.
    #[wasm_bindgen(js_name = geometricMean, unchecked_return_type = "Float64Array")]
    pub fn geometric_mean(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.geometric_mean())
    }

    /// Signed geometric capture on strictly positive benchmark observations.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects a missing explicit benchmark or observation frequency, or unrepresentable exact-paired statistics. Empty/insufficient populations and zero over zero remain NaN; no near-zero denominator cutoff is used.
    #[wasm_bindgen(js_name = upCapture, unchecked_return_type = "Float64Array")]
    pub fn up_capture(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.up_capture())
    }

    /// Signed geometric capture on strictly negative benchmark observations.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects a missing explicit benchmark or observation frequency, or unrepresentable exact-paired statistics. Empty/insufficient populations and zero over zero remain NaN; no near-zero denominator cutoff is used.
    #[wasm_bindgen(js_name = downCapture, unchecked_return_type = "Float64Array")]
    pub fn down_capture(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.down_capture())
    }

    /// Up capture divided by down capture, preserving signed zero-risk outcomes.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects a missing explicit benchmark or observation frequency, or unrepresentable exact-paired statistics. Empty/insufficient populations and zero over zero remain NaN; no near-zero denominator cutoff is used.
    #[wasm_bindgen(js_name = captureRatio, unchecked_return_type = "Float64Array")]
    pub fn capture_ratio(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.capture_ratio())
    }

    /// Arithmetic return sum divided by negative-return magnitudes.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects an unrepresentable requested arithmetic sum ratio. Empty or all-neutral history remains NaN; observed gains without losses yield infinity.
    #[wasm_bindgen(js_name = gainToPain, unchecked_return_type = "Float64Array")]
    pub fn gain_to_pain(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.gain_to_pain())
    }

    /// Root mean squared percentage drawdown across return-ending observations.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects an unrepresentable endpoint root-mean-square drawdown. Empty history remains NaN.
    #[wasm_bindgen(js_name = ulcerIndex, unchecked_return_type = "Float64Array")]
    pub fn ulcer_index(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.ulcer_index())
    }

    /// Total compounded return divided by maximum drawdown magnitude.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects unrepresentable total growth or a finite-denominator recovery ratio. Empty history and zero over zero remain NaN.
    #[wasm_bindgen(js_name = recoveryFactor, unchecked_return_type = "Float64Array")]
    pub fn recovery_factor(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.recovery_factor())
    }

    /// Mean absolute percentage drawdown across return-ending observations.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects an unrepresentable mean endpoint drawdown magnitude. Empty history remains NaN.
    #[wasm_bindgen(js_name = painIndex, unchecked_return_type = "Float64Array")]
    pub fn pain_index(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.pain_index())
    }

    /// Equal-weight paired intercept OLS squared correlation.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects a missing explicit benchmark or failed exact-paired numerical calculation. Empty or estimator-specific unavailable populations remain NaN.
    #[wasm_bindgen(js_name = rSquared, unchecked_return_type = "Float64Array")]
    pub fn r_squared(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.r_squared())
    }

    /// Share of exact paired observations with strictly larger ticker return.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects a missing explicit benchmark or failed exact-paired numerical calculation. Empty or estimator-specific unavailable populations remain NaN.
    #[wasm_bindgen(js_name = battingAverage, unchecked_return_type = "Float64Array")]
    pub fn batting_average(&self) -> Result<JsValue, JsValue> {
        result_vec_f64_to_js(self.inner.batting_average())
    }

    /// Annualized arithmetic excess mean over sample volatility.
    /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects missing/invalid annual-effective cash, absent explicit observation frequency, or unrepresentable requested statistics. Unavailable or zero-over-zero estimates remain NaN; genuine nonzero-over-zero risk retains signed infinity.
    #[wasm_bindgen(unchecked_return_type = "Float64Array")]
    pub fn sharpe(
        &self,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
        risk_free_rate: JsValue,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_f64(&risk_free_rate, "riskFreeRate")?;
        result_vec_f64_to_js(self.inner.sharpe(risk_free_rate))
    }

    /// Annualized arithmetic excess mean divided by paired beta.
    /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects missing/invalid annual-effective cash, absent explicit observation frequency, missing selected benchmark, or unrepresentable requested statistics. Unavailable or zero-over-zero estimates remain NaN; genuine nonzero-over-zero risk retains signed infinity.
    #[wasm_bindgen(unchecked_return_type = "Float64Array")]
    pub fn treynor(
        &self,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
        risk_free_rate: JsValue,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_f64(&risk_free_rate, "riskFreeRate")?;
        result_vec_f64_to_js(self.inner.treynor(risk_free_rate))
    }

    /// Cash-adjusted actual-date CAGR divided by Ulcer Index.
    /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects missing/nonfinite annual-effective cash or cash at/below -1, unrepresentable actual-date CAGR/path denominators, or a finite-denominator result that cannot be represented. Empty histories and zero over zero remain NaN.
    #[wasm_bindgen(js_name = martinRatio, unchecked_return_type = "Float64Array")]
    pub fn martin_ratio(
        &self,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
        risk_free_rate: JsValue,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_f64(&risk_free_rate, "riskFreeRate")?;
        result_vec_f64_to_js(self.inner.martin_ratio(risk_free_rate))
    }

    /// Cash-adjusted actual-date CAGR divided by Pain Index.
    /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects missing/nonfinite annual-effective cash or cash at/below -1, unrepresentable actual-date CAGR/path denominators, or a finite-denominator result that cannot be represented. Empty histories and zero over zero remain NaN.
    #[wasm_bindgen(js_name = painRatio, unchecked_return_type = "Float64Array")]
    pub fn pain_ratio(
        &self,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
        risk_free_rate: JsValue,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_f64(&risk_free_rate, "riskFreeRate")?;
        result_vec_f64_to_js(self.inner.pain_ratio(risk_free_rate))
    }

    /// Absolute benchmark-volatility adjusted return including cash.
    /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects missing/invalid annual-effective cash, absent explicit observation frequency, missing selected benchmark, or unrepresentable requested statistics. Unavailable or zero-over-zero estimates remain NaN; genuine nonzero-over-zero risk retains signed infinity.
    #[wasm_bindgen(js_name = mSquared, unchecked_return_type = "Float64Array")]
    pub fn m_squared(
        &self,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
        risk_free_rate: JsValue,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_f64(&risk_free_rate, "riskFreeRate")?;
        result_vec_f64_to_js(self.inner.m_squared(risk_free_rate))
    }

    /// Modified Burke: cash-adjusted CAGR over the square root of all episode-depth squares divided by drawdown-path observation count.
    /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects missing/nonfinite annual-effective cash or cash at/below -1, unrepresentable actual-date CAGR/path denominators, or a finite-denominator result that cannot be represented. Empty histories and zero over zero remain NaN.
    #[wasm_bindgen(js_name = burkeRatio, unchecked_return_type = "Float64Array")]
    pub fn burke_ratio(
        &self,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
        risk_free_rate: JsValue,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_f64(&risk_free_rate, "riskFreeRate")?;
        result_vec_f64_to_js(self.inner.burke_ratio(risk_free_rate))
    }

    /// Frequency-scaled arithmetic excess over all-observation downside deviation.
    /// @param mar - Required finite per-observation minimum acceptable return, in the same units and periodicity as the returns.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects missing/nonfinite MAR, absent explicit frequency, or unrepresentable reward/downside calculations. Fewer than two observations or zero over zero remain NaN.
    #[wasm_bindgen(unchecked_return_type = "Float64Array")]
    pub fn sortino(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] mar: JsValue,
    ) -> Result<JsValue, JsValue> {
        let mar = js_f64(&mar, "mar")?;
        result_vec_f64_to_js(self.inner.sortino(mar))
    }

    /// Annualized all-observation downside deviation.
    /// @param mar - Required finite per-observation minimum acceptable return, in the same units and periodicity as the returns.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects missing/nonfinite MAR, absent explicit frequency, or unrepresentable requested downside deviation. Empty history remains NaN.
    #[wasm_bindgen(js_name = downsideDeviation, unchecked_return_type = "Float64Array")]
    pub fn downside_deviation(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] mar: JsValue,
    ) -> Result<JsValue, JsValue> {
        let mar = js_f64(&mar, "mar")?;
        result_vec_f64_to_js(self.inner.downside_deviation(mar))
    }

    /// Unannualized empirical gains divided by shortfalls around an explicit hurdle.
    /// @param threshold - Required finite per-observation decimal hurdle; no reuse of cash or MAR.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects a missing/nonfinite threshold or an unrepresentable two-sided gain/shortfall ratio. Empty/all-at-threshold histories remain NaN.
    #[wasm_bindgen(js_name = omegaRatio, unchecked_return_type = "Float64Array")]
    pub fn omega_ratio(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] threshold: JsValue,
    ) -> Result<JsValue, JsValue> {
        let threshold = js_f64(&threshold, "threshold")?;
        result_vec_f64_to_js(self.inner.omega_ratio(threshold))
    }

    /// Longest observed drawdown duration in calendar days.
    /// @returns One calendar-day count per ticker, or null for an empty active history.
    /// @throws Error - Rejects an invalid derived drawdown path or serialization failure; empty ticker histories produce null durations.
    #[wasm_bindgen(js_name = maxDrawdownDuration, unchecked_return_type = "(number | null)[]")]
    pub fn max_drawdown_duration(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.max_drawdown_duration().map_err(to_js_err)?)
    }

    /// Historical signed R-7 return quantile.
    /// @param confidence - Required finite tail confidence strictly between 0.5 and 1.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects missing/nonfinite confidence or confidence outside 0.5 < c < 1, and any unrepresentable requested tail statistic. Empty valid histories remain NaN; no tail-count cutoff is imposed.
    #[wasm_bindgen(js_name = valueAtRisk, unchecked_return_type = "Float64Array")]
    pub fn value_at_risk(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] confidence: JsValue,
    ) -> Result<JsValue, JsValue> {
        let confidence = js_f64(&confidence, "confidence")?;
        result_vec_f64_to_js(self.inner.value_at_risk(confidence))
    }

    /// Historical signed mean of exactly the worst 1-confidence empirical mass with fractional boundary weight.
    /// @param confidence - Required finite tail confidence strictly between 0.5 and 1.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects missing/nonfinite confidence or confidence outside 0.5 < c < 1, and any unrepresentable requested tail statistic. Empty valid histories remain NaN; no tail-count cutoff is imposed.
    #[wasm_bindgen(js_name = expectedShortfall, unchecked_return_type = "Float64Array")]
    pub fn expected_shortfall(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] confidence: JsValue,
    ) -> Result<JsValue, JsValue> {
        let confidence = js_f64(&confidence, "confidence")?;
        result_vec_f64_to_js(self.inner.expected_shortfall(confidence))
    }

    /// Magnitude ratio of upper and lower R-7 return quantiles.
    /// @param confidence - Required finite tail confidence strictly between 0.5 and 1.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects missing/nonfinite confidence or confidence outside 0.5 < c < 1, and any unrepresentable requested tail statistic. Empty valid histories remain NaN; no tail-count cutoff is imposed.
    #[wasm_bindgen(js_name = tailRatio, unchecked_return_type = "Float64Array")]
    pub fn tail_ratio(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] confidence: JsValue,
    ) -> Result<JsValue, JsValue> {
        let confidence = js_f64(&confidence, "confidence")?;
        result_vec_f64_to_js(self.inner.tail_ratio(confidence))
    }

    /// Nonpositive average of the worst percentage-drawdown empirical mass.
    /// @param confidence - Required finite tail confidence strictly between 0.5 and 1.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects missing/nonfinite confidence or confidence outside 0.5 < c < 1, and any unrepresentable requested tail statistic. Empty valid histories remain NaN; no tail-count cutoff is imposed.
    #[wasm_bindgen(unchecked_return_type = "Float64Array")]
    pub fn cdar(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] confidence: JsValue,
    ) -> Result<JsValue, JsValue> {
        let confidence = js_f64(&confidence, "confidence")?;
        result_vec_f64_to_js(self.inner.cdar(confidence))
    }

    /// Additive i.i.d. Gaussian signed return quantile; outputs may extend below -1.
    /// @param confidence - Required finite tail confidence strictly between 0.5 and 1.
    /// @param horizonPeriods - Optional whole observation-period horizon in 1..=9007199254740991; omitted is one. No rounding or fractional horizons.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects missing/invalid confidence, horizons outside integral 1..=9007199254740991, or unrepresentable requested model estimates. Estimator-specific insufficient histories remain NaN; no fallback or output floor is applied.
    #[wasm_bindgen(js_name = parametricVar, unchecked_return_type = "Float64Array")]
    pub fn parametric_var(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] confidence: JsValue,
        #[wasm_bindgen(js_name = "horizonPeriods", unchecked_optional_param_type = "number")]
        horizon_periods: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let confidence = js_f64(&confidence, "confidence")?;
        let horizon_periods: Option<u64> = js_opt_uint(horizon_periods.as_ref(), "horizonPeriods")?;
        result_vec_f64_to_js(self.inner.parametric_var(confidence, horizon_periods))
    }

    /// Classical Type 2 plug-in Cornish-Fisher signed quantile with global ordering validation and explicit rejection of invalid polynomials.
    /// @param confidence - Required finite tail confidence strictly between 0.5 and 1.
    /// @param horizonPeriods - Optional whole observation-period horizon in 1..=9007199254740991; omitted is one. No rounding or fractional horizons.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects missing/invalid confidence, horizons outside integral 1..=9007199254740991, a globally unordered Cornish-Fisher polynomial, or unrepresentable requested model estimates. Estimator-specific insufficient histories remain NaN; no fallback or output floor is applied.
    #[wasm_bindgen(js_name = cornishFisherVar, unchecked_return_type = "Float64Array")]
    pub fn cornish_fisher_var(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] confidence: JsValue,
        #[wasm_bindgen(js_name = "horizonPeriods", unchecked_optional_param_type = "number")]
        horizon_periods: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let confidence = js_f64(&confidence, "confidence")?;
        let horizon_periods: Option<u64> = js_opt_uint(horizon_periods.as_ref(), "horizonPeriods")?;
        result_vec_f64_to_js(self.inner.cornish_fisher_var(confidence, horizon_periods))
    }

    /// Observation-period arithmetic cash excess divided by the negated total-return Cornish-Fisher quantile; no annualization.
    /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
    /// @param confidence - Required finite tail confidence strictly between 0.5 and 1.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects missing/invalid cash or confidence, absent explicit frequency, a globally unordered Cornish-Fisher polynomial, or unrepresentable requested results. Unavailable CF samples or nonpositive CF loss denominators remain NaN.
    #[wasm_bindgen(js_name = modifiedSharpe, unchecked_return_type = "Float64Array")]
    pub fn modified_sharpe(
        &self,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
        risk_free_rate: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] confidence: JsValue,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_f64(&risk_free_rate, "riskFreeRate")?;
        let confidence = js_f64(&confidence, "confidence")?;
        result_vec_f64_to_js(self.inner.modified_sharpe(risk_free_rate, confidence))
    }

    /// Cash-adjusted actual-date CAGR divided by mean magnitude of the selected largest episode depths.
    /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
    /// @param n - Required positive count of largest episodes; fewer observed episodes uses all available episodes.
    /// @returns Per-ticker numeric values as Float64Array in supplied ticker order; NaN denotes unavailable estimates.
    /// @throws Error - Rejects missing/invalid cash, a nonpositive/fractional episode count, or unrepresentable reward/episode-denominator calculations. Empty history and zero over zero remain NaN.
    #[wasm_bindgen(js_name = sterlingRatio, unchecked_return_type = "Float64Array")]
    pub fn sterling_ratio(
        &self,
        #[wasm_bindgen(js_name = "riskFreeRate", unchecked_param_type = "number")]
        risk_free_rate: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] n: JsValue,
    ) -> Result<JsValue, JsValue> {
        let risk_free_rate = js_f64(&risk_free_rate, "riskFreeRate")?;
        let n = js_uint(&n, "n")?;
        result_vec_f64_to_js(self.inner.sterling_ratio(risk_free_rate, n))
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
        let frequency = parse_frequency(&frequency, "frequency")?;
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

    /// Rolling annualized sample standard deviation.
    /// @param tickerIdx - Zero-based ticker column index; must be a finite non-negative integer in range.
    /// @param window - Required positive integer observation count; overlong windows produce empty compact series.
    /// @returns Compact DatedSeries with right-labeled dates and numeric unavailable/zero-risk values.
    /// @throws Error - Rejects an invalid ticker/window, missing explicit observation frequency, or unrepresentable window calculations. Overlong windows return empty carriers; estimator-specific unavailable windows remain NaN.
    #[wasm_bindgen(js_name = rollingVolatility, unchecked_return_type = "DatedSeries")]
    pub fn rolling_volatility(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] window: JsValue,
    ) -> Result<JsValue, JsValue> {
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        let window = js_uint(&window, "window")?;
        let series = self
            .inner
            .rolling_volatility(ticker_idx, window)
            .map_err(to_js_err)?;
        dated_series_to_js(&series)
    }

    /// Rolling annualized all-observation downside ratio.
    /// @param tickerIdx - Zero-based ticker column index; must be a finite non-negative integer in range.
    /// @param window - Required positive integer observation count; overlong windows produce empty compact series.
    /// @param mar - Required finite per-observation minimum acceptable return, in the same units and periodicity as the returns.
    /// @returns Compact DatedSeries with right-labeled dates and numeric unavailable/zero-risk values.
    /// @throws Error - Rejects an invalid ticker/window, missing explicit observation frequency, missing/nonfinite MAR, or unrepresentable window calculations. Overlong windows return empty carriers; estimator-specific unavailable windows remain NaN.
    #[wasm_bindgen(js_name = rollingSortino, unchecked_return_type = "DatedSeries")]
    pub fn rolling_sortino(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] window: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] mar: JsValue,
    ) -> Result<JsValue, JsValue> {
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        let window = js_uint(&window, "window")?;
        let mar = js_f64(&mar, "mar")?;
        let series = self
            .inner
            .rolling_sortino(ticker_idx, window, mar)
            .map_err(to_js_err)?;
        dated_series_to_js(&series)
    }

    /// Rolling annualized arithmetic cash-excess ratio.
    /// @param tickerIdx - Zero-based ticker column index; must be a finite non-negative integer in range.
    /// @param window - Required positive integer observation count; overlong windows produce empty compact series.
    /// @param riskFreeRate - Required annual-effective decimal cash rate greater than -1; converted geometrically where periodic cash is used.
    /// @returns Compact DatedSeries with right-labeled dates and numeric unavailable/zero-risk values.
    /// @throws Error - Rejects an invalid ticker/window, missing explicit observation frequency, missing/invalid cash, or unrepresentable window calculations. Overlong windows return empty carriers; estimator-specific unavailable windows remain NaN.
    #[wasm_bindgen(js_name = rollingSharpe, unchecked_return_type = "DatedSeries")]
    pub fn rolling_sharpe(
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
            .rolling_sharpe(ticker_idx, window, risk_free_rate)
            .map_err(to_js_err)?;
        dated_series_to_js(&series)
    }

    /// Rolling geometric simple return.
    /// @param tickerIdx - Zero-based ticker column index; must be a finite non-negative integer in range.
    /// @param window - Required positive integer observation count; overlong windows produce empty compact series.
    /// @returns Compact DatedSeries with right-labeled dates and numeric unavailable/zero-risk values.
    /// @throws Error - Rejects an invalid ticker/window, or unrepresentable window calculations. Overlong windows return empty carriers; estimator-specific unavailable windows remain NaN.
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

    /// Calendar month/quarter/year and fiscal-year-to-date returns with observed sample coverage.
    /// @param refDate - Optional ISO-8601 inclusive reference date; omitted uses last active date and errors if none.
    /// @returns LookbackReturns including fiscal context and per-ticker window observation counts and actual holding bounds.
    /// @throws Error - Rejects an invalid ISO reference date, an omitted reference while no dates are active, an unrepresentable calendar/fiscal boundary or compounded requested result. Empty windows remain NaN with zero-count coverage.
    #[wasm_bindgen(js_name = lookbackReturns, unchecked_return_type = "LookbackReturns")]
    pub fn lookback_returns(
        &self,
        #[wasm_bindgen(js_name = "refDate", unchecked_optional_param_type = "string")]
        ref_date: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let ref_date = crate::utils::wire::js_opt_date(ref_date.as_ref(), "refDate")?;
        let result = self
            .inner
            .lookback_returns(ref_date, None)
            .map_err(to_js_err)?;
        lookback_to_js(&result)
    }

    /// Calendar month/quarter/year and fiscal-year-to-date returns with observed sample coverage.
    /// @param refDate - Optional ISO-8601 inclusive reference date; omitted uses last active date and errors if none.
    /// @param fiscalConfig - Explicit FiscalConfig selecting fiscal-year boundaries.
    /// @returns LookbackReturns including fiscal context and per-ticker window observation counts and actual holding bounds.
    /// @throws Error - Rejects an invalid reference date or fiscal handle, omitted reference on empty history, unrepresentable boundary or compounded result. Empty windows remain NaN with zero-count coverage.
    #[wasm_bindgen(skip_typescript, js_name = lookbackReturnsWithFiscalConfig, unchecked_return_type = "LookbackReturns")]
    pub fn lookback_returns_with_fiscal_config(
        &self,
        #[wasm_bindgen(js_name = "refDate", unchecked_param_type = "string | undefined")]
        ref_date: Option<JsValue>,
        #[wasm_bindgen(js_name = "fiscalConfig")] fiscal_config: &JsFiscalConfig,
    ) -> Result<JsValue, JsValue> {
        let ref_date = crate::utils::wire::js_opt_date(ref_date.as_ref(), "refDate")?;
        let result = self
            .inner
            .lookback_returns(ref_date, Some(fiscal_config.inner))
            .map_err(to_js_err)?;
        lookback_to_js(&result)
    }

    /// Observed occupied-bucket win/loss statistics with explicit grouping.
    /// @param tickerIdx - Zero-based ticker column index; must be a finite non-negative integer in range.
    /// @param aggregationFrequency - Required explicit grouping frequency; no default or inference from observation frequency.
    /// @returns PeriodStats including bucket/sample counts and unavailable conditional or zero-denominator estimates.
    /// @throws Error - Rejects an invalid ticker index, absent/unknown grouping frequency, unrepresentable containing period boundary or requested statistic. Unavailable conditional/zero-denominator fields remain NaN.
    #[wasm_bindgen(js_name = periodStats, unchecked_return_type = "PeriodStats")]
    pub fn period_stats(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
        #[wasm_bindgen(js_name = "aggregationFrequency", unchecked_param_type = "string")]
        aggregation_frequency: JsValue,
    ) -> Result<JsValue, JsValue> {
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        let frequency = parse_frequency(&aggregation_frequency, "aggregationFrequency")?;
        to_js_value_numeric(
            &self
                .inner
                .period_stats(ticker_idx, frequency, None)
                .map_err(to_js_err)?,
        )
    }

    /// Observed occupied-bucket win/loss statistics with explicit grouping.
    /// @param tickerIdx - Zero-based ticker column index; must be a finite non-negative integer in range.
    /// @param aggregationFrequency - Required explicit grouping frequency; no default or inference from observation frequency.
    /// @param fiscalConfig - Explicit fiscal partition context applied to all grouping frequencies.
    /// @returns PeriodStats including bucket/sample counts and unavailable conditional or zero-denominator estimates.
    /// @throws Error - Rejects an invalid ticker/fiscal handle, absent/unknown grouping frequency, unrepresentable fiscal boundary or requested statistic. Unavailable conditional fields remain NaN.
    #[wasm_bindgen(skip_typescript, js_name = periodStatsWithFiscalConfig, unchecked_return_type = "PeriodStats")]
    pub fn period_stats_with_fiscal_config(
        &self,
        #[wasm_bindgen(js_name = "tickerIdx", unchecked_param_type = "number")] ticker_idx: JsValue,
        #[wasm_bindgen(js_name = "aggregationFrequency", unchecked_param_type = "string")]
        aggregation_frequency: JsValue,
        #[wasm_bindgen(js_name = "fiscalConfig")] fiscal_config: &JsFiscalConfig,
    ) -> Result<JsValue, JsValue> {
        let ticker_idx = js_uint(&ticker_idx, "tickerIdx")?;
        let frequency = parse_frequency(&aggregation_frequency, "aggregationFrequency")?;
        to_js_value_numeric(
            &self
                .inner
                .period_stats(ticker_idx, frequency, Some(fiscal_config.inner))
                .map_err(to_js_err)?,
        )
    }

    /// Serialize canonical validated panel source state.
    /// @returns Compact JSON retaining actual dates, optional frequency/benchmark, spans and active selection.
    /// @throws Error - Rejects serialization failure or invalid source state; no legacy state shape or saved derived cache is accepted.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Restore canonical panel state and rebuild all derived caches.
    /// @param json - Canonical Performance source state; unknown keys and invalid nested domains are rejected.
    /// @returns Validated restored Performance handle.
    /// @throws Error - Rejects malformed JSON, unknown/nested invalid state, invalid actual dates/returns/spans, duplicate identities or out-of-range selection/context. Derived caches are rebuilt after validation.
    /// @example
    /// ```typescript
    /// import init, { analytics } from "finstack-quant-wasm";
    /// await init();
    /// const p = new analytics.Performance({ dates: ["2025-01-01", "2025-01-02"], series: [[0.01]], kind: "returns", ticker_names: ["A"] });
    /// const restored = analytics.Performance.fromJson(p.toJson());
    /// console.log(restored.meanReturn(false)[0]);
    /// restored.free(); p.free();
    /// ```
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(
        #[wasm_bindgen(unchecked_param_type = "string | Record<string, unknown>")] json: JsValue,
    ) -> Result<JsPerformance, JsValue> {
        let json = json_text(&json, "json")?;
        let inner = serde_json::from_str(&json).map_err(to_js_err)?;
        Ok(Self { inner })
    }
}
