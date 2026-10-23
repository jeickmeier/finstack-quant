//! Stateful [`Performance`] analytics engine and the four single-series
//! scalar functions (`sharpe`, `sortino`, `volatility`, `max_drawdown`).

use super::types::*;
use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::periods::PyFiscalConfig;
use crate::bindings::core::table::envelope_to_pandas;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::macros::wire_methods;
use crate::bindings::pandas_utils::{dates_to_pylist, int_values_to_series, values_to_series};
use crate::bindings::pydoc::pydoc;
use crate::errors::analytics_to_py;
use finstack_quant_analytics::{
    CagrDayCount, Performance, ReturnKind, DEFAULT_ANNUALIZE, DEFAULT_CONFIDENCE,
    DEFAULT_DRAWDOWN_COUNT, DEFAULT_FREQUENCY, DEFAULT_MAR, DEFAULT_PERIODIC_FREQUENCY,
    DEFAULT_PERIODS_PER_YEAR, DEFAULT_RISK_FREE_RATE, DEFAULT_ROLLING_WINDOW,
};
use finstack_quant_core::dates::{HolidayCalendar, PeriodKind};
use pyo3::buffer::PyBuffer;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

type PyPeriodicReturnPanel<'py> = Vec<Vec<(Bound<'py, PyAny>, f64)>>;

fn parse_cagr_day_count(day_count: Option<&Bound<'_, PyAny>>) -> PyResult<CagrDayCount> {
    let Some(value) = day_count.filter(|value| !value.is_none()) else {
        return Ok(CagrDayCount::default());
    };
    if let Ok(day_count) = value.extract::<PyRef<'_, PyDayCount>>() {
        return Ok(CagrDayCount::DayCount(day_count.inner));
    }
    if let Ok(label) = value.extract::<String>() {
        return label.parse::<CagrDayCount>().map_err(analytics_to_py);
    }
    Err(PyTypeError::new_err(
        "day_count must be None, 'act365_25', a DayCount name such as 'act_365f', or a DayCount",
    ))
}

fn resolve_optional_calendar(
    calendar_id: Option<&str>,
) -> PyResult<Option<&'static dyn HolidayCalendar>> {
    calendar_id
        .map(finstack_quant_core::dates::calendar_by_id_strict)
        .transpose()
        .map_err(analytics_to_py)
}

/// Parse a canonical frequency label, or use the Rust default when omitted.
fn parse_frequency(frequency: Option<&str>, default: PeriodKind) -> PyResult<PeriodKind> {
    Ok(frequency
        .map(str::parse::<PeriodKind>)
        .transpose()
        .map_err(analytics_to_py)?
        .unwrap_or(default))
}

/// Extract a 1-D float vector from a list, tuple, NumPy array, or pandas
/// ``Series`` (anything exposing ``to_numpy`` or the sequence protocol).
fn extract_f64_vec(obj: &Bound<'_, PyAny>, label: &str) -> PyResult<Vec<f64>> {
    if let Ok(values) = obj.extract::<Vec<f64>>() {
        return Ok(values);
    }
    if obj.hasattr("to_numpy")? {
        return extract_float64_column(obj, label);
    }
    Err(PyTypeError::new_err(format!(
        "{label} must be a sequence of floats, a NumPy array, or a pandas Series"
    )))
}

/// Decomposed DataFrame: dates, column-major numeric values, and ticker names.
struct DataFramePanel {
    /// Chronological observation dates.
    dates: Vec<finstack_quant_core::dates::Date>,
    /// `columns[ticker_idx][date_idx]`.
    columns: Vec<Vec<f64>>,
    /// Column names from the DataFrame.
    ticker_names: Vec<String>,
}

/// Extract dates, numeric matrix, and ticker names from a pandas DataFrame.
///
/// Expects a DataFrame with a date-like index and float64 columns; any other
/// object raises `TypeError` with `error_message`. Numeric data flows through
/// the NumPy buffer protocol rather than ``Series.tolist()`` so a 100k×N price
/// panel does not pay for a Python list of `float` objects per cell.
fn extract_dataframe_panel(df: &Bound<'_, PyAny>, error_message: &str) -> PyResult<DataFramePanel> {
    let df_type = df.py().import("pandas")?.getattr("DataFrame")?;
    if !df.is_instance(&df_type)? {
        return Err(PyTypeError::new_err(error_message.to_owned()));
    }
    let index = df.getattr("index")?;
    let dates_list = index.call_method0("tolist")?;
    let dates_py: Vec<Bound<'_, PyAny>> = dates_list.extract()?;
    let dates = dates_py
        .iter()
        .map(py_to_date)
        .collect::<PyResult<Vec<_>>>()?;

    let columns = df.getattr("columns")?;
    let cols_list = columns.call_method0("tolist")?;
    let ticker_names: Vec<String> = cols_list.extract().map_err(|_| {
        PyTypeError::new_err(
            "Performance requires string column labels (ticker names); the DataFrame \
             columns are not all str — rename them, e.g. df.columns = df.columns.astype(str)",
        )
    })?;

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

/// Promote a pandas ``Series`` to a one-column ``DataFrame`` named after the
/// series (or ``"asset"`` when unnamed); any other object passes through.
fn series_to_frame<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let pd = value.py().import("pandas")?;
    if !value.is_instance(&pd.getattr("Series")?)? {
        return Ok(value.clone());
    }
    let name = value.getattr("name")?;
    let label: String = if name.is_none() {
        "asset".to_owned()
    } else {
        name.str()?.extract()?
    };
    value.call_method1("to_frame", (label,))
}

type PerformanceBuilder = fn(
    Vec<finstack_quant_core::dates::Date>,
    Vec<Vec<f64>>,
    Vec<String>,
    Option<&str>,
    PeriodKind,
) -> finstack_quant_core::Result<Performance>;

/// Build a `Performance` from pre-extracted panel arrays using the given Rust constructor.
fn build_performance_with(
    py: Python<'_>,
    dates: Vec<finstack_quant_core::dates::Date>,
    columns: Vec<Vec<f64>>,
    ticker_names: Vec<String>,
    benchmark_ticker: Option<&str>,
    frequency: Option<&str>,
    ctor: PerformanceBuilder,
) -> PyResult<PyPerformance> {
    let period_kind = parse_frequency(frequency, DEFAULT_FREQUENCY)?;
    let inner = py
        .detach(|| ctor(dates, columns, ticker_names, benchmark_ticker, period_kind))
        .map_err(analytics_to_py)?;
    Ok(PyPerformance { inner })
}

#[doc = pydoc!("finstack_quant.analytics", "Performance")]
#[pyclass(name = "Performance", module = "finstack_quant.analytics")]
pub(super) struct PyPerformance {
    inner: Performance,
}

impl PyPerformance {
    /// Resolve a ``ticker_idx`` argument given as an ``int`` column index or a
    /// ``str`` ticker name. Names are resolved in Rust
    /// (`Performance::get_ticker_index`); a miss raises ``KeyError``.
    fn resolve_ticker(&self, ticker: &Bound<'_, PyAny>) -> PyResult<usize> {
        if let Ok(name) = ticker.extract::<String>() {
            return self.inner.get_ticker_index(&name).map_err(analytics_to_py);
        }
        ticker.extract::<usize>().map_err(|_| {
            PyTypeError::new_err("ticker_idx must be an int column index or a str ticker name")
        })
    }
}

#[pymethods]
impl PyPerformance {
    #[new]
    #[pyo3(signature = (prices, benchmark_ticker=None, frequency=None))]
    fn new(
        py: Python<'_>,
        prices: Bound<'_, PyAny>,
        benchmark_ticker: Option<&str>,
        frequency: Option<&str>,
    ) -> PyResult<Self> {
        let prices = series_to_frame(&prices)?;
        let panel = extract_dataframe_panel(
            &prices,
            "Expected a pandas DataFrame or Series; use Performance.from_arrays() for raw lists",
        )?;
        build_performance_with(
            py,
            panel.dates,
            panel.columns,
            panel.ticker_names,
            benchmark_ticker,
            frequency,
            Performance::new,
        )
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.from_arrays")]
    #[staticmethod]
    #[pyo3(signature = (dates, prices, ticker_names, benchmark_ticker=None, frequency=None))]
    fn from_arrays(
        py: Python<'_>,
        dates: Vec<Bound<'_, PyAny>>,
        prices: Vec<Vec<f64>>,
        ticker_names: Vec<String>,
        benchmark_ticker: Option<&str>,
        frequency: Option<&str>,
    ) -> PyResult<Self> {
        let rust_dates = dates.iter().map(py_to_date).collect::<PyResult<Vec<_>>>()?;
        build_performance_with(
            py,
            rust_dates,
            prices,
            ticker_names,
            benchmark_ticker,
            frequency,
            Performance::new,
        )
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.from_returns")]
    #[staticmethod]
    #[pyo3(signature = (returns, benchmark_ticker=None, frequency=None))]
    fn from_returns(
        py: Python<'_>,
        returns: Bound<'_, PyAny>,
        benchmark_ticker: Option<&str>,
        frequency: Option<&str>,
    ) -> PyResult<Self> {
        let returns = series_to_frame(&returns)?;
        let panel = extract_dataframe_panel(
            &returns,
            "Expected a pandas DataFrame or Series; use Performance.from_returns_arrays() for raw lists",
        )?;
        build_performance_with(
            py,
            panel.dates,
            panel.columns,
            panel.ticker_names,
            benchmark_ticker,
            frequency,
            Performance::from_returns,
        )
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.from_returns_arrays")]
    #[staticmethod]
    #[pyo3(signature = (dates, returns, ticker_names, benchmark_ticker=None, frequency=None))]
    fn from_returns_arrays(
        py: Python<'_>,
        dates: Vec<Bound<'_, PyAny>>,
        returns: Vec<Vec<f64>>,
        ticker_names: Vec<String>,
        benchmark_ticker: Option<&str>,
        frequency: Option<&str>,
    ) -> PyResult<Self> {
        let rust_dates = dates.iter().map(py_to_date).collect::<PyResult<Vec<_>>>()?;
        build_performance_with(
            py,
            rust_dates,
            returns,
            ticker_names,
            benchmark_ticker,
            frequency,
            Performance::from_returns,
        )
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.reset_date_range")]
    fn reset_date_range(&mut self, start: Bound<'_, PyAny>, end: Bound<'_, PyAny>) -> PyResult<()> {
        let s = py_to_date(&start)?;
        let e = py_to_date(&end)?;
        self.inner.reset_date_range(s, e);
        Ok(())
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.reset_bench_ticker")]
    fn reset_bench_ticker(&mut self, ticker: &str) -> PyResult<()> {
        self.inner
            .reset_bench_ticker(ticker)
            .map_err(analytics_to_py)
    }

    // -- Getters --

    #[doc = pydoc!("finstack_quant.analytics", "Performance.get_ticker_index")]
    fn get_ticker_index(&self, ticker: &str) -> PyResult<usize> {
        self.inner.get_ticker_index(ticker).map_err(analytics_to_py)
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.get_ticker_names")]
    fn get_ticker_names(&self) -> Vec<String> {
        self.inner.get_ticker_names().to_vec()
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.get_benchmark_idx")]
    fn get_benchmark_idx(&self) -> usize {
        self.inner.get_benchmark_idx()
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.get_frequency")]
    fn get_frequency(&self) -> String {
        self.inner.get_frequency().to_string()
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.get_dates")]
    fn get_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        dates_to_pylist(py, self.inner.get_dates())
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.get_active_dates")]
    fn get_active_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        dates_to_pylist(py, self.inner.get_active_dates())
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.get_active_dates_for_ticker")]
    fn get_active_dates_for_ticker<'py>(
        &self,
        py: Python<'py>,
        ticker_idx: &Bound<'_, PyAny>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        dates_to_pylist(
            py,
            self.inner
                .get_active_dates_for_ticker(ticker_idx)
                .map_err(analytics_to_py)?,
        )
    }

    // -- Scalar-per-ticker methods --

    #[doc = pydoc!("finstack_quant.analytics", "Performance.cagr")]
    #[pyo3(signature = (day_count = None, calendar_id = None))]
    fn cagr<'py>(
        &self,
        py: Python<'py>,
        day_count: Option<&Bound<'_, PyAny>>,
        calendar_id: Option<&str>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let day_count = parse_cagr_day_count(day_count)?;
        let calendar = resolve_optional_calendar(calendar_id)?;
        let values = self
            .inner
            .cagr(day_count, calendar)
            .map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "cagr")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.mean_return")]
    #[pyo3(signature = (annualize = DEFAULT_ANNUALIZE))]
    fn mean_return<'py>(&self, py: Python<'py>, annualize: bool) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.mean_return(annualize);
        values_to_series(py, values, self.inner.get_ticker_names(), "mean_return")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.volatility")]
    #[pyo3(signature = (annualize = DEFAULT_ANNUALIZE))]
    fn volatility<'py>(&self, py: Python<'py>, annualize: bool) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.volatility(annualize);
        values_to_series(py, values, self.inner.get_ticker_names(), "volatility")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.sharpe")]
    #[pyo3(signature = (risk_free_rate = DEFAULT_RISK_FREE_RATE))]
    fn sharpe<'py>(&self, py: Python<'py>, risk_free_rate: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.sharpe(risk_free_rate);
        values_to_series(py, values, self.inner.get_ticker_names(), "sharpe")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.sortino")]
    #[pyo3(signature = (mar = DEFAULT_MAR))]
    fn sortino<'py>(&self, py: Python<'py>, mar: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.sortino(mar);
        values_to_series(py, values, self.inner.get_ticker_names(), "sortino")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.calmar")]
    fn calmar<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.calmar().map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "calmar")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.max_drawdown")]
    fn max_drawdown<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.max_drawdown();
        values_to_series(py, values, self.inner.get_ticker_names(), "max_drawdown")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.mean_drawdown")]
    fn mean_drawdown<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.mean_drawdown();
        values_to_series(py, values, self.inner.get_ticker_names(), "mean_drawdown")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.value_at_risk")]
    #[pyo3(signature = (confidence = DEFAULT_CONFIDENCE))]
    fn value_at_risk<'py>(&self, py: Python<'py>, confidence: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .value_at_risk(confidence)
            .map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "value_at_risk")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.expected_shortfall")]
    #[pyo3(signature = (confidence = DEFAULT_CONFIDENCE))]
    fn expected_shortfall<'py>(
        &self,
        py: Python<'py>,
        confidence: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .expected_shortfall(confidence)
            .map_err(analytics_to_py)?;
        values_to_series(
            py,
            values,
            self.inner.get_ticker_names(),
            "expected_shortfall",
        )
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.tracking_error")]
    fn tracking_error<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.tracking_error();
        values_to_series(py, values, self.inner.get_ticker_names(), "tracking_error")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.information_ratio")]
    fn information_ratio<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.information_ratio();
        values_to_series(
            py,
            values,
            self.inner.get_ticker_names(),
            "information_ratio",
        )
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.skewness")]
    fn skewness<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.skewness();
        values_to_series(py, values, self.inner.get_ticker_names(), "skewness")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.kurtosis")]
    fn kurtosis<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.kurtosis();
        values_to_series(py, values, self.inner.get_ticker_names(), "kurtosis")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.geometric_mean")]
    fn geometric_mean<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.geometric_mean();
        values_to_series(py, values, self.inner.get_ticker_names(), "geometric_mean")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.downside_deviation")]
    #[pyo3(signature = (mar = DEFAULT_MAR))]
    fn downside_deviation<'py>(&self, py: Python<'py>, mar: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.downside_deviation(mar);
        values_to_series(
            py,
            values,
            self.inner.get_ticker_names(),
            "downside_deviation",
        )
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.max_drawdown_duration")]
    fn max_drawdown_duration<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.max_drawdown_duration();
        int_values_to_series(
            py,
            values,
            self.inner.get_ticker_names(),
            "max_drawdown_duration",
        )
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.up_capture")]
    fn up_capture<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.up_capture();
        values_to_series(py, values, self.inner.get_ticker_names(), "up_capture")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.down_capture")]
    fn down_capture<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.down_capture();
        values_to_series(py, values, self.inner.get_ticker_names(), "down_capture")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.capture_ratio")]
    fn capture_ratio<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.capture_ratio();
        values_to_series(py, values, self.inner.get_ticker_names(), "capture_ratio")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.omega_ratio")]
    #[pyo3(signature = (threshold = DEFAULT_MAR))]
    fn omega_ratio<'py>(&self, py: Python<'py>, threshold: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.omega_ratio(threshold);
        values_to_series(py, values, self.inner.get_ticker_names(), "omega_ratio")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.treynor")]
    #[pyo3(signature = (risk_free_rate = DEFAULT_RISK_FREE_RATE))]
    fn treynor<'py>(&self, py: Python<'py>, risk_free_rate: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.treynor(risk_free_rate);
        values_to_series(py, values, self.inner.get_ticker_names(), "treynor")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.gain_to_pain")]
    fn gain_to_pain<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.gain_to_pain();
        values_to_series(py, values, self.inner.get_ticker_names(), "gain_to_pain")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.ulcer_index")]
    fn ulcer_index<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.ulcer_index();
        values_to_series(py, values, self.inner.get_ticker_names(), "ulcer_index")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.martin_ratio")]
    fn martin_ratio<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.martin_ratio().map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "martin_ratio")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.recovery_factor")]
    fn recovery_factor<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.recovery_factor();
        values_to_series(py, values, self.inner.get_ticker_names(), "recovery_factor")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.pain_index")]
    fn pain_index<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.pain_index();
        values_to_series(py, values, self.inner.get_ticker_names(), "pain_index")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.pain_ratio")]
    #[pyo3(signature = (risk_free_rate = DEFAULT_RISK_FREE_RATE))]
    fn pain_ratio<'py>(&self, py: Python<'py>, risk_free_rate: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .pain_ratio(risk_free_rate)
            .map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "pain_ratio")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.tail_ratio")]
    #[pyo3(signature = (confidence = DEFAULT_CONFIDENCE))]
    fn tail_ratio<'py>(&self, py: Python<'py>, confidence: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.tail_ratio(confidence).map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "tail_ratio")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.r_squared")]
    fn r_squared<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.r_squared();
        values_to_series(py, values, self.inner.get_ticker_names(), "r_squared")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.batting_average")]
    fn batting_average<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.batting_average();
        values_to_series(py, values, self.inner.get_ticker_names(), "batting_average")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.parametric_var")]
    #[pyo3(signature = (confidence = DEFAULT_CONFIDENCE, horizon_periods = None))]
    fn parametric_var<'py>(
        &self,
        py: Python<'py>,
        confidence: f64,
        horizon_periods: Option<f64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .parametric_var(confidence, horizon_periods)
            .map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "parametric_var")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.cornish_fisher_var")]
    #[pyo3(signature = (confidence = DEFAULT_CONFIDENCE, horizon_periods = None))]
    fn cornish_fisher_var<'py>(
        &self,
        py: Python<'py>,
        confidence: f64,
        horizon_periods: Option<f64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .cornish_fisher_var(confidence, horizon_periods)
            .map_err(analytics_to_py)?;
        values_to_series(
            py,
            values,
            self.inner.get_ticker_names(),
            "cornish_fisher_var",
        )
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.cdar")]
    #[pyo3(signature = (confidence = DEFAULT_CONFIDENCE))]
    fn cdar<'py>(&self, py: Python<'py>, confidence: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.cdar(confidence).map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "cdar")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.m_squared")]
    #[pyo3(signature = (risk_free_rate = DEFAULT_RISK_FREE_RATE))]
    fn m_squared<'py>(&self, py: Python<'py>, risk_free_rate: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.m_squared(risk_free_rate);
        values_to_series(py, values, self.inner.get_ticker_names(), "m_squared")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.modified_sharpe")]
    #[pyo3(signature = (risk_free_rate = DEFAULT_RISK_FREE_RATE, confidence = DEFAULT_CONFIDENCE))]
    fn modified_sharpe<'py>(
        &self,
        py: Python<'py>,
        risk_free_rate: f64,
        confidence: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .modified_sharpe(risk_free_rate, confidence)
            .map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "modified_sharpe")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.sterling_ratio")]
    #[pyo3(signature = (risk_free_rate = DEFAULT_RISK_FREE_RATE, n = DEFAULT_DRAWDOWN_COUNT))]
    fn sterling_ratio<'py>(
        &self,
        py: Python<'py>,
        risk_free_rate: f64,
        n: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .sterling_ratio(risk_free_rate, n)
            .map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "sterling_ratio")
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.burke_ratio")]
    #[pyo3(signature = (risk_free_rate = DEFAULT_RISK_FREE_RATE, n = DEFAULT_DRAWDOWN_COUNT))]
    fn burke_ratio<'py>(
        &self,
        py: Python<'py>,
        risk_free_rate: f64,
        n: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .burke_ratio(risk_free_rate, n)
            .map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "burke_ratio")
    }

    // -- Vector-per-ticker methods --

    #[doc = pydoc!("finstack_quant.analytics", "Performance.get_returns")]
    fn get_returns(&self) -> Vec<Vec<f64>> {
        self.inner.get_returns()
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.get_returns_for_ticker")]
    fn get_returns_for_ticker(&self, ticker_idx: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        self.inner
            .get_returns_for_ticker(ticker_idx)
            .map_err(analytics_to_py)
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.cumulative_returns")]
    fn cumulative_returns(&self) -> Vec<Vec<f64>> {
        self.inner.cumulative_returns()
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.periodic_returns")]
    #[pyo3(signature = (frequency = None))]
    fn periodic_returns<'py>(
        &self,
        py: Python<'py>,
        frequency: Option<&str>,
    ) -> PyResult<PyPeriodicReturnPanel<'py>> {
        let kind = parse_frequency(frequency, DEFAULT_PERIODIC_FREQUENCY)?;
        let panel = self.inner.periodic_returns(kind).map_err(analytics_to_py)?;
        panel
            .into_iter()
            .map(|series| {
                series
                    .into_iter()
                    .map(|point| Ok((date_to_py(py, point.date)?, point.value)))
                    .collect()
            })
            .collect()
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.get_drawdown_series")]
    fn get_drawdown_series(&self) -> Vec<Vec<f64>> {
        self.inner.get_drawdown_series()
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.correlation_matrix")]
    fn correlation_matrix(&self, py: Python<'_>) -> PyResult<(Vec<Vec<f64>>, bool)> {
        py.detach(|| self.inner.correlation_matrix().map_err(analytics_to_py))
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.cumulative_returns_outperformance")]
    fn cumulative_returns_outperformance(&self) -> Vec<Vec<f64>> {
        self.inner.cumulative_returns_outperformance()
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.drawdown_difference")]
    fn drawdown_difference(&self) -> Vec<Vec<f64>> {
        self.inner.drawdown_difference()
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.excess_returns")]
    #[pyo3(signature = (rf, nperiods = None))]
    fn excess_returns(
        &self,
        rf: &Bound<'_, PyAny>,
        nperiods: Option<f64>,
    ) -> PyResult<Vec<Vec<f64>>> {
        let rf = extract_f64_vec(rf, "rf")?;
        self.inner
            .excess_returns(&rf, nperiods)
            .map_err(analytics_to_py)
    }

    // -- Per-ticker indexed methods --

    #[doc = pydoc!("finstack_quant.analytics", "Performance.beta")]
    fn beta(&self) -> Vec<PyBetaResult> {
        self.inner
            .beta()
            .into_iter()
            .map(|b| PyBetaResult { inner: b })
            .collect()
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.greeks")]
    #[pyo3(signature = (risk_free_rate = DEFAULT_RISK_FREE_RATE))]
    fn greeks(&self, risk_free_rate: f64) -> Vec<PyGreeksResult> {
        self.inner
            .greeks(risk_free_rate)
            .into_iter()
            .map(|g| PyGreeksResult { inner: g })
            .collect()
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.rolling_greeks")]
    #[pyo3(signature = (ticker_idx, window = DEFAULT_ROLLING_WINDOW, risk_free_rate = DEFAULT_RISK_FREE_RATE))]
    fn rolling_greeks(
        &self,
        py: Python<'_>,
        ticker_idx: &Bound<'_, PyAny>,
        window: usize,
        risk_free_rate: f64,
    ) -> PyResult<PyRollingGreeks> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        let inner = py
            .detach(|| {
                self.inner
                    .rolling_greeks(ticker_idx, window, risk_free_rate)
            })
            .map_err(analytics_to_py)?;
        Ok(PyRollingGreeks { inner })
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.rolling_volatility")]
    #[pyo3(signature = (ticker_idx, window = DEFAULT_ROLLING_WINDOW))]
    fn rolling_volatility(
        &self,
        py: Python<'_>,
        ticker_idx: &Bound<'_, PyAny>,
        window: usize,
    ) -> PyResult<PyDatedSeries> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        let inner = py
            .detach(|| self.inner.rolling_volatility(ticker_idx, window))
            .map_err(analytics_to_py)?;
        Ok(PyDatedSeries { inner })
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.rolling_sortino")]
    #[pyo3(signature = (ticker_idx, window = DEFAULT_ROLLING_WINDOW, mar = DEFAULT_MAR))]
    fn rolling_sortino(
        &self,
        py: Python<'_>,
        ticker_idx: &Bound<'_, PyAny>,
        window: usize,
        mar: f64,
    ) -> PyResult<PyDatedSeries> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        let inner = py
            .detach(|| self.inner.rolling_sortino(ticker_idx, window, mar))
            .map_err(analytics_to_py)?;
        Ok(PyDatedSeries { inner })
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.rolling_sharpe")]
    #[pyo3(signature = (ticker_idx, window = DEFAULT_ROLLING_WINDOW, risk_free_rate = DEFAULT_RISK_FREE_RATE))]
    fn rolling_sharpe(
        &self,
        py: Python<'_>,
        ticker_idx: &Bound<'_, PyAny>,
        window: usize,
        risk_free_rate: f64,
    ) -> PyResult<PyDatedSeries> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        let inner = py
            .detach(|| {
                self.inner
                    .rolling_sharpe(ticker_idx, window, risk_free_rate)
            })
            .map_err(analytics_to_py)?;
        Ok(PyDatedSeries { inner })
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.drawdown_details")]
    #[pyo3(signature = (ticker_idx, n = DEFAULT_DRAWDOWN_COUNT))]
    fn drawdown_details(
        &self,
        ticker_idx: &Bound<'_, PyAny>,
        n: usize,
    ) -> PyResult<Vec<PyDrawdownEpisode>> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        Ok(self
            .inner
            .drawdown_details(ticker_idx, n)
            .map_err(analytics_to_py)?
            .into_iter()
            .map(|e| PyDrawdownEpisode { inner: e })
            .collect())
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.multi_factor_greeks")]
    #[pyo3(signature = (ticker_idx, factor_returns, return_kind = None, risk_free_rate = DEFAULT_RISK_FREE_RATE))]
    fn multi_factor_greeks(
        &self,
        py: Python<'_>,
        ticker_idx: &Bound<'_, PyAny>,
        factor_returns: Vec<Vec<f64>>,
        return_kind: Option<&str>,
        risk_free_rate: f64,
    ) -> PyResult<PyMultiFactorResult> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        let refs: Vec<&[f64]> = factor_returns.iter().map(|v| v.as_slice()).collect();
        let kind = ReturnKind::from_label(return_kind, risk_free_rate).map_err(analytics_to_py)?;
        py.detach(|| self.inner.multi_factor_greeks(ticker_idx, &refs, kind))
            .map(|r| PyMultiFactorResult { inner: r })
            .map_err(analytics_to_py)
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.rolling_returns")]
    #[pyo3(signature = (ticker_idx, window))]
    fn rolling_returns(
        &self,
        py: Python<'_>,
        ticker_idx: &Bound<'_, PyAny>,
        window: usize,
    ) -> PyResult<PyDatedSeries> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        let inner = py
            .detach(|| self.inner.rolling_returns(ticker_idx, window))
            .map_err(analytics_to_py)?;
        Ok(PyDatedSeries { inner })
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.lookback_returns")]
    #[pyo3(signature = (ref_date, fiscal_config = None))]
    fn lookback_returns(
        &self,
        ref_date: Bound<'_, PyAny>,
        fiscal_config: Option<&PyFiscalConfig>,
    ) -> PyResult<PyLookbackReturns> {
        let d = py_to_date(&ref_date)?;
        Ok(PyLookbackReturns {
            inner: self
                .inner
                .lookback_returns(d, fiscal_config.map(|config| config.inner)),
        })
    }

    #[doc = pydoc!("finstack_quant.analytics", "Performance.period_stats")]
    #[pyo3(signature = (ticker_idx, aggregation_frequency = None, fiscal_config = None))]
    fn period_stats(
        &self,
        ticker_idx: &Bound<'_, PyAny>,
        aggregation_frequency: Option<&str>,
        fiscal_config: Option<&PyFiscalConfig>,
    ) -> PyResult<PyPeriodStats> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        let pk = parse_frequency(aggregation_frequency, DEFAULT_PERIODIC_FREQUENCY)?;
        Ok(PyPeriodStats {
            inner: self
                .inner
                .period_stats(ticker_idx, pk, fiscal_config.map(|config| config.inner))
                .map_err(analytics_to_py)?,
        })
    }

    // -- DataFrame export --

    #[doc = pydoc!("finstack_quant.analytics", "Performance.to_dataframe")]
    #[pyo3(signature = (risk_free_rate = DEFAULT_RISK_FREE_RATE, confidence = DEFAULT_CONFIDENCE))]
    fn to_dataframe<'py>(
        &self,
        py: Python<'py>,
        risk_free_rate: f64,
        confidence: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        // The 22-metric table is assembled in Rust (`Performance::summary`),
        // so pandas and JS read the same rows; release the GIL for the
        // O(22 * n * m) walk.
        let table = py
            .detach(|| self.inner.summary(risk_free_rate, confidence))
            .map_err(analytics_to_py)?;
        envelope_to_pandas(py, &table)?.call_method1("set_index", ("ticker",))
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Returns a compact summary of the serialized fields, with collections
    /// summarized by length. Use accessors such as :meth:`get_ticker_names`,
    /// :meth:`get_dates`, and :meth:`get_active_dates`, or a DataFrame exit such as
    /// :meth:`to_dataframe`, when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("Performance", &self.inner)
    }
}

wire_methods!(
    PyPerformance,
    Performance,
    "Performance",
    module = "finstack_quant.analytics"
);

#[doc = pydoc!("finstack_quant.analytics", "sharpe")]
#[pyfunction]
#[pyo3(signature = (returns, risk_free_rate = DEFAULT_RISK_FREE_RATE, periods_per_year = DEFAULT_PERIODS_PER_YEAR))]
fn sharpe(returns: &Bound<'_, PyAny>, risk_free_rate: f64, periods_per_year: f64) -> PyResult<f64> {
    let returns = extract_f64_vec(returns, "returns")?;
    Ok(finstack_quant_analytics::sharpe(
        &returns,
        risk_free_rate,
        periods_per_year,
    ))
}

#[doc = pydoc!("finstack_quant.analytics", "sortino")]
#[pyfunction]
#[pyo3(signature = (returns, mar = DEFAULT_MAR, periods_per_year = DEFAULT_PERIODS_PER_YEAR))]
fn sortino(returns: &Bound<'_, PyAny>, mar: f64, periods_per_year: f64) -> PyResult<f64> {
    let returns = extract_f64_vec(returns, "returns")?;
    Ok(finstack_quant_analytics::sortino(
        &returns,
        mar,
        periods_per_year,
    ))
}

#[doc = pydoc!("finstack_quant.analytics", "volatility")]
#[pyfunction]
#[pyo3(signature = (returns, periods_per_year = DEFAULT_PERIODS_PER_YEAR))]
fn volatility(returns: &Bound<'_, PyAny>, periods_per_year: f64) -> PyResult<f64> {
    let returns = extract_f64_vec(returns, "returns")?;
    Ok(finstack_quant_analytics::volatility(
        &returns,
        periods_per_year,
    ))
}

#[doc = pydoc!("finstack_quant.analytics", "max_drawdown")]
#[pyfunction]
fn max_drawdown(returns: &Bound<'_, PyAny>) -> PyResult<f64> {
    let returns = extract_f64_vec(returns, "returns")?;
    Ok(finstack_quant_analytics::max_drawdown(&returns))
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPerformance>()?;
    m.add_function(wrap_pyfunction!(sharpe, m)?)?;
    m.add_function(wrap_pyfunction!(sortino, m)?)?;
    m.add_function(wrap_pyfunction!(volatility, m)?)?;
    m.add_function(wrap_pyfunction!(max_drawdown, m)?)?;
    Ok(())
}
