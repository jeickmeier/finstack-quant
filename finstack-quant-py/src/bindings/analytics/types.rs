//! Result classes for the analytics domain.

use crate::bindings::core::dates::periods::PyFiscalConfig;
use crate::bindings::date_utils::date_to_py;
use crate::bindings::macros::{impl_repr_html_via_dataframe, py_getters, wire_methods};
use crate::bindings::pandas_utils::{
    dates_to_datetime_index, dates_to_pylist, dict_to_dataframe, labeled_values_to_series,
};
use crate::bindings::pydoc::pydoc;
use finstack_quant_analytics::{
    BetaResult, CorrelationResult, DatedSeries, DrawdownEpisode, GreeksResult, LookbackReturns,
    LookbackSample, MultiFactorResult, PeriodStats, RollingGreeks,
};
use finstack_quant_core::dates::Date;
use numpy::PyArray1;
use pyo3::prelude::*;
use pyo3::types::PyDict;

fn opt_date_to_py(py: Python<'_>, date: Option<Date>) -> PyResult<Option<Bound<'_, PyAny>>> {
    date.map(|d| date_to_py(py, d)).transpose()
}

fn wrap_samples(samples: &[LookbackSample]) -> Vec<PyLookbackSample> {
    samples
        .iter()
        .copied()
        .map(|inner| PyLookbackSample { inner })
        .collect()
}

#[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats")]
#[pyclass(name = "PeriodStats", module = "finstack_quant.analytics", frozen)]
pub struct PyPeriodStats {
    pub(super) inner: PeriodStats,
}

py_getters!(PyPeriodStats |s| {
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.n_obs")]
    n_obs -> usize = s.inner.n_obs;
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.wins")]
    wins -> usize = s.inner.wins;
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.losses")]
    losses -> usize = s.inner.losses;
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.best")]
    best -> f64 = s.inner.best;
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.worst")]
    worst -> f64 = s.inner.worst;
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.consecutive_wins")]
    consecutive_wins -> usize = s.inner.consecutive_wins;
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.consecutive_losses")]
    consecutive_losses -> usize = s.inner.consecutive_losses;
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.win_rate")]
    win_rate -> f64 = s.inner.win_rate;
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.avg_return")]
    avg_return -> f64 = s.inner.avg_return;
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.avg_win")]
    avg_win -> f64 = s.inner.avg_win;
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.avg_loss")]
    avg_loss -> f64 = s.inner.avg_loss;
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.payoff_ratio")]
    payoff_ratio -> f64 = s.inner.payoff_ratio;
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.profit_factor")]
    profit_factor -> f64 = s.inner.profit_factor;
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.cpc_index")]
    cpc_index -> f64 = s.inner.cpc_index;
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.kelly_risk_fraction")]
    kelly_risk_fraction -> f64 = s.inner.kelly_risk_fraction;
});

#[pymethods]
impl PyPeriodStats {
    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.to_series")]
    fn to_series<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let (labels, values): (Vec<String>, Vec<f64>) = self
            .fields()
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .unzip();
        labeled_values_to_series(py, &labels, values, "period_stats")
    }

    #[doc = pydoc!("finstack_quant.analytics._types", "PeriodStats.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let s = &self.inner;
        let data = PyDict::new(py);
        for (name, value) in self.fields() {
            data.set_item(name, PyArray1::from_slice(py, &[value]))?;
        }
        // Observation and streak counts stay integer columns.
        for (name, count) in [
            ("n_obs", s.n_obs),
            ("wins", s.wins),
            ("losses", s.losses),
            ("consecutive_wins", s.consecutive_wins),
            ("consecutive_losses", s.consecutive_losses),
        ] {
            data.set_item(name, vec![count])?;
        }
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

py_getters!(PyBetaResult |s| {
    #[doc = pydoc!("finstack_quant.analytics._types", "BetaResult.beta")]
    beta -> f64 = s.inner.beta;
    #[doc = pydoc!("finstack_quant.analytics._types", "BetaResult.std_err")]
    std_err -> f64 = s.inner.std_err;
    #[doc = pydoc!("finstack_quant.analytics._types", "BetaResult.ci_lower")]
    ci_lower -> f64 = s.inner.ci_lower;
    #[doc = pydoc!("finstack_quant.analytics._types", "BetaResult.ci_upper")]
    ci_upper -> f64 = s.inner.ci_upper;
    #[doc = pydoc!("finstack_quant.analytics._types", "BetaResult.n_obs")]
    n_obs -> usize = s.inner.n_obs;
    #[doc = pydoc!("finstack_quant.analytics._types", "BetaResult.confidence_level")]
    confidence_level -> f64 = s.inner.confidence_level;
});

#[pymethods]
impl PyBetaResult {
    #[doc = pydoc!("finstack_quant.analytics._types", "BetaResult.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let r = &self.inner;
        let data = PyDict::new(py);
        for (name, val) in [
            ("beta", r.beta),
            ("std_err", r.std_err),
            ("ci_lower", r.ci_lower),
            ("ci_upper", r.ci_upper),
        ] {
            data.set_item(name, PyArray1::from_slice(py, &[val]))?;
        }
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

#[doc = pydoc!("finstack_quant.analytics._types", "GreeksResult")]
#[pyclass(name = "GreeksResult", module = "finstack_quant.analytics", frozen)]
pub struct PyGreeksResult {
    pub(super) inner: GreeksResult,
}

py_getters!(PyGreeksResult |s| {
    #[doc = pydoc!("finstack_quant.analytics._types", "GreeksResult.alpha")]
    alpha -> f64 = s.inner.alpha;
    #[doc = pydoc!("finstack_quant.analytics._types", "GreeksResult.beta")]
    beta -> f64 = s.inner.beta;
    #[doc = pydoc!("finstack_quant.analytics._types", "GreeksResult.r_squared")]
    r_squared -> f64 = s.inner.r_squared;
    #[doc = pydoc!("finstack_quant.analytics._types", "GreeksResult.adjusted_r_squared")]
    adjusted_r_squared -> f64 = s.inner.adjusted_r_squared;
    #[doc = pydoc!("finstack_quant.analytics._types", "GreeksResult.n_obs")]
    n_obs -> usize = s.inner.n_obs;
});

#[pymethods]
impl PyGreeksResult {
    #[doc = pydoc!("finstack_quant.analytics._types", "GreeksResult.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let r = &self.inner;
        let data = PyDict::new(py);
        for (name, val) in [
            ("alpha", r.alpha),
            ("beta", r.beta),
            ("r_squared", r.r_squared),
            ("adjusted_r_squared", r.adjusted_r_squared),
        ] {
            data.set_item(name, PyArray1::from_slice(py, &[val]))?;
        }
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

    #[doc = pydoc!("finstack_quant.analytics._types", "RollingGreeks.to_dataframe")]
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

#[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult")]
#[pyclass(
    name = "MultiFactorResult",
    module = "finstack_quant.analytics",
    frozen
)]
pub struct PyMultiFactorResult {
    pub(super) inner: MultiFactorResult,
}

py_getters!(PyMultiFactorResult |s| {
    #[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult.alpha")]
    alpha -> f64 = s.inner.alpha;
    #[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult.r_squared")]
    r_squared -> f64 = s.inner.r_squared;
    #[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult.adjusted_r_squared")]
    adjusted_r_squared -> f64 = s.inner.adjusted_r_squared;
    #[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult.residual_vol")]
    residual_vol -> f64 = s.inner.residual_vol;
    #[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult.n_obs")]
    n_obs -> usize = s.inner.n_obs;
});

#[pymethods]
impl PyMultiFactorResult {
    #[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult.betas")]
    #[getter]
    fn betas<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        PyArray1::from_slice(py, &self.inner.betas)
    }

    #[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult.to_dataframe")]
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

    #[doc = pydoc!("finstack_quant.analytics._types", "MultiFactorResult._repr_html_")]
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py, None).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

#[doc = pydoc!("finstack_quant.analytics._types", "DrawdownEpisode")]
#[pyclass(name = "DrawdownEpisode", module = "finstack_quant.analytics", frozen)]
pub struct PyDrawdownEpisode {
    pub(super) inner: DrawdownEpisode,
}

py_getters!(PyDrawdownEpisode |s| {
    #[doc = pydoc!("finstack_quant.analytics._types", "DrawdownEpisode.duration_days")]
    duration_days -> i64 = s.inner.duration_days;
    #[doc = pydoc!("finstack_quant.analytics._types", "DrawdownEpisode.max_drawdown")]
    max_drawdown -> f64 = s.inner.max_drawdown;
});

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
        opt_date_to_py(py, self.inner.end)
    }

    #[doc = pydoc!("finstack_quant.analytics._types", "DrawdownEpisode.to_dataframe")]
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
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "DrawdownEpisode(dd={:.4}, days={})",
            self.inner.max_drawdown, self.inner.duration_days
        )
    }
}

#[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns")]
#[pyclass(name = "LookbackReturns", module = "finstack_quant.analytics", frozen)]
pub struct PyLookbackReturns {
    pub(super) inner: LookbackReturns,
}

py_getters!(PyLookbackReturns |s| {
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns.ticker_names")]
    ticker_names -> Vec<String> = s.inner.ticker_names.clone();
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns.fiscal_config")]
    fiscal_config -> PyFiscalConfig = PyFiscalConfig { inner: s.inner.fiscal_config };
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns.mtd_samples")]
    mtd_samples -> Vec<PyLookbackSample> = wrap_samples(&s.inner.mtd_samples);
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns.qtd_samples")]
    qtd_samples -> Vec<PyLookbackSample> = wrap_samples(&s.inner.qtd_samples);
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns.ytd_samples")]
    ytd_samples -> Vec<PyLookbackSample> = wrap_samples(&s.inner.ytd_samples);
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns.fytd_samples")]
    fytd_samples -> Vec<PyLookbackSample> = wrap_samples(&s.inner.fytd_samples);
});

#[pymethods]
impl PyLookbackReturns {
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns.ref_date")]
    #[getter]
    fn ref_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.ref_date)
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

    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackReturns.to_dataframe")]
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

#[doc = pydoc!("finstack_quant.analytics._types", "DatedSeries")]
#[pyclass(name = "DatedSeries", module = "finstack_quant.analytics", frozen)]
pub struct PyDatedSeries {
    pub(super) inner: DatedSeries,
}

py_getters!(PyDatedSeries |s| {
    #[doc = pydoc!("finstack_quant.analytics._types", "DatedSeries.value_column")]
    value_column -> &'static str = s.inner.value_column.as_str();
});

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

    #[doc = pydoc!("finstack_quant.analytics._types", "DatedSeries.to_dataframe")]
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

#[doc = pydoc!("finstack_quant.analytics._types", "LookbackSample")]
#[pyclass(name = "LookbackSample", module = "finstack_quant.analytics", frozen)]
pub struct PyLookbackSample {
    pub(super) inner: LookbackSample,
}

py_getters!(PyLookbackSample |s| {
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackSample.n_obs")]
    n_obs -> usize = s.inner.n_obs;
});

#[pymethods]
impl PyLookbackSample {
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackSample.holding_start")]
    #[getter]
    fn holding_start<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_date_to_py(py, self.inner.holding_start)
    }
    #[doc = pydoc!("finstack_quant.analytics._types", "LookbackSample.holding_end")]
    #[getter]
    fn holding_end<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_date_to_py(py, self.inner.holding_end)
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

py_getters!(PyCorrelationResult |s| {
    #[doc = pydoc!("finstack_quant.analytics._types", "CorrelationResult.tickers")]
    tickers -> Vec<String> = s.inner.tickers.clone();
    #[doc = pydoc!("finstack_quant.analytics._types", "CorrelationResult.matrix")]
    matrix -> Vec<Vec<f64>> = s.inner.matrix.clone();
    #[doc = pydoc!("finstack_quant.analytics._types", "CorrelationResult.n_obs")]
    n_obs -> usize = s.inner.n_obs;
});

#[pymethods]
impl PyCorrelationResult {
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

    #[doc = pydoc!("finstack_quant.analytics._types", "CorrelationResult.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let tickers = &self.inner.tickers;
        if self.inner.matrix.len() != tickers.len() {
            return Err(crate::errors::value_error(format!(
                "correlation matrix has {} rows for {} tickers",
                self.inner.matrix.len(),
                tickers.len()
            )));
        }
        let data = PyDict::new(py);
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
            data.set_item(ticker, PyArray1::from_vec(py, column))?;
        }
        let index = pyo3::types::PyList::new(py, tickers)?;
        dict_to_dataframe(py, &data, Some(index.into_any()))
    }

    fn __repr__(&self) -> String {
        format!(
            "CorrelationResult(tickers={:?}, n_obs={}, start_date={}, end_date={})",
            self.inner.tickers, self.inner.n_obs, self.inner.start_date, self.inner.end_date
        )
    }
}

wire_methods!(
    PyPeriodStats,
    PeriodStats,
    "PeriodStats",
    module = "finstack_quant.analytics._types"
);
impl_repr_html_via_dataframe!(
    PyPeriodStats,
    pydoc!("finstack_quant.analytics._types", "PeriodStats._repr_html_")
);
wire_methods!(
    PyBetaResult,
    BetaResult,
    "BetaResult",
    module = "finstack_quant.analytics._types"
);
impl_repr_html_via_dataframe!(
    PyBetaResult,
    pydoc!("finstack_quant.analytics._types", "BetaResult._repr_html_")
);
wire_methods!(
    PyGreeksResult,
    GreeksResult,
    "GreeksResult",
    module = "finstack_quant.analytics._types"
);
impl_repr_html_via_dataframe!(
    PyGreeksResult,
    pydoc!(
        "finstack_quant.analytics._types",
        "GreeksResult._repr_html_"
    )
);
wire_methods!(
    PyRollingGreeks,
    RollingGreeks,
    "RollingGreeks",
    module = "finstack_quant.analytics._types"
);
impl_repr_html_via_dataframe!(
    PyRollingGreeks,
    pydoc!(
        "finstack_quant.analytics._types",
        "RollingGreeks._repr_html_"
    )
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
impl_repr_html_via_dataframe!(
    PyDrawdownEpisode,
    pydoc!(
        "finstack_quant.analytics._types",
        "DrawdownEpisode._repr_html_"
    )
);
wire_methods!(
    PyLookbackReturns,
    LookbackReturns,
    "LookbackReturns",
    module = "finstack_quant.analytics._types"
);
impl_repr_html_via_dataframe!(
    PyLookbackReturns,
    pydoc!(
        "finstack_quant.analytics._types",
        "LookbackReturns._repr_html_"
    )
);
wire_methods!(
    PyDatedSeries,
    DatedSeries,
    "DatedSeries",
    module = "finstack_quant.analytics._types"
);
impl_repr_html_via_dataframe!(
    PyDatedSeries,
    pydoc!("finstack_quant.analytics._types", "DatedSeries._repr_html_")
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
impl_repr_html_via_dataframe!(
    PyCorrelationResult,
    pydoc!(
        "finstack_quant.analytics._types",
        "CorrelationResult._repr_html_"
    )
);
