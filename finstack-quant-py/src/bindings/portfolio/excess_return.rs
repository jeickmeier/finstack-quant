//! Python bindings for duration-matched credit excess returns.
//!
//! Binds `finstack_quant_portfolio::excess_return`: duration-cell base-return
//! tables built either from a reference universe (Dynkin, Hyman & Vankudre
//! 1998, Appendix B) or from discount-curve snapshots, plus position-level
//! duration-matched excess returns measured against either table. The typed
//! entry points return `Py*` wrappers; the paired `*_json` functions keep the
//! exact JSON wire strings. The curve-snapshot path takes typed
//! `DiscountCurve` objects directly (same pattern as the XVA bindings in
//! `crate::bindings::margin::xva`).

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

use crate::bindings::core::market_data::curves::PyDiscountCurve;
use crate::bindings::pandas_utils::{
    serde_rows_to_dataframe_with_schema, serde_to_py, ColumnSchema,
};
use crate::errors::{display_to_py, portfolio_to_py, serde_json_to_py};

/// Column schema for [`PyDurationCellTable::to_dataframe`].
const CELL_COLUMNS: &[ColumnSchema<'static>] = &[
    ("label", "str"),
    ("lower", "float64"),
    ("upper", "float64"),
    ("base_return", "float64"),
    ("observed", "bool"),
];

/// Column schema for [`PyExcessReturnResult::to_dataframe`].
const POSITION_EXCESS_COLUMNS: &[ColumnSchema<'static>] = &[
    ("id", "str"),
    ("cell", "str"),
    ("base_return", "float64"),
    ("excess_return", "float64"),
];

/// Duration-cell base-return table.
///
/// Returned by :func:`cell_returns_from_reference` and
/// :func:`cell_returns_from_curves`; consumed by :func:`excess_returns`
/// (pass ``table.to_json()``).
#[pyclass(
    name = "DurationCellTable",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyDurationCellTable {
    pub(crate) inner: finstack_quant_portfolio::DurationCellTable,
}

#[pymethods]
impl PyDurationCellTable {
    /// Label identifying the reference universe (e.g. ``"UST"``).
    #[getter]
    fn base_label(&self) -> String {
        self.inner.base_label.clone()
    }

    /// Cells as a list of dicts, in ascending duration order.
    #[getter]
    fn cells<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.cells)
    }

    /// Cells as a :class:`pandas.DataFrame`.
    ///
    /// Columns: ``label``, ``lower``, ``upper``, ``base_return``,
    /// ``observed``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(py, &self.inner.cells, CELL_COLUMNS)
    }

    /// Serialize to a compact JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_portfolio::DurationCellTable =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "DurationCellTable(base_label={:?}, cells={})",
            self.inner.base_label,
            self.inner.cells.len(),
        )
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

/// Per-position and portfolio-level duration-matched credit excess returns.
///
/// Returned by :func:`excess_returns`.
#[pyclass(
    name = "ExcessReturnResult",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyExcessReturnResult {
    pub(crate) inner: finstack_quant_portfolio::ExcessReturnResult,
}

#[pymethods]
impl PyExcessReturnResult {
    /// Label of the base curve the excess returns were measured against.
    #[getter]
    fn base_label(&self) -> String {
        self.inner.base_label.clone()
    }

    /// Per-position results as a list of dicts, in input order.
    #[getter]
    fn positions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.positions)
    }

    /// Weight-weighted portfolio total return.
    #[getter]
    fn portfolio_total_return(&self) -> f64 {
        self.inner.portfolio_total_return
    }

    /// Weight-weighted portfolio base return.
    #[getter]
    fn portfolio_base_return(&self) -> f64 {
        self.inner.portfolio_base_return
    }

    /// Weight-weighted portfolio excess return.
    #[getter]
    fn portfolio_excess_return(&self) -> f64 {
        self.inner.portfolio_excess_return
    }

    /// Per-position excess returns as a :class:`pandas.DataFrame`.
    ///
    /// Columns: ``id``, ``cell``, ``base_return``, ``excess_return``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(py, &self.inner.positions, POSITION_EXCESS_COLUMNS)
    }

    /// Serialize to a compact JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_portfolio::ExcessReturnResult =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "ExcessReturnResult(positions={}, portfolio_excess_return={})",
            self.inner.positions.len(),
            self.inner.portfolio_excess_return,
        )
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

/// Parse inputs and build the canonical reference-universe cell table.
fn run_cell_returns_from_reference(
    py: Python<'_>,
    reference_json: &str,
    base_label: &str,
    config_json: &str,
) -> PyResult<finstack_quant_portfolio::DurationCellTable> {
    let reference_json = reference_json.to_owned();
    let base_label = base_label.to_owned();
    let config_json = config_json.to_owned();
    py.detach(move || {
        let reference: Vec<finstack_quant_portfolio::ReferenceReturn> =
            serde_json::from_str(&reference_json)
                .map_err(|err| serde_json_to_py(err, "invalid reference JSON"))?;
        let config: finstack_quant_portfolio::CellConfig = serde_json::from_str(&config_json)
            .map_err(|err| serde_json_to_py(err, "invalid config JSON"))?;
        finstack_quant_portfolio::cell_returns_from_reference(&reference, &base_label, &config)
            .map_err(portfolio_to_py)
    })
}

/// Parse inputs and build the canonical curve-snapshot cell table.
fn run_cell_returns_from_curves(
    py: Python<'_>,
    start: &PyDiscountCurve,
    end: &PyDiscountCurve,
    horizon_years: f64,
    max_duration: f64,
    base_label: &str,
    config_json: &str,
) -> PyResult<finstack_quant_portfolio::DurationCellTable> {
    let start_curve = std::sync::Arc::clone(&start.inner);
    let end_curve = std::sync::Arc::clone(&end.inner);
    let base_label = base_label.to_owned();
    let config_json = config_json.to_owned();
    py.detach(move || {
        let config: finstack_quant_portfolio::CellConfig = serde_json::from_str(&config_json)
            .map_err(|err| serde_json_to_py(err, "invalid config JSON"))?;
        finstack_quant_portfolio::cell_returns_from_curves(
            &start_curve,
            &end_curve,
            horizon_years,
            max_duration,
            &base_label,
            &config,
        )
        .map_err(portfolio_to_py)
    })
}

/// Parse inputs and run the canonical excess-return calculation.
fn run_excess_returns(
    py: Python<'_>,
    positions_json: &str,
    table_json: &str,
) -> PyResult<finstack_quant_portfolio::ExcessReturnResult> {
    let positions_json = positions_json.to_owned();
    let table_json = table_json.to_owned();
    py.detach(move || {
        let positions: Vec<finstack_quant_portfolio::ExcessReturnPosition> =
            serde_json::from_str(&positions_json)
                .map_err(|err| serde_json_to_py(err, "invalid positions JSON"))?;
        let table: finstack_quant_portfolio::DurationCellTable = serde_json::from_str(&table_json)
            .map_err(|err| serde_json_to_py(err, "invalid table JSON"))?;
        finstack_quant_portfolio::excess_returns(&positions, &table).map_err(portfolio_to_py)
    })
}

/// Build a duration-cell base-return table from a reference universe.
///
/// Binds Rust `finstack_quant_portfolio::cell_returns_from_reference`
/// (Dynkin, Hyman & Vankudre 1998, Appendix B): buckets `reference_json` into
/// fixed-width duration cells and averages each cell's member total returns,
/// interpolating interior gaps and flat-extrapolating leading/trailing gaps.
///
/// Parameters
/// ----------
/// reference_json : str | dict | list | pandas.DataFrame
///     JSON array of ``ReferenceReturn`` objects (``duration``,
///     ``total_return``, both decimals with duration in years); must be
///     non-empty. Unknown fields are rejected.
/// base_label : str
///     Label identifying the resulting curve (e.g. ``"UST"``), carried
///     through to the output's ``base_label`` for policy visibility.
/// config_json : str | dict | list | pandas.DataFrame
///     JSON ``CellConfig``; ``width`` is its only field (cell width in
///     years, finite and positive) and is required — there is no default.
///
/// Returns
/// -------
/// DurationCellTable
///     Typed cell table with ``to_dataframe()`` / ``to_json()`` exits. Use
///     :meth:`to_json` for the raw wire string.
///
/// Raises
/// ------
/// PortfolioError
///     If ``reference_json`` is empty, ``config.width`` is not finite and
///     positive, any reference entry has a non-finite ``total_return`` or a
///     non-finite/negative ``duration``, the width produces two numerically
///     distinct cells that collide on their one-decimal label, or the
///     largest reference ``duration`` divided by ``config.width`` would
///     require more than an internal sanity bound of cells (100,000; a units
///     mistake, e.g. days instead of years, rather than a legitimate
///     duration grid).
/// ValueError
///     If any JSON argument is malformed or carries unknown fields.
///
/// Sources
/// -------
/// See ``docs/REFERENCES.md#dynkin-hyman-vankudre-1998``.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant.portfolio import cell_returns_from_reference
/// >>> reference = [{"duration": 1.0, "total_return": 0.02}]
/// >>> table = cell_returns_from_reference(json.dumps(reference), "UST", '{"width": 2.0}')
/// >>> table.cells[0]["base_return"]
/// 0.02
#[pyfunction]
#[pyo3(text_signature = "(reference_json, base_label, config_json)")]
fn cell_returns_from_reference(
    py: Python<'_>,
    reference_json: &Bound<'_, PyAny>,
    base_label: &str,
    config_json: &Bound<'_, PyAny>,
) -> PyResult<PyDurationCellTable> {
    let reference_json =
        crate::bindings::extract::extract_records_json(py, reference_json, "reference")?;
    let reference_json: &str = &reference_json;
    let config_json = crate::bindings::extract::extract_records_json(py, config_json, "config")?;
    let config_json: &str = &config_json;
    Ok(PyDurationCellTable {
        inner: run_cell_returns_from_reference(py, reference_json, base_label, config_json)?,
    })
}
/// Build a duration-cell base-return table from start/end discount curves.
///
/// Binds Rust `finstack_quant_portfolio::cell_returns_from_curves`: each
/// cell's base return is the holding-period return of a hypothetical
/// zero-coupon position bought at the cell midpoint off ``start`` and
/// revalued off ``end`` after ``horizon_years`` have elapsed. Every
/// resulting cell is observed (a curve has a discount factor at every
/// queried point), unlike the reference-universe path in
/// :func:`cell_returns_from_reference`.
///
/// Parameters
/// ----------
/// start : DiscountCurve
///     Discount curve observed at the start of the holding period.
/// end : DiscountCurve
///     Discount curve observed ``horizon_years`` later, at period end.
/// horizon_years : float
///     Length of the holding period, in years; must be finite and positive.
/// max_duration : float
///     Upper bound of the duration grid, in years; must be finite and
///     strictly greater than ``horizon_years``.
/// base_label : str
///     Label identifying the base curve (e.g. ``"UST"``, ``"USD-SOFR"``),
///     stamped into the result purely for policy visibility.
/// config_json : str | dict | list | pandas.DataFrame
///     JSON ``CellConfig``; ``width`` is its only field and is required.
///
/// Returns
/// -------
/// DurationCellTable
///     Typed cell table with ``to_dataframe()`` / ``to_json()`` exits. Use
///     :meth:`to_json` for the raw wire string.
///
/// Raises
/// ------
/// PortfolioError
///     If ``config.width`` or ``horizon_years`` is not finite and positive,
///     ``max_duration`` does not strictly exceed ``horizon_years``,
///     ``max_duration`` divided by ``config.width`` would require more than
///     an internal sanity bound of cells (100,000; a units mistake rather
///     than a legitimate duration grid), a cell's midpoint does not exceed
///     ``horizon_years`` (this is unavoidable whenever ``config.width`` is
///     not strictly greater than ``2 * horizon_years``, since the first
///     cell's midpoint is ``config.width / 2``), either curve produces a
///     non-positive/non-finite discount factor at a queried time, or the
///     width produces colliding one-decimal cell labels.
/// ValueError
///     If ``config_json`` is malformed.
///
/// Sources
/// -------
/// See ``docs/REFERENCES.md#dynkin-hyman-vankudre-1998``.
///
/// Examples
/// --------
/// >>> from datetime import date
/// >>> from finstack_quant.core.market_data import DiscountCurve
/// >>> from finstack_quant.portfolio import cell_returns_from_curves
/// >>> start = DiscountCurve.flat("start", date(2025, 1, 1), 0.02)
/// >>> end = DiscountCurve.flat("end", date(2025, 4, 1), 0.03)
/// >>> table = cell_returns_from_curves(start, end, 0.25, 2.0, "UST", '{"width": 1.0}')
/// >>> len(table.cells)
/// 2
#[pyfunction]
#[pyo3(text_signature = "(start, end, horizon_years, max_duration, base_label, config_json)")]
fn cell_returns_from_curves(
    py: Python<'_>,
    start: &PyDiscountCurve,
    end: &PyDiscountCurve,
    horizon_years: f64,
    max_duration: f64,
    base_label: &str,
    config_json: &Bound<'_, PyAny>,
) -> PyResult<PyDurationCellTable> {
    let config_json = crate::bindings::extract::extract_records_json(py, config_json, "config")?;
    let config_json: &str = &config_json;
    Ok(PyDurationCellTable {
        inner: run_cell_returns_from_curves(
            py,
            start,
            end,
            horizon_years,
            max_duration,
            base_label,
            config_json,
        )?,
    })
}
/// Compute duration-matched credit excess returns against a base-return table.
///
/// Binds Rust `finstack_quant_portfolio::excess_returns` (Dynkin, Hyman &
/// Vankudre 1998, Appendix B): each position's ``duration`` is matched to its
/// duration cell in ``table_json`` and the position's excess return is
/// ``total_return - cell.base_return``, the credit-specific component of
/// performance isolated from the general level/shape move of the base curve.
///
/// Parameters
/// ----------
/// positions_json : str | dict | list | pandas.DataFrame
///     JSON array of ``ExcessReturnPosition`` objects (``id``, ``weight``,
///     ``duration``, ``total_return``); weights must sum to ``1.0`` within
///     ``1e-6``.
/// table_json : str | dict | list | pandas.DataFrame
///     JSON ``DurationCellTable``, as returned by
///     :meth:`to_json`,
///     :meth:`to_json`, or
///     ``DurationCellTable.to_json()``.
///
/// Returns
/// -------
/// ExcessReturnResult
///     Typed result with per-position and portfolio-level total/base/excess
///     returns. Use :meth:`to_json` for the raw wire string.
///
/// Raises
/// ------
/// PortfolioError
///     If ``table_json`` has no cells; a cell has an empty or duplicate
///     label, non-finite bounds or base return, a negative lower bound, a
///     non-positive width, non-ascending lower bounds, or overlaps its
///     predecessor; a position has a non-finite
///     weight/duration/total_return; a position's duration falls outside
///     every cell, including a valid gap (the error names the position); or
///     the position weights do not sum to ``1.0`` within tolerance.
/// ValueError
///     If any JSON argument is malformed.
///
/// Sources
/// -------
/// See ``docs/REFERENCES.md#dynkin-hyman-vankudre-1998``.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant.portfolio import cell_returns_from_reference, excess_returns
/// >>> reference = [{"duration": 1.0, "total_return": 0.02}]
/// >>> table = cell_returns_from_reference(json.dumps(reference), "UST", '{"width": 2.0}')
/// >>> positions = [{"id": "B1", "weight": 1.0, "duration": 1.0, "total_return": 0.03}]
/// >>> result = excess_returns(json.dumps(positions), table.to_json())
/// >>> round(result.portfolio_excess_return, 4)
/// 0.01
#[pyfunction]
#[pyo3(text_signature = "(positions_json, table_json)")]
fn excess_returns(
    py: Python<'_>,
    positions_json: &Bound<'_, PyAny>,
    table_json: &Bound<'_, PyAny>,
) -> PyResult<PyExcessReturnResult> {
    let positions_json =
        crate::bindings::extract::extract_records_json(py, positions_json, "positions")?;
    let positions_json: &str = &positions_json;
    let table_json = crate::bindings::extract::extract_records_json(py, table_json, "table")?;
    let table_json: &str = &table_json;
    Ok(PyExcessReturnResult {
        inner: run_excess_returns(py, positions_json, table_json)?,
    })
}
/// Register duration-matched credit excess return functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyDurationCellTable>()?;
    m.add_class::<PyExcessReturnResult>()?;
    m.add_function(wrap_pyfunction!(cell_returns_from_reference, m)?)?;
    m.add_function(wrap_pyfunction!(cell_returns_from_curves, m)?)?;
    m.add_function(wrap_pyfunction!(excess_returns, m)?)?;
    Ok(())
}
