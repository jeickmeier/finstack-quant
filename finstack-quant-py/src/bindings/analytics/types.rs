//! Result structs and enums for the analytics domain.

use crate::bindings::date_utils::date_to_py;
use crate::bindings::pandas_utils::{
    dates_to_datetime_index, dict_to_dataframe, labeled_values_to_series,
};
use crate::errors::display_to_py;
use finstack_quant_analytics as fa;
use numpy::PyArray1;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde::de::DeserializeOwned;
use serde::Serialize;

#[inline]
fn slice_to_pyarray<'py>(py: Python<'py>, values: &[f64]) -> Bound<'py, PyArray1<f64>> {
    PyArray1::from_slice(py, values)
}

fn serde_from_json<T: DeserializeOwned>(json: &str) -> PyResult<T> {
    serde_json::from_str(json).map_err(display_to_py)
}

fn serde_to_json<T: Serialize>(value: &T) -> PyResult<String> {
    serde_json::to_string(value).map_err(display_to_py)
}

fn pickle_reduce_json<'py, T: pyo3::PyTypeInfo>(
    py: Python<'py>,
    json: String,
) -> PyResult<(Bound<'py, PyAny>, (String,))> {
    let from_json = py.get_type::<T>().getattr("from_json")?;
    crate::bindings::pickle_support::reduce_via_json(from_json, json)
}

/// Aggregated statistics for grouped periodic returns.
#[pyclass(name = "PeriodStats", module = "finstack_quant.analytics", frozen)]
pub struct PyPeriodStats {
    pub(super) inner: fa::PeriodStats,
}

#[pymethods]
impl PyPeriodStats {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        pickle_reduce_json::<Self>(py, self.to_json()?)
    }

    /// Deserialize from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_from_json(json)?,
        })
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_to_json(&self.inner)
    }

    /// Best period return.
    #[getter]
    fn best(&self) -> f64 {
        self.inner.best
    }
    /// Worst period return.
    #[getter]
    fn worst(&self) -> f64 {
        self.inner.worst
    }
    /// Longest consecutive winning streak.
    #[getter]
    fn consecutive_wins(&self) -> usize {
        self.inner.consecutive_wins
    }
    /// Longest consecutive losing streak.
    #[getter]
    fn consecutive_losses(&self) -> usize {
        self.inner.consecutive_losses
    }
    /// Fraction of positive-return periods.
    #[getter]
    fn win_rate(&self) -> f64 {
        self.inner.win_rate
    }
    /// Average return across all periods.
    #[getter]
    fn avg_return(&self) -> f64 {
        self.inner.avg_return
    }
    /// Average return of positive periods.
    #[getter]
    fn avg_win(&self) -> f64 {
        self.inner.avg_win
    }
    /// Average return of negative periods.
    #[getter]
    fn avg_loss(&self) -> f64 {
        self.inner.avg_loss
    }
    /// Payoff ratio (avg win / |avg loss|).
    #[getter]
    fn payoff_ratio(&self) -> f64 {
        self.inner.payoff_ratio
    }
    /// Profit factor (gross profits / gross losses).
    #[getter]
    fn profit_factor(&self) -> f64 {
        self.inner.profit_factor
    }
    /// Common-sense ratio (CPC).
    #[getter]
    fn cpc_ratio(&self) -> f64 {
        self.inner.cpc_ratio
    }
    /// Kelly criterion optimal fraction.
    #[getter]
    fn kelly_criterion(&self) -> f64 {
        self.inner.kelly_criterion
    }

    /// The twelve statistics as a ``pandas.Series`` named ``period_stats``
    /// and indexed by statistic name (``best``, ``worst``, ``win_rate``, ...).
    ///
    /// Streak counts are cast to ``float`` so the Series stays ``float64``.
    fn to_series<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let labels: Vec<String> = PERIOD_STATS_COLUMNS
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        labeled_values_to_series(py, &labels, self.values(), "period_stats")
    }

    /// The twelve statistics as a single-row ``pandas.DataFrame``.
    ///
    /// Columns: ``best``, ``worst``, ``consecutive_wins``,
    /// ``consecutive_losses``, ``win_rate``, ``avg_return``, ``avg_win``,
    /// ``avg_loss``, ``payoff_ratio``, ``profit_factor``, ``cpc_ratio``,
    /// ``kelly_criterion``. One row per ticker after ``pd.concat`` across
    /// tickers. Non-finite ratios remain native ``NaN`` / ``inf`` in
    /// ``float64`` columns; streak counts remain integers.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let s = &self.inner;
        let data = PyDict::new(py);
        data.set_item("best", slice_to_pyarray(py, &[s.best]))?;
        data.set_item("worst", slice_to_pyarray(py, &[s.worst]))?;
        data.set_item("consecutive_wins", vec![s.consecutive_wins])?;
        data.set_item("consecutive_losses", vec![s.consecutive_losses])?;
        data.set_item("win_rate", slice_to_pyarray(py, &[s.win_rate]))?;
        data.set_item("avg_return", slice_to_pyarray(py, &[s.avg_return]))?;
        data.set_item("avg_win", slice_to_pyarray(py, &[s.avg_win]))?;
        data.set_item("avg_loss", slice_to_pyarray(py, &[s.avg_loss]))?;
        data.set_item("payoff_ratio", slice_to_pyarray(py, &[s.payoff_ratio]))?;
        data.set_item("profit_factor", slice_to_pyarray(py, &[s.profit_factor]))?;
        data.set_item("cpc_ratio", slice_to_pyarray(py, &[s.cpc_ratio]))?;
        data.set_item(
            "kelly_criterion",
            slice_to_pyarray(py, &[s.kelly_criterion]),
        )?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "PeriodStats(win_rate={:.4}, avg_return={:.6})",
            self.inner.win_rate, self.inner.avg_return
        )
    }

    /// Render as an HTML table in Jupyter notebooks (delegates to
    /// ``to_dataframe``; ``None`` falls back to ``__repr__``).
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

impl PyPeriodStats {
    /// Statistic values in `PERIOD_STATS_COLUMNS` order.
    fn values(&self) -> Vec<f64> {
        let s = &self.inner;
        vec![
            s.best,
            s.worst,
            s.consecutive_wins as f64,
            s.consecutive_losses as f64,
            s.win_rate,
            s.avg_return,
            s.avg_win,
            s.avg_loss,
            s.payoff_ratio,
            s.profit_factor,
            s.cpc_ratio,
            s.kelly_criterion,
        ]
    }
}

/// Column order shared by `PeriodStats.to_series` / `to_dataframe`.
const PERIOD_STATS_COLUMNS: &[&str] = &[
    "best",
    "worst",
    "consecutive_wins",
    "consecutive_losses",
    "win_rate",
    "avg_return",
    "avg_win",
    "avg_loss",
    "payoff_ratio",
    "profit_factor",
    "cpc_ratio",
    "kelly_criterion",
];

/// Regression beta with confidence interval.
#[pyclass(name = "BetaResult", module = "finstack_quant.analytics", frozen)]
pub struct PyBetaResult {
    pub(super) inner: fa::BetaResult,
}

#[pymethods]
impl PyBetaResult {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        pickle_reduce_json::<Self>(py, self.to_json()?)
    }

    /// Deserialize from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_from_json(json)?,
        })
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_to_json(&self.inner)
    }

    /// Beta coefficient.
    #[getter]
    fn beta(&self) -> f64 {
        self.inner.beta
    }
    /// Standard error of the beta estimate.
    #[getter]
    fn std_err(&self) -> f64 {
        self.inner.std_err
    }
    /// Lower 95% confidence bound.
    #[getter]
    fn ci_lower(&self) -> f64 {
        self.inner.ci_lower
    }
    /// Upper 95% confidence bound.
    #[getter]
    fn ci_upper(&self) -> f64 {
        self.inner.ci_upper
    }

    /// Export as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``beta``, ``std_err``, ``ci_lower``, ``ci_upper``.
    ///
    /// One flat record describes one regression, so a one-row frame is the
    /// right shape: ``pd.concat([r.to_dataframe() for r in results])`` stacks
    /// every ticker's beta into one comparison table without reshaping.
    ///
    /// Undefined estimates from a degenerate regression remain ``NaN``
    /// in numeric ``float64`` columns.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        beta_to_dataframe(py, std::slice::from_ref(&self.inner), None)
    }

    fn __repr__(&self) -> String {
        format!(
            "BetaResult(beta={:.4}, se={:.4}, ci=[{:.4}, {:.4}])",
            self.inner.beta, self.inner.std_err, self.inner.ci_lower, self.inner.ci_upper
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Alpha, beta, and R-squared from a single-index regression.
#[pyclass(name = "GreeksResult", module = "finstack_quant.analytics", frozen)]
pub struct PyGreeksResult {
    pub(super) inner: fa::GreeksResult,
}

#[pymethods]
impl PyGreeksResult {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        pickle_reduce_json::<Self>(py, self.to_json()?)
    }

    /// Deserialize from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_from_json(json)?,
        })
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_to_json(&self.inner)
    }

    /// Annualized Jensen alpha.
    #[getter]
    fn alpha(&self) -> f64 {
        self.inner.alpha
    }
    /// Beta coefficient.
    #[getter]
    fn beta(&self) -> f64 {
        self.inner.beta
    }
    /// Coefficient of determination of the fitted model.
    #[getter]
    fn r_squared(&self) -> f64 {
        self.inner.r_squared
    }
    /// Adjusted R-squared.
    #[getter]
    fn adjusted_r_squared(&self) -> f64 {
        self.inner.adjusted_r_squared
    }

    /// Export as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``alpha``, ``beta``, ``r_squared``, ``adjusted_r_squared``.
    ///
    /// One flat record describes one regression, so a one-row frame is the
    /// right shape: ``pd.concat([r.to_dataframe() for r in results])`` stacks
    /// every ticker's greeks into one comparison table without reshaping.
    ///
    /// Undefined estimates from a degenerate fit remain ``NaN`` in numeric
    /// ``float64`` columns.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        greeks_to_dataframe(py, std::slice::from_ref(&self.inner), None)
    }

    fn __repr__(&self) -> String {
        format!(
            "GreeksResult(alpha={:.6}, beta={:.4}, r2={:.4}, adj_r2={:.4})",
            self.inner.alpha, self.inner.beta, self.inner.r_squared, self.inner.adjusted_r_squared
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Rolling alpha and beta time series.
#[pyclass(name = "RollingGreeks", module = "finstack_quant.analytics", frozen)]
pub struct PyRollingGreeks {
    pub(super) inner: fa::RollingGreeks,
}

#[pymethods]
impl PyRollingGreeks {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        pickle_reduce_json::<Self>(py, self.to_json()?)
    }

    /// Deserialize from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_from_json(json)?,
        })
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_to_json(&self.inner)
    }

    /// Date labels for each rolling window.
    #[getter]
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        self.inner
            .dates
            .iter()
            .map(|&d| date_to_py(py, d))
            .collect()
    }
    /// Rolling alpha values.
    #[getter]
    fn alphas<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        slice_to_pyarray(py, &self.inner.alphas)
    }
    /// Rolling beta values.
    #[getter]
    fn betas<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        slice_to_pyarray(py, &self.inner.betas)
    }

    /// Convert to a pandas ``DataFrame`` with a ``DatetimeIndex`` and
    /// ``alpha`` / ``beta`` columns.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("alpha", slice_to_pyarray(py, &self.inner.alphas))?;
        data.set_item("beta", slice_to_pyarray(py, &self.inner.betas))?;
        let idx = dates_to_datetime_index(py, &self.inner.dates)?;
        dict_to_dataframe(py, &data, Some(idx))
    }

    fn __repr__(&self) -> String {
        format!("RollingGreeks(len={})", self.inner.dates.len())
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Multi-factor regression result.
#[pyclass(
    name = "MultiFactorResult",
    module = "finstack_quant.analytics",
    frozen
)]
pub struct PyMultiFactorResult {
    pub(super) inner: fa::MultiFactorResult,
}

#[pymethods]
impl PyMultiFactorResult {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        pickle_reduce_json::<Self>(py, self.to_json()?)
    }

    /// Deserialize from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_from_json(json)?,
        })
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_to_json(&self.inner)
    }

    /// Annualized OLS intercept of the (possibly rf-adjusted) dependent series.
    #[getter]
    fn alpha(&self) -> f64 {
        self.inner.alpha
    }
    /// One beta per factor, in factor order.
    #[getter]
    fn betas<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        slice_to_pyarray(py, &self.inner.betas)
    }
    /// Coefficient of determination of the fitted model.
    #[getter]
    fn r_squared(&self) -> f64 {
        self.inner.r_squared
    }
    /// Adjusted R-squared.
    #[getter]
    fn adjusted_r_squared(&self) -> f64 {
        self.inner.adjusted_r_squared
    }
    /// Residual volatility.
    #[getter]
    fn residual_vol(&self) -> f64 {
        self.inner.residual_vol
    }

    /// Export the factor loadings as a pandas ``DataFrame``, one row per factor.
    ///
    /// Columns: ``factor``, ``beta``, ``alpha``, ``r_squared``,
    /// ``adjusted_r_squared``, ``residual_vol``.
    ///
    /// The loadings are the per-row payload; the four regression-level
    /// statistics repeat on every row so a single row carries its own fit
    /// context after ``pd.concat`` across tickers or ``groupby("factor")``.
    ///
    /// Rows follow the order of the ``factor_returns`` passed to
    /// :meth:`Performance.multi_factor_greeks`, which is also the order of
    /// :attr:`betas`. There is always at least one row: the regression rejects
    /// an empty factor set.
    ///
    /// Parameters
    /// ----------
    /// factor_names : list[str], optional
    ///     Labels for the ``factor`` column, positionally aligned with
    ///     :attr:`betas`. Defaults to ``factor_0``, ``factor_1``, ... because
    ///     the regression itself carries no names.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``factor_names`` is supplied and its length differs from the
    ///     number of fitted betas.
    #[pyo3(signature = (factor_names=None))]
    #[pyo3(text_signature = "(self, factor_names=None)")]
    fn to_dataframe<'py>(
        &self,
        py: Python<'py>,
        factor_names: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let betas = &self.inner.betas;
        let names = match factor_names {
            Some(names) if names.len() != betas.len() => {
                return Err(crate::errors::value_error(format!(
                    "factor_names has {} entries but the regression fitted {} betas",
                    names.len(),
                    betas.len()
                )));
            }
            Some(names) => names,
            None => (0..betas.len()).map(|i| format!("factor_{i}")).collect(),
        };
        let data = PyDict::new(py);
        let kwargs = PyDict::new(py);
        kwargs.set_item("dtype", "str")?;
        let factors = py
            .import("pandas")?
            .getattr("Series")?
            .call((names,), Some(&kwargs))?;
        data.set_item("factor", factors)?;
        data.set_item("beta", slice_to_pyarray(py, betas))?;
        data.set_item(
            "alpha",
            PyArray1::from_vec(py, vec![self.inner.alpha; betas.len()]),
        )?;
        data.set_item(
            "r_squared",
            PyArray1::from_vec(py, vec![self.inner.r_squared; betas.len()]),
        )?;
        data.set_item(
            "adjusted_r_squared",
            PyArray1::from_vec(py, vec![self.inner.adjusted_r_squared; betas.len()]),
        )?;
        data.set_item(
            "residual_vol",
            PyArray1::from_vec(py, vec![self.inner.residual_vol; betas.len()]),
        )?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "MultiFactorResult(alpha={:.6}, r2={:.4}, adj_r2={:.4})",
            self.inner.alpha, self.inner.r_squared, self.inner.adjusted_r_squared
        )
    }
}

/// A single drawdown episode with timing and depth information.
#[pyclass(name = "DrawdownEpisode", module = "finstack_quant.analytics", frozen)]
pub struct PyDrawdownEpisode {
    pub(super) inner: fa::DrawdownEpisode,
}

#[pymethods]
impl PyDrawdownEpisode {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        pickle_reduce_json::<Self>(py, self.to_json()?)
    }

    /// Deserialize from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_from_json(json)?,
        })
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_to_json(&self.inner)
    }

    /// Start date of the drawdown.
    #[getter]
    fn start<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start)
    }
    /// Date of the maximum drawdown within this episode.
    #[getter]
    fn valley<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.valley)
    }
    /// Recovery date (``None`` if still in drawdown).
    #[getter]
    fn end<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match self.inner.end {
            Some(d) => date_to_py(py, d).map(Some),
            None => Ok(None),
        }
    }
    /// Duration in calendar days.
    #[getter]
    fn duration_days(&self) -> i64 {
        self.inner.duration_days
    }
    /// Maximum drawdown depth (negative).
    #[getter]
    fn max_drawdown(&self) -> f64 {
        self.inner.max_drawdown
    }
    /// Near-recovery threshold.
    #[getter]
    fn near_recovery_threshold(&self) -> f64 {
        self.inner.near_recovery_threshold
    }
    /// ``True`` when the episode began before the first observation
    /// (left-censored: no prior peak observed).
    #[getter]
    fn truncated_at_start(&self) -> bool {
        self.inner.truncated_at_start
    }

    /// Export as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``start``, ``valley``, ``end`` (``datetime64``; ``NaT`` while
    /// still in drawdown), ``duration_days``, ``max_drawdown``,
    /// ``near_recovery_threshold``, ``truncated_at_start``. Stack episodes
    /// with ``pd.concat`` or use
    /// ``Performance.to_drawdown_details_dataframe`` for the top-N table.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        drawdowns_to_dataframe(py, std::slice::from_ref(&self.inner))
    }

    fn __repr__(&self) -> String {
        format!(
            "DrawdownEpisode(dd={:.4}, days={})",
            self.inner.max_drawdown, self.inner.duration_days
        )
    }

    /// Render as an HTML table in Jupyter notebooks (delegates to
    /// ``to_dataframe``; ``None`` falls back to ``__repr__``).
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Period-to-date returns for each ticker.
#[pyclass(name = "LookbackReturns", module = "finstack_quant.analytics", frozen)]
pub struct PyLookbackReturns {
    pub(super) inner: fa::LookbackReturns,
}

#[pymethods]
impl PyLookbackReturns {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        pickle_reduce_json::<Self>(py, self.to_json()?)
    }

    /// Deserialize from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_from_json(json)?,
        })
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_to_json(&self.inner)
    }

    /// Ticker names aligned with the ``mtd`` / ``qtd`` / ``ytd`` / ``fytd``
    /// vectors.
    #[getter]
    fn ticker_names(&self) -> Vec<String> {
        self.inner.ticker_names.clone()
    }
    /// Month-to-date returns per ticker.
    #[getter]
    fn mtd<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        slice_to_pyarray(py, &self.inner.mtd)
    }
    /// Quarter-to-date returns per ticker.
    #[getter]
    fn qtd<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        slice_to_pyarray(py, &self.inner.qtd)
    }
    /// Year-to-date returns per ticker.
    #[getter]
    fn ytd<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        slice_to_pyarray(py, &self.inner.ytd)
    }
    /// Fiscal-year-to-date returns per ticker.
    #[getter]
    fn fytd<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        slice_to_pyarray(py, &self.inner.fytd)
    }

    /// Convert to a pandas ``DataFrame`` indexed by :attr:`ticker_names`.
    ///
    /// Columns: mtd, qtd, ytd, and fytd.
    pub(super) fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("mtd", slice_to_pyarray(py, &self.inner.mtd))?;
        data.set_item("qtd", slice_to_pyarray(py, &self.inner.qtd))?;
        data.set_item("ytd", slice_to_pyarray(py, &self.inner.ytd))?;
        data.set_item("fytd", slice_to_pyarray(py, &self.inner.fytd))?;
        let idx = self
            .inner
            .ticker_names
            .clone()
            .into_pyobject(py)?
            .into_any();
        dict_to_dataframe(py, &data, Some(idx))
    }

    /// Render as an HTML table in Jupyter notebooks (delegates to
    /// ``to_dataframe``; ``None`` falls back to ``__repr__``).
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }

    fn __repr__(&self) -> String {
        format!(
            "LookbackReturns(mtd_len={}, fytd_len={})",
            self.inner.mtd.len(),
            self.inner.fytd.len()
        )
    }
}

/// Date-indexed numeric series returned by the rolling-window analytics.
///
/// Wraps Rust `DatedSeries`, whose `value_column` (a `RollingMetric`) names the
/// metric and becomes the DataFrame column name.
#[pyclass(name = "DatedSeries", module = "finstack_quant.analytics", frozen)]
pub struct PyDatedSeries {
    pub(super) inner: fa::DatedSeries,
}

#[pymethods]
impl PyDatedSeries {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        pickle_reduce_json::<Self>(py, self.to_json()?)
    }

    /// Deserialize from JSON.
    ///
    /// Expects the Rust `DatedSeries` shape (`values` / `dates` /
    /// `value_column`) emitted by [`to_json`](Self::to_json).
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: fa::DatedSeries = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to compact JSON (the Rust `DatedSeries` serde form).
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Numeric values, one per window.
    #[getter]
    fn values<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        slice_to_pyarray(py, &self.inner.values)
    }
    /// Window-end dates aligned 1:1 with [`values`](Self::values).
    #[getter]
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        self.inner
            .dates
            .iter()
            .map(|&d| date_to_py(py, d))
            .collect()
    }
    /// Metric name of the series (Rust `RollingMetric`), also the column name
    /// used by [`to_dataframe`](Self::to_dataframe).
    #[getter]
    fn value_column(&self) -> &'static str {
        self.inner.value_column.as_str()
    }

    /// Convert to a pandas ``DataFrame`` with a ``DatetimeIndex`` and a
    /// single value column named after [`value_column`](Self::value_column).
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item(
            self.inner.value_column.as_str(),
            slice_to_pyarray(py, &self.inner.values),
        )?;
        let idx = dates_to_datetime_index(py, &self.inner.dates)?;
        dict_to_dataframe(py, &data, Some(idx))
    }

    fn __repr__(&self) -> String {
        format!(
            "DatedSeries(name={:?}, len={})",
            self.inner.value_column.as_str(),
            self.inner.values.len()
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Native numeric beta columns, shared by the leaf and panel pandas exits.
///
/// # Arguments
/// * `py` - Interpreter token used to allocate NumPy columns and the pandas frame.
/// * `rows` - Canonical Rust beta results, in output row order.
/// * `index` - Optional row labels; omitted labels use pandas' default integer index.
pub(super) fn beta_to_dataframe<'py>(
    py: Python<'py>,
    rows: &[fa::BetaResult],
    index: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let data = PyDict::new(py);
    data.set_item(
        "beta",
        PyArray1::from_vec(py, rows.iter().map(|r| r.beta).collect()),
    )?;
    data.set_item(
        "std_err",
        PyArray1::from_vec(py, rows.iter().map(|r| r.std_err).collect()),
    )?;
    data.set_item(
        "ci_lower",
        PyArray1::from_vec(py, rows.iter().map(|r| r.ci_lower).collect()),
    )?;
    data.set_item(
        "ci_upper",
        PyArray1::from_vec(py, rows.iter().map(|r| r.ci_upper).collect()),
    )?;
    dict_to_dataframe(py, &data, index)
}

/// Native numeric Greeks columns, shared by the leaf and panel pandas exits.
///
/// # Arguments
/// * `py` - Interpreter token used to allocate NumPy columns and the pandas frame.
/// * `rows` - Canonical Rust regression results, in output row order.
/// * `index` - Optional row labels; omitted labels use pandas' default integer index.
pub(super) fn greeks_to_dataframe<'py>(
    py: Python<'py>,
    rows: &[fa::GreeksResult],
    index: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let data = PyDict::new(py);
    data.set_item(
        "alpha",
        PyArray1::from_vec(py, rows.iter().map(|r| r.alpha).collect()),
    )?;
    data.set_item(
        "beta",
        PyArray1::from_vec(py, rows.iter().map(|r| r.beta).collect()),
    )?;
    data.set_item(
        "r_squared",
        PyArray1::from_vec(py, rows.iter().map(|r| r.r_squared).collect()),
    )?;
    data.set_item(
        "adjusted_r_squared",
        PyArray1::from_vec(py, rows.iter().map(|r| r.adjusted_r_squared).collect()),
    )?;
    dict_to_dataframe(py, &data, index)
}

/// Typed episode columns, including stable dtypes when no episode is present.
///
/// # Arguments
/// * `py` - Interpreter token used to allocate date, numeric and boolean columns.
/// * `episodes` - Canonical Rust episodes, retained in their existing severity order.
pub(super) fn drawdowns_to_dataframe<'py>(
    py: Python<'py>,
    episodes: &[fa::DrawdownEpisode],
) -> PyResult<Bound<'py, PyAny>> {
    let data = PyDict::new(py);
    let starts: Vec<_> = episodes.iter().map(|e| e.start).collect();
    let valleys: Vec<_> = episodes.iter().map(|e| e.valley).collect();
    let ends = episodes
        .iter()
        .map(|e| match e.end {
            Some(date) => date_to_py(py, date).map(|value| value.into_any()),
            None => Ok(py.None().into_bound(py)),
        })
        .collect::<PyResult<Vec<_>>>()?;
    data.set_item("start", dates_to_datetime_index(py, &starts)?)?;
    data.set_item("valley", dates_to_datetime_index(py, &valleys)?)?;
    data.set_item(
        "end",
        py.import("pandas")?.call_method1("to_datetime", (ends,))?,
    )?;
    data.set_item(
        "duration_days",
        PyArray1::from_vec(py, episodes.iter().map(|e| e.duration_days).collect()),
    )?;
    data.set_item(
        "max_drawdown",
        PyArray1::from_vec(py, episodes.iter().map(|e| e.max_drawdown).collect()),
    )?;
    data.set_item(
        "near_recovery_threshold",
        PyArray1::from_vec(
            py,
            episodes.iter().map(|e| e.near_recovery_threshold).collect(),
        ),
    )?;
    data.set_item(
        "truncated_at_start",
        PyArray1::from_vec(py, episodes.iter().map(|e| e.truncated_at_start).collect()),
    )?;
    dict_to_dataframe(py, &data, None)
}

pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPeriodStats>()?;
    m.add_class::<PyBetaResult>()?;
    m.add_class::<PyGreeksResult>()?;
    m.add_class::<PyRollingGreeks>()?;
    m.add_class::<PyMultiFactorResult>()?;
    m.add_class::<PyDrawdownEpisode>()?;
    m.add_class::<PyLookbackReturns>()?;
    m.add_class::<PyDatedSeries>()?;
    let _ = py;
    Ok(())
}
