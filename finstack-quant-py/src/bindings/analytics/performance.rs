//! Stateful [`Performance`] analytics engine.

use super::types::*;
use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::date_utils::{date_to_py, extract_date, py_to_date};
use crate::bindings::pandas_utils::{
    dates_to_datetime_index, dict_to_dataframe, int_values_to_series,
    serde_rows_to_dataframe_with_schema, table_to_dataframe, values_to_series, ColumnSchema,
};
use crate::errors::analytics_to_py as core_to_py;
use crate::errors::display_to_py;
use finstack_quant_analytics as fa;
use finstack_quant_core::dates::{calendar_by_id, FiscalConfig, HolidayCalendar, PeriodKind};
use numpy::PyArray1;
use pyo3::buffer::PyBuffer;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

type PyPeriodicReturnPanel<'py> = Vec<Vec<(Bound<'py, PyAny>, f64)>>;

/// Wrap a `Vec<f64>` as a NumPy `float64` array, taking ownership of the
/// buffer so no per-element `PyFloat` boxing occurs.
fn vec_to_pyarray<'py>(py: Python<'py>, values: Vec<f64>) -> Bound<'py, PyArray1<f64>> {
    PyArray1::from_vec(py, values)
}

/// Wrap a borrowed `&[f64]` as a NumPy `float64` array (one copy).
fn slice_to_pyarray<'py>(py: Python<'py>, values: &[f64]) -> Bound<'py, PyArray1<f64>> {
    PyArray1::from_slice(py, values)
}

/// Resolve optional fiscal-year-start month/day into a [`FiscalConfig`].
///
/// Returns `None` when both are omitted so the Rust default (calendar year)
/// applies; a partially specified start fills the other half with `1`.
fn make_fiscal_config(month: Option<u8>, day: Option<u8>) -> PyResult<Option<FiscalConfig>> {
    if month.is_none() && day.is_none() {
        return Ok(None);
    }
    FiscalConfig::new(month.unwrap_or(1), day.unwrap_or(1))
        .map(Some)
        .map_err(core_to_py)
}

/// Fiscal config for lookback returns, which always need one: the Rust
/// default is a January-1 fiscal year.
fn lookback_fiscal_config(month: Option<u8>, day: Option<u8>) -> PyResult<FiscalConfig> {
    match make_fiscal_config(month, day)? {
        Some(config) => Ok(config),
        None => FiscalConfig::new(1, 1).map_err(core_to_py),
    }
}

fn resolve_fiscal_calendar(calendar_id: &str) -> PyResult<&'static dyn HolidayCalendar> {
    calendar_by_id(calendar_id).ok_or_else(|| {
        core_to_py(
            finstack_quant_core::Error::calendar_not_found_with_suggestions(
                calendar_id.to_string(),
                finstack_quant_core::dates::available_calendars(),
            ),
        )
    })
}

fn parse_cagr_day_count(day_count: Option<&Bound<'_, PyAny>>) -> PyResult<fa::CagrDayCount> {
    let Some(value) = day_count else {
        return Ok(fa::CagrDayCount::Act365_25);
    };
    if value.is_none() {
        return Ok(fa::CagrDayCount::Act365_25);
    }
    if let Ok(day_count) = value.extract::<PyRef<'_, PyDayCount>>() {
        return Ok(fa::CagrDayCount::DayCount(day_count.inner));
    }
    if let Ok(label) = value.extract::<String>() {
        return label.parse::<fa::CagrDayCount>().map_err(core_to_py);
    }
    Err(PyTypeError::new_err(
        "day_count must be None, 'act365_25', a DayCount name such as 'act_365f', or a DayCount",
    ))
}

fn resolve_optional_calendar(
    calendar_id: Option<&str>,
) -> PyResult<Option<&'static dyn HolidayCalendar>> {
    calendar_id.map(resolve_fiscal_calendar).transpose()
}

fn parse_return_kind(return_kind: &str, risk_free_rate: f64) -> PyResult<fa::ReturnKind> {
    return_kind
        .parse::<fa::ReturnKind>()
        .map(|kind| kind.with_risk_free_rate(risk_free_rate))
        .map_err(core_to_py)
}

/// Parse a frequency token into a [`PeriodKind`].
///
/// Accepts the canonical tokens (`daily`, `weekly`, `monthly`, `quarterly`,
/// `semi_annual`, `annual`) plus the pandas offset aliases `D`/`B`, `W`,
/// `M`/`ME`, `Q`/`QE`, `A`/`Y`/`YE`; the descriptive error comes from core.
fn parse_frequency(frequency: &str) -> PyResult<PeriodKind> {
    frequency.parse::<PeriodKind>().map_err(core_to_py)
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

/// Column schema of `Performance.to_beta_dataframe`.
const BETA_COLUMNS: &[ColumnSchema<'static>] = &[
    ("ticker", "str"),
    ("beta", "float64"),
    ("std_err", "float64"),
    ("ci_lower", "float64"),
    ("ci_upper", "float64"),
];

/// Column schema of `Performance.to_greeks_dataframe`.
const GREEKS_COLUMNS: &[ColumnSchema<'static>] = &[
    ("ticker", "str"),
    ("alpha", "float64"),
    ("beta", "float64"),
    ("r_squared", "float64"),
    ("adjusted_r_squared", "float64"),
];

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

/// Build a `Performance` from pre-extracted arrays.
fn build_performance(
    py: Python<'_>,
    dates: Vec<time::Date>,
    prices: Vec<Vec<f64>>,
    ticker_names: Vec<String>,
    benchmark_ticker: Option<&str>,
    frequency: &str,
) -> PyResult<PyPerformance> {
    let period_kind = parse_frequency(frequency)?;
    let inner = py
        .detach(|| fa::Performance::new(dates, prices, ticker_names, benchmark_ticker, period_kind))
        .map_err(core_to_py)?;
    Ok(PyPerformance { inner })
}

/// Build a `Performance` from pre-extracted return arrays.
fn build_returns_performance(
    py: Python<'_>,
    dates: Vec<time::Date>,
    returns: Vec<Vec<f64>>,
    ticker_names: Vec<String>,
    benchmark_ticker: Option<&str>,
    frequency: &str,
) -> PyResult<PyPerformance> {
    let period_kind = parse_frequency(frequency)?;
    let inner = py
        .detach(|| {
            fa::Performance::from_returns(
                dates,
                returns,
                ticker_names,
                benchmark_ticker,
                period_kind,
            )
        })
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
    let (dates, columns) = perf.aligned_panel(panel).map_err(core_to_py)?;
    aligned_columns_to_dataframe(py, perf, &dates, columns)
}

/// Calendar-bucketed returns on the union of period-end dates, one column per
/// ticker.
fn periodic_panel_to_dataframe<'py>(
    py: Python<'py>,
    perf: &fa::Performance,
    frequency: PeriodKind,
) -> PyResult<Bound<'py, PyAny>> {
    let (dates, columns) = perf.periodic_returns_aligned(frequency);
    aligned_columns_to_dataframe(py, perf, &dates, columns)
}

/// Build a date-indexed frame from Rust-aligned per-ticker columns.
fn aligned_columns_to_dataframe<'py>(
    py: Python<'py>,
    perf: &fa::Performance,
    dates: &[time::Date],
    columns: Vec<Vec<f64>>,
) -> PyResult<Bound<'py, PyAny>> {
    let data = PyDict::new(py);
    for (name, padded) in perf.ticker_names().iter().zip(columns) {
        data.set_item(name, vec_to_pyarray(py, padded))?;
    }
    let idx = dates_to_datetime_index(py, dates)?;
    dict_to_dataframe(py, &data, Some(idx))
}

/// Stateful performance analytics engine over a panel of ticker price series.
///
/// Accepts a pandas ``DataFrame`` where the index contains dates and each
/// column is a price series for one ticker.
///
/// Scalar-per-ticker metrics return a ``pandas.Series`` indexed by ticker name
/// and named after the metric, so results are selected by label rather than by
/// column position, and ``pd.concat([perf.sharpe(), perf.sortino()], axis=1)``
/// yields correctly-named columns.
#[pyclass(name = "Performance", module = "finstack_quant.analytics")]
pub(super) struct PyPerformance {
    inner: fa::Performance,
}

impl PyPerformance {
    /// Resolve a ``ticker_idx`` argument given as an ``int`` column index or a
    /// ``str`` ticker name. Names are resolved in Rust
    /// (`Performance::ticker_index`); a miss raises ``KeyError``.
    fn resolve_ticker(&self, ticker: &Bound<'_, PyAny>) -> PyResult<usize> {
        if let Ok(name) = ticker.extract::<String>() {
            return self.inner.ticker_index(&name).map_err(core_to_py);
        }
        ticker.extract::<usize>().map_err(|_| {
            PyTypeError::new_err("ticker_idx must be an int column index or a str ticker name")
        })
    }

    fn lookback_returns_inner(
        &self,
        ref_date: time::Date,
        fiscal_year_start_month: Option<u8>,
        fiscal_year_start_day: Option<u8>,
    ) -> PyResult<fa::LookbackReturns> {
        let fc = lookback_fiscal_config(fiscal_year_start_month, fiscal_year_start_day)?;
        Ok(self.inner.lookback_returns(ref_date, fc))
    }
}

#[pymethods]
impl PyPerformance {
    /// Construct from a pandas DataFrame of prices.
    ///
    /// The DataFrame index must contain ``datetime.date`` or ``pd.Timestamp``
    /// values, and each column represents one ticker's price series.
    ///
    /// Parameters
    /// ----------
    /// prices : pandas.DataFrame
    ///     Price panel with a date-like index and one ``str`` column per ticker.
    /// benchmark_ticker : str, optional
    ///     Benchmark column name; defaults to the first column.
    /// frequency : str, default "daily"
    ///     Observation frequency: ``"daily"``, ``"weekly"``, ``"monthly"``,
    ///     ``"quarterly"``, ``"semi_annual"``, ``"annual"`` or a pandas offset
    ///     alias (``D``/``B``, ``W``, ``M``, ``Q``, ``A``/``Y``). Sets the
    ///     annualization factor (252, 52, 12, 4, 2, 1).
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``prices`` is not a DataFrame or its columns are not ``str``.
    /// AnalyticsError
    ///     If the panel is empty, dates are not strictly ascending, or
    ///     ``frequency`` is unknown.
    #[new]
    #[pyo3(signature = (prices, benchmark_ticker=None, frequency="daily"))]
    fn new(
        py: Python<'_>,
        prices: Bound<'_, PyAny>,
        benchmark_ticker: Option<&str>,
        frequency: &str,
    ) -> PyResult<Self> {
        let panel = extract_dataframe_panel(
            &prices,
            "Expected a pandas DataFrame; use Performance.from_arrays() for raw lists",
        )?;
        build_performance(
            py,
            panel.dates,
            panel.columns,
            panel.ticker_names,
            benchmark_ticker,
            frequency,
        )
    }

    /// Construct from raw arrays (dates, prices matrix, ticker names).
    #[staticmethod]
    #[pyo3(signature = (dates, prices, ticker_names, benchmark_ticker=None, frequency="daily"))]
    fn from_arrays(
        py: Python<'_>,
        dates: Vec<Bound<'_, PyAny>>,
        prices: Vec<Vec<f64>>,
        ticker_names: Vec<String>,
        benchmark_ticker: Option<&str>,
        frequency: &str,
    ) -> PyResult<Self> {
        let rust_dates = dates.iter().map(py_to_date).collect::<PyResult<Vec<_>>>()?;
        build_performance(
            py,
            rust_dates,
            prices,
            ticker_names,
            benchmark_ticker,
            frequency,
        )
    }

    /// Construct from a pandas DataFrame (or Series) of simple returns.
    ///
    /// The index must contain ``datetime.date`` or ``pd.Timestamp`` values,
    /// and each column represents one ticker's simple-return series aligned
    /// with the index. A ``pandas.Series`` is treated as a single-asset panel
    /// whose ticker is the series ``name`` (``"asset"`` when unnamed).
    #[staticmethod]
    #[pyo3(signature = (returns, benchmark_ticker=None, frequency="daily"))]
    fn from_returns(
        py: Python<'_>,
        returns: Bound<'_, PyAny>,
        benchmark_ticker: Option<&str>,
        frequency: &str,
    ) -> PyResult<Self> {
        let returns = series_to_frame(&returns)?;
        let panel = extract_dataframe_panel(
            &returns,
            "Expected a pandas DataFrame or Series; use Performance.from_returns_arrays() for raw lists",
        )?;
        build_returns_performance(
            py,
            panel.dates,
            panel.columns,
            panel.ticker_names,
            benchmark_ticker,
            frequency,
        )
    }

    /// Construct from raw return arrays (dates, returns matrix, ticker names).
    #[staticmethod]
    #[pyo3(signature = (dates, returns, ticker_names, benchmark_ticker=None, frequency="daily"))]
    fn from_returns_arrays(
        py: Python<'_>,
        dates: Vec<Bound<'_, PyAny>>,
        returns: Vec<Vec<f64>>,
        ticker_names: Vec<String>,
        benchmark_ticker: Option<&str>,
        frequency: &str,
    ) -> PyResult<Self> {
        let rust_dates = dates.iter().map(py_to_date).collect::<PyResult<Vec<_>>>()?;
        build_returns_performance(
            py,
            rust_dates,
            returns,
            ticker_names,
            benchmark_ticker,
            frequency,
        )
    }

    /// Restrict analytics to a date window.
    fn reset_date_range(&mut self, start: Bound<'_, PyAny>, end: Bound<'_, PyAny>) -> PyResult<()> {
        let s = extract_date(&start)?;
        let e = extract_date(&end)?;
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

    /// Observation frequency, as the canonical lowercase token (``"daily"``,
    /// ``"weekly"``, ``"monthly"``, ``"quarterly"``, ``"semi_annual"``,
    /// ``"annual"``) that round-trips through the ``frequency`` constructor
    /// argument and the :meth:`period_stats` ``aggregation_frequency``
    /// parameter. Inputs also accept the pandas offset aliases ``D``/``B``,
    /// ``W``, ``M``, ``Q``, ``A``/``Y``.
    #[getter]
    fn frequency(&self) -> String {
        self.inner.frequency().to_string()
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
        ticker_idx: &Bound<'_, PyAny>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        self.inner
            .active_dates_for_ticker(ticker_idx)
            .map_err(core_to_py)?
            .iter()
            .map(|&d| date_to_py(py, d))
            .collect()
    }

    // -- Scalar-per-ticker methods --

    /// CAGR for each ticker.
    ///
    /// ``day_count=None`` uses Act/365.25. Pass ``"act365_25"`` for the same
    /// default, a core DayCount name such as ``"act_365f"`` / ``"bus_252"``,
    /// or a :class:`~finstack_quant.core.dates.DayCount`. ``bus_252`` requires
    /// ``calendar_id``.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Compound annual growth rate indexed by ticker name.
    #[pyo3(signature = (day_count = None, calendar_id = None))]
    fn cagr<'py>(
        &self,
        py: Python<'py>,
        day_count: Option<&Bound<'_, PyAny>>,
        calendar_id: Option<&str>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let day_count = parse_cagr_day_count(day_count)?;
        let calendar = resolve_optional_calendar(calendar_id)?;
        let values = self.inner.cagr(day_count, calendar).map_err(core_to_py)?;
        values_to_series(py, values, self.inner.ticker_names(), "cagr")
    }

    /// Mean return for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Mean return indexed by ticker name.
    #[pyo3(signature = (annualize = true))]
    fn mean_return<'py>(&self, py: Python<'py>, annualize: bool) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.mean_return(annualize);
        values_to_series(py, values, self.inner.ticker_names(), "mean_return")
    }

    /// Volatility for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Standard deviation of returns indexed by ticker name.
    #[pyo3(signature = (annualize = true))]
    fn volatility<'py>(&self, py: Python<'py>, annualize: bool) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.volatility(annualize);
        values_to_series(py, values, self.inner.ticker_names(), "volatility")
    }

    /// Sharpe ratio for each ticker.
    ///
    /// ``risk_free_rate`` is an annualized decimal (``0.02`` for 2%),
    /// geometrically decompounded to the panel frequency before subtraction.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Sharpe ratio indexed by ticker name.
    #[pyo3(signature = (risk_free_rate = 0.0))]
    fn sharpe<'py>(&self, py: Python<'py>, risk_free_rate: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.sharpe(risk_free_rate);
        values_to_series(py, values, self.inner.ticker_names(), "sharpe")
    }

    /// Sortino ratio for each ticker.
    ///
    /// ``mar`` is a per-period minimum acceptable return, unlike Sharpe
    /// ``risk_free_rate`` inputs, which are annualized.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Sortino ratio indexed by ticker name.
    #[pyo3(signature = (mar = 0.0))]
    fn sortino<'py>(&self, py: Python<'py>, mar: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.sortino(mar);
        values_to_series(py, values, self.inner.ticker_names(), "sortino")
    }

    /// Calmar ratio for each ticker over the active window
    /// (CAGR / |max drawdown|), not Young's 36-month CTA definition.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Calmar ratio indexed by ticker name.
    fn calmar<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.calmar().map_err(core_to_py)?;
        values_to_series(py, values, self.inner.ticker_names(), "calmar")
    }

    /// Max drawdown for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Peak-to-trough drawdown (negative) indexed by ticker name.
    fn max_drawdown<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.max_drawdown();
        values_to_series(py, values, self.inner.ticker_names(), "max_drawdown")
    }

    /// Mean drawdown (path-weighted average) for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Average drawdown (negative) indexed by ticker name.
    fn mean_drawdown<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.mean_drawdown();
        values_to_series(py, values, self.inner.ticker_names(), "mean_drawdown")
    }

    /// Historical VaR for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Historical value at risk indexed by ticker name.
    #[pyo3(signature = (confidence = 0.95))]
    fn value_at_risk<'py>(&self, py: Python<'py>, confidence: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.value_at_risk(confidence).map_err(core_to_py)?;
        values_to_series(py, values, self.inner.ticker_names(), "value_at_risk")
    }

    /// Expected Shortfall for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Expected shortfall indexed by ticker name.
    #[pyo3(signature = (confidence = 0.95))]
    fn expected_shortfall<'py>(
        &self,
        py: Python<'py>,
        confidence: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .expected_shortfall(confidence)
            .map_err(core_to_py)?;
        values_to_series(py, values, self.inner.ticker_names(), "expected_shortfall")
    }

    /// Tracking error for each ticker vs benchmark.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Tracking error indexed by ticker name.
    fn tracking_error<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.tracking_error();
        values_to_series(py, values, self.inner.ticker_names(), "tracking_error")
    }

    /// Information ratio for each ticker vs benchmark.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Information ratio indexed by ticker name.
    fn information_ratio<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.information_ratio();
        values_to_series(py, values, self.inner.ticker_names(), "information_ratio")
    }

    /// Skewness for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Skewness indexed by ticker name.
    fn skewness<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.skewness();
        values_to_series(py, values, self.inner.ticker_names(), "skewness")
    }

    /// Kurtosis for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Kurtosis indexed by ticker name.
    fn kurtosis<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.kurtosis();
        values_to_series(py, values, self.inner.ticker_names(), "kurtosis")
    }

    /// Geometric mean for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Geometric mean return indexed by ticker name.
    fn geometric_mean<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.geometric_mean();
        values_to_series(py, values, self.inner.ticker_names(), "geometric_mean")
    }

    /// Downside deviation for each ticker.
    ///
    /// ``mar`` is a per-period threshold, unlike Sharpe ``risk_free_rate``
    /// inputs, which are annualized.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Downside deviation indexed by ticker name.
    #[pyo3(signature = (mar = 0.0))]
    fn downside_deviation<'py>(&self, py: Python<'py>, mar: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.downside_deviation(mar);
        values_to_series(py, values, self.inner.ticker_names(), "downside_deviation")
    }

    /// Max drawdown duration (calendar days) for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Integer-valued drawdown duration in calendar days, indexed by
    ///     ticker name.
    fn max_drawdown_duration<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.max_drawdown_duration();
        int_values_to_series(
            py,
            values,
            self.inner.ticker_names(),
            "max_drawdown_duration",
        )
    }

    /// Empyrical-style annualized geometric up-capture ratio vs benchmark.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Up-capture ratio indexed by ticker name.
    fn up_capture<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.up_capture();
        values_to_series(py, values, self.inner.ticker_names(), "up_capture")
    }

    /// Empyrical-style annualized geometric down-capture ratio vs benchmark.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Down-capture ratio indexed by ticker name.
    fn down_capture<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.down_capture();
        values_to_series(py, values, self.inner.ticker_names(), "down_capture")
    }

    /// Empyrical-style annualized geometric capture ratio vs benchmark.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Capture ratio indexed by ticker name.
    fn capture_ratio<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.capture_ratio();
        values_to_series(py, values, self.inner.ticker_names(), "capture_ratio")
    }

    /// Omega ratio for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Omega ratio indexed by ticker name.
    #[pyo3(signature = (threshold = 0.0))]
    fn omega_ratio<'py>(&self, py: Python<'py>, threshold: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.omega_ratio(threshold);
        values_to_series(py, values, self.inner.ticker_names(), "omega_ratio")
    }

    /// Treynor ratio for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Treynor ratio indexed by ticker name.
    #[pyo3(signature = (risk_free_rate = 0.0))]
    fn treynor<'py>(&self, py: Python<'py>, risk_free_rate: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.treynor(risk_free_rate);
        values_to_series(py, values, self.inner.ticker_names(), "treynor")
    }

    /// Gain-to-pain ratio for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Gain-to-pain ratio indexed by ticker name.
    fn gain_to_pain<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.gain_to_pain();
        values_to_series(py, values, self.inner.ticker_names(), "gain_to_pain")
    }

    /// Ulcer index for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Ulcer index indexed by ticker name.
    fn ulcer_index<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.ulcer_index();
        values_to_series(py, values, self.inner.ticker_names(), "ulcer_index")
    }

    /// Martin ratio for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Martin ratio indexed by ticker name.
    fn martin_ratio<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.martin_ratio().map_err(core_to_py)?;
        values_to_series(py, values, self.inner.ticker_names(), "martin_ratio")
    }

    /// Recovery factor for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Recovery factor indexed by ticker name.
    fn recovery_factor<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.recovery_factor();
        values_to_series(py, values, self.inner.ticker_names(), "recovery_factor")
    }

    /// Pain index for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Pain index indexed by ticker name.
    fn pain_index<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.pain_index();
        values_to_series(py, values, self.inner.ticker_names(), "pain_index")
    }

    /// Pain ratio for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Pain ratio indexed by ticker name.
    #[pyo3(signature = (risk_free_rate = 0.0))]
    fn pain_ratio<'py>(&self, py: Python<'py>, risk_free_rate: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.pain_ratio(risk_free_rate).map_err(core_to_py)?;
        values_to_series(py, values, self.inner.ticker_names(), "pain_ratio")
    }

    /// Tail ratio for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Tail ratio indexed by ticker name.
    #[pyo3(signature = (confidence = 0.95))]
    fn tail_ratio<'py>(&self, py: Python<'py>, confidence: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.tail_ratio(confidence).map_err(core_to_py)?;
        values_to_series(py, values, self.inner.ticker_names(), "tail_ratio")
    }

    /// R-squared for each ticker vs benchmark.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Coefficient of determination indexed by ticker name.
    fn r_squared<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.r_squared();
        values_to_series(py, values, self.inner.ticker_names(), "r_squared")
    }

    /// Batting average for each ticker vs benchmark.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Batting average indexed by ticker name.
    fn batting_average<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.batting_average();
        values_to_series(py, values, self.inner.ticker_names(), "batting_average")
    }

    /// Equal-weight Gaussian VaR for each ticker.
    ///
    /// ``horizon_periods=None`` is one-period VaR. Pass a positive count to
    /// scale mean by ``h`` and volatility by ``sqrt(h)``.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Parametric value at risk indexed by ticker name.
    #[pyo3(signature = (confidence = 0.95, horizon_periods = None))]
    fn parametric_var<'py>(
        &self,
        py: Python<'py>,
        confidence: f64,
        horizon_periods: Option<f64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .parametric_var(confidence, horizon_periods)
            .map_err(core_to_py)?;
        values_to_series(py, values, self.inner.ticker_names(), "parametric_var")
    }

    /// Cornish-Fisher VaR for each ticker.
    ///
    /// ``horizon_periods=None`` is one-period VaR. Pass a positive count to
    /// scale the Cornish–Fisher moments to that horizon.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Cornish-Fisher modified value at risk indexed by ticker name.
    #[pyo3(signature = (confidence = 0.95, horizon_periods = None))]
    fn cornish_fisher_var<'py>(
        &self,
        py: Python<'py>,
        confidence: f64,
        horizon_periods: Option<f64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .cornish_fisher_var(confidence, horizon_periods)
            .map_err(core_to_py)?;
        values_to_series(py, values, self.inner.ticker_names(), "cornish_fisher_var")
    }

    /// CDaR for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Conditional drawdown at risk indexed by ticker name.
    #[pyo3(signature = (confidence = 0.95))]
    fn cdar<'py>(&self, py: Python<'py>, confidence: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.cdar(confidence).map_err(core_to_py)?;
        values_to_series(py, values, self.inner.ticker_names(), "cdar")
    }

    /// M-squared for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     M-squared measure indexed by ticker name.
    #[pyo3(signature = (risk_free_rate = 0.0))]
    fn m_squared<'py>(&self, py: Python<'py>, risk_free_rate: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.m_squared(risk_free_rate);
        values_to_series(py, values, self.inner.ticker_names(), "m_squared")
    }

    /// Modified Sharpe ratio for each ticker.
    ///
    /// Uses annualized excess return in the numerator and Cornish-Fisher VaR
    /// at the corresponding annual horizon in the denominator. The panel
    /// frequency determines the periods-per-year scaling for both terms,
    /// including the horizon decay of skewness and excess kurtosis; the
    /// denominator is not one-period VaR.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Modified Sharpe ratio indexed by ticker name.
    #[pyo3(signature = (risk_free_rate = 0.0, confidence = 0.95))]
    fn modified_sharpe<'py>(
        &self,
        py: Python<'py>,
        risk_free_rate: f64,
        confidence: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .modified_sharpe(risk_free_rate, confidence)
            .map_err(core_to_py)?;
        values_to_series(py, values, self.inner.ticker_names(), "modified_sharpe")
    }

    /// Sterling ratio for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Sterling ratio indexed by ticker name.
    #[pyo3(signature = (risk_free_rate = 0.0, n = 5))]
    fn sterling_ratio<'py>(
        &self,
        py: Python<'py>,
        risk_free_rate: f64,
        n: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .sterling_ratio(risk_free_rate, n)
            .map_err(core_to_py)?;
        values_to_series(py, values, self.inner.ticker_names(), "sterling_ratio")
    }

    /// Burke ratio for each ticker.
    ///
    /// Returns
    /// -------
    /// pandas.Series
    ///     Burke ratio indexed by ticker name.
    #[pyo3(signature = (risk_free_rate = 0.0, n = 5))]
    fn burke_ratio<'py>(
        &self,
        py: Python<'py>,
        risk_free_rate: f64,
        n: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .burke_ratio(risk_free_rate, n)
            .map_err(core_to_py)?;
        values_to_series(py, values, self.inner.ticker_names(), "burke_ratio")
    }

    // -- Vector-per-ticker methods --

    /// Per-period simple returns for each ticker.
    ///
    /// Canonical accessor for the raw return panel. Prefer this over
    /// :meth:`excess_returns` with an all-zero risk-free series or
    /// un-compounding :meth:`cumulative_returns`. Series are span-aware and
    /// therefore ragged across tickers on edge-ragged panels.
    fn returns(&self) -> Vec<Vec<f64>> {
        self.inner.returns()
    }

    /// Per-period simple returns for a single ticker.
    fn returns_for_ticker(&self, ticker_idx: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        self.inner
            .returns_for_ticker(ticker_idx)
            .map_err(core_to_py)
    }

    /// Cumulative returns for each ticker.
    fn cumulative_returns(&self) -> Vec<Vec<f64>> {
        self.inner.cumulative_returns()
    }

    /// Calendar-bucketed compounded returns for each ticker.
    ///
    /// ``frequency`` accepts ``"daily"``, ``"weekly"``, ``"monthly"``,
    /// ``"quarterly"``, ``"semi_annual"``, or ``"annual"`` (or the pandas
    /// offset aliases ``D``/``B``, ``W``, ``M``, ``Q``, ``A``/``Y``). The outer list is
    /// ticker-major in :attr:`ticker_names` order. Each inner list contains
    /// chronological ``(period_end_date, compounded_return)`` tuples, where
    /// returns are decimal fractions (``0.01`` means 1%). Chaining the values
    /// for a ticker reconciles with its final :meth:`cumulative_returns` value.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``frequency`` is not a supported token.
    #[pyo3(signature = (frequency = "monthly"))]
    fn periodic_returns<'py>(
        &self,
        py: Python<'py>,
        frequency: &str,
    ) -> PyResult<PyPeriodicReturnPanel<'py>> {
        let kind = parse_frequency(frequency)?;
        let panel = self.inner.periodic_returns(kind);
        panel
            .into_iter()
            .map(|series| {
                series
                    .into_iter()
                    .map(|(date, value)| Ok((date_to_py(py, date)?, value)))
                    .collect()
            })
            .collect()
    }

    /// Drawdown series for each ticker.
    ///
    /// Despite the ``_series`` suffix (which mirrors the Rust name) this is a
    /// nested ``list[list[float]]`` panel, not a :class:`pandas.Series`. Use
    /// :meth:`to_drawdown_series_dataframe` for the tabular view.
    fn drawdown_series(&self) -> Vec<Vec<f64>> {
        self.inner.drawdown_series()
    }

    /// Correlation matrix across all tickers.
    ///
    /// Uses the complete-case common window when every ticker has at least
    /// two overlapping points; otherwise pairwise intersecting spans. The
    /// matrix is Higham-repaired to the nearest correlation matrix.
    /// Raises when a pair is degenerate or repair fails.
    fn correlation_matrix(&self, py: Python<'_>) -> PyResult<Vec<Vec<f64>>> {
        py.detach(|| self.inner.correlation_matrix().map_err(core_to_py))
    }

    /// Cumulative returns outperformance vs benchmark.
    fn cumulative_returns_outperformance(&self) -> Vec<Vec<f64>> {
        self.inner.cumulative_returns_outperformance()
    }

    /// Drawdown difference vs benchmark.
    fn drawdown_difference(&self) -> Vec<Vec<f64>> {
        self.inner.drawdown_difference()
    }

    /// Excess returns over a risk-free rate series aligned to the panel grid.
    ///
    /// ``rf`` must have one value per active panel date. ``nperiods=None``
    /// geometrically decompounds an annual series using the engine frequency;
    /// pass ``1.0`` when ``rf`` is already periodic.
    #[pyo3(signature = (rf, nperiods = None))]
    fn excess_returns(&self, rf: Vec<f64>, nperiods: Option<f64>) -> PyResult<Vec<Vec<f64>>> {
        self.inner.excess_returns(&rf, nperiods).map_err(core_to_py)
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
            .map_err(core_to_py)?;
        Ok(PyRollingGreeks { inner })
    }

    /// Rolling volatility for a specific ticker.
    #[pyo3(signature = (ticker_idx, window = 63))]
    fn rolling_volatility(
        &self,
        py: Python<'_>,
        ticker_idx: &Bound<'_, PyAny>,
        window: usize,
    ) -> PyResult<PyDatedSeries> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
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
        ticker_idx: &Bound<'_, PyAny>,
        window: usize,
        mar: f64,
    ) -> PyResult<PyDatedSeries> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
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
            .map_err(core_to_py)?;
        Ok(PyDatedSeries::new(inner, "sharpe"))
    }

    /// Drawdown episodes for a specific ticker.
    #[pyo3(signature = (ticker_idx, n = 5))]
    fn drawdown_details(
        &self,
        ticker_idx: &Bound<'_, PyAny>,
        n: usize,
    ) -> PyResult<Vec<PyDrawdownEpisode>> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        Ok(self
            .inner
            .drawdown_details(ticker_idx, n)
            .map_err(core_to_py)?
            .into_iter()
            .map(|e| PyDrawdownEpisode { inner: e })
            .collect())
    }

    /// Multi-factor regression for a specific ticker.
    ///
    /// Factor series are already-excess. ``return_kind="excess"`` leaves the
    /// ticker series unchanged. ``return_kind="total"`` subtracts the
    /// geometrically decompounded period risk-free rate from the ticker
    /// series only.
    #[pyo3(signature = (ticker_idx, factor_returns, return_kind = "excess", risk_free_rate = 0.0))]
    fn multi_factor_greeks(
        &self,
        py: Python<'_>,
        ticker_idx: &Bound<'_, PyAny>,
        factor_returns: Vec<Vec<f64>>,
        return_kind: &str,
        risk_free_rate: f64,
    ) -> PyResult<PyMultiFactorResult> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        let refs: Vec<&[f64]> = factor_returns.iter().map(|v| v.as_slice()).collect();
        let kind = parse_return_kind(return_kind, risk_free_rate)?;
        py.detach(|| self.inner.multi_factor_greeks(ticker_idx, &refs, kind))
            .map(|r| PyMultiFactorResult { inner: r })
            .map_err(core_to_py)
    }

    /// Rolling N-period total compounded return for a specific ticker.
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
            .map_err(core_to_py)?;
        Ok(PyDatedSeries::new(inner, "return"))
    }

    /// Period-to-date lookback returns.
    ///
    /// FYTD is the first observation on or after the fiscal calendar start
    /// through ``ref_date``. Holidays are not skipped. The first included
    /// simple return still spans the prior close.
    #[pyo3(signature = (ref_date, fiscal_year_start_month = None, fiscal_year_start_day = None))]
    fn lookback_returns(
        &self,
        ref_date: Bound<'_, PyAny>,
        fiscal_year_start_month: Option<u8>,
        fiscal_year_start_day: Option<u8>,
    ) -> PyResult<PyLookbackReturns> {
        let d = py_to_date(&ref_date)?;
        Ok(PyLookbackReturns {
            inner: self.lookback_returns_inner(
                d,
                fiscal_year_start_month,
                fiscal_year_start_day,
            )?,
        })
    }

    /// Period statistics for a specific ticker at a given aggregation frequency.
    #[pyo3(signature = (ticker_idx, aggregation_frequency = "monthly", fiscal_year_start_month = None, fiscal_year_start_day = None))]
    fn period_stats(
        &self,
        ticker_idx: &Bound<'_, PyAny>,
        aggregation_frequency: &str,
        fiscal_year_start_month: Option<u8>,
        fiscal_year_start_day: Option<u8>,
    ) -> PyResult<PyPeriodStats> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        let pk = parse_frequency(aggregation_frequency)?;
        let fc = make_fiscal_config(fiscal_year_start_month, fiscal_year_start_day)?;
        Ok(PyPeriodStats {
            inner: self
                .inner
                .period_stats(ticker_idx, pk, fc)
                .map_err(core_to_py)?,
        })
    }

    // -- DataFrame export methods --

    /// The primary pandas view of this object: the summary statistics table.
    ///
    /// One row per ticker, one column per scalar metric. This is an alias for
    /// :meth:`to_summary_dataframe` with default arguments, provided so every
    /// result type in the library answers to a plain ``to_dataframe()``. The
    /// ``*_to_dataframe`` methods on this class are the secondary views
    /// (returns, drawdowns, correlations, lookbacks); use those when you want
    /// something other than the summary.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.to_summary_dataframe(py, 0.0, 0.95)
    }

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
    fn to_summary_dataframe<'py>(
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
            .map_err(core_to_py)?;
        table_to_dataframe(py, &table)?.call_method1("set_index", ("ticker",))
    }

    /// Per-period simple returns for all tickers as a pandas ``DataFrame``.
    ///
    /// Returns a DataFrame with a date index and one column per ticker.
    /// Ragged per-ticker series are padded with ``NaN`` onto the active date
    /// grid. Prefer this over :meth:`excess_returns` with an all-zero
    /// risk-free series or un-compounding
    /// :meth:`to_cumulative_returns_dataframe`.
    fn to_returns_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        panel_to_dataframe(py, &self.inner, self.inner.returns())
    }

    /// Cumulative returns for all tickers as a pandas ``DataFrame``.
    ///
    /// Returns a DataFrame with a date index and one column per ticker.
    fn to_cumulative_returns_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        panel_to_dataframe(py, &self.inner, self.inner.cumulative_returns())
    }

    /// Calendar-bucketed compounded returns as a pandas ``DataFrame``.
    ///
    /// ``frequency`` is one of ``"daily"``, ``"weekly"``, ``"monthly"``,
    /// ``"quarterly"``, ``"semi_annual"``, or ``"annual"`` (pandas offset
    /// aliases ``D``/``B``, ``W``, ``M``, ``Q``, ``A``/``Y`` are accepted too).
    /// Returns a DataFrame
    /// indexed by period-end date with one column per ticker; buckets reconcile
    /// with :meth:`to_cumulative_returns_dataframe`. This convenience exit is
    /// built from the same canonical Rust result as :meth:`periodic_returns`.
    #[pyo3(signature = (frequency = "monthly"))]
    fn to_periodic_returns_dataframe<'py>(
        &self,
        py: Python<'py>,
        frequency: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let kind = parse_frequency(frequency)?;
        periodic_panel_to_dataframe(py, &self.inner, kind)
    }

    /// Drawdown series for all tickers as a pandas ``DataFrame``.
    ///
    /// Returns a DataFrame with a date index and one column per ticker.
    fn to_drawdown_series_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        panel_to_dataframe(py, &self.inner, self.inner.drawdown_series())
    }

    /// Correlation matrix as a pandas ``DataFrame``.
    ///
    /// Returns a ticker × ticker matrix with ticker names as index and columns.
    /// ``df.attrs["repaired"]`` is ``True`` when the estimate was
    /// Higham-repaired (see :meth:`correlation_matrix_repaired`).
    fn to_correlation_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let names = self.inner.ticker_names();
        let (matrix, repaired) = self
            .inner
            .correlation_matrix_with_repair_flag()
            .map_err(core_to_py)?;

        let pd = py.import("pandas")?;
        let kwargs = PyDict::new(py);
        let idx: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        let cols: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        kwargs.set_item("index", idx)?;
        kwargs.set_item("columns", cols)?;
        let frame = pd.call_method("DataFrame", (matrix,), Some(&kwargs))?;
        frame.getattr("attrs")?.set_item("repaired", repaired)?;
        Ok(frame)
    }

    /// Top-N drawdown episodes for a ticker as a pandas ``DataFrame``.
    ///
    /// Columns: start, valley, end (``datetime64``, ``NaT`` while still in
    /// drawdown), duration_days, max_drawdown, near_recovery_threshold,
    /// truncated_at_start.
    #[pyo3(signature = (ticker_idx, n = 5))]
    fn to_drawdown_details_dataframe<'py>(
        &self,
        py: Python<'py>,
        ticker_idx: &Bound<'_, PyAny>,
        n: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        let episodes = self
            .inner
            .drawdown_details(ticker_idx, n)
            .map_err(core_to_py)?;
        let data = PyDict::new(py);
        let pd = py.import("pandas")?;
        let starts: Vec<time::Date> = episodes.iter().map(|e| e.start).collect();
        let valleys: Vec<time::Date> = episodes.iter().map(|e| e.valley).collect();
        let ends: PyResult<Vec<_>> = episodes
            .iter()
            .map(|e| match e.end {
                Some(d) => date_to_py(py, d).map(|v| v.into_any()),
                None => Ok(py.None().into_bound(py)),
            })
            .collect();

        data.set_item("start", dates_to_datetime_index(py, &starts)?)?;
        data.set_item("valley", dates_to_datetime_index(py, &valleys)?)?;
        data.set_item("end", pd.call_method1("to_datetime", (ends?,))?)?;
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
    /// fiscal-start semantics.
    #[pyo3(signature = (ref_date, fiscal_year_start_month = None, fiscal_year_start_day = None))]
    fn to_lookback_returns_dataframe<'py>(
        &self,
        py: Python<'py>,
        ref_date: Bound<'_, PyAny>,
        fiscal_year_start_month: Option<u8>,
        fiscal_year_start_day: Option<u8>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let d = py_to_date(&ref_date)?;
        let lb = self.lookback_returns_inner(d, fiscal_year_start_month, fiscal_year_start_day)?;

        let data = PyDict::new(py);
        data.set_item("mtd", slice_to_pyarray(py, &lb.mtd))?;
        data.set_item("qtd", slice_to_pyarray(py, &lb.qtd))?;
        data.set_item("ytd", slice_to_pyarray(py, &lb.ytd))?;
        data.set_item("fytd", slice_to_pyarray(py, &lb.fytd))?;

        let idx = ticker_index(py, self.inner.ticker_names())?;
        dict_to_dataframe(py, &data, Some(idx))
    }

    /// Beta regression statistics for every ticker vs the benchmark as a
    /// pandas ``DataFrame`` indexed by ticker.
    ///
    /// Columns: ``beta``, ``std_err``, ``ci_lower``, ``ci_upper`` (95%
    /// confidence bounds). Non-finite estimates from a degenerate regression
    /// arrive as ``None``.
    fn to_beta_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .beta()
            .iter()
            .zip(self.inner.ticker_names())
            .map(|(b, name)| {
                serde_json::json!({
                    "ticker": name,
                    "beta": b.beta,
                    "std_err": b.std_err,
                    "ci_lower": b.ci_lower,
                    "ci_upper": b.ci_upper,
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, BETA_COLUMNS)?
            .call_method1("set_index", ("ticker",))
    }

    /// Single-index greeks (annualized Jensen alpha, beta, R², adjusted R²)
    /// for every ticker vs the benchmark as a pandas ``DataFrame`` indexed by
    /// ticker.
    ///
    /// Parameters
    /// ----------
    /// risk_free_rate : float, default 0.0
    ///     Annualized decimal risk-free rate used for Jensen alpha.
    #[pyo3(signature = (risk_free_rate = 0.0))]
    fn to_greeks_dataframe<'py>(
        &self,
        py: Python<'py>,
        risk_free_rate: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .greeks(risk_free_rate)
            .iter()
            .zip(self.inner.ticker_names())
            .map(|(g, name)| {
                serde_json::json!({
                    "ticker": name,
                    "alpha": g.alpha,
                    "beta": g.beta,
                    "r_squared": g.r_squared,
                    "adjusted_r_squared": g.adjusted_r_squared,
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, GREEKS_COLUMNS)?
            .call_method1("set_index", ("ticker",))
    }

    /// Excess returns over a risk-free rate as a pandas ``DataFrame`` with a
    /// date index and one column per ticker.
    ///
    /// Parameters
    /// ----------
    /// rf : float | pandas.Series | sequence of float
    ///     Annualized decimal risk-free rate. A scalar is broadcast to every
    ///     active panel date; a Series/sequence must already be aligned to
    ///     :meth:`active_dates` (one value per date).
    /// nperiods : float, optional
    ///     ``None`` geometrically decompounds the annual rate using the
    ///     panel frequency; pass ``1.0`` when ``rf`` is already per-period.
    ///
    /// Raises
    /// ------
    /// AnalyticsError
    ///     If ``rf`` does not have one value per active date.
    #[pyo3(signature = (rf, nperiods = None))]
    fn to_excess_returns_dataframe<'py>(
        &self,
        py: Python<'py>,
        rf: &Bound<'py, PyAny>,
        nperiods: Option<f64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let rf = if let Ok(rate) = rf.extract::<f64>() {
            vec![rate; self.inner.active_dates().len()]
        } else {
            extract_f64_vec(rf, "rf")?
        };
        let excess = self
            .inner
            .excess_returns(&rf, nperiods)
            .map_err(core_to_py)?;
        panel_to_dataframe(py, &self.inner, excess)
    }

    /// ``True`` when :meth:`correlation_matrix` had to be Higham-repaired.
    ///
    /// The raw pairwise estimate on ragged panels can fail positive
    /// semi-definiteness; the engine then projects it to the nearest valid
    /// correlation matrix. This flag distinguishes a clean estimate from a
    /// repaired one.
    fn correlation_matrix_repaired(&self, py: Python<'_>) -> PyResult<bool> {
        py.detach(|| {
            self.inner
                .correlation_matrix_with_repair_flag()
                .map(|(_, repaired)| repaired)
                .map_err(core_to_py)
        })
    }

    /// Serialize authoritative engine inputs (dates, returns, spans, benchmark,
    /// frequency, active window) to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Rebuild an engine from :meth:`to_json` output.
    ///
    /// Revalidates dates, returns, spans, unique names, benchmark, and active
    /// window, then rebuilds caches. Raises ``ValueError`` for invalid inputs
    /// or unknown fields, including obsolete serialized caches.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: fa::Performance = serde_json::from_str(json)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid Performance JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` (and therefore ``copy.deepcopy``, ``joblib``,
    /// ``multiprocessing``) via the :meth:`to_json` / :meth:`from_json` pair.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Returns a compact summary of the serialized fields, with collections
    /// summarized by length. Use accessors such as :attr:`ticker_names`,
    /// :meth:`dates`, and :meth:`active_dates`, or a DataFrame exit such as
    /// :meth:`to_dataframe`, when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("Performance", &self.inner)
    }
}

/// Sharpe ratio of one return series.
///
/// Annualized excess arithmetic mean over annualized sample volatility, the
/// same kernel as ``Performance.sharpe``.
///
/// Parameters
/// ----------
/// returns : sequence of float | numpy.ndarray | pandas.Series
///     Per-period simple decimal returns (``0.01`` is +1%) in date order.
/// rf : float, default 0.0
///     Annualized risk-free rate as a decimal (``0.02`` for 2%),
///     geometrically decompounded to the observation frequency.
/// periods_per_year : float, default 252
///     Observations per year used to annualize (252 daily, 52 weekly,
///     12 monthly).
///
/// Returns
/// -------
/// float
///     Sharpe ratio; ``inf``/``-inf`` when volatility is zero with non-zero
///     excess return, ``nan`` when ``periods_per_year`` is not positive.
///
/// Raises
/// ------
/// TypeError
///     If ``returns`` is not a float sequence, NumPy array, or Series.
///
/// Examples
/// --------
/// >>> from finstack_quant.analytics import sharpe
/// >>> round(sharpe([0.01, -0.02, 0.015, 0.003], 0.0, 252), 4)
/// 2.0034
#[pyfunction]
#[pyo3(signature = (returns, rf = 0.0, periods_per_year = 252.0))]
fn sharpe(returns: &Bound<'_, PyAny>, rf: f64, periods_per_year: f64) -> PyResult<f64> {
    let returns = extract_f64_vec(returns, "returns")?;
    Ok(fa::sharpe(&returns, rf, periods_per_year))
}

/// Annualized Sortino ratio of one return series.
///
/// Parameters
/// ----------
/// returns : sequence of float | numpy.ndarray | pandas.Series
///     Per-period simple decimal returns in date order.
/// mar : float, default 0.0
///     Minimum acceptable return **per period** as a decimal (not
///     annualized), matching ``Performance.sortino``.
/// periods_per_year : float, default 252
///     Observations per year used to annualize.
///
/// Returns
/// -------
/// float
///     Sortino ratio; ``±inf`` when there is no downside deviation but a
///     non-zero excess mean, ``nan`` for an invalid ``periods_per_year``.
///
/// Raises
/// ------
/// TypeError
///     If ``returns`` is not a float sequence, NumPy array, or Series.
///
/// Examples
/// --------
/// >>> from finstack_quant.analytics import sortino
/// >>> sortino([0.01, -0.02, 0.015, 0.003]) > 0
/// True
#[pyfunction]
#[pyo3(signature = (returns, mar = 0.0, periods_per_year = 252.0))]
fn sortino(returns: &Bound<'_, PyAny>, mar: f64, periods_per_year: f64) -> PyResult<f64> {
    let returns = extract_f64_vec(returns, "returns")?;
    Ok(fa::sortino(&returns, mar, periods_per_year))
}

/// Annualized sample volatility (n−1 denominator) of one return series.
///
/// Parameters
/// ----------
/// returns : sequence of float | numpy.ndarray | pandas.Series
///     Per-period simple decimal returns in date order.
/// periods_per_year : float, default 252
///     Observations per year; the per-period standard deviation is scaled by
///     its square root.
///
/// Returns
/// -------
/// float
///     Annualized volatility as a decimal (``0.15`` is 15%); ``0.0`` for an
///     empty input, ``nan`` for an invalid ``periods_per_year``.
///
/// Raises
/// ------
/// TypeError
///     If ``returns`` is not a float sequence, NumPy array, or Series.
///
/// Examples
/// --------
/// >>> from finstack_quant.analytics import volatility
/// >>> round(volatility([0.01, -0.01, 0.01, -0.01], 252), 4)
/// 0.1833
#[pyfunction]
#[pyo3(signature = (returns, periods_per_year = 252.0))]
fn volatility(returns: &Bound<'_, PyAny>, periods_per_year: f64) -> PyResult<f64> {
    let returns = extract_f64_vec(returns, "returns")?;
    Ok(fa::volatility(&returns, periods_per_year))
}

/// Maximum peak-to-trough drawdown of one return series.
///
/// Parameters
/// ----------
/// returns : sequence of float | numpy.ndarray | pandas.Series
///     Per-period simple decimal returns in date order; they are compounded
///     into a wealth path before the running-peak decline is measured.
///
/// Returns
/// -------
/// float
///     Non-positive fraction (``-0.25`` is a 25% loss); ``0.0`` when the
///     series never falls below its running peak or is empty.
///
/// Raises
/// ------
/// TypeError
///     If ``returns`` is not a float sequence, NumPy array, or Series.
///
/// Examples
/// --------
/// >>> from finstack_quant.analytics import max_drawdown
/// >>> round(max_drawdown([0.10, -0.20, 0.05]), 4)
/// -0.2
#[pyfunction]
fn max_drawdown(returns: &Bound<'_, PyAny>) -> PyResult<f64> {
    let returns = extract_f64_vec(returns, "returns")?;
    Ok(fa::max_drawdown(&returns))
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPerformance>()?;
    m.add_function(wrap_pyfunction!(sharpe, m)?)?;
    m.add_function(wrap_pyfunction!(sortino, m)?)?;
    m.add_function(wrap_pyfunction!(volatility, m)?)?;
    m.add_function(wrap_pyfunction!(max_drawdown, m)?)?;
    Ok(())
}
