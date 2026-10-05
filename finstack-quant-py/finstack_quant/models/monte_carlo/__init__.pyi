"""
Monte Carlo convenience bindings (``finstack-quant-models``).

Exposes path simulation for the built-in Markov processes
(:func:`simulate_paths`), GBM and Heston pricers, and finite-difference Greek
estimators. Processes and discretization schemes are selected by the plain-data
spec passed to :func:`simulate_paths`; the Rust process, discretization, RNG
and payoff types are not surfaced as standalone Python types, and the pricers
take their parameters directly as numeric arguments.

Examples
--------
>>> from finstack_quant.models.monte_carlo import heston_satisfies_feller
>>> heston_satisfies_feller(2.0, 0.04, 0.3)
True
"""

from __future__ import annotations

from collections.abc import Sequence
from typing import Any

import pandas as pd

from finstack_quant.core.money import Money

__all__ = [
    "Estimate",
    "EuropeanPricer",
    "LrmGreeks",
    "LsmcPricer",
    "MoneyEstimate",
    "PathDependentPricer",
    "PathSummary",
    "finite_diff_delta",
    "finite_diff_gamma",
    "heston_satisfies_feller",
    "price_heston_call",
    "price_heston_put",
    "simulate_paths",
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

class LrmGreeks:
    """
    Monte Carlo price with likelihood-ratio delta and vega from the same paths.

    Returned by :meth:`PathDependentPricer.price_with_lrm_greeks`. Each Greek
    is a sample mean of ``discounted payoff x score``, so it carries its own
    standard error and 95% confidence interval. The three estimates share one
    set of paths and are correlated.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import PathDependentPricer
    >>> pricer = PathDependentPricer(2000, 7, use_parallel=False)
    >>> greeks = pricer.price_with_lrm_greeks(100, 100, 0.04, 0.01, 0.25, 1.0, True, num_steps=12)
    >>> (greeks.price.num_paths, greeks.delta.num_paths, greeks.vega.num_paths)
    (2000, 2000, 2000)
    >>> list(greeks.to_dataframe().index)
    ['price', 'delta', 'vega']
    """

    @staticmethod
    def from_json(json: str) -> LrmGreeks:
        """
        Deserialize an ``LrmGreeks`` from JSON.

        Parameters
        ----------
        json : str
            JSON string produced by :meth:`to_json`.

        Returns
        -------
        LrmGreeks
            Parsed ``LrmGreeks`` instance.

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not satisfy the serialized schema.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import LrmGreeks, PathDependentPricer
        >>> pricer = PathDependentPricer(2000, 7, use_parallel=False)
        >>> greeks = pricer.price_with_lrm_greeks(100, 100, 0.04, 0.01, 0.25, 1.0, False, num_steps=12)
        >>> LrmGreeks.from_json(greeks.to_json()).delta.mean == greeks.delta.mean
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON string with the ``price``, ``delta`` and ``vega``
            estimates.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def price(self) -> MoneyEstimate:
        """
        Discounted price estimate in the payoff currency.

        Returns
        -------
        MoneyEstimate
            Mean, standard error and 95% confidence interval of the price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def delta(self) -> Estimate:
        """
        Likelihood-ratio delta.

        Returns
        -------
        Estimate
            Change in price per unit change in the initial spot, with its
            standard error and 95% confidence interval.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def vega(self) -> Estimate:
        """
        Likelihood-ratio vega.

        Returns
        -------
        Estimate
            Change in price per one volatility point (``0.01`` of annualized
            volatility), with its standard error and 95% confidence interval.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Tabulate the three estimates as a pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            Three rows indexed ``["price", "delta", "vega"]`` with float64
            columns ``mean``, ``stderr``, ``ci_lower`` and ``ci_upper`` (the
            95% confidence interval). The price row is in units of the price
            currency.

        Raises
        ------
        ImportError
            If pandas is not installed.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import PathDependentPricer
        >>> pricer = PathDependentPricer(2000, 7, use_parallel=False)
        >>> frame = pricer.price_with_lrm_greeks(100, 100, 0.04, 0.01, 0.25, 1.0, True, num_steps=12).to_dataframe()
        >>> list(frame.columns)
        ['mean', 'stderr', 'ci_lower', 'ci_upper']
        """
        ...

class PathSummary:
    """
    Simulated paths of a Markov process on a shared time grid.

    Returned by :func:`simulate_paths`. ``values`` holds every state in
    row-major ``[path][time][factor]`` order; :meth:`to_dataframe` reshapes it
    into one row per path and time.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import simulate_paths
    >>> spec = {
    ...     "process": {"type": "gbm", "r": 0.05, "q": 0.0, "sigma": 0.2},
    ...     "initial_state": [100.0],
    ...     "time_grid": {"type": "uniform", "expiry": 1.0, "num_steps": 2},
    ...     "num_paths": 3,
    ...     "seed": 7,
    ... }
    >>> paths = simulate_paths(spec)
    >>> (paths.num_simulated_paths, paths.times, paths.factor_names)
    (3, [0.0, 0.5, 1.0], ['spot'])
    >>> paths.to_dataframe().shape
    (9, 1)
    """

    @staticmethod
    def from_json(json: str) -> PathSummary:
        """
        Deserialize a ``PathSummary`` from JSON.

        Parameters
        ----------
        json : str
            JSON string produced by :meth:`to_json`.

        Returns
        -------
        PathSummary
            Parsed ``PathSummary`` instance.

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not satisfy the serialized schema.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import PathSummary, simulate_paths
        >>> spec = {
        ...     "process": {"type": "gbm", "r": 0.05, "q": 0.0, "sigma": 0.2},
        ...     "initial_state": [100.0],
        ...     "time_grid": {"type": "uniform", "expiry": 1.0, "num_steps": 2},
        ...     "num_paths": 3,
        ...     "seed": 7,
        ... }
        >>> paths = simulate_paths(spec)
        >>> PathSummary.from_json(paths.to_json()).values == paths.values
        True
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
        Number of independent random streams requested.

        Returns
        -------
        int
            The ``num_paths`` of the simulation spec.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_simulated_paths(self) -> int:
        """
        Number of stored paths.

        Returns
        -------
        int
            ``num_paths``, or ``2 * num_paths`` with antithetic sampling, where
            stored paths ``2k`` and ``2k + 1`` are stream ``k`` and its
            antithetic partner.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def dim(self) -> int:
        """
        State dimension of the process.

        Returns
        -------
        int
            Number of state components; equals ``len(factor_names)``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def times(self) -> list[float]:
        """
        Simulation times in year fractions, starting at zero.

        Returns
        -------
        list[float]
            Shared time grid in years; its length is the step count plus one.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def factor_names(self) -> list[str]:
        """
        Name of each state component, in state-vector order.

        Returns
        -------
        list[str]
            For example ``["spot"]`` for GBM, ``["spot", "variance"]`` for
            Heston and ``["short_rate"]`` for the short-rate models.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def values(self) -> list[float]:
        """
        Simulated states in row-major ``[path][time][factor]`` order.

        Returns
        -------
        list[float]
            Flat list of ``num_simulated_paths * len(times) * dim`` states: the
            value of factor ``f`` on stored path ``p`` at ``times[s]`` is
            ``values[(p * len(times) + s) * dim + f]``. Reshape with
            ``numpy.asarray(values).reshape(num_simulated_paths, len(times), dim)``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the paths as a long pandas DataFrame.

        One row per stored path and time, indexed by a ``(path, time)``
        ``MultiIndex`` in the order Rust produced (``path`` is the zero-based
        stored-path number, ``time`` the year fraction), with one float64
        column per entry of ``factor_names``. Use
        ``frame["spot"].unstack("path")`` for the time-by-path table of one
        factor.

        Returns
        -------
        pd.DataFrame
            ``num_simulated_paths * len(times)`` rows and ``dim`` columns.

        Raises
        ------
        ValueError
            If ``values`` does not hold ``num_simulated_paths * len(times) *
            len(factor_names)`` entries, which is only possible for a summary
            rebuilt from inconsistent JSON.
        """
        ...

def simulate_paths(spec: dict[str, Any] | str) -> PathSummary:
    """
    Simulate paths of any built-in process on a shared time grid.

    Binds Rust ``monte_carlo::simulate::simulate_paths``. The spec is plain
    data validated in Rust: it selects the process, the discretization scheme,
    the time grid and the random streams. The same spec always reproduces the
    same paths bit for bit, on any thread count.

    Parameters
    ----------
    spec : dict or str
        ``PathSimulationSpec`` as a dict or its JSON text, with keys:

        - ``process`` : dict tagged by ``"type"`` plus that process's
          parameters. Rates, yields and volatilities are annualized decimals
          (``0.05`` for 5%); times are in years.

          - ``"gbm"`` : ``r``, ``q``, ``sigma``. State ``[spot]``.
          - ``"gbm_with_dividends"`` : ``params`` (GBM parameters) and
            ``dividends``, a list of ``[time, {"cash": amount}]`` or
            ``[time, {"proportional": fraction}]``. State ``[spot]``.
          - ``"multi_gbm"`` : ``assets`` (list of GBM parameters) and optional
            ``correlation`` (row-major ``n x n``). State ``[spot_0, ...]``.
          - ``"brownian"`` : ``mu``, ``sigma``. State ``[x]``.
          - ``"multi_brownian"`` : ``mus``, ``sigmas``, optional
            ``correlation``. State ``[x_0, ...]``.
          - ``"multi_ou"`` : ``kappas``, ``thetas``, ``sigmas``, optional
            ``correlation``. State ``[x_0, ...]``.
          - ``"hull_white_1f"`` : ``kappa``, ``volatility``
            (``{"times": [...], "values": [...]}``), ``theta_curve``,
            ``theta_times``. State ``[short_rate]``.
          - ``"cir"`` : ``kappa``, ``theta``, ``sigma``. State ``[short_rate]``.
          - ``"cir_plus_plus"`` : ``params`` (CIR parameters),
            ``shift_curve``, ``shift_times``. State ``[short_rate]``.
          - ``"heston"`` : ``r``, ``q``, ``kappa``, ``theta``, ``sigma_v``,
            ``rho``, ``v0``. State ``[spot, variance]``; the starting variance
            must equal ``v0``.
          - ``"schwartz_smith"`` : ``kappa``, ``sigma_x``, ``mu_y``,
            ``sigma_y``, ``rho_xy``, optional ``lambda_x``. State ``[x, y]``.
          - ``"local_vol"`` : ``r``, ``q`` and ``surface``, a Dupire local
            volatility grid ``{"expiries": [...], "strikes": [...],
            "local_vols": [...]}`` (row-major, expiry as the slow axis) such
            as ``json.loads(LocalVolSurface.to_json())`` from
            :mod:`finstack_quant.models.volatility`. State ``[spot]``. Use a
            fine time grid: the scheme freezes the volatility over each step.
          - ``"lmm"`` : ``num_forwards``, ``num_factors`` (2 or 3),
            ``tenors`` (``num_forwards + 1`` year fractions),
            ``accrual_factors``, ``displacements``, ``vol_times``,
            ``vol_values`` (per volatility period, per forward, three factor
            loadings) and ``initial_forwards``. State ``[forward_0, ...]``,
            simple forward rates under the terminal measure;
            ``initial_state`` must equal ``initial_forwards``, and the time
            grid must contain every fixing date and volatility breakpoint
            inside the horizon.
          - ``"rough_bergomi"`` : ``r``, ``q``, ``hurst`` (``{"h": H}`` with
            ``H`` in ``(0, 1)``), ``eta``, ``rho`` and ``xi``, the forward
            variance curve ``{"interpolation": "linear" |
            "constant_intervals", "times": [...], "values": [...]}``. State
            ``[spot]``.
          - ``"rough_heston"`` : ``r``, ``q``, ``hurst`` (``H`` in
            ``(0, 0.5)``), ``kappa``, ``theta``, ``sigma_v``, ``rho``, ``v0``.
            State ``[spot, variance]``; the starting variance must equal
            ``v0``. At most 8,000 steps.
          - ``"cheyette_rough"`` : ``kappa``, ``sigma_base`` (base volatility
            curve, same shape as ``xi``), ``hurst``, ``eta``, ``rho``,
            ``phi_times`` and ``phi_values`` (initial forward curve). State
            ``[x, y]``, normally started at ``[0, 0]``; the short rate is
            ``x`` plus the initial forward rate.

        - ``scheme`` : ``"default"`` (the process's canonical scheme — its
          exact transition where one exists, quadratic-exponential for
          square-root variance, Euler otherwise; used when omitted),
          ``"euler"``, ``"log_euler"`` or ``"milstein"``. ``"log_euler"`` and
          ``"milstein"`` apply to ``"gbm"`` and ``"multi_gbm"`` only, except
          that ``"local_vol"`` also accepts ``"log_euler"`` (its default);
          ``"lmm"``, ``"rough_bergomi"``, ``"rough_heston"`` and
          ``"cheyette_rough"`` accept ``"default"`` only.
        - ``initial_state`` : list of float, the state at time zero in the
          process's state layout.
        - ``time_grid`` : ``{"type": "uniform", "expiry": years, "num_steps": n}``
          or ``{"type": "times", "times": [0.0, ...]}`` with strictly
          increasing year fractions.
        - ``num_paths`` : int in ``[1, 100_000]``, the number of independent
          random streams.
        - ``seed`` : int, root seed of the Philox generator.
        - ``antithetic`` : bool, default ``False``. When ``True`` each stream's
          path is followed by its antithetic partner, driven by the negated
          normal draws.
        - ``fbm`` : dict, optional. Generator of the fractional noise that
          ``"rough_bergomi"`` and ``"cheyette_rough"`` consume:
          ``{"type": "volterra"}`` (used when omitted; the Riemann-Liouville
          Volterra process of the published models, uniform grids only),
          ``{"type": "cholesky"}`` (exact fractional Brownian motion, at most
          8,000 steps) or ``{"type": "windowed_conditional",
          "near_field_size": n}`` (approximate fractional Brownian motion
          with an ``n``-step window; ``n`` optional). The last two give a
          different model with the same forwards.

    Returns
    -------
    PathSummary
        Every simulated state, including the initial state at time zero, in
        stream order.

    Raises
    ------
    ValueError
        If ``spec`` is not valid ``PathSimulationSpec`` data (unknown key or
        tag, missing field, wrong type); a process parameter or correlation
        matrix is out of range; the scheme is not available for the process;
        ``fbm`` is set for a process that does not consume fractional noise;
        ``initial_state`` has the wrong length or lies outside the process's
        domain (for example a non-positive GBM spot); the time grid is
        invalid; ``num_paths`` is outside ``[1, 100_000]``; the output would
        exceed ``64_000_000`` stored values; or a simulated state is
        non-finite.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import simulate_paths
    >>> spec = {
    ...     "process": {"type": "gbm", "r": 0.05, "q": 0.0, "sigma": 0.2},
    ...     "initial_state": [100.0],
    ...     "time_grid": {"type": "uniform", "expiry": 1.0, "num_steps": 2},
    ...     "num_paths": 3,
    ...     "seed": 7,
    ... }
    >>> paths = simulate_paths(spec)
    >>> (paths.num_paths, paths.times, paths.factor_names)
    (3, [0.0, 0.5, 1.0], ['spot'])
    >>> heston = simulate_paths({
    ...     **spec,
    ...     "process": {
    ...         "type": "heston",
    ...         "r": 0.04,
    ...         "q": 0.01,
    ...         "kappa": 2.0,
    ...         "theta": 0.05,
    ...         "sigma_v": 0.3,
    ...         "rho": -0.7,
    ...         "v0": 0.03,
    ...     },
    ...     "initial_state": [100.0, 0.03],
    ...     "antithetic": True,
    ... })
    >>> (heston.num_simulated_paths, heston.factor_names)
    (6, ['spot', 'variance'])
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

    def price_with_lrm_greeks(
        self,
        spot: float,
        strike: float,
        rate: float,
        div_yield: float,
        vol: float,
        expiry: float,
        is_call: bool,
        num_steps: int | None = None,
        currency: str | None = None,
    ) -> LrmGreeks:
        """
        Price an arithmetic Asian option with likelihood-ratio delta and vega.

        The price, delta and vega come from one set of GBM paths: the Greeks
        weight each discounted payoff by the score of the path density
        (Glasserman 2003, section 7.3), so no bumped re-simulation is needed.
        The payoff is the one :meth:`price_asian_call` / :meth:`price_asian_put`
        price: unit notional, fixings at steps ``1..=num_steps``.

        Parameters
        ----------
        spot : float
            Finite, strictly positive spot price at time zero.
        strike : float
            Strike price in the same units as ``spot``.
        rate : float
            Risk-free rate (continuously compounded decimal).
        div_yield : float
            Dividend yield (continuously compounded decimal).
        vol : float
            Annualized volatility (decimal), strictly positive.
        expiry : float
            Maturity in years.
        is_call : bool
            ``True`` for a call on the arithmetic average, ``False`` for a put.
        num_steps : int, optional
            Time-grid steps, each an averaging date. Defaults to the registry
            value ``convenience.greeks.lrm_num_steps`` (32), not the 252 steps
            of :meth:`price_asian_call`: every path is kept in memory, so
            ``num_paths * (num_steps + 1)`` may not exceed ``4_000_000``. A
            default pricer (100,000 paths) therefore accepts at most 39 steps.
        currency : str, optional
            ISO currency code. Defaults to USD.

        Returns
        -------
        LrmGreeks
            Price in ``currency``, delta per unit of spot and vega per
            volatility point (``0.01``), each with its standard error and 95%
            confidence interval.

        Raises
        ------
        ValueError
            If ``spot``, ``vol`` or ``expiry`` is not finite and strictly
            positive, ``rate`` or ``div_yield`` is non-finite, ``num_steps``
            is zero, the pricer uses Sobol or antithetic sampling,
            ``num_paths`` exceeds ``100_000``, ``num_paths * (num_steps + 1)``
            exceeds ``4_000_000``, or ``currency`` is unknown.
        TypeError
            If a non-``None`` ``currency`` is neither a string nor a ``Currency`` instance.

        Examples
        --------
        >>> from finstack_quant.models.monte_carlo import PathDependentPricer
        >>> greeks = PathDependentPricer().price_with_lrm_greeks(100, 100, 0.04, 0.01, 0.25, 1.0, True)
        >>> 0.0 < greeks.delta.mean < 1.0
        True
        >>> greeks.vega.stderr > 0.0
        True
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
    Finite-difference delta for a vanilla European option under GBM.

    Central difference in spot with common random numbers: the up and down
    valuations share each path's random draws, the estimate is the mean of
    the per-path differences, and ``stderr`` is the paired
    (common-random-number) standard error of those differences.

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
        ``mean`` is the delta, ``stderr`` its paired (common-random-number)
        standard error, ``ci_lower`` / ``ci_upper`` the symmetric 95% band.

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
    Finite-difference gamma for a vanilla European option under GBM.

    Second central difference in spot with common random numbers; the
    estimate is the mean of the per-path second differences and ``stderr``
    is their paired (common-random-number) standard error.

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
        ``mean`` is the gamma, ``stderr`` its paired (common-random-number)
        standard error.

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
