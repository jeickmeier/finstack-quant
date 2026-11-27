//! Python bindings for Brinson-Fachler attribution.
//!
//! The entry points (`brinson_fachler`, `carino_link`, `brinson_linked`) return `Py*` wrappers
//! over the canonical Rust results; each wrapper's `to_json()` gives the exact
//! JSON wire string for pipelines that exchange documents.

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::tabular_methods;
use crate::bindings::table_builders::serde_rows_to_arrow_table;
use crate::errors::{portfolio_to_py, serde_json_to_py};

/// Column schema shared by [`PyBrinsonPeriodResult::to_arrow_table`] and
/// [`PyCarinoLinkedAttribution::to_arrow_table`] (both tables are sector-effect
/// tables).
const SECTOR_EFFECT_COLUMNS: &[(&str, &str)] = &[
    ("sector", "str"),
    ("allocation", "float64"),
    ("selection", "float64"),
    ("interaction", "float64"),
    ("total", "float64"),
];

/// Single-period Brinson-Fachler attribution result.
///
/// Returned by :func:`brinson_fachler`. Sector effects preserve input order.
#[pyclass(
    name = "BrinsonPeriodResult",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyBrinsonPeriodResult {
    pub(crate) inner: finstack_quant_portfolio::brinson::BrinsonPeriodResult,
}

crate::bindings::macros::wire_methods!(
    PyBrinsonPeriodResult,
    finstack_quant_portfolio::brinson::BrinsonPeriodResult,
    "BrinsonPeriodResult"
);

#[pymethods]
impl PyBrinsonPeriodResult {
    /// Per-sector effects as a list of dicts, in the order supplied.
    #[getter]
    fn sectors<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.sectors)
    }

    /// Sum of allocation effects across sectors.
    #[getter]
    fn total_allocation(&self) -> f64 {
        self.inner.total_allocation
    }

    /// Sum of selection effects across sectors.
    #[getter]
    fn total_selection(&self) -> f64 {
        self.inner.total_selection
    }

    /// Sum of interaction effects across sectors.
    #[getter]
    fn total_interaction(&self) -> f64 {
        self.inner.total_interaction
    }

    /// Portfolio total return for the period.
    #[getter]
    fn portfolio_return(&self) -> f64 {
        self.inner.portfolio_return
    }

    /// Benchmark total return for the period.
    #[getter]
    fn benchmark_return(&self) -> f64 {
        self.inner.benchmark_return
    }

    /// Active return; equals the sum of the three effect totals.
    #[getter]
    fn total_excess_return(&self) -> f64 {
        self.inner.total_excess_return
    }

    fn __repr__(&self) -> String {
        format!(
            "BrinsonPeriodResult(sectors={}, total_excess_return={})",
            self.inner.sectors.len(),
            self.inner.total_excess_return,
        )
    }
}

impl PyBrinsonPeriodResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        serde_rows_to_arrow_table(&self.inner.sectors, SECTOR_EFFECT_COLUMNS)
    }
}

tabular_methods!(PyBrinsonPeriodResult);

/// Carino-linked multi-period Brinson attribution result.
///
/// Returned by :func:`carino_link`.
#[pyclass(
    name = "CarinoLinkedAttribution",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCarinoLinkedAttribution {
    pub(crate) inner: finstack_quant_portfolio::brinson::CarinoLinkedAttribution,
}

crate::bindings::macros::wire_methods!(
    PyCarinoLinkedAttribution,
    finstack_quant_portfolio::brinson::CarinoLinkedAttribution,
    "CarinoLinkedAttribution"
);

#[pymethods]
impl PyCarinoLinkedAttribution {
    /// Per-period decompositions as a list of dicts, in chronological order.
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

    /// Per-sector Carino-smoothed effects summed across periods.
    #[getter]
    fn linked_sectors<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.linked_sectors)
    }

    /// Sum of per-sector linked allocation effects.
    #[getter]
    fn linked_allocation(&self) -> f64 {
        self.inner.linked_allocation
    }

    /// Sum of per-sector linked selection effects.
    #[getter]
    fn linked_selection(&self) -> f64 {
        self.inner.linked_selection
    }

    /// Sum of per-sector linked interaction effects.
    #[getter]
    fn linked_interaction(&self) -> f64 {
        self.inner.linked_interaction
    }

    fn __repr__(&self) -> String {
        format!(
            "CarinoLinkedAttribution(periods={}, sectors={})",
            self.inner.periods.len(),
            self.inner.linked_sectors.len(),
        )
    }
}

impl PyCarinoLinkedAttribution {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        serde_rows_to_arrow_table(&self.inner.linked_sectors, SECTOR_EFFECT_COLUMNS)
    }
}

tabular_methods!(PyCarinoLinkedAttribution);

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._brinson", "brinson_fachler")]
#[pyfunction]
#[pyo3(text_signature = "(sectors)")]
fn brinson_fachler(py: Python<'_>, sectors: &Bound<'_, PyAny>) -> PyResult<PyBrinsonPeriodResult> {
    let sectors = crate::bindings::extract::extract_records_json(py, sectors, "sectors")?;
    Ok(PyBrinsonPeriodResult {
        inner: py.detach(move || {
            let sectors: Vec<finstack_quant_portfolio::brinson::SectorPeriod> =
                serde_json::from_str(&sectors)
                    .map_err(|err| serde_json_to_py(err, "invalid Brinson sectors JSON"))?;
            finstack_quant_portfolio::brinson::brinson_fachler(&sectors).map_err(portfolio_to_py)
        })?,
    })
}
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._brinson", "carino_link")]
#[pyfunction]
#[pyo3(text_signature = "(periods)")]
fn carino_link(py: Python<'_>, periods: &Bound<'_, PyAny>) -> PyResult<PyCarinoLinkedAttribution> {
    if let Ok(typed) = periods.extract::<Vec<PyRef<'_, PyBrinsonPeriodResult>>>() {
        let items: Vec<_> = typed.into_iter().map(|item| item.inner.clone()).collect();
        return Ok(PyCarinoLinkedAttribution {
            inner: py
                .detach(move || finstack_quant_portfolio::brinson::carino_link(&items))
                .map_err(portfolio_to_py)?,
        });
    }
    let periods = crate::bindings::extract::extract_records_json(py, periods, "periods")?;
    Ok(PyCarinoLinkedAttribution {
        inner: py.detach(move || {
            let periods: Vec<finstack_quant_portfolio::brinson::BrinsonPeriodResult> =
                serde_json::from_str(&periods)
                    .map_err(|err| serde_json_to_py(err, "invalid Brinson period results JSON"))?;
            finstack_quant_portfolio::brinson::carino_link(&periods).map_err(portfolio_to_py)
        })?,
    })
}
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._brinson", "brinson_linked")]
#[pyfunction]
#[pyo3(text_signature = "(periods)")]
fn brinson_linked(
    py: Python<'_>,
    periods: &Bound<'_, PyAny>,
) -> PyResult<PyCarinoLinkedAttribution> {
    let grouped_json = if let Some(records_py) =
        crate::bindings::dataframe_utils::table_to_py_records(py, periods)?
    {
        let records: Vec<Bound<'_, pyo3::types::PyDict>> = records_py.extract()?;
        let mut order: Vec<String> = Vec::new();
        let mut groups: std::collections::HashMap<String, Vec<Bound<'_, pyo3::types::PyDict>>> =
            std::collections::HashMap::new();
        for row in records {
            let copy = row.copy()?;
            let period_key = copy
                .call_method1("pop", ("period",))
                .map_err(|_| {
                    crate::errors::value_error(
                        "brinson_linked expects a 'period' column when given a single DataFrame",
                    )
                })?
                .str()?
                .to_string();
            groups
                .entry(period_key.clone())
                .or_insert_with(|| {
                    order.push(period_key);
                    Vec::new()
                })
                .push(copy);
        }
        let grouped: Vec<Vec<Bound<'_, pyo3::types::PyDict>>> = order
            .into_iter()
            .filter_map(|key| groups.remove(&key))
            .collect();
        let list = pyo3::types::PyList::new(py, grouped)?;
        crate::bindings::module_utils::py_to_json_string(py, &list, "periods")?
    } else {
        crate::bindings::extract::extract_records_json(py, periods, "periods")?
    };
    Ok(PyCarinoLinkedAttribution {
        inner: py.detach(move || {
            let periods: Vec<Vec<finstack_quant_portfolio::brinson::SectorPeriod>> =
                serde_json::from_str(&grouped_json)
                    .map_err(|err| serde_json_to_py(err, "invalid Brinson sector periods JSON"))?;
            finstack_quant_portfolio::brinson::brinson_linked(&periods).map_err(portfolio_to_py)
        })?,
    })
}
/// Register Brinson attribution functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBrinsonPeriodResult>()?;
    m.add_class::<PyCarinoLinkedAttribution>()?;
    m.add_function(wrap_pyfunction!(brinson_fachler, m)?)?;
    m.add_function(wrap_pyfunction!(carino_link, m)?)?;
    m.add_function(wrap_pyfunction!(brinson_linked, m)?)?;
    Ok(())
}
