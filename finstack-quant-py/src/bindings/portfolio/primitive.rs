//! Portfolio-wide primitive exposure look-through
//! (`finstack_quant_portfolio::primitive_exposures`).

use pyo3::prelude::*;

use finstack_quant_portfolio::primitive::PortfolioPrimitiveExposureReport;
use finstack_quant_valuations::metrics::MetricId;

use crate::bindings::core::currency::PyCurrency;
use crate::bindings::extract::{extract_market_ref, extract_portfolio_ref};
use crate::bindings::pandas_utils::{serde_rows_to_dataframe_with_schema, ColumnSchema};
use crate::errors::{core_to_py, portfolio_to_py};

use crate::bindings::json_bridge::{deserialize_json, serialize_json};

const AGGREGATE_COLUMNS: &[ColumnSchema<'static>] = &[
    ("instrument_id", "str"),
    ("instrument_type", "str"),
    ("net_quantity", "float64"),
    ("gross_quantity", "float64"),
    ("net_value", "float64"),
    ("gross_value", "float64"),
    ("currency", "str"),
];

const PATH_COLUMNS: &[ColumnSchema<'static>] = &[
    ("position_id", "str"),
    ("path", "str"),
    ("instrument_id", "str"),
    ("instrument_type", "str"),
    ("quantity", "float64"),
    ("value", "float64"),
    ("currency", "str"),
];

/// Portfolio primitive decomposition with path and net/gross concentration views.
///
/// Every amount (values and additive risk measures) is in
/// :attr:`base_currency`. Per-metric ``measures`` / ``net_measures`` /
/// ``gross_measures`` are carried on the :meth:`to_json` wire form.
#[pyclass(
    name = "PortfolioPrimitiveExposureReport",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyPortfolioPrimitiveExposureReport {
    pub(crate) inner: PortfolioPrimitiveExposureReport,
}

#[pymethods]
impl PyPortfolioPrimitiveExposureReport {
    /// Parse a report from its canonical JSON.
    ///
    /// Parameters
    /// ----------
    /// json_str : str
    ///     Strict JSON produced by :meth:`to_json`.
    ///
    /// Returns
    /// -------
    /// PortfolioPrimitiveExposureReport
    ///     Parsed report.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed or does not match the report schema.
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: PortfolioPrimitiveExposureReport = deserialize_json(json_str)?;
        Ok(Self { inner })
    }

    /// Serialize paths and aggregates, including per-metric measures, to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Portfolio reporting currency of every value and risk amount.
    #[getter]
    fn base_currency(&self) -> PyCurrency {
        PyCurrency::from_inner(self.inner.base_currency)
    }

    /// Number of position-aware primitive paths before overlap netting.
    #[getter]
    fn path_count(&self) -> usize {
        self.inner.paths.len()
    }

    /// Number of net/gross aggregates (one per primitive instrument id).
    #[getter]
    fn aggregate_count(&self) -> usize {
        self.inner.aggregates.len()
    }

    /// Export the net/gross aggregates as a pandas ``DataFrame``.
    ///
    /// One row per primitive instrument id, ordered by id. Columns:
    /// ``instrument_id``, ``instrument_type``, ``net_quantity``,
    /// ``gross_quantity``, ``net_value``, ``gross_value``, ``currency``.
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = self
            .inner
            .aggregates
            .iter()
            .map(|aggregate| {
                serde_json::json!({
                    "instrument_id": aggregate.instrument_id.as_str(),
                    "instrument_type": aggregate.instrument_type,
                    "net_quantity": aggregate.net_quantity,
                    "gross_quantity": aggregate.gross_quantity,
                    "net_value": aggregate.net_value.amount(),
                    "gross_value": aggregate.gross_value.amount(),
                    "currency": aggregate.net_value.currency().to_string(),
                })
            })
            .collect::<Vec<_>>();
        serde_rows_to_dataframe_with_schema(py, &rows, AGGREGATE_COLUMNS)
    }

    /// Export the position-aware primitive paths as a pandas ``DataFrame``.
    ///
    /// One row per path. Columns: ``position_id``, ``path`` (composite and
    /// leg ids joined with ``/``), ``instrument_id``, ``instrument_type``,
    /// ``quantity``, ``value``, ``currency``.
    #[pyo3(text_signature = "(self)")]
    fn to_paths_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = self
            .inner
            .paths
            .iter()
            .map(|path| {
                serde_json::json!({
                    "position_id": path.position_id.as_str(),
                    "path": path.path.join("/"),
                    "instrument_id": path.instrument_id.as_str(),
                    "instrument_type": path.instrument_type,
                    "quantity": path.quantity,
                    "value": path.value.amount(),
                    "currency": path.value.currency().to_string(),
                })
            })
            .collect::<Vec<_>>();
        serde_rows_to_dataframe_with_schema(py, &rows, PATH_COLUMNS)
    }

    /// Support pickle through the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "PortfolioPrimitiveExposureReport(base_currency={}, paths={}, aggregates={})",
            self.inner.base_currency,
            self.inner.paths.len(),
            self.inner.aggregates.len()
        )
    }
}

/// Decompose every portfolio position into primitive economic exposures.
///
/// Direct instruments yield one primitive path; composite positions recurse
/// through their frozen resolved leg quantities. The position quantity scales
/// every primitive quantity, value and additive risk amount, and aggregates
/// net and gross exposure per primitive instrument id across positions.
///
/// Parameters
/// ----------
/// portfolio : Portfolio | str
///     Built ``Portfolio`` or ``PortfolioSpec`` JSON; ``base_currency`` is the
///     reporting currency.
/// market : MarketContext | str
///     Complete market for primitive valuation and FX conversion.
/// metrics : list[str]
///     Additive metric ids to report per primitive (e.g. ``"dv01"``); pass
///     ``[]`` for quantity and value only.
///
/// Returns
/// -------
/// PortfolioPrimitiveExposureReport
///     Path-level and net/gross primitive exposures in the base currency.
///
/// Raises
/// ------
/// ValueError
///     If a metric id is not canonical.
/// PortfolioError
///     If a metric is non-additive, primitive instruments sharing an id have
///     conflicting definitions, or a composite is invalid.
/// KeyError
///     If market data or FX needed for valuation is missing.
#[pyfunction]
#[pyo3(text_signature = "(portfolio, market, metrics)")]
fn primitive_exposures(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    metrics: Vec<String>,
) -> PyResult<PyPortfolioPrimitiveExposureReport> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let market = extract_market_ref(py, market)?;
    let metrics = metrics
        .into_iter()
        .map(MetricId::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(core_to_py)?;
    let (portfolio, market) = (&*portfolio, &*market);
    py.detach(move || finstack_quant_portfolio::primitive_exposures(portfolio, market, &metrics))
        .map(|inner| PyPortfolioPrimitiveExposureReport { inner })
        .map_err(portfolio_to_py)
}

/// Register the primitive exposure report class and function.
pub(super) fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPortfolioPrimitiveExposureReport>()?;
    m.add_function(wrap_pyfunction!(primitive_exposures, m)?)?;
    Ok(())
}
