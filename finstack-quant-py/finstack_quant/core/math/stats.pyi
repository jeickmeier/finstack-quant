"""
Statistical functions: mean, variance, correlation, covariance, quantiles,
standard deviation, median, finite extrema, log returns and realized variance.

Vector parameters accept any ``Sequence[float]``, numeric 1-D NumPy array,
``pandas.Series``, or ``polars.Series``; ``ols`` predictors accept nested
sequences, 2-D numeric NumPy arrays, ``pandas.DataFrame``, ``polars.DataFrame``,
or Arrow tables. Inputs are copied before Rust computation releases the GIL;
sorting operations do not mutate the caller's array. Invalid series types or
dimensions raise ``TypeError``, and non-finite or invalid inputs raise
``ValueError`` (for exact-finite functions).

Examples
--------
>>> from finstack_quant.core.math import stats
>>> (stats.mean([1.0, 2.0, 3.0]), stats.quantile([1.0, 2.0, 3.0], 0.5))
(2.0, 2.0)

"""

from __future__ import annotations

from collections.abc import Sequence
from typing import Any

import numpy as np
import numpy.typing as npt
import pandas as pd
import polars as pl

SeriesInput = Sequence[float] | npt.NDArray[Any] | pd.Series | pl.Series
MatrixInput = Sequence[Sequence[float]] | npt.NDArray[Any] | pd.DataFrame | pl.DataFrame | Any

__all__ = [
    "OlsResult",
    "correlation",
    "covariance",
    "expected_shortfall",
    "finite_count",
    "finite_max",
    "finite_min",
    "log_returns",
    "mean",
    "mean_var",
    "median",
    "ols",
    "population_variance",
    "quantile",
    "realized_variance",
    "realized_variance_ohlc",
    "std_dev",
    "variance",
]

def mean_var(data: SeriesInput) -> tuple[float | None, float | None]:
    """
    ``(mean, sample_variance)`` in a single pass.

    Parameters
    ----------
    data : Sequence[float] or numpy.ndarray
        Finite observations; sample variance uses the ``n - 1`` denominator.

    Returns
    -------
    tuple[float | None, float | None]
        ``(mean, variance)``; ``(None, None)`` for empty input and
        ``(mean, None)`` for a single observation.

    Raises
    ------
    TypeError
        If a series is not one-dimensional or contains values that cannot
        be converted to float.
    ValueError
        If any observation is non-finite or ``len(data)`` exceeds ``2**53``.
    RuntimeError
        If the mean or a non-constant sample variance overflows or
        underflows ``float``.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> stats.mean_var([1.0, 2.0, 3.0])
    (2.0, 1.0)

    """
    ...

def std_dev(data: SeriesInput) -> float | None:
    """
    Unbiased sample standard deviation (``n - 1`` denominator).

    Parameters
    ----------
    data : Sequence[float] or numpy.ndarray
        Finite sample observations.

    Returns
    -------
    float | None
        Square root of the unbiased sample variance, or ``None`` for fewer
        than two observations; ``0.0`` when ``len(data) >= 2`` and all
        observations are identical.

    Raises
    ------
    TypeError
        If a series is not one-dimensional or contains values that cannot
        be converted to float.
    ValueError
        If any observation is non-finite or ``len(data)`` exceeds ``2**53``.
    RuntimeError
        If a non-constant sample standard deviation overflows or underflows
        ``float``.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> stats.std_dev([1.0, 2.0, 3.0])
    1.0

    """
    ...

def median(data: SeriesInput) -> float | None:
    """
    Sample median using R-7 / NumPy quantile interpolation at ``q = 0.5``.

    Parameters
    ----------
    data : Sequence[float] or numpy.ndarray
        Finite observations (sorted on a copy; caller storage is unchanged).

    Returns
    -------
    float | None
        Median value, or ``None`` for empty input.

    Raises
    ------
    TypeError
        If a series is not one-dimensional or contains values that cannot
        be converted to float.
    ValueError
        If any observation is non-finite or ``len(data)`` exceeds ``2**53``.
    RuntimeError
        If a nonzero median underflows ``float``.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> stats.median([3.0, 1.0, 2.0, 4.0])
    2.5

    """
    ...

def finite_min(data: SeriesInput) -> float | None:
    """
    Minimum over finite values, ignoring ``nan`` and infinities.

    Parameters
    ----------
    data : Sequence[float] or numpy.ndarray
        Observations; ``nan`` and infinities are ignored.

    Returns
    -------
    float | None
        Smallest finite value, or ``None`` when no observation is finite.

    Raises
    ------
    TypeError
        If a series is not one-dimensional or contains values that cannot
        be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> stats.finite_min([3.0, float("nan"), 1.0])
    1.0

    """
    ...

def finite_max(data: SeriesInput) -> float | None:
    """
    Maximum over finite values, ignoring ``nan`` and infinities.

    Parameters
    ----------
    data : Sequence[float] or numpy.ndarray
        Observations; ``nan`` and infinities are ignored.

    Returns
    -------
    float | None
        Largest finite value, or ``None`` when no observation is finite.

    Raises
    ------
    TypeError
        If a series is not one-dimensional or contains values that cannot
        be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> stats.finite_max([3.0, float("inf"), 1.0])
    3.0

    """
    ...

def finite_count(data: SeriesInput) -> int:
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

    Raises
    ------
    TypeError
        If a series is not one-dimensional or contains values that cannot
        be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> stats.finite_count([1.0, float("nan"), 2.0])
    2

    """
    ...

def log_returns(prices: SeriesInput) -> list[float]:
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
        Adjacent log returns in chronological order;
        fewer than two prices yield an empty list.

    Raises
    ------
    ValueError
        If any price is nonpositive or nonfinite.
    TypeError
        If a series is not one-dimensional or contains values that cannot
        be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> [round(r, 6) for r in stats.log_returns([100.0, 110.0, 99.0])]
    [0.09531, -0.105361]

    """
    ...

def realized_variance(
    prices: SeriesInput,
    annualization_factor: float | None = None,
) -> float | None:
    """
    Annualized realized variance of a close price series.

    Averages squared log returns without mean subtraction (market
    convention) and scales by *annualization_factor*.

    Parameters
    ----------
    prices : Sequence[float] or numpy.ndarray
        Finite, strictly positive close prices in time order.
    annualization_factor : float or None
        Positive observations-per-year scaling factor. ``None`` selects
        the Rust daily convention (``PeriodKind::Daily``, 252).

    Returns
    -------
    float or None
        Annualized decimal variance, or None for fewer than two valid closes.

    Raises
    ------
    ValueError
        If a price is non-positive or non-finite, or *annualization_factor*
        is not positive or finite.
    RuntimeError
        If the annualized variance is outside the representable range.
    TypeError
        If a series is not one-dimensional or contains values that cannot
        be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> v = stats.realized_variance([100.0, 101.0, 100.0], annualization_factor=1.0)
    >>> assert v is not None
    >>> round(v, 8)
    9.901e-05

    """
    ...

def realized_variance_ohlc(
    open: SeriesInput,
    high: SeriesInput,
    low: SeriesInput,
    close: SeriesInput,
    method: str | None = None,
    annualization_factor: float | None = None,
    previous_close: float | None = None,
) -> float | None:
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
    previous_close : float, optional
        Positive finite close preceding the first target bar. Required for close-to-close and Yang-Zhang and omitted for range estimators.
    method : str or None
        ``"close_to_close"``, ``"parkinson"``, ``"garman_klass"``,
        ``"rogers_satchell"`` or ``"yang_zhang"``. ``None`` selects the
        Rust OHLC default (``RealizedVarMethod::OHLC_DEFAULT``, Yang-Zhang).
    annualization_factor : float or None
        Positive bars-per-year scaling factor. ``None`` selects the Rust
        daily convention (``PeriodKind::Daily``, 252).

    Returns
    -------
    float or None
        Annualized variance over every target bar. No value is available for an empty window or fewer than two Yang–Zhang bars.

    Raises
    ------
    RuntimeError
        If the annual variance cannot be represented as a finite float.
    ValueError
        If the four series differ in length, *method* is unknown, prices
        are invalid, the preceding close is missing or inappropriate, or *annualization_factor* is not positive.
    TypeError
        If a series is not one-dimensional or contains values that cannot
        be converted to float.

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

def mean(data: SeriesInput) -> float | None:
    """
    Arithmetic mean of a data series.

    Returns ``None`` for an empty series.

    Parameters
    ----------
    data : Sequence[float] or numpy.ndarray
        Finite input observations.

    Returns
    -------
    float | None
        Arithmetic mean, or ``None`` when ``data`` is empty.

    Raises
    ------
    TypeError
        If ``data`` is not a one-dimensional numeric sequence or contains
        values that cannot be converted to float.
    ValueError
        If any observation is non-finite or ``len(data)`` exceeds ``2**53``.
    RuntimeError
        If reconstructing the scaled mean overflows ``float``.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> stats.mean([1.0, 2.0, 3.0])
    2.0
    """
    ...

def variance(data: SeriesInput) -> float | None:
    """
    Sample variance (unbiased, ``n - 1`` denominator).

    Returns ``None`` for fewer than 2 observations, and ``0.0`` when
    ``len(data) >= 2`` and all observations are identical.

    Parameters
    ----------
    data : Sequence[float] or numpy.ndarray
        Finite sample observations.

    Returns
    -------
    float | None
        Sample variance, or ``None`` when ``len(data) < 2``.

    Raises
    ------
    TypeError
        If ``data`` is not a one-dimensional numeric sequence or contains
        values that cannot be converted to float.
    ValueError
        If any observation is non-finite or ``len(data)`` exceeds ``2**53``.
    RuntimeError
        If a non-constant sample variance overflows or underflows ``float``.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> round(stats.variance([1.0, 2.0, 3.0]) or 0.0, 10)
    1.0
    """
    ...

def population_variance(data: SeriesInput) -> float | None:
    """
    Population variance (``n`` denominator).

    Returns ``None`` for an empty series, and ``0.0`` for a constant series.

    Parameters
    ----------
    data : Sequence[float] or numpy.ndarray
        Finite input observations.

    Returns
    -------
    float | None
        Population variance, or ``None`` when ``data`` is empty.

    Raises
    ------
    TypeError
        If ``data`` is not a one-dimensional numeric sequence or contains
        values that cannot be converted to float.
    ValueError
        If any observation is non-finite or ``len(data)`` exceeds ``2**53``.
    RuntimeError
        If a non-constant population variance overflows or underflows
        ``float``.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> round(stats.population_variance([1.0, 2.0, 3.0]) or 0.0, 10)
    0.6666666667
    """
    ...

def correlation(x: SeriesInput, y: SeriesInput) -> float | None:
    """
    Pearson correlation coefficient between two equal-length series.

    Returns ``None`` when fewer than two observations exist or either series is
    constant.

    Parameters
    ----------
    x : Sequence[float] or numpy.ndarray
        First finite data series.
    y : Sequence[float] or numpy.ndarray
        Second finite data series, matching ``len(x)``.

    Returns
    -------
    float | None
        Correlation in ``[-1, 1]``, or ``None`` for insufficient or constant data.

    Raises
    ------
    TypeError
        If either series is not one-dimensional or contains values that
        cannot be converted to float.
    ValueError
        If series lengths differ, any observation is non-finite, or
        ``len(x)`` exceeds ``2**53``.
    RuntimeError
        If numerical evaluation leaves ``[-1, 1]`` or becomes non-finite.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> round(stats.correlation([1.0, 2.0, 3.0], [2.0, 4.0, 6.0]) or 0.0, 10)
    1.0
    """
    ...

def covariance(x: SeriesInput, y: SeriesInput) -> float | None:
    """
    Sample covariance (unbiased, ``n - 1`` denominator).

    Returns ``None`` when fewer than two observations exist.

    Parameters
    ----------
    x : Sequence[float] or numpy.ndarray
        First finite data series.
    y : Sequence[float] or numpy.ndarray
        Second finite data series, matching ``len(x)``.

    Returns
    -------
    float | None
        Sample covariance, or ``None`` when ``len(x) < 2``.

    Raises
    ------
    TypeError
        If either series is not one-dimensional or contains values that
        cannot be converted to float.
    ValueError
        If series lengths differ, any observation is non-finite, or
        ``len(x)`` exceeds ``2**53``.
    RuntimeError
        If a nonzero co-moment overflows or underflows ``float``.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> round(stats.covariance([1.0, 2.0, 3.0], [2.0, 4.0, 6.0]) or 0.0, 10)
    2.0
    """
    ...

def expected_shortfall(data: SeriesInput, confidence: float) -> float:
    """Compute the signed mean of the worst fractional tail of a sample.

    Parameters
    ----------
    data : Sequence[float] or numpy.ndarray
        Nonempty one-dimensional finite observations, such as simple decimal
        returns. Observations may be supplied in any order.
    confidence : float
        Confidence level strictly between zero and one. The lower tail has
        exactly ``1 - confidence`` of the empirical probability mass.

    Returns
    -------
    float
        Signed lower-tail mean in the same units as the observations.

    Raises
    ------
    TypeError
        If the observations cannot be read as a one-dimensional numeric series.
    ValueError
        If the confidence is out of range, or the sample is empty or non-finite.
    RuntimeError
        If the result cannot be represented.

    Examples
    --------
    >>> from finstack_quant.core.math.stats import expected_shortfall
    >>> round(expected_shortfall([1.0, -4.0, -1.0, 2.0], 0.625), 3)
    -3.0
    """
    ...

def quantile(data: SeriesInput, q: float) -> float | None:
    """
    Empirical quantile (R-7 / NumPy default) with linear interpolation.

    Returns ``None`` for empty data; caller storage is never reordered.

    Parameters
    ----------
    data : Sequence[float] or numpy.ndarray
        Finite input observations (sorted on a copy).
    q : float
        Quantile probability in ``[0, 1]``.

    Returns
    -------
    float | None
        Quantile value, or ``None`` when ``data`` is empty.

    Raises
    ------
    TypeError
        If ``data`` is not a one-dimensional numeric sequence or contains
        values that cannot be converted to float, or ``q`` cannot be
        converted to float.
    ValueError
        If ``q`` is non-finite or outside ``[0, 1]``, any observation in
        ``data`` is non-finite, or ``len(data)`` exceeds ``2**53``.
    RuntimeError
        If a nonzero interpolated quantile underflows ``float``.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> stats.quantile([1.0, 2.0, 3.0, 4.0, 5.0], 0.5)
    3.0
    """
    ...

class OlsResult:
    """A full-rank ordinary least-squares fit and its diagnostics.

    Examples
    --------
    >>> from finstack_quant.core.math.stats import ols
    >>> fit = ols([[0], [1], [2]], [1, 3, 5])
    >>> round(fit.coefficients[0], 10), round(fit.intercept or 0, 10)
    (2.0, 1.0)
    """

    @property
    def coefficients(self) -> list[float]:
        """Read coefficients from the fitted regression.

        Returns
        -------
        list[float]
            Fitted slopes in predictor column order.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def intercept(self) -> float | None:
        """Read intercept from the fitted regression.

        Returns
        -------
        float | None
            Fitted constant in response units, or None for a through-origin regression.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def r_squared(self) -> float:
        """Read r squared from the fitted regression.

        Returns
        -------
        float
            Fraction of total variation accounted for by the fit.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def adj_r_squared(self) -> float:
        """Read adj r squared from the fitted regression.

        Returns
        -------
        float
            Coefficient of determination adjusted for residual degrees of freedom.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def residual_std_error(self) -> float:
        """Read residual std error from the fitted regression.

        Returns
        -------
        float
            Residual standard error in original response units.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def residuals(self) -> list[float]:
        """Read residuals from the fitted regression.

        Returns
        -------
        list[float]
            Observed minus fitted responses in input row order.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def n_obs(self) -> int:
        """Read n obs from the fitted regression.

        Returns
        -------
        int
            Number of observations in the fit.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def n_features(self) -> int:
        """Read n features from the fitted regression.

        Returns
        -------
        int
            Number of fitted slopes.

        Notes
        -----
        This accessor does not raise.
        """
        ...

def ols(
    x: MatrixInput,
    y: SeriesInput,
    *,
    add_intercept: bool = True,
) -> OlsResult:
    """Fit a linear regression by singular value decomposition.

    Parameters
    ----------
    x : Sequence[Sequence[float]] | numpy.ndarray | pandas.DataFrame | polars.DataFrame | Any
        Rectangular finite predictors with one observation per row and
        one feature per column, in regression column order.
    y : Sequence[float] | numpy.ndarray | pandas.Series | polars.Series
        One finite response per observation, in the same row order.
    add_intercept : bool
        Fit a constant and report centered R². False fits through the
        origin and uses uncentered total variation.

    Returns
    -------
    OlsResult
        The fitted slopes, optional intercept and diagnostics in input units.

    Raises
    ------
    TypeError
        If the supplied arrays have incompatible element or container types.
    ValueError
        If the dimensions mismatch, inputs are non-finite, the design is rank
        deficient or lacks residual degrees of freedom, the response variation
        is zero on the selected basis, or the fit cannot be represented.

    Examples
    --------
    >>> from finstack_quant.core.math.stats import ols
    >>> round(ols([[1], [2], [3]], [2, 4, 6], add_intercept=False).r_squared, 10)
    1.0
    """
    ...
