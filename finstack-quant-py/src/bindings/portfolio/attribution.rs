//! Portfolio-level P&L attribution bindings.

use crate::bindings::attribution::pnl_attribution::{PyPnlAttribution, WIDE_COLUMNS};
use crate::bindings::core::money::PyMoney;
use crate::bindings::extract::{extract_market_ref, extract_portfolio_ref};
use crate::bindings::module_utils::py_to_json_string;
use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
    ColumnSchema,
};
use crate::errors::{display_to_py, portfolio_to_py, serde_json_to_py};
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Reconciliation of an attribution's effect buckets against its total.
///
/// Returned by :meth:`PortfolioAttribution.reconciliation_check`, where the
/// residual is ``total_pnl - (sum of factor buckets + fx_translation_pnl)`` in
/// the portfolio base currency and ``is_reconciled`` is forced ``False`` when
/// the attribution was flagged invalid, and by
/// :meth:`FiAttributionResult.reconciliation_check`, where it is
/// ``active_return`` minus the five Campisi effect totals in return units.
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

crate::bindings::macros::wire_methods!(
    PyReconciliationReport,
    finstack_quant_portfolio::attribution::ReconciliationReport,
    "ReconciliationReport"
);

#[pymethods]
impl PyReconciliationReport {
    /// Unexplained amount after all buckets are summed (base currency for P&L
    /// attribution, return units for Campisi attribution).
    #[getter]
    fn total_residual(&self) -> f64 {
        self.inner.total_residual
    }

    /// Whether ``abs(total_residual) <= tolerance`` (and, for P&L attribution,
    /// the attribution is valid).
    #[getter]
    fn is_reconciled(&self) -> bool {
        self.inner.is_reconciled
    }

    /// Absolute tolerance used for the check, in the units of ``total_residual``.
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

crate::bindings::macros::wire_methods!(
    PyPortfolioAttribution,
    finstack_quant_portfolio::attribution::PortfolioAttribution,
    "PortfolioAttribution"
);

impl PyPortfolioAttribution {
    fn from_inner(inner: finstack_quant_portfolio::attribution::PortfolioAttribution) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPortfolioAttribution {
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
    /// Each row is ``position_id`` followed by the canonical Rust wide row
    /// (``finstack_quant_attribution::long_rows::pnl_attribution_wide_row``) of
    /// that position's attribution, the same columns as
    /// :meth:`finstack_quant.attribution.PnlAttribution.to_dataframe`:
    /// ``instrument_id``, ``method``, ``t0``, ``t1``, ``currency`` (native),
    /// ``total_pnl``, ``mark_to_market_pnl``, ``pv_t0``, ``pv_t1``, ``carry``,
    /// ``rates_curves_pnl``, ``credit_curves_pnl``, ``inflation_curves_pnl``,
    /// ``correlations_pnl``, ``fx_pnl``, ``fx_translation_pnl``, ``vol_pnl``,
    /// ``cross_factor_pnl``, ``model_params_pnl``, ``market_scalars_pnl``,
    /// ``residual``, ``residual_pct``, ``num_repricings``, ``result_invalid``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a position attribution mixes currencies across its factors.
    #[pyo3(text_signature = "(self)")]
    fn to_position_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        #[derive(serde::Serialize)]
        struct PositionRow<'a> {
            position_id: &'a str,
            #[serde(flatten)]
            row: finstack_quant_attribution::long_rows::PnlAttributionWideRow,
        }
        let rows =
            self.inner
                .by_position
                .iter()
                .map(|(id, attribution)| {
                    finstack_quant_attribution::long_rows::pnl_attribution_wide_row(attribution)
                        .map(|row| PositionRow {
                            position_id: id.as_str(),
                            row,
                        })
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(crate::errors::core_to_py)?;
        let columns: Vec<ColumnSchema<'static>> = std::iter::once(("position_id", "str"))
            .chain(WIDE_COLUMNS)
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, &columns)
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
    let as_of_t0 = crate::bindings::date_utils::py_to_date(as_of_t0)?;
    let as_of_t1 = crate::bindings::date_utils::py_to_date(as_of_t1)?;

    let method_json = py_to_json_string(py, method, "method")?;
    let config_json = config
        .map(|value| py_to_json_string(py, value, "config"))
        .transpose()?;
    let (method, config): (
        finstack_quant_attribution::AttributionMethod,
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
