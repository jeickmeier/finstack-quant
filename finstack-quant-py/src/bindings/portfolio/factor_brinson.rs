//! Python bindings for Factor-Brinson unified attribution (Jeet & Partani 2023).
//!
//! Binds `finstack_quant_portfolio::factor_brinson::factor_brinson_attribution`. The typed
//! entry point returns a `Py*` wrapper; the paired `*_json` function keeps
//! the exact JSON wire string. `factor_returns` is a plain numeric list
//! rather than embedded in the JSON payload because it is typically produced
//! by `finstack_quant.analytics.constrained_least_squares`.

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::tabular_methods;
use crate::bindings::table_builders::serde_rows_to_arrow_table;
use crate::errors::{portfolio_to_py, serde_json_to_py};

/// Column schema for [`PyFactorBrinsonResult::to_arrow_table`] (per-factor
/// contributions).
const FACTOR_CONTRIBUTION_COLUMNS: &[(&str, &str)] = &[
    ("factor", "str"),
    ("active_loading", "float64"),
    ("factor_return", "float64"),
    ("contribution", "float64"),
];

/// Column schema for
/// [`PyFactorBrinsonResult::to_asset_contributions_table`].
const ASSET_CONTRIBUTION_COLUMNS: &[(&str, &str)] = &[
    ("asset", "str"),
    ("specific_return", "float64"),
    ("active_weight", "float64"),
    ("contribution", "float64"),
];

/// Factor-Brinson unified attribution result.
///
/// Returned by :func:`factor_brinson_attribution`.
#[pyclass(
    name = "FactorBrinsonResult",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFactorBrinsonResult {
    pub(crate) inner: finstack_quant_portfolio::factor_brinson::FactorBrinsonResult,
}

crate::bindings::macros::wire_methods!(
    PyFactorBrinsonResult,
    finstack_quant_portfolio::factor_brinson::FactorBrinsonResult,
    "FactorBrinsonResult"
);

#[pymethods]
impl PyFactorBrinsonResult {
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

    /// Factor (allocation) contribution.
    #[getter]
    fn allocation(&self) -> f64 {
        self.inner.allocation
    }

    /// Specific (selection) contribution.
    #[getter]
    fn selection(&self) -> f64 {
        self.inner.selection
    }

    /// Per-factor breakdown of ``allocation`` as a list of dicts, in
    /// ``factor_names`` order.
    #[getter]
    fn factor_contributions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.factor_contributions)
    }

    /// Per-asset breakdown of ``selection`` as a list of dicts, in
    /// ``asset_ids`` order.
    #[getter]
    fn asset_contributions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.asset_contributions)
    }

    /// Per-asset specific contributions as an
    /// :class:`~finstack_quant.core.table.ArrowTable`.
    ///
    /// Columns: ``asset``, ``specific_return``, ``active_weight``,
    /// ``contribution``.
    fn to_asset_contributions_table(&self) -> PyResult<PyArrowTable> {
        serde_rows_to_arrow_table(&self.inner.asset_contributions, ASSET_CONTRIBUTION_COLUMNS)
    }

    fn __repr__(&self) -> String {
        format!(
            "FactorBrinsonResult(factors={}, assets={}, active_return={})",
            self.inner.factor_contributions.len(),
            self.inner.asset_contributions.len(),
            self.inner.active_return,
        )
    }
}

impl PyFactorBrinsonResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        serde_rows_to_arrow_table(
            &self.inner.factor_contributions,
            FACTOR_CONTRIBUTION_COLUMNS,
        )
    }
}

tabular_methods!(PyFactorBrinsonResult);

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._factor_brinson", "factor_brinson_attribution")]
#[pyfunction]
#[pyo3(text_signature = "(input, factor_returns)")]
fn factor_brinson_attribution(
    py: Python<'_>,
    input: &Bound<'_, PyAny>,
    factor_returns: &Bound<'_, PyAny>,
) -> PyResult<PyFactorBrinsonResult> {
    let input = crate::bindings::extract::extract_records_json(py, input, "input")?;
    let factor_returns =
        crate::bindings::dataframe_utils::extract_f64_vec(factor_returns, "factor_returns")?;
    Ok(PyFactorBrinsonResult {
        inner: py.detach(move || {
            let input: finstack_quant_portfolio::factor_brinson::FactorBrinsonInput =
                serde_json::from_str(&input)
                    .map_err(|err| serde_json_to_py(err, "invalid factor-Brinson input JSON"))?;
            finstack_quant_portfolio::factor_brinson::factor_brinson_attribution(
                &input,
                &factor_returns,
            )
            .map_err(portfolio_to_py)
        })?,
    })
}
/// Register factor-Brinson attribution functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFactorBrinsonResult>()?;
    m.add_function(wrap_pyfunction!(factor_brinson_attribution, m)?)?;
    Ok(())
}
