//! Result structs and enums for the analytics domain.

use crate::bindings::date_utils::date_to_py;
use crate::bindings::macros::{impl_repr_html_via_dataframe, wire_methods};
use crate::bindings::pandas_utils::{
    dates_to_datetime_index, dates_to_pylist, dict_to_dataframe, labeled_values_to_series,
};
use crate::bindings::pydoc::pydoc;
use finstack_quant_analytics::{
    BetaResult, DatedSeries, DrawdownEpisode, GreeksResult, LookbackReturns, MultiFactorResult,
    PeriodStats, RollingGreeks,
};
use numpy::PyArray1;
use pyo3::prelude::*;
use pyo3::types::PyDict;

#[doc = pydoc!("finstack_quant.analytics", "PeriodStats")]
#[pyclass(name = "PeriodStats", module = "finstack_quant.analytics", frozen)]
pub struct PyPeriodStats {
    pub(super) inner: PeriodStats,
}

#[pymethods]
impl PyPeriodStats {
    #[doc = pydoc!("finstack_quant.analytics", "PeriodStats.best")]
    #[getter]
    fn best(&self) -> f64 {
        self.inner.best
    }
    #[doc = pydoc!("finstack_quant.analytics", "PeriodStats.worst")]
    #[getter]
    fn worst(&self) -> f64 {
        self.inner.worst
    }
    #[doc = pydoc!("finstack_quant.analytics", "PeriodStats.consecutive_wins")]
    #[getter]
    fn consecutive_wins(&self) -> usize {
        self.inner.consecutive_wins
    }
    #[doc = pydoc!("finstack_quant.analytics", "PeriodStats.consecutive_losses")]
    #[getter]
    fn consecutive_losses(&self) -> usize {
        self.inner.consecutive_losses
    }
    #[doc = pydoc!("finstack_quant.analytics", "PeriodStats.win_rate")]
    #[getter]
    fn win_rate(&self) -> f64 {
        self.inner.win_rate
    }
    #[doc = pydoc!("finstack_quant.analytics", "PeriodStats.avg_return")]
    #[getter]
    fn avg_return(&self) -> f64 {
        self.inner.avg_return
    }
    #[doc = pydoc!("finstack_quant.analytics", "PeriodStats.avg_win")]
    #[getter]
    fn avg_win(&self) -> f64 {
        self.inner.avg_win
    }
    #[doc = pydoc!("finstack_quant.analytics", "PeriodStats.avg_loss")]
    #[getter]
    fn avg_loss(&self) -> f64 {
        self.inner.avg_loss
    }
    #[doc = pydoc!("finstack_quant.analytics", "PeriodStats.payoff_ratio")]
    #[getter]
    fn payoff_ratio(&self) -> f64 {
        self.inner.payoff_ratio
    }
    #[doc = pydoc!("finstack_quant.analytics", "PeriodStats.profit_factor")]
    #[getter]
    fn profit_factor(&self) -> f64 {
        self.inner.profit_factor
    }
    #[doc = pydoc!("finstack_quant.analytics", "PeriodStats.cpc_ratio")]
    #[getter]
    fn cpc_ratio(&self) -> f64 {
        self.inner.cpc_ratio
    }
    #[doc = pydoc!("finstack_quant.analytics", "PeriodStats.kelly_criterion")]
    #[getter]
    fn kelly_criterion(&self) -> f64 {
        self.inner.kelly_criterion
    }

    #[doc = pydoc!("finstack_quant.analytics", "PeriodStats.to_series")]
    fn to_series<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let (labels, values): (Vec<String>, Vec<f64>) = self
            .fields()
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .unzip();
        labeled_values_to_series(py, &labels, values, "period_stats")
    }

    #[doc = pydoc!("finstack_quant.analytics", "PeriodStats.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        for (name, value) in self.fields() {
            data.set_item(name, PyArray1::from_slice(py, &[value]))?;
        }
        // Streak counts stay integer columns; re-setting a key keeps its position.
        data.set_item("consecutive_wins", vec![self.inner.consecutive_wins])?;
        data.set_item("consecutive_losses", vec![self.inner.consecutive_losses])?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "PeriodStats(win_rate={:.4}, avg_return={:.6})",
            self.inner.win_rate, self.inner.avg_return
        )
    }
}

impl PyPeriodStats {
    /// Statistic names and values in `PeriodStats` field order (streak
    /// counts as `f64`).
    fn fields(&self) -> [(&'static str, f64); 12] {
        let s = &self.inner;
        [
            ("best", s.best),
            ("worst", s.worst),
            ("consecutive_wins", s.consecutive_wins as f64),
            ("consecutive_losses", s.consecutive_losses as f64),
            ("win_rate", s.win_rate),
            ("avg_return", s.avg_return),
            ("avg_win", s.avg_win),
            ("avg_loss", s.avg_loss),
            ("payoff_ratio", s.payoff_ratio),
            ("profit_factor", s.profit_factor),
            ("cpc_ratio", s.cpc_ratio),
            ("kelly_criterion", s.kelly_criterion),
        ]
    }
}

#[doc = pydoc!("finstack_quant.analytics", "BetaResult")]
#[pyclass(name = "BetaResult", module = "finstack_quant.analytics", frozen)]
pub struct PyBetaResult {
    pub(super) inner: BetaResult,
}

#[pymethods]
impl PyBetaResult {
    #[doc = pydoc!("finstack_quant.analytics", "BetaResult.beta")]
    #[getter]
    fn beta(&self) -> f64 {
        self.inner.beta
    }
    #[doc = pydoc!("finstack_quant.analytics", "BetaResult.std_err")]
    #[getter]
    fn std_err(&self) -> f64 {
        self.inner.std_err
    }
    #[doc = pydoc!("finstack_quant.analytics", "BetaResult.ci_lower")]
    #[getter]
    fn ci_lower(&self) -> f64 {
        self.inner.ci_lower
    }
    #[doc = pydoc!("finstack_quant.analytics", "BetaResult.ci_upper")]
    #[getter]
    fn ci_upper(&self) -> f64 {
        self.inner.ci_upper
    }
    #[doc = pydoc!("finstack_quant.analytics", "BetaResult.n_obs")]
    #[getter]
    fn n_obs(&self) -> usize {
        self.inner.n_obs
    }
    #[doc = pydoc!("finstack_quant.analytics", "BetaResult.confidence_level")]
    #[getter]
    fn confidence_level(&self) -> f64 {
        self.inner.confidence_level
    }

    #[doc = pydoc!("finstack_quant.analytics", "BetaResult.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let r = &self.inner;
        let data = PyDict::new(py);
        data.set_item("beta", PyArray1::from_slice(py, &[r.beta]))?;
        data.set_item("std_err", PyArray1::from_slice(py, &[r.std_err]))?;
        data.set_item("ci_lower", PyArray1::from_slice(py, &[r.ci_lower]))?;
        data.set_item("ci_upper", PyArray1::from_slice(py, &[r.ci_upper]))?;
        data.set_item("n_obs", PyArray1::from_slice(py, &[r.n_obs as i64]))?;
        data.set_item(
            "confidence_level",
            PyArray1::from_slice(py, &[r.confidence_level]),
        )?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "BetaResult(beta={:.4}, se={:.4}, ci=[{:.4}, {:.4}])",
            self.inner.beta, self.inner.std_err, self.inner.ci_lower, self.inner.ci_upper
        )
    }
}

#[doc = pydoc!("finstack_quant.analytics", "GreeksResult")]
#[pyclass(name = "GreeksResult", module = "finstack_quant.analytics", frozen)]
pub struct PyGreeksResult {
    pub(super) inner: GreeksResult,
}

#[pymethods]
impl PyGreeksResult {
    #[doc = pydoc!("finstack_quant.analytics", "GreeksResult.alpha")]
    #[getter]
    fn alpha(&self) -> f64 {
        self.inner.alpha
    }
    #[doc = pydoc!("finstack_quant.analytics", "GreeksResult.beta")]
    #[getter]
    fn beta(&self) -> f64 {
        self.inner.beta
    }
    #[doc = pydoc!("finstack_quant.analytics", "GreeksResult.r_squared")]
    #[getter]
    fn r_squared(&self) -> f64 {
        self.inner.r_squared
    }
    #[doc = pydoc!("finstack_quant.analytics", "GreeksResult.adjusted_r_squared")]
    #[getter]
    fn adjusted_r_squared(&self) -> f64 {
        self.inner.adjusted_r_squared
    }
    #[doc = pydoc!("finstack_quant.analytics", "GreeksResult.n_obs")]
    #[getter]
    fn n_obs(&self) -> usize {
        self.inner.n_obs
    }

    #[doc = pydoc!("finstack_quant.analytics", "GreeksResult.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let r = &self.inner;
        let data = PyDict::new(py);
        data.set_item("alpha", PyArray1::from_slice(py, &[r.alpha]))?;
        data.set_item("beta", PyArray1::from_slice(py, &[r.beta]))?;
        data.set_item("r_squared", PyArray1::from_slice(py, &[r.r_squared]))?;
        data.set_item(
            "adjusted_r_squared",
            PyArray1::from_slice(py, &[r.adjusted_r_squared]),
        )?;
        data.set_item("n_obs", PyArray1::from_slice(py, &[r.n_obs as i64]))?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "GreeksResult(alpha={:.6}, beta={:.4}, r2={:.4}, adj_r2={:.4})",
            self.inner.alpha, self.inner.beta, self.inner.r_squared, self.inner.adjusted_r_squared
        )
    }
}

#[doc = pydoc!("finstack_quant.analytics", "RollingGreeks")]
#[pyclass(name = "RollingGreeks", module = "finstack_quant.analytics", frozen)]
pub struct PyRollingGreeks {
    pub(super) inner: RollingGreeks,
}

#[pymethods]
impl PyRollingGreeks {
    #[doc = pydoc!("finstack_quant.analytics", "RollingGreeks.dates")]
    #[getter]
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        dates_to_pylist(py, &self.inner.dates)
    }
    #[doc = pydoc!("finstack_quant.analytics", "RollingGreeks.alphas")]
    #[getter]
    fn alphas<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_slice(py, &self.inner.alphas)
    }
    #[doc = pydoc!("finstack_quant.analytics", "RollingGreeks.betas")]
    #[getter]
    fn betas<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_slice(py, &self.inner.betas)
    }

    #[doc = pydoc!("finstack_quant.analytics", "RollingGreeks.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("alpha", PyArray1::from_slice(py, &self.inner.alphas))?;
        data.set_item("beta", PyArray1::from_slice(py, &self.inner.betas))?;
        let idx = dates_to_datetime_index(py, &self.inner.dates)?;
        dict_to_dataframe(py, &data, Some(idx))
    }

    fn __repr__(&self) -> String {
        format!("RollingGreeks(len={})", self.inner.dates.len())
    }
}

#[doc = pydoc!("finstack_quant.analytics", "MultiFactorResult")]
#[pyclass(
    name = "MultiFactorResult",
    module = "finstack_quant.analytics",
    frozen
)]
pub struct PyMultiFactorResult {
    pub(super) inner: MultiFactorResult,
}

#[pymethods]
impl PyMultiFactorResult {
    #[doc = pydoc!("finstack_quant.analytics", "MultiFactorResult.alpha")]
    #[getter]
    fn alpha(&self) -> f64 {
        self.inner.alpha
    }
    #[doc = pydoc!("finstack_quant.analytics", "MultiFactorResult.betas")]
    #[getter]
    fn betas<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_slice(py, &self.inner.betas)
    }
    #[doc = pydoc!("finstack_quant.analytics", "MultiFactorResult.r_squared")]
    #[getter]
    fn r_squared(&self) -> f64 {
        self.inner.r_squared
    }
    #[doc = pydoc!("finstack_quant.analytics", "MultiFactorResult.adjusted_r_squared")]
    #[getter]
    fn adjusted_r_squared(&self) -> f64 {
        self.inner.adjusted_r_squared
    }
    #[doc = pydoc!("finstack_quant.analytics", "MultiFactorResult.residual_vol")]
    #[getter]
    fn residual_vol(&self) -> f64 {
        self.inner.residual_vol
    }
    #[doc = pydoc!("finstack_quant.analytics", "MultiFactorResult.n_obs")]
    #[getter]
    fn n_obs(&self) -> usize {
        self.inner.n_obs
    }

    #[doc = pydoc!("finstack_quant.analytics", "MultiFactorResult.to_dataframe")]
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
        data.set_item("beta", PyArray1::from_slice(py, betas))?;
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
        data.set_item(
            "n_obs",
            PyArray1::from_vec(py, vec![self.inner.n_obs as i64; betas.len()]),
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

#[doc = pydoc!("finstack_quant.analytics", "DrawdownEpisode")]
#[pyclass(name = "DrawdownEpisode", module = "finstack_quant.analytics", frozen)]
pub struct PyDrawdownEpisode {
    pub(super) inner: DrawdownEpisode,
}

#[pymethods]
impl PyDrawdownEpisode {
    #[doc = pydoc!("finstack_quant.analytics", "DrawdownEpisode.start")]
    #[getter]
    fn start<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start)
    }
    #[doc = pydoc!("finstack_quant.analytics", "DrawdownEpisode.valley")]
    #[getter]
    fn valley<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.valley)
    }
    #[doc = pydoc!("finstack_quant.analytics", "DrawdownEpisode.end")]
    #[getter]
    fn end<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match self.inner.end {
            Some(d) => date_to_py(py, d).map(Some),
            None => Ok(None),
        }
    }
    #[doc = pydoc!("finstack_quant.analytics", "DrawdownEpisode.duration_days")]
    #[getter]
    fn duration_days(&self) -> i64 {
        self.inner.duration_days
    }
    #[doc = pydoc!("finstack_quant.analytics", "DrawdownEpisode.max_drawdown")]
    #[getter]
    fn max_drawdown(&self) -> f64 {
        self.inner.max_drawdown
    }
    #[doc = pydoc!("finstack_quant.analytics", "DrawdownEpisode.near_recovery_threshold")]
    #[getter]
    fn near_recovery_threshold(&self) -> f64 {
        self.inner.near_recovery_threshold
    }
    #[doc = pydoc!("finstack_quant.analytics", "DrawdownEpisode.truncated_at_start")]
    #[getter]
    fn truncated_at_start(&self) -> bool {
        self.inner.truncated_at_start
    }

    #[doc = pydoc!("finstack_quant.analytics", "DrawdownEpisode.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let e = &self.inner;
        let data = PyDict::new(py);
        let end = match e.end {
            Some(date) => date_to_py(py, date)?.into_any(),
            None => py.None().into_bound(py),
        };
        data.set_item("start", dates_to_datetime_index(py, &[e.start])?)?;
        data.set_item("valley", dates_to_datetime_index(py, &[e.valley])?)?;
        data.set_item(
            "end",
            py.import("pandas")?.call_method1("to_datetime", ([end],))?,
        )?;
        data.set_item(
            "duration_days",
            PyArray1::from_slice(py, &[e.duration_days]),
        )?;
        data.set_item("max_drawdown", PyArray1::from_slice(py, &[e.max_drawdown]))?;
        data.set_item(
            "near_recovery_threshold",
            PyArray1::from_slice(py, &[e.near_recovery_threshold]),
        )?;
        data.set_item(
            "truncated_at_start",
            PyArray1::from_slice(py, &[e.truncated_at_start]),
        )?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "DrawdownEpisode(dd={:.4}, days={})",
            self.inner.max_drawdown, self.inner.duration_days
        )
    }
}

#[doc = pydoc!("finstack_quant.analytics", "LookbackReturns")]
#[pyclass(name = "LookbackReturns", module = "finstack_quant.analytics", frozen)]
pub struct PyLookbackReturns {
    pub(super) inner: LookbackReturns,
}

#[pymethods]
impl PyLookbackReturns {
    #[doc = pydoc!("finstack_quant.analytics", "LookbackReturns.ticker_names")]
    #[getter]
    fn ticker_names(&self) -> Vec<String> {
        self.inner.ticker_names.clone()
    }
    #[doc = pydoc!("finstack_quant.analytics", "LookbackReturns.mtd")]
    #[getter]
    fn mtd<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_slice(py, &self.inner.mtd)
    }
    #[doc = pydoc!("finstack_quant.analytics", "LookbackReturns.qtd")]
    #[getter]
    fn qtd<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_slice(py, &self.inner.qtd)
    }
    #[doc = pydoc!("finstack_quant.analytics", "LookbackReturns.ytd")]
    #[getter]
    fn ytd<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_slice(py, &self.inner.ytd)
    }
    #[doc = pydoc!("finstack_quant.analytics", "LookbackReturns.fytd")]
    #[getter]
    fn fytd<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_slice(py, &self.inner.fytd)
    }

    #[doc = pydoc!("finstack_quant.analytics", "LookbackReturns.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("mtd", PyArray1::from_slice(py, &self.inner.mtd))?;
        data.set_item("qtd", PyArray1::from_slice(py, &self.inner.qtd))?;
        data.set_item("ytd", PyArray1::from_slice(py, &self.inner.ytd))?;
        data.set_item("fytd", PyArray1::from_slice(py, &self.inner.fytd))?;
        let idx = self
            .inner
            .ticker_names
            .clone()
            .into_pyobject(py)?
            .into_any();
        dict_to_dataframe(py, &data, Some(idx))
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
    pub(super) inner: DatedSeries,
}

#[pymethods]
impl PyDatedSeries {
    /// Numeric values, one per window.
    #[getter]
    fn values<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_slice(py, &self.inner.values)
    }
    /// Window-end dates aligned 1:1 with [`values`](Self::values).
    #[getter]
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        dates_to_pylist(py, &self.inner.dates)
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
            PyArray1::from_slice(py, &self.inner.values),
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
}

wire_methods!(
    PyPeriodStats,
    PeriodStats,
    "PeriodStats",
    module = "finstack_quant.analytics"
);
impl_repr_html_via_dataframe!(
    PyPeriodStats,
    pydoc!("finstack_quant.analytics", "PeriodStats._repr_html_")
);
wire_methods!(
    PyBetaResult,
    BetaResult,
    "BetaResult",
    module = "finstack_quant.analytics"
);
impl_repr_html_via_dataframe!(
    PyBetaResult,
    pydoc!("finstack_quant.analytics", "BetaResult._repr_html_")
);
wire_methods!(
    PyGreeksResult,
    GreeksResult,
    "GreeksResult",
    module = "finstack_quant.analytics"
);
impl_repr_html_via_dataframe!(
    PyGreeksResult,
    pydoc!("finstack_quant.analytics", "GreeksResult._repr_html_")
);
wire_methods!(
    PyRollingGreeks,
    RollingGreeks,
    "RollingGreeks",
    module = "finstack_quant.analytics"
);
impl_repr_html_via_dataframe!(
    PyRollingGreeks,
    pydoc!("finstack_quant.analytics", "RollingGreeks._repr_html_")
);
wire_methods!(
    PyMultiFactorResult,
    MultiFactorResult,
    "MultiFactorResult",
    module = "finstack_quant.analytics"
);
wire_methods!(
    PyDrawdownEpisode,
    DrawdownEpisode,
    "DrawdownEpisode",
    module = "finstack_quant.analytics"
);
impl_repr_html_via_dataframe!(
    PyDrawdownEpisode,
    pydoc!("finstack_quant.analytics", "DrawdownEpisode._repr_html_")
);
wire_methods!(
    PyLookbackReturns,
    LookbackReturns,
    "LookbackReturns",
    module = "finstack_quant.analytics"
);
impl_repr_html_via_dataframe!(
    PyLookbackReturns,
    pydoc!("finstack_quant.analytics", "LookbackReturns._repr_html_")
);
wire_methods!(PyDatedSeries, DatedSeries, "DatedSeries");
impl_repr_html_via_dataframe!(PyDatedSeries);

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPeriodStats>()?;
    m.add_class::<PyBetaResult>()?;
    m.add_class::<PyGreeksResult>()?;
    m.add_class::<PyRollingGreeks>()?;
    m.add_class::<PyMultiFactorResult>()?;
    m.add_class::<PyDrawdownEpisode>()?;
    m.add_class::<PyLookbackReturns>()?;
    m.add_class::<PyDatedSeries>()?;
    Ok(())
}
