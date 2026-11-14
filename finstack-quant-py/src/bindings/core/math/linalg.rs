//! Python bindings for `finstack_quant_core::math::linalg`.

use finstack_quant_core::math::linalg;
use numpy::{
    PyArrayDescrMethods, PyReadonlyArray1, PyReadonlyArray2, PyUntypedArray, PyUntypedArrayMethods,
};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyFloat, PyInt, PyList, PyModule, PyString};

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

/// Extract a numeric scalar as `f64`, rejecting Python `bool` and NumPy `bool_`.
fn extract_linalg_f64(elem: &Bound<'_, PyAny>, what: &str) -> PyResult<f64> {
    if elem.is_instance_of::<PyBool>()
        || elem
            .get_type()
            .name()
            .is_ok_and(|name| name == "bool" || name == "bool_")
    {
        return Err(PyTypeError::new_err(format!(
            "{what} must be a number, not bool"
        )));
    }
    elem.extract::<f64>()
        .map_err(|_| PyTypeError::new_err(format!("{what} must be a number")))
}

/// Extract a 1-D numeric vector, rejecting `bool` and non-numeric types with `TypeError`.
fn extract_vector(vec_obj: &Bound<'_, PyAny>, label: &str) -> PyResult<Vec<f64>> {
    if let Ok(untyped) = vec_obj.cast::<PyUntypedArray>() {
        let kind = untyped.dtype().kind();
        if !matches!(kind, b'f' | b'i' | b'u') {
            return Err(PyTypeError::new_err(format!(
                "{label} must have a real numeric dtype, not bool or non-numeric"
            )));
        }
        if untyped.ndim() != 1 {
            return Err(crate::errors::value_error(format!(
                "{label} must be a 1-D sequence, got {}-D array",
                untyped.ndim()
            )));
        }
        if let Ok(array) = vec_obj.extract::<PyReadonlyArray1<'_, f64>>() {
            return Ok(array.as_array().iter().copied().collect());
        }
    }
    if vec_obj.is_instance_of::<PyString>() || vec_obj.is_instance_of::<PyBytes>() {
        return Err(PyTypeError::new_err(format!(
            "{label} must be a 1-D sequence of numbers"
        )));
    }
    let iter = vec_obj
        .try_iter()
        .map_err(|_| PyTypeError::new_err(format!("{label} must be a 1-D sequence of numbers")))?;
    let mut out = Vec::new();
    for item in iter {
        let item = item?;
        out.push(extract_linalg_f64(&item, &format!("{label} element"))?);
    }
    Ok(out)
}

/// Extract a matrix in logical row-major order regardless of NumPy strides.
///
/// Shape mismatches (1-D numeric list, wrong array dimensionality, or ragged
/// rows) raise `ValueError`; non-sequence containers, strings, `bool` values,
/// and non-numeric entries raise `TypeError`.
fn extract_matrix(matrix: &Bound<'_, PyAny>) -> PyResult<(Vec<f64>, usize, usize)> {
    if let Ok(untyped) = matrix.cast::<PyUntypedArray>() {
        let kind = untyped.dtype().kind();
        if !matches!(kind, b'f' | b'i' | b'u') {
            return Err(PyTypeError::new_err(
                "matrix elements must have a real numeric dtype, not bool or non-numeric",
            ));
        }
        if untyped.ndim() != 2 {
            return Err(crate::errors::value_error(
                "expected a square nested list or 2-D array",
            ));
        }
        if let Ok(array) = matrix.extract::<PyReadonlyArray2<'_, f64>>() {
            let shape = array.shape();
            let (rows, cols) = (shape[0], shape[1]);
            return Ok((
                crate::bindings::matrix_input::numpy_row_major(&array),
                rows,
                cols,
            ));
        }
    }
    if matrix.is_instance_of::<PyString>() || matrix.is_instance_of::<PyBytes>() {
        return Err(PyTypeError::new_err(
            "expected a 2-D sequence of numbers or NumPy array",
        ));
    }
    let outer = matrix
        .try_iter()
        .map_err(|_| PyTypeError::new_err("expected a 2-D sequence of numbers or NumPy array"))?;
    let mut rows: Vec<Vec<f64>> = Vec::new();
    for (i, row_res) in outer.enumerate() {
        let row_obj = row_res?;
        if row_obj.is_instance_of::<PyString>() || row_obj.is_instance_of::<PyBytes>() {
            return Err(PyTypeError::new_err(
                "expected a 2-D sequence of numbers, got string row",
            ));
        }
        if !row_obj.is_instance_of::<PyBool>()
            && (row_obj.is_instance_of::<PyFloat>() || row_obj.is_instance_of::<PyInt>())
        {
            return Err(crate::errors::value_error(
                "expected a square nested list or 2-D array",
            ));
        }
        let row_iter = row_obj.try_iter().map_err(|_| {
            PyTypeError::new_err("expected a 2-D sequence of numbers or NumPy array")
        })?;
        let mut row = Vec::new();
        for elem_res in row_iter {
            let elem = elem_res?;
            row.push(extract_linalg_f64(&elem, "matrix element")?);
        }
        if let Some(first) = rows.first()
            && row.len() != first.len()
        {
            return Err(crate::errors::value_error(format!(
                "Row {i} has length {} but expected {} (ragged matrix)",
                row.len(),
                first.len()
            )));
        }
        rows.push(row);
    }
    let nrows = rows.len();
    let ncols = rows.first().map_or(0, Vec::len);
    Ok((rows.into_iter().flatten().collect(), nrows, ncols))
}

/// Extract an n×n matrix from a NumPy array or a nested list.
fn square_matrix(matrix: &Bound<'_, PyAny>) -> PyResult<(Vec<f64>, usize)> {
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
/// symmetric, not positive-definite, is numerically singular, or contains
/// non-finite entries. Raises ``ValueError`` when the input cannot be read as a
/// square matrix.
#[pyfunction]
#[pyo3(text_signature = "(matrix)")]
fn cholesky_decomposition(py: Python<'_>, matrix: &Bound<'_, PyAny>) -> PyResult<Vec<Vec<f64>>> {
    let (flat, n) = square_matrix(matrix)?;
    let result = py
        .detach(|| linalg::cholesky_decomposition(&flat, n))
        .map_err(cholesky_err)?;
    linalg::unflatten_square(&result, n).map_err(cholesky_err)
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
fn cholesky_solve(
    py: Python<'_>,
    chol: &Bound<'_, PyAny>,
    b: &Bound<'_, PyAny>,
) -> PyResult<Vec<f64>> {
    let (flat, _) = square_matrix(chol)?;
    let b = extract_vector(b, "b")?;
    let mut x = vec![0.0; b.len()];
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
/// dimension, and ``TypeError`` when inputs contain ``bool`` or non-numeric values.
#[pyfunction]
#[pyo3(text_signature = "(l, z)")]
fn apply_lower_triangular(
    py: Python<'_>,
    l: &Bound<'_, PyAny>,
    z: &Bound<'_, PyAny>,
) -> PyResult<Vec<f64>> {
    let (flat, n) = square_matrix(l)?;
    let z = extract_vector(z, "z")?;
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
/// Raises ``CholeskyError`` on asymmetry, a dimension mismatch, or non-finite entries.
#[pyfunction]
#[pyo3(text_signature = "(matrix)")]
fn symmetric_eigen(
    py: Python<'_>,
    matrix: &Bound<'_, PyAny>,
) -> PyResult<(Vec<f64>, Vec<Vec<f64>>)> {
    let (flat, n) = square_matrix(matrix)?;
    let (values, vectors) = py
        .detach(|| linalg::symmetric_eigen(&flat, n))
        .map_err(cholesky_err)?;
    let vectors = linalg::unflatten_square(&vectors, n).map_err(cholesky_err)?;
    Ok((values, vectors))
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
    let covariance = linalg::unflatten_square(&result.covariance, n).map_err(cholesky_err)?;
    Ok((covariance, result.shrinkage))
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

    let all = PyList::new(
        py,
        [
            "CholeskyError",
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
