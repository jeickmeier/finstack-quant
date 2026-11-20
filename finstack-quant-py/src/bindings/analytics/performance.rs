//! Stateful [`Performance`] analytics engine and the four single-series
//! scalar functions (`sharpe`, `sortino`, `volatility`, `max_drawdown`).

use super::types::*;
use crate::bindings::core::dates::daycount::{PyDayCount, PyDayCountContext};
use crate::bindings::core::dates::periods::PyFiscalConfig;
use crate::bindings::core::table::envelope_to_pandas;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::macros::{py_getters, wire_methods};
use crate::bindings::pandas_utils::{dates_to_pylist, values_to_series};
use crate::bindings::pydoc::pydoc;
use crate::errors::analytics_to_py;
use finstack_quant_analytics::{
    CagrDayCount, PanelSeriesKind, Performance, PerformancePanel, ReturnKind,
};
use finstack_quant_core::dates::{Date, DayCountContext, PeriodKind};
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

fn align_factor_panel(df: &Bound<'_, PyAny>, active_dates: &[Date]) -> PyResult<Vec<Vec<f64>>> {
    let message = "factor_returns must be factor-major sequences or a DataFrame";
    if !df.is_instance(&df.py().import("pandas")?.getattr("DataFrame")?)? {
        return Err(PyTypeError::new_err(message));
    }
    let labels: Vec<Bound<'_, PyAny>> = df.getattr("index")?.call_method0("tolist")?.extract()?;
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
                pyo3::exceptions::PyKeyError::new_err(format!(
                    "factor_returns is missing active date {date}"
                ))
            })
        })
        .collect::<PyResult<Vec<_>>>()?;
    let selected = df.getattr("iloc")?.get_item(positions)?;
    let panel = extract_dataframe_panel(&selected, message)?;
    if panel
        .columns
        .iter()
        .flatten()
        .any(|value| !value.is_finite())
    {
        return Err(crate::errors::value_error(
            "each aligned factor return must be finite",
        ));
    }
    Ok(panel.columns)
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

/// Build a `Performance` from a pandas price or return panel.
fn performance_from_frame(
    py: Python<'_>,
    frame: &Bound<'_, PyAny>,
    kind: PanelSeriesKind,
    benchmark_ticker: Option<&str>,
    frequency: Option<&str>,
    initial_date: Option<Date>,
) -> PyResult<PyPerformance> {
    let frame = series_to_frame(frame)?;
    let panel = extract_dataframe_panel(
        &frame,
        "Expected a pandas DataFrame or Series; use Performance.from_panel(PerformancePanel(...)) for raw lists",
    )?;
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
    performance_from_panel(py, &panel)
}

fn performance_from_panel(py: Python<'_>, panel: &PerformancePanel) -> PyResult<PyPerformance> {
    let inner = py
        .detach(|| Performance::new(panel))
        .map_err(analytics_to_py)?;
    Ok(PyPerformance { inner })
}

fn parse_period_kind(frequency: &str) -> PyResult<PeriodKind> {
    frequency.parse::<PeriodKind>().map_err(analytics_to_py)
}

fn parse_series_kind(kind: &str) -> PyResult<PanelSeriesKind> {
    match kind {
        "prices" => Ok(PanelSeriesKind::Prices),
        "returns" => Ok(PanelSeriesKind::Returns),
        other => Err(crate::errors::value_error(format!(
            "kind must be 'prices' or 'returns', got {other:?}"
        ))),
    }
}

#[doc = pydoc!("finstack_quant.analytics._performance", "PerformancePanel")]
#[pyclass(
    name = "PerformancePanel",
    module = "finstack_quant.analytics",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, PartialEq)]
pub(super) struct PyPerformancePanel {
    pub(crate) inner: PerformancePanel,
}

py_getters!(PyPerformancePanel |s| {
    #[doc = pydoc!("finstack_quant.analytics._performance", "PerformancePanel.series")]
    series -> Vec<Vec<f64>> = s.inner.series.clone();
    #[doc = pydoc!("finstack_quant.analytics._performance", "PerformancePanel.kind")]
    kind -> &'static str = match s.inner.kind {
        PanelSeriesKind::Prices => "prices",
        PanelSeriesKind::Returns => "returns",
    };
    #[doc = pydoc!("finstack_quant.analytics._performance", "PerformancePanel.ticker_names")]
    ticker_names -> Vec<String> = s.inner.ticker_names.clone();
    #[doc = pydoc!("finstack_quant.analytics._performance", "PerformancePanel.benchmark_ticker")]
    benchmark_ticker -> Option<String> = s.inner.benchmark_ticker.clone();
    #[doc = pydoc!("finstack_quant.analytics._performance", "PerformancePanel.frequency")]
    frequency -> Option<String> = s.inner.frequency.map(|kind| kind.to_string());
});

#[pymethods]
impl PyPerformancePanel {
    #[new]
    #[pyo3(signature = (*, dates, series, kind, ticker_names, benchmark_ticker=None, frequency=None))]
    fn new(
        dates: Vec<Bound<'_, PyAny>>,
        series: Vec<Vec<f64>>,
        kind: &str,
        ticker_names: Vec<String>,
        benchmark_ticker: Option<String>,
        frequency: Option<&str>,
    ) -> PyResult<Self> {
        let panel = PerformancePanel {
            dates: dates.iter().map(py_to_date).collect::<PyResult<Vec<_>>>()?,
            series,
            kind: parse_series_kind(kind)?,
            ticker_names,
            benchmark_ticker,
            frequency: frequency.map(parse_period_kind).transpose()?,
        };
        Performance::new(&panel).map_err(analytics_to_py)?;
        Ok(Self { inner: panel })
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "PerformancePanel.dates")]
    #[getter]
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        dates_to_pylist(py, &self.inner.dates)
    }

    fn __repr__(&self) -> String {
        format!(
            "PerformancePanel(kind={:?}, tickers={:?}, dates={})",
            self.kind(),
            self.inner.ticker_names,
            self.inner.dates.len()
        )
    }
}

wire_methods!(
    PyPerformancePanel,
    PerformancePanel,
    "PerformancePanel",
    module = "finstack_quant.analytics._performance"
);

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

    fn series_out<'py>(
        &self,
        py: Python<'py>,
        name: &str,
        res: finstack_quant_core::Result<Vec<f64>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let values = res.map_err(analytics_to_py)?;
        values_to_series(py, values, self.inner.get_ticker_names(), name)
    }

    fn summary_table(
        &self,
        py: Python<'_>,
        risk_free_rate: f64,
        mar: f64,
        omega_threshold: f64,
        confidence: f64,
    ) -> PyResult<finstack_quant_core::table::TableEnvelope> {
        // The 22-metric table is assembled in Rust (`Performance::summary`),
        // so summary(), to_dataframe(), and WASM read the same rows; release
        // the GIL for the O(22 * n * m) walk.
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
    #[pyo3(signature = (prices, benchmark_ticker=None, frequency=None))]
    fn new(
        py: Python<'_>,
        prices: Bound<'_, PyAny>,
        benchmark_ticker: Option<&str>,
        frequency: Option<&str>,
    ) -> PyResult<Self> {
        performance_from_frame(
            py,
            &prices,
            PanelSeriesKind::Prices,
            benchmark_ticker,
            frequency,
            None,
        )
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.from_returns")]
    #[staticmethod]
    #[pyo3(signature = (returns, initial_date, benchmark_ticker=None, frequency=None))]
    fn from_returns(
        py: Python<'_>,
        returns: Bound<'_, PyAny>,
        initial_date: Bound<'_, PyAny>,
        benchmark_ticker: Option<&str>,
        frequency: Option<&str>,
    ) -> PyResult<Self> {
        performance_from_frame(
            py,
            &returns,
            PanelSeriesKind::Returns,
            benchmark_ticker,
            frequency,
            Some(py_to_date(&initial_date)?),
        )
    }

    #[doc = pydoc!("finstack_quant.analytics._performance", "Performance.from_panel")]
    #[staticmethod]
    fn from_panel(py: Python<'_>, panel: &PyPerformancePanel) -> PyResult<Self> {
        performance_from_panel(py, &panel.inner)
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
        self.series_out(py, "cagr", self.inner.cagr(day_count, context))
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
    #[pyo3(signature = (ticker_idx, factor_returns, risk_free_rate))]
    fn multi_factor_greeks(
        &self,
        py: Python<'_>,
        ticker_idx: &Bound<'_, PyAny>,
        factor_returns: &Bound<'_, PyAny>,
        risk_free_rate: f64,
    ) -> PyResult<PyMultiFactorResult> {
        let ticker_idx = self.resolve_ticker(ticker_idx)?;
        let factor_returns = match factor_returns.extract::<Vec<Vec<f64>>>() {
            Ok(factors) => factors,
            Err(_) => align_factor_panel(
                factor_returns,
                self.inner
                    .get_active_dates_for_ticker(ticker_idx)
                    .map_err(analytics_to_py)?,
            )?,
        };
        let refs: Vec<&[f64]> = factor_returns.iter().map(Vec::as_slice).collect();
        py.detach(|| {
            self.inner
                .multi_factor_greeks(ticker_idx, &refs, risk_free_rate)
        })
        .map(|r| PyMultiFactorResult { inner: r })
        .map_err(analytics_to_py)
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
        let table = self.summary_table(py, risk_free_rate, mar, omega_threshold, confidence)?;
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
        let table = self.summary_table(py, risk_free_rate, mar, omega_threshold, confidence)?;
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
    module = "finstack_quant.analytics._performance"
);

macro_rules! py_series_methods {
    (
        zero { $( $zmethod:ident => $zdoc:literal ),* $(,)? }
        one { $( $omethod:ident ( $oarg:ident : $oty:ty ) => $odoc:literal ),* $(,)? }
        two { $( $tmethod:ident ( $ta:ident : $taty:ty, $tb:ident : $tbty:ty ) => $tdoc:literal ),* $(,)? }
        two_opt { $( $pmethod:ident ( $pa:ident : $paty:ty, $pb:ident : $pbty:ty = None ) => $pdoc:literal ),* $(,)? }
    ) => {
        #[pymethods]
        impl PyPerformance {
            $(
                #[doc = pydoc!("finstack_quant.analytics._performance", $zdoc)]
                fn $zmethod<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
                    self.series_out(py, stringify!($zmethod), self.inner.$zmethod())
                }
            )*
            $(
                #[doc = pydoc!("finstack_quant.analytics._performance", $odoc)]
                #[pyo3(signature = ($oarg))]
                fn $omethod<'py>(&self, py: Python<'py>, $oarg: $oty) -> PyResult<Bound<'py, PyAny>> {
                    self.series_out(py, stringify!($omethod), self.inner.$omethod($oarg))
                }
            )*
            $(
                #[doc = pydoc!("finstack_quant.analytics._performance", $tdoc)]
                #[pyo3(signature = ($ta, $tb))]
                fn $tmethod<'py>(&self, py: Python<'py>, $ta: $taty, $tb: $tbty) -> PyResult<Bound<'py, PyAny>> {
                    self.series_out(py, stringify!($tmethod), self.inner.$tmethod($ta, $tb))
                }
            )*
            $(
                #[doc = pydoc!("finstack_quant.analytics._performance", $pdoc)]
                #[pyo3(signature = ($pa, $pb = None))]
                fn $pmethod<'py>(&self, py: Python<'py>, $pa: $paty, $pb: $pbty) -> PyResult<Bound<'py, PyAny>> {
                    self.series_out(py, stringify!($pmethod), self.inner.$pmethod($pa, $pb))
                }
            )*
        }
    };
}

macro_rules! py_rolling_methods {
    (
        two { $( $rmethod2:ident => $rdoc2:literal ),* $(,)? }
        three { $( $rmethod3:ident ( $rarg:ident : $rty:ty ) => $rdoc3:literal ),* $(,)? }
    ) => {
        #[pymethods]
        impl PyPerformance {
            $(
                #[doc = pydoc!("finstack_quant.analytics._performance", $rdoc2)]
                #[pyo3(signature = (ticker_idx, window))]
                fn $rmethod2(
                    &self,
                    py: Python<'_>,
                    ticker_idx: &Bound<'_, PyAny>,
                    window: usize,
                ) -> PyResult<PyDatedSeries> {
                    let ticker_idx = self.resolve_ticker(ticker_idx)?;
                    let inner = py
                        .detach(|| self.inner.$rmethod2(ticker_idx, window))
                        .map_err(analytics_to_py)?;
                    Ok(PyDatedSeries { inner })
                }
            )*
            $(
                #[doc = pydoc!("finstack_quant.analytics._performance", $rdoc3)]
                #[pyo3(signature = (ticker_idx, window, $rarg))]
                fn $rmethod3(
                    &self,
                    py: Python<'_>,
                    ticker_idx: &Bound<'_, PyAny>,
                    window: usize,
                    $rarg: $rty,
                ) -> PyResult<PyDatedSeries> {
                    let ticker_idx = self.resolve_ticker(ticker_idx)?;
                    let inner = py
                        .detach(|| self.inner.$rmethod3(ticker_idx, window, $rarg))
                        .map_err(analytics_to_py)?;
                    Ok(PyDatedSeries { inner })
                }
            )*
        }
    };
}

py_series_methods!(
    zero {
        calmar => "Performance.calmar",
        max_drawdown => "Performance.max_drawdown",
        mean_drawdown => "Performance.mean_drawdown",
        tracking_error => "Performance.tracking_error",
        information_ratio => "Performance.information_ratio",
        skewness => "Performance.skewness",
        kurtosis => "Performance.kurtosis",
        geometric_mean => "Performance.geometric_mean",
        up_capture => "Performance.up_capture",
        down_capture => "Performance.down_capture",
        capture_ratio => "Performance.capture_ratio",
        gain_to_pain => "Performance.gain_to_pain",
        ulcer_index => "Performance.ulcer_index",
        recovery_factor => "Performance.recovery_factor",
        pain_index => "Performance.pain_index",
        r_squared => "Performance.r_squared",
        batting_average => "Performance.batting_average",
    }
    one {
        mean_return(annualize: bool) => "Performance.mean_return",
        volatility(annualize: bool) => "Performance.volatility",
        sharpe(risk_free_rate: f64) => "Performance.sharpe",
        sortino(mar: f64) => "Performance.sortino",
        value_at_risk(confidence: f64) => "Performance.value_at_risk",
        expected_shortfall(confidence: f64) => "Performance.expected_shortfall",
        downside_deviation(mar: f64) => "Performance.downside_deviation",
        omega_ratio(threshold: f64) => "Performance.omega_ratio",
        treynor(risk_free_rate: f64) => "Performance.treynor",
        martin_ratio(risk_free_rate: f64) => "Performance.martin_ratio",
        pain_ratio(risk_free_rate: f64) => "Performance.pain_ratio",
        tail_ratio(confidence: f64) => "Performance.tail_ratio",
        cdar(confidence: f64) => "Performance.cdar",
        m_squared(risk_free_rate: f64) => "Performance.m_squared",
        burke_ratio(risk_free_rate: f64) => "Performance.burke_ratio",
    }
    two {
        modified_sharpe(risk_free_rate: f64, confidence: f64) => "Performance.modified_sharpe",
        sterling_ratio(risk_free_rate: f64, n: usize) => "Performance.sterling_ratio",
    }
    two_opt {
        parametric_var(confidence: f64, horizon_periods: Option<u64> = None) => "Performance.parametric_var",
        cornish_fisher_var(confidence: f64, horizon_periods: Option<u64> = None) => "Performance.cornish_fisher_var",
    }
);

py_rolling_methods!(
    two {
        rolling_volatility => "Performance.rolling_volatility",
        rolling_returns => "Performance.rolling_returns",
    }
    three {
        rolling_sortino(mar: f64) => "Performance.rolling_sortino",
        rolling_sharpe(risk_free_rate: f64) => "Performance.rolling_sharpe",
    }
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
    factor_returns: Vec<Vec<f64>>,
    periods_per_year: f64,
    return_kind: &str,
    risk_free_rate: Option<f64>,
) -> PyResult<PyMultiFactorResult> {
    let returns = extract_f64_vec(returns, "returns")?;
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
    m.add_class::<PyPerformancePanel>()?;
    m.add_function(wrap_pyfunction!(sharpe, m)?)?;
    m.add_function(wrap_pyfunction!(sortino, m)?)?;
    m.add_function(wrap_pyfunction!(volatility, m)?)?;
    m.add_function(wrap_pyfunction!(max_drawdown, m)?)?;
    m.add_function(wrap_pyfunction!(multi_factor_greeks, m)?)?;
    m.add_function(wrap_pyfunction!(validate_summary, m)?)?;
    Ok(())
}
