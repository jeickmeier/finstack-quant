//! Result classes for the analytics domain.

use crate::bindings::core::dates::periods::PyFiscalConfig;
use crate::bindings::core::table::{PyArrowTable, PyLabeledSeries};
use crate::bindings::date_utils::date_to_py;
use crate::bindings::date_utils::dates_to_pylist;
use crate::bindings::macros::{tabular_methods, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::bindings::table_builders::TableBuilder;
use finstack_quant_analytics::{
    BetaResult, CorrelationResult, DatedSeries, DrawdownEpisode, GreeksResult, LookbackReturns,
    LookbackSample, MultiFactorResult, PeriodStats, RollingGreeks,
};
use numpy::PyArray1;
use pyo3::prelude::*;
use pyo3::types::PyCapsule;

#[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats")]
#[pyclass(name = "PeriodStats", module = "finstack_quant.analytics", frozen)]
pub struct PyPeriodStats {
    pub(super) inner: PeriodStats,
}

#[pymethods]
impl PyPeriodStats {
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.n_obs")]
    #[getter]
    fn n_obs(&self) -> usize {
        self.inner.n_obs
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.wins")]
    #[getter]
    fn wins(&self) -> usize {
        self.inner.wins
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.losses")]
    #[getter]
    fn losses(&self) -> usize {
        self.inner.losses
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.best")]
    #[getter]
    fn best(&self) -> f64 {
        self.inner.best
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.worst")]
    #[getter]
    fn worst(&self) -> f64 {
        self.inner.worst
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.consecutive_wins")]
    #[getter]
    fn consecutive_wins(&self) -> usize {
        self.inner.consecutive_wins
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.consecutive_losses")]
    #[getter]
    fn consecutive_losses(&self) -> usize {
        self.inner.consecutive_losses
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.win_rate")]
    #[getter]
    fn win_rate(&self) -> f64 {
        self.inner.win_rate
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.avg_return")]
    #[getter]
    fn avg_return(&self) -> f64 {
        self.inner.avg_return
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.avg_win")]
    #[getter]
    fn avg_win(&self) -> f64 {
        self.inner.avg_win
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.avg_loss")]
    #[getter]
    fn avg_loss(&self) -> f64 {
        self.inner.avg_loss
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.payoff_ratio")]
    #[getter]
    fn payoff_ratio(&self) -> f64 {
        self.inner.payoff_ratio
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.profit_factor")]
    #[getter]
    fn profit_factor(&self) -> f64 {
        self.inner.profit_factor
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.cpc_index")]
    #[getter]
    fn cpc_index(&self) -> f64 {
        self.inner.cpc_index
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.kelly_risk_fraction")]
    #[getter]
    fn kelly_risk_fraction(&self) -> f64 {
        self.inner.kelly_risk_fraction
    }

    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.to_series")]
    fn to_series(&self) -> PyResult<PyLabeledSeries> {
        let (labels, values): (Vec<String>, Vec<f64>) = self
            .fields()
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .unzip();
        Ok(PyLabeledSeries::from_f64("period_stats", labels, values)?.with_index_name("metric"))
    }

    fn __repr__(&self) -> String {
        format!(
            "PeriodStats(win_rate={:.4}, avg_return={:.6})",
            self.inner.win_rate, self.inner.avg_return
        )
    }
}

impl PyPeriodStats {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let s = &self.inner;
        TableBuilder::new()
            .i64("n_obs", vec![s.n_obs as i64])?
            .i64("wins", vec![s.wins as i64])?
            .i64("losses", vec![s.losses as i64])?
            .f64("best", vec![s.best])?
            .f64("worst", vec![s.worst])?
            .i64("consecutive_wins", vec![s.consecutive_wins as i64])?
            .i64("consecutive_losses", vec![s.consecutive_losses as i64])?
            .f64("win_rate", vec![s.win_rate])?
            .f64("avg_return", vec![s.avg_return])?
            .f64("avg_win", vec![s.avg_win])?
            .f64("avg_loss", vec![s.avg_loss])?
            .f64("payoff_ratio", vec![s.payoff_ratio])?
            .f64("profit_factor", vec![s.profit_factor])?
            .f64("cpc_index", vec![s.cpc_index])?
            .f64("kelly_risk_fraction", vec![s.kelly_risk_fraction])?
            .build()
    }

    /// Statistic names and values in `PeriodStats` field order (streak
    /// counts as `f64`).
    fn fields(&self) -> [(&'static str, f64); 15] {
        let s = &self.inner;
        [
            ("n_obs", s.n_obs as f64),
            ("wins", s.wins as f64),
            ("losses", s.losses as f64),
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
            ("cpc_index", s.cpc_index),
            ("kelly_risk_fraction", s.kelly_risk_fraction),
        ]
    }
}

#[doc = pydoc!("finstack_quant.analytics._types", "BetaResult")]
#[pyclass(name = "BetaResult", module = "finstack_quant.analytics", frozen)]
pub struct PyBetaResult {
    pub(super) inner: BetaResult,
}

#[pymethods]
impl PyBetaResult {
    #[doc = pydoc!("finstack_quant.analytics._types", "BetaResult.beta")]
    #[getter]
    fn beta(&self) -> f64 {
        self.inner.beta
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "BetaResult.std_err")]
    #[getter]
    fn std_err(&self) -> f64 {
        self.inner.std_err
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "BetaResult.ci_lower")]
    #[getter]
    fn ci_lower(&self) -> f64 {
        self.inner.ci_lower
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "BetaResult.ci_upper")]
    #[getter]
    fn ci_upper(&self) -> f64 {
        self.inner.ci_upper
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "BetaResult.n_obs")]
    #[getter]
    fn n_obs(&self) -> usize {
        self.inner.n_obs
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "BetaResult.confidence_level")]
    #[getter]
    fn confidence_level(&self) -> f64 {
        self.inner.confidence_level
    }

    fn __repr__(&self) -> String {
        format!(
            "BetaResult(beta={:.4}, se={:.4}, ci=[{:.4}, {:.4}])",
            self.inner.beta, self.inner.std_err, self.inner.ci_lower, self.inner.ci_upper
        )
    }
}

impl PyBetaResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let r = &self.inner;
        TableBuilder::new()
            .f64("beta", vec![r.beta])?
            .f64("std_err", vec![r.std_err])?
            .f64("ci_lower", vec![r.ci_lower])?
            .f64("ci_upper", vec![r.ci_upper])?
            .i64("n_obs", vec![r.n_obs as i64])?
            .f64("confidence_level", vec![r.confidence_level])?
            .build()
    }
}

#[doc = pydoc!("finstack_quant.analytics._types", "GreeksResult")]
#[pyclass(name = "GreeksResult", module = "finstack_quant.analytics", frozen)]
pub struct PyGreeksResult {
    pub(super) inner: GreeksResult,
}

#[pymethods]
impl PyGreeksResult {
    #[doc = pydoc!("finstack_quant.analytics._types", "GreeksResult.alpha")]
    #[getter]
    fn alpha(&self) -> f64 {
        self.inner.alpha
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "GreeksResult.beta")]
    #[getter]
    fn beta(&self) -> f64 {
        self.inner.beta
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "GreeksResult.r_squared")]
    #[getter]
    fn r_squared(&self) -> f64 {
        self.inner.r_squared
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "GreeksResult.adjusted_r_squared")]
    #[getter]
    fn adjusted_r_squared(&self) -> f64 {
        self.inner.adjusted_r_squared
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "GreeksResult.n_obs")]
    #[getter]
    fn n_obs(&self) -> usize {
        self.inner.n_obs
    }

    fn __repr__(&self) -> String {
        format!(
            "GreeksResult(alpha={:.6}, beta={:.4}, r2={:.4}, adj_r2={:.4})",
            self.inner.alpha, self.inner.beta, self.inner.r_squared, self.inner.adjusted_r_squared
        )
    }
}

impl PyGreeksResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let r = &self.inner;
        TableBuilder::new()
            .f64("alpha", vec![r.alpha])?
            .f64("beta", vec![r.beta])?
            .f64("r_squared", vec![r.r_squared])?
            .f64("adjusted_r_squared", vec![r.adjusted_r_squared])?
            .i64("n_obs", vec![r.n_obs as i64])?
            .build()
    }
}

#[doc = pydoc!("finstack_quant.analytics._types", "RollingGreeks")]
#[pyclass(name = "RollingGreeks", module = "finstack_quant.analytics", frozen)]
pub struct PyRollingGreeks {
    pub(super) inner: RollingGreeks,
}

#[pymethods]
impl PyRollingGreeks {
    #[doc = pydoc!("finstack_quant.analytics._types", "RollingGreeks.dates")]
    #[getter]
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        dates_to_pylist(py, &self.inner.dates)
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "RollingGreeks.alphas")]
    #[getter]
    fn alphas<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_slice(py, &self.inner.alphas)
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "RollingGreeks.betas")]
    #[getter]
    fn betas<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_slice(py, &self.inner.betas)
    }

    fn __repr__(&self) -> String {
        format!("RollingGreeks(len={})", self.inner.dates.len())
    }
}

impl PyRollingGreeks {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .date("date", &self.inner.dates)?
            .f64("alpha", self.inner.alphas.clone())?
            .f64("beta", self.inner.betas.clone())?
            .build()
    }
}

#[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult")]
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
    #[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult.alpha")]
    #[getter]
    fn alpha(&self) -> f64 {
        self.inner.alpha
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult.betas")]
    #[getter]
    fn betas<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_slice(py, &self.inner.betas)
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult.r_squared")]
    #[getter]
    fn r_squared(&self) -> f64 {
        self.inner.r_squared
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult.adjusted_r_squared")]
    #[getter]
    fn adjusted_r_squared(&self) -> f64 {
        self.inner.adjusted_r_squared
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult.residual_vol")]
    #[getter]
    fn residual_vol(&self) -> f64 {
        self.inner.residual_vol
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult.n_obs")]
    #[getter]
    fn n_obs(&self) -> usize {
        self.inner.n_obs
    }

    #[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult.to_arrow")]
    #[pyo3(signature = (factor_names=None), text_signature = "(self, factor_names=None)")]
    fn to_arrow(&self, factor_names: Option<Vec<String>>) -> PyResult<PyArrowTable> {
        self.build_arrow_table(factor_names)
    }

    #[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult.to_pandas")]
    #[pyo3(signature = (factor_names=None), text_signature = "(self, factor_names=None)")]
    fn to_pandas<'py>(
        &self,
        py: Python<'py>,
        factor_names: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let table = Bound::new(py, self.build_arrow_table(factor_names)?)?;
        PyArrowTable::to_pandas(&table, py)
    }

    #[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult.to_polars")]
    #[pyo3(signature = (factor_names=None), text_signature = "(self, factor_names=None)")]
    fn to_polars<'py>(
        &self,
        py: Python<'py>,
        factor_names: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let table = Bound::new(py, self.build_arrow_table(factor_names)?)?;
        PyArrowTable::to_polars(&table, py)
    }

    #[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult.to_pyarrow")]
    #[pyo3(signature = (factor_names=None), text_signature = "(self, factor_names=None)")]
    fn to_pyarrow<'py>(
        &self,
        py: Python<'py>,
        factor_names: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let table = Bound::new(py, self.build_arrow_table(factor_names)?)?;
        PyArrowTable::to_pyarrow(&table, py)
    }

    #[pyo3(signature = (requested_schema=None))]
    fn __arrow_c_stream__<'py>(
        &self,
        py: Python<'py>,
        requested_schema: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        let _ = requested_schema;
        self.build_arrow_table(None)?.arrow_c_stream(py)
    }

    fn __arrow_c_schema__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyCapsule>> {
        self.build_arrow_table(None)?.arrow_c_schema(py)
    }

    fn __repr__(&self) -> String {
        format!(
            "MultiFactorResult(alpha={:.6}, r2={:.4}, adj_r2={:.4})",
            self.inner.alpha, self.inner.r_squared, self.inner.adjusted_r_squared
        )
    }

    fn _repr_html_(&self) -> Option<String> {
        self.build_arrow_table(None).ok().map(|t| t.render_html())
    }
}

impl PyMultiFactorResult {
    fn build_arrow_table(&self, factor_names: Option<Vec<String>>) -> PyResult<PyArrowTable> {
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
        let n = betas.len();
        TableBuilder::new()
            .utf8("factor", names)?
            .f64("beta", betas.clone())?
            .f64("alpha", vec![self.inner.alpha; n])?
            .f64("r_squared", vec![self.inner.r_squared; n])?
            .f64("adjusted_r_squared", vec![self.inner.adjusted_r_squared; n])?
            .f64("residual_vol", vec![self.inner.residual_vol; n])?
            .i64("n_obs", vec![self.inner.n_obs as i64; n])?
            .build()
    }
}

#[doc = pydoc!("finstack_quant.analytics._types", "DrawdownEpisode")]
#[pyclass(name = "DrawdownEpisode", module = "finstack_quant.analytics", frozen)]
pub struct PyDrawdownEpisode {
    pub(super) inner: DrawdownEpisode,
}

#[pymethods]
impl PyDrawdownEpisode {
    #[doc = pydoc!("finstack_quant.analytics._types", "DrawdownEpisode.start")]
    #[getter]
    fn start<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start)
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "DrawdownEpisode.valley")]
    #[getter]
    fn valley<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.valley)
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "DrawdownEpisode.end")]
    #[getter]
    fn end<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match self.inner.end {
            Some(d) => date_to_py(py, d).map(Some),
            None => Ok(None),
        }
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "DrawdownEpisode.duration_days")]
    #[getter]
    fn duration_days(&self) -> i64 {
        self.inner.duration_days
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "DrawdownEpisode.max_drawdown")]
    #[getter]
    fn max_drawdown(&self) -> f64 {
        self.inner.max_drawdown
    }

    fn __repr__(&self) -> String {
        format!(
            "DrawdownEpisode(dd={:.4}, days={})",
            self.inner.max_drawdown, self.inner.duration_days
        )
    }
}

impl PyDrawdownEpisode {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let e = &self.inner;
        TableBuilder::new()
            .date("start", &[e.start])?
            .date("valley", &[e.valley])?
            .nullable_date("end", &[e.end])?
            .i64("duration_days", vec![e.duration_days])?
            .f64("max_drawdown", vec![e.max_drawdown])?
            .build()
    }
}

#[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns")]
#[pyclass(name = "LookbackReturns", module = "finstack_quant.analytics", frozen)]
pub struct PyLookbackReturns {
    pub(super) inner: LookbackReturns,
}

#[pymethods]
impl PyLookbackReturns {
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns.ref_date")]
    #[getter]
    fn ref_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.ref_date)
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns.ticker_names")]
    #[getter]
    fn ticker_names(&self) -> Vec<String> {
        self.inner.ticker_names.clone()
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns.fiscal_config")]
    #[getter]
    fn fiscal_config(&self) -> PyFiscalConfig {
        PyFiscalConfig {
            inner: self.inner.fiscal_config,
        }
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns.mtd_samples")]
    #[getter]
    fn mtd_samples(&self) -> Vec<PyLookbackSample> {
        self.inner
            .mtd_samples
            .iter()
            .cloned()
            .map(|inner| PyLookbackSample { inner })
            .collect()
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns.qtd_samples")]
    #[getter]
    fn qtd_samples(&self) -> Vec<PyLookbackSample> {
        self.inner
            .qtd_samples
            .iter()
            .cloned()
            .map(|inner| PyLookbackSample { inner })
            .collect()
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns.ytd_samples")]
    #[getter]
    fn ytd_samples(&self) -> Vec<PyLookbackSample> {
        self.inner
            .ytd_samples
            .iter()
            .cloned()
            .map(|inner| PyLookbackSample { inner })
            .collect()
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns.fytd_samples")]
    #[getter]
    fn fytd_samples(&self) -> Vec<PyLookbackSample> {
        self.inner
            .fytd_samples
            .iter()
            .cloned()
            .map(|inner| PyLookbackSample { inner })
            .collect()
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns.mtd")]
    #[getter]
    fn mtd<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_slice(py, &self.inner.mtd)
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns.qtd")]
    #[getter]
    fn qtd<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_slice(py, &self.inner.qtd)
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns.ytd")]
    #[getter]
    fn ytd<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_slice(py, &self.inner.ytd)
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns.fytd")]
    #[getter]
    fn fytd<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_slice(py, &self.inner.fytd)
    }

    fn __repr__(&self) -> String {
        format!(
            "LookbackReturns(mtd_len={}, fytd_len={})",
            self.inner.mtd.len(),
            self.inner.fytd.len()
        )
    }
}

impl PyLookbackReturns {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .utf8("ticker", self.inner.ticker_names.clone())?
            .f64("mtd", self.inner.mtd.clone())?
            .f64("qtd", self.inner.qtd.clone())?
            .f64("ytd", self.inner.ytd.clone())?
            .f64("fytd", self.inner.fytd.clone())?
            .build()
    }
}

#[doc = pydoc!("finstack_quant.analytics._types", "DatedSeries")]
#[pyclass(name = "DatedSeries", module = "finstack_quant.analytics", frozen)]
pub struct PyDatedSeries {
    pub(super) inner: DatedSeries,
}

#[pymethods]
impl PyDatedSeries {
    #[doc = pydoc!("finstack_quant.analytics._types", "DatedSeries.values")]
    #[getter]
    fn values<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_slice(py, &self.inner.values)
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "DatedSeries.dates")]
    #[getter]
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        dates_to_pylist(py, &self.inner.dates)
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "DatedSeries.value_column")]
    #[getter]
    fn value_column(&self) -> &'static str {
        self.inner.value_column.as_str()
    }

    fn __repr__(&self) -> String {
        format!(
            "DatedSeries(name={:?}, len={})",
            self.inner.value_column.as_str(),
            self.inner.values.len()
        )
    }
}

impl PyDatedSeries {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .date("date", &self.inner.dates)?
            .f64(self.inner.value_column.as_str(), self.inner.values.clone())?
            .build()
    }
}

#[doc = pydoc!("finstack_quant.analytics._types", "LookbackSample")]
#[pyclass(name = "LookbackSample", module = "finstack_quant.analytics", frozen)]
pub struct PyLookbackSample {
    pub(super) inner: LookbackSample,
}

#[pymethods]
impl PyLookbackSample {
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackSample.n_obs")]
    #[getter]
    fn n_obs(&self) -> usize {
        self.inner.n_obs
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackSample.holding_start")]
    #[getter]
    fn holding_start<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .holding_start
            .map(|date| date_to_py(py, date))
            .transpose()
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackSample.holding_end")]
    #[getter]
    fn holding_end<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .holding_end
            .map(|date| date_to_py(py, date))
            .transpose()
    }
}

#[doc = pydoc!("finstack_quant.analytics._types", "CorrelationResult")]
#[pyclass(
    name = "CorrelationResult",
    module = "finstack_quant.analytics",
    frozen
)]
pub struct PyCorrelationResult {
    pub(super) inner: CorrelationResult,
}

#[pymethods]
impl PyCorrelationResult {
    #[doc = pydoc!("finstack_quant.analytics._types", "CorrelationResult.tickers")]
    #[getter]
    fn tickers(&self) -> Vec<String> {
        self.inner.tickers.clone()
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "CorrelationResult.matrix")]
    #[getter]
    fn matrix(&self) -> Vec<Vec<f64>> {
        self.inner.matrix.clone()
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "CorrelationResult.n_obs")]
    #[getter]
    fn n_obs(&self) -> usize {
        self.inner.n_obs
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "CorrelationResult.start_date")]
    #[getter]
    fn start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start_date)
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "CorrelationResult.end_date")]
    #[getter]
    fn end_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.end_date)
    }

    fn __repr__(&self) -> String {
        format!(
            "CorrelationResult(tickers={:?}, n_obs={}, start_date={}, end_date={})",
            self.inner.tickers, self.inner.n_obs, self.inner.start_date, self.inner.end_date
        )
    }
}

impl PyCorrelationResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let tickers = &self.inner.tickers;
        if self.inner.matrix.len() != tickers.len() {
            return Err(crate::errors::value_error(format!(
                "correlation matrix has {} rows for {} tickers",
                self.inner.matrix.len(),
                tickers.len()
            )));
        }
        let mut builder = TableBuilder::new().utf8("ticker", tickers.clone())?;
        for (j, ticker) in tickers.iter().enumerate() {
            let column: Vec<f64> = self
                .inner
                .matrix
                .iter()
                .map(|row| {
                    row.get(j).copied().ok_or_else(|| {
                        crate::errors::value_error("correlation matrix row shorter than tickers")
                    })
                })
                .collect::<PyResult<_>>()?;
            builder = builder.f64(ticker, column)?;
        }
        builder.build()
    }
}

wire_methods!(
    PyPeriodStats,
    PeriodStats,
    "PeriodStats",
    module = "finstack_quant.analytics._types"
);
tabular_methods!(
    PyPeriodStats,
    "PeriodStats",
    module = "finstack_quant.analytics._types"
);
wire_methods!(
    PyBetaResult,
    BetaResult,
    "BetaResult",
    module = "finstack_quant.analytics._types"
);
tabular_methods!(
    PyBetaResult,
    "BetaResult",
    module = "finstack_quant.analytics._types"
);
wire_methods!(
    PyGreeksResult,
    GreeksResult,
    "GreeksResult",
    module = "finstack_quant.analytics._types"
);
tabular_methods!(
    PyGreeksResult,
    "GreeksResult",
    module = "finstack_quant.analytics._types"
);
wire_methods!(
    PyRollingGreeks,
    RollingGreeks,
    "RollingGreeks",
    module = "finstack_quant.analytics._types"
);
tabular_methods!(
    PyRollingGreeks,
    "RollingGreeks",
    module = "finstack_quant.analytics._types",
    index = "date"
);
wire_methods!(
    PyMultiFactorResult,
    MultiFactorResult,
    "MultiFactorResult",
    module = "finstack_quant.analytics._types"
);
wire_methods!(
    PyDrawdownEpisode,
    DrawdownEpisode,
    "DrawdownEpisode",
    module = "finstack_quant.analytics._types"
);
tabular_methods!(
    PyDrawdownEpisode,
    "DrawdownEpisode",
    module = "finstack_quant.analytics._types"
);
wire_methods!(
    PyLookbackReturns,
    LookbackReturns,
    "LookbackReturns",
    module = "finstack_quant.analytics._types"
);
tabular_methods!(
    PyLookbackReturns,
    "LookbackReturns",
    module = "finstack_quant.analytics._types",
    index = "ticker"
);
wire_methods!(
    PyDatedSeries,
    DatedSeries,
    "DatedSeries",
    module = "finstack_quant.analytics._types"
);
tabular_methods!(
    PyDatedSeries,
    "DatedSeries",
    module = "finstack_quant.analytics._types",
    index = "date"
);

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCorrelationResult>()?;
    m.add_class::<PyLookbackSample>()?;
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

wire_methods!(
    PyLookbackSample,
    LookbackSample,
    "LookbackSample",
    module = "finstack_quant.analytics._types"
);

wire_methods!(
    PyCorrelationResult,
    CorrelationResult,
    "CorrelationResult",
    module = "finstack_quant.analytics._types"
);
tabular_methods!(
    PyCorrelationResult,
    "CorrelationResult",
    module = "finstack_quant.analytics._types",
    index = "ticker"
);
