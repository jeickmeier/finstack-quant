//! Portfolio margin aggregation: netting-set initial and variation margin.

use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};
use serde::Serialize;

use crate::bindings::core::money::PyMoney;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::macros::tabular_methods;
use crate::bindings::margin::im::PySimmSensitivities;
use crate::bindings::margin::types::{PyImCollateralResult, PyImMethodology, PyNettingSetId};
use crate::bindings::table_builders::serde_rows_to_arrow_table;
use crate::errors::portfolio_to_py;
use finstack_quant_portfolio::{margin::NettingSetMargin, margin::PortfolioMarginResult};

use crate::bindings::core::config::PyFinstackConfig;
use crate::bindings::core::market_data::context::PyMarketContext;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::margin::PyFrtbSensitivities;
use crate::bindings::module_utils::{parse_currency, py_to_serde};
use crate::bindings::portfolio::container::PyPortfolio;
use finstack_quant_portfolio::margin::{
    CsrQuoteClassification, CsrQuoteDelta, CsrQuoteSource, GirrQuoteDelta, GirrQuoteTarget,
    csr_quote_delta as csr_quote_delta_rs, girr_quote_delta as girr_quote_delta_rs,
};

/// Column schema for [`PyPortfolioMarginResult::to_arrow_table`].
const NETTING_SET_COLUMNS: &[(&str, &str)] = &[
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

/// Column schema for [`PyNettingSetMargin::to_arrow_table`].
const IM_BREAKDOWN_COLUMNS: &[(&str, &str)] = &[
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

impl PyNettingSetMargin {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
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
        serde_rows_to_arrow_table(&rows, IM_BREAKDOWN_COLUMNS)
    }
}

tabular_methods!(PyNettingSetMargin);

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
        for margin in self.inner.netting_sets_sorted() {
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

impl PyPortfolioMarginResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let rows: Vec<NettingSetRow> = self
            .inner
            .netting_sets_sorted()
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
        serde_rows_to_arrow_table(&rows, NETTING_SET_COLUMNS)
    }
}

tabular_methods!(PyPortfolioMarginResult);

/// Calculate portfolio margin requirements by netting set.
///
/// Groups the portfolio's marginable positions into netting sets from their
/// instruments' margin metadata and computes initial and variation margin per
/// netting set, plus one-way CSA IM collateral terms, in the portfolio base
/// currency. Positions without margin metadata are counted in
/// ``PortfolioMarginResult.positions_without_margin``.
///
/// Parameters
/// ----------
/// portfolio : Portfolio
///     A built :class:`Portfolio`.
/// market : MarketContext
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
///     If positions in one netting set carry conflicting margin
///     specifications, ``current_im_collateral`` names an unknown CSA id, or
///     a required FX rate is unavailable.
/// TypeError
///     If ``as_of`` is neither a string nor date-like.
#[pyfunction]
#[pyo3(signature = (portfolio, market, as_of, current_im_collateral=None))]
#[pyo3(text_signature = "(portfolio, market, as_of, current_im_collateral=None)")]
fn portfolio_margin(
    py: Python<'_>,
    portfolio: PyRef<'_, PyPortfolio>,
    market: PyRef<'_, PyMarketContext>,
    as_of: &Bound<'_, PyAny>,
    current_im_collateral: Option<HashMap<String, PyMoney>>,
) -> PyResult<PyPortfolioMarginResult> {
    let portfolio = &portfolio.inner;
    let market = &market.inner;
    let as_of = py_to_date(as_of)?;
    let collateral: finstack_quant_core::FxHashMap<String, finstack_quant_core::money::Money> =
        current_im_collateral
            .unwrap_or_default()
            .into_iter()
            .map(|(csa_id, money)| (csa_id, money.inner))
            .collect();
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = portfolio;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = market;
    let inner = py
        .detach(|| {
            finstack_quant_portfolio::margin::portfolio_margin(
                portfolio_ref,
                market_ref,
                as_of,
                &collateral,
            )
        })
        .map_err(portfolio_to_py)?;
    Ok(PyPortfolioMarginResult { inner })
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "GirrQuoteTarget")]
#[pyclass(
    name = "GirrQuoteTarget",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
struct PyGirrQuoteTarget {
    inner: GirrQuoteTarget,
}

#[pymethods]
impl PyGirrQuoteTarget {
    #[new]
    #[pyo3(signature = (*, risk_currency, risk_curve_id, vertex, source, quote))]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "GirrQuoteTarget.__init__")]
    fn new(
        py: Python<'_>,
        risk_currency: &str,
        risk_curve_id: &str,
        vertex: String,
        source: &Bound<'_, PyAny>,
        quote: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: GirrQuoteTarget {
                risk_currency: parse_currency(risk_currency)?,
                risk_curve_id: risk_curve_id.into(),
                vertex,
                source: py_to_serde(py, source, "GIRR quote source")?,
                quote: py_to_serde(py, quote, "GIRR quote coordinate")?,
            },
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "GirrQuoteTarget.to_dict")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "GirrQuoteDelta")]
#[pyclass(
    name = "GirrQuoteDelta",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
struct PyGirrQuoteDelta {
    inner: GirrQuoteDelta,
}

#[pymethods]
impl PyGirrQuoteDelta {
    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "GirrQuoteDelta.as_of")]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.as_of)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "GirrQuoteDelta.sensitivities")]
    fn sensitivities(&self) -> PyFrtbSensitivities {
        PyFrtbSensitivities::from_inner(self.inner.sensitivities.clone())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "GirrQuoteDelta.to_dict")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }
}

#[pyfunction]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "girr_quote_delta")]
fn girr_quote_delta(
    py: Python<'_>,
    portfolio: PyRef<'_, PyPortfolio>,
    market: PyRef<'_, PyMarketContext>,
    pricing: &PyFinstackConfig,
    targets: Vec<Py<PyGirrQuoteTarget>>,
) -> PyResult<PyGirrQuoteDelta> {
    let portfolio = &portfolio.inner;
    let market = &market.inner;
    let targets = targets
        .into_iter()
        .map(|target| target.borrow(py).inner.clone())
        .collect::<Vec<_>>();
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = portfolio;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = market;
    let inner = py
        .detach(|| girr_quote_delta_rs(portfolio_ref, market_ref, &pricing.inner, &targets))
        .map_err(portfolio_to_py)?;
    Ok(PyGirrQuoteDelta { inner })
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "CsrQuoteClassification")]
#[pyclass(
    name = "CsrQuoteClassification",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
struct PyCsrQuoteClassification {
    inner: CsrQuoteClassification,
}

#[pymethods]
impl PyCsrQuoteClassification {
    #[new]
    #[pyo3(signature = (*, issuer, currency, seniority, bucket, source_id, as_of))]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "CsrQuoteClassification.__init__")]
    fn new(
        issuer: String,
        currency: &str,
        seniority: &str,
        bucket: u8,
        source_id: String,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: CsrQuoteClassification {
                issuer,
                currency: parse_currency(currency)?,
                seniority: seniority.parse().map_err(crate::errors::core_to_py)?,
                bucket,
                source_id,
                as_of: crate::bindings::date_utils::py_to_date(as_of)?,
            },
        })
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "CsrQuoteSource")]
#[pyclass(
    name = "CsrQuoteSource",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
struct PyCsrQuoteSource {
    inner: CsrQuoteSource,
}

#[pymethods]
impl PyCsrQuoteSource {
    #[new]
    #[pyo3(signature = (*, curve_id, classification, vertices))]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "CsrQuoteSource.__init__")]
    fn new(
        py: Python<'_>,
        curve_id: &str,
        classification: PyRef<'_, PyCsrQuoteClassification>,
        vertices: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: CsrQuoteSource {
                curve_id: curve_id.into(),
                classification: classification.inner.clone(),
                vertices: py_to_serde(py, vertices, "CSR quote coordinates")?,
            },
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "CsrQuoteSource.to_dict")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "CsrQuoteDelta")]
#[pyclass(
    name = "CsrQuoteDelta",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
struct PyCsrQuoteDelta {
    inner: CsrQuoteDelta,
}

#[pymethods]
impl PyCsrQuoteDelta {
    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "CsrQuoteDelta.as_of")]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.as_of)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "CsrQuoteDelta.sensitivities")]
    fn sensitivities(&self) -> PyFrtbSensitivities {
        PyFrtbSensitivities::from_inner(self.inner.sensitivities.clone())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "CsrQuoteDelta.to_dict")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }
}

#[pyfunction]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "csr_quote_delta")]
fn csr_quote_delta(
    py: Python<'_>,
    portfolio: PyRef<'_, PyPortfolio>,
    market: PyRef<'_, PyMarketContext>,
    pricing: &PyFinstackConfig,
    sources: Vec<Py<PyCsrQuoteSource>>,
) -> PyResult<PyCsrQuoteDelta> {
    let portfolio = &portfolio.inner;
    let market = &market.inner;
    let sources = sources
        .into_iter()
        .map(|source| source.borrow(py).inner.clone())
        .collect::<Vec<_>>();
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = portfolio;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = market;
    let inner = py
        .detach(|| csr_quote_delta_rs(portfolio_ref, market_ref, &pricing.inner, &sources))
        .map_err(portfolio_to_py)?;
    Ok(PyCsrQuoteDelta { inner })
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "PortfolioFrtbConfig")]
#[pyclass(
    name = "PortfolioFrtbConfig",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
struct PyPortfolioFrtbConfig {
    inner: finstack_quant_portfolio::margin::PortfolioFrtbConfig,
}

#[pymethods]
impl PyPortfolioFrtbConfig {
    #[new]
    #[pyo3(signature = (*, girr_targets, csr_sources))]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "PortfolioFrtbConfig.__init__")]
    fn new(
        py: Python<'_>,
        girr_targets: Vec<Py<PyGirrQuoteTarget>>,
        csr_sources: Vec<Py<PyCsrQuoteSource>>,
    ) -> Self {
        Self {
            inner: finstack_quant_portfolio::margin::PortfolioFrtbConfig {
                girr_targets: girr_targets
                    .iter()
                    .map(|target| target.borrow(py).inner.clone())
                    .collect(),
                csr_sources: csr_sources
                    .iter()
                    .map(|source| source.borrow(py).inner.clone())
                    .collect(),
            },
        }
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "PortfolioFrtbSensitivitiesResult")]
#[pyclass(
    name = "PortfolioFrtbSensitivitiesResult",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
struct PyPortfolioFrtbSensitivitiesResult {
    inner: finstack_quant_portfolio::margin::PortfolioFrtbSensitivitiesResult,
}

#[pymethods]
impl PyPortfolioFrtbSensitivitiesResult {
    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "PortfolioFrtbSensitivitiesResult.sensitivities")]
    fn sensitivities(&self) -> PyFrtbSensitivities {
        PyFrtbSensitivities::from_inner(self.inner.sensitivities.clone())
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "PortfolioFrtbSensitivitiesResult.girr")]
    fn girr(&self) -> PyGirrQuoteDelta {
        PyGirrQuoteDelta {
            inner: self.inner.girr.clone(),
        }
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "PortfolioFrtbSensitivitiesResult.csr")]
    fn csr(&self) -> PyCsrQuoteDelta {
        PyCsrQuoteDelta {
            inner: self.inner.csr.clone(),
        }
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "PortfolioFrtbSensitivitiesResult.positions_mapped")]
    fn positions_mapped(&self) -> usize {
        self.inner.positions_mapped
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "PortfolioFrtbSensitivitiesResult.positions_unmapped")]
    fn positions_unmapped<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.positions_unmapped)
    }
}

#[pyfunction]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._margin", "portfolio_frtb_sensitivities")]
fn portfolio_frtb_sensitivities(
    py: Python<'_>,
    portfolio: PyRef<'_, PyPortfolio>,
    market: PyRef<'_, PyMarketContext>,
    pricing: &PyFinstackConfig,
    config: &PyPortfolioFrtbConfig,
) -> PyResult<PyPortfolioFrtbSensitivitiesResult> {
    let portfolio = &portfolio.inner;
    let market = &market.inner;
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = portfolio;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = market;
    let inner = py
        .detach(|| {
            finstack_quant_portfolio::margin::portfolio_frtb_sensitivities(
                portfolio_ref,
                market_ref,
                &pricing.inner,
                &config.inner,
            )
        })
        .map_err(portfolio_to_py)?;
    Ok(PyPortfolioFrtbSensitivitiesResult { inner })
}

pub(super) fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPortfolioFrtbConfig>()?;
    m.add_class::<PyPortfolioFrtbSensitivitiesResult>()?;
    m.add_function(wrap_pyfunction!(portfolio_frtb_sensitivities, m)?)?;
    m.add_class::<PyCsrQuoteClassification>()?;
    m.add_class::<PyCsrQuoteSource>()?;
    m.add_class::<PyCsrQuoteDelta>()?;
    m.add_function(wrap_pyfunction!(csr_quote_delta, m)?)?;
    m.add_class::<PyGirrQuoteTarget>()?;
    m.add_class::<PyGirrQuoteDelta>()?;
    m.add_function(wrap_pyfunction!(girr_quote_delta, m)?)?;
    m.add_class::<PyNettingSetMargin>()?;
    m.add_class::<PyPortfolioMarginResult>()?;
    m.add_function(wrap_pyfunction!(portfolio_margin, m)?)?;
    Ok(())
}
