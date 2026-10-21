//! Python bindings for portfolio valuation: `value_portfolio`, `PortfolioValuation`
//! and `PositionValue`.

use crate::bindings::core::money::PyMoney;
use crate::bindings::core::table::envelope_to_pandas;
use crate::bindings::date_utils::date_to_py;
use crate::bindings::extract::extract_market_ref;
use crate::bindings::extract::extract_portfolio_ref;
use crate::bindings::pandas_utils::serde_to_py;
use crate::errors::core_to_py;
use crate::errors::display_to_py;
use crate::errors::portfolio_to_py;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::PyAny;
use pyo3::types::PyDict;
use pyo3::types::PyList;
use pyo3::types::PyModule;

/// Valuation of a single position inside a ``PortfolioValuation``.
///
/// Carries the native- and base-currency values, the linear scale applied to
/// summable risk metrics, and the risk-completeness diagnostics recorded when
/// a position fell back to PV-only valuation.
#[pyclass(
    name = "PositionValue",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPositionValue {
    pub(crate) inner: finstack_quant_portfolio::valuation::PositionValue,
}

crate::bindings::macros::wire_methods!(
    PyPositionValue,
    finstack_quant_portfolio::valuation::PositionValue,
    "PositionValue"
);

#[pymethods]
impl PyPositionValue {
    /// Position identifier.
    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    /// Owning entity identifier.
    #[getter]
    fn entity_id(&self) -> String {
        self.inner.entity_id.as_str().to_owned()
    }

    /// Value in the instrument's native currency.
    #[getter]
    fn value_native(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.value_native)
    }

    /// Value converted to the portfolio base currency.
    #[getter]
    fn value_base(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.value_base)
    }

    /// Spot FX rate applied to convert ``value_native`` into ``value_base``:
    /// base-currency units per one unit of the native currency, so
    /// ``value_native.amount * fx_rate == value_base.amount`` up to base
    /// currency rounding. ``None`` when the native currency is the base
    /// currency (no FX lookup is made).
    #[getter]
    fn fx_rate(&self) -> Option<f64> {
        self.inner.fx_rate
    }

    /// Whether ``fx_rate`` was triangulated through the FX matrix's pivot
    /// currency (``True``) or read from a direct or inverse quote (``False``).
    /// ``None`` exactly when ``fx_rate`` is ``None``.
    #[getter]
    fn fx_triangulated(&self) -> Option<bool> {
        self.inner.fx_triangulated
    }

    /// Linear scale applied to summable risk metrics (position size and sign).
    #[getter]
    fn metric_scale(&self) -> f64 {
        self.inner.metric_scale
    }

    /// Whether every risk metric requested *of this position* was computed.
    ///
    /// Metrics narrowed away as inapplicable were never requested, so they do
    /// not clear this flag; see ``inapplicable_metrics``.
    #[getter]
    fn risk_metrics_complete(&self) -> bool {
        self.inner.risk_metrics_complete
    }

    /// Original metric-failure message when the valuation fell back to
    /// PV-only, otherwise ``None``.
    #[getter]
    fn risk_error(&self) -> Option<String> {
        self.inner.risk_error.clone()
    }

    /// Requested metrics this position's instrument type has no calculator
    /// for, in request order.
    #[getter]
    fn inapplicable_metrics(&self) -> Vec<String> {
        self.inner
            .inapplicable_metrics
            .iter()
            .map(|metric| metric.as_str().to_owned())
            .collect()
    }

    /// Full instrument ``ValuationResult`` as a JSON-shaped ``dict`` (metrics
    /// included), or ``None`` when it was not retained.
    #[getter]
    fn valuation_result<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .valuation_result
            .as_ref()
            .map(|r| serde_to_py(py, r))
            .transpose()
    }

    fn __repr__(&self) -> String {
        format!(
            "PositionValue(position_id={:?}, value_base={} {}, risk_metrics_complete={})",
            self.inner.position_id.as_str(),
            self.inner.value_base.amount(),
            self.inner.value_base.currency(),
            if self.inner.risk_metrics_complete {
                "True"
            } else {
                "False"
            },
        )
    }
}

/// Complete portfolio valuation: per-position values, entity rollups and the
/// base-currency total, plus the FX policy stamp and degraded-risk
/// diagnostics.
///
/// Avoids re-parsing the (potentially large) valuation JSON every time a
/// downstream function (``aggregate_metrics``, ``PortfolioResult``) needs to
/// read from it.
#[pyclass(
    name = "PortfolioValuation",
    module = "finstack_quant.portfolio",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPortfolioValuation {
    pub(crate) inner: finstack_quant_portfolio::valuation::PortfolioValuation,
}

impl PyPortfolioValuation {
    pub(crate) fn from_inner(
        inner: finstack_quant_portfolio::valuation::PortfolioValuation,
    ) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPortfolioValuation {
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

    /// Parse a valuation from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(valuation_json)")]
    fn from_json(py: Python<'_>, valuation_json: &str) -> PyResult<Self> {
        let valuation_json = valuation_json.to_owned();
        let inner: finstack_quant_portfolio::valuation::PortfolioValuation = py
            .detach(move || serde_json::from_str(&valuation_json))
            .map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize back to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self, py: Python<'_>) -> PyResult<String> {
        let valuation = &self.inner;
        py.detach(|| serde_json::to_string(valuation))
            .map_err(display_to_py)
    }

    /// Total portfolio value in the base currency (amount).
    #[getter]
    fn total_value(&self) -> f64 {
        self.inner.total_base_currency.amount()
    }

    /// Base currency of the total.
    #[getter]
    fn base_currency(&self) -> String {
        self.inner.total_base_currency.currency().to_string()
    }

    /// Valuation date as a ``datetime.date``.
    #[getter]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.as_of)
    }

    /// Per-position valuations keyed by position id, in valuation order.
    #[getter]
    fn position_values<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        for (id, value) in &self.inner.position_values {
            out.set_item(
                id.as_str(),
                PyPositionValue {
                    inner: value.clone(),
                },
            )?;
        }
        Ok(out)
    }

    /// Base-currency totals by entity id, in valuation order.
    #[getter]
    fn by_entity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        for (id, money) in &self.inner.by_entity {
            out.set_item(id.as_str(), PyMoney::from_inner(*money))?;
        }
        Ok(out)
    }

    /// Positions whose valuation fell back to PV-only because a requested
    /// risk metric could not be computed.
    #[getter]
    fn degraded_positions(&self) -> Vec<String> {
        self.inner
            .degraded_positions
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect()
    }

    /// FX conversion policy stamped on the base-currency rollups (serde
    /// name, e.g. ``"cashflow_date"``).
    #[getter]
    fn fx_collapse_policy(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(&self.inner.fx_collapse_policy).map_err(core_to_py)
    }

    /// Look up one position's valuation.
    ///
    /// Raises ``KeyError`` when ``position_id`` is not in the valuation.
    #[pyo3(text_signature = "(self, position_id)")]
    fn get_position_value(&self, position_id: &str) -> PyResult<PyPositionValue> {
        self.inner
            .get_position_value(position_id)
            .map(|value| PyPositionValue {
                inner: value.clone(),
            })
            .ok_or_else(|| PyKeyError::new_err(format!("position '{position_id}' not valued")))
    }

    /// Look up one entity's base-currency total.
    ///
    /// Raises ``KeyError`` when ``entity_id`` has no positions.
    #[pyo3(text_signature = "(self, entity_id)")]
    fn get_entity_value(&self, entity_id: &str) -> PyResult<PyMoney> {
        self.inner
            .get_entity_value(entity_id)
            .map(|money| PyMoney::from_inner(*money))
            .ok_or_else(|| PyKeyError::new_err(format!("entity '{entity_id}' not valued")))
    }

    /// Export per-position values via Arrow (zero-copy for consumers).
    ///
    /// Columns: ``position_id``, ``entity_id``, ``value_native``,
    /// ``value_base``, ``currency_native``, ``currency_base``, ``fx_rate``,
    /// ``risk_error`` (see
    /// ``finstack_quant_portfolio::dataframe::positions_to_table``). Returns an
    /// :class:`finstack_quant.core.table.ArrowTable`.
    #[pyo3(text_signature = "($self)")]
    fn to_arrow_positions(&self) -> PyResult<crate::bindings::core::table::PyArrowTable> {
        let table = finstack_quant_portfolio::dataframe::positions_to_table(&self.inner)
            .map_err(crate::errors::core_to_py)?;
        crate::bindings::core::table::PyArrowTable::from_envelope(&table)
    }

    /// Export per-position values as a pandas ``DataFrame``.
    ///
    /// One row per entry of ``position_values``. Built from the same
    /// ``positions_to_table`` envelope that backs :meth:`to_arrow_positions`
    /// plus the two boolean risk diagnostics, which the table envelope cannot
    /// carry (it has no boolean column type).
    ///
    /// Columns: ``position_id``, ``entity_id``, ``value_native``,
    /// ``value_base``, ``currency_native``, ``currency_base``, ``fx_rate``
    /// (base-currency units per native unit applied to the row; missing when
    /// the native currency is the base currency), ``fx_triangulated``
    /// (``True``/``False`` for a converted row, else ``None``),
    /// ``risk_metrics_complete`` (bool), ``risk_error`` (``None`` unless the
    /// position degraded to PV-only). Values are floats and the currency codes
    /// are strings; use :meth:`to_arrow_positions` when a zero-copy handoff
    /// matters more than pandas ergonomics.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let table = finstack_quant_portfolio::dataframe::positions_to_table(&self.inner)
            .map_err(core_to_py)?;
        let frame = envelope_to_pandas(py, &table)?;
        let positions = self.inner.position_values.values();
        let fx_triangulated: Vec<Option<bool>> =
            positions.clone().map(|v| v.fx_triangulated).collect();
        let complete: Vec<bool> = positions.map(|v| v.risk_metrics_complete).collect();
        // `assign` returns a new frame. Appending with `set_item` instead trips
        // pandas' copy-on-write heuristic (PyO3 holds an extra reference, so the
        // refcount check reads as chained assignment) and warns on every call.
        let columns = PyDict::new(py);
        columns.set_item("fx_triangulated", fx_triangulated)?;
        columns.set_item("risk_metrics_complete", complete)?;
        let frame = frame.call_method("assign", (), Some(&columns))?;
        let order = [
            "position_id",
            "entity_id",
            "value_native",
            "value_base",
            "currency_native",
            "currency_base",
            "fx_rate",
            "fx_triangulated",
            "risk_metrics_complete",
            "risk_error",
        ];
        frame.get_item(PyList::new(py, order)?)
    }

    /// Number of position valuations in the result.
    fn __len__(&self) -> usize {
        self.inner.position_values.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "PortfolioValuation(as_of={}, total={} {}, positions={}, degraded={})",
            self.inner.as_of,
            self.inner.total_base_currency.amount(),
            self.inner.total_base_currency.currency(),
            self.inner.position_values.len(),
            self.inner.degraded_positions.len(),
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

/// Value a portfolio.
///
/// Parameters
/// ----------
/// portfolio : Portfolio | str
///     A :class:`Portfolio` object (fast path, no rebuild) or a
///     JSON-serialized ``PortfolioSpec`` string.
/// market : MarketContext | str
///     A ``MarketContext`` object or a JSON string.
/// strict_risk : bool | None
///     ``None`` uses the Rust ``PortfolioValuationOptions`` default, ``True``:
///     any risk metric failure aborts the entire valuation. Set ``False``
///     only for an intentional PV-preserving fallback that records failed
///     metrics as diagnostics.
/// metrics : list[str] | None
///     Risk metrics to offer every position. ``None`` requests the standard
///     set (PV plus ``dv01``, which every rate-sensitive pricer supports);
///     an empty list performs PV-only valuation. Pricer-specific metrics
///     such as ``theta``, ``cs01`` or the Greeks must be listed explicitly.
///     Names resolve exactly as in ``price_instrument``: every id
///     ``list_standard_metrics()`` returns is accepted, and an unknown name
///     raises ``ValueError`` listing the closest standard metrics. The list is
///     a menu: each position is asked for the entries its instrument can
///     compute (a composite: the additive entries at least one leg supports)
///     and the rest appear on its ``inapplicable_metrics``.
///
/// Returns
/// -------
/// PortfolioValuation
///     Typed valuation wrapper that can be passed directly to
///     ``aggregate_metrics`` without a JSON round-trip.
#[pyfunction]
#[pyo3(signature = (portfolio, market, strict_risk=None, metrics=None))]
fn value_portfolio(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    strict_risk: Option<bool>,
    metrics: Option<Vec<String>>,
) -> PyResult<PyPortfolioValuation> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let market = extract_market_ref(py, market)?;
    let config = finstack_quant_core::config::FinstackConfig::default();
    let mut options = finstack_quant_portfolio::valuation::PortfolioValuationOptions {
        // Strict parse shared with WASM: an unknown metric name is a ValueError
        // naming the closest standard identifiers, not a silent PV-only run.
        metrics: finstack_quant_portfolio::valuation::RequestedMetrics::try_from_metric_names(
            metrics,
        )
        .map_err(crate::errors::core_to_py)?,
        ..Default::default()
    };
    if let Some(strict_risk) = strict_risk {
        options.strict_risk = strict_risk;
    }
    // Release the GIL (PyO3 `detach`) while the CPU-bound Rust valuation runs
    // so other Python threads can execute concurrently. The `*Access` wrappers
    // contain a `PyRef` (not `Ungil`), so we deref to plain Rust references
    // before entering the closure — these are `Send + Sync` and therefore
    // `Ungil`. No Python state is touched inside.
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
    let valuation = py
        .detach(|| {
            finstack_quant_portfolio::valuation::value_portfolio(
                portfolio_ref,
                market_ref,
                &config,
                &options,
            )
        })
        .map_err(portfolio_to_py)?;
    Ok(PyPortfolioValuation::from_inner(valuation))
}

/// Register the valuation bindings on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPositionValue>()?;
    m.add_class::<PyPortfolioValuation>()?;
    m.add_function(pyo3::wrap_pyfunction!(value_portfolio, m)?)?;
    Ok(())
}
