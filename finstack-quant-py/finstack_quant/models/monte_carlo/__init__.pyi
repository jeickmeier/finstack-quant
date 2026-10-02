"""
Monte Carlo convenience bindings (``finstack-quant-models``).

Exposes simulation primitives: time grids, engine configuration, pricers,
closed-form Black-Scholes helpers, and selected non-GBM process wrappers.
Advanced Rust process, discretization, RNG, payoff, and Greeks types are not
surfaced as standalone Python types yet; their parameters are passed directly
as numeric arguments to the exposed pricer constructors and methods.

Examples
--------
>>> from finstack_quant.models.monte_carlo import heston_satisfies_feller
>>> heston_satisfies_feller(2.0, 0.04, 0.3)
True
"""

from __future__ import annotations

from collections.abc import Sequence

import pandas as pd

from finstack_quant.core.money import Money

__all__ = [
    "Estimate",
    "EuropeanPricer",
    "GbmPathSummary",
    "LsmcPricer",
    "MoneyEstimate",
    "PathDependentPricer",
    "finite_diff_delta",
    "finite_diff_delta_crn",
    "finite_diff_gamma",
    "finite_diff_gamma_crn",
    "heston_satisfies_feller",
    "price_heston_call",
    "price_heston_put",
    "simulate_gbm_paths",
]

class MoneyEstimate:
    """
    Discounted Monte Carlo estimate with money units and confidence bands.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import EuropeanPricer
    >>> r = EuropeanPricer(10_000, seed=42).price_call(100, 100, 0.05, 0.0, 0.2, 1.0)
    >>> r.num_paths
    10000
    """

    @staticmethod
    def from_json(json: str) -> MoneyEstimate:
        """
        Deserialize a ``MoneyEstimate`` from JSON.

        Parameters
        ----------
        json : str
            JSON string produced by :meth:`to_json`.

        Returns
        -------
        MoneyEstimate
            Parsed ``MoneyEstimate`` instance.

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not satisfy the serialized schema.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import EuropeanPricer, MoneyEstimate
        >>> priced = EuropeanPricer(10_000, seed=42).price_call(100, 100, 0.05, 0.0, 0.2, 1.0)
        >>> MoneyEstimate.from_json(priced.to_json()).num_paths
        10000
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def mean(self) -> Money:
        """
        Discounted mean present value.

        Returns
        -------
        Money
            Mean PV with currency tag.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import EuropeanPricer
        >>> pricer = EuropeanPricer(1000, seed=42)
        >>> pricer.price_call(100, 100, 0.05, 0.0, 0.2, 1.0).mean.amount > 0
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import EuropeanPricer
        >>> pricer = EuropeanPricer(1000, seed=42)
        >>> pricer.price_call(100, 100, 0.05, 0.0, 0.2, 1.0).stderr >= 0
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import EuropeanPricer
        >>> r = EuropeanPricer(2000, seed=42).price_call(100, 100, 0.05, 0.0, 0.2, 1.0)
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import EuropeanPricer
        >>> r = EuropeanPricer(2000, seed=42).price_call(100, 100, 0.05, 0.0, 0.2, 1.0)
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import EuropeanPricer
        >>> EuropeanPricer(1234, seed=42).price_call(100, 100, 0.05, 0.0, 0.2, 1.0).num_paths
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def relative_stderr(self) -> float:
        """
        Relative standard error (stderr divided by absolute mean amount).

        Returns
        -------
        float
            Dimensionless relative stderr.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import EuropeanPricer
        >>> pricer = EuropeanPricer(5000, seed=42)
        >>> pricer.price_call(100, 100, 0.05, 0.0, 0.2, 1.0).relative_stderr() >= 0
        True
        """
        ...

class Estimate:
    """
    Scalar Monte Carlo estimate without currency tagging.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import Estimate
    >>> # Estimate objects are returned by scalar MC functions.
    """

    @staticmethod
    def from_json(json: str) -> Estimate:
        """
        Deserialize an ``Estimate`` from JSON.

        Parameters
        ----------
        json : str
            JSON string produced by :meth:`to_json`.

        Returns
        -------
        Estimate
            Parsed ``Estimate`` instance.

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not satisfy the serialized schema.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import Estimate
        >>> payload = '{"mean":1.5,"stderr":0.02,"ci_95":[1.46,1.54],"num_paths":10000,"num_simulated_paths":10000}'
        >>> Estimate.from_json(payload).ci_lower
        1.46
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def mean(self) -> float:
        """
        Sample-mean present value across the simulated paths.

        Returns
        -------
        float
            Mean sample value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class GbmPathSummary:
    """
    Compact captured GBM spot paths.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import simulate_gbm_paths
    >>> paths = simulate_gbm_paths(100, 0.05, 0.0, 0.2, 1.0, 2, 3, seed=7)
    >>> (paths.num_paths, paths.times)
    (3, [0.0, 0.5, 1.0])
    """

    @staticmethod
    def from_json(json: str) -> GbmPathSummary:
        """
        Deserialize a ``GbmPathSummary`` from JSON.

        Parameters
        ----------
        json : str
            JSON string produced by :meth:`to_json`.

        Returns
        -------
        GbmPathSummary
            Parsed ``GbmPathSummary`` instance.

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not satisfy the serialized schema.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import GbmPathSummary, simulate_gbm_paths
        >>> paths = simulate_gbm_paths(100, 0.05, 0.0, 0.2, 1.0, 2, 3, seed=7)
        >>> GbmPathSummary.from_json(paths.to_json()).times
        [0.0, 0.5, 1.0]
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def num_paths(self) -> int:
        """
        Number of independent path estimators.

        Returns
        -------
        int
            Count of independent estimators; half of simulated paths when antithetic.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_simulated_paths(self) -> int:
        """
        Total number of simulated sample paths.

        Returns
        -------
        int
            Raw simulated path count including antithetic partners when enabled.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def times(self) -> list[float]:
        """
        Shared path times in year fractions, including time zero.

        Returns
        -------
        list[float]
            Common time grid in years, including the origin at time zero.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def paths(self) -> list[list[float]]:
        """
        Captured spot paths in deterministic path-id order.

        Returns
        -------
        list[list[float]]
            One spot path per captured estimator, each aligned to ``times``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the captured paths as a pandas DataFrame indexed by time.

        Columns: ``path_0``, ``path_1``, ... — one column per captured path,
        in the deterministic path-id order Rust produced. The index is the
        shared time grid in year fractions, including time zero.

        Wide (time x path) rather than one row: it is the shape ``df.plot()``
        and ``df.quantile(axis=1)`` expect for a path bundle, and every path
        already shares the one time grid. There is always at least one column:
        the engine rejects a zero-path simulation.

        Returns
        -------
        pd.DataFrame
            Time-indexed frame with one column per captured path.

        Raises
        ------
        ValueError
            If a captured path's length differs from the time grid's, which
            would silently misalign the index.
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
    Simulate compact GBM spot paths with Rust's exact GBM transitions.

    ``num_paths`` is the estimator and simulated-path count because captured
    paths do not support antithetic pairing. Passing ``antithetic=True`` raises
    ``ValueError``. The compact output and shared time grids must satisfy
    ``(num_paths + 3) * (num_steps + 1) <= 64_000_000`` scalar values,
    including time zero in every path.

    Parameters
    ----------
    spot : float
        Positive initial underlying price in the output path's price units.
    rate : float
        Continuously compounded annual risk-free rate as a decimal.
    div_yield : float
        Continuously compounded annual dividend or carry yield as a decimal.
    vol : float
        Strictly positive annualized GBM volatility as a decimal, such as ``0.20``.
    expiry : float
        Positive time to maturity in years.
    num_steps : int
        Positive number of equally spaced simulation steps over the expiry
        horizon, subject to the aggregate storage limit above.
    num_paths : int
        Number of independently simulated paths retained in the summary,
        in ``[1, 100_000]`` and subject to the aggregate storage limit above.
    seed : int or None, default None
        Optional deterministic Philox seed; ``None`` uses the Rust
        ``GbmPathConfig`` default (``42``), so unseeded calls are repeatable.
    antithetic : bool, default False
        Antithetic-path request. This compact path API rejects ``True``.

    Returns
    -------
    GbmPathSummary
        Captured time grid and simulated spot paths. ``num_paths`` is the
        number of returned paths and ``num_simulated_paths`` records the same
        count because antithetic capture is unsupported.

    Raises
    ------
    ValueError
        If ``spot`` is non-finite or not strictly positive; ``rate`` or
        ``div_yield`` is non-finite; ``vol`` is not strictly positive or is
        non-finite; ``expiry`` is non-finite or not strictly positive; ``num_steps`` is
        zero or cannot form a time grid; ``num_paths`` is zero or exceeds the
        ``100_000``-path capture limit; the compact output and shared time grids
        exceed ``64_000_000`` scalar values; a simulated spot is non-finite;
        or ``antithetic`` is ``True``.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import simulate_gbm_paths
    >>> summary = simulate_gbm_paths(100, 0.05, 0.0, 0.2, 1.0, 2, 3, seed=7)
    >>> (summary.num_paths, summary.times)
    (3, [0.0, 0.5, 1.0])
    """
    ...

def heston_satisfies_feller(kappa: float, theta: float, vol_of_vol: float) -> bool:
    """
    Test the inclusive Feller condition ``2 * kappa * theta >= vol_of_vol**2``.

    This is the Monte Carlo engine's own predicate, so the answer at the
    boundary matches :func:`price_heston_call` / :func:`price_heston_put`.
    Inputs are not validated: non-finite values typically yield ``False``.

    Parameters
    ----------
    kappa : float
        Mean-reversion speed of the variance process per year.
    theta : float
        Long-run variance level in squared-volatility units.
    vol_of_vol : float
        Annualized volatility of the variance process.

    Returns
    -------
    bool
        ``True`` when ``2 * kappa * theta >= vol_of_vol**2``.

    Sources
    -------
    - Heston (1993): see docs/REFERENCES.md#heston-1993

    Notes
    -----
    This helper does not raise; non-finite inputs typically yield ``False``.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import heston_satisfies_feller
    >>> heston_satisfies_feller(2.0, 0.04, 0.3)
    True
    >>> heston_satisfies_feller(1.0, 0.045, 0.3)
    True
    >>> heston_satisfies_feller(1.0, 0.04, 0.5)
    False
    """
    ...

class EuropeanPricer:
    """
    European-option Monte Carlo pricer under GBM (exact time-stepping).

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import EuropeanPricer
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
            Independent path count in ``[2, 10_000_000]``. Defaults to the
            registry default (``100_000``); the upper limit is checked when pricing.
        seed : int, optional
            RNG seed. Defaults to the registry default (``42``).
        use_parallel : bool, optional
            Parallel accumulation flag. Defaults to the registry default.

        Raises
        ------
        ValueError
            If the path count is less than two or embedded defaults cannot be loaded.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import EuropeanPricer
        >>> EuropeanPricer(500, 9).seed
        9
        """
        ...

    @property
    def num_paths(self) -> int:
        """
        Number of Monte Carlo paths this pricer will simulate.

        Returns
        -------
        int
            Number of Monte Carlo paths.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import EuropeanPricer
        >>> EuropeanPricer(1234).num_paths
        1234
        """
        ...

    @property
    def seed(self) -> int:
        """
        Seed value used for path generation.

        Returns
        -------
        int
            Seed value used for path generation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import EuropeanPricer
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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
        Monte Carlo present value of a European call on the configured process.

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
            Time steps. Defaults to the registry default (``1``).
            Exact GBM is unbiased for any step size, so a European payoff
            only needs one step.
        currency : str, optional
            ISO currency code. Defaults to USD.

        Returns
        -------
        MoneyEstimate
            Monte Carlo price with stderr and confidence bands.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import EuropeanPricer
        >>> EuropeanPricer(800, 0).price_call(100, 100, 0.05, 0.0, 0.2, 1.0, num_steps=52).num_paths
        800

        Raises
        ------
        ValueError
            If ``rate`` or ``div_yield`` is non-finite, ``vol`` is negative or non-finite,
            ``expiry`` is non-finite or not strictly positive, ``num_steps`` is zero, the
            configured path count is zero or exceeds ``10_000_000``,
            ``currency`` is unknown, or discounting produces a non-finite value.
        TypeError
            If a non-``None`` ``currency`` is neither a string nor a ``Currency`` instance.

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
        Monte Carlo present value of a European put on the configured process.

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
            Time steps. Defaults to the registry default (``1``).
            Exact GBM is unbiased for any step size, so a European payoff
            only needs one step.
        currency : str, optional
            ISO currency code. Defaults to USD.

        Returns
        -------
        MoneyEstimate
            Monte Carlo price with stderr and confidence bands.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import EuropeanPricer
        >>> EuropeanPricer(800, 0).price_put(100, 100, 0.05, 0.0, 0.2, 1.0, num_steps=52).num_paths
        800

        Raises
        ------
        ValueError
            If ``rate`` or ``div_yield`` is non-finite, ``vol`` is negative or non-finite,
            ``expiry`` is non-finite or not strictly positive, ``num_steps`` is zero, the
            configured path count is zero or exceeds ``10_000_000``,
            ``currency`` is unknown, or discounting produces a non-finite value.
        TypeError
            If a non-``None`` ``currency`` is neither a string nor a ``Currency`` instance.

        """
        ...

class PathDependentPricer:
    """
    Path-dependent Monte Carlo pricer (Asian-style exotics on GBM).

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import PathDependentPricer
    >>> PathDependentPricer(600, 2).price_asian_call(100, 100, 0.05, 0.0, 0.2, 1.0).num_paths
    600
    """

    def __init__(
        self,
        num_paths: int | None = None,
        seed: int | None = None,
        use_parallel: bool | None = None,
        antithetic: bool | None = None,
        use_sobol: bool | None = None,
        use_brownian_bridge: bool | None = None,
    ) -> None:
        """
        Create a path-dependent pricer.

        Parameters
        ----------
        num_paths : int, optional
            Path budget of at least two. Without Sobol, counts independent
            estimators, with each antithetic pair counted once; with Sobol,
            counts points across independent scrambles. Defaults to the registry default.
        seed : int, optional
            RNG seed. Defaults to the registry default.
        use_parallel : bool, optional
            Parallel accumulation flag. Defaults to the registry default.
            Incompatible with ``use_sobol=True``.
        antithetic : bool, optional
            Pair each path with its sign-flipped counterpart. Defaults to the
            registry default.
        use_sobol : bool, optional
            Drive paths from a Sobol quasi-random sequence. Enabling Sobol
            also switches on the Brownian bridge unless
            ``use_brownian_bridge`` is passed explicitly. Defaults to the
            registry default.
        use_brownian_bridge : bool, optional
            Brownian-bridge path construction (Sobol only). Defaults to the
            registry default.

        Raises
        ------
        ValueError
            If the path budget is less than two, embedded defaults cannot be loaded, or
            the configuration is inconsistent (``use_sobol`` with
            ``use_parallel``).

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import PathDependentPricer
        >>> PathDependentPricer(100, 1, use_parallel=True).num_paths
        100
        >>> PathDependentPricer(100, 1, use_parallel=False, use_sobol=True).use_brownian_bridge
        True
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
        >>> from finstack_quant.models.monte_carlo import PathDependentPricer
        >>> PathDependentPricer(400, 0).price_asian_call(100, 100, 0.05, 0.0, 0.2, 1.0, num_steps=12).num_paths
        400

        Raises
        ------
        ValueError
            If ``rate`` or ``div_yield`` is non-finite, ``vol`` is negative or non-finite,
            ``expiry`` is non-finite or not strictly positive, ``num_steps`` is zero, the
            configured path count is zero or exceeds ``10_000_000``,
            ``currency`` is unknown, or discounting produces a non-finite value.
        TypeError
            If a non-``None`` ``currency`` is neither a string nor a ``Currency`` instance.

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
        >>> from finstack_quant.models.monte_carlo import PathDependentPricer
        >>> PathDependentPricer(400, 0).price_asian_put(100, 100, 0.05, 0.0, 0.2, 1.0, num_steps=12).num_paths
        400

        Raises
        ------
        ValueError
            If ``rate`` or ``div_yield`` is non-finite, ``vol`` is negative or non-finite,
            ``expiry`` is non-finite or not strictly positive, ``num_steps`` is zero, the
            configured path count is zero or exceeds ``10_000_000``,
            ``currency`` is unknown, or discounting produces a non-finite value.
        TypeError
            If a non-``None`` ``currency`` is neither a string nor a ``Currency`` instance.

        """
        ...

    @property
    def num_paths(self) -> int:
        """
        Number of Monte Carlo paths this pricer will simulate.

        Returns
        -------
        int
            Number of Monte Carlo paths.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import PathDependentPricer
        >>> PathDependentPricer(777).num_paths
        777
        """
        ...

    @property
    def seed(self) -> int:
        """
        Seed value used for path generation.

        Returns
        -------
        int
            Seed value used for path generation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import PathDependentPricer
        >>> PathDependentPricer(seed=44).seed
        44
        """
        ...

    @property
    def use_parallel(self) -> bool:
        """
        Whether path generation runs on the rayon pool.

        Returns
        -------
        bool
            Parallel flag. This accessor does not raise.
        """
        ...

    @property
    def antithetic(self) -> bool:
        """
        Whether each path is paired with its sign-flipped counterpart.

        Returns
        -------
        bool
            Antithetic flag. This accessor does not raise.
        """
        ...

    @property
    def use_sobol(self) -> bool:
        """
        Whether paths are driven by a Sobol quasi-random sequence.

        Returns
        -------
        bool
            Sobol flag. This accessor does not raise.
        """
        ...

    @property
    def use_brownian_bridge(self) -> bool:
        """
        Whether Brownian-bridge construction is enabled (Sobol only).

        Returns
        -------
        bool
            Brownian-bridge flag. This accessor does not raise.
        """
        ...

class LsmcPricer:
    """
    Longstaff–Schwartz Monte Carlo pricer for Bermudan options under GBM.

    Exercise is decided on the discrete grid ``1..=num_steps``, not as a
    continuous American. Immediate exercise at valuation (``t = 0``) floors
    the reported price at intrinsic.

    Independent paths must be in ``[1, 10_000_000]`` and time steps in
    ``[1, 100_000]``. Each pricing pass retains at most ``64_000_000`` spot
    values, including time zero and both rows of every antithetic pair:
    ``num_paths * (2 if antithetic else 1) * (num_steps + 1)``. Construction
    checks the storage limit without antithetic partners; pricing checks
    the selected pairing before allocating paths.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import LsmcPricer
    >>> LsmcPricer(300, 0, num_steps=10).price_american_put(100, 100, 0.05, 0.0, 0.3, 1.0).num_paths
    300
    """

    def __init__(
        self,
        num_paths: int | None = None,
        seed: int | None = None,
        use_parallel: bool | None = None,
        num_steps: int | None = None,
        basis: str | None = None,
        basis_degree: int | None = None,
        antithetic: bool | None = None,
    ) -> None:
        """
        Create a Longstaff–Schwartz Monte Carlo pricer for early exercise.

        Parameters
        ----------
        num_paths : int, optional
            Independent path count in ``[2, 10_000_000]``. Defaults to the
            registry default; subject to the retained-storage limit above.
        seed : int, optional
            RNG seed. Defaults to the registry default.
        use_parallel : bool, optional
            Parallel path generation flag. Defaults to the registry default.
        num_steps : int, optional
            Time-grid steps and exercise dates in ``[1, 100_000]``. Defaults
            to the registry value; subject to the retained-storage limit above.
        basis : str, optional
            Regression basis family. One of ``"laguerre"``,
            ``"polynomial"``, or ``"normalized_polynomial"``. Defaults to
            the registry default.
        basis_degree : int, optional
            Polynomial/Laguerre degree. Defaults to the registry default.
            Must be positive; for ``"laguerre"`` it must additionally be
            in ``[1, 4]``.
        antithetic : bool, optional
            Pair each path with its sign-flipped counterpart (``Z`` and
            ``-Z``). Defaults to the registry default (``True``). Both rows
            count toward the retained-storage limit when pricing.

        Raises
        ------
        ValueError
            If ``basis`` is not a recognized family or ``basis_degree`` is
            out of range; the independent path or step count is outside the
            documented range; or the paths without antithetic partners exceed
            ``64_000_000`` retained spot values including time zero.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import LsmcPricer
        >>> LsmcPricer(50, 3).num_paths
        50
        """
        ...

    @property
    def num_paths(self) -> int:
        """
        Number of Monte Carlo paths this pricer will simulate.

        Returns
        -------
        int
            Number of Monte Carlo paths.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def seed(self) -> int:
        """
        Seed value used for path generation.

        Returns
        -------
        int
            Seed value used for path generation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def antithetic(self) -> bool:
        """
        Whether each path is paired with its sign-flipped counterpart.

        Returns
        -------
        bool
            Antithetic flag as passed to ``__init__`` or the registry default.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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
        currency: str | None = None,
        num_steps: int | None = None,
        basis: str | None = None,
        basis_degree: int | None = None,
    ) -> MoneyEstimate:
        """
        Price a Bermudan put via LSMC on the grid ``1..=num_steps``.

        Immediate exercise at valuation floors the reported price at
        ``max(strike - spot, 0)``. If that floor binds, stderr and the 95%
        CI collapse to the intrinsic value.

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
        currency : str, optional
            ISO currency code. Defaults to USD.
        num_steps : int, optional
            Per-call exercise-grid override in ``[1, 100_000]``; defaults to
            the instance ``num_steps``. Subject to the retained-storage limit
            described by :class:`LsmcPricer`.
        basis : str, optional
            Per-call override of the regression basis family; defaults to
            the instance ``basis``.
        basis_degree : int, optional
            Per-call override of the basis degree; defaults to the instance
            ``basis_degree``.

        Returns
        -------
        MoneyEstimate
            LSMC price with stderr and confidence bands.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import LsmcPricer
        >>> LsmcPricer(200, 0, num_steps=8).price_american_put(100, 100, 0.05, 0.0, 0.25, 1.0).num_paths
        200

        Raises
        ------
        ValueError
            If ``strike`` is non-finite or non-positive, or is no greater than ``1e-14``
            with the normalized-polynomial basis; ``rate`` or ``div_yield``
            is non-finite; ``vol`` is negative or non-finite; ``expiry`` is
            non-finite or not strictly positive;
            the time-step or independent path count is outside the documented
            range; retained spots exceed ``64_000_000`` values including time
            zero and both rows of each antithetic pair; or ``currency`` is unknown.
        TypeError
            If a non-``None`` ``currency`` is neither a string nor a ``Currency`` instance.

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
        currency: str | None = None,
        num_steps: int | None = None,
        basis: str | None = None,
        basis_degree: int | None = None,
    ) -> MoneyEstimate:
        """
        Price a Bermudan call via LSMC on the grid ``1..=num_steps``.

        Immediate exercise at valuation floors the reported price at
        ``max(spot - strike, 0)``. If that floor binds, stderr and the 95%
        CI collapse to the intrinsic value.

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
        currency : str, optional
            ISO currency code. Defaults to USD.
        num_steps : int, optional
            Per-call exercise-grid override in ``[1, 100_000]``; defaults to
            the instance ``num_steps``. Subject to the retained-storage limit
            described by :class:`LsmcPricer`.
        basis : str, optional
            Per-call override of the regression basis family; defaults to
            the instance ``basis``.
        basis_degree : int, optional
            Per-call override of the basis degree; defaults to the instance
            ``basis_degree``.

        Returns
        -------
        MoneyEstimate
            LSMC price with stderr and confidence bands.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import LsmcPricer
        >>> LsmcPricer(200, 0, num_steps=8).price_american_call(100, 100, 0.05, 0.0, 0.25, 1.0).num_paths
        200

        Raises
        ------
        ValueError
            If ``strike`` is non-finite or non-positive, or is no greater than ``1e-14``
            with the normalized-polynomial basis; ``rate`` or ``div_yield``
            is non-finite; ``vol`` is negative or non-finite; ``expiry`` is
            non-finite or not strictly positive;
            the time-step or independent path count is outside the documented
            range; retained spots exceed ``64_000_000`` values including time
            zero and both rows of each antithetic pair; or ``currency`` is unknown.
        TypeError
            If a non-``None`` ``currency`` is neither a string nor a ``Currency`` instance.

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
        currency: str | None = None,
        num_steps: int | None = None,
        basis: str | None = None,
        basis_degree: int | None = None,
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
        currency : str, optional
            ISO currency code. Defaults to USD.
        num_steps : int, optional
            Per-call exercise-grid override in ``[1, 100_000]``; defaults to
            the instance ``num_steps``. Subject to the retained-storage limit
            described by :class:`LsmcPricer`.
        basis : str, optional
            Per-call override of the regression basis family; defaults to
            the instance ``basis``.
        basis_degree : int, optional
            Per-call override of the basis degree; defaults to the instance
            ``basis_degree``.

        Returns
        -------
        MoneyEstimate
            Out-of-sample price with stderr and confidence bands.

        Raises
        ------
        ValueError
            If ``pricing_seed`` equals the pricer's training seed; the time-step
            or independent path count is outside the documented range; or each
            pass exceeds ``64_000_000`` retained spot values including time zero
            and both rows of each antithetic pair. The input validation errors
            documented by :meth:`price_american_put` also apply.
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
        currency: str | None = None,
        num_steps: int | None = None,
        basis: str | None = None,
        basis_degree: int | None = None,
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
            Per-call exercise-grid override in ``[1, 100_000]``; defaults to
            the instance ``num_steps``. Subject to the retained-storage limit
            described by :class:`LsmcPricer`.
        basis : str, optional
            Per-call override of the regression basis family; defaults to
            the instance ``basis``.
        basis_degree : int, optional
            Per-call override of the basis degree; defaults to the instance
            ``basis_degree``.
        currency : str, optional
            ISO currency code. Defaults to USD.
        num_steps : int, optional
            Per-call exercise-grid override in ``[1, 100_000]``; defaults to
            the instance ``num_steps``. Subject to the retained-storage limit
            described by :class:`LsmcPricer`.
        basis : str, optional
            Per-call override of the regression basis family; defaults to
            the instance ``basis``.
        basis_degree : int, optional
            Per-call override of the basis degree; defaults to the instance
            ``basis_degree``.

        Returns
        -------
        MoneyEstimate
            Out-of-sample price with stderr and confidence bands.

        Raises
        ------
        ValueError
            If ``pricing_seed`` equals the pricer's training seed; the time-step
            or independent path count is outside the documented range; or each
            pass exceeds ``64_000_000`` retained spot values including time zero
            and both rows of each antithetic pair. The input validation errors
            documented by :meth:`price_american_put` also apply.
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

    Simulates spot and variance with Andersen's quadratic-exponential (QE)
    discretization, which stays stable when the Feller condition is violated.
    Rates and dividend yield are continuously compounded decimals; Heston
    parameters follow the standard square-root variance specification.

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
        Independent path estimators in ``[2, 10_000_000]``; each antithetic
        pair counts once. The registry default is ``100_000``.
    seed : int, optional
        RNG seed (registry default ``42``).
    num_steps : int, optional
        Time steps per path (registry default ``252``).
    currency : str, optional
        ISO currency code; ``None`` uses the registry binding default.

    Returns
    -------
    MoneyEstimate
        Discounted Monte Carlo price with stderr and confidence bands.

    Raises
    ------
    ValueError
        If Heston parameters, expiry, path count, step count, or the discount
        factor fail validation, or a simulated discounted payoff is
        non-finite. Violating the Feller condition does not raise.

    Sources
    -------
    - Heston (1993): see docs/REFERENCES.md#heston-1993
    - Andersen QE (2008): see docs/REFERENCES.md#andersen-2008-heston-qe

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import price_heston_call
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
        Independent path estimators in ``[2, 10_000_000]``; each antithetic
        pair counts once. ``None`` selects the engine default.
    seed : int or None, default None
        Optional deterministic random seed for reproducible path generation.
    num_steps : int or None, default None
        Optional number of time steps; ``None`` selects the engine default grid.
    currency : str or None, default None
        ISO-4217 output currency tag; ``None`` uses the registry binding default.

    Returns
    -------
    MoneyEstimate
        Discounted Monte Carlo put price.

    Raises
    ------
    ValueError
        If Heston parameters, expiry, path count, step count, or the discount
        factor fail validation, or a simulated discounted payoff is
        non-finite. Violating the Feller condition does not raise.

    Sources
    -------
    - Heston (1993): see docs/REFERENCES.md#heston-1993
    - Andersen QE (2008): see docs/REFERENCES.md#andersen-2008-heston-qe

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import price_heston_put
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
    is_call: bool,
    num_paths: int | None = None,
    seed: int | None = None,
    num_steps: int | None = None,
    bump_size: float | None = None,
    currency: str | None = None,
) -> Estimate:
    """
    Finite-difference delta for a European option (independence-bound stderr).

    Both this function and :func:`finite_diff_delta_crn` reuse common random
    numbers. This function reports a conservative independence-bound stderr;
    :func:`finite_diff_delta_crn` reports the tighter paired CRN stderr.

    Parameters
    ----------
    spot : float
        Finite positive spot price. The down-bumped state must remain at least
        ``1e-12``.
    strike : float
        Strike price.
    rate : float
        Risk-free rate (continuously compounded decimal).
    div_yield : float
        Dividend yield (continuously compounded decimal).
    vol : float
        Volatility (decimal); must be strictly positive.
    expiry : float
        Maturity in years.
    is_call : bool
        ``True`` for a call, ``False`` for a put.
    num_paths : int, optional
        Paths per evaluation (default ``10_000``).
    seed : int, optional
        RNG seed (default ``42``).
    num_steps : int, optional
        Time-grid steps (default ``50``).
    bump_size : float, optional
        Relative Monte Carlo spot shock (default ``0.01`` = 1% of spot), not
        a closed-form local Greek step. The absolute bump is
        ``max(abs(spot) * bump_size, 1e-8)`` and must leave a symmetric
        central stencil above the spot floor.
    currency : str, optional
        ISO currency code. Defaults to USD.

    Returns
    -------
    Estimate
        ``mean`` is the delta, ``stderr`` its standard error, ``ci_lower`` /
        ``ci_upper`` the symmetric 95% band.

    Raises
    ------
    ValueError
        If ``vol`` is not strictly positive, ``spot`` or ``bump_size`` is
        non-finite or non-positive, the symmetric down-bump falls below
        ``1e-12``, or another pricing input is invalid.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import finite_diff_delta
    >>> est = finite_diff_delta(100, 100, 0.05, 0.0, 0.2, 1.0, True, num_paths=200, seed=7, num_steps=10)
    >>> 0 < est.mean < 1 and est.stderr >= 0
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
    is_call: bool,
    num_paths: int | None = None,
    seed: int | None = None,
    num_steps: int | None = None,
    bump_size: float | None = None,
    currency: str | None = None,
) -> Estimate:
    """
    Finite-difference delta with paired common-random-number stderr.

    Same CRN-priced central difference as :func:`finite_diff_delta`; only the
    reported stderr estimator differs (paired pathwise differences instead of
    the independence bound). Always runs serially.

    Parameters
    ----------
    spot : float
        Finite positive spot price. The down-bumped state must remain at least
        ``1e-12``.
    strike : float
        Strike price.
    rate : float
        Risk-free rate (continuously compounded decimal).
    div_yield : float
        Dividend yield (continuously compounded decimal).
    vol : float
        Volatility (decimal); must be strictly positive.
    expiry : float
        Maturity in years.
    is_call : bool
        ``True`` for a call, ``False`` for a put.
    num_paths : int, optional
        Paths per evaluation (default ``10_000``).
    seed : int, optional
        RNG seed (default ``42``).
    num_steps : int, optional
        Time-grid steps (default ``50``).
    bump_size : float, optional
        Relative Monte Carlo spot shock (default ``0.01`` = 1% of spot), not
        a closed-form local Greek step. The absolute bump is
        ``max(abs(spot) * bump_size, 1e-8)`` and must leave a symmetric
        central stencil above the spot floor.
    currency : str, optional
        ISO currency code. Defaults to USD.

    Returns
    -------
    Estimate
        ``mean`` is the delta, ``stderr`` the paired CRN standard error.

    Raises
    ------
    ValueError
        If ``vol`` is not strictly positive, ``spot`` or ``bump_size`` is
        non-finite or non-positive, the symmetric down-bump falls below
        ``1e-12``, or another pricing input is invalid.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import finite_diff_delta_crn
    >>> est = finite_diff_delta_crn(100, 100, 0.05, 0.0, 0.2, 1.0, True, num_paths=200, seed=7, num_steps=10)
    >>> 0 < est.mean < 1 and est.stderr >= 0
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
    is_call: bool,
    num_paths: int | None = None,
    seed: int | None = None,
    num_steps: int | None = None,
    bump_size: float | None = None,
    currency: str | None = None,
) -> Estimate:
    """
    Finite-difference gamma (independence-bound stderr).

    See :func:`finite_diff_gamma_crn` for the tighter paired CRN variant.

    Parameters
    ----------
    spot : float
        Finite positive spot price. The down-bumped state must remain at least
        ``1e-12``.
    strike : float
        Strike price.
    rate : float
        Risk-free rate (continuously compounded decimal).
    div_yield : float
        Dividend yield (continuously compounded decimal).
    vol : float
        Volatility (decimal); must be strictly positive.
    expiry : float
        Maturity in years.
    is_call : bool
        ``True`` for a call, ``False`` for a put.
    num_paths : int, optional
        Paths per evaluation (default ``10_000``).
    seed : int, optional
        RNG seed (default ``42``).
    num_steps : int, optional
        Time-grid steps (default ``50``).
    bump_size : float, optional
        Relative Monte Carlo spot shock (default ``0.01`` = 1% of spot), not
        a closed-form local Greek step. The absolute bump is
        ``max(abs(spot) * bump_size, 1e-8)`` and must leave a symmetric
        central stencil above the spot floor.
    currency : str, optional
        ISO currency code. Defaults to USD.

    Returns
    -------
    Estimate
        ``mean`` is the gamma, ``stderr`` its standard error.

    Raises
    ------
    ValueError
        If ``vol`` is not strictly positive, ``spot`` or ``bump_size`` is
        non-finite or non-positive, the symmetric down-bump falls below
        ``1e-12``, or another pricing input is invalid.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import finite_diff_gamma
    >>> est = finite_diff_gamma(100, 100, 0.05, 0.0, 0.2, 1.0, True, num_paths=200, seed=7, num_steps=10)
    >>> est.mean > 0 and est.stderr >= 0
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
    is_call: bool,
    num_paths: int | None = None,
    seed: int | None = None,
    num_steps: int | None = None,
    bump_size: float | None = None,
    currency: str | None = None,
) -> Estimate:
    """
    Finite-difference gamma with paired common-random-number stderr.

    Returns ``(gamma, paired_stderr)`` where the standard error is the
    per-path paired error of ``(V_up_i − 2 V_base_i + V_down_i) / h²``.
    Always runs serially.

    Parameters
    ----------
    spot : float
        Finite positive spot price. The down-bumped state must remain at least
        ``1e-12``.
    strike : float
        Strike price.
    rate : float
        Risk-free rate (continuously compounded decimal).
    div_yield : float
        Dividend yield (continuously compounded decimal).
    vol : float
        Volatility (decimal); must be strictly positive.
    expiry : float
        Maturity in years.
    is_call : bool
        ``True`` for a call, ``False`` for a put.
    num_paths : int, optional
        Paths per evaluation (default ``10_000``).
    seed : int, optional
        RNG seed (default ``42``).
    num_steps : int, optional
        Time-grid steps (default ``50``).
    bump_size : float, optional
        Relative Monte Carlo spot shock (default ``0.01`` = 1% of spot), not
        a closed-form local Greek step. The absolute bump is
        ``max(abs(spot) * bump_size, 1e-8)`` and must leave a symmetric
        central stencil above the spot floor.
    currency : str, optional
        ISO currency code. Defaults to USD.

    Returns
    -------
    Estimate
        ``mean`` is the gamma, ``stderr`` the paired CRN standard error.

    Raises
    ------
    ValueError
        If ``vol`` is not strictly positive, ``spot`` or ``bump_size`` is
        non-finite or non-positive, the symmetric down-bump falls below
        ``1e-12``, or another pricing input is invalid.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import finite_diff_gamma_crn
    >>> est = finite_diff_gamma_crn(100, 100, 0.05, 0.0, 0.2, 1.0, True, num_paths=200, seed=7, num_steps=10)
    >>> est.mean > 0 and est.stderr >= 0
    True
    """
    ...
