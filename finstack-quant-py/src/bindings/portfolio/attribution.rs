//! Portfolio-level P&L attribution bindings.

use crate::bindings::attribution::pnl_attribution::PyPnlAttribution;
use crate::bindings::core::money::PyMoney;
use crate::bindings::extract::{extract_market_ref, extract_portfolio_ref};
use crate::bindings::module_utils::py_to_json_string;
use crate::bindings::pandas_utils::{
    dict_to_dataframe, serde_object_to_single_row_dataframe_with_schema,
};
use crate::errors::{display_to_py, portfolio_to_py, serde_json_to_py};
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Reconciliation of the portfolio factor buckets against ``total_pnl``.
///
/// Returned by :meth:`PortfolioAttribution.reconciliation_check`. The residual
/// is ``total_pnl - (sum of factor buckets + fx_translation_pnl)`` in the
/// portfolio base currency; ``is_reconciled`` is forced ``False`` when the
/// attribution was flagged invalid, whatever the numeric residual.
#[pyclass(
    name = "ReconciliationReport",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyReconciliationReport {
    pub(crate) inner: finstack_quant_portfolio::attribution::ReconciliationReport,
}

#[pymethods]
impl PyReconciliationReport {
    /// Unexplained base-currency amount after all buckets are summed.
    #[getter]
    fn total_residual(&self) -> f64 {
        self.inner.total_residual
    }

    /// Whether ``abs(total_residual) <= tolerance`` and the attribution is valid.
    #[getter]
    fn is_reconciled(&self) -> bool {
        self.inner.is_reconciled
    }

    /// Absolute base-currency tolerance used for the check.
    #[getter]
    fn tolerance(&self) -> f64 {
        self.inner.tolerance
    }

    /// Single-row :class:`pandas.DataFrame` view of the report.
    ///
    /// Columns: ``total_residual``, ``is_reconciled``, ``tolerance``.
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &self.inner,
            &["total_residual", "is_reconciled", "tolerance"],
        )
    }

    /// Serialize to a compact JSON string.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize from JSON produced by :meth:`to_json`.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Support `pickle` via the same serde round-trip as ``to_json``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "ReconciliationReport(total_residual={}, is_reconciled={}, tolerance={})",
            self.inner.total_residual,
            if self.inner.is_reconciled {
                "True"
            } else {
                "False"
            },
            self.inner.tolerance,
        )
    }
}

/// Portfolio-level P&L attribution result.
///
/// Aggregate fields are currency-tagged :class:`~finstack_quant.core.money.Money`
/// values computed by Rust. Per-position attributions are typed
/// ``PnlAttribution`` objects (:attr:`by_position`); the aggregate detail
/// blocks are exposed through the ``*_detail`` getters.
#[pyclass(
    name = "PortfolioAttribution",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyPortfolioAttribution {
    inner: finstack_quant_portfolio::attribution::PortfolioAttribution,
}

impl PyPortfolioAttribution {
    fn from_inner(inner: finstack_quant_portfolio::attribution::PortfolioAttribution) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPortfolioAttribution {
    /// Serialize the complete canonical attribution payload to compact JSON.
    fn to_json(&self, py: Python<'_>) -> PyResult<String> {
        let attribution = &self.inner;
        py.detach(|| serde_json::to_string(attribution))
            .map_err(display_to_py)
    }

    /// Serialize the position-native nested attribution map to compact JSON.
    ///
    /// Position keys retain the canonical Rust ``IndexMap`` insertion order.
    fn by_position_json(&self, py: Python<'_>) -> PyResult<String> {
        let by_position = &self.inner.by_position;
        py.detach(|| serde_json::to_string(by_position))
            .map_err(display_to_py)
    }

    /// Check that aggregate factor P&L reconciles to total P&L.
    ///
    /// Parameters
    /// ----------
    /// tolerance : float
    ///     Absolute tolerance in base-currency units (``0.01`` for one cent).
    ///
    /// Returns
    /// -------
    /// ReconciliationReport
    ///     Typed report with ``total_residual``, ``is_reconciled`` and
    ///     ``tolerance``.
    #[pyo3(text_signature = "(self, tolerance)")]
    fn reconciliation_check(&self, tolerance: f64) -> PyReconciliationReport {
        PyReconciliationReport {
            inner: self.inner.reconciliation_check(tolerance),
        }
    }

    /// Per-position attributions in each instrument's native currency, keyed
    /// by position id in canonical order.
    ///
    /// Values are typed :class:`~finstack_quant.attribution.PnlAttribution`
    /// objects; they exclude FX translation, so they do not sum to the
    /// base-currency portfolio aggregates.
    #[getter]
    fn by_position<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        for (id, attribution) in &self.inner.by_position {
            out.set_item(
                id.as_str(),
                PyPnlAttribution {
                    inner: attribution.clone(),
                },
            )?;
        }
        Ok(out)
    }

    /// Export the per-position native-currency attributions as a pandas
    /// ``DataFrame``, one row per position.
    ///
    /// Columns: ``position_id``, ``currency`` (native), ``total_pnl``,
    /// ``carry``, ``rates_curves_pnl``, ``credit_curves_pnl``,
    /// ``inflation_curves_pnl``, ``correlations_pnl``, ``fx_pnl``,
    /// ``cross_factor_pnl``, ``vol_pnl``, ``model_params_pnl``,
    /// ``market_scalars_pnl``, ``residual``, ``result_invalid``.
    #[pyo3(text_signature = "(self)")]
    fn to_position_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = &self.inner.by_position;
        let data = PyDict::new(py);
        let ids: Vec<&str> = rows.keys().map(|id| id.as_str()).collect();
        data.set_item("position_id", ids)?;
        let currencies: Vec<String> = rows
            .values()
            .map(|a| a.total_pnl.currency().to_string())
            .collect();
        data.set_item("currency", currencies)?;
        macro_rules! money_column {
            ($name:literal, $field:ident) => {
                let values: Vec<f64> = rows.values().map(|a| a.$field.amount()).collect();
                data.set_item($name, values)?;
            };
        }
        money_column!("total_pnl", total_pnl);
        money_column!("carry", carry);
        money_column!("rates_curves_pnl", rates_curves_pnl);
        money_column!("credit_curves_pnl", credit_curves_pnl);
        money_column!("inflation_curves_pnl", inflation_curves_pnl);
        money_column!("correlations_pnl", correlations_pnl);
        money_column!("fx_pnl", fx_pnl);
        money_column!("cross_factor_pnl", cross_factor_pnl);
        money_column!("vol_pnl", vol_pnl);
        money_column!("model_params_pnl", model_params_pnl);
        money_column!("market_scalars_pnl", market_scalars_pnl);
        money_column!("residual", residual);
        let invalid: Vec<bool> = rows.values().map(|a| a.result_invalid).collect();
        data.set_item("result_invalid", invalid)?;
        dict_to_dataframe(py, &data, None)
    }

    /// Human-readable explanation tree of the portfolio-level buckets with
    /// each bucket's share of ``total_pnl`` (mirrors the Rust ``explain``).
    #[pyo3(text_signature = "(self)")]
    fn explain(&self) -> String {
        self.inner.explain()
    }
    /// Export the portfolio-level factor totals as a single-row pandas ``DataFrame``.
    ///
    /// Every ``Money`` aggregate is flattened to a float column plus one
    /// shared ``currency`` column; the per-position nested breakdown stays on
    /// :meth:`by_position_json`.
    ///
    /// Columns: ``currency``, ``total_pnl``, ``carry``, ``rates_curves_pnl``,
    /// ``credit_curves_pnl``, ``inflation_curves_pnl``, ``correlations_pnl``,
    /// ``fx_pnl``, ``fx_translation_pnl``, ``cross_factor_pnl``, ``vol_pnl``,
    /// ``model_params_pnl``, ``market_scalars_pnl``, ``residual``,
    /// ``result_invalid``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        // `currency` is taken from `total_pnl`; the aggregates are guaranteed
        // currency-consistent by the Rust attribution engine.
        let row = serde_json::json!({
            "currency": self.inner.total_pnl.currency().to_string(),
            "total_pnl": self.inner.total_pnl.amount(),
            "carry": self.inner.carry.amount(),
            "rates_curves_pnl": self.inner.rates_curves_pnl.amount(),
            "credit_curves_pnl": self.inner.credit_curves_pnl.amount(),
            "inflation_curves_pnl": self.inner.inflation_curves_pnl.amount(),
            "correlations_pnl": self.inner.correlations_pnl.amount(),
            "fx_pnl": self.inner.fx_pnl.amount(),
            "fx_translation_pnl": self.inner.fx_translation_pnl.amount(),
            "cross_factor_pnl": self.inner.cross_factor_pnl.amount(),
            "vol_pnl": self.inner.vol_pnl.amount(),
            "model_params_pnl": self.inner.model_params_pnl.amount(),
            "market_scalars_pnl": self.inner.market_scalars_pnl.amount(),
            "residual": self.inner.residual.amount(),
            // Lets downstream pipelines refuse to aggregate attributions that
            // Rust flagged invalid (non-finite sensitivities, residual failures).
            "result_invalid": self.inner.result_invalid,
        });
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "currency",
                "total_pnl",
                "carry",
                "rates_curves_pnl",
                "credit_curves_pnl",
                "inflation_curves_pnl",
                "correlations_pnl",
                "fx_pnl",
                "fx_translation_pnl",
                "cross_factor_pnl",
                "vol_pnl",
                "model_params_pnl",
                "market_scalars_pnl",
                "residual",
                "result_invalid",
            ],
        )
    }

    /// Total portfolio P&L between the two market snapshots.
    #[getter]
    fn total_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.total_pnl)
    }

    /// Carry (time-decay plus coupon/dividend income) component of total P&L.
    #[getter]
    fn carry(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.carry)
    }

    /// P&L attributed to interest-rate (discount and forward) curve moves.
    #[getter]
    fn rates_curves_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.rates_curves_pnl)
    }

    /// P&L attributed to credit-spread / hazard-rate curve moves.
    #[getter]
    fn credit_curves_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.credit_curves_pnl)
    }

    /// P&L attributed to inflation curve moves.
    #[getter]
    fn inflation_curves_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.inflation_curves_pnl)
    }

    /// P&L attributed to correlation-input moves.
    #[getter]
    fn correlations_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.correlations_pnl)
    }

    /// P&L attributed to FX spot/forward moves on FX-sensitive instruments.
    ///
    /// Distinct from :attr:`fx_translation_pnl`, which covers restating
    /// unchanged foreign-currency values into the base currency.
    #[getter]
    fn fx_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.fx_pnl)
    }

    /// P&L from translating non-base-currency position values into the base
    /// currency at the new FX rates.
    #[getter]
    fn fx_translation_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.fx_translation_pnl)
    }

    /// Second-order P&L from joint moves of two or more factors.
    #[getter]
    fn cross_factor_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.cross_factor_pnl)
    }

    /// P&L attributed to implied-volatility surface moves.
    #[getter]
    fn vol_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.vol_pnl)
    }

    /// P&L attributed to changes in calibrated model parameters.
    #[getter]
    fn model_params_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.model_params_pnl)
    }

    /// P&L attributed to scalar market inputs (spots, dividends, recovery rates).
    #[getter]
    fn market_scalars_pnl(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.market_scalars_pnl)
    }

    /// Unexplained P&L left after every factor has been attributed.
    ///
    /// Returns
    /// -------
    /// Money
    ///     ``total_pnl`` less the sum of all factor components. Check it
    ///     against a tolerance with :meth:`reconciliation_check`.
    #[getter]
    fn residual(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.residual)
    }

    /// Whether Rust flagged the attribution as unusable.
    ///
    /// Returns
    /// -------
    /// bool
    ///     ``True`` when at least one position produced non-finite
    ///     sensitivities or failed residual computation. Aggregating an
    ///     invalid result across instruments is not meaningful.
    #[getter]
    fn result_invalid(&self) -> bool {
        self.inner.result_invalid
    }

    fn __repr__(&self) -> String {
        format!(
            "PortfolioAttribution(total_pnl={}, positions={}, result_invalid={})",
            self.inner.total_pnl,
            self.inner.by_position.len(),
            self.inner.result_invalid,
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

    /// Deserialize from JSON produced by `to_json`.
    ///
    /// Completes the wire round-trip, which is also what makes this type
    /// picklable (see `__reduce__`).
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_portfolio::attribution::PortfolioAttribution =
            serde_json::from_str(json).map_err(crate::errors::display_to_py)?;
        Ok(Self { inner })
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json(py)?)
    }
}

/// Attribute portfolio P&L between two market snapshots.
///
/// ``portfolio`` and both markets accept either typed binding objects or their
/// canonical JSON representations. ``method`` uses the same serde shape as
/// instrument attribution (for example ``"parallel"`` or
/// ``{"waterfall": ["carry", "rates_curves"]}``). ``config`` is an optional
/// canonical ``FinstackConfig`` dictionary or JSON string. ``as_of_t0`` and
/// ``as_of_t1`` are the two snapshot dates, each either a date-like object
/// (``datetime.date``, ``pandas.Timestamp``) or an ISO 8601 string.
#[pyfunction]
#[pyo3(signature = (portfolio, market_t0, market_t1, as_of_t0, as_of_t1, method, config=None))]
#[allow(clippy::too_many_arguments)]
fn attribute_portfolio_pnl(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    market_t0: &Bound<'_, PyAny>,
    market_t1: &Bound<'_, PyAny>,
    as_of_t0: &Bound<'_, PyAny>,
    as_of_t1: &Bound<'_, PyAny>,
    method: &Bound<'_, PyAny>,
    config: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyPortfolioAttribution> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let market_t0 = extract_market_ref(py, market_t0)?;
    let market_t1 = extract_market_ref(py, market_t1)?;
    let as_of_t0 = crate::bindings::date_utils::extract_date(as_of_t0)?;
    let as_of_t1 = crate::bindings::date_utils::extract_date(as_of_t1)?;

    let method_json = py_to_json_string(py, method, "method")?;
    let config_json = config
        .map(|value| py_to_json_string(py, value, "config"))
        .transpose()?;
    let (method, config): (
        finstack_quant_portfolio::attribution::AttributionMethod,
        finstack_quant_core::config::FinstackConfig,
    ) = py.detach(move || {
        let method = serde_json::from_str(&method_json)
            .map_err(|error| serde_json_to_py(error, "invalid attribution method"))?;
        let config = config_json
            .map(|json| {
                serde_json::from_str(&json)
                    .map_err(|error| serde_json_to_py(error, "invalid finstack config"))
            })
            .transpose()?
            .unwrap_or_default();
        Ok::<_, PyErr>((method, config))
    })?;

    let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
    let market_t0_ref: &finstack_quant_core::market_data::context::MarketContext = &market_t0;
    let market_t1_ref: &finstack_quant_core::market_data::context::MarketContext = &market_t1;
    let result = py
        .detach(|| {
            finstack_quant_portfolio::attribution::attribute_portfolio_pnl(
                portfolio_ref,
                market_t0_ref,
                market_t1_ref,
                as_of_t0,
                as_of_t1,
                &config,
                method,
            )
        })
        .map_err(portfolio_to_py)?;
    Ok(PyPortfolioAttribution::from_inner(result))
}

pub fn register(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyPortfolioAttribution>()?;
    module.add_class::<PyReconciliationReport>()?;
    module.add_function(pyo3::wrap_pyfunction!(attribute_portfolio_pnl, module)?)?;
    Ok(())
}
