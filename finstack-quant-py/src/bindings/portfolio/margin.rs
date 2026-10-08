//! Portfolio margin aggregation: netting-set initial and variation margin.

use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};
use serde::Serialize;

use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::{extract_market_ref, extract_portfolio_ref};
use crate::bindings::margin::im::PySimmSensitivities;
use crate::bindings::margin::types::{PyImCollateralResult, PyImMethodology, PyNettingSetId};
use crate::bindings::pandas_utils::{serde_rows_to_dataframe_with_schema, ColumnSchema};
use crate::errors::portfolio_to_py;
use finstack_quant_portfolio::{
    NettingSetMargin, PortfolioMarginAggregator, PortfolioMarginResult,
};

/// Column schema for [`PyPortfolioMarginResult::to_dataframe`].
const NETTING_SET_COLUMNS: &[ColumnSchema<'static>] = &[
    ("netting_set_id", "str"),
    ("csa_id", "str"),
    ("is_cleared", "bool"),
    ("initial_margin", "float64"),
    ("variation_margin", "float64"),
    ("total_margin", "float64"),
    ("currency", "str"),
    ("position_count", "int64"),
    ("im_methodology", "str"),
    ("is_approximate", "bool"),
];

#[derive(Serialize)]
struct NettingSetRow {
    netting_set_id: String,
    csa_id: Option<String>,
    is_cleared: bool,
    initial_margin: f64,
    variation_margin: f64,
    total_margin: f64,
    currency: String,
    position_count: usize,
    im_methodology: String,
    is_approximate: bool,
}

/// Column schema for [`PyNettingSetMargin::to_dataframe`].
const IM_BREAKDOWN_COLUMNS: &[ColumnSchema<'static>] = &[
    ("netting_set_id", "str"),
    ("risk_class", "str"),
    ("initial_margin", "float64"),
    ("currency", "str"),
];

#[derive(Serialize)]
struct ImBreakdownRow<'a> {
    netting_set_id: String,
    risk_class: &'a str,
    initial_margin: f64,
    currency: String,
}

/// Netting sets in the canonical wire order (ascending identifier string).
fn sorted_netting_sets(result: &PortfolioMarginResult) -> Vec<&NettingSetMargin> {
    let mut netting_sets: Vec<&NettingSetMargin> = result.by_netting_set.values().collect();
    netting_sets.sort_by_key(|margin| margin.netting_set_id.to_string());
    netting_sets
}

/// Margin results for one netting set, in the portfolio base currency.
///
/// Obtained from :attr:`PortfolioMarginResult.by_netting_set`. ``sensitivities``
/// echoes the netted SIMM inputs the initial margin was computed from.
#[pyclass(
    name = "NettingSetMargin",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyNettingSetMargin {
    pub(crate) inner: NettingSetMargin,
}

crate::bindings::macros::wire_methods!(PyNettingSetMargin, NettingSetMargin, "NettingSetMargin");

#[pymethods]
impl PyNettingSetMargin {
    /// Netting set identifier (bilateral counterparty/CSA or cleared CCP).
    #[getter]
    fn netting_set_id(&self) -> PyNettingSetId {
        PyNettingSetId {
            inner: self.inner.netting_set_id.clone(),
        }
    }

    /// Contractual CSA id used for one-time IM terms; ``None`` without an OTC CSA.
    #[getter]
    fn csa_id(&self) -> Option<String> {
        self.inner.csa_id.clone()
    }

    /// Margin calculation date as a ``datetime.date``.
    #[getter]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.as_of)
    }

    /// Gross model initial margin before CSA collateral terms.
    #[getter]
    fn initial_margin(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.initial_margin)
    }

    /// Signed desk variation-margin outflow: positive posts, negative collects.
    #[getter]
    fn variation_margin(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.variation_margin)
    }

    /// ``initial_margin + max(variation_margin, 0)``.
    #[getter]
    fn total_margin(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.total_margin)
    }

    /// Number of positions aggregated into the netting set.
    #[getter]
    fn position_count(&self) -> usize {
        self.inner.position_count
    }

    /// Initial-margin methodology used for the netting set.
    #[getter]
    fn im_methodology(&self) -> PyImMethodology {
        PyImMethodology {
            inner: self.inner.im_methodology,
        }
    }

    /// Whether the IM uses an approximate model (historical SIMM or CCP proxy).
    #[getter]
    fn is_approximate(&self) -> bool {
        self.inner.is_approximate
    }

    /// Netted SIMM sensitivities the initial margin was computed from, or
    /// ``None`` when the netting set did not go through SIMM.
    #[getter]
    fn sensitivities(&self) -> Option<PySimmSensitivities> {
        self.inner
            .sensitivities
            .clone()
            .map(|inner| PySimmSensitivities { inner })
    }

    /// Initial margin by SIMM risk class, keyed by risk-class name (sorted).
    #[getter]
    fn im_breakdown<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let mut entries: Vec<_> = self.inner.im_breakdown.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        let out = PyDict::new(py);
        for (risk_class, amount) in entries {
            out.set_item(risk_class, PyMoney::from_inner(*amount))?;
        }
        Ok(out)
    }

    /// Whether the netting set is cleared through a CCP.
    #[getter]
    fn is_cleared(&self) -> bool {
        self.inner.is_cleared()
    }

    /// Initial margin by SIMM risk class as a :class:`pandas.DataFrame`.
    ///
    /// Columns: ``netting_set_id``, ``risk_class``, ``initial_margin``,
    /// ``currency``. One row per entry of ``im_breakdown`` (sorted by risk
    /// class); empty when no breakdown was produced.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut rows: Vec<ImBreakdownRow<'_>> = self
            .inner
            .im_breakdown
            .iter()
            .map(|(risk_class, amount)| ImBreakdownRow {
                netting_set_id: self.inner.netting_set_id.to_string(),
                risk_class,
                initial_margin: amount.amount(),
                currency: amount.currency().to_string(),
            })
            .collect();
        rows.sort_by(|a, b| a.risk_class.cmp(b.risk_class));
        serde_rows_to_dataframe_with_schema(py, &rows, IM_BREAKDOWN_COLUMNS)
    }

    fn __repr__(&self) -> String {
        format!(
            "NettingSetMargin(netting_set_id={:?}, initial_margin={} {ccy}, variation_margin={} {ccy}, positions={})",
            self.inner.netting_set_id.to_string(),
            self.inner.initial_margin.amount(),
            self.inner.variation_margin.amount(),
            self.inner.position_count,
            ccy = self.inner.initial_margin.currency(),
        )
    }
}

/// Portfolio-wide margin results in the portfolio base currency.
///
/// Returned by :meth:`PortfolioMarginAggregator.calculate`.
#[pyclass(
    name = "PortfolioMarginResult",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPortfolioMarginResult {
    pub(crate) inner: PortfolioMarginResult,
}

crate::bindings::macros::wire_methods!(
    PyPortfolioMarginResult,
    PortfolioMarginResult,
    "PortfolioMarginResult"
);

#[pymethods]
impl PyPortfolioMarginResult {
    /// Margin calculation date as a ``datetime.date``.
    #[getter]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.as_of)
    }

    /// ISO code of the base currency every aggregate is reported in.
    #[getter]
    fn base_currency(&self) -> String {
        self.inner.base_currency.to_string()
    }

    /// Gross model initial margin across all netting sets, before CSA terms.
    #[getter]
    fn total_initial_margin(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.total_initial_margin)
    }

    /// Signed desk variation-margin outflow across all netting sets.
    #[getter]
    fn total_variation_margin(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.total_variation_margin)
    }

    /// Sum over netting sets of gross IM plus positive VM.
    #[getter]
    fn total_margin(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.total_margin)
    }

    /// One-way IM accounts keyed by contractual CSA id (sorted), each in its
    /// CSA's own currency.
    #[getter]
    fn by_csa<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let mut entries: Vec<_> = self.inner.by_csa.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        let out = PyDict::new(py);
        for (csa_id, account) in entries {
            out.set_item(
                csa_id,
                PyImCollateralResult {
                    inner: account.clone(),
                },
            )?;
        }
        Ok(out)
    }

    /// Sum of target IM account balances, in the base currency.
    #[getter]
    fn total_required_im_collateral(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.total_required_im_collateral)
    }

    /// Signed IM transfers across CSAs in the base currency; positive posts.
    #[getter]
    fn total_im_transfer(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.total_im_transfer)
    }

    /// Required IM held in segregated custody, in the base currency.
    #[getter]
    fn total_segregated_im(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.total_segregated_im)
    }

    /// Per-netting-set results keyed by :class:`NettingSetId`, in ascending
    /// identifier order.
    #[getter]
    fn by_netting_set<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        for margin in sorted_netting_sets(&self.inner) {
            out.set_item(
                PyNettingSetId {
                    inner: margin.netting_set_id.clone(),
                },
                PyNettingSetMargin {
                    inner: margin.clone(),
                },
            )?;
        }
        Ok(out)
    }

    /// Number of positions whose mark-to-market entered a netting set.
    #[getter]
    fn total_positions(&self) -> usize {
        self.inner.total_positions
    }

    /// Number of distinct positions not registered for margin or degraded.
    #[getter]
    fn positions_without_margin(&self) -> usize {
        self.inner.positions_without_margin
    }

    /// ``(position_id, message)`` for every position whose sensitivity or VM
    /// valuation failed during aggregation.
    #[getter]
    fn degraded_positions(&self) -> Vec<(String, String)> {
        self.inner
            .degraded_positions
            .iter()
            .map(|(id, message)| (id.as_str().to_owned(), message.clone()))
            .collect()
    }

    /// Per-netting-set margin as a :class:`pandas.DataFrame`.
    ///
    /// Columns: ``netting_set_id``, ``csa_id``, ``is_cleared``,
    /// ``initial_margin``, ``variation_margin``, ``total_margin``,
    /// ``currency``, ``position_count``, ``im_methodology``,
    /// ``is_approximate``. One row per netting set in ascending identifier
    /// order; amounts are floats in ``currency`` (the base currency).
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<NettingSetRow> = sorted_netting_sets(&self.inner)
            .into_iter()
            .map(|margin| NettingSetRow {
                netting_set_id: margin.netting_set_id.to_string(),
                csa_id: margin.csa_id.clone(),
                is_cleared: margin.is_cleared(),
                initial_margin: margin.initial_margin.amount(),
                variation_margin: margin.variation_margin.amount(),
                total_margin: margin.total_margin.amount(),
                currency: margin.initial_margin.currency().to_string(),
                position_count: margin.position_count,
                im_methodology: margin.im_methodology.to_string(),
                is_approximate: margin.is_approximate,
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, NETTING_SET_COLUMNS)
    }

    fn __repr__(&self) -> String {
        format!(
            "PortfolioMarginResult(as_of={}, total_initial_margin={} {ccy}, total_variation_margin={} {ccy}, netting_sets={}, degraded={})",
            self.inner.as_of,
            self.inner.total_initial_margin.amount(),
            self.inner.total_variation_margin.amount(),
            self.inner.by_netting_set.len(),
            self.inner.degraded_positions.len(),
            ccy = self.inner.base_currency,
        )
    }
}

crate::bindings::macros::impl_repr_html_via_dataframe!(PyPortfolioMarginResult);

/// Aggregates margin requirements across a portfolio by netting set.
///
/// Build one with :meth:`from_portfolio`, then call :meth:`calculate` for each
/// market snapshot. Positions are grouped by the netting set named on their
/// instrument's margin specification; positions without one are counted in
/// ``PortfolioMarginResult.positions_without_margin``.
#[pyclass(
    name = "PortfolioMarginAggregator",
    module = "finstack_quant.portfolio",
    skip_from_py_object
)]
pub struct PyPortfolioMarginAggregator {
    inner: PortfolioMarginAggregator,
}

#[pymethods]
impl PyPortfolioMarginAggregator {
    /// Create an aggregator from a portfolio.
    ///
    /// Parameters
    /// ----------
    /// portfolio : Portfolio | str
    ///     A :class:`Portfolio` object or a JSON-serialized ``PortfolioSpec``.
    ///
    /// Returns
    /// -------
    /// PortfolioMarginAggregator
    ///     Aggregator whose netting sets are seeded from the positions that
    ///     carry margin metadata, reporting in the portfolio base currency.
    ///
    /// Raises
    /// ------
    /// PortfolioError
    ///     If positions in one netting set carry conflicting margin
    ///     specifications.
    #[staticmethod]
    #[pyo3(text_signature = "(portfolio)")]
    fn from_portfolio(py: Python<'_>, portfolio: &Bound<'_, PyAny>) -> PyResult<Self> {
        let portfolio = extract_portfolio_ref(py, portfolio)?;
        let inner =
            PortfolioMarginAggregator::from_portfolio(&portfolio).map_err(portfolio_to_py)?;
        Ok(Self { inner })
    }

    /// Calculate margin requirements for the portfolio.
    ///
    /// Parameters
    /// ----------
    /// portfolio : Portfolio | str
    ///     Portfolio used for mark-to-market and sensitivity lookups; normally
    ///     the one passed to :meth:`from_portfolio`.
    /// market : MarketContext | str
    ///     Market data for VM and SIMM sensitivity extraction, and the FX
    ///     matrix for base-currency reporting.
    /// as_of : datetime.date | str
    ///     Valuation date of the margin run.
    /// current_im_collateral : dict[str, Money] | None
    ///     One-way IM balances already held, keyed by CSA id, each in that
    ///     CSA's currency. ``None`` or a missing key means zero.
    ///
    /// Returns
    /// -------
    /// PortfolioMarginResult
    ///     Per-netting-set and portfolio-level margin in the base currency.
    ///
    /// Raises
    /// ------
    /// PortfolioError
    ///     If ``current_im_collateral`` names an unknown CSA id, or a required
    ///     FX rate is unavailable.
    /// TypeError
    ///     If ``as_of`` is neither a string nor date-like.
    #[pyo3(signature = (portfolio, market, as_of, current_im_collateral=None))]
    #[pyo3(text_signature = "($self, portfolio, market, as_of, current_im_collateral=None)")]
    fn calculate(
        &mut self,
        py: Python<'_>,
        portfolio: &Bound<'_, PyAny>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        current_im_collateral: Option<HashMap<String, PyMoney>>,
    ) -> PyResult<PyPortfolioMarginResult> {
        let portfolio = extract_portfolio_ref(py, portfolio)?;
        let market = extract_market_ref(py, market)?;
        let as_of = py_to_date(as_of)?;
        let collateral: finstack_quant_core::HashMap<String, finstack_quant_core::money::Money> =
            current_im_collateral
                .unwrap_or_default()
                .into_iter()
                .map(|(csa_id, money)| (csa_id, money.inner))
                .collect();
        let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
        let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
        let aggregator = &mut self.inner;
        let inner = py
            .detach(|| aggregator.calculate(portfolio_ref, market_ref, as_of, &collateral))
            .map_err(portfolio_to_py)?;
        Ok(PyPortfolioMarginResult { inner })
    }

    fn __repr__(&self) -> String {
        "PortfolioMarginAggregator(...)".to_string()
    }
}

pub(super) fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyNettingSetMargin>()?;
    m.add_class::<PyPortfolioMarginResult>()?;
    m.add_class::<PyPortfolioMarginAggregator>()?;
    Ok(())
}
