"""
Linear algebra utilities: Cholesky decomposition and triangular solves.

Correlation-matrix validation lives in
:func:`finstack_quant.models.correlation.validate_correlation_matrix`.

Matrix inputs accept nested ``list[list[float]]`` or two-dimensional
``numpy.ndarray`` (``float64``) arrays. NumPy inputs use logical row and
column order for C-contiguous, Fortran-contiguous, transposed and strided
layouts; vectors are ``list[float]``.

Examples
--------
>>> from finstack_quant.core.math import linalg
>>> linalg.apply_lower_triangular([[2.0, 0.0], [1.0, 3.0]], [1.0, 2.0])
[2.0, 7.0]

"""

from __future__ import annotations

from collections.abc import Sequence

import numpy as np
import numpy.typing as npt

from finstack_quant.core import FinstackError

__all__ = [
    "CholeskyError",
    "DIAGONAL_TOLERANCE",
    "SINGULAR_THRESHOLD",
    "SYMMETRY_TOLERANCE",
    "apply_lower_triangular",
    "cholesky_decomposition",
    "cholesky_solve",
    "ledoit_wolf_shrinkage",
    "symmetric_eigen",
]

SINGULAR_THRESHOLD: float
"""Relative factor-diagonal cutoff for numerical singularity.

Decomposition scales this fraction by the square root of the largest
input diagonal magnitude; substitution scales it by the largest factor
diagonal magnitude.
"""

DIAGONAL_TOLERANCE: float
"""Tolerance for diagonal element checks in correlation matrices."""

SYMMETRY_TOLERANCE: float
"""Tolerance for symmetry checks in correlation matrices."""

class CholeskyError(FinstackError):
    """
    Cholesky decomposition failure.

    Raised when the input matrix is not positive-definite, is numerically
    singular, or contains non-finite entries. Inherits from ``ValueError``.

    Examples
    --------
    >>> from finstack_quant.core.math import linalg
    >>> str(linalg.CholeskyError("matrix is not positive definite"))
    'matrix is not positive definite'

    """

    ...

def apply_lower_triangular(
    l: list[list[float]] | npt.NDArray[np.float64], z: Sequence[float] | npt.NDArray[np.float64]
) -> list[float]:
    """
    Apply a lower-triangular factor L to a vector z, returning ``L z``.

    This is the Cholesky "apply" step that turns independent standard
    normals into correlated normals: if ``A = L L^T`` and ``z ~ N(0, I)``,
    then ``L z ~ N(0, A)``.

    Parameters
    ----------
    l : list[list[float]] or numpy.ndarray
        Square lower-triangular factor L, typically the output of
        ``cholesky_decomposition``. Only the lower triangle is read; the
        upper triangle is assumed zero and ignored.
    z : list[float]
        Vector to transform, of the same length as L's dimension,
        typically independent standard-normal draws.

    Returns
    -------
    list[float]
        The product ``L z``, in the same variable order as ``z``.

    Raises
    ------
    ValueError
        If ``l`` is not a square nested list / 2-D array, or ``z``'s
        length does not match L's dimension (core dimension-mismatch
        errors map to plain ``ValueError``, not ``CholeskyError``).

    Examples
    --------
    >>> from finstack_quant.core.math import linalg
    >>> linalg.apply_lower_triangular([[2.0, 0.0], [1.0, 3.0]], [1.0, 2.0])
    [2.0, 7.0]

    """
    ...

def cholesky_decomposition(
    matrix: list[list[float]] | npt.NDArray[np.float64],
) -> list[list[float]]:
    """
    Compute the Cholesky decomposition L of a symmetric positive-definite
    matrix such that A = L L^T.

    Rust normalizes the matrix before factorization and restores the
    factor's units afterwards. Numerical singularity is measured relative
    to the square root of the largest input diagonal magnitude. Uniformly
    scaling A by a positive constant scales L by its square root.

    Parameters
    ----------
    matrix : list[list[float]] or numpy.ndarray
        Finite symmetric positive-definite square matrix in logical row
        and column order; NumPy strides do not change that ordering.

    Returns
    -------
    list[list[float]]
        Lower-triangular factor L in the original variable order, with its
        scale restored so that A = L L^T.

    Raises
    ------
    CholeskyError
        If the matrix is not positive-definite, is numerically singular,
        or contains non-finite entries. This exception is a ``ValueError``
        subclass.
    ValueError
        If the input is not a square matrix.

    Examples
    --------
    >>> from finstack_quant.core.math import linalg
    >>> [[round(value, 6) for value in row] for row in linalg.cholesky_decomposition([[4.0, 2.0], [2.0, 3.0]])]
    [[2.0, 0.0], [1.0, 1.414214]]

    """
    ...

def cholesky_solve(
    chol: list[list[float]] | npt.NDArray[np.float64], b: Sequence[float] | npt.NDArray[np.float64]
) -> list[float]:
    """
    Solve a symmetric positive-definite linear system A x = b given
    the Cholesky factor L of A (where A = L L^T).

    The singularity threshold is relative to the largest factor diagonal.
    Uniformly scaling A and b preserves x when L is scaled by the square
    root of the same positive constant.

    Parameters
    ----------
    chol : list[list[float]] or numpy.ndarray
        Square Cholesky factor L; its consumed lower triangle must be
        finite. The unused upper triangle is ignored. NumPy inputs use
        logical row and column order regardless of strides.
    b : Sequence[float] or numpy.ndarray
        Finite right-hand side vector with one value per factor row.

    Returns
    -------
    list[float]
        Solution vector x.

    Raises
    ------
    ValueError
        On dimension mismatch, a non-square factor, a diagonal too close
        to zero relative to the largest diagonal magnitude, non-finite
        lower-triangle or right-hand-side entries, or a non-finite
        substitution result (including overflow). ``CholeskyError`` is
        reserved for ``cholesky_decomposition``.
    TypeError
        If ``b`` contains values that cannot be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import linalg
    >>> [round(value, 6) for value in linalg.cholesky_solve([[2.0, 0.0], [1.0, 2**0.5]], [6.0, 5.0])]
    [1.0, 1.0]

    """
    ...

def symmetric_eigen(
    matrix: list[list[float]] | npt.NDArray[np.float64],
) -> tuple[list[float], list[list[float]]]:
    """
    Symmetric eigendecomposition of a square matrix.

    Parameters
    ----------
    matrix : list[list[float]] or numpy.ndarray
        Symmetric square matrix (only symmetric input is meaningful; no
        symmetry check is performed).

    Returns
    -------
    tuple[list[float], list[list[float]]]
        ``(eigenvalues, eigenvectors)`` where ``eigenvectors[i][k]`` is
        the ``i``-th component of the ``k``-th eigenvector (eigenvectors
        are the columns). Eigenvalues are not sorted.

    Raises
    ------
    CholeskyError
        If the matrix contains non-finite entries.
    ValueError
        If the input is not a square nested list / 2-D array.

    Examples
    --------
    >>> from finstack_quant.core.math import linalg
    >>> values, _ = linalg.symmetric_eigen([[2.0, 0.0], [0.0, 5.0]])
    >>> sorted(round(v, 10) for v in values)
    [2.0, 5.0]

    """
    ...

def ledoit_wolf_shrinkage(
    observations: list[list[float]] | npt.NDArray[np.float64],
) -> tuple[list[list[float]], float]:
    """
    Ledoit-Wolf (2004) shrinkage of a sample covariance toward a scaled identity.

    Columns are demeaned and the sample covariance uses the divisor ``t``.
    Centered observations are normalized before computing second and
    fourth moments, then covariance units are restored. Uniformly rescaling
    all observations preserves the shrinkage intensity and rescales
    covariance by the square of that factor.

    Parameters
    ----------
    observations : list[list[float]] or numpy.ndarray
        ``t x n`` matrix: ``t`` observations (rows) of ``n`` variables
        (columns), ``t >= 2``, ``n >= 1``; every entry must be finite. Use
        comparable units across columns because the target pools their
        variances.

    Returns
    -------
    tuple[list[list[float]], float]
        ``(covariance, shrinkage)``: the ``n x n`` shrunk covariance
        ``delta * mu * I + (1 - delta) * S`` and the optimal intensity
        ``delta`` in ``[0, 1]``. The covariance is unannualized and restored
        to squared observation units. Constant observations produce a zero
        covariance matrix and zero intensity.

    Raises
    ------
    ValueError
        If ``t < 2``, ``n == 0``, rows are ragged, or any entry is
        non-finite; also if centered observations or covariance entries
        exceed the finite ``float64`` range.

    Examples
    --------
    >>> from finstack_quant.core.math import linalg
    >>> cov, delta = linalg.ledoit_wolf_shrinkage([[1.0, 1.0], [-1.0, -1.0], [2.0, -2.0], [-2.0, 2.0]])
    >>> round(delta, 10)
    0.9444444444

    """
    ...
