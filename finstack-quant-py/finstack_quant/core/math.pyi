"""
Numerical helpers: linear algebra, statistics, special functions, summation.

Provides pure-function submodules for numerical computation backed by
``finstack-quant-core`` Rust implementations.

Examples
--------
>>> import finstack_quant.core.math as math
>>> math.__name__
'finstack_quant.core.math'
"""

from __future__ import annotations

__all__ = ["count_consecutive", "linalg", "stats", "special_functions", "summation"]

def count_consecutive(values: list[float]) -> int:
    """
    Count longest consecutive run of strictly positive values.

    Parameters
    ----------
    values:
        Ordered numeric observations; only values strictly greater than zero
        extend a run, while zero and negative values reset it.

    Returns
    -------
    int
        Longest positive run length.

    Examples
    --------
    >>> from finstack_quant.core.math import count_consecutive
    >>> count_consecutive([1.0, 2.0, -1.0, 3.0])
    2

    Raises
    ------
    ValueError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
    """
    ...

class linalg:
    """
    Linear algebra utilities: Cholesky decomposition, correlation matrices.

    All functions in this submodule operate on nested ``list[list[float]]``
    matrices (row-major square matrices) and ``list[float]`` vectors.

    Examples
    --------
    >>> from finstack_quant.core.math import linalg
    >>> linalg.__name__
    'linalg'
    """

    SINGULAR_THRESHOLD: float
    """Threshold below which a diagonal element is considered singular."""

    DIAGONAL_TOLERANCE: float
    """Tolerance for diagonal element checks in correlation matrices."""

    SYMMETRY_TOLERANCE: float
    """Tolerance for symmetry checks in correlation matrices."""

    class CholeskyError(ValueError):
        """
        Cholesky decomposition failure.

        Raised when the input matrix is not positive-definite, is singular,
        or has mismatched dimensions. Inherits from ``ValueError``.

        Examples
        --------
        >>> import finstack_quant.core.math as binding
        >>> binding.linalg.CholeskyError.__name__
        'CholeskyError'
        """

        ...

    @staticmethod
    def apply_lower_triangular(l: list[list[float]], z: list[float]) -> list[float]:
        """
        Apply a lower-triangular factor L to a vector z, returning ``L z``.

        This is the Cholesky "apply" step that turns independent standard
        normals into correlated normals: if ``A = L L^T`` and ``z ~ N(0, I)``,
        then ``L z ~ N(0, A)``.

        Parameters
        ----------
        l : list[list[float]]
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
        CholeskyError
            If ``z``'s length does not match L's dimension.
        ValueError
            If the input is not a square matrix.

        Examples
        --------
        >>> from finstack_quant.core.math import linalg
        >>> correlated = linalg.apply_lower_triangular(L, [1.0, 0.0])  # doctest: +SKIP
        """
        ...

    @staticmethod
    def cholesky_decomposition(matrix: list[list[float]]) -> list[list[float]]:
        """
        Compute the Cholesky decomposition L of a symmetric positive-definite
        matrix such that A = L L^T.

        Parameters
        ----------
        matrix : list[list[float]]
            Square symmetric positive-definite matrix.

        Returns
        -------
        list[list[float]]
            Lower-triangular Cholesky factor L.

        Raises
        ------
        CholeskyError
            If the matrix is not positive-definite, is singular, or has
            mismatched dimensions.
        ValueError
            If the input is not a square matrix.

        Examples
        --------
        >>> from finstack_quant.core.math import linalg
        >>> L = linalg.cholesky_decomposition([[1.0, 0.5], [0.5, 1.0]])  # doctest: +SKIP
        """
        ...

    @staticmethod
    def cholesky_solve(chol: list[list[float]], b: list[float]) -> list[float]:
        """
        Solve a symmetric positive-definite linear system A x = b given
        the Cholesky factor L of A (where A = L L^T).

        Parameters
        ----------
        chol : list[list[float]]
            Lower-triangular Cholesky factor L.
        b : list[float]
            Right-hand side vector.

        Returns
        -------
        list[float]
            Solution vector x.

        Raises
        ------
        CholeskyError
            On dimension mismatch or singular factor.
        ValueError
            If dimensions are inconsistent.

        Examples
        --------
        >>> from finstack_quant.core.math import linalg
        >>> x = linalg.cholesky_solve(L, [1.0, 2.0])  # doctest: +SKIP
        """
        ...

    @staticmethod
    def validate_correlation_matrix(matrix: list[list[float]]) -> None:
        """
        Validate that a matrix is a valid correlation matrix.

        Checks: diagonal elements are 1, off-diagonal entries are in
        ``[-1, 1]``, symmetry, and positive semi-definiteness.

        Parameters
        ----------
        matrix : list[list[float]]
            Square matrix to validate.

        Raises
        ------
        CholeskyError
            If any validation check fails.
        ValueError
            If the input is not a square matrix.

        Examples
        --------
        >>> from finstack_quant.core.math import linalg
        >>> linalg.validate_correlation_matrix([[1.0, 0.5], [0.5, 1.0]])  # doctest: +SKIP
        """
        ...

class stats:
    """
    Statistical functions: mean, variance, correlation, covariance, quantile.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> stats.__name__
    'stats'
    """

    @staticmethod
    def mean(data: list[float]) -> float:
        """
        Arithmetic mean of a data series.

        Returns ``0.0`` for an empty list.

        Parameters
        ----------
        data : list[float]
            Input data.

        Returns
        -------
        float
            Arithmetic mean.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> stats.mean([1.0, 2.0, 3.0])
        2.0

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @staticmethod
    def variance(data: list[float]) -> float:
        """
        Sample variance (unbiased, n-1 denominator).

        Returns ``0.0`` for fewer than 2 observations.

        Parameters
        ----------
        data : list[float]
            Input data.

        Returns
        -------
        float
            Sample variance.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> round(stats.variance([1.0, 2.0, 3.0]), 10)
        1.0

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @staticmethod
    def population_variance(data: list[float]) -> float:
        """
        Population variance (n denominator).

        Returns ``0.0`` for an empty list.

        Parameters
        ----------
        data : list[float]
            Input data.

        Returns
        -------
        float
            Population variance.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> round(stats.population_variance([1.0, 2.0, 3.0]), 10)
        0.6666666667

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @staticmethod
    def correlation(x: list[float], y: list[float]) -> float:
        """
        Pearson correlation coefficient between two equal-length series.

        Returns ``NaN`` if the input lengths differ.

        Parameters
        ----------
        x : list[float]
            First data series.
        y : list[float]
            Second data series.

        Returns
        -------
        float
            Correlation in ``[-1, 1]``, or ``NaN`` on error.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> round(stats.correlation([1.0, 2.0, 3.0], [2.0, 4.0, 6.0]), 10)
        1.0

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @staticmethod
    def covariance(x: list[float], y: list[float]) -> float:
        """
        Sample covariance (unbiased, n-1 denominator).

        Returns ``NaN`` if the input lengths differ.

        Parameters
        ----------
        x : list[float]
            First data series.
        y : list[float]
            Second data series.

        Returns
        -------
        float
            Sample covariance.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> round(stats.covariance([1.0, 2.0, 3.0], [2.0, 4.0, 6.0]), 10)
        2.0

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @staticmethod
    def quantile(data: list[float], q: float) -> float:
        """
        Empirical quantile (R-7 / NumPy default) with linear interpolation.

        Returns ``NaN`` for empty data, *q* outside ``[0, 1]``, or
        non-finite inputs.

        Parameters
        ----------
        data : list[float]
            Input data (will be sorted internally).
        q : float
            Quantile in ``[0, 1]``.

        Returns
        -------
        float
            Quantile value.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> stats.quantile([1.0, 2.0, 3.0, 4.0, 5.0], 0.5)
        3.0

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

class special_functions:
    """Special mathematical functions: normal distribution, error function, gamma.

    Examples
    --------
    >>> from finstack_quant.core.math import special_functions
    >>> round(special_functions.norm_cdf(0.0), 10)
    0.5
    """

    @staticmethod
    def norm_cdf(x: float) -> float:
        r"""Standard normal cumulative distribution function :math:`\Phi(x)`.

        Returns :math:`P(Z \le x)` where :math:`Z \sim N(0, 1)`.

        Parameters
        ----------
        x : float
            Input value.

        Returns
        -------
        float
            Probability in ``[0, 1]``.

        Raises
        ------
        None
            This pure function returns a floating-point result for every ``float`` input.

        Examples
        --------
        >>> from finstack_quant.core.math import special_functions
        >>> round(special_functions.norm_cdf(0.0), 10)
        0.5
        """
        ...

    @staticmethod
    def norm_pdf(x: float) -> float:
        r"""Standard normal probability density function :math:`\varphi(x)`.

        Returns :math:`\frac{1}{\sqrt{2\pi}} \exp(-x^2/2)`.

        Parameters
        ----------
        x : float
            Input value.

        Returns
        -------
        float
            Density value.

        Raises
        ------
        None
            This pure function returns a floating-point result for every ``float`` input.

        Examples
        --------
        >>> from finstack_quant.core.math import special_functions
        >>> round(special_functions.norm_pdf(0.0), 10)
        0.3989422804
        """
        ...

    @staticmethod
    def standard_normal_inv_cdf(p: float) -> float:
        r"""Inverse standard normal CDF :math:`\Phi^{-1}(p)`.

        Returns *x* such that :math:`\Phi(x) = p`.

        Parameters
        ----------
        p : float
            Probability in ``(0, 1)``.

        Returns
        -------
        float
            Quantile *x* such that ``Phi(x) = p``.

        Raises
        ------
        None
            Out-of-range probabilities map to signed infinity; NaN propagates to the result.

        Examples
        --------
        >>> from finstack_quant.core.math import special_functions
        >>> round(special_functions.standard_normal_inv_cdf(0.5), 10)
        0.0
        """
        ...

    @staticmethod
    def erf(x: float) -> float:
        r"""Error function :math:`\mathrm{erf}(x) = \frac{2}{\sqrt{\pi}} \int_0^x e^{-t^2} dt`.

        Parameters
        ----------
        x : float
            Input value.

        Returns
        -------
        float
            Value in ``[-1, 1]``.

        Raises
        ------
        None
            This pure function delegates to the error-function approximation without validation.

        Examples
        --------
        >>> from finstack_quant.core.math import special_functions
        >>> round(special_functions.erf(1.0), 10)
        0.8427007929
        """
        ...

    @staticmethod
    def ln_gamma(x: float) -> float:
        r"""Natural logarithm of the Gamma function :math:`\ln(\Gamma(x))`.

        Returns ``float('inf')`` for :math:`x \le 0`.

        Parameters
        ----------
        x : float
            Input value.

        Returns
        -------
        float
            Natural logarithm of the gamma function; returns positive infinity for ``x <= 0``.

        Raises
        ------
        None
            This pure function returns a floating-point result for every ``float`` input.

        Examples
        --------
        >>> from finstack_quant.core.math import special_functions
        >>> round(special_functions.ln_gamma(1.0), 10)
        0.0
        """
        ...

    @staticmethod
    def student_t_cdf(x: float, df: float) -> float:
        r"""Student-t cumulative distribution function.

        Returns :math:`P(T \le x)` where :math:`T \sim t(\nu)`.

        Parameters
        ----------
        x : float
            Input value.
        df : float
            Degrees of freedom (:math:`\nu > 0`).

        Returns
        -------
        float
            Probability in ``[0, 1]``.

        Raises
        ------
        ValueError
            If ``df`` is non-finite or not positive.

        Examples
        --------
        >>> from finstack_quant.core.math import special_functions
        >>> round(special_functions.student_t_cdf(0.0, 10.0), 10)
        0.5
        """
        ...

    @staticmethod
    def student_t_inv_cdf(p: float, df: float) -> float:
        r"""Inverse Student-t CDF (quantile function).

        Returns *x* such that :math:`P(T \le x) = p` where :math:`T \sim t(\nu)`.

        Parameters
        ----------
        p : float
            Probability in ``(0, 1)``.
        df : float
            Degrees of freedom (:math:`\nu > 0`).

        Returns
        -------
        float
            Quantile *x*.

        Raises
        ------
        ValueError
            If ``df`` is non-finite or not positive. Probabilities at or beyond
            the domain edges saturate to negative or positive infinity.

        Examples
        --------
        >>> from finstack_quant.core.math import special_functions
        >>> round(special_functions.student_t_inv_cdf(0.5, 10.0), 10)
        0.0
        """
        ...

class summation:
    """
    Numerically stable summation: Kahan and Neumaier compensated sums.

    Examples
    --------
    >>> from finstack_quant.core.math import summation
    >>> summation.__name__
    'summation'
    """

    @staticmethod
    def kahan_sum(values: list[float]) -> float:
        """
        Kahan compensated summation -- reduces floating-point rounding errors.

        Best for sequences where all values have the same sign. For
        mixed-sign values, prefer :func:`neumaier_sum`.

        Parameters
        ----------
        values : list[float]
            Values to sum.

        Returns
        -------
        float
            Compensated sum.

        Examples
        --------
        >>> from finstack_quant.core.math import summation
        >>> summation.kahan_sum([1.0, 2.0, 3.0])
        6.0

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @staticmethod
    def neumaier_sum(values: list[float]) -> float:
        """
        Neumaier compensated summation -- handles mixed-sign values
        better than Kahan.

        Recommended for financial calculations with mixed-sign cashflows.

        Parameters
        ----------
        values : list[float]
            Values to sum.

        Returns
        -------
        float
            Compensated sum.

        Examples
        --------
        >>> from finstack_quant.core.math import summation
        >>> summation.neumaier_sum([1.0, -2.0, 3.0])
        2.0

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...
