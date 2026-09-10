"""
Numerical helpers: linear algebra, statistics, special functions, summation.

Provides pure-function submodules for numerical computation backed by
``finstack-quant-core`` Rust implementations.

Examples
--------
>>> from finstack_quant.core.math import stats
>>> stats.mean([1.0, 2.0, 3.0])
2.0

"""

from __future__ import annotations

from collections.abc import Sequence

import numpy as np
import numpy.typing as npt

from finstack_quant.core import FinstackError

__all__ = ["linalg", "longest_positive_run", "special_functions", "stats", "summation"]

def longest_positive_run(values: Sequence[float] | npt.NDArray[np.float64]) -> int:
    """
    Length of the longest run of strictly positive values.

    Parameters
    ----------
    values:
        Ordered numeric observations; only values strictly greater than zero
        extend a run, while zero and negative values reset it.

    Returns
    -------
    int
        Longest positive run length.

    Notes
    -----
    This helper does not raise; an empty series yields ``0``.

    Examples
    --------
    >>> from finstack_quant.core.math import longest_positive_run
    >>> longest_positive_run([1.0, 2.0, -1.0, 3.0])
    2

    """
    ...

class linalg:
    """
    Linear algebra utilities: Cholesky decomposition and triangular solves.

    Correlation-matrix validation lives in
    :func:`finstack_quant.models.correlation.validate_correlation_matrix`.

    Matrix inputs accept either nested ``list[list[float]]`` (row-major
    square matrices) or C-contiguous ``numpy.ndarray`` (``float64``) arrays;
    vectors are ``list[float]``.

    Examples
    --------
    >>> from finstack_quant.core.math import linalg
    >>> linalg.apply_lower_triangular([[2.0, 0.0], [1.0, 3.0]], [1.0, 2.0])
    [2.0, 7.0]

    """

    SINGULAR_THRESHOLD: float
    """Threshold below which a diagonal element is considered singular."""

    DIAGONAL_TOLERANCE: float
    """Tolerance for diagonal element checks in correlation matrices."""

    SYMMETRY_TOLERANCE: float
    """Tolerance for symmetry checks in correlation matrices."""

    class CholeskyError(FinstackError):
        """
        Cholesky decomposition failure.

        Raised when the input matrix is not positive-definite, is singular,
        or has mismatched dimensions. Inherits from ``ValueError``.

        Examples
        --------
        >>> from finstack_quant.core.math import linalg
        >>> str(linalg.CholeskyError("matrix is not positive definite"))
        'matrix is not positive definite'

        """

        ...

    @staticmethod
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

    @staticmethod
    def cholesky_decomposition(
        matrix: list[list[float]] | npt.NDArray[np.float64],
    ) -> list[list[float]]:
        """
        Compute the Cholesky decomposition L of a symmetric positive-definite
        matrix such that A = L L^T.

        Parameters
        ----------
        matrix : list[list[float]] or numpy.ndarray
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
        >>> [[round(value, 6) for value in row] for row in linalg.cholesky_decomposition([[4.0, 2.0], [2.0, 3.0]])]
        [[2.0, 0.0], [1.0, 1.414214]]

        """
        ...

    @staticmethod
    def cholesky_solve(
        chol: list[list[float]] | npt.NDArray[np.float64], b: Sequence[float] | npt.NDArray[np.float64]
    ) -> list[float]:
        """
        Solve a symmetric positive-definite linear system A x = b given
        the Cholesky factor L of A (where A = L L^T).

        Parameters
        ----------
        chol : list[list[float]] or numpy.ndarray
            Lower-triangular Cholesky factor L.
        b : list[float]
            Right-hand side vector.

        Returns
        -------
        list[float]
            Solution vector x.

        Raises
        ------
        ValueError
            On dimension mismatch, a non-square factor, or a singular
            (near-zero diagonal) factor. ``CholeskyError`` is reserved for
            ``cholesky_decomposition``.

        Examples
        --------
        >>> from finstack_quant.core.math import linalg
        >>> [round(value, 6) for value in linalg.cholesky_solve([[2.0, 0.0], [1.0, 2**0.5]], [6.0, 5.0])]
        [1.0, 1.0]

        """
        ...

    @staticmethod
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

    @staticmethod
    def ledoit_wolf_shrinkage(
        observations: list[list[float]] | npt.NDArray[np.float64],
    ) -> tuple[list[list[float]], float]:
        """
        Ledoit-Wolf (2004) shrinkage of a sample covariance toward a scaled identity.

        Parameters
        ----------
        observations : list[list[float]] or numpy.ndarray
            ``t x n`` matrix: ``t`` observations (rows) of ``n`` variables
            (columns), ``t >= 2``, ``n >= 1``.

        Returns
        -------
        tuple[list[list[float]], float]
            ``(covariance, shrinkage)``: the ``n x n`` shrunk covariance
            ``delta * mu * I + (1 - delta) * S`` and the optimal intensity
            ``delta`` in ``[0, 1]``.

        Raises
        ------
        ValueError
            If ``t < 2``, ``n == 0``, rows are ragged, or any entry is
            non-finite.

        Examples
        --------
        >>> from finstack_quant.core.math import linalg
        >>> cov, delta = linalg.ledoit_wolf_shrinkage([[1.0, 1.0], [-1.0, -1.0], [2.0, -2.0], [-2.0, 2.0]])
        >>> round(delta, 10)
        0.9444444444

        """
        ...

class stats:
    """
    Statistical functions: mean, variance, correlation, covariance, quantiles,
    NaN-sentinel summaries, log returns and realized variance.

    Vector parameters accept any ``Sequence[float]`` or a 1-D ``float64``
    NumPy array.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> (stats.mean([1.0, 2.0, 3.0]), stats.quantile([1.0, 2.0, 3.0], 0.5))
    (2.0, 2.0)

    """

    @staticmethod
    def mean_var(data: Sequence[float] | npt.NDArray[np.float64]) -> tuple[float, float]:
        """
        ``(mean, sample_variance)`` in a single Welford pass.

        Parameters
        ----------
        data : Sequence[float] or numpy.ndarray
            Observations; the variance uses the n-1 denominator.

        Returns
        -------
        tuple[float, float]
            ``(mean, variance)``; ``(0.0, 0.0)`` for empty input.

        Notes
        -----
        This function does not raise.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> stats.mean_var([1.0, 2.0, 3.0])
        (2.0, 1.0)

        """
        ...

    @staticmethod
    def mean_or_nan(data: Sequence[float] | npt.NDArray[np.float64]) -> float:
        """
        Arithmetic mean, or ``nan`` for empty input.

        Parameters
        ----------
        data : Sequence[float] or numpy.ndarray
            Observations.

        Returns
        -------
        float
            Mean, or ``nan`` when *data* is empty.

        Notes
        -----
        This function does not raise.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> stats.mean_or_nan([1.0, 3.0])
        2.0

        """
        ...

    @staticmethod
    def sample_variance_or_nan(data: Sequence[float] | npt.NDArray[np.float64]) -> float:
        """
        Sample variance (n-1 denominator), or ``nan`` for fewer than 2 observations.

        Parameters
        ----------
        data : Sequence[float] or numpy.ndarray
            Observations.

        Returns
        -------
        float
            Unbiased variance, or ``nan``.

        Notes
        -----
        This function does not raise.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> stats.sample_variance_or_nan([1.0, 2.0, 3.0])
        1.0

        """
        ...

    @staticmethod
    def sample_std_or_nan(data: Sequence[float] | npt.NDArray[np.float64]) -> float:
        """
        Sample standard deviation (n-1 denominator), or ``nan`` for fewer than 2 observations.

        Parameters
        ----------
        data : Sequence[float] or numpy.ndarray
            Observations.

        Returns
        -------
        float
            Square root of the unbiased variance, or ``nan``.

        Notes
        -----
        This function does not raise.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> stats.sample_std_or_nan([1.0, 2.0, 3.0])
        1.0

        """
        ...

    @staticmethod
    def median_or_nan(data: Sequence[float] | npt.NDArray[np.float64]) -> float:
        """
        Median (mean of the two middle values for even counts), or ``nan`` for empty input.

        Parameters
        ----------
        data : Sequence[float] or numpy.ndarray
            Observations (sorted internally).

        Returns
        -------
        float
            Median, or ``nan``.

        Notes
        -----
        This function does not raise.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> stats.median_or_nan([3.0, 1.0, 2.0, 4.0])
        2.5

        """
        ...

    @staticmethod
    def quantile_linear_or_nan(data: Sequence[float] | npt.NDArray[np.float64], q: float) -> float:
        """
        Linearly interpolated quantile (R-7), or ``nan`` when undefined.

        Parameters
        ----------
        data : Sequence[float] or numpy.ndarray
            Finite observations (sorted internally); empty or non-finite data returns ``nan``.
        q : float
            Quantile probability, clamped to ``[0, 1]``; NaN returns ``nan``.

        Returns
        -------
        float
            Quantile value, or ``nan`` for empty/non-finite data or a NaN probability.

        Notes
        -----
        This function does not raise.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> stats.quantile_linear_or_nan([1.0, 2.0, 3.0, 4.0], 0.5)
        2.5

        """
        ...

    @staticmethod
    def finite_min_or_nan(data: Sequence[float] | npt.NDArray[np.float64]) -> float:
        """
        Minimum over finite values, or ``nan`` when there are none.

        Parameters
        ----------
        data : Sequence[float] or numpy.ndarray
            Observations; NaN and infinities are ignored.

        Returns
        -------
        float
            Smallest finite value, or ``nan``.

        Notes
        -----
        This function does not raise.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> stats.finite_min_or_nan([3.0, float("nan"), 1.0])
        1.0

        """
        ...

    @staticmethod
    def finite_max_or_nan(data: Sequence[float] | npt.NDArray[np.float64]) -> float:
        """
        Maximum over finite values, or ``nan`` when there are none.

        Parameters
        ----------
        data : Sequence[float] or numpy.ndarray
            Observations; NaN and infinities are ignored.

        Returns
        -------
        float
            Largest finite value, or ``nan``.

        Notes
        -----
        This function does not raise.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> stats.finite_max_or_nan([3.0, float("inf"), 1.0])
        3.0

        """
        ...

    @staticmethod
    def finite_count(data: Sequence[float] | npt.NDArray[np.float64]) -> int:
        """
        Number of finite (non-NaN, non-infinite) values.

        Parameters
        ----------
        data : Sequence[float] or numpy.ndarray
            Observations.

        Returns
        -------
        int
            Count of finite entries.

        Notes
        -----
        This function does not raise.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> stats.finite_count([1.0, float("nan"), 2.0])
        2

        """
        ...

    @staticmethod
    def log_returns(prices: Sequence[float] | npt.NDArray[np.float64]) -> list[float]:
        """
        Log returns ``ln(p_t / p_{t-1})`` of a chronological price series.

        Parameters
        ----------
        prices : Sequence[float] or numpy.ndarray
            Price levels in time order; adjacent prices must be finite and
            strictly positive to produce a finite return.

        Returns
        -------
        list[float]
            ``len(prices) - 1`` returns; invalid windows yield ``nan``, and
            fewer than two prices yield an empty list.

        Notes
        -----
        This function does not raise.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> [round(r, 6) for r in stats.log_returns([100.0, 110.0, 99.0])]
        [0.09531, -0.105361]

        """
        ...

    @staticmethod
    def realized_variance(
        prices: Sequence[float] | npt.NDArray[np.float64],
        method: str = "close_to_close",
        annualization_factor: float = 252.0,
    ) -> float:
        """
        Annualized realized variance of a close price series.

        Sums squared log returns without mean subtraction (market
        convention) and scales by *annualization_factor*.

        Parameters
        ----------
        prices : Sequence[float] or numpy.ndarray
            Finite, strictly positive close prices in time order.
        method : str
            Must be ``"close_to_close"``; OHLC estimators require
            :func:`realized_variance_ohlc`.
        annualization_factor : float
            Positive scaling factor (``252`` for daily bars).

        Returns
        -------
        float
            Annualized variance (not volatility).

        Raises
        ------
        ValueError
            If *method* is unknown or needs OHLC data, a price is non-positive
            or non-finite, or *annualization_factor* is not positive.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> round(stats.realized_variance([100.0, 101.0, 100.0], annualization_factor=1.0), 8)
        9.901e-05

        """
        ...

    @staticmethod
    def realized_variance_ohlc(
        open: Sequence[float] | npt.NDArray[np.float64],
        high: Sequence[float] | npt.NDArray[np.float64],
        low: Sequence[float] | npt.NDArray[np.float64],
        close: Sequence[float] | npt.NDArray[np.float64],
        method: str = "yang_zhang",
        annualization_factor: float = 252.0,
    ) -> float:
        """
        Annualized realized variance from OHLC bars.

        Parameters
        ----------
        open : Sequence[float] or numpy.ndarray
            Bar opening prices in time order, strictly positive and finite,
            in the same price units as the other three series.
        high : Sequence[float] or numpy.ndarray
            Bar high prices, same length and ordering as *open*; each value
            must be at least the corresponding low.
        low : Sequence[float] or numpy.ndarray
            Bar low prices, same length and ordering as *open*, strictly
            positive.
        close : Sequence[float] or numpy.ndarray
            Bar closing prices, same length and ordering as *open*; the
            close-to-close estimator uses only this series.
        method : str
            ``"close_to_close"``, ``"parkinson"``, ``"garman_klass"``,
            ``"rogers_satchell"`` or ``"yang_zhang"``.
        annualization_factor : float
            Positive scaling factor (``252`` for daily bars).

        Returns
        -------
        float
            Annualized variance under the chosen estimator.

        Raises
        ------
        ValueError
            If the four series differ in length, *method* is unknown, prices
            are invalid, or *annualization_factor* is not positive.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> v = stats.realized_variance_ohlc(
        ...     [100.0, 101.0],
        ...     [102.0, 103.0],
        ...     [99.0, 100.0],
        ...     [101.0, 102.0],
        ...     method="parkinson",
        ...     annualization_factor=1.0,
        ... )
        >>> v > 0.0
        True

        """
        ...

    @staticmethod
    def mean(data: Sequence[float] | npt.NDArray[np.float64]) -> float:
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

        Notes
        -----
        This method does not raise; an empty series returns ``0.0``.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> stats.mean([1.0, 2.0, 3.0])
        2.0
        """
        ...

    @staticmethod
    def variance(data: Sequence[float] | npt.NDArray[np.float64]) -> float:
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

        Notes
        -----
        This method does not raise; fewer than two observations return ``0.0``.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> round(stats.variance([1.0, 2.0, 3.0]), 10)
        1.0
        """
        ...

    @staticmethod
    def population_variance(data: Sequence[float] | npt.NDArray[np.float64]) -> float:
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

        Notes
        -----
        This method does not raise; an empty series returns ``0.0``.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> round(stats.population_variance([1.0, 2.0, 3.0]), 10)
        0.6666666667
        """
        ...

    @staticmethod
    def correlation(
        x: Sequence[float] | npt.NDArray[np.float64], y: Sequence[float] | npt.NDArray[np.float64]
    ) -> float:
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

        Notes
        -----
        This method does not raise; unequal lengths return ``NaN``.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> round(stats.correlation([1.0, 2.0, 3.0], [2.0, 4.0, 6.0]), 10)
        1.0
        """
        ...

    @staticmethod
    def covariance(x: Sequence[float] | npt.NDArray[np.float64], y: Sequence[float] | npt.NDArray[np.float64]) -> float:
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

        Notes
        -----
        This method does not raise; unequal lengths return ``NaN``.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> round(stats.covariance([1.0, 2.0, 3.0], [2.0, 4.0, 6.0]), 10)
        2.0
        """
        ...

    @staticmethod
    def quantile(data: Sequence[float] | npt.NDArray[np.float64], q: float) -> float:
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

        Notes
        -----
        This method does not raise; empty data, ``q`` outside ``[0, 1]``, or non-finite inputs return ``NaN``.

        Examples
        --------
        >>> from finstack_quant.core.math import stats
        >>> stats.quantile([1.0, 2.0, 3.0, 4.0, 5.0], 0.5)
        3.0
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
    def norm_cdf_with_params(x: float, mean: float, std_dev: float) -> float:
        """
        Normal CDF with explicit mean and standard deviation.

        Parameters
        ----------
        x : float
            Evaluation point.
        mean : float
            Distribution mean.
        std_dev : float
            Distribution standard deviation; must be strictly positive.

        Returns
        -------
        float
            ``P(X <= x)`` for ``X ~ N(mean, std_dev**2)``.

        Raises
        ------
        ValueError
            If *std_dev* is not strictly positive or any input is non-finite.

        Examples
        --------
        >>> from finstack_quant.core.math import special_functions
        >>> round(special_functions.norm_cdf_with_params(1.0, 1.0, 2.0), 10)
        0.5

        """
        ...

    @staticmethod
    def norm_pdf_with_params(x: float, mean: float, std_dev: float) -> float:
        """
        Normal PDF with explicit mean and standard deviation.

        Parameters
        ----------
        x : float
            Evaluation point.
        mean : float
            Distribution mean.
        std_dev : float
            Distribution standard deviation; must be strictly positive.

        Returns
        -------
        float
            Density of ``N(mean, std_dev**2)`` at *x*.

        Raises
        ------
        ValueError
            If *std_dev* is not strictly positive or any input is non-finite.

        Examples
        --------
        >>> from finstack_quant.core.math import special_functions
        >>> round(special_functions.norm_pdf_with_params(0.0, 0.0, 1.0), 10)
        0.3989422804

        """
        ...

    @staticmethod
    def norm_cdf(x: float) -> float:
        r"""Standard normal cumulative distribution function :math:`\Phi(x)`.

        Scalar only: for a vector of inputs use ``[norm_cdf(v) for v in xs]``
        or ``numpy.vectorize(norm_cdf)(xs)``; there is no array overload.

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
        -0.0
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
    >>> summation.kahan_sum([1.0, 2.0, 3.0])
    6.0

    """

    @staticmethod
    def kahan_sum(values: Sequence[float] | npt.NDArray[np.float64]) -> float:
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

        Notes
        -----
        This method does not raise; it returns the compensated sum of ``values``.

        Examples
        --------
        >>> from finstack_quant.core.math import summation
        >>> summation.kahan_sum([1.0, 2.0, 3.0])
        6.0
        """
        ...

    @staticmethod
    def neumaier_sum(values: Sequence[float] | npt.NDArray[np.float64]) -> float:
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

        Notes
        -----
        This method does not raise; it returns the compensated sum of ``values``.

        Examples
        --------
        >>> from finstack_quant.core.math import summation
        >>> summation.neumaier_sum([1.0, -2.0, 3.0])
        2.0
        """
        ...
