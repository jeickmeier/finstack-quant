//! Python bindings for Campisi fixed-income benchmark attribution.
//!
//! The entry points return `Py*` wrappers over the canonical Rust results;
//! each wrapper's `to_json()` gives the exact JSON wire string (same pattern as
//! the Brinson bindings in [`super::brinson`]).

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::tabular_methods;
use crate::bindings::portfolio::attribution::PyReconciliationReport;
use crate::bindings::table_builders::serde_rows_to_arrow_table;
use crate::errors::portfolio_to_py;

/// Column schema for [`PyFiAttributionResult::to_arrow_table`] (per-sector
/// effects).
const FI_SECTOR_COLUMNS: &[(&str, &str)] = &[
    ("sector", "str"),
    ("portfolio_weight", "float64"),
    ("benchmark_weight", "float64"),
    ("portfolio_return", "float64"),
    ("benchmark_return", "float64"),
    ("benchmark_comparator", "float64"),
    ("allocation", "float64"),
    ("active_carry", "float64"),
    ("active_treasury", "float64"),
    ("active_spread", "float64"),
    ("residual", "float64"),
    ("total_active", "float64"),
];

/// Column schema for [`PyFiCarinoLinkedResult::to_arrow_table`] (linked
/// per-sector effects).
const FI_LINKED_SECTOR_COLUMNS: &[(&str, &str)] = &[
    ("sector", "str"),
    ("allocation", "float64"),
    ("active_carry", "float64"),
    ("active_treasury", "float64"),
    ("active_spread", "float64"),
    ("residual", "float64"),
    ("total_active", "float64"),
];

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._fi_attribution", "FiAttributionResult")]
#[pyclass(
    name = "FiAttributionResult",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFiAttributionResult {
    pub(crate) inner: finstack_quant_portfolio::fi_attribution::FiAttributionResult,
}

crate::bindings::macros::wire_methods!(
    PyFiAttributionResult,
    finstack_quant_portfolio::fi_attribution::FiAttributionResult,
    "FiAttributionResult"
);

#[pymethods]
impl PyFiAttributionResult {
    /// Per-sector effects as a list of dicts, in declared sector order.
    #[getter]
    fn sectors<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.sectors)
    }

    /// Portfolio-side absolute Campisi split (``carry``, ``treasury``,
    /// ``spread``, ``residual``, ``total``).
    #[getter]
    fn portfolio_components<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.portfolio_components)
    }

    /// Benchmark-side absolute Campisi split.
    #[getter]
    fn benchmark_components<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.benchmark_components)
    }

    /// Portfolio total return.
    #[getter]
    fn portfolio_return(&self) -> f64 {
        self.inner.portfolio_return
    }

    /// Benchmark total return.
    #[getter]
    fn benchmark_return(&self) -> f64 {
        self.inner.benchmark_return
    }

    /// Active return, ``portfolio_return - benchmark_return``.
    #[getter]
    fn active_return(&self) -> f64 {
        self.inner.active_return
    }

    /// Sum of sector allocation effects.
    #[getter]
    fn total_allocation(&self) -> f64 {
        self.inner.total_allocation
    }

    /// Sum of sector active carry effects.
    #[getter]
    fn total_active_carry(&self) -> f64 {
        self.inner.total_active_carry
    }

    /// Sum of sector active treasury effects.
    #[getter]
    fn total_active_treasury(&self) -> f64 {
        self.inner.total_active_treasury
    }

    /// Sum of sector active spread effects.
    #[getter]
    fn total_active_spread(&self) -> f64 {
        self.inner.total_active_spread
    }

    /// Sum of sector residual effects.
    #[getter]
    fn total_residual(&self) -> f64 {
        self.inner.total_residual
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._fi_attribution", "FiAttributionResult.reconciliation_check")]
    #[pyo3(text_signature = "(self, tolerance)")]
    fn reconciliation_check(&self, tolerance: f64) -> PyResult<PyReconciliationReport> {
        self.inner
            .reconciliation_check(tolerance)
            .map(|inner| PyReconciliationReport { inner })
            .map_err(portfolio_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "FiAttributionResult(sectors={}, active_return={})",
            self.inner.sectors.len(),
            self.inner.active_return,
        )
    }
}

impl PyFiAttributionResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        serde_rows_to_arrow_table(&self.inner.sectors, FI_SECTOR_COLUMNS)
    }
}

tabular_methods!(PyFiAttributionResult);

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._fi_attribution", "FiCarinoLinkedResult")]
#[pyclass(
    name = "FiCarinoLinkedResult",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFiCarinoLinkedResult {
    pub(crate) inner: finstack_quant_portfolio::fi_attribution::FiCarinoLinkedResult,
}

crate::bindings::macros::wire_methods!(
    PyFiCarinoLinkedResult,
    finstack_quant_portfolio::fi_attribution::FiCarinoLinkedResult,
    "FiCarinoLinkedResult"
);

#[pymethods]
impl PyFiCarinoLinkedResult {
    /// Per-period single-period results as a list of dicts, in chronological
    /// order.
    #[getter]
    fn periods<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.periods)
    }

    /// Geometrically compounded portfolio return.
    #[getter]
    fn portfolio_return_compounded(&self) -> f64 {
        self.inner.portfolio_return_compounded
    }

    /// Geometrically compounded benchmark return.
    #[getter]
    fn benchmark_return_compounded(&self) -> f64 {
        self.inner.benchmark_return_compounded
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._fi_attribution", "FiCarinoLinkedResult.active_return_compounded")]
    fn active_return_compounded(&self) -> f64 {
        self.inner.active_return_compounded
    }

    /// Per-sector linked effects as a list of dicts.
    #[getter]
    fn linked_sectors<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.linked_sectors)
    }

    /// Sum of linked allocation effects.
    #[getter]
    fn linked_allocation(&self) -> f64 {
        self.inner.linked_allocation
    }

    /// Sum of linked active carry effects.
    #[getter]
    fn linked_active_carry(&self) -> f64 {
        self.inner.linked_active_carry
    }

    /// Sum of linked active treasury effects.
    #[getter]
    fn linked_active_treasury(&self) -> f64 {
        self.inner.linked_active_treasury
    }

    /// Sum of linked active spread effects.
    #[getter]
    fn linked_active_spread(&self) -> f64 {
        self.inner.linked_active_spread
    }

    /// Sum of linked residual effects.
    #[getter]
    fn linked_residual(&self) -> f64 {
        self.inner.linked_residual
    }

    fn __repr__(&self) -> String {
        format!(
            "FiCarinoLinkedResult(periods={}, sectors={})",
            self.inner.periods.len(),
            self.inner.linked_sectors.len(),
        )
    }
}

impl PyFiCarinoLinkedResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        serde_rows_to_arrow_table(&self.inner.linked_sectors, FI_LINKED_SECTOR_COLUMNS)
    }
}

tabular_methods!(PyFiCarinoLinkedResult);

use finstack_quant_portfolio::fi_attribution::{
    FiAttributionConfig, FiPositionSnapshot, FiQuote, FiQuoteLeg, FiReportingBasis,
};

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._fi_attribution", "FiQuote")]
#[pyclass(
    name = "FiQuote",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
struct PyFiQuote {
    inner: FiQuote,
}

#[pymethods]
impl PyFiQuote {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._fi_attribution", "FiQuote.__init__")]
    #[new]
    fn new(basis: String, rate: f64) -> Self {
        Self {
            inner: FiQuote { basis, rate },
        }
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._fi_attribution", "FiQuoteLeg")]
#[pyclass(
    name = "FiQuoteLeg",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
struct PyFiQuoteLeg {
    inner: FiQuoteLeg,
}

#[pymethods]
impl PyFiQuoteLeg {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._fi_attribution", "FiQuoteLeg.__init__")]
    #[new]
    fn new(
        sensitivity_basis: String,
        duration: f64,
        opening: &PyFiQuote,
        closing: &PyFiQuote,
    ) -> Self {
        Self {
            inner: FiQuoteLeg {
                sensitivity_basis,
                duration,
                opening: opening.inner.clone(),
                closing: closing.inner.clone(),
            },
        }
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._fi_attribution", "FiReportingBasis")]
#[pyclass(
    name = "FiReportingBasis",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
struct PyFiReportingBasis {
    inner: FiReportingBasis,
}

#[pymethods]
impl PyFiReportingBasis {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._fi_attribution", "FiReportingBasis.__init__")]
    #[new]
    fn new(
        portfolio_series_id: String,
        benchmark_series_id: String,
        return_policy: String,
        valuation: String,
        fx: String,
        fees: String,
    ) -> Self {
        Self {
            inner: FiReportingBasis {
                portfolio_series_id,
                benchmark_series_id,
                return_policy,
                valuation,
                fx,
                fees,
            },
        }
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._fi_attribution", "FiAttributionConfig")]
#[pyclass(
    name = "FiAttributionConfig",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
struct PyFiAttributionConfig {
    inner: FiAttributionConfig,
}

#[pymethods]
impl PyFiAttributionConfig {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._fi_attribution", "FiAttributionConfig.__init__")]
    #[new]
    fn new(
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
        currency: &Bound<'_, PyAny>,
        basis: &PyFiReportingBasis,
        sectors: Vec<String>,
    ) -> PyResult<Self> {
        let start = crate::bindings::date_utils::py_to_date(start)?;
        let end = crate::bindings::date_utils::py_to_date(end)?;
        let currency = crate::bindings::core::currency::extract_currency(currency)?;
        Ok(Self {
            inner: FiAttributionConfig {
                start,
                end,
                currency,
                basis: basis.inner.clone(),
                sectors,
            },
        })
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._fi_attribution", "FiPositionSnapshot")]
#[pyclass(
    name = "FiPositionSnapshot",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
struct PyFiPositionSnapshot {
    inner: FiPositionSnapshot,
}

#[pymethods]
impl PyFiPositionSnapshot {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._fi_attribution", "FiPositionSnapshot.__init__")]
    #[new]
    fn new(
        id: String,
        sector: String,
        weight: &Bound<'_, PyAny>,
        total_return: f64,
        opening_market_value: &crate::bindings::core::money::PyMoney,
        annual_coupon: &crate::bindings::core::money::PyMoney,
    ) -> PyResult<Self> {
        let weight = crate::bindings::core::money::extract_exact_decimal(
            weight,
            "weight must be int, decimal string or decimal.Decimal",
        )?;
        Ok(Self {
            inner: FiPositionSnapshot {
                id,
                sector,
                weight: finstack_quant_core::wire::DecimalWire(weight),
                total_return,
                opening_market_value: opening_market_value.inner,
                annual_coupon: annual_coupon.inner,
                treasury: None,
                spread: None,
            },
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._fi_attribution", "FiPositionSnapshot.with_quote_legs")]
    #[pyo3(signature = (*, treasury=None, spread=None))]
    fn with_quote_legs(
        &self,
        treasury: Option<&PyFiQuoteLeg>,
        spread: Option<&PyFiQuoteLeg>,
    ) -> Self {
        Self {
            inner: FiPositionSnapshot {
                treasury: treasury.map(|leg| leg.inner.clone()),
                spread: spread.map(|leg| leg.inner.clone()),
                ..self.inner.clone()
            },
        }
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._fi_attribution", "campisi_attribution")]
#[pyfunction]
fn campisi_attribution(
    py: Python<'_>,
    portfolio: Vec<Py<PyFiPositionSnapshot>>,
    benchmark: Vec<Py<PyFiPositionSnapshot>>,
    config: &PyFiAttributionConfig,
) -> PyResult<PyFiAttributionResult> {
    let snapshots = |side: Vec<Py<PyFiPositionSnapshot>>| {
        side.into_iter()
            .map(|position| position.get().inner.clone())
            .collect::<Vec<_>>()
    };
    let portfolio = snapshots(portfolio);
    let benchmark = snapshots(benchmark);
    let config = config.inner.clone();
    py.detach(move || {
        finstack_quant_portfolio::fi_attribution::campisi_attribution(
            &portfolio, &benchmark, &config,
        )
        .map(|inner| PyFiAttributionResult { inner })
        .map_err(portfolio_to_py)
    })
}

#[pyfunction]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._fi_attribution", "campisi_carino_link")]
fn campisi_carino_link(
    py: Python<'_>,
    periods: Vec<Py<PyFiAttributionResult>>,
) -> PyResult<PyFiCarinoLinkedResult> {
    let periods = periods
        .into_iter()
        .map(|period| period.borrow(py).inner.clone())
        .collect::<Vec<_>>();
    let inner = py
        .detach(|| finstack_quant_portfolio::fi_attribution::campisi_carino_link(&periods))
        .map_err(portfolio_to_py)?;
    Ok(PyFiCarinoLinkedResult { inner })
}
/// Register Campisi attribution functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFiAttributionResult>()?;
    m.add_class::<PyFiAttributionConfig>()?;
    m.add_class::<PyFiPositionSnapshot>()?;
    m.add_class::<PyFiQuote>()?;
    m.add_class::<PyFiQuoteLeg>()?;
    m.add_class::<PyFiReportingBasis>()?;
    m.add_class::<PyFiCarinoLinkedResult>()?;
    m.add_function(wrap_pyfunction!(campisi_attribution, m)?)?;
    m.add_function(wrap_pyfunction!(campisi_carino_link, m)?)?;
    Ok(())
}
