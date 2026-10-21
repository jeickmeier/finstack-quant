//! Python binding for the `PortfolioResult` envelope.

use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::portfolio::metrics::PyPortfolioMetrics;
use crate::bindings::portfolio::valuation::PyPortfolioValuation;
use crate::errors::display_to_py;
use pyo3::prelude::*;
use pyo3::types::PyAny;
use pyo3::types::PyModule;

/// Combined portfolio result envelope: a ``PortfolioValuation``, the
/// aggregated ``PortfolioMetrics`` and the calculation metadata stamp.
#[pyclass(
    name = "PortfolioResult",
    module = "finstack_quant.portfolio",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPortfolioResult {
    pub(crate) inner: finstack_quant_portfolio::PortfolioResult,
}

impl PyPortfolioResult {
    pub(crate) fn from_inner(inner: finstack_quant_portfolio::PortfolioResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPortfolioResult {
    /// Assemble a result envelope from a valuation and its aggregated metrics.
    ///
    /// The ``meta`` stamp is built in Rust: the numeric mode, rounding context
    /// and library version come from ``config``, and ``fx_policy_applied``
    /// records the valuation's ``fx_collapse_policy``.
    ///
    /// Parameters
    /// ----------
    /// valuation : PortfolioValuation
    ///     Output of ``value_portfolio``.
    /// metrics : PortfolioMetrics
    ///     Output of ``aggregate_metrics`` for the same valuation.
    /// config : FinstackConfig, optional
    ///     Configuration whose rounding context is stamped into ``meta``.
    ///     ``None`` uses the default ``FinstackConfig``.
    /// timestamp : datetime.datetime, optional
    ///     Timezone-aware audit timestamp for ``meta["timestamp"]``. ``None``
    ///     (the default) leaves it out, so identical inputs serialize
    ///     identically.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``timestamp`` is not a timezone-aware ``datetime``.
    #[new]
    #[pyo3(signature = (valuation, metrics, config=None, timestamp=None))]
    #[pyo3(text_signature = "(valuation, metrics, config=None, timestamp=None)")]
    fn new(
        valuation: &PyPortfolioValuation,
        metrics: &PyPortfolioMetrics,
        config: Option<PyRef<'_, crate::bindings::core::config::PyFinstackConfig>>,
        timestamp: Option<time::OffsetDateTime>,
    ) -> Self {
        let default_config;
        let config = match &config {
            Some(cfg) => &cfg.inner,
            None => {
                default_config = finstack_quant_core::config::FinstackConfig::default();
                &default_config
            }
        };
        Self::from_inner(finstack_quant_portfolio::PortfolioResult::new(
            valuation.inner.clone(),
            metrics.inner.clone(),
            config,
            timestamp,
        ))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let payload = self.to_json(py)?;
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, payload)
    }

    /// Parse a result from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(result_json)")]
    fn from_json(py: Python<'_>, result_json: &str) -> PyResult<Self> {
        let result_json = result_json.to_owned();
        let inner: finstack_quant_portfolio::PortfolioResult = py
            .detach(move || serde_json::from_str(&result_json))
            .map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize back to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self, py: Python<'_>) -> PyResult<String> {
        let result = &self.inner;
        py.detach(|| serde_json::to_string(result))
            .map_err(display_to_py)
    }

    /// The valuation component.
    #[getter]
    fn valuation(&self) -> PyPortfolioValuation {
        PyPortfolioValuation::from_inner(self.inner.valuation.clone())
    }

    /// The aggregated-metrics component.
    #[getter]
    fn metrics(&self) -> PyPortfolioMetrics {
        PyPortfolioMetrics::from_inner(self.inner.metrics.clone())
    }

    /// Calculation metadata stamp (numeric mode, rounding context, version)
    /// as a JSON-shaped ``dict``.
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    fn __repr__(&self) -> String {
        let total = &self.inner.valuation.total_base_currency;
        format!(
            "PortfolioResult(total={} {})",
            total.amount(),
            total.currency(),
        )
    }
}

/// Register the results bindings on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPortfolioResult>()?;
    Ok(())
}
