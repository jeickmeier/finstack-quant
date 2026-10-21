"""
Statistical functions: mean, variance, correlation, covariance, quantiles,
NaN-sentinel summaries, log returns and realized variance.

Vector parameters accept any ``Sequence[float]`` or a 1-D ``float64``
NumPy array, including strided or reversed views. Inputs are copied before
Rust computation releases the GIL; sorting operations do not mutate the
caller's array. Invalid series types or dimensions raise ``TypeError``.

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
    "finite_max_or_nan",
    "finite_min_or_nan",
    "log_returns",
    "longest_positive_run",
    "mean",
    "mean_or_nan",
    "mean_var",
    "median_or_nan",
    "population_variance",
    "quantile",
    "realized_variance",
    "realized_variance_ohlc",
    "sample_std_or_nan",
    "sample_variance_or_nan",
    "variance",
]

def longest_positive_run(values: Sequence[float] | npt.NDArray[np.float64]) -> int:
    """
    Length of the longest run of strictly positive values.

    Parameters
    ----------
    values : Sequence[float] or numpy.ndarray
        Ordered numeric observations; only values strictly greater than zero
        extend a run, while zero, negative and NaN values reset it.

    Returns
    -------
    int
        Longest positive run length; ``0`` for an empty series.

    Raises
    ------
    TypeError
        If a series is not one-dimensional or contains values that cannot
        be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math.stats import longest_positive_run
    >>> longest_positive_run([1.0, 2.0, -1.0, 3.0])
    2

    """
    ...

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

    Raises
    ------
    TypeError
        If a series is not one-dimensional or contains values that cannot
        be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> stats.mean_var([1.0, 2.0, 3.0])
    (2.0, 1.0)

    """
    ...

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

    Raises
    ------
    TypeError
        If a series is not one-dimensional or contains values that cannot
        be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> stats.mean_or_nan([1.0, 3.0])
    2.0

    """
    ...

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

    Raises
    ------
    TypeError
        If a series is not one-dimensional or contains values that cannot
        be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> stats.sample_variance_or_nan([1.0, 2.0, 3.0])
    1.0

    """
    ...

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

    Raises
    ------
    TypeError
        If a series is not one-dimensional or contains values that cannot
        be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> stats.sample_std_or_nan([1.0, 2.0, 3.0])
    1.0

    """
    ...

def median_or_nan(data: Sequence[float] | npt.NDArray[np.float64]) -> float:
    """
    Median (mean of the two middle values for even counts), or ``nan`` for empty
    input or when any observation is non-finite.

    Parameters
    ----------
    data : Sequence[float] or numpy.ndarray
        Observations (sorted internally).

    Returns
    -------
    float
        Median, or ``nan``.

    Raises
    ------
    TypeError
        If a series is not one-dimensional or contains values that cannot
        be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> stats.median_or_nan([3.0, 1.0, 2.0, 4.0])
    2.5

    """
    ...

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

    Raises
    ------
    TypeError
        If a series is not one-dimensional or contains values that cannot
        be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> stats.finite_min_or_nan([3.0, float("nan"), 1.0])
    1.0

    """
    ...

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

    Raises
    ------
    TypeError
        If a series is not one-dimensional or contains values that cannot
        be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> stats.finite_max_or_nan([3.0, float("inf"), 1.0])
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
    An empty series returns ``0.0``.

    Raises
    ------
    TypeError
        If ``data`` is not a one-dimensional numeric sequence or contains
        values that cannot be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> stats.mean([1.0, 2.0, 3.0])
    2.0
    """
    ...

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
    Fewer than two observations return ``0.0``.

    Raises
    ------
    TypeError
        If ``data`` is not a one-dimensional numeric sequence or contains
        values that cannot be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> round(stats.variance([1.0, 2.0, 3.0]), 10)
    1.0
    """
    ...

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
    An empty series returns ``0.0``.

    Raises
    ------
    TypeError
        If ``data`` is not a one-dimensional numeric sequence or contains
        values that cannot be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> round(stats.population_variance([1.0, 2.0, 3.0]), 10)
    0.6666666667
    """
    ...

def correlation(x: Sequence[float] | npt.NDArray[np.float64], y: Sequence[float] | npt.NDArray[np.float64]) -> float:
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
    Unequal lengths return ``NaN``; fewer than two observations or a
    constant series return ``0.0``.

    Raises
    ------
    TypeError
        If either series is not one-dimensional or contains values that
        cannot be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> round(stats.correlation([1.0, 2.0, 3.0], [2.0, 4.0, 6.0]), 10)
    1.0
    """
    ...

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
    Unequal lengths return ``NaN``; fewer than two observations return
    ``0.0``.

    Raises
    ------
    TypeError
        If either series is not one-dimensional or contains values that
        cannot be converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> round(stats.covariance([1.0, 2.0, 3.0], [2.0, 4.0, 6.0]), 10)
    2.0
    """
    ...

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
    Empty data, ``q`` outside ``[0, 1]``, or non-finite inputs return
    ``NaN``. Caller storage is never reordered.

    Raises
    ------
    TypeError
        If ``data`` is not a one-dimensional numeric sequence or contains
        values that cannot be converted to float, or ``q`` cannot be
        converted to float.

    Examples
    --------
    >>> from finstack_quant.core.math import stats
    >>> stats.quantile([1.0, 2.0, 3.0, 4.0, 5.0], 0.5)
    3.0
    """
    ...
