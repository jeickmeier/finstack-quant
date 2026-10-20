//! Python bindings for metric aggregation: `aggregate_metrics` and `PortfolioMetrics`.

use crate::bindings::core::currency::extract_currency;
use crate::bindings::core::table::envelope_to_pandas;
use crate::bindings::extract::extract_market_ref;
use crate::bindings::extract::extract_valuation_ref;
use crate::bindings::pandas_utils::serde_to_py;
use crate::errors::core_to_py;
use crate::errors::display_to_py;
use crate::errors::portfolio_to_py;
use pyo3::prelude::*;
use pyo3::types::PyAny;
use pyo3::types::PyDict;
use pyo3::types::PyModule;

type PyMetricSeriesEntry = (Vec<String>, f64, Py<PyDict>);

/// Aggregated portfolio metrics: portfolio-wide totals by metric id, the raw
/// per-position values they were summed from, and the diagnostics recording
/// what was skipped or not summable.
#[pyclass(
    name = "PortfolioMetrics",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPortfolioMetrics {
    pub(crate) inner: finstack_quant_portfolio::metrics::PortfolioMetrics,
}

impl PyPortfolioMetrics {
    pub(crate) fn from_inner(inner: finstack_quant_portfolio::metrics::PortfolioMetrics) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPortfolioMetrics {
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

    /// Parse aggregate portfolio metrics from canonical JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(metrics_json)")]
    fn from_json(py: Python<'_>, metrics_json: &str) -> PyResult<Self> {
        let metrics_json = metrics_json.to_owned();
        let inner = py
            .detach(move || serde_json::from_str(&metrics_json))
            .map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize aggregate portfolio metrics to canonical JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self, py: Python<'_>) -> PyResult<String> {
        let metrics = &self.inner;
        py.detach(|| serde_json::to_string(metrics))
            .map_err(display_to_py)
    }

    /// Portfolio-wide aggregated metrics keyed by metric id.
    ///
    /// Each value carries ``metric_id``, ``total`` and the ``by_entity``
    /// breakdown, in canonical Rust ``IndexMap`` insertion order. Only summable
    /// metrics are aggregated.
    #[getter]
    fn aggregated<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.aggregated)
    }

    /// Raw per-position metric values keyed by position id.
    ///
    /// Each value carries the position's native ``currency`` — non-summable
    /// metrics are quoted in it — and its ``metrics`` mapping.
    #[getter]
    fn by_position<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.by_position)
    }

    /// One aggregated metric (``{"metric_id", "total", "by_entity"}``) or
    /// ``None`` when ``metric_id`` was not aggregated.
    #[pyo3(text_signature = "(self, metric_id)")]
    fn get_metric<'py>(
        &self,
        py: Python<'py>,
        metric_id: &str,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .get_metric(metric_id)
            .map(|metric| serde_to_py(py, metric))
            .transpose()
    }

    /// One position's raw metrics (``{"currency", "metrics"}``) or ``None``
    /// when ``position_id`` is absent.
    #[pyo3(text_signature = "(self, position_id)")]
    fn get_position_metrics<'py>(
        &self,
        py: Python<'py>,
        position_id: &str,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .get_position_metrics(position_id)
            .map(|metrics| serde_to_py(py, metrics))
            .transpose()
    }

    /// Portfolio total of one aggregated metric, or ``None`` when absent.
    #[pyo3(text_signature = "(self, metric_id)")]
    fn get_total(&self, metric_id: &str) -> Option<f64> {
        self.inner.get_total(metric_id)
    }

    /// Metric values excluded from aggregation because they were non-finite.
    ///
    /// A non-empty list means aggregation is incomplete; each entry records the
    /// ``position_id``, ``metric_id`` and the offending ``value``.
    #[getter]
    fn skipped_metrics<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.skipped_metrics)
    }

    /// Positions that carried no risk measures because their valuation fell
    /// back to PV-only.
    ///
    /// Such positions contribute zero to every total without producing a
    /// ``skipped_metrics`` entry, so a non-empty list is the only signal that
    /// the aggregate is partial.
    #[getter]
    fn degraded_positions(&self) -> Vec<String> {
        self.inner
            .degraded_positions
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect()
    }

    /// Metric identifiers present in ``by_position`` that were not aggregated
    /// into a portfolio total because they are not summable across positions
    /// (for example ``ytm`` or ``duration``). Sorted and de-duplicated.
    #[getter]
    fn unaggregated_metrics(&self) -> Vec<String> {
        self.inner.unaggregated_metrics.clone()
    }

    /// Decoded components, total, and ordered entity breakdown by base metric.
    ///
    /// Despite the ``_series`` suffix (which mirrors the Rust name) this is a
    /// plain ``list`` of tuples, not a :class:`pandas.Series`. Use
    /// :meth:`to_dataframe` for the tabular view.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the base metric key is not canonically encoded.
    #[pyo3(text_signature = "(self, base)")]
    fn metric_series(&self, py: Python<'_>, base: &str) -> PyResult<Vec<PyMetricSeriesEntry>> {
        let base = base.parse().map_err(core_to_py)?;
        self.inner
            .metric_series(&base)
            .into_iter()
            .map(|(components, aggregate)| {
                let by_entity = PyDict::new(py);
                for (entity, value) in &aggregate.by_entity {
                    by_entity.set_item(entity.to_string(), value)?;
                }
                Ok((components, aggregate.total, by_entity.unbind()))
            })
            .collect()
    }

    /// Export the portfolio-wide aggregated metrics as a pandas ``DataFrame``.
    ///
    /// One row per entry of the ``aggregated`` map, in canonical Rust
    /// ``IndexMap`` insertion order. Built from the canonical
    /// ``finstack_quant_portfolio::dataframe::aggregated_metrics_to_table`` envelope so
    /// the frame cannot drift from the Rust table export. The per-entity
    /// breakdown is not flattened here — reach it through
    /// :meth:`metric_series`; the per-position long table comes from
    /// :meth:`to_position_dataframe`.
    ///
    /// Columns: ``metric_id``, ``total`` (sum across positions; only summable
    /// metrics are aggregated).
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let table = finstack_quant_portfolio::dataframe::aggregated_metrics_to_table(&self.inner)
            .map_err(core_to_py)?;
        envelope_to_pandas(py, &table)
    }

    /// Export the raw per-position metric values as a long-format pandas
    /// ``DataFrame``.
    ///
    /// One row per ``(position, metric)`` pair — the row count is the total
    /// number of metric values across positions, not the number of positions.
    /// Pivot with ``df.pivot(index="position_id", columns="metric_id",
    /// values="value")`` for a wide view. Built from the canonical
    /// ``finstack_quant_portfolio::dataframe::metrics_to_table`` envelope so the frame
    /// cannot drift from the Rust table export.
    ///
    /// Columns: ``metric_id``, ``position_id``, ``currency`` (the position's
    /// native currency; non-summable metrics are quoted in it), ``value``.
    #[pyo3(text_signature = "(self)")]
    fn to_position_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let table = finstack_quant_portfolio::dataframe::metrics_to_table(&self.inner)
            .map_err(core_to_py)?;
        envelope_to_pandas(py, &table)
    }

    fn __repr__(&self) -> String {
        format!(
            "PortfolioMetrics(aggregated={}, positions={})",
            self.inner.aggregated.len(),
            self.inner.by_position.len(),
        )
    }

    /// Render as an HTML table in Jupyter notebooks (delegates to
    /// :meth:`to_dataframe`; ``None`` when the frame cannot be built).
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Aggregate portfolio metrics from a valuation.
///
/// Parameters
/// ----------
/// valuation : PortfolioValuation | str
///     A :class:`PortfolioValuation` object (fast path) or JSON string.
/// base_currency : Currency | str
///     Base currency (``Currency`` or ISO-4217 code); an unknown code raises
///     ``ValueError``.
/// market : MarketContext | str
///     A ``MarketContext`` object or a JSON string.
/// as_of : datetime.date | str
///     Valuation date, either a date-like object (``datetime.date``,
///     ``pandas.Timestamp``) or an ISO 8601 string.
///
/// Returns
/// -------
/// PortfolioMetrics
///     Typed aggregate-metrics wrapper. Use :meth:`to_json`
///     for the raw wire string.
#[pyfunction]
#[pyo3(text_signature = "(valuation, base_currency, market, as_of)")]
pub fn aggregate_metrics(
    py: Python<'_>,
    valuation: &Bound<'_, PyAny>,
    base_currency: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
) -> PyResult<crate::bindings::portfolio::metrics::PyPortfolioMetrics> {
    let valuation = extract_valuation_ref(py, valuation)?;
    let ccy = extract_currency(base_currency)?;
    let market = extract_market_ref(py, market)?;
    let date = crate::bindings::date_utils::py_to_date(as_of)?;
    let valuation_ref: &finstack_quant_portfolio::valuation::PortfolioValuation = &valuation;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
    let metrics = py
        .detach(|| {
            finstack_quant_portfolio::metrics::aggregate_metrics(
                valuation_ref,
                ccy,
                market_ref,
                date,
            )
        })
        .map_err(portfolio_to_py)?;
    Ok(PyPortfolioMetrics::from_inner(metrics))
}

/// Register the metrics bindings on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPortfolioMetrics>()?;
    m.add_function(pyo3::wrap_pyfunction!(aggregate_metrics, m)?)?;
    Ok(())
}
