//! Stateful [`Performance`] analytics engine.

use super::types::*;
use crate::bindings::core::dates::utils::{date_to_py, py_to_date};
use crate::bindings::pandas_utils::{dates_to_datetime_index, dict_to_dataframe};
use crate::errors::analytics_to_py as core_to_py;
use finstack_quant_analytics as fa;
use finstack_quant_core::dates::{CalendarRegistry, FiscalConfig, HolidayCalendar, PeriodKind};
use numpy::PyArray1;
use pyo3::buffer::PyBuffer;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Wrap a `Vec<f64>` as a NumPy `float64` array, taking ownership of the
/// buffer so no per-element `PyFloat` boxing occurs.
fn vec_to_pyarray<'py>(py: Python<'py>, values: Vec<f64>) -> Bound<'py, PyArray1<f64>> {
    PyArray1::from_vec(py, values)
}

/// Wrap a borrowed `&[f64]` as a NumPy `float64` array (one copy).
fn slice_to_pyarray<'py>(py: Python<'py>, values: &[f64]) -> Bound<'py, PyArray1<f64>> {
    PyArray1::from_slice(py, values)
}

fn py_dates_to_rust(dates: &[Bound<'_, PyAny>]) -> PyResult<Vec<time::Date>> {
    dates.iter().map(py_to_date).collect()
}

/// Default fiscal-year start month (January) when callers do not override.
const DEFAULT_FISCAL_START_MONTH: u8 = 1;
/// Default fiscal-year start day-of-month.
const DEFAULT_FISCAL_START_DAY: u8 = 1;

/// Resolve optional fiscal-year-start month/day into a [`FiscalConfig`].
fn make_fiscal_config(month: Option<u8>, day: Option<u8>) -> PyResult<FiscalConfig> {
    FiscalConfig::new(
        month.unwrap_or(DEFAULT_FISCAL_START_MONTH),
        day.unwrap_or(DEFAULT_FISCAL_START_DAY),
    )
    .map_err(core_to_py)
}

fn resolve_fiscal_calendar(calendar_id: &str) -> PyResult<&'static dyn HolidayCalendar> {
    CalendarRegistry::global()
        .resolve_str(calendar_id)
        .ok_or_else(|| {
            core_to_py(
                finstack_quant_core::Error::calendar_not_found_with_suggestions(
                    calendar_id.to_string(),
                    finstack_quant_core::dates::available_calendars(),
                ),
            )
        })
}

/// Parse a frequency string into a [`PeriodKind`].
fn parse_freq(freq: &str) -> PyResult<PeriodKind> {
    freq.parse::<PeriodKind>().map_err(|_| {
        crate::errors::value_error(format!(
            "Unknown frequency {freq:?}; expected one of: \
             daily, weekly, monthly, quarterly, semiannual, annual"
        ))
    })
}

fn ensure_pandas_dataframe(value: &Bound<'_, PyAny>, error_message: &str) -> PyResult<()> {
    let pd = value.py().import("pandas")?;
    let df_type = pd.getattr("DataFrame")?;
    if value.is_instance(&df_type)? {
        Ok(())
    } else {
        Err(PyTypeError::new_err(error_message.to_owned()))
    }
}

/// Decomposed DataFrame: dates, column-major numeric values, and ticker names.
struct DataFramePanel {
    /// Chronological observation dates.
    dates: Vec<time::Date>,
    /// `columns[ticker_idx][date_idx]`.
    columns: Vec<Vec<f64>>,
    /// Column names from the DataFrame.
    ticker_names: Vec<String>,
}

fn extract_dataframe_panel(df: &Bound<'_, PyAny>, error_message: &str) -> PyResult<DataFramePanel> {
    ensure_pandas_dataframe(df, error_message)?;
    extract_dataframe(df)
}

/// Extract dates, numeric matrix, and ticker names from a pandas DataFrame.
///
/// Expects a DataFrame with a date-like index and float64 columns. Numeric
/// data flows through the NumPy buffer protocol rather than
/// ``Series.tolist()`` so a 100k×N price panel does not pay for a Python list
/// of `float` objects per cell.
fn extract_dataframe(df: &Bound<'_, PyAny>) -> PyResult<DataFramePanel> {
    let index = df.getattr("index")?;
    let dates_list = index.call_method0("tolist")?;
    let dates_py: Vec<Bound<'_, PyAny>> = dates_list.extract()?;
    let dates = py_dates_to_rust(&dates_py)?;

    let columns = df.getattr("columns")?;
    let cols_list = columns.call_method0("tolist")?;
    let ticker_names: Vec<String> = cols_list.extract()?;

    let n_tickers = ticker_names.len();
    let mut columns = Vec::with_capacity(n_tickers);
    for col in &ticker_names {
        let series = df.get_item(col)?;
        columns.push(extract_float64_column(&series, col)?);
    }

    Ok(DataFramePanel {
        dates,
        columns,
        ticker_names,
    })
}

/// Pull a `Series` column out as a contiguous `Vec<f64>` via the NumPy buffer
/// protocol. Returns explicit `PyTypeError` / `PyValueError` instead of
/// silently coercing through `Series.tolist()`.
fn extract_float64_column(series: &Bound<'_, PyAny>, col_label: &str) -> PyResult<Vec<f64>> {
    // `Series.to_numpy(dtype="float64", copy=False)` keeps existing float64
    // arrays zero-copy and forces numeric coercion errors at the boundary
    // rather than letting them propagate as silent NaNs.
    let py = series.py();
    let kwargs = PyDict::new(py);
    kwargs.set_item("dtype", "float64")?;
    kwargs.set_item("copy", false)?;
    let array = series
        .call_method("to_numpy", (), Some(&kwargs))
        .map_err(|err| {
            PyTypeError::new_err(format!(
                "Column {col_label:?} could not be converted to a float64 NumPy array: {err}"
            ))
        })?;

    let buffer = PyBuffer::<f64>::get(&array).map_err(|err| {
        crate::errors::value_error(format!(
            "Column {col_label:?} did not expose a contiguous float64 buffer: {err}"
        ))
    })?;
    buffer.to_vec(py).map_err(|err| {
        crate::errors::value_error(format!(
            "Column {col_label:?} could not be read as float64 buffer: {err}"
        ))
    })
}

/// Build a `Performance` from pre-extracted arrays.
fn build_performance(
    dates: Vec<time::Date>,
    prices: Vec<Vec<f64>>,
    ticker_names: Vec<String>,
    benchmark_ticker: Option<&str>,
    freq: &str,
) -> PyResult<PyPerformance> {
    let period_kind = parse_freq(freq)?;
    let inner = fa::Performance::new(dates, prices, ticker_names, benchmark_ticker, period_kind)
        .map_err(core_to_py)?;
    Ok(PyPerformance { inner })
}

/// Build a `Performance` from pre-extracted return arrays.
fn build_returns_performance(
    dates: Vec<time::Date>,
    returns: Vec<Vec<f64>>,
    ticker_names: Vec<String>,
    benchmark_ticker: Option<&str>,
    freq: &str,
) -> PyResult<PyPerformance> {
    let period_kind = parse_freq(freq)?;
    let inner =
        fa::Performance::from_returns(dates, returns, ticker_names, benchmark_ticker, period_kind)
            .map_err(core_to_py)?;
    Ok(PyPerformance { inner })
}

fn ticker_index<'py>(py: Python<'py>, ticker_names: &[String]) -> PyResult<Bound<'py, PyAny>> {
    let index: Vec<&str> = ticker_names.iter().map(String::as_str).collect();
    Ok(index.into_pyobject(py)?.into_any())
}

fn panel_to_dataframe<'py>(
    py: Python<'py>,
    perf: &fa::Performance,
    panel: Vec<Vec<f64>>,
) -> PyResult<Bound<'py, PyAny>> {
    let ticker_names = perf.ticker_names();
    let dates = perf.active_dates();
    let data = PyDict::new(py);
    for (ticker_idx, (name, series)) in ticker_names.iter().zip(panel.into_iter()).enumerate() {
        let ticker_dates = perf
            .active_dates_for_ticker(ticker_idx)
            .map_err(core_to_py)?;
        let mut padded = vec![f64::NAN; dates.len()];
        let mut global_idx = 0usize;
        for (&date, &value) in ticker_dates.iter().zip(series.iter()) {
            while global_idx < dates.len() && dates[global_idx] < date {
                global_idx += 1;
            }
            if global_idx < dates.len() && dates[global_idx] == date {
                padded[global_idx] = value;
            }
        }
        data.set_item(name, vec_to_pyarray(py, padded))?;
    }
    let idx = dates_to_datetime_index(py, dates)?;
    dict_to_dataframe(py, &data, Some(idx))
}

/// Stateful performance analytics engine over a panel of ticker price series.
///
/// Accepts a pandas ``DataFrame`` where the index contains dates and each
/// column is a price series for one ticker.
#[pyclass(name = "Performance", module = "finstack_quant.analytics")]
pub(super) struct PyPerformance {
    inner: fa::Performance,
}

impl PyPerformance {
    fn lookback_returns_inner(
        &self,
        ref_date: time::Date,
        fiscal_year_start_month: Option<u8>,
        fiscal_year_start_day: Option<u8>,
        calendar: &str,
    ) -> PyResult<fa::LookbackReturns> {
        let fc = make_fiscal_config(fiscal_year_start_month, fiscal_year_start_day)?;
        let calendar = resolve_fiscal_calendar(calendar)?;
        self.inner
            .lookback_returns(ref_date, fc, calendar)
            .map_err(core_to_py)
    }
}

#[pymethods]
impl PyPerformance {
    /// Construct from a pandas DataFrame of prices.
    ///
    /// The DataFrame index must contain ``datetime.date`` or ``pd.Timestamp``
    /// values, and each column represents one ticker's price series.
    #[new]
    #[pyo3(signature = (prices, benchmark_ticker=None, freq="daily"))]
    fn new(prices: Bound<'_, PyAny>, benchmark_ticker: Option<&str>, freq: &str) -> PyResult<Self> {
        let panel = extract_dataframe_panel(
            &prices,
            "Expected a pandas DataFrame; use Performance.from_arrays() for raw lists",
        )?;
        build_performance(
            panel.dates,
            panel.columns,
            panel.ticker_names,
            benchmark_ticker,
            freq,
        )
    }

    /// Construct from raw arrays (dates, prices matrix, ticker names).
    #[staticmethod]
    #[pyo3(signature = (dates, prices, ticker_names, benchmark_ticker=None, freq="daily"))]
    fn from_arrays(
        dates: Vec<Bound<'_, PyAny>>,
        prices: Vec<Vec<f64>>,
        ticker_names: Vec<String>,
        benchmark_ticker: Option<&str>,
        freq: &str,
    ) -> PyResult<Self> {
        let rust_dates = py_dates_to_rust(&dates)?;
        build_performance(rust_dates, prices, ticker_names, benchmark_ticker, freq)
    }

    /// Construct from a pandas DataFrame of returns.
    ///
    /// The DataFrame index must contain ``datetime.date`` or ``pd.Timestamp``
    /// values, and each column represents one ticker's simple-return series
    /// aligned with the index.
    #[staticmethod]
    #[pyo3(signature = (returns, benchmark_ticker=None, freq="daily"))]
    fn from_returns(
        returns: Bound<'_, PyAny>,
        benchmark_ticker: Option<&str>,
        freq: &str,
    ) -> PyResult<Self> {
        let panel = extract_dataframe_panel(
            &returns,
            "Expected a pandas DataFrame; use Performance.from_returns_arrays() for raw lists",
        )?;
        build_returns_performance(
            panel.dates,
            panel.columns,
            panel.ticker_names,
            benchmark_ticker,
            freq,
        )
    }

    /// Construct from raw return arrays (dates, returns matrix, ticker names).
    #[staticmethod]
    #[pyo3(signature = (dates, returns, ticker_names, benchmark_ticker=None, freq="daily"))]
    fn from_returns_arrays(
        dates: Vec<Bound<'_, PyAny>>,
        returns: Vec<Vec<f64>>,
        ticker_names: Vec<String>,
        benchmark_ticker: Option<&str>,
        freq: &str,
    ) -> PyResult<Self> {
        let rust_dates = py_dates_to_rust(&dates)?;
        build_returns_performance(rust_dates, returns, ticker_names, benchmark_ticker, freq)
    }

    // -- Mutators --

    /// Restrict analytics to a date window.
    fn reset_date_range(&mut self, start: Bound<'_, PyAny>, end: Bound<'_, PyAny>) -> PyResult<()> {
        let s = py_to_date(&start)?;
        let e = py_to_date(&end)?;
        self.inner.reset_date_range(s, e);
        Ok(())
    }

    /// Change the benchmark ticker.
    fn reset_bench_ticker(&mut self, ticker: &str) -> PyResult<()> {
        self.inner.reset_bench_ticker(ticker).map_err(core_to_py)
    }

    // -- Getters --

    /// Ticker names in column order.
    #[getter]
    fn ticker_names(&self) -> Vec<String> {
        self.inner.ticker_names().to_vec()
    }

    /// Benchmark column index.
    #[getter]
    fn benchmark_idx(&self) -> usize {
        self.inner.benchmark_idx()
    }

    /// Observation frequency, as the canonical lowercase token (e.g. ``"daily"``,
    /// ``"monthly"``, ``"semiannual"``) that round-trips through the ``freq``
    /// constructor argument and :meth:`period_stats` ``agg_freq`` parameter.
    #[getter]
    fn freq(&self) -> String {
        self.inner.freq().to_string()
    }

    /// Full return-aligned date grid (independent of any active window).
    ///
    /// Matches Rust ``Performance::dates``. For the dates inside the
    /// currently selected analysis window, use :meth:`active_dates`.
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        self.inner
            .dates()
            .iter()
            .map(|&d| date_to_py(py, d))
            .collect()
    }

    /// Date grid of the currently active analysis window.
    ///
    /// Matches Rust ``Performance::active_dates``. Equal to :meth:`dates`
    /// until :meth:`reset_date_range` narrows the window.
    fn active_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        self.inner
            .active_dates()
            .iter()
            .map(|&d| date_to_py(py, d))
            .collect()
    }

    /// Date grid corresponding to one ticker's active return series.
    ///
    /// On edge-ragged panels this excludes leading/trailing missing rows for
    /// the selected ticker.
    fn active_dates_for_ticker<'py>(
        &self,
        py: Python<'py>,
        ticker_idx: usize,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        self.inner
            .active_dates_for_ticker(ticker_idx)
            .map_err(core_to_py)?
            .iter()
            .map(|&d| date_to_py(py, d))
            .collect()
    }

    // -- Scalar-per-ticker methods --

    /// CAGR for each ticker.
    fn cagr(&self) -> PyResult<Vec<f64>> {
        self.inner.cagr().map_err(core_to_py)
    }

    /// Mean return for each ticker.
    #[pyo3(signature = (annualize = true))]
    fn mean_return(&self, annualize: bool) -> Vec<f64> {
        self.inner.mean_return(annualize)
    }

    /// Volatility for each ticker.
    #[pyo3(signature = (annualize = true))]
    fn volatility(&self, annualize: bool) -> Vec<f64> {
        self.inner.volatility(annualize)
    }

    /// Sharpe ratio for each ticker.
    #[pyo3(signature = (risk_free_rate = 0.0))]
    fn sharpe(&self, risk_free_rate: f64) -> Vec<f64> {
        self.inner.sharpe(risk_free_rate)
    }

    /// Sortino ratio for each ticker.
    ///
    /// ``mar`` is a per-period minimum acceptable return, unlike Sharpe
    /// ``risk_free_rate`` inputs, which are annualized.
    #[pyo3(signature = (mar = 0.0))]
    fn sortino(&self, mar: f64) -> Vec<f64> {
        self.inner.sortino(mar)
    }

    /// Calmar ratio for each ticker.
    fn calmar(&self) -> PyResult<Vec<f64>> {
        self.inner.calmar().map_err(core_to_py)
    }

    /// Max drawdown for each ticker.
    fn max_drawdown(&self) -> Vec<f64> {
        self.inner.max_drawdown()
    }

    /// Mean drawdown (path-weighted average) for each ticker.
    fn mean_drawdown(&self) -> Vec<f64> {
        self.inner.mean_drawdown()
    }

    /// Historical VaR for each ticker.
    #[pyo3(signature = (confidence = 0.95))]
    fn value_at_risk(&self, confidence: f64) -> Vec<f64> {
        self.inner.value_at_risk(confidence)
    }

    /// Expected Shortfall for each ticker.
    #[pyo3(signature = (confidence = 0.95))]
    fn expected_shortfall(&self, confidence: f64) -> Vec<f64> {
        self.inner.expected_shortfall(confidence)
    }

    /// Tracking error for each ticker vs benchmark.
    fn tracking_error(&self) -> Vec<f64> {
        self.inner.tracking_error()
    }

    /// Information ratio for each ticker vs benchmark.
    fn information_ratio(&self) -> Vec<f64> {
        self.inner.information_ratio()
    }

    /// Skewness for each ticker.
    fn skewness(&self) -> Vec<f64> {
        self.inner.skewness()
    }

    /// Kurtosis for each ticker.
    fn kurtosis(&self) -> Vec<f64> {
        self.inner.kurtosis()
    }

    /// Geometric mean for each ticker.
    fn geometric_mean(&self) -> Vec<f64> {
        self.inner.geometric_mean()
    }

    /// Per-ticker ``(skewness, kurtosis)`` from one moments pass per ticker.
    ///
    /// Returns two parallel lists, equivalent to calling :meth:`skewness`
    /// and :meth:`kurtosis` but walking each ticker once.
    fn skew_kurt(&self) -> (Vec<f64>, Vec<f64>) {
        self.inner.skew_kurt()
    }

    /// Per-ticker ``(value_at_risk, expected_shortfall)`` from one tail pass.
    ///
    /// Returns two parallel lists, equivalent to calling
    /// :meth:`value_at_risk` and :meth:`expected_shortfall` but sharing the
    /// tail partition.
    #[pyo3(signature = (confidence = 0.95))]
    fn value_at_risk_and_es(&self, confidence: f64) -> (Vec<f64>, Vec<f64>) {
        self.inner.value_at_risk_and_es(confidence)
    }

    /// Downside deviation for each ticker.
    ///
    /// ``mar`` is a per-period threshold, unlike Sharpe ``risk_free_rate``
    /// inputs, which are annualized.
    #[pyo3(signature = (mar = 0.0))]
    fn downside_deviation(&self, mar: f64) -> Vec<f64> {
        self.inner.downside_deviation(mar)
    }

    /// Max drawdown duration (calendar days) for each ticker.
    fn max_drawdown_duration(&self) -> Vec<i64> {
        self.inner.max_drawdown_duration()
    }

    /// Empyrical-style annualized geometric up-capture ratio vs benchmark.
    fn up_capture(&self) -> Vec<f64> {
        self.inner.up_capture()
    }

    /// Empyrical-style annualized geometric down-capture ratio vs benchmark.
    fn down_capture(&self) -> Vec<f64> {
        self.inner.down_capture()
    }

    /// Empyrical-style annualized geometric capture ratio vs benchmark.
    fn capture_ratio(&self) -> Vec<f64> {
        self.inner.capture_ratio()
    }

    /// Omega ratio for each ticker.
    #[pyo3(signature = (threshold = 0.0))]
    fn omega_ratio(&self, threshold: f64) -> Vec<f64> {
        self.inner.omega_ratio(threshold)
    }

    /// Treynor ratio for each ticker.
    #[pyo3(signature = (risk_free_rate = 0.0))]
    fn treynor(&self, risk_free_rate: f64) -> Vec<f64> {
        self.inner.treynor(risk_free_rate)
    }

    /// Gain-to-pain ratio for each ticker.
    fn gain_to_pain(&self) -> Vec<f64> {
        self.inner.gain_to_pain()
    }

    /// Ulcer index for each ticker.
    fn ulcer_index(&self) -> Vec<f64> {
        self.inner.ulcer_index()
    }

    /// Martin ratio for each ticker.
    fn martin_ratio(&self) -> PyResult<Vec<f64>> {
        self.inner.martin_ratio().map_err(core_to_py)
    }

    /// Recovery factor for each ticker.
    fn recovery_factor(&self) -> Vec<f64> {
        self.inner.recovery_factor()
    }

    /// Pain index for each ticker.
    fn pain_index(&self) -> Vec<f64> {
        self.inner.pain_index()
    }

    /// Pain ratio for each ticker.
    #[pyo3(signature = (risk_free_rate = 0.0))]
    fn pain_ratio(&self, risk_free_rate: f64) -> PyResult<Vec<f64>> {
        self.inner.pain_ratio(risk_free_rate).map_err(core_to_py)
    }

    /// Tail ratio for each ticker.
    #[pyo3(signature = (confidence = 0.95))]
    fn tail_ratio(&self, confidence: f64) -> Vec<f64> {
        self.inner.tail_ratio(confidence)
    }

    /// R-squared for each ticker vs benchmark.
    fn r_squared(&self) -> Vec<f64> {
        self.inner.r_squared()
    }

    /// Batting average for each ticker vs benchmark.
    fn batting_average(&self) -> Vec<f64> {
        self.inner.batting_average()
    }

    /// Parametric VaR for each ticker.
    #[pyo3(signature = (confidence = 0.95))]
    fn parametric_var(&self, confidence: f64) -> Vec<f64> {
        self.inner.parametric_var(confidence)
    }

    /// Cornish-Fisher VaR for each ticker.
    #[pyo3(signature = (confidence = 0.95))]
    fn cornish_fisher_var(&self, confidence: f64) -> Vec<f64> {
        self.inner.cornish_fisher_var(confidence)
    }

    /// CDaR for each ticker.
    #[pyo3(signature = (confidence = 0.95))]
    fn cdar(&self, confidence: f64) -> Vec<f64> {
        self.inner.cdar(confidence)
    }

    /// M-squared for each ticker.
    #[pyo3(signature = (risk_free_rate = 0.0))]
    fn m_squared(&self, risk_free_rate: f64) -> Vec<f64> {
        self.inner.m_squared(risk_free_rate)
    }

    /// Modified Sharpe ratio for each ticker.
    #[pyo3(signature = (risk_free_rate = 0.0, confidence = 0.95))]
    fn modified_sharpe(&self, risk_free_rate: f64, confidence: f64) -> Vec<f64> {
        self.inner.modified_sharpe(risk_free_rate, confidence)
    }

    /// Sterling ratio for each ticker.
    #[pyo3(signature = (risk_free_rate = 0.0, n = 5))]
    fn sterling_ratio(&self, risk_free_rate: f64, n: usize) -> PyResult<Vec<f64>> {
        self.inner
            .sterling_ratio(risk_free_rate, n)
            .map_err(core_to_py)
    }

    /// Burke ratio for each ticker.
    #[pyo3(signature = (risk_free_rate = 0.0, n = 5))]
    fn burke_ratio(&self, risk_free_rate: f64, n: usize) -> PyResult<Vec<f64>> {
        self.inner
            .burke_ratio(risk_free_rate, n)
            .map_err(core_to_py)
    }

    // -- Vector-per-ticker methods --

    /// Cumulative returns for each ticker.
    fn cumulative_returns(&self) -> Vec<Vec<f64>> {
        self.inner.cumulative_returns()
    }

    /// Drawdown series for each ticker.
    fn drawdown_series(&self) -> Vec<Vec<f64>> {
        self.inner.drawdown_series()
    }

    /// Correlation matrix across all tickers.
    fn correlation_matrix(&self) -> Vec<Vec<f64>> {
        self.inner.correlation_matrix()
    }

    /// Cumulative returns outperformance vs benchmark.
    fn cumulative_returns_outperformance(&self) -> Vec<Vec<f64>> {
        self.inner.cumulative_returns_outperformance()
    }

    /// Drawdown difference vs benchmark.
    fn drawdown_difference(&self) -> Vec<Vec<f64>> {
        self.inner.drawdown_difference()
    }

    /// Excess returns over a risk-free rate series.
    #[pyo3(signature = (rf, nperiods = None))]
    fn excess_returns(&self, rf: Vec<f64>, nperiods: Option<f64>) -> Vec<Vec<f64>> {
        self.inner.excess_returns(&rf, nperiods)
    }

    // -- Per-ticker indexed methods --

    /// Beta for each ticker vs benchmark.
    fn beta(&self) -> Vec<PyBetaResult> {
        self.inner
            .beta()
            .into_iter()
            .map(|b| PyBetaResult { inner: b })
            .collect()
    }

    /// Greeks (annualized Jensen alpha, beta, R²) for each ticker vs benchmark.
    #[pyo3(signature = (risk_free_rate = 0.0))]
    fn greeks(&self, risk_free_rate: f64) -> Vec<PyGreeksResult> {
        self.inner
            .greeks(risk_free_rate)
            .into_iter()
            .map(|g| PyGreeksResult { inner: g })
            .collect()
    }

    /// Rolling greeks for a specific ticker.
    #[pyo3(signature = (ticker_idx, window = 63, risk_free_rate = 0.0))]
    fn rolling_greeks(
        &self,
        py: Python<'_>,
        ticker_idx: usize,
        window: usize,
        risk_free_rate: f64,
    ) -> PyResult<PyRollingGreeks> {
        let inner = py
            .detach(|| {
                self.inner
                    .rolling_greeks(ticker_idx, window, risk_free_rate)
            })
            .map_err(core_to_py)?;
        Ok(PyRollingGreeks { inner })
    }

    /// Rolling volatility for a specific ticker.
    #[pyo3(signature = (ticker_idx, window = 63))]
    fn rolling_volatility(
        &self,
        py: Python<'_>,
        ticker_idx: usize,
        window: usize,
    ) -> PyResult<PyDatedSeries> {
        let inner = py
            .detach(|| self.inner.rolling_volatility(ticker_idx, window))
            .map_err(core_to_py)?;
        Ok(PyDatedSeries::new(inner, "volatility"))
    }

    /// Rolling Sortino for a specific ticker.
    #[pyo3(signature = (ticker_idx, window = 63, mar = 0.0))]
    fn rolling_sortino(
        &self,
        py: Python<'_>,
        ticker_idx: usize,
        window: usize,
        mar: f64,
    ) -> PyResult<PyDatedSeries> {
        let inner = py
            .detach(|| self.inner.rolling_sortino(ticker_idx, window, mar))
            .map_err(core_to_py)?;
        Ok(PyDatedSeries::new(inner, "sortino"))
    }

    /// Rolling Sharpe for a specific ticker.
    #[pyo3(signature = (ticker_idx, window = 63, risk_free_rate = 0.0))]
    fn rolling_sharpe(
        &self,
        py: Python<'_>,
        ticker_idx: usize,
        window: usize,
        risk_free_rate: f64,
    ) -> PyResult<PyDatedSeries> {
        let inner = py
            .detach(|| {
                self.inner
                    .rolling_sharpe(ticker_idx, window, risk_free_rate)
            })
            .map_err(core_to_py)?;
        Ok(PyDatedSeries::new(inner, "sharpe"))
    }

    /// Drawdown episodes for a specific ticker.
    #[pyo3(signature = (ticker_idx, n = 5))]
    fn drawdown_details(&self, ticker_idx: usize, n: usize) -> PyResult<Vec<PyDrawdownEpisode>> {
        Ok(self
            .inner
            .drawdown_details(ticker_idx, n)
            .map_err(core_to_py)?
            .into_iter()
            .map(|e| PyDrawdownEpisode { inner: e })
            .collect())
    }

    /// Multi-factor regression for a specific ticker.
    fn multi_factor_greeks(
        &self,
        py: Python<'_>,
        ticker_idx: usize,
        factor_returns: Vec<Vec<f64>>,
    ) -> PyResult<PyMultiFactorResult> {
        let refs: Vec<&[f64]> = factor_returns.iter().map(|v| v.as_slice()).collect();
        py.detach(|| self.inner.multi_factor_greeks(ticker_idx, &refs))
            .map(|r| PyMultiFactorResult { inner: r })
            .map_err(core_to_py)
    }

    /// Rolling N-period total compounded return for a specific ticker.
    #[pyo3(signature = (ticker_idx, window))]
    fn rolling_returns(
        &self,
        py: Python<'_>,
        ticker_idx: usize,
        window: usize,
    ) -> PyResult<PyDatedSeries> {
        let inner = py
            .detach(|| self.inner.rolling_returns(ticker_idx, window))
            .map_err(core_to_py)?;
        Ok(PyDatedSeries::new(inner, "return"))
    }

    /// Period-to-date lookback returns.
    ///
    /// The FYTD window starts at the fiscal-year start
    /// (``fiscal_year_start_month`` / ``fiscal_year_start_day``) adjusted to
    /// the next business day on ``calendar``. The default calendar is
    /// ``"nyse"``; pass the calendar matching your market (any id registered
    /// in the core ``CalendarRegistry``) for non-US panels.
    #[pyo3(signature = (ref_date, fiscal_year_start_month = None, fiscal_year_start_day = None, calendar = "nyse"))]
    fn lookback_returns(
        &self,
        ref_date: Bound<'_, PyAny>,
        fiscal_year_start_month: Option<u8>,
        fiscal_year_start_day: Option<u8>,
        calendar: &str,
    ) -> PyResult<PyLookbackReturns> {
        let d = py_to_date(&ref_date)?;
        Ok(PyLookbackReturns {
            inner: self.lookback_returns_inner(
                d,
                fiscal_year_start_month,
                fiscal_year_start_day,
                calendar,
            )?,
        })
    }

    /// Period statistics for a specific ticker at a given aggregation frequency.
    #[pyo3(signature = (ticker_idx, agg_freq = "monthly", fiscal_year_start_month = None, fiscal_year_start_day = None))]
    fn period_stats(
        &self,
        ticker_idx: usize,
        agg_freq: &str,
        fiscal_year_start_month: Option<u8>,
        fiscal_year_start_day: Option<u8>,
    ) -> PyResult<PyPeriodStats> {
        let pk = parse_freq(agg_freq)?;
        let fc = make_fiscal_config(fiscal_year_start_month, fiscal_year_start_day)?;
        Ok(PyPeriodStats {
            inner: self
                .inner
                .period_stats(ticker_idx, pk, Some(fc))
                .map_err(core_to_py)?,
        })
    }

    // -- DataFrame export methods --

    /// Summary statistics for all tickers as a pandas ``DataFrame``.
    ///
    /// Returns a DataFrame with one row per ticker and columns for each
    /// scalar metric (CAGR, volatility, Sharpe, max drawdown, etc.).
    ///
    /// ``risk_free_rate`` affects only the ``sharpe`` column; the MAR-based
    /// metrics (``sortino``, ``downside_deviation``) and the
    /// ``omega_ratio`` threshold are fixed at ``0.0``. Call
    /// :meth:`sortino`, :meth:`downside_deviation`, or :meth:`omega_ratio`
    /// directly for non-zero thresholds. ``confidence`` applies to
    /// ``value_at_risk``, ``expected_shortfall``, and ``tail_ratio``.
    #[pyo3(signature = (risk_free_rate = 0.0, confidence = 0.95))]
    fn summary_to_dataframe<'py>(
        &self,
        py: Python<'py>,
        risk_free_rate: f64,
        confidence: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        // Share work where the analytics layer offers combined helpers, so
        // pairs that always travel together (VaR/ES, skewness/kurtosis) only
        // walk each ticker once.
        let (var, es) = self.inner.value_at_risk_and_es(confidence);
        let (skew, kurt) = self.inner.skew_kurt();

        let metrics: [(&str, Vec<f64>); 22] = [
            ("cagr", self.inner.cagr().map_err(core_to_py)?),
            ("mean_return", self.inner.mean_return(true)),
            ("volatility", self.inner.volatility(true)),
            ("sharpe", self.inner.sharpe(risk_free_rate)),
            ("sortino", self.inner.sortino(0.0)),
            ("calmar", self.inner.calmar().map_err(core_to_py)?),
            ("max_drawdown", self.inner.max_drawdown()),
            ("value_at_risk", var),
            ("expected_shortfall", es),
            ("tracking_error", self.inner.tracking_error()),
            ("information_ratio", self.inner.information_ratio()),
            ("skewness", skew),
            ("kurtosis", kurt),
            ("geometric_mean", self.inner.geometric_mean()),
            ("downside_deviation", self.inner.downside_deviation(0.0)),
            ("omega_ratio", self.inner.omega_ratio(0.0)),
            ("gain_to_pain", self.inner.gain_to_pain()),
            ("ulcer_index", self.inner.ulcer_index()),
            ("pain_index", self.inner.pain_index()),
            ("recovery_factor", self.inner.recovery_factor()),
            ("tail_ratio", self.inner.tail_ratio(confidence)),
            ("r_squared", self.inner.r_squared()),
        ];

        let data = PyDict::new(py);
        for (name, values) in metrics {
            data.set_item(name, vec_to_pyarray(py, values))?;
        }

        let idx = ticker_index(py, self.inner.ticker_names())?;
        dict_to_dataframe(py, &data, Some(idx))
    }

    /// Cumulative returns for all tickers as a pandas ``DataFrame``.
    ///
    /// Returns a DataFrame with a date index and one column per ticker.
    fn cumulative_returns_to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        panel_to_dataframe(py, &self.inner, self.inner.cumulative_returns())
    }

    /// Drawdown series for all tickers as a pandas ``DataFrame``.
    ///
    /// Returns a DataFrame with a date index and one column per ticker.
    fn drawdown_series_to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        panel_to_dataframe(py, &self.inner, self.inner.drawdown_series())
    }

    /// Correlation matrix as a pandas ``DataFrame``.
    ///
    /// Returns a ticker × ticker matrix with ticker names as index and columns.
    fn correlation_to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let names = self.inner.ticker_names();
        let matrix = self.inner.correlation_matrix();

        let pd = py.import("pandas")?;
        let kwargs = PyDict::new(py);
        let idx: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        let cols: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        kwargs.set_item("index", idx)?;
        kwargs.set_item("columns", cols)?;
        pd.call_method("DataFrame", (matrix,), Some(&kwargs))
    }

    /// Top-N drawdown episodes for a ticker as a pandas ``DataFrame``.
    ///
    /// Columns: start, valley, end, duration_days, max_drawdown,
    /// near_recovery_threshold, truncated_at_start.
    #[pyo3(signature = (ticker_idx, n = 5))]
    fn drawdown_details_to_dataframe<'py>(
        &self,
        py: Python<'py>,
        ticker_idx: usize,
        n: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        let episodes = self
            .inner
            .drawdown_details(ticker_idx, n)
            .map_err(core_to_py)?;
        let data = PyDict::new(py);
        let starts: PyResult<Vec<_>> = episodes.iter().map(|e| date_to_py(py, e.start)).collect();
        let valleys: PyResult<Vec<_>> = episodes.iter().map(|e| date_to_py(py, e.valley)).collect();
        let ends: PyResult<Vec<_>> = episodes
            .iter()
            .map(|e| match e.end {
                Some(d) => date_to_py(py, d).map(|v| v.into_any()),
                None => Ok(py.None().into_bound(py)),
            })
            .collect();

        data.set_item("start", starts?)?;
        data.set_item("valley", valleys?)?;
        data.set_item("end", ends?)?;
        data.set_item(
            "duration_days",
            episodes.iter().map(|e| e.duration_days).collect::<Vec<_>>(),
        )?;
        data.set_item(
            "max_drawdown",
            episodes.iter().map(|e| e.max_drawdown).collect::<Vec<_>>(),
        )?;
        data.set_item(
            "near_recovery_threshold",
            episodes
                .iter()
                .map(|e| e.near_recovery_threshold)
                .collect::<Vec<_>>(),
        )?;
        data.set_item(
            "truncated_at_start",
            episodes
                .iter()
                .map(|e| e.truncated_at_start)
                .collect::<Vec<_>>(),
        )?;
        dict_to_dataframe(py, &data, None)
    }

    /// Period-to-date lookback returns as a pandas ``DataFrame``.
    ///
    /// Returns a DataFrame with ticker names as index and columns:
    /// mtd, qtd, ytd, and fytd. See :meth:`lookback_returns` for the FYTD
    /// fiscal-start and ``calendar`` semantics (default ``"nyse"``).
    #[pyo3(signature = (ref_date, fiscal_year_start_month = None, fiscal_year_start_day = None, calendar = "nyse"))]
    fn lookback_returns_to_dataframe<'py>(
        &self,
        py: Python<'py>,
        ref_date: Bound<'_, PyAny>,
        fiscal_year_start_month: Option<u8>,
        fiscal_year_start_day: Option<u8>,
        calendar: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let d = py_to_date(&ref_date)?;
        let lb = self.lookback_returns_inner(
            d,
            fiscal_year_start_month,
            fiscal_year_start_day,
            calendar,
        )?;

        let data = PyDict::new(py);
        data.set_item("mtd", slice_to_pyarray(py, &lb.mtd))?;
        data.set_item("qtd", slice_to_pyarray(py, &lb.qtd))?;
        data.set_item("ytd", slice_to_pyarray(py, &lb.ytd))?;
        if let Some(ref fytd) = lb.fytd {
            data.set_item("fytd", slice_to_pyarray(py, fytd))?;
        }

        let idx = ticker_index(py, self.inner.ticker_names())?;
        dict_to_dataframe(py, &data, Some(idx))
    }
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPerformance>()?;
    Ok(())
}
