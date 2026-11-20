//! Stateful [`Performance`] analytics engine and the four single-series
//! scalar functions (`sharpe`, `sortino`, `volatility`, `max_drawdown`).

use super::types::*;
use crate::bindings::core::dates::daycount::{PyDayCount, PyDayCountContext};
use crate::bindings::core::dates::periods::PyFiscalConfig;
use crate::bindings::core::table::{PyArrowTable, envelope_to_pandas};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::macros::wire_methods;
use crate::bindings::pandas_utils::{dates_to_pylist, values_to_series};
use crate::bindings::pydoc::pydoc;
use crate::errors::analytics_to_py;
use finstack_quant_analytics::{
    CagrDayCount, PanelSeriesKind, Performance, PerformancePanel, ReturnKind,
};
use finstack_quant_core::dates::{Date, DayCountContext, PeriodKind};
use finstack_quant_core::table::TableEnvelope;
use pyo3::buffer::PyBuffer;
use pyo3::exceptions::{PyKeyError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PySlice, PyString};

type PyPeriodicReturnPanel<'py> = Vec<Vec<(Bound<'py, PyAny>, f64)>>;

const FRAME_TYPE_ERROR: &str = "Expected a pandas DataFrame, pandas Series, or Polars DataFrame";

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

/// Look up `polars` in `sys.modules` without importing it, so callers who do
/// not use Polars incur zero import cost and need not have it installed.
fn polars_module<'py>(py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
    let module = py
        .import("sys")?
        .getattr("modules")?
        .call_method1("get", ("polars",))?;
    if module.is_none() {
        Ok(None)
    } else {
        Ok(Some(module))
    }
}

fn require_polars_module<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    match polars_module(py)? {
        Some(pl) => Ok(pl),
        None => Ok(py.import("polars")?.into_any()),
    }
}

fn is_polars_dataframe(obj: &Bound<'_, PyAny>) -> PyResult<bool> {
    let Some(pl) = polars_module(obj.py())? else {
        return Ok(false);
    };
    obj.is_instance(&pl.getattr("DataFrame")?)
}

fn is_polars_series(obj: &Bound<'_, PyAny>) -> PyResult<bool> {
    let Some(pl) = polars_module(obj.py())? else {
        return Ok(false);
    };
    obj.is_instance(&pl.getattr("Series")?)
}

/// Return true only for `pl.Date` or `pl.Datetime` (excluding `Time`/`Duration`).
fn is_polars_date_dtype(pl: &Bound<'_, PyAny>, dtype: &Bound<'_, PyAny>) -> PyResult<bool> {
    let pl_date = pl.getattr("Date")?;
    let pl_datetime = pl.getattr("Datetime")?;
    Ok(dtype.eq(&pl_date)? || dtype.is_instance(&pl_datetime)? || dtype.eq(&pl_datetime)?)
}

fn resolve_polars_date_col(
    pl: &Bound<'_, PyAny>,
    df: &Bound<'_, PyAny>,
    col_names: &[String],
    date_col: Option<&str>,
) -> PyResult<String> {
    if let Some(name) = date_col {
        if col_names.iter().any(|col| col == name) {
            return Ok(name.to_owned());
        }
        return Err(PyKeyError::new_err(format!(
            "date_col {name:?} not found in DataFrame columns"
        )));
    }
    let mut matches = Vec::new();
    for col in col_names {
        let dtype = df.get_item(col)?.getattr("dtype")?;
        if is_polars_date_dtype(pl, &dtype)? {
            matches.push(col.as_str());
        }
    }
    match matches.as_slice() {
        [single] => Ok((*single).to_owned()),
        [] => Err(crate::errors::value_error(
            "Polars DataFrame has no Date or Datetime column; pass date_col=... to specify the date column",
        )),
        _ => Err(crate::errors::value_error(format!(
            "Polars DataFrame has multiple Date/Datetime columns ({}); keep only the observation date column and numeric series columns",
            matches.join(", ")
        ))),
    }
}

fn extract_polars_dates(series: &Bound<'_, PyAny>) -> PyResult<Vec<Date>> {
    let dates_py: Vec<Bound<'_, PyAny>> = series.call_method0("to_list")?.extract()?;
    dates_py.iter().map(py_to_date).collect()
}

/// Extract a numeric Polars ``Series`` as `Vec<f64>`, converting `null` to
/// `f64::NAN` while preserving `inf`/`nan` and rejecting non-numeric dtypes.
fn extract_polars_float64_column(series: &Bound<'_, PyAny>, col_label: &str) -> PyResult<Vec<f64>> {
    let py = series.py();
    let pl = require_polars_module(py)?;
    let dtype = series.getattr("dtype")?;
    let is_numeric: bool = dtype.call_method0("is_numeric")?.extract()?;
    let is_null: bool = dtype.eq(pl.getattr("Null")?)?;
    if !is_numeric && !is_null {
        return Err(PyTypeError::new_err(format!(
            "Column {col_label:?} has non-numeric Polars dtype {dtype}; expected numeric values"
        )));
    }
    let float_dtype = pl.getattr("Float64")?;
    let float_series = series
        .call_method1("cast", (float_dtype,))
        .map_err(|err| {
            PyTypeError::new_err(format!(
                "Column {col_label:?} could not be converted to Polars Float64: {err}"
            ))
        })?
        .call_method1("fill_null", (f64::NAN,))?;
    let array = float_series.call_method0("to_numpy").map_err(|err| {
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

/// Extract a 1-D float vector from a Polars ``Series``, list, tuple, NumPy
/// array, or pandas ``Series``.
fn extract_f64_vec(obj: &Bound<'_, PyAny>, label: &str) -> PyResult<Vec<f64>> {
    if is_polars_series(obj)? {
        return extract_polars_float64_column(obj, label);
    }
    if is_polars_dataframe(obj)? {
        return Err(PyTypeError::new_err(format!(
            "{label} must be a sequence of floats, a NumPy array, a pandas Series, or a Polars Series"
        )));
    }
    if let Ok(values) = obj.extract::<Vec<f64>>() {
        return Ok(values);
    }
    if obj.hasattr("to_numpy")? {
        return extract_float64_column(obj, label);
    }
    Err(PyTypeError::new_err(format!(
        "{label} must be a sequence of floats, a NumPy array, a pandas Series, or a Polars Series"
    )))
}

fn extract_factor_sequences(factor_returns: &Bound<'_, PyAny>) -> PyResult<Vec<Vec<f64>>> {
    if factor_returns.is_instance_of::<PyString>() || factor_returns.is_instance_of::<PyBytes>() {
        return Err(PyTypeError::new_err(
            "factor_returns must be factor-major sequences of floats",
        ));
    }
    if let Ok(factors) = factor_returns.extract::<Vec<Vec<f64>>>() {
        return Ok(factors);
    }
    let iter = factor_returns.try_iter().map_err(|_| {
        PyTypeError::new_err("factor_returns must be factor-major sequences of floats")
    })?;
    let mut factors = Vec::new();
    for item in iter {
        let item = item?;
        factors.push(extract_f64_vec(&item, "factor_returns")?);
    }
    Ok(factors)
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

fn pandas_column_names(df: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let columns = df.getattr("columns")?;
    let cols_list = columns.call_method0("tolist")?;
    cols_list.extract().map_err(|_| {
        PyTypeError::new_err(
            "Performance requires string column labels (ticker names); the DataFrame \
             columns are not all str — rename them, e.g. df.columns = df.columns.astype(str)",
        )
    })
}

fn resolve_pandas_date_col_idx(col_names: &[String], date_col: &str) -> PyResult<usize> {
    let mut matches = col_names
        .iter()
        .enumerate()
        .filter_map(|(idx, col)| (col == date_col).then_some(idx));
    let Some(first) = matches.next() else {
        return Err(PyKeyError::new_err(format!(
            "date_col {date_col:?} not found in DataFrame columns"
        )));
    };
    if matches.next().is_some() {
        return Err(crate::errors::value_error(format!(
            "duplicate date_col {date_col:?} in DataFrame columns"
        )));
    }
    Ok(first)
}

fn extract_pandas_panel(df: &Bound<'_, PyAny>, date_col: Option<&str>) -> PyResult<DataFramePanel> {
    let py = df.py();
    let col_names = pandas_column_names(df)?;
    let iloc = df.getattr("iloc")?;
    let all_rows = PySlice::full(py);

    let (dates_list, date_idx) = if let Some(name) = date_col {
        let idx = resolve_pandas_date_col_idx(&col_names, name)?;
        let series = iloc.get_item((&all_rows, idx))?;
        (series.call_method0("tolist")?, Some(idx))
    } else {
        (df.getattr("index")?.call_method0("tolist")?, None)
    };

    let dates_py: Vec<Bound<'_, PyAny>> = dates_list.extract()?;
    let dates = dates_py
        .iter()
        .map(py_to_date)
        .collect::<PyResult<Vec<_>>>()?;

    let mut ticker_names = Vec::with_capacity(col_names.len());
    let mut columns = Vec::with_capacity(col_names.len());
    for (i, col) in col_names.iter().enumerate() {
        if date_idx == Some(i) {
            continue;
        }
        // Index positionally so duplicate column names still reach Rust validation.
        let series = iloc.get_item((&all_rows, i))?;
        ticker_names.push(col.clone());
        columns.push(extract_float64_column(&series, col)?);
    }

    Ok(DataFramePanel {
        dates,
        columns,
        ticker_names,
    })
}

fn extract_polars_panel(df: &Bound<'_, PyAny>, date_col: Option<&str>) -> PyResult<DataFramePanel> {
    let py = df.py();
    let pl = require_polars_module(py)?;
    let col_names: Vec<String> = df.getattr("columns")?.extract()?;
    let date_name = resolve_polars_date_col(&pl, df, &col_names, date_col)?;
    let dates = extract_polars_dates(&df.get_item(&date_name)?)?;

    let mut ticker_names = Vec::with_capacity(col_names.len().saturating_sub(1));
    let mut columns = Vec::with_capacity(col_names.len().saturating_sub(1));
    for col in &col_names {
        if col == &date_name {
            continue;
        }
        let series = df.get_item(col)?;
        ticker_names.push(col.clone());
        columns.push(extract_polars_float64_column(&series, col)?);
    }

    Ok(DataFramePanel {
        dates,
        columns,
        ticker_names,
    })
}

/// Extract dates, numeric matrix, and ticker names from a pandas ``DataFrame``,
/// pandas ``Series``, or Polars ``DataFrame``.
fn extract_dataframe_panel(
    frame: &Bound<'_, PyAny>,
    date_col: Option<&str>,
    error_message: &str,
) -> PyResult<DataFramePanel> {
    if is_polars_dataframe(frame)? {
        return extract_polars_panel(frame, date_col);
    }
    if is_polars_series(frame)? {
        return Err(PyTypeError::new_err(
            "Expected a pandas DataFrame/Series or a Polars DataFrame; a Polars Series has no date index — wrap it in a polars.DataFrame with a date column",
        ));
    }
    let pd = frame.py().import("pandas")?;
    if frame.is_instance(&pd.getattr("Series")?)? {
        if date_col.is_some() {
            return Err(crate::errors::value_error(
                "date_col cannot be used with a pandas Series; a Series always uses its index for dates",
            ));
        }
        let promoted = series_to_frame(frame)?;
        return extract_pandas_panel(&promoted, None);
    }
    if frame.is_instance(&pd.getattr("DataFrame")?)? {
        return extract_pandas_panel(frame, date_col);
    }
    Err(PyTypeError::new_err(error_message.to_owned()))
}

fn align_factor_panel(
    df: &Bound<'_, PyAny>,
    active_dates: &[Date],
    date_col: Option<&str>,
) -> PyResult<Vec<Vec<f64>>> {
    let py = df.py();
    let is_polars = is_polars_dataframe(df)?;
    let (labels, polars_meta) = if is_polars {
        let pl = require_polars_module(py)?;
        let col_names: Vec<String> = df.getattr("columns")?.extract()?;
        let date_name = resolve_polars_date_col(&pl, df, &col_names, date_col)?;
        let labels: Vec<Bound<'_, PyAny>> = df
            .get_item(&date_name)?
            .call_method0("to_list")?
            .extract()?;
        (labels, Some((col_names, date_name)))
    } else if df.is_instance(&py.import("pandas")?.getattr("DataFrame")?)? {
        let labels: Vec<Bound<'_, PyAny>> = if let Some(name) = date_col {
            let col_names = pandas_column_names(df)?;
            let date_idx = resolve_pandas_date_col_idx(&col_names, name)?;
            df.getattr("iloc")?
                .get_item((PySlice::full(py), date_idx))?
                .call_method0("tolist")?
                .extract()?
        } else {
            df.getattr("index")?.call_method0("tolist")?.extract()?
        };
        (labels, None)
    } else {
        return Err(PyTypeError::new_err(
            "factor_returns must be factor-major sequences, a pandas DataFrame, or a Polars DataFrame",
        ));
    };

    let mut rows = finstack_quant_core::FxHashMap::default();
    for (row, label) in labels.iter().enumerate() {
        let date = py_to_date(label)?;
        if rows.insert(date, row).is_some() {
            return Err(crate::errors::value_error(format!(
                "duplicate factor date {date}"
            )));
        }
    }
    let positions = active_dates
        .iter()
        .map(|date| {
            rows.get(date).copied().ok_or_else(|| {
                PyKeyError::new_err(format!("factor_returns is missing active date {date}"))
            })
        })
        .collect::<PyResult<Vec<_>>>()?;

    let columns = if let Some((col_names, date_name)) = polars_meta {
        let mut cols = Vec::with_capacity(col_names.len().saturating_sub(1));
        for col in &col_names {
            if col == &date_name {
                continue;
            }
            let gathered = df.get_item(col)?.call_method1("gather", (&positions,))?;
            cols.push(extract_polars_float64_column(&gathered, col)?);
        }
        cols
    } else {
        let selected = df.getattr("iloc")?.get_item(positions)?;
        extract_pandas_panel(&selected, date_col)?.columns
    };

    if columns.iter().flatten().any(|value| !value.is_finite()) {
        return Err(crate::errors::value_error(
            "each aligned factor return must be finite",
        ));
    }
    Ok(columns)
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
/// series (or ``"asset"`` when unnamed).
fn series_to_frame<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let name = value.getattr("name")?;
    let label: String = if name.is_none() {
        "asset".to_owned()
    } else {
        name.str()?.extract()?
    };
    value.call_method1("to_frame", (label,))
}

/// Build a `Performance` from an extracted price or return DataFrame panel.
fn performance_from_extracted(
    py: Python<'_>,
    panel: DataFramePanel,
    kind: PanelSeriesKind,
    benchmark_ticker: Option<&str>,
    frequency: Option<&str>,
    initial_date: Option<Date>,
) -> PyResult<PyPerformance> {
    let mut dates = panel.dates;
    if let Some(initial_date) = initial_date {
        dates.insert(0, initial_date);
    }
    let panel = PerformancePanel {
        dates,
        series: panel.columns,
        kind,
        ticker_names: panel.ticker_names,
        benchmark_ticker: benchmark_ticker.map(str::to_owned),
        frequency: frequency.map(parse_period_kind).transpose()?,
    };
    let inner = py
        .detach(|| Performance::new(&panel))
        .map_err(analytics_to_py)?;
    Ok(PyPerformance { inner })
}

fn parse_period_kind(frequency: &str) -> PyResult<PeriodKind> {
    frequency.parse::<PeriodKind>().map_err(analytics_to_py)
}

#[doc = pydoc!("finstack_quant.analytics._performance", "Performance")]
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

    fn compute_summary_table(
        &self,
        py: Python<'_>,
        risk_free_rate: f64,
        mar: f64,
        omega_threshold: f64,
        confidence: f64,
    ) -> PyResult<TableEnvelope> {
        // The 22-metric table is assembled in Rust (`Performance::summary`),
        // so summary(), to_dataframe(), to_arrow(), and WASM read the same
        // rows; release the GIL for the O(22 * n * m) walk.
        py.detach(|| {
            self.inner
                .summary(risk_free_rate, mar, omega_threshold, confidence)
        })
        .map_err(analytics_to_py)
    }
}

#[pymethods]
impl PyPerformance {
    #[new]
    #[pyo3(signature = (prices, benchmark_ticker=None, frequency=None, date_col=None))]
    fn new(
        py: Python<'_>,
        prices: Bound<'_, PyAny>,
        benchmark_ticker: Option<&str>,
        frequency: Option<&str>,
        date_col: Option<&str>,
    ) -> PyResult<Self> {
        let panel = extract_dataframe_panel(&prices, date_col, FRAME_TYPE_ERROR)?;
        performance_from_extracted(
            py,
            panel,
            PanelSeriesKind::Prices,
            benchmark_ticker,
            frequency,
            None,
        )
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.from_returns")]
    #[staticmethod]
    #[pyo3(signature = (returns, initial_date, benchmark_ticker=None, frequency=None, date_col=None))]
    fn from_returns(
        py: Python<'_>,
        returns: Bound<'_, PyAny>,
        initial_date: Bound<'_, PyAny>,
        benchmark_ticker: Option<&str>,
        frequency: Option<&str>,
        date_col: Option<&str>,
    ) -> PyResult<Self> {
        let panel = extract_dataframe_panel(&returns, date_col, FRAME_TYPE_ERROR)?;
        performance_from_extracted(
            py,
            panel,
            PanelSeriesKind::Returns,
            benchmark_ticker,
            frequency,
            Some(py_to_date(&initial_date)?),
        )
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.reset_date_range")]
    fn reset_date_range(&mut self, start: Bound<'_, PyAny>, end: Bound<'_, PyAny>) -> PyResult<()> {
        let s = py_to_date(&start)?;
        let e = py_to_date(&end)?;
        self.inner.reset_date_range(s, e).map_err(analytics_to_py)
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.reset_bench_ticker")]
    fn reset_bench_ticker(&mut self, ticker: &str) -> PyResult<()> {
        self.inner
            .reset_bench_ticker(ticker)
            .map_err(analytics_to_py)
    }

    // -- Getters --

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.get_ticker_index")]
    fn get_ticker_index(&self, ticker: &str) -> PyResult<usize> {
        self.inner.get_ticker_index(ticker).map_err(analytics_to_py)
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.get_ticker_names")]
    fn get_ticker_names(&self) -> Vec<String> {
        self.inner.get_ticker_names().to_vec()
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.get_benchmark_idx")]
    fn get_benchmark_idx(&self) -> Option<usize> {
        self.inner.get_benchmark_idx()
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.get_frequency")]
    fn get_frequency(&self) -> Option<String> {
        self.inner.get_frequency().map(|kind| kind.to_string())
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.get_dates")]
    fn get_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        dates_to_pylist(py, self.inner.get_dates())
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.get_active_dates")]
    fn get_active_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        dates_to_pylist(py, self.inner.get_active_dates())
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.get_active_dates_for_ticker")]
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

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.get_paired_dates")]
    fn get_paired_dates<'py>(
        &self,
        py: Python<'py>,
        ticker_idx: &Bound<'_, PyAny>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        dates_to_pylist(
            py,
            self.inner
                .get_paired_dates(ticker_idx)
                .map_err(analytics_to_py)?,
        )
    }

    // -- Scalar-per-ticker methods --

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.cagr")]
    #[pyo3(signature = (day_count = None, ctx = None))]
    fn cagr<'py>(
        &self,
        py: Python<'py>,
        day_count: Option<&Bound<'_, PyAny>>,
        ctx: Option<&PyDayCountContext>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let day_count = parse_cagr_day_count(day_count)?;
        let context = ctx
            .map(|ctx| ctx.inner.to_ctx())
            .transpose()
            .map_err(analytics_to_py)?
            .unwrap_or_else(DayCountContext::default);
        let values = self
            .inner
            .cagr(day_count, context)
            .map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "cagr")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.mean_return")]
    #[pyo3(signature = (annualize))]
    fn mean_return<'py>(&self, py: Python<'py>, annualize: bool) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.mean_return(annualize).map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "mean_return")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.volatility")]
    #[pyo3(signature = (annualize))]
    fn volatility<'py>(&self, py: Python<'py>, annualize: bool) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.volatility(annualize).map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "volatility")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.sharpe")]
    #[pyo3(signature = (risk_free_rate))]
    fn sharpe<'py>(&self, py: Python<'py>, risk_free_rate: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.sharpe(risk_free_rate).map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "sharpe")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.sortino")]
    #[pyo3(signature = (mar))]
    fn sortino<'py>(&self, py: Python<'py>, mar: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.sortino(mar).map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "sortino")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.calmar")]
    fn calmar<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.calmar().map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "calmar")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.max_drawdown")]
    fn max_drawdown<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.max_drawdown().map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "max_drawdown")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.mean_drawdown")]
    fn mean_drawdown<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.mean_drawdown().map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "mean_drawdown")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.value_at_risk")]
    #[pyo3(signature = (confidence))]
    fn value_at_risk<'py>(&self, py: Python<'py>, confidence: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .value_at_risk(confidence)
            .map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "value_at_risk")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.expected_shortfall")]
    #[pyo3(signature = (confidence))]
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

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.tracking_error")]
    fn tracking_error<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.tracking_error().map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "tracking_error")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.information_ratio")]
    fn information_ratio<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.information_ratio().map_err(analytics_to_py)?;
        values_to_series(
            py,
            values,
            self.inner.get_ticker_names(),
            "information_ratio",
        )
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.skewness")]
    fn skewness<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.skewness().map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "skewness")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.kurtosis")]
    fn kurtosis<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.kurtosis().map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "kurtosis")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.geometric_mean")]
    fn geometric_mean<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.geometric_mean().map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "geometric_mean")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.downside_deviation")]
    #[pyo3(signature = (mar))]
    fn downside_deviation<'py>(&self, py: Python<'py>, mar: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .downside_deviation(mar)
            .map_err(analytics_to_py)?;
        values_to_series(
            py,
            values,
            self.inner.get_ticker_names(),
            "downside_deviation",
        )
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.max_drawdown_duration")]
    fn max_drawdown_duration<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .max_drawdown_duration()
            .map_err(analytics_to_py)?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("index", self.inner.get_ticker_names())?;
        kwargs.set_item("name", "max_drawdown_duration")?;
        kwargs.set_item("dtype", "Int64")?;
        py.import("pandas")?
            .getattr("Series")?
            .call((values,), Some(&kwargs))
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.up_capture")]
    fn up_capture<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.up_capture().map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "up_capture")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.down_capture")]
    fn down_capture<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.down_capture().map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "down_capture")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.capture_ratio")]
    fn capture_ratio<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.capture_ratio().map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "capture_ratio")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.omega_ratio")]
    #[pyo3(signature = (threshold))]
    fn omega_ratio<'py>(&self, py: Python<'py>, threshold: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.omega_ratio(threshold).map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "omega_ratio")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.treynor")]
    #[pyo3(signature = (risk_free_rate))]
    fn treynor<'py>(&self, py: Python<'py>, risk_free_rate: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .treynor(risk_free_rate)
            .map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "treynor")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.gain_to_pain")]
    fn gain_to_pain<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.gain_to_pain().map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "gain_to_pain")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.ulcer_index")]
    fn ulcer_index<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.ulcer_index().map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "ulcer_index")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.martin_ratio")]
    fn martin_ratio<'py>(
        &self,
        py: Python<'py>,
        risk_free_rate: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .martin_ratio(risk_free_rate)
            .map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "martin_ratio")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.recovery_factor")]
    fn recovery_factor<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.recovery_factor().map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "recovery_factor")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.pain_index")]
    fn pain_index<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.pain_index().map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "pain_index")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.pain_ratio")]
    #[pyo3(signature = (risk_free_rate))]
    fn pain_ratio<'py>(&self, py: Python<'py>, risk_free_rate: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .pain_ratio(risk_free_rate)
            .map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "pain_ratio")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.tail_ratio")]
    #[pyo3(signature = (confidence))]
    fn tail_ratio<'py>(&self, py: Python<'py>, confidence: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.tail_ratio(confidence).map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "tail_ratio")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.r_squared")]
    fn r_squared<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.r_squared().map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "r_squared")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.batting_average")]
    fn batting_average<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.batting_average().map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "batting_average")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.parametric_var")]
    #[pyo3(signature = (confidence, horizon_periods = None))]
    fn parametric_var<'py>(
        &self,
        py: Python<'py>,
        confidence: f64,
        horizon_periods: Option<u64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .parametric_var(confidence, horizon_periods)
            .map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "parametric_var")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.cornish_fisher_var")]
    #[pyo3(signature = (confidence, horizon_periods = None))]
    fn cornish_fisher_var<'py>(
        &self,
        py: Python<'py>,
        confidence: f64,
        horizon_periods: Option<u64>,
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

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.cdar")]
    #[pyo3(signature = (confidence))]
    fn cdar<'py>(&self, py: Python<'py>, confidence: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self.inner.cdar(confidence).map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "cdar")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.m_squared")]
    #[pyo3(signature = (risk_free_rate))]
    fn m_squared<'py>(&self, py: Python<'py>, risk_free_rate: f64) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .m_squared(risk_free_rate)
            .map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "m_squared")
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.modified_sharpe")]
    #[pyo3(signature = (risk_free_rate, confidence))]
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

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.sterling_ratio")]
    #[pyo3(signature = (risk_free_rate, n))]
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

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.burke_ratio")]
    fn burke_ratio<'py>(
        &self,
        py: Python<'py>,
        risk_free_rate: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        let values = self
            .inner
            .burke_ratio(risk_free_rate)
            .map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), "burke_ratio")
    }

    // -- Vector-per-ticker methods --

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.get_returns")]
    fn get_returns(&self) -> Vec<Vec<f64>> {
        self.inner.get_returns()
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.get_returns_for_ticker")]
    fn get_returns_for_ticker(&self, ticker_idx: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        self.inner
            .get_returns_for_ticker(ticker_idx)
            .map_err(analytics_to_py)
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.cumulative_returns")]
    fn cumulative_returns(&self) -> PyResult<Vec<Vec<f64>>> {
        self.inner.cumulative_returns().map_err(analytics_to_py)
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.periodic_returns")]
    #[pyo3(signature = (frequency))]
    fn periodic_returns<'py>(
        &self,
        py: Python<'py>,
        frequency: &str,
    ) -> PyResult<PyPeriodicReturnPanel<'py>> {
        let kind = parse_period_kind(frequency)?;
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

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.get_drawdown_series")]
    fn get_drawdown_series(&self) -> Vec<Vec<f64>> {
        self.inner.get_drawdown_series()
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.correlation_matrix")]
    fn correlation_matrix(&self, py: Python<'_>) -> PyResult<PyCorrelationResult> {
        let inner = py
            .detach(|| self.inner.correlation_matrix())
            .map_err(analytics_to_py)?;
        Ok(PyCorrelationResult { inner })
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.cumulative_returns_outperformance")]
    fn cumulative_returns_outperformance(&self) -> PyResult<Vec<Vec<f64>>> {
        self.inner
            .cumulative_returns_outperformance()
            .map_err(analytics_to_py)
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.drawdown_difference")]
    fn drawdown_difference(&self) -> PyResult<Vec<Vec<f64>>> {
        self.inner.drawdown_difference().map_err(analytics_to_py)
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.excess_returns")]
    fn excess_returns(&self, rf: &Bound<'_, PyAny>) -> PyResult<Vec<Vec<f64>>> {
        let rf = extract_f64_vec(rf, "rf")?;
        self.inner.excess_returns(&rf).map_err(analytics_to_py)
    }

    // -- Per-ticker indexed methods --

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.beta")]
    fn beta(&self) -> PyResult<Vec<PyBetaResult>> {
        Ok(self
            .inner
            .beta()
            .map_err(analytics_to_py)?
            .into_iter()
            .map(|b| PyBetaResult { inner: b })
            .collect())
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.greeks")]
    #[pyo3(signature = (risk_free_rate))]
    fn greeks(&self, risk_free_rate: f64) -> PyResult<Vec<PyGreeksResult>> {
        Ok(self
            .inner
            .greeks(risk_free_rate)
            .map_err(analytics_to_py)?
            .into_iter()
            .map(|g| PyGreeksResult { inner: g })
            .collect())
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.rolling_greeks")]
    #[pyo3(signature = (ticker_idx, window, risk_free_rate))]
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

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.rolling_volatility")]
    #[pyo3(signature = (ticker_idx, window))]
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

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.rolling_sortino")]
    #[pyo3(signature = (ticker_idx, window, mar))]
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

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.rolling_sharpe")]
    #[pyo3(signature = (ticker_idx, window, risk_free_rate))]
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

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.drawdown_details")]
    #[pyo3(signature = (ticker_idx, n))]
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

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.multi_factor_greeks")]
    #[pyo3(signature = (ticker_idx, factor_returns, risk_free_rate, date_col=None))]
    fn multi_factor_greeks(
        &self,
        py: Python<'_>,
        ticker_idx: &Bound<'_, PyAny>,
        factor_returns: &Bound<'_, PyAny>,
        risk_free_rate: f64,
        date_col: Option<&str>,
    ) -> PyResult<PyMultiFactorResult> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        let factor_returns = if is_polars_dataframe(factor_returns)?
            || factor_returns.is_instance(&py.import("pandas")?.getattr("DataFrame")?)?
        {
            align_factor_panel(
                factor_returns,
                self.inner
                    .get_active_dates_for_ticker(ticker_idx)
                    .map_err(analytics_to_py)?,
                date_col,
            )?
        } else {
            let factors = extract_factor_sequences(factor_returns)?;
            if date_col.is_some() {
                return Err(crate::errors::value_error(
                    "date_col can only be used when factor_returns is a DataFrame",
                ));
            }
            factors
        };
        let refs: Vec<&[f64]> = factor_returns.iter().map(Vec::as_slice).collect();
        py.detach(|| {
            self.inner
                .multi_factor_greeks(ticker_idx, &refs, risk_free_rate)
        })
        .map(|r| PyMultiFactorResult { inner: r })
        .map_err(analytics_to_py)
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.rolling_returns")]
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

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.lookback_returns")]
    #[pyo3(signature = (ref_date = None, fiscal_config = None))]
    fn lookback_returns(
        &self,
        ref_date: Option<Bound<'_, PyAny>>,
        fiscal_config: Option<&PyFiscalConfig>,
    ) -> PyResult<PyLookbackReturns> {
        let ref_date = ref_date.as_ref().map(py_to_date).transpose()?;
        let inner = self
            .inner
            .lookback_returns(ref_date, fiscal_config.map(|config| config.inner))
            .map_err(analytics_to_py)?;
        Ok(PyLookbackReturns { inner })
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.period_stats")]
    #[pyo3(signature = (ticker_idx, aggregation_frequency, fiscal_config = None))]
    fn period_stats(
        &self,
        ticker_idx: &Bound<'_, PyAny>,
        aggregation_frequency: &str,
        fiscal_config: Option<&PyFiscalConfig>,
    ) -> PyResult<PyPeriodStats> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        let pk = parse_period_kind(aggregation_frequency)?;
        Ok(PyPeriodStats {
            inner: self
                .inner
                .period_stats(ticker_idx, pk, fiscal_config.map(|config| config.inner))
                .map_err(analytics_to_py)?,
        })
    }

    // -- DataFrame / TableEnvelope export --

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.summary")]
    #[pyo3(signature = (risk_free_rate, mar, omega_threshold, confidence))]
    fn summary<'py>(
        &self,
        py: Python<'py>,
        risk_free_rate: f64,
        mar: f64,
        omega_threshold: f64,
        confidence: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        let table =
            self.compute_summary_table(py, risk_free_rate, mar, omega_threshold, confidence)?;
        crate::bindings::pandas_utils::serde_to_py(py, &table)
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.to_dataframe")]
    #[pyo3(signature = (risk_free_rate, mar, omega_threshold, confidence))]
    fn to_dataframe<'py>(
        &self,
        py: Python<'py>,
        risk_free_rate: f64,
        mar: f64,
        omega_threshold: f64,
        confidence: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        let table =
            self.compute_summary_table(py, risk_free_rate, mar, omega_threshold, confidence)?;
        envelope_to_pandas(py, &table)?.call_method1("set_index", ("ticker",))
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.to_arrow")]
    #[pyo3(signature = (risk_free_rate, mar, omega_threshold, confidence))]
    fn to_arrow(
        &self,
        py: Python<'_>,
        risk_free_rate: f64,
        mar: f64,
        omega_threshold: f64,
        confidence: f64,
    ) -> PyResult<PyArrowTable> {
        let table =
            self.compute_summary_table(py, risk_free_rate, mar, omega_threshold, confidence)?;
        PyArrowTable::from_envelope(&table)
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
    module = "finstack_quant.analytics._performance"
);

#[doc = pydoc!("finstack_quant.analytics._performance", "sharpe")]
#[pyfunction]
#[pyo3(signature = (returns, risk_free_rate, periods_per_year))]
fn sharpe(returns: &Bound<'_, PyAny>, risk_free_rate: f64, periods_per_year: f64) -> PyResult<f64> {
    let returns = extract_f64_vec(returns, "returns")?;
    finstack_quant_analytics::sharpe(&returns, risk_free_rate, periods_per_year)
        .map_err(analytics_to_py)
}

#[doc = pydoc!("finstack_quant.analytics._performance", "sortino")]
#[pyfunction]
#[pyo3(signature = (returns, mar, periods_per_year))]
fn sortino(returns: &Bound<'_, PyAny>, mar: f64, periods_per_year: f64) -> PyResult<f64> {
    let returns = extract_f64_vec(returns, "returns")?;
    finstack_quant_analytics::sortino(&returns, mar, periods_per_year).map_err(analytics_to_py)
}

#[doc = pydoc!("finstack_quant.analytics._performance", "volatility")]
#[pyfunction]
#[pyo3(signature = (returns, periods_per_year))]
fn volatility(returns: &Bound<'_, PyAny>, periods_per_year: f64) -> PyResult<f64> {
    let returns = extract_f64_vec(returns, "returns")?;
    finstack_quant_analytics::volatility(&returns, periods_per_year).map_err(analytics_to_py)
}

#[doc = pydoc!("finstack_quant.analytics._performance", "max_drawdown")]
#[pyfunction]
fn max_drawdown(returns: &Bound<'_, PyAny>) -> PyResult<f64> {
    let returns = extract_f64_vec(returns, "returns")?;
    finstack_quant_analytics::max_drawdown(&returns).map_err(analytics_to_py)
}

#[doc = pydoc!("finstack_quant.analytics._performance", "multi_factor_greeks")]
#[pyfunction]
#[pyo3(signature = (returns, factor_returns, periods_per_year, return_kind, risk_free_rate=None))]
fn multi_factor_greeks(
    py: Python<'_>,
    returns: &Bound<'_, PyAny>,
    factor_returns: &Bound<'_, PyAny>,
    periods_per_year: f64,
    return_kind: &str,
    risk_free_rate: Option<f64>,
) -> PyResult<PyMultiFactorResult> {
    let returns = extract_f64_vec(returns, "returns")?;
    let factor_returns = extract_factor_sequences(factor_returns)?;
    let refs: Vec<&[f64]> = factor_returns.iter().map(Vec::as_slice).collect();
    let kind = ReturnKind::from_label(return_kind, risk_free_rate).map_err(analytics_to_py)?;
    let inner = py
        .detach(|| {
            finstack_quant_analytics::multi_factor_greeks(&returns, &refs, periods_per_year, kind)
        })
        .map_err(analytics_to_py)?;
    Ok(PyMultiFactorResult { inner })
}

#[doc = pydoc!("finstack_quant.analytics._performance", "validate_summary")]
#[pyfunction]
fn validate_summary(py: Python<'_>, table: &Bound<'_, PyAny>) -> PyResult<()> {
    let table: finstack_quant_core::table::TableEnvelope =
        crate::bindings::module_utils::spec_from_py(py, table, "analytics summary")?;
    py.detach(|| finstack_quant_analytics::validate_summary(&table))
        .map_err(analytics_to_py)
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPerformance>()?;
    m.add_function(wrap_pyfunction!(sharpe, m)?)?;
    m.add_function(wrap_pyfunction!(sortino, m)?)?;
    m.add_function(wrap_pyfunction!(volatility, m)?)?;
    m.add_function(wrap_pyfunction!(max_drawdown, m)?)?;
    m.add_function(wrap_pyfunction!(multi_factor_greeks, m)?)?;
    m.add_function(wrap_pyfunction!(validate_summary, m)?)?;
    Ok(())
}
