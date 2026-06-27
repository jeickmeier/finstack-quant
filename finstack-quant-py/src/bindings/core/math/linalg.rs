//! Python bindings for `finstack_quant_core::math::linalg`.

use finstack_quant_core::math::linalg;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

// ---------------------------------------------------------------------------
// Custom exception
// ---------------------------------------------------------------------------

pyo3::create_exception!(
    finstack_quant.core.math.linalg,
    CholeskyError,
    PyValueError,
    "Cholesky decomposition failure (inherits ValueError)."
);

/// Map a core linear-algebra error to a Python `CholeskyError` exception.
fn cholesky_err(e: impl std::fmt::Display) -> PyErr {
    CholeskyError::new_err(e.to_string())
}

// ---------------------------------------------------------------------------
// Helpers: nested ↔ flat matrix conversion
// ---------------------------------------------------------------------------

/// Flatten a `list[list[float]]` into a row-major `Vec<f64>` and return `(flat, n)`.
///
/// Returns a `PyResult::Err` when the input is not a square matrix.
fn flatten_matrix(rows: Vec<Vec<f64>>) -> PyResult<(Vec<f64>, usize)> {
    let n = rows.len();
    for (i, row) in rows.iter().enumerate() {
        if row.len() != n {
            return Err(crate::errors::value_error(format!(
                "Row {i} has length {} but expected {n} for a square matrix",
                row.len()
            )));
        }
    }
    let flat: Vec<f64> = rows.into_iter().flatten().collect();
    Ok((flat, n))
}

/// Unflatten a row-major `Vec<f64>` of length `n*n` into `Vec<Vec<f64>>`.
fn unflatten_matrix(flat: Vec<f64>, n: usize) -> Vec<Vec<f64>> {
    flat.chunks(n).map(|c| c.to_vec()).collect()
}

// ---------------------------------------------------------------------------
// Module-level functions
// ---------------------------------------------------------------------------

/// Compute the Cholesky decomposition L of a symmetric positive-definite matrix
/// such that A = L L^T.
///
/// Accepts a square matrix as `list[list[float]]` and returns the lower-triangular
/// factor in the same shape.
///
/// Raises ``CholeskyError`` when the matrix is not positive-definite, is singular,
/// or has mismatched dimensions.
#[pyfunction]
#[pyo3(text_signature = "(matrix)")]
fn cholesky_decomposition(py: Python<'_>, matrix: Vec<Vec<f64>>) -> PyResult<Vec<Vec<f64>>> {
    let (flat, n) = flatten_matrix(matrix)?;
    let result = py
        .detach(|| linalg::cholesky_decomposition(&flat, n))
        .map_err(cholesky_err)?;
    Ok(unflatten_matrix(result, n))
}

/// Solve a symmetric positive-definite linear system A x = b given the Cholesky
/// factor L of A (where A = L L^T).
///
/// Accepts L as `list[list[float]]` and b as `list[float]`. Returns x as `list[float]`.
///
/// Raises ``CholeskyError`` on dimension mismatch or singular factor.
#[pyfunction]
#[pyo3(text_signature = "(chol, b)")]
fn cholesky_solve(py: Python<'_>, chol: Vec<Vec<f64>>, b: Vec<f64>) -> PyResult<Vec<f64>> {
    let (flat, n) = flatten_matrix(chol)?;
    if b.len() != n {
        return Err(crate::errors::value_error(format!(
            "Right-hand side has length {} but Cholesky factor is {n}x{n}",
            b.len()
        )));
    }
    let mut x = vec![0.0; n];
    py.detach(|| linalg::cholesky_solve(&flat, &b, &mut x))
        .map_err(cholesky_err)?;
    Ok(x)
}

/// Validate that a matrix is a valid correlation matrix.
///
/// Checks diagonal elements are 1, off-diagonal entries are in [-1, 1],
/// symmetry, and positive semi-definiteness.
///
/// Raises ``CholeskyError`` if any check fails.
#[pyfunction]
#[pyo3(text_signature = "(matrix)")]
fn validate_correlation_matrix(py: Python<'_>, matrix: Vec<Vec<f64>>) -> PyResult<()> {
    let (flat, n) = flatten_matrix(matrix)?;
    py.detach(|| linalg::validate_correlation_matrix(&flat, n))
        .map_err(|e| CholeskyError::new_err(e.to_string()))
}

// ---------------------------------------------------------------------------
// Register
// ---------------------------------------------------------------------------

/// Build the `finstack_quant.core.math.linalg` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "linalg")?;
    m.setattr(
        "__doc__",
        "Linear algebra utilities: Cholesky decomposition, correlation matrices.",
    )?;

    m.add_function(wrap_pyfunction!(cholesky_decomposition, &m)?)?;
    m.add_function(wrap_pyfunction!(cholesky_solve, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_correlation_matrix, &m)?)?;

    m.add("CholeskyError", py.get_type::<CholeskyError>())?;

    m.add("SINGULAR_THRESHOLD", linalg::SINGULAR_THRESHOLD)?;
    m.add("DIAGONAL_TOLERANCE", linalg::DIAGONAL_TOLERANCE)?;
    m.add("SYMMETRY_TOLERANCE", linalg::SYMMETRY_TOLERANCE)?;

    let all = PyList::new(
        py,
        [
            "cholesky_decomposition",
            "cholesky_solve",
            "validate_correlation_matrix",
            "CholeskyError",
            "SINGULAR_THRESHOLD",
            "DIAGONAL_TOLERANCE",
            "SYMMETRY_TOLERANCE",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "linalg",
        "finstack_quant.core.math",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
