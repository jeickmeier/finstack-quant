use numpy::{PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use finstack_quant_factor::risk as model_risk;

use crate::errors::{core_to_py, display_to_py, value_error};

use super::contributions::{PyPositionRiskDecomposition, PyRiskDecomposition};
use crate::bindings::matrix_input::{
    extract_position_pnls, extract_square_matrix, PositionPnlMatrix,
};
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::portfolio::sensitivity::PySensitivityMatrix;

/// `True` when `obj` is a `pandas.DataFrame` (checked by module + type name so
/// pandas is not imported when it is not already loaded).
fn is_dataframe(obj: &Bound<'_, PyAny>) -> PyResult<bool> {
    let ty = obj.get_type();
    let name: String = ty.getattr("__name__")?.extract()?;
    if name != "DataFrame" {
        return Ok(false);
    }
    let module: String = ty.getattr("__module__")?.extract()?;
    Ok(module.starts_with("pandas"))
}

/// Column labels of a DataFrame as strings.
fn dataframe_columns(frame: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let columns = frame.getattr("columns")?;
    columns
        .try_iter()?
        .map(|c| c?.str()?.extract::<String>())
        .collect()
}

/// `frame.to_numpy(dtype="float64")` as a NumPy array object.
fn dataframe_to_f64_array<'py>(
    py: Python<'py>,
    frame: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let kwargs = PyDict::new(py);
    kwargs.set_item("dtype", "float64")?;
    frame
        .call_method("to_numpy", (), Some(&kwargs))
        .map_err(|_| value_error("DataFrame must be convertible to a float64 array"))
}

/// Resolve `position_ids` and the square `covariance` matrix.
///
/// `covariance` accepts a nested list, a 2-D NumPy array, or a
/// `pandas.DataFrame`; for a DataFrame `position_ids` may be `None`, in
/// which case the frame's column labels are used.
fn extract_covariance_input(
    py: Python<'_>,
    position_ids: Option<Vec<String>>,
    covariance: &Bound<'_, PyAny>,
    n_weights: usize,
) -> PyResult<(Vec<String>, Vec<f64>)> {
    if is_dataframe(covariance)? {
        let columns = dataframe_columns(covariance)?;
        let ids = position_ids.unwrap_or(columns);
        let array = dataframe_to_f64_array(py, covariance)?;
        let flat = extract_square_matrix(py, &array, n_weights, "covariance")?;
        return Ok((ids, flat));
    }
    let ids = position_ids.ok_or_else(|| {
        value_error("position_ids is required unless covariance is a pandas.DataFrame")
    })?;
    let flat = extract_square_matrix(py, covariance, n_weights, "covariance")?;
    Ok((ids, flat))
}

/// Resolve `position_ids` and the position × scenario P&L matrix.
///
/// Accepted shapes for `position_pnls`:
///
/// - `pandas.DataFrame` — rows are scenarios, columns are positions
///   (`position_ids` defaults to the column labels);
/// - nested list or 2-D NumPy array shaped `n_positions × n_scenarios`
///   (position-major), the Rust `flatten_position_pnls` layout WASM shares.
pub(super) fn extract_pnl_input(
    py: Python<'_>,
    position_ids: Option<Vec<String>>,
    position_pnls: &Bound<'_, PyAny>,
) -> PyResult<(Vec<String>, PositionPnlMatrix)> {
    if is_dataframe(position_pnls)? {
        let columns = dataframe_columns(position_pnls)?;
        let ids = position_ids.unwrap_or(columns);
        let array = dataframe_to_f64_array(py, position_pnls)?;
        let array = array
            .extract::<PyReadonlyArray2<'_, f64>>()
            .map_err(|_| value_error("position_pnls DataFrame must be two-dimensional"))?;
        let shape = array.shape();
        if shape[1] != ids.len() {
            return Err(value_error(format!(
                "position_pnls DataFrame has {} columns but {} position ids; \
                 rows are scenarios and columns are positions",
                shape[1],
                ids.len()
            )));
        }
        let data: Vec<f64> = array.as_array().iter().copied().collect();
        let matrix = PositionPnlMatrix::from_scenario_major(data, shape[0]);
        return Ok((ids, matrix));
    }
    let ids = position_ids.ok_or_else(|| {
        value_error("position_ids is required unless position_pnls is a pandas.DataFrame")
    })?;
    let matrix = extract_position_pnls(py, position_pnls, ids.len())?;
    Ok((ids, matrix))
}

/// Decompose portfolio VaR/ES into position contributions via parametric
/// Euler allocation, returning a typed ``PositionRiskDecomposition``.
///
/// Args:
///     position_ids: Position identifiers aligned with ``weights``; may be
///         ``None`` when ``covariance`` is a ``pandas.DataFrame`` (its column
///         labels are used).
///     weights: Portfolio weights or exposures, one per position.
///     covariance: Square position-return covariance matrix aligned with
///         ``position_ids`` — nested list, 2-D NumPy array, or
///         ``pandas.DataFrame``.
///     confidence: Tail confidence as a decimal probability strictly inside
///         ``(0.5, 1)``. ``None`` uses the default ``0.95``.
///     compute_incremental: Whether to compute leave-one-out incremental VaR
///         (one full repricing per position). Defaults to ``False``.
///
/// Returns:
///     ``PositionRiskDecomposition`` with portfolio VaR/ES (losses negative)
///     and per-position VaR and ES contributions.
///
/// Raises:
///     ValueError: If dimensions disagree, the covariance is not symmetric
///         positive semidefinite, or ``confidence`` is outside ``(0.5, 1)``.
#[pyfunction]
#[pyo3(signature = (position_ids, weights, covariance, confidence = None, compute_incremental = false))]
pub(super) fn parametric_var_decomposition(
    py: Python<'_>,
    position_ids: Option<Vec<String>>,
    weights: Vec<f64>,
    covariance: &Bound<'_, PyAny>,
    confidence: Option<f64>,
    compute_incremental: bool,
) -> PyResult<PyPositionRiskDecomposition> {
    let n = weights.len();
    let (position_ids, cov_flat) = extract_covariance_input(py, position_ids, covariance, n)?;
    let result = py
        .detach(move || {
            model_risk::parametric_var_decomposition(
                &position_ids,
                &weights,
                &cov_flat,
                confidence,
                compute_incremental,
            )
        })
        .map_err(core_to_py)?;

    Ok(PyPositionRiskDecomposition::from_inner(result))
}

/// Decompose portfolio VaR and ES from per-position scenario P&Ls via
/// historical simulation, returning a typed ``PositionRiskDecomposition``.
///
/// Args:
///     position_ids: Position identifiers; may be ``None`` when
///         ``position_pnls`` is a ``pandas.DataFrame`` (column labels are
///         used).
///     position_pnls: P&L matrix. A ``pandas.DataFrame`` is read as rows =
///         scenarios, columns = positions. A nested list or 2-D NumPy array is
///         read as ``n_positions x n_scenarios`` (position-major, one row per
///         position). Losses are negative.
///     confidence: Tail confidence as a decimal probability strictly inside
///         ``(0.5, 1)``. ``None`` uses the default ``0.95``.
///
/// Returns:
///     ``PositionRiskDecomposition`` with historical VaR/ES totals and
///     per-position contributions (marginal and incremental VaR are ``None``).
///
/// Raises:
///     ValueError: If the matrix is empty, ragged, does not have one row per
///         position id, too few scenarios resolve the tail, or ``confidence``
///         is outside ``(0.5, 1)``.
#[pyfunction]
#[pyo3(signature = (position_ids, position_pnls, confidence = None))]
pub(super) fn historical_var_decomposition(
    py: Python<'_>,
    position_ids: Option<Vec<String>>,
    position_pnls: &Bound<'_, PyAny>,
    confidence: Option<f64>,
) -> PyResult<PyPositionRiskDecomposition> {
    let (position_ids, position_pnls) = extract_pnl_input(py, position_ids, position_pnls)?;
    let n = position_ids.len();
    let n_scenarios = position_pnls.n_scenarios();

    let result = py
        .detach(move || {
            let flat = position_pnls.into_scenario_major(n);
            model_risk::historical_var_decomposition(&position_ids, &flat, n_scenarios, confidence)
        })
        .map_err(core_to_py)?;

    Ok(PyPositionRiskDecomposition::from_inner(result))
}

/// Look up a position by id inside a ``PositionRiskDecomposition`` and
/// return its component VaR.
///
/// Args:
///     decomp: Decomposition to search.
///     position_id: Position identifier.
///
/// Raises:
///     KeyError: If ``position_id`` is not in the decomposition.
#[pyfunction]
#[pyo3(signature = (decomp, position_id))]
pub(super) fn position_component_var(
    decomp: &PyPositionRiskDecomposition,
    position_id: &str,
) -> PyResult<f64> {
    decomp.inner.component_var(position_id).map_err(core_to_py)
}

/// Decompose portfolio risk into factor and position contributions.
///
/// Uses the parametric (covariance-based) Euler decomposition to attribute
/// forecasted portfolio risk across factors and individual positions.
///
/// Args:
///     sensitivities: Weighted position x factor ``SensitivityMatrix``, as
///         returned by ``finstack_quant.portfolio.compute_factor_sensitivities``.
///     covariance_json: ``FactorCovarianceMatrix`` as JSON text or a dict
///         (``{"factor_ids", "n", "data"}``, ``data`` row-major). Must use the
///         same factor IDs and ordering as the sensitivity matrix.
///     risk_measure: ``RiskMeasure`` wire value: ``"variance"``,
///         ``"volatility"``, ``{"var": {"confidence": 0.99}}`` or
///         ``{"expected_shortfall": {"confidence": 0.975}}``. ``None`` uses
///         the Rust default (``"variance"``).
///
/// Returns:
///     ``RiskDecomposition`` with portfolio-level risk and factor and
///     position detail.
///
/// Raises:
///     ValueError: If the covariance or risk measure is malformed, the factor
///         axes of the sensitivities and covariance disagree, or the
///         decomposition produces a non-finite value.
#[pyfunction]
#[pyo3(signature = (sensitivities, covariance_json, risk_measure=None))]
pub(super) fn decompose_factors(
    py: Python<'_>,
    sensitivities: &PySensitivityMatrix,
    covariance_json: &Bound<'_, PyAny>,
    risk_measure: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyRiskDecomposition> {
    let covariance_json =
        crate::bindings::extract::extract_records_json(py, covariance_json, "covariance")?;
    let measure: finstack_quant_factor::RiskMeasure = match risk_measure {
        Some(obj) => py_to_serde(py, obj, "risk_measure")?,
        None => finstack_quant_factor::RiskMeasure::default(),
    };

    let matrix = sensitivities.inner.clone();
    py.detach(move || {
        let covariance: finstack_quant_factor::FactorCovarianceMatrix =
            serde_json::from_str(&covariance_json).map_err(display_to_py)?;
        let result =
            model_risk::decompose_factors(&matrix, &covariance, &measure).map_err(core_to_py)?;
        Ok(PyRiskDecomposition::from_inner(result))
    })
}
