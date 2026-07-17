"""
Monte Carlo convenience bindings (``finstack-quant-monte-carlo``).

Exposes simulation primitives: time grids, engine configuration, pricers,
closed-form Black-Scholes helpers, and selected non-GBM process wrappers.
Advanced Rust process, discretization, RNG, payoff, and Greeks types are not
surfaced as standalone Python types yet; their parameters are passed directly
as numeric arguments to the exposed pricer constructors and methods.

Examples
--------
>>> import finstack_quant.monte_carlo as monte_carlo
>>> monte_carlo.__name__
'finstack_quant.monte_carlo'
"""

from __future__ import annotations

from collections.abc import Sequence

from finstack_quant.core.money import Money

__all__ = [
    "Estimate",
    "EuropeanPricer",
    "GbmPathSummary",
    "LsmcPricer",
    "McEngine",
    "MoneyEstimate",
    "PathDependentPricer",
    "TimeGrid",
    "black_scholes_call",
    "black_scholes_put",
    "finite_diff_delta",
    "finite_diff_delta_crn",
    "finite_diff_gamma",
    "finite_diff_gamma_crn",
    "heston_satisfies_feller",
    "price_european_call",
    "price_european_put",
    "price_heston_call",
    "price_heston_put",
    "simulate_gbm_paths",
]

class MoneyEstimate:
    """
    Discounted Monte Carlo estimate with money units and confidence bands.

    Examples
    --------
    >>> from finstack_quant.monte_carlo import price_european_call
    >>> r = price_european_call(100, 100, 0.05, 0.0, 0.2, 1.0, num_paths=10_000)
    >>> r.num_paths
    10000
    """

    @property
    def mean(self) -> Money:
        """
        Discounted mean present value.

        Returns
        -------
        Money
            Mean PV with currency tag.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import price_european_call
        >>> price_european_call(100, 100, 0.05, 0.0, 0.2, 1.0, num_paths=1000).mean.amount > 0
        True
        """
        ...

    @property
    def stderr(self) -> float:
        """
        Standard error of the discounted mean.

        Returns
        -------
        float
            Standard error in the same currency units as :attr:`mean`.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import price_european_call
        >>> price_european_call(100, 100, 0.05, 0.0, 0.2, 1.0, num_paths=1000).stderr >= 0
        True
        """
        ...

    @property
    def std_dev(self) -> float | None:
        """
        Sample standard deviation of path discounted values, if available.

        Returns
        -------
        float or None
            Sample standard deviation, or ``None`` if not captured by the engine.
        """
        ...

    @property
    def ci_lower(self) -> Money:
        """
        Lower bound of the 95% confidence interval for the mean.

        Returns
        -------
        Money
            Lower CI bound.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import price_european_call
        >>> r = price_european_call(100, 100, 0.05, 0.0, 0.2, 1.0, num_paths=2000)
        >>> r.ci_lower.amount <= r.mean.amount
        True
        """
        ...

    @property
    def ci_upper(self) -> Money:
        """
        Upper bound of the 95% confidence interval for the mean.

        Returns
        -------
        Money
            Upper CI bound.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import price_european_call
        >>> r = price_european_call(100, 100, 0.05, 0.0, 0.2, 1.0, num_paths=2000)
        >>> r.ci_upper.amount >= r.mean.amount
        True
        """
        ...

    @property
    def num_paths(self) -> int:
        """
        Number of independent path estimators contributing to the result.

        Equals the configured ``num_paths`` when antithetic variates are off,
        or half the number of simulated paths when antithetic pairing is on.

        Returns
        -------
        int
            Path-estimator count.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import price_european_call
        >>> price_european_call(100, 100, 0.05, 0.0, 0.2, 1.0, num_paths=1234).num_paths
        1234
        """
        ...

    @property
    def num_simulated_paths(self) -> int:
        """
        Total number of simulated sample paths driving the estimator.

        Equals :attr:`num_paths` without variance reduction, or
        ``2 * num_paths`` when antithetic variates are enabled.

        Returns
        -------
        int
            Count of simulated sample paths.
        """
        ...

    @property
    def median(self) -> float | None:
        """
        Median of captured discounted path values, if captured.

        Returns
        -------
        float or None
            Median discounted path value, or ``None`` when percentile capture is
            disabled in the engine configuration.
        """
        ...

    @property
    def percentile_25(self) -> float | None:
        """
        25th percentile of captured discounted path values, if captured.

        Returns
        -------
        float or None
            25th percentile of discounted path values, or ``None`` when
            percentile capture is disabled.
        """
        ...

    @property
    def percentile_75(self) -> float | None:
        """
        75th percentile of captured discounted path values, if captured.

        Returns
        -------
        float or None
            75th percentile of discounted path values, or ``None`` when
            percentile capture is disabled.
        """
        ...

    @property
    def min(self) -> float | None:
        """
        Minimum of captured discounted path values, if captured.

        Returns
        -------
        float or None
            Minimum sampled discounted value, or ``None`` when range capture is
            disabled.
        """
        ...

    @property
    def max(self) -> float | None:
        """
        Maximum of captured discounted path values, if captured.

        Returns
        -------
        float or None
            Maximum sampled discounted value, or ``None`` when range capture is
            disabled.
        """
        ...

    def relative_stderr(self) -> float:
        """
        Relative standard error (stderr divided by absolute mean amount).

        Returns
        -------
        float
            Dimensionless relative stderr.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import price_european_call
        >>> price_european_call(100, 100, 0.05, 0.0, 0.2, 1.0, num_paths=5000).relative_stderr() >= 0
        True
        """
        ...

class Estimate:
    """
    Scalar Monte Carlo estimate without currency tagging.

    Examples
    --------
    >>> from finstack_quant.monte_carlo import Estimate
    >>> # Estimate objects are returned by scalar MC functions.
    """

    @property
    def mean(self) -> float:
        """
        Point estimate (mean).

        Returns
        -------
        float
            Mean sample value.
        """
        ...

    @property
    def stderr(self) -> float:
        """
        Standard error of the mean.

        Returns
        -------
        float
            Standard error.
        """
        ...

    @property
    def std_dev(self) -> float | None:
        """
        Sample standard deviation, if available.

        Returns
        -------
        float or None
            Sample standard deviation or ``None``.
        """
        ...

    @property
    def ci_lower(self) -> float:
        """
        Lower 95% confidence bound.

        Returns
        -------
        float
            Lower bound.
        """
        ...

    @property
    def ci_upper(self) -> float:
        """
        Upper 95% confidence bound.

        Returns
        -------
        float
            Upper bound.
        """
        ...

    @property
    def num_paths(self) -> int:
        """
        Number of independent path estimators contributing to the estimate.

        Equals the configured ``num_paths`` when antithetic variates are off,
        or half the number of simulated paths when antithetic pairing is on.

        Returns
        -------
        int
            Path-estimator count.
        """
        ...

    @property
    def num_simulated_paths(self) -> int:
        """
        Total number of simulated sample paths driving the estimator.

        Equals :attr:`num_paths` without variance reduction, or
        ``2 * num_paths`` when antithetic variates are enabled.

        Returns
        -------
        int
            Count of simulated sample paths.
        """
        ...

    @property
    def median(self) -> float | None:
        """
        Median of captured path values, if captured.

        Returns
        -------
        float or None
            Median path value, or ``None`` when percentile capture is disabled.
        """
        ...

    @property
    def percentile_25(self) -> float | None:
        """
        25th percentile of captured path values, if captured.

        Returns
        -------
        float or None
            25th percentile path value, or ``None`` when percentile capture is
            disabled.
        """
        ...

    @property
    def percentile_75(self) -> float | None:
        """
        75th percentile of captured path values, if captured.

        Returns
        -------
        float or None
            75th percentile path value, or ``None`` when percentile capture is
            disabled.
        """
        ...

    @property
    def min(self) -> float | None:
        """
        Minimum of captured path values, if captured.

        Returns
        -------
        float or None
            Minimum sampled path value, or ``None`` when range capture is
            disabled.
        """
        ...

    @property
    def max(self) -> float | None:
        """
        Maximum of captured path values, if captured.

        Returns
        -------
        float or None
            Maximum sampled path value, or ``None`` when range capture is
            disabled.
        """
        ...

class TimeGrid:
    """
    Discretised time axis for Monte Carlo stepping.

    Examples
    --------
    >>> from finstack_quant.monte_carlo import TimeGrid
    >>> TimeGrid(1.0, 4).num_steps
    4
    """

    def __init__(self, t_max: float, num_steps: int) -> None:
        """
        Build a uniform grid from ``0`` to ``t_max`` with ``num_steps`` steps.

        Parameters
        ----------
        t_max : float
            Terminal time in years.
        num_steps : int
            Number of steps between 0 and ``t_max``.

        Raises
        ------
        ValueError
            If ``t_max`` is non-positive or ``num_steps`` is less than 1.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import TimeGrid
        >>> TimeGrid(0.5, 10).t_max
        0.5
        """
        ...

    @staticmethod
    def from_times(times: Sequence[float]) -> TimeGrid:
        """
        Construct a grid from explicit increasing time points.

        Parameters
        ----------
        times : Sequence[float]
            Strictly increasing time knot sequence (copied as ``list[float]``
            internally).

        Returns
        -------
        TimeGrid
            A ``TimeGrid`` instance.

        Raises
        ------
        ValueError
            If ``times`` is empty, not strictly increasing, or contains
            non-finite values.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import TimeGrid
        >>> TimeGrid.from_times([0.0, 0.25, 0.5, 1.0]).num_steps
        3
        """
        ...

    @property
    def num_steps(self) -> int:
        """
        Number of time steps on the grid.

        Returns
        -------
        int
            Step count.

            The num steps exposed by this `TimeGrid`.
        Examples
        --------
        >>> from finstack_quant.monte_carlo import TimeGrid
        >>> TimeGrid(1.0, 100).num_steps
        100
        """
        ...

    @property
    def t_max(self) -> float:
        """
        Terminal time of the grid.

        Returns
        -------
        float
            Maximum time coordinate.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import TimeGrid
        >>> TimeGrid(2.0, 8).t_max
        2.0
        """
        ...

    @property
    def is_uniform(self) -> bool:
        """
        Whether step sizes are uniform.

        Returns
        -------
        bool
            ``True`` if all inner steps share one ``dt``.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import TimeGrid
        >>> TimeGrid(1.0, 5).is_uniform
        True
        """
        ...

    @property
    def times(self) -> list[float]:
        """
        All time coordinates including the origin.

        Returns
        -------
        list[float]
            Copy of knot times.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import TimeGrid
        >>> TimeGrid(1.0, 2).times[0]
        0.0
        """
        ...

    @property
    def dts(self) -> list[float]:
        """
        Step sizes between consecutive times.

        Returns
        -------
        list[float]
            Per-step ``dt`` values.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import TimeGrid
        >>> len(TimeGrid(1.0, 4).dts)
            4
        """
        ...

    def time(self, step: int) -> float:
        """
        Time at a given step index.

        Parameters
        ----------
        step : int
            Step index in ``[0, num_steps]``.

        Returns
        -------
        float
            Time coordinate.

        Raises
        ------
        IndexError
            If ``step`` is out of bounds.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import TimeGrid
        >>> TimeGrid(1.0, 4).time(0)
            0.0
        """
        ...

    def dt(self, step: int) -> float:
        """
        Step size following the given step index.

        Parameters
        ----------
        step : int
            Step index in ``[0, num_steps - 1]``.

        Returns
        -------
        float
            Increment to the next time.

        Raises
        ------
        IndexError
            If ``step`` is out of bounds.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import TimeGrid
        >>> TimeGrid(1.0, 4).dt(0)
            0.25
        """
        ...

class GbmPathSummary:
    """
    Compact captured GBM spot paths.

    Examples
    --------
    >>> from finstack_quant.monte_carlo import GbmPathSummary
    >>> GbmPathSummary.__name__
    'GbmPathSummary'
    """

    @property
    def num_paths(self) -> int:
        """
        Return the num paths for `GbmPathSummary`.

        Returns
        -------
        int
            The num paths exposed by this `GbmPathSummary`.
        """
        ...

    @property
    def num_simulated_paths(self) -> int:
        """
        Return the num simulated paths for `GbmPathSummary`.

        Returns
        -------
        int
            The num simulated paths exposed by this `GbmPathSummary`.
        """
        ...

    @property
    def times(self) -> list[float]:
        """
        Return the times for `GbmPathSummary`.

        Returns
        -------
        list[float]
            The times exposed by this `GbmPathSummary`.
        """
        ...

    @property
    def paths(self) -> list[list[float]]:
        """
        Return the paths for `GbmPathSummary`.

        Returns
        -------
        list[list[float]]
            The paths exposed by this `GbmPathSummary`.
        """
        ...

class McEngine:
    """
    Full Monte Carlo engine bound to a :class:`TimeGrid`.

    Examples
    --------
    >>> from finstack_quant.monte_carlo import McEngine, TimeGrid
    >>> McEngine(100, TimeGrid(1.0, 50), seed=7).price_european_call(100, 100, 0.05, 0.0, 0.2).num_paths
    100
    """

    def __init__(
        self,
        num_paths: int,
        time_grid: TimeGrid,
        seed: int | None = None,
        use_parallel: bool | None = None,
        antithetic: bool | None = None,
    ) -> None:
        """
        Create a Monte Carlo engine.

        Parameters
        ----------
        num_paths : int
            Number of independent estimators. Without antithetic pairing this
            is also the simulated-path count; with pairing, each estimator uses
            two simulated paths.
        time_grid : TimeGrid
            Discretisation grid for path generation.
        seed : int, optional
            RNG seed. Defaults to the registry default (``42``).
        use_parallel : bool, optional
            Enable parallel path generation. Defaults to ``False``.
        antithetic : bool, optional
            Enable antithetic pairing. This preserves ``num_paths`` as the
            estimator count and simulates ``2 * num_paths`` paths. Antithetic
            pairing is incompatible with path capture.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import McEngine, TimeGrid
        >>> McEngine(10, TimeGrid(1.0, 5), seed=1, use_parallel=True)  # doctest: +ELLIPSIS
        McEngine(...)

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def price_european_call(
        self,
        spot: float,
        strike: float,
        rate: float,
        div_yield: float,
        vol: float,
        currency: str | None = None,
    ) -> MoneyEstimate:
        """
        Price a European call on the engine's grid under GBM.

        Parameters
        ----------
        spot : float
            Initial spot price.
        strike : float
            Strike price.
        rate : float
            Risk-free rate (continuously compounded decimal).
        div_yield : float
            Dividend yield (continuously compounded decimal).
        vol : float
            Volatility (decimal).
        currency : str, optional
            ISO currency code. Defaults to USD.

        Returns
        -------
        MoneyEstimate
            Priced result with mean, stderr, and confidence bands.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import McEngine, TimeGrid
        >>> McEngine(500, TimeGrid(1.0, 52)).price_european_call(100, 100, 0.05, 0.0, 0.25).num_paths
        500

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def price_european_put(
        self,
        spot: float,
        strike: float,
        rate: float,
        div_yield: float,
        vol: float,
        currency: str | None = None,
    ) -> MoneyEstimate:
        """
        Price a European put on the engine's grid under GBM.

        Parameters
        ----------
        spot : float
            Initial spot price.
        strike : float
            Strike price.
        rate : float
            Risk-free rate (continuously compounded decimal).
        div_yield : float
            Dividend yield (continuously compounded decimal).
        vol : float
            Volatility (decimal).
        currency : str, optional
            ISO currency code. Defaults to USD.

        Returns
        -------
        MoneyEstimate
            Priced result with mean, stderr, and confidence bands.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import McEngine, TimeGrid
        >>> McEngine(500, TimeGrid(1.0, 52)).price_european_put(100, 100, 0.05, 0.0, 0.25).num_paths
        500

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

def simulate_gbm_paths(
    spot: float,
    rate: float,
    div_yield: float,
    vol: float,
    expiry: float,
    num_steps: int,
    num_paths: int,
    seed: int | None = None,
    antithetic: bool = False,
) -> GbmPathSummary:
    """
    Simulate compact GBM spot paths through Rust path capture.

    ``num_paths`` is the estimator and simulated-path count because captured
    paths do not support antithetic pairing. Passing ``antithetic=True`` raises
    ``ValueError``.

    Parameters
    ----------
    spot : float
        Positive initial underlying price in the output path's price units.
    rate : float
        Continuously compounded annual risk-free rate as a decimal.
    div_yield : float
        Continuously compounded annual dividend or carry yield as a decimal.
    vol : float
        Positive annualized GBM volatility as a decimal, such as ``0.20``.
    expiry : float
        Positive time to maturity in years.
    num_steps : int
        Number of equally spaced simulation steps over the expiry horizon.
    num_paths : int
        Number of independently simulated paths retained in the summary.
    seed : int or None, default None
        Optional deterministic random seed; ``None`` uses the runtime generator.
    antithetic : bool, default False
        Antithetic-path request. This compact path API rejects ``True``.

    Returns
    -------
    GbmPathSummary
        Result of simulate gbm paths for the binding in the annotated representation.

    Raises
    ------
    ValueError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.monte_carlo import simulate_gbm_paths
    >>> callable(simulate_gbm_paths)
    True
    """
    ...

def heston_satisfies_feller(kappa: float, theta: float, vol_of_vol: float) -> bool:
    """
    Validate Heston parameters and test the strict Feller condition.

    Parameters
    ----------
    kappa : float
        Positive mean-reversion speed of the variance process per year.
    theta : float
        Positive long-run variance level in squared-volatility units.
    vol_of_vol : float
        Positive annualized volatility of the variance process.

    Returns
    -------
    bool
        Result of heston satisfies feller for the binding in the annotated representation.

    Raises
    ------
    ValueError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.monte_carlo import heston_satisfies_feller
    >>> callable(heston_satisfies_feller)
    True
    """
    ...

class EuropeanPricer:
    """
    European-option Monte Carlo pricer under GBM (exact time-stepping).

    Examples
    --------
    >>> from finstack_quant.monte_carlo import EuropeanPricer
    >>> EuropeanPricer(num_paths=1000, seed=1).price_call(100, 100, 0.05, 0.0, 0.2, 1.0).num_paths
    1000
    """

    def __init__(
        self,
        num_paths: int | None = None,
        seed: int | None = None,
        use_parallel: bool | None = None,
    ) -> None:
        """
        Create a European-option pricer.

        Parameters
        ----------
        num_paths : int, optional
            Path count. Defaults to the registry default (``100_000``).
        seed : int, optional
            RNG seed. Defaults to the registry default (``42``).
        use_parallel : bool, optional
            Parallel accumulation flag. Defaults to the registry default.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import EuropeanPricer
        >>> EuropeanPricer(500, 9).seed
        9

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def num_paths(self) -> int:
        """
        Configured path count.

        Returns
        -------
        int
            Number of Monte Carlo paths.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import EuropeanPricer
        >>> EuropeanPricer(1234).num_paths
        1234
        """
        ...

    @property
    def seed(self) -> int:
        """
        Return the seed for `EuropeanPricer`.
        RNG seed.

        Returns
        -------
        int
            Seed value used for path generation.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import EuropeanPricer
        >>> EuropeanPricer(seed=55).seed
        55
        """
        ...

    @property
    def use_parallel(self) -> bool:
        """
        Whether path accumulation runs on the rayon pool.

        Returns
        -------
        bool
            Parallel flag as passed to ``__init__``.
        """
        ...

    def price_call(
        self,
        spot: float,
        strike: float,
        rate: float,
        div_yield: float,
        vol: float,
        expiry: float,
        num_steps: int | None = None,
        currency: str | None = None,
    ) -> MoneyEstimate:
        """
        Price a European call.

        Parameters
        ----------
        spot : float
            Spot price.
        strike : float
            Strike price.
        rate : float
            Risk-free rate (continuously compounded decimal).
        div_yield : float
            Dividend yield (continuously compounded decimal).
        vol : float
            Volatility (decimal).
        expiry : float
            Time to maturity in years.
        num_steps : int, optional
            Time steps. Defaults to the registry default (``252``).
        currency : str, optional
            ISO currency code. Defaults to USD.

        Returns
        -------
        MoneyEstimate
            Monte Carlo price with stderr and confidence bands.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import EuropeanPricer
        >>> EuropeanPricer(800, 0).price_call(100, 100, 0.05, 0.0, 0.2, 1.0, num_steps=52).num_paths
        800

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def price_put(
        self,
        spot: float,
        strike: float,
        rate: float,
        div_yield: float,
        vol: float,
        expiry: float,
        num_steps: int | None = None,
        currency: str | None = None,
    ) -> MoneyEstimate:
        """
        Price a European put.

        Parameters
        ----------
        spot : float
            Spot price.
        strike : float
            Strike price.
        rate : float
            Risk-free rate (continuously compounded decimal).
        div_yield : float
            Dividend yield (continuously compounded decimal).
        vol : float
            Volatility (decimal).
        expiry : float
            Time to maturity in years.
        num_steps : int, optional
            Time steps. Defaults to the registry default (``252``).
        currency : str, optional
            ISO currency code. Defaults to USD.

        Returns
        -------
        MoneyEstimate
            Monte Carlo price with stderr and confidence bands.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import EuropeanPricer
        >>> EuropeanPricer(800, 0).price_put(100, 100, 0.05, 0.0, 0.2, 1.0, num_steps=52).num_paths
        800

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

class PathDependentPricer:
    """
    Path-dependent Monte Carlo pricer (Asian-style exotics on GBM).

    Examples
    --------
    >>> from finstack_quant.monte_carlo import PathDependentPricer
    >>> PathDependentPricer(600, 2).price_asian_call(100, 100, 0.05, 0.0, 0.2, 1.0).num_paths
    600
    """

    def __init__(
        self,
        num_paths: int | None = None,
        seed: int | None = None,
        use_parallel: bool | None = None,
    ) -> None:
        """
        Create a path-dependent pricer.

        Parameters
        ----------
        num_paths : int, optional
            Path count. Defaults to the registry default.
        seed : int, optional
            RNG seed. Defaults to the registry default.
        use_parallel : bool, optional
            Parallel accumulation flag. Defaults to the registry default.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import PathDependentPricer
        >>> PathDependentPricer(100, 1, use_parallel=True).num_paths
        100

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def price_asian_call(
        self,
        spot: float,
        strike: float,
        rate: float,
        div_yield: float,
        vol: float,
        expiry: float,
        num_steps: int | None = None,
        currency: str | None = None,
    ) -> MoneyEstimate:
        """
        Price an arithmetic Asian call (post-initial fixings at every step).

        Parameters
        ----------
        spot : float
            Spot price.
        strike : float
            Strike price.
        rate : float
            Risk-free rate (continuously compounded decimal).
        div_yield : float
            Dividend yield (continuously compounded decimal).
        vol : float
            Volatility (decimal).
        expiry : float
            Maturity in years.
        num_steps : int, optional
            Steps. Defaults to the registry default. The default fixing
            schedule is steps ``1..=num_steps`` and excludes the initial spot
            at step ``0``.
        currency : str, optional
            ISO currency code. Defaults to USD.

        Returns
        -------
        MoneyEstimate
            Monte Carlo price with stderr and confidence bands.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import PathDependentPricer
        >>> PathDependentPricer(400, 0).price_asian_call(100, 100, 0.05, 0.0, 0.2, 1.0, num_steps=12).num_paths
        400

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def price_asian_put(
        self,
        spot: float,
        strike: float,
        rate: float,
        div_yield: float,
        vol: float,
        expiry: float,
        num_steps: int | None = None,
        currency: str | None = None,
    ) -> MoneyEstimate:
        """
        Price an arithmetic Asian put (post-initial fixings at every step).

        Parameters
        ----------
        spot : float
            Spot price.
        strike : float
            Strike price.
        rate : float
            Risk-free rate (continuously compounded decimal).
        div_yield : float
            Dividend yield (continuously compounded decimal).
        vol : float
            Volatility (decimal).
        expiry : float
            Maturity in years.
        num_steps : int, optional
            Steps. Defaults to the registry default. The default fixing
            schedule is steps ``1..=num_steps`` and excludes the initial spot
            at step ``0``.
        currency : str, optional
            ISO currency code. Defaults to USD.

        Returns
        -------
        MoneyEstimate
            Monte Carlo price with stderr and confidence bands.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import PathDependentPricer
        >>> PathDependentPricer(400, 0).price_asian_put(100, 100, 0.05, 0.0, 0.2, 1.0, num_steps=12).num_paths
        400

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def num_paths(self) -> int:
        """
        Configured path count.

        Returns
        -------
        int
            Number of Monte Carlo paths.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import PathDependentPricer
        >>> PathDependentPricer(777).num_paths
        777
        """
        ...

    @property
    def seed(self) -> int:
        """
        Return the seed for `PathDependentPricer`.
        RNG seed.

        Returns
        -------
        int
            Seed value used for path generation.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import PathDependentPricer
        >>> PathDependentPricer(seed=44).seed
        44
        """
        ...

class LsmcPricer:
    """
    Longstaff–Schwartz Monte Carlo pricer for American options under GBM.

    Examples
    --------
    >>> from finstack_quant.monte_carlo import LsmcPricer
    >>> LsmcPricer(300, 0).price_american_put(100, 100, 0.05, 0.0, 0.3, 1.0, num_steps=10).num_paths
    300
    """

    def __init__(
        self,
        num_paths: int | None = None,
        seed: int | None = None,
        use_parallel: bool | None = None,
        basis: str | None = None,
        basis_degree: int | None = None,
    ) -> None:
        """
        Create an LSMC pricer.

        Parameters
        ----------
        num_paths : int, optional
            Path count. Defaults to the registry default.
        seed : int, optional
            RNG seed. Defaults to the registry default.
        use_parallel : bool, optional
            Parallel path generation flag. Defaults to the registry default.
        basis : str, optional
            Regression basis family. One of ``"laguerre"``,
            ``"polynomial"``, or ``"normalized_polynomial"``. Defaults to
            the registry default.
        basis_degree : int, optional
            Polynomial/Laguerre degree. Defaults to the registry default.
            Must be positive; for ``"laguerre"`` it must additionally be
            in ``[1, 4]``.

        Raises
        ------
        ValueError
            If ``basis`` is not a recognized family or ``basis_degree`` is
            out of range.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import LsmcPricer
        >>> LsmcPricer(50, 3).num_paths
        50
        """
        ...

    @property
    def num_paths(self) -> int:
        """
        Configured path count.

        Returns
        -------
        int
            Number of Monte Carlo paths.
        """
        ...

    @property
    def seed(self) -> int:
        """
        Return the seed for `LsmcPricer`.
        RNG seed.

        Returns
        -------
        int
            Seed value used for path generation.
        """
        ...

    @property
    def use_parallel(self) -> bool:
        """
        Whether path generation runs on the rayon pool.

        Returns
        -------
        bool
            Parallel flag as passed to ``__init__``.
        """
        ...

    @property
    def basis(self) -> str:
        """
        Regression basis family name.

        Returns
        -------
        str
            One of ``"laguerre"``, ``"polynomial"``,
            ``"normalized_polynomial"``.
        """
        ...

    @property
    def basis_degree(self) -> int:
        """
        Configured polynomial/Laguerre degree.

        Returns
        -------
        int
            Degree value used in the regression basis.
        """
        ...

    def price_american_put(
        self,
        spot: float,
        strike: float,
        rate: float,
        div_yield: float,
        vol: float,
        expiry: float,
        num_steps: int | None = None,
        currency: str | None = None,
    ) -> MoneyEstimate:
        """
        Price an American put via LSMC.

        Parameters
        ----------
        spot : float
            Spot price.
        strike : float
            Strike price.
        rate : float
            Risk-free rate (continuously compounded decimal).
        div_yield : float
            Dividend yield (continuously compounded decimal).
        vol : float
            Volatility (decimal).
        expiry : float
            Maturity in years.
        num_steps : int, optional
            Exercise grid steps. Defaults to the registry default.
        currency : str, optional
            ISO currency code. Defaults to USD.

        Returns
        -------
        MoneyEstimate
            LSMC price with stderr and confidence bands.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import LsmcPricer
        >>> LsmcPricer(200, 0).price_american_put(100, 100, 0.05, 0.0, 0.25, 1.0, num_steps=8).num_paths
        200

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def price_american_call(
        self,
        spot: float,
        strike: float,
        rate: float,
        div_yield: float,
        vol: float,
        expiry: float,
        num_steps: int | None = None,
        currency: str | None = None,
    ) -> MoneyEstimate:
        """
        Price an American call via LSMC.

        Parameters
        ----------
        spot : float
            Spot price.
        strike : float
            Strike price.
        rate : float
            Risk-free rate (continuously compounded decimal).
        div_yield : float
            Dividend yield (continuously compounded decimal).
        vol : float
            Volatility (decimal).
        expiry : float
            Maturity in years.
        num_steps : int, optional
            Exercise grid steps. Defaults to the registry default.
        currency : str, optional
            ISO currency code. Defaults to USD.

        Returns
        -------
        MoneyEstimate
            LSMC price with stderr and confidence bands.

        Examples
        --------
        >>> from finstack_quant.monte_carlo import LsmcPricer
        >>> LsmcPricer(200, 0).price_american_call(100, 100, 0.05, 0.0, 0.25, 1.0, num_steps=8).num_paths
        200

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def price_american_put_unbiased(
        self,
        spot: float,
        strike: float,
        rate: float,
        div_yield: float,
        vol: float,
        expiry: float,
        pricing_seed: int,
        num_steps: int | None = None,
        currency: str | None = None,
    ) -> MoneyEstimate:
        """
        Two-pass unbiased American put price.

        Mitigates the in-sample upward bias of single-pass LSMC by fitting
        the regression on a training path set seeded by the pricer's ``seed``
        and pricing on an independent path set seeded by ``pricing_seed``.

        Parameters
        ----------
        spot : float
            Spot price.
        strike : float
            Strike price.
        rate : float
            Risk-free rate (continuously compounded decimal).
        div_yield : float
            Dividend yield (continuously compounded decimal).
        vol : float
            Volatility (decimal).
        expiry : float
            Maturity in years.
        pricing_seed : int
            Seed for the pricing pass; must differ from the pricer's training
            seed (passing the same value reintroduces the in-sample bias and
            is rejected).
        num_steps : int, optional
            Exercise grid steps. Defaults to the registry default.
        currency : str, optional
            ISO currency code. Defaults to USD.

        Returns
        -------
        MoneyEstimate
            Out-of-sample price with stderr and confidence bands.

        Raises
        ------
        ValueError
            If ``pricing_seed`` equals the pricer's training seed.
        """
        ...

    def price_american_call_unbiased(
        self,
        spot: float,
        strike: float,
        rate: float,
        div_yield: float,
        vol: float,
        expiry: float,
        pricing_seed: int,
        num_steps: int | None = None,
        currency: str | None = None,
    ) -> MoneyEstimate:
        """
        Two-pass unbiased American call price.

        See :meth:`price_american_put_unbiased` for the bias-mitigation
        rationale and the meaning of ``pricing_seed``.

        Parameters
        ----------
        spot : float
            Spot price.
        strike : float
            Strike price.
        rate : float
            Risk-free rate (continuously compounded decimal).
        div_yield : float
            Dividend yield (continuously compounded decimal).
        vol : float
            Volatility (decimal).
        expiry : float
            Maturity in years.
        pricing_seed : int
            Seed for the pricing pass; must differ from the pricer's training
            seed.
        num_steps : int, optional
            Exercise grid steps. Defaults to the registry default.
        currency : str, optional
            ISO currency code. Defaults to USD.

        Returns
        -------
        MoneyEstimate
            Out-of-sample price with stderr and confidence bands.

        Raises
        ------
        ValueError
            If ``pricing_seed`` equals the pricer's training seed.
        """
        ...

def black_scholes_call(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    vol: float,
    expiry: float,
) -> float:
    """
    Black–Scholes European call present value under GBM.

    Uses continuously compounded ``rate`` and ``div_yield`` with volatility
    quoted in decimal form. This is a closed-form option price, not a raw
    terminal payoff.

    Parameters
    ----------
    spot : float
        Spot price.
    strike : float
        Strike price.
    rate : float
        Risk-free rate (continuously compounded decimal).
    div_yield : float
        Dividend yield (continuously compounded decimal).
    vol : float
        Volatility (decimal).
    expiry : float
        Time to maturity in years.

    Returns
    -------
    float
        Present value of the European call.

    Raises
    ------
    ValueError
        If any parameter is non-finite or ``expiry`` is negative.

    Examples
    --------
    >>> from finstack_quant.monte_carlo import black_scholes_call
    >>> black_scholes_call(100, 100, 0.05, 0.0, 0.2, 1.0) > 0
    True
    """
    ...

def black_scholes_put(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    vol: float,
    expiry: float,
) -> float:
    """
    Black–Scholes European put present value under GBM.

    Uses continuously compounded ``rate`` and ``div_yield`` with volatility
    quoted in decimal form. This is a closed-form option price, not a raw
    terminal payoff.

    Parameters
    ----------
    spot : float
        Spot price.
    strike : float
        Strike price.
    rate : float
        Risk-free rate (continuously compounded decimal).
    div_yield : float
        Dividend yield (continuously compounded decimal).
    vol : float
        Volatility (decimal).
    expiry : float
        Time to maturity in years.

    Returns
    -------
    float
        Present value of the European put.

    Raises
    ------
    ValueError
        If any parameter is non-finite or ``expiry`` is negative.

    Examples
    --------
    >>> from finstack_quant.monte_carlo import black_scholes_put
    >>> black_scholes_put(100, 100, 0.05, 0.0, 0.2, 1.0) > 0
    True
    """
    ...

def price_european_call(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    vol: float,
    expiry: float,
    num_paths: int | None = None,
    seed: int | None = None,
    num_steps: int | None = None,
    currency: str | None = None,
) -> MoneyEstimate:
    """
    Monte Carlo European call under GBM (standalone convenience).

    Parameters
    ----------
    spot : float
        Spot price.
    strike : float
        Strike price.
    rate : float
        Risk-free rate (continuously compounded decimal).
    div_yield : float
        Dividend yield (continuously compounded decimal).
    vol : float
        Volatility (decimal).
    expiry : float
        Maturity in years.
    num_paths : int, optional
        Paths (default ``100_000``).
    seed : int, optional
        RNG seed (default ``42``).
    num_steps : int, optional
        Time steps (default ``252``).
    currency : str, optional
        ISO currency code. Defaults to USD.

    Returns
    -------
    MoneyEstimate
        Monte Carlo price with stderr and confidence bands.

    Examples
    --------
    >>> from finstack_quant.monte_carlo import price_european_call
    >>> price_european_call(100, 100, 0.05, 0.0, 0.2, 1.0, num_paths=2000).num_paths
    2000

    Raises
    ------
    ValueError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
    """
    ...

def price_european_put(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    vol: float,
    expiry: float,
    num_paths: int | None = None,
    seed: int | None = None,
    num_steps: int | None = None,
    currency: str | None = None,
) -> MoneyEstimate:
    """
    Monte Carlo European put under GBM (standalone convenience).

    Parameters
    ----------
    spot : float
        Spot price.
    strike : float
        Strike price.
    rate : float
        Risk-free rate (continuously compounded decimal).
    div_yield : float
        Dividend yield (continuously compounded decimal).
    vol : float
        Volatility (decimal).
    expiry : float
        Maturity in years.
    num_paths : int, optional
        Paths (default ``100_000``).
    seed : int, optional
        RNG seed (default ``42``).
    num_steps : int, optional
        Time steps (default ``252``).
    currency : str, optional
        ISO currency code. Defaults to USD.

    Returns
    -------
    MoneyEstimate
        Monte Carlo price with stderr and confidence bands.

    Examples
    --------
    >>> from finstack_quant.monte_carlo import price_european_put
    >>> price_european_put(100, 100, 0.05, 0.0, 0.2, 1.0, num_paths=2000).num_paths
    2000

    Raises
    ------
    ValueError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
    """
    ...

def price_heston_call(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    kappa: float,
    theta: float,
    vol_of_vol: float,
    rho: float,
    v0: float,
    expiry: float,
    num_paths: int | None = None,
    seed: int | None = None,
    num_steps: int | None = None,
    currency: str | None = None,
) -> MoneyEstimate:
    """
    Monte Carlo European call under Heston stochastic volatility.

    Simulates spot and variance with the QE Heston discretization. Rates and
    dividend yield are continuously compounded decimals; Heston parameters follow
    the standard square-root variance specification.

    Parameters
    ----------
    spot : float
        Initial spot price.
    strike : float
        Strike price.
    rate : float
        Risk-free rate as a decimal.
    div_yield : float
        Dividend yield as a decimal.
    kappa : float
        Mean-reversion speed of variance.
    theta : float
        Long-run variance level.
    vol_of_vol : float
        Volatility of variance (``sigma`` in Heston notation).
    rho : float
        Correlation between spot and variance Brownian motions in ``[-1, 1]``.
    v0 : float
        Initial variance (not volatility).
    expiry : float
        Time to maturity in years.
    num_paths : int, optional
        Path count (registry default ``100_000``).
    seed : int, optional
        RNG seed (registry default ``42``).
    num_steps : int, optional
        Time steps per path (registry default ``252``).
    currency : str, optional
        ISO currency code; defaults to USD.

    Returns
    -------
    MoneyEstimate
        Discounted Monte Carlo price with stderr and confidence bands.

    Raises
    ------
    ValueError
        If parameters are non-finite or violate Feller / positivity constraints.

    Sources
    -------
    See ``docs/REFERENCES.md#heston-1993``.

    Examples
    --------
    >>> from finstack_quant.monte_carlo import price_heston_call
    >>> r = price_heston_call(100, 100, 0.05, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04, 1.0, num_paths=5000)
    >>> r.num_paths
    5000
    """
    ...

def price_heston_put(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    kappa: float,
    theta: float,
    vol_of_vol: float,
    rho: float,
    v0: float,
    expiry: float,
    num_paths: int | None = None,
    seed: int | None = None,
    num_steps: int | None = None,
    currency: str | None = None,
) -> MoneyEstimate:
    """
    Monte Carlo European put under Heston stochastic volatility.

    Same conventions as :func:`price_heston_call` but pays ``max(K - S_T, 0)``.

    Parameters
    ----------
    spot : float
        Positive initial underlying price in the requested currency units.
    strike : float
        Positive put strike in the same price units as ``spot``.
    rate : float
        Continuously compounded annual risk-free rate as a decimal.
    div_yield : float
        Continuously compounded annual dividend or carry yield as a decimal.
    kappa : float
        Positive mean-reversion speed of the Heston variance process per year.
    theta : float
        Positive long-run variance level in squared-volatility units.
    vol_of_vol : float
        Positive annualized volatility of the Heston variance process.
    rho : float
        Spot/variance Brownian correlation in the closed interval ``[-1, 1]``.
    v0 : float
        Positive initial variance, not initial volatility.
    expiry : float
        Positive time to the European put expiry in years.
    num_paths : int or None, default None
        Optional number of Monte Carlo paths; ``None`` selects the engine default.
    seed : int or None, default None
        Optional deterministic random seed for reproducible path generation.
    num_steps : int or None, default None
        Optional number of time steps; ``None`` selects the engine default grid.
    currency : str or None, default None
        ISO-4217 output currency tag; ``None`` applies the binding default.

    Returns
    -------
    MoneyEstimate
        Discounted Monte Carlo put price.

    Raises
    ------
    ValueError
        If parameters are invalid.

    Sources
    -------
    See ``docs/REFERENCES.md#heston-1993``.

    Examples
    --------
    >>> from finstack_quant.monte_carlo import price_heston_put
    >>> r = price_heston_put(100, 100, 0.05, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04, 1.0, num_paths=5000)
    >>> r.mean.amount > 0
    True
    """
    ...

def finite_diff_delta(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    vol: float,
    expiry: float,
    num_paths: int | None = None,
    seed: int | None = None,
    num_steps: int | None = None,
    bump_size: float | None = None,
    option_type: str | None = None,
    currency: str | None = None,
) -> tuple[float, float]:
    """
    Finite-difference delta for a European option (independence-bound stderr).

    Reports a conservative upper bound on the standard error that treats
    the bumped and base runs as if they were statistically independent.
    For hedge-ratio sizing prefer :func:`finite_diff_delta_crn`, which returns the
    tighter paired CRN stderr.

    Parameters
    ----------
    spot : float
        Spot price.
    strike : float
        Strike price.
    rate : float
        Risk-free rate (continuously compounded decimal).
    div_yield : float
        Dividend yield (continuously compounded decimal).
    vol : float
        Volatility (decimal).
    expiry : float
        Maturity in years.
    num_paths : int, optional
        Paths per evaluation (default ``10_000``).
    seed : int, optional
        RNG seed (default ``42``).
    num_steps : int, optional
        Time-grid steps (default ``50``).
    bump_size : float, optional
        Relative bump fraction of spot (default ``0.01``).
    option_type : str, optional
        ``"call"`` or ``"put"``.
    currency : str, optional
        ISO currency code. Defaults to USD.

    Returns
    -------
    tuple[float, float]
        ``(delta, stderr)``.

    Raises
    ------
    ValueError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.monte_carlo import finite_diff_delta
    >>> callable(finite_diff_delta)
    True
    """
    ...

def finite_diff_delta_crn(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    vol: float,
    expiry: float,
    num_paths: int | None = None,
    seed: int | None = None,
    num_steps: int | None = None,
    bump_size: float | None = None,
    option_type: str | None = None,
    currency: str | None = None,
) -> tuple[float, float]:
    """
    Finite-difference delta with paired common-random-number stderr.

    Computes per-path paired differences and reports their true standard
    error, which exploits CRN cancellation and is typically 1–2 orders of
    magnitude tighter than the independence bound returned by
    :func:`finite_diff_delta`. Always runs serially.

    Parameters
    ----------
    spot : float
        Spot price.
    strike : float
        Strike price.
    rate : float
        Risk-free rate (continuously compounded decimal).
    div_yield : float
        Dividend yield (continuously compounded decimal).
    vol : float
        Volatility (decimal).
    expiry : float
        Maturity in years.
    num_paths : int, optional
        Paths per evaluation (default ``10_000``).
    seed : int, optional
        RNG seed (default ``42``).
    num_steps : int, optional
        Time-grid steps (default ``50``).
    bump_size : float, optional
        Relative bump fraction of spot (default ``0.01``).
    option_type : str, optional
        ``"call"`` or ``"put"``.
    currency : str, optional
        ISO currency code. Defaults to USD.

    Returns
    -------
    tuple[float, float]
        ``(delta, paired_stderr)``.

    Raises
    ------
    ValueError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.monte_carlo import finite_diff_delta_crn
    >>> callable(finite_diff_delta_crn)
    True
    """
    ...

def finite_diff_gamma(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    vol: float,
    expiry: float,
    num_paths: int | None = None,
    seed: int | None = None,
    num_steps: int | None = None,
    bump_size: float | None = None,
    option_type: str | None = None,
    currency: str | None = None,
) -> tuple[float, float]:
    """
    Finite-difference gamma (independence-bound stderr).

    See :func:`finite_diff_gamma_crn` for the tighter paired CRN variant.

    Parameters
    ----------
    spot : float
        Spot price.
    strike : float
        Strike price.
    rate : float
        Risk-free rate (continuously compounded decimal).
    div_yield : float
        Dividend yield (continuously compounded decimal).
    vol : float
        Volatility (decimal).
    expiry : float
        Maturity in years.
    num_paths : int, optional
        Paths per evaluation (default ``10_000``).
    seed : int, optional
        RNG seed (default ``42``).
    num_steps : int, optional
        Time-grid steps (default ``50``).
    bump_size : float, optional
        Relative bump fraction of spot (default ``0.01``).
    option_type : str, optional
        ``"call"`` or ``"put"``.
    currency : str, optional
        ISO currency code. Defaults to USD.

    Returns
    -------
    tuple[float, float]
        ``(gamma, stderr)``.

    Raises
    ------
    ValueError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.monte_carlo import finite_diff_gamma
    >>> callable(finite_diff_gamma)
    True
    """
    ...

def finite_diff_gamma_crn(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    vol: float,
    expiry: float,
    num_paths: int | None = None,
    seed: int | None = None,
    num_steps: int | None = None,
    bump_size: float | None = None,
    option_type: str | None = None,
    currency: str | None = None,
) -> tuple[float, float]:
    """
    Finite-difference gamma with paired common-random-number stderr.

    Returns ``(gamma, paired_stderr)`` where the standard error is the
    per-path paired error of ``(V_up_i − 2 V_base_i + V_down_i) / h²``.
    Always runs serially.

    Parameters
    ----------
    spot : float
        Spot price.
    strike : float
        Strike price.
    rate : float
        Risk-free rate (continuously compounded decimal).
    div_yield : float
        Dividend yield (continuously compounded decimal).
    vol : float
        Volatility (decimal).
    expiry : float
        Maturity in years.
    num_paths : int, optional
        Paths per evaluation (default ``10_000``).
    seed : int, optional
        RNG seed (default ``42``).
    num_steps : int, optional
        Time-grid steps (default ``50``).
    bump_size : float, optional
        Relative bump fraction of spot (default ``0.01``).
    option_type : str, optional
        ``"call"`` or ``"put"``.
    currency : str, optional
        ISO currency code. Defaults to USD.

    Returns
    -------
    tuple[float, float]
        ``(gamma, paired_stderr)``.

    Raises
    ------
    ValueError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.monte_carlo import finite_diff_gamma_crn
    >>> callable(finite_diff_gamma_crn)
    True
    """
    ...
