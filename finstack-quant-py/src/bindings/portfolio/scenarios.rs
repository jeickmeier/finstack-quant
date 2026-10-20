//! Python bindings for scenario application and scenario P&L.

use crate::bindings::extract::extract_market_ref;
use crate::bindings::extract::extract_portfolio_ref;
use crate::bindings::extract::extract_scenario_spec;
use crate::bindings::extract::extract_scenario_specs;
use crate::bindings::pandas_utils::labeled_values_to_series;
use crate::bindings::pandas_utils::serde_rows_to_dataframe_with_schema;
use crate::bindings::pandas_utils::ColumnSchema;
use crate::bindings::portfolio::valuation::PyPortfolioValuation;
use crate::bindings::scenarios::engine::PyApplicationReport;
use crate::errors::portfolio_to_py;
use pyo3::prelude::*;
use pyo3::types::PyAny;
use pyo3::types::PyDict;
use pyo3::types::PyModule;
use serde::Serialize;

/// Apply a scenario to a portfolio and revalue it.
///
/// Parameters
/// ----------
/// portfolio : Portfolio | str
///     Built portfolio or canonical ``PortfolioSpec`` JSON.
/// scenario : ScenarioSpec | str
///     Typed :class:`~finstack_quant.scenarios.ScenarioSpec` or its canonical
///     JSON string.
/// market : MarketContext | str
///     Base market to shock before revaluation.
///
/// Returns
/// -------
/// tuple[PortfolioValuation, ApplicationReport]
///     The revalued portfolio and the scenario application report. Call
///     ``.to_json()`` on either for its wire form.
#[pyfunction]
#[pyo3(text_signature = "(portfolio, scenario, market)")]
fn apply_scenario_and_revalue(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    scenario: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
) -> PyResult<(PyPortfolioValuation, PyApplicationReport)> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let scenario = extract_scenario_spec(py, scenario)?;
    let market = extract_market_ref(py, market)?;
    let config = finstack_quant_core::config::FinstackConfig::default();
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
    let finstack_quant_portfolio::scenarios::ScenarioRevalueView { valuation, report } = py
        .detach(|| {
            finstack_quant_portfolio::scenarios::apply_and_revalue(
                portfolio_ref,
                &scenario,
                market_ref,
                &config,
            )
        })
        .map_err(portfolio_to_py)?;
    Ok((
        PyPortfolioValuation::from_inner(valuation),
        PyApplicationReport { inner: report },
    ))
}

/// Compute the profit and loss attributable to a scenario.
///
/// Parameters
/// ----------
/// portfolio : Portfolio | str
///     Built portfolio or canonical ``PortfolioSpec`` JSON.
/// scenario : ScenarioSpec | str
///     Typed :class:`~finstack_quant.scenarios.ScenarioSpec` or its canonical
///     JSON string.
/// market : MarketContext | str
///     Unshocked base market.
///
/// Returns
/// -------
/// tuple[ScenarioPnl, ApplicationReport]
///     The P&L ladder (``total`` plus ``by_position``, all base-currency
///     floats; ``ScenarioPnl.currency`` names the currency) and the scenario
///     application report. ``ScenarioPnl`` offers ``to_dataframe()`` and
///     ``to_series()``; call ``.to_json()`` on either for its wire form.
#[pyfunction]
#[pyo3(text_signature = "(portfolio, scenario, market)")]
fn scenario_pnl(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    scenario: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
) -> PyResult<(PyScenarioPnl, PyApplicationReport)> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let scenario = extract_scenario_spec(py, scenario)?;
    let market = extract_market_ref(py, market)?;
    let config = finstack_quant_core::config::FinstackConfig::default();
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
    let finstack_quant_portfolio::scenarios::ScenarioPnlView { pnl, report } = py
        .detach(|| {
            finstack_quant_portfolio::scenarios::scenario_pnl(
                portfolio_ref,
                &scenario,
                market_ref,
                &config,
            )
        })
        .map_err(portfolio_to_py)?;
    Ok((
        PyScenarioPnl { inner: pnl },
        PyApplicationReport { inner: report },
    ))
}

/// Compute ordered portfolio P&L for a batch of scenarios.
///
/// Parameters
/// ----------
/// portfolio : Portfolio | str
///     A built :class:`Portfolio` or canonical JSON-serialized
///     ``PortfolioSpec``. The Rust batch engine values its unstressed base leg
///     once for the complete request.
/// scenarios : list[ScenarioSpec | str] | str
///     Ordered scenarios: a Python sequence whose items are typed
///     ``ScenarioSpec`` objects or canonical JSON strings, or one canonical
///     JSON array string. Output order matches input order. An empty batch
///     returns ``[]`` without a valuation.
/// market : MarketContext | str
///     The unshocked market snapshot, supplied as a typed object or canonical
///     JSON string.
///
/// Returns
/// -------
/// list[ScenarioPnlBatchItem]
///     One ordered item per input scenario, each carrying ``scenario_id``,
///     a typed ``pnl`` (:class:`ScenarioPnl`) and ``report``
///     (:class:`~finstack_quant.scenarios.ApplicationReport`); each item's
///     ``to_json()`` gives its wire form.
///
/// Raises
/// ------
/// ValueError
///     If ``scenarios`` contains JSON that is not a valid ``ScenarioSpec``.
/// PortfolioError
///     If scenario application, valuation, or base-currency P&L differencing
///     fails. The error identifies the earliest failing input scenario.
#[pyfunction]
#[pyo3(text_signature = "(portfolio, scenarios, market)")]
fn scenario_pnl_batch(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    scenarios: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
) -> PyResult<Vec<crate::bindings::portfolio::scenarios::PyScenarioPnlBatchItem>> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let scenarios = extract_scenario_specs(py, scenarios)?;
    let market = extract_market_ref(py, market)?;
    let config = finstack_quant_core::config::FinstackConfig::default();

    // Extract plain Rust references before detaching: the typed access
    // wrappers own PyRefs and are not Ungil, while their Rust inners are
    // Send + Sync. Parsing happens above, so the complete batch evaluation
    // releases the GIL without touching Python state.
    let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
    let results = py
        .detach(|| {
            finstack_quant_portfolio::scenarios::scenario_pnl_batch(
                portfolio_ref,
                &scenarios,
                market_ref,
                &config,
            )
        })
        .map_err(portfolio_to_py)?;
    Ok(results
        .into_iter()
        .map(|inner| crate::bindings::portfolio::scenarios::PyScenarioPnlBatchItem { inner })
        .collect())
}

/// Column schema for [`PyScenarioPnl::to_dataframe`].
///
/// Pinned so an empty ladder keeps the same dtypes as a populated one.
const PNL_ROW_COLUMNS: &[ColumnSchema<'static>] = &[("position_id", "str"), ("pnl", "f64")];

#[derive(Serialize)]
struct PnlRow<'a> {
    position_id: &'a str,
    pnl: f64,
}

/// Scenario-attributable profit and loss, in the portfolio base currency.
///
/// Returned by :func:`scenario_pnl` alongside the scenario
/// :class:`ApplicationReport`.
#[pyclass(
    name = "ScenarioPnl",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyScenarioPnl {
    pub(crate) inner: finstack_quant_portfolio::scenarios::ScenarioPnl,
}

crate::bindings::macros::wire_methods!(
    PyScenarioPnl,
    finstack_quant_portfolio::scenarios::ScenarioPnl,
    "ScenarioPnl"
);

impl PyScenarioPnl {
    fn rows(&self) -> Vec<PnlRow<'_>> {
        self.inner
            .by_position
            .iter()
            .map(|(id, money)| PnlRow {
                position_id: id.as_str(),
                pnl: money.amount(),
            })
            .collect()
    }
}

#[pymethods]
impl PyScenarioPnl {
    /// Total scenario P&L in the portfolio base currency.
    #[getter]
    fn total(&self) -> f64 {
        self.inner.total.amount()
    }

    /// Base currency the P&L is denominated in.
    #[getter]
    fn currency(&self) -> String {
        self.inner.total.currency().to_string()
    }

    /// Per-position scenario P&L, keyed by position id.
    ///
    /// The dict preserves the canonical Rust ``IndexMap`` iteration order
    /// (stressed positions first, then base-only positions), matching
    /// :meth:`to_dataframe`, :meth:`to_series`, and :meth:`to_json`.
    #[getter]
    fn by_position<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        for (id, money) in &self.inner.by_position {
            out.set_item(id.as_str(), money.amount())?;
        }
        Ok(out)
    }

    /// Per-position P&L ladder as a :class:`pandas.DataFrame`.
    ///
    /// Columns: ``position_id``, ``pnl``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(py, &self.rows(), PNL_ROW_COLUMNS)
    }

    /// Per-position P&L as a :class:`pandas.Series` indexed by position id.
    fn to_series<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let labels: Vec<String> = self
            .inner
            .by_position
            .keys()
            .map(ToString::to_string)
            .collect();
        let values: Vec<f64> = self
            .inner
            .by_position
            .values()
            .map(finstack_quant_core::money::Money::amount)
            .collect();
        labeled_values_to_series(py, &labels, values, "pnl")
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ScenarioPnl", &self.inner)
    }
}

/// Column schema for [`PyScenarioPnlBatchItem::to_dataframe`].
///
/// The per-position ladder columns, prefixed with the owning scenario id so
/// frames from a batch can be concatenated directly.
const BATCH_PNL_ROW_COLUMNS: &[ColumnSchema<'static>] = &[
    ("scenario_id", "str"),
    ("position_id", "str"),
    ("pnl", "float64"),
];

#[derive(Serialize)]
struct BatchPnlRow<'a> {
    scenario_id: &'a str,
    position_id: &'a str,
    pnl: f64,
}

/// One ordered result from :func:`scenario_pnl_batch`.
///
/// Carries the scenario identifier, its typed :class:`ScenarioPnl` ladder,
/// and the scenario :class:`~finstack_quant.scenarios.ApplicationReport`.
#[pyclass(
    name = "ScenarioPnlBatchItem",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyScenarioPnlBatchItem {
    pub(crate) inner: finstack_quant_portfolio::scenarios::ScenarioPnlBatchItem,
}

crate::bindings::macros::wire_methods!(
    PyScenarioPnlBatchItem,
    finstack_quant_portfolio::scenarios::ScenarioPnlBatchItem,
    "ScenarioPnlBatchItem"
);

impl PyScenarioPnlBatchItem {
    fn rows(&self) -> Vec<BatchPnlRow<'_>> {
        self.inner
            .pnl
            .by_position
            .iter()
            .map(|(id, money)| BatchPnlRow {
                scenario_id: &self.inner.scenario_id,
                position_id: id.as_str(),
                pnl: money.amount(),
            })
            .collect()
    }
}

#[pymethods]
impl PyScenarioPnlBatchItem {
    /// Identifier copied from the input scenario.
    #[getter]
    fn scenario_id(&self) -> String {
        self.inner.scenario_id.clone()
    }

    /// Scenario-attributable portfolio P&L ladder.
    #[getter]
    fn pnl(&self) -> PyScenarioPnl {
        PyScenarioPnl {
            inner: self.inner.pnl.clone(),
        }
    }

    /// Application provenance and warnings for this scenario.
    #[getter]
    fn report(&self) -> crate::bindings::scenarios::engine::PyApplicationReport {
        crate::bindings::scenarios::engine::PyApplicationReport {
            inner: self.inner.report.clone(),
        }
    }

    /// Per-position P&L ladder as a :class:`pandas.DataFrame`.
    ///
    /// Columns: ``scenario_id``, ``position_id``, ``pnl``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(py, &self.rows(), BATCH_PNL_ROW_COLUMNS)
    }

    fn __repr__(&self) -> String {
        format!(
            "ScenarioPnlBatchItem(scenario_id={:?}, positions={})",
            self.inner.scenario_id,
            self.inner.pnl.by_position.len(),
        )
    }
}

/// Register the scenarios bindings on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyScenarioPnl>()?;
    m.add_class::<PyScenarioPnlBatchItem>()?;
    m.add_function(pyo3::wrap_pyfunction!(apply_scenario_and_revalue, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(scenario_pnl, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(scenario_pnl_batch, m)?)?;
    Ok(())
}
