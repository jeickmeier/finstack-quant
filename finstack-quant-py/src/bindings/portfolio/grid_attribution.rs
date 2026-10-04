//! Python bindings for hierarchical duration-cell x sector grid attribution.
//!
//! Binds `finstack_quant_portfolio::grid_attribution` (Dynkin, Hyman &
//! Vankudre 1998, Appendix A). The typed entry points return `Py*` wrappers;
//! Result wrappers serialize through the canonical Rust wire format.

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
    serde_to_py, ColumnSchema,
};
use crate::errors::{display_to_py, portfolio_to_py, serde_json_to_py};

/// Column schema for [`PyGridAttributionResult::to_dataframe`] (per-cell
/// curve effects).
const GRID_CURVE_COLUMNS: &[ColumnSchema<'static>] = &[
    ("cell", "str"),
    ("portfolio_weight", "float64"),
    ("benchmark_weight", "float64"),
    ("benchmark_cell_return", "float64"),
    ("curve_effect", "float64"),
];

/// Column schema for [`PyGridAttributionResult::to_sector_effects_dataframe`].
const GRID_SECTOR_COLUMNS: &[ColumnSchema<'static>] = &[
    ("cell", "str"),
    ("sector", "str"),
    ("allocation_effect", "float64"),
    ("selection_effect", "float64"),
];

/// Single-period hierarchical grid attribution result.
///
/// Returned by :func:`grid_attribution`.
#[pyclass(
    name = "GridAttributionResult",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyGridAttributionResult {
    pub(crate) inner: finstack_quant_portfolio::GridAttributionResult,
}

#[pymethods]
impl PyGridAttributionResult {
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

    /// Per-cell curve effects as a list of dicts, in first-appearance order.
    #[getter]
    fn curve_effects<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.curve_effects)
    }

    /// Per-(cell, sector) allocation and selection effects as a list of dicts.
    #[getter]
    fn sector_effects<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.sector_effects)
    }
    /// Sum of the curve effects.
    #[getter]
    fn total_curve(&self) -> f64 {
        self.inner.total_curve
    }

    /// Sum of the sector allocation effects.
    #[getter]
    fn total_sector(&self) -> f64 {
        self.inner.total_sector
    }

    /// Sum of the selection effects.
    #[getter]
    fn total_selection(&self) -> f64 {
        self.inner.total_selection
    }

    /// Per-cell curve effects as a :class:`pandas.DataFrame`.
    ///
    /// The primary frame is the duration-cell axis; the two (cell, sector)
    /// tables are available from :meth:`to_sector_effects_dataframe` .
    ///
    /// Columns: ``cell``, ``portfolio_weight``, ``benchmark_weight``,
    /// ``benchmark_cell_return``, ``curve_effect``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(py, &self.inner.curve_effects, GRID_CURVE_COLUMNS)
    }

    /// Per-(cell, sector) allocation and selection effects as a :class:`pandas.DataFrame`.
    ///
    /// Columns: ``cell``, ``sector``, ``allocation_effect``, ``selection_effect``.
    fn to_sector_effects_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(py, &self.inner.sector_effects, GRID_SECTOR_COLUMNS)
    }
    /// Serialize to a compact JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_portfolio::GridAttributionResult =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "GridAttributionResult(cells={}, active_return={})",
            self.inner.curve_effects.len(),
            self.inner.active_return,
        )
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

/// Multi-period Carino-linked hierarchical grid attribution result.
///
/// Returned by :func:`grid_carino_link`. Only the three top-level effects
/// are linked (see the Rust module docs).
#[pyclass(
    name = "GridCarinoLinkedResult",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyGridCarinoLinkedResult {
    pub(crate) inner: finstack_quant_portfolio::GridCarinoLinkedResult,
}

#[pymethods]
impl PyGridCarinoLinkedResult {
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

    /// Sum of per-period Carino-scaled curve effects.
    #[getter]
    fn linked_curve(&self) -> f64 {
        self.inner.linked_curve
    }

    /// Sum of per-period Carino-scaled sector allocation effects.
    #[getter]
    fn linked_sector(&self) -> f64 {
        self.inner.linked_sector
    }

    /// Sum of per-period Carino-scaled selection effects.
    #[getter]
    fn linked_selection(&self) -> f64 {
        self.inner.linked_selection
    }

    /// Single-row :class:`pandas.DataFrame` view of the linked totals.
    ///
    /// Columns: ``portfolio_return_compounded``,
    /// ``benchmark_return_compounded``, ``linked_curve``, ``linked_sector``,
    /// ``linked_selection``. The per-period detail is available from the
    /// ``periods`` getter.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let row = serde_json::json!({
            "portfolio_return_compounded": self.inner.portfolio_return_compounded,
            "benchmark_return_compounded": self.inner.benchmark_return_compounded,
            "linked_curve": self.inner.linked_curve,
            "linked_sector": self.inner.linked_sector,
            "linked_selection": self.inner.linked_selection,
        });
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "portfolio_return_compounded",
                "benchmark_return_compounded",
                "linked_curve",
                "linked_sector",
                "linked_selection",
            ],
        )
    }

    /// Serialize to a compact JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_portfolio::GridCarinoLinkedResult =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "GridCarinoLinkedResult(periods={}, linked_selection={})",
            self.inner.periods.len(),
            self.inner.linked_selection,
        )
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

/// Parse both sides and run the canonical single-period grid attribution.
fn run_grid_attribution(
    py: Python<'_>,
    portfolio_json: &str,
    benchmark_json: &str,
) -> PyResult<finstack_quant_portfolio::GridAttributionResult> {
    let portfolio_json = portfolio_json.to_owned();
    let benchmark_json = benchmark_json.to_owned();
    py.detach(move || {
        let portfolio: Vec<finstack_quant_portfolio::GridPosition> =
            serde_json::from_str(&portfolio_json)
                .map_err(|err| serde_json_to_py(err, "invalid grid portfolio JSON"))?;
        let benchmark: Vec<finstack_quant_portfolio::GridPosition> =
            serde_json::from_str(&benchmark_json)
                .map_err(|err| serde_json_to_py(err, "invalid grid benchmark JSON"))?;
        finstack_quant_portfolio::grid_attribution(&portfolio, &benchmark).map_err(portfolio_to_py)
    })
}

/// Parse period JSON and run the canonical grid Carino linking.
fn run_grid_carino_link(
    py: Python<'_>,
    periods_json: &str,
) -> PyResult<finstack_quant_portfolio::GridCarinoLinkedResult> {
    let periods_json = periods_json.to_owned();
    py.detach(move || {
        let periods: Vec<finstack_quant_portfolio::GridAttributionResult> =
            serde_json::from_str(&periods_json)
                .map_err(|err| serde_json_to_py(err, "invalid grid periods JSON"))?;
        finstack_quant_portfolio::grid_carino_link(&periods).map_err(portfolio_to_py)
    })
}

/// Compute a single-period hierarchical duration-cell x sector grid attribution.
///
/// Binds Rust `finstack_quant_portfolio::grid_attribution` (Dynkin, Hyman &
/// Vankudre 1998, Appendix A): decomposes active return into a per-cell
/// curve (positioning) effect, a within-cell sector allocation effect, and a
/// security-selection residual per (cell, sector).
///
/// Parameters
/// ----------
/// portfolio_json : str | dict | list | pandas.DataFrame
///     JSON array of ``GridPosition`` objects (``cell``, ``sector``,
///     ``weight``, ``total_return``) for the portfolio side; weights must
///     sum to ``1.0`` within ``1e-6``.
/// benchmark_json : str | dict | list | pandas.DataFrame
///     JSON array of ``GridPosition`` objects for the benchmark side; same
///     weight-sum requirement.
///
/// Returns
/// -------
/// GridAttributionResult
///     Typed result whose ``total_curve``, ``total_sector`` and
///     ``total_selection`` sum to ``active_return`` to floating-point
///     precision for well-conditioned inputs; among *accepted* inputs (those
///     that clear the near-zero-net-weight check below), the reconciliation
///     residual grows the closer any bucket's net weight sits to that
///     check's own rejection boundary — see the Rust module docs' measured
///     residuals for magnitudes. Use :meth:`to_json` for the
///     raw wire string.
///
/// Raises
/// ------
/// PortfolioError
///     If any weight or return is non-finite, either side's weights don't
///     sum to ``1.0`` within tolerance, or a (cell) or (cell, sector)
///     bucket has positions on a side but nets to a weight that is zero, or
///     numerically near zero (within ``1e-6`` relative to its own gross
///     weight), which is undefined-or-explosive to attribute (the error
///     names the bucket and the side).
/// ValueError
///     If either JSON argument is malformed or carries unknown fields.
///
/// Sources
/// -------
/// See ``docs/REFERENCES.md#dynkin-hyman-vankudre-1998``.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant.portfolio import grid_attribution
/// >>> portfolio = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.02}]
/// >>> benchmark = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.01}]
/// >>> result = grid_attribution(json.dumps(portfolio), json.dumps(benchmark))
/// >>> result.total_selection
/// 0.01
#[pyfunction]
#[pyo3(text_signature = "(portfolio_json, benchmark_json)")]
fn grid_attribution(
    py: Python<'_>,
    portfolio_json: &Bound<'_, PyAny>,
    benchmark_json: &Bound<'_, PyAny>,
) -> PyResult<PyGridAttributionResult> {
    let portfolio_json =
        crate::bindings::extract::extract_records_json(py, portfolio_json, "portfolio")?;
    let portfolio_json: &str = &portfolio_json;
    let benchmark_json =
        crate::bindings::extract::extract_records_json(py, benchmark_json, "benchmark")?;
    let benchmark_json: &str = &benchmark_json;
    Ok(PyGridAttributionResult {
        inner: run_grid_attribution(py, portfolio_json, benchmark_json)?,
    })
}
/// Carino-link multi-period hierarchical grid attribution results.
///
/// Binds Rust `finstack_quant_portfolio::grid_carino_link` (Carino 1999):
/// applies Carino smoothing to a chronological sequence of single-period
/// `grid_attribution` results so the three top-level effects
/// (``linked_curve``, ``linked_sector``, ``linked_selection``) sum exactly
/// to the geometrically compounded active return. Only the three top-level
/// effects are linked; per-cell / per-(cell, sector) multi-period linking is
/// out of scope.
///
/// Parameters
/// ----------
/// periods_json : str | dict | list | pandas.DataFrame
///     JSON array of ``GridAttributionResult`` objects, in chronological
///     order, each the wire output of :meth:`to_json` (or
///     ``GridAttributionResult.to_json()``).
///
/// Returns
/// -------
/// GridCarinoLinkedResult
///     Typed result with the three linked effects and compounded returns.
///     Use :meth:`to_json` serialize with ``to_json()`` for the wire string.
///
/// Raises
/// ------
/// PortfolioError
///     If ``periods_json`` is empty; any consumed return or top-level effect
///     is non-finite; ``active_return`` disagrees with the portfolio-minus-
///     benchmark return; the three effect totals do not reconcile to
///     ``active_return`` within an overflow-safe scaled-L1 tolerance; a
///     return-identity or reconciliation residual is non-finite; or any
///     per-period or compounded return is at or below -100% (outside the
///     Carino domain).
/// ValueError
///     If ``periods_json`` is malformed or does not match the
///     ``GridAttributionResult`` schema.
///
/// Sources
/// -------
/// See ``docs/REFERENCES.md#carino-1999`` and
/// ``docs/REFERENCES.md#dynkin-hyman-vankudre-1998``.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant.portfolio import grid_attribution, grid_carino_link
/// >>> portfolio = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.02}]
/// >>> benchmark = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.01}]
/// >>> period = json.loads(grid_attribution(json.dumps(portfolio), json.dumps(benchmark)).to_json())
/// >>> result = grid_carino_link(json.dumps([period, period]))
/// >>> round(result.linked_selection, 4)
/// 0.0203
#[pyfunction]
#[pyo3(text_signature = "(periods_json)")]
fn grid_carino_link(
    py: Python<'_>,
    periods_json: &Bound<'_, PyAny>,
) -> PyResult<PyGridCarinoLinkedResult> {
    let periods_json = crate::bindings::extract::extract_records_json(py, periods_json, "periods")?;
    let periods_json: &str = &periods_json;
    Ok(PyGridCarinoLinkedResult {
        inner: run_grid_carino_link(py, periods_json)?,
    })
}
/// Register grid attribution functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyGridAttributionResult>()?;
    m.add_class::<PyGridCarinoLinkedResult>()?;
    m.add_function(wrap_pyfunction!(grid_attribution, m)?)?;
    m.add_function(wrap_pyfunction!(grid_carino_link, m)?)?;
    Ok(())
}
