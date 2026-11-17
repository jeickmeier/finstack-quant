"""
Statistical functions: mean, variance, correlation, covariance, quantiles,
standard deviation, median, finite extrema, log returns and realized variance.

Vector parameters accept any ``Sequence[float]`` or a 1-D ``float64``
NumPy array, including strided or reversed views. Inputs are copied before
Rust computation releases the GIL; sorting operations do not mutate the
caller's array. Invalid series types or dimensions raise ``TypeError``, and
non-finite or invalid inputs raise ``ValueError`` (for exact-finite functions).

Examples
--------
>>> from finstack_quant.core.math import stats
>>> (stats.mean([1.0, 2.0, 3.0]), stats.quantile([1.0, 2.0, 3.0], 0.5))
(2.0, 2.0)

"""

from __future__ import annotations

from collections.abc import Sequence

import numpy as np
import numpy.typing as npt

__all__ = [
    "correlation",
    "covariance",
    "finite_count",
    "finite_max",
    "finite_min",
    "log_returns",
    "mean",
    "mean_var",
    "median",
    "population_variance",
    "quantile",
    "realized_variance",
    "realized_variance_ohlc",
    "std_dev",
    "variance",
]

def mean_var(data: Sequence[float] | npt.NDArray[np.float64]) -> tuple[float | None, float | None]:
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

def std_dev(data: Sequence[float] | npt.NDArray[np.float64]) -> float | None:
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

def median(data: Sequence[float] | npt.NDArray[np.float64]) -> float | None:
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

def finite_min(data: Sequence[float] | npt.NDArray[np.float64]) -> float | None:
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

def finite_max(data: Sequence[float] | npt.NDArray[np.float64]) -> float | None:
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

    Raises
    ------
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
    prices: Sequence[float] | npt.NDArray[np.float64],
    annualization_factor: float | None = None,
) -> float:
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
    float
        Annualized variance (not volatility).

    Raises
    ------
    ValueError
        If a price is non-positive or non-finite, or *annualization_factor*
        is not positive.
    TypeError
        If a series is not one-dimensional or contains values that cannot
        be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> round(stats.realized_variance([100.0, 101.0, 100.0], annualization_factor=1.0), 8)
    9.901e-05

    """
    ...

def realized_variance_ohlc(
    open: Sequence[float] | npt.NDArray[np.float64],
    high: Sequence[float] | npt.NDArray[np.float64],
    low: Sequence[float] | npt.NDArray[np.float64],
    close: Sequence[float] | npt.NDArray[np.float64],
    method: str | None = None,
    annualization_factor: float | None = None,
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
    method : str or None
        ``"close_to_close"``, ``"parkinson"``, ``"garman_klass"``,
        ``"rogers_satchell"`` or ``"yang_zhang"``. ``None`` selects the
        Rust OHLC default (``RealizedVarMethod::OHLC_DEFAULT``, Yang-Zhang).
    annualization_factor : float or None
        Positive bars-per-year scaling factor. ``None`` selects the Rust
        daily convention (``PeriodKind::Daily``, 252).

    Returns
    -------
    float
        Annualized variance under the chosen estimator.

    Raises
    ------
    ValueError
        If the four series differ in length, *method* is unknown, prices
        are invalid, or *annualization_factor* is not positive.
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

def mean(data: Sequence[float] | npt.NDArray[np.float64]) -> float | None:
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

def variance(data: Sequence[float] | npt.NDArray[np.float64]) -> float | None:
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

def population_variance(data: Sequence[float] | npt.NDArray[np.float64]) -> float | None:
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

def correlation(
    x: Sequence[float] | npt.NDArray[np.float64], y: Sequence[float] | npt.NDArray[np.float64]
) -> float | None:
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

def covariance(
    x: Sequence[float] | npt.NDArray[np.float64], y: Sequence[float] | npt.NDArray[np.float64]
) -> float | None:
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

def quantile(data: Sequence[float] | npt.NDArray[np.float64], q: float) -> float | None:
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
