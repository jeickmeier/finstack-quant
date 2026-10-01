//! Python bindings for `finstack_quant_core::math::linalg`.

use finstack_quant_core::math::linalg;
use numpy::{PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use crate::errors::core_to_py;

pyo3::create_exception!(
    finstack_quant.core.math.linalg,
    CholeskyError,
    crate::errors::FinstackError,
    "Cholesky decomposition failure (inherits FinstackError, ValueError)."
);

/// Map a core linear-algebra error to a Python `CholeskyError` exception.
fn cholesky_err(e: impl std::fmt::Display) -> PyErr {
    CholeskyError::new_err(e.to_string())
}

/// Extract a matrix in logical row-major order regardless of NumPy strides.
fn extract_matrix(matrix: &Bound<'_, PyAny>) -> PyResult<(Vec<f64>, usize, usize)> {
    if let Ok(array) = matrix.extract::<PyReadonlyArray2<'_, f64>>() {
        let shape = array.shape();
        let (rows, cols) = (shape[0], shape[1]);
        let flat = match array.as_slice() {
            Ok(slice) if array.is_c_contiguous() => slice.to_vec(),
            // Fortran-contiguous slices expose column-major storage; strided
            // views also need logical-order iteration before calling Rust.
            _ => array.as_array().iter().copied().collect(),
        };
        return Ok((flat, rows, cols));
    }
    let rows: Vec<Vec<f64>> = matrix
        .extract()
        .map_err(|_| crate::errors::value_error("expected a square nested list or 2-D array"))?;
    let nrows = rows.len();
    let ncols = rows.first().map_or(0, Vec::len);
    for (i, row) in rows.iter().enumerate() {
        if row.len() != ncols {
            return Err(crate::errors::value_error(format!(
                "Row {i} has length {} but expected {ncols} (ragged matrix)",
                row.len()
            )));
        }
    }
    Ok((rows.into_iter().flatten().collect(), nrows, ncols))
}

/// Extract an n×n matrix from a NumPy array or a nested list.
fn extract_square_matrix(matrix: &Bound<'_, PyAny>) -> PyResult<(Vec<f64>, usize)> {
    let (flat, rows, cols) = extract_matrix(matrix)?;
    if rows != cols {
        return Err(crate::errors::value_error(format!(
            "Matrix must be square, got {rows}x{cols}; expected a square nested list or 2-D array"
        )));
    }
    Ok((flat, rows))
}

/// Compute the Cholesky decomposition L of a symmetric positive-definite matrix
/// such that A = L L^T.
///
/// Accepts a square matrix as a ``numpy.ndarray`` (``float64``) or
/// ``list[list[float]]`` and returns the lower-triangular factor in the same
/// shape. Rust normalizes the matrix before factorization and restores the
/// factor's units afterwards. Numerical singularity is measured relative to
/// the square root of the largest input diagonal magnitude: uniformly scaling
/// A by a positive constant scales L by its square root.
///
/// # Arguments
///
/// * `matrix` - Finite symmetric positive-definite square matrix in logical row and column order; NumPy strides do not change that ordering.
///
/// # Returns
///
/// Lower-triangular factor L as a nested list, satisfying A = L L^T in the
/// original variable order, with the factor's scale restored.
///
/// # Errors
///
/// Raises ``CholeskyError`` (a ``ValueError`` subclass) when the matrix is not
/// positive-definite, is numerically singular, or contains non-finite entries.
/// Raises ``ValueError`` when the input cannot be read as a square matrix.
#[pyfunction]
#[pyo3(text_signature = "(matrix)")]
fn cholesky_decomposition(py: Python<'_>, matrix: &Bound<'_, PyAny>) -> PyResult<Vec<Vec<f64>>> {
    let (flat, n) = extract_square_matrix(matrix)?;
    let result = py
        .detach(|| linalg::cholesky_decomposition(&flat, n))
        .map_err(cholesky_err)?;
    Ok(linalg::unflatten_square(&result, n))
}

/// Solve a symmetric positive-definite linear system A x = b given the Cholesky
/// factor L of A (where A = L L^T).
///
/// Returns x as ``list[float]``. The unused upper triangle of L is ignored.
/// The singularity threshold is relative to the largest factor diagonal;
/// uniformly scaling A and b preserves x when L is scaled by the square root
/// of the same positive constant.
///
/// # Arguments
///
/// * `chol` - Square lower-triangular Cholesky factor L as a nested list or float64 NumPy array; the consumed lower triangle must be finite.
/// * `b` - Finite right-hand side sequence with one value per factor row.
///
/// # Errors
///
/// Raises ``ValueError`` for a non-square factor, dimension mismatch, a diagonal
/// too close to zero relative to the largest diagonal magnitude, non-finite
/// entries in the consumed lower triangle or right-hand side, or a non-finite
/// substitution result (including overflow). Raises ``TypeError`` if ``b``
/// contains values that cannot be converted to float.
#[pyfunction]
#[pyo3(text_signature = "(chol, b)")]
fn cholesky_solve(py: Python<'_>, chol: &Bound<'_, PyAny>, b: Vec<f64>) -> PyResult<Vec<f64>> {
    let (flat, n) = extract_square_matrix(chol)?;
    if b.len() != n {
        return Err(crate::errors::value_error(format!(
            "Right-hand side has length {} but Cholesky factor is {n}x{n}",
            b.len()
        )));
    }
    let mut x = vec![0.0; n];
    py.detach(|| linalg::cholesky_solve(&flat, &b, &mut x))
        .map_err(core_to_py)?;
    Ok(x)
}

/// Apply a lower-triangular factor L to a vector z, returning L z.
///
/// This is the Cholesky "apply" step that turns independent standard normals into
/// correlated normals: if A = L L^T and z ~ N(0, I), then L z ~ N(0, A).
///
/// Accepts L as a ``numpy.ndarray`` (``float64``) or ``list[list[float]]``
/// (only the lower triangle is read; the upper triangle is assumed zero) and z
/// as ``list[float]``. Returns L z as ``list[float]``.
///
/// Raises ``ValueError`` when L is not square or z's length does not match L's
/// dimension.
#[pyfunction]
#[pyo3(text_signature = "(l, z)")]
fn apply_lower_triangular(py: Python<'_>, l: &Bound<'_, PyAny>, z: Vec<f64>) -> PyResult<Vec<f64>> {
    let (flat, n) = extract_square_matrix(l)?;
    py.detach(|| linalg::apply_lower_triangular(&flat, n, &z))
        .map_err(core_to_py)
}

/// Symmetric eigendecomposition of a square matrix.
///
/// Returns ``(eigenvalues, eigenvectors)`` where ``eigenvectors[i][k]`` is the
/// ``i``-th component of the ``k``-th eigenvector (eigenvectors are the
/// columns). Eigenvalues are not sorted. Accepts ``list[list[float]]`` or a
/// ``numpy.ndarray``.
///
/// Raises ``CholeskyError`` on a dimension mismatch or non-finite entries.
#[pyfunction]
#[pyo3(text_signature = "(matrix)")]
fn symmetric_eigen(
    py: Python<'_>,
    matrix: &Bound<'_, PyAny>,
) -> PyResult<(Vec<f64>, Vec<Vec<f64>>)> {
    let (flat, n) = extract_square_matrix(matrix)?;
    let (values, vectors) = py
        .detach(|| linalg::symmetric_eigen(&flat, n))
        .map_err(cholesky_err)?;
    Ok((values, linalg::unflatten_square(&vectors, n)))
}

/// Ledoit-Wolf (2004) shrinkage of a sample covariance matrix toward a scaled
/// identity target with the analytic optimal shrinkage intensity.
///
/// ``observations`` is a ``t × n`` matrix (``t`` observations of ``n``
/// variables, one observation per row) as ``list[list[float]]`` or a
/// ``numpy.ndarray``. Returns ``(covariance, shrinkage)``: the ``n × n``
/// shrunk covariance as nested lists and the intensity ``δ* ∈ [0, 1]``.
/// Columns are demeaned and the sample covariance uses the divisor ``t``.
/// Centered observations are normalized before computing second and fourth
/// moments, then covariance units are restored. Uniformly rescaling all
/// observations preserves the shrinkage intensity and rescales covariance by
/// the square of that factor.
///
/// # Arguments
///
/// * `observations` - Finite rectangular observation matrix with ``t >= 2`` rows and ``n >= 1`` columns; use comparable units across columns because the target pools their variances.
///
/// # Returns
///
/// Pair of the unannualized covariance matrix in squared observation units
/// and its dimensionless shrinkage intensity. Constant observations produce
/// a zero covariance matrix and zero intensity.
///
/// # Errors
///
/// Raises ``ValueError`` for an invalid matrix shape, ``t < 2``, ``n == 0``,
/// non-finite entries, or centered observations or covariance entries that
/// exceed the finite ``float64`` range.
#[pyfunction]
#[pyo3(text_signature = "(observations)")]
fn ledoit_wolf_shrinkage(
    py: Python<'_>,
    observations: &Bound<'_, PyAny>,
) -> PyResult<(Vec<Vec<f64>>, f64)> {
    let (flat, t, n) = extract_matrix(observations)?;
    let result = py
        .detach(|| linalg::ledoit_wolf_shrinkage(&flat, t, n))
        .map_err(core_to_py)?;
    Ok((
        linalg::unflatten_square(&result.covariance, n),
        result.shrinkage,
    ))
}

/// Build the `finstack_quant.core.math.linalg` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "linalg")?;
    m.setattr(
        "__doc__",
        "Linear algebra utilities: Cholesky decomposition, triangular solves, symmetric eigendecomposition, Ledoit-Wolf shrinkage.",
    )?;

    m.add_function(wrap_pyfunction!(apply_lower_triangular, &m)?)?;
    m.add_function(wrap_pyfunction!(cholesky_decomposition, &m)?)?;
    m.add_function(wrap_pyfunction!(cholesky_solve, &m)?)?;
    m.add_function(wrap_pyfunction!(ledoit_wolf_shrinkage, &m)?)?;
    m.add_function(wrap_pyfunction!(symmetric_eigen, &m)?)?;

    m.add("CholeskyError", py.get_type::<CholeskyError>())?;

    m.add("SINGULAR_THRESHOLD", linalg::SINGULAR_THRESHOLD)?;
    m.add("DIAGONAL_TOLERANCE", linalg::DIAGONAL_TOLERANCE)?;
    m.add("SYMMETRY_TOLERANCE", linalg::SYMMETRY_TOLERANCE)?;

    let all = PyList::new(
        py,
        [
            "CholeskyError",
            "DIAGONAL_TOLERANCE",
            "SINGULAR_THRESHOLD",
            "SYMMETRY_TOLERANCE",
            "apply_lower_triangular",
            "cholesky_decomposition",
            "cholesky_solve",
            "ledoit_wolf_shrinkage",
            "symmetric_eigen",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    Ok(())
}
