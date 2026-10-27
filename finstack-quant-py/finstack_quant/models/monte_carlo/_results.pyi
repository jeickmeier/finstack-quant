"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/monte_carlo/results.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
from finstack_quant.core.money import Money

class MoneyEstimate:
    """
    Discounted Monte Carlo estimate with money units and confidence bands.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import EuropeanPricer
    >>> r = EuropeanPricer(10_000, seed=42).price_gbm(100, 100, 0.05, 0.0, 0.2, 1.0, True)
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
        >>> priced = EuropeanPricer(10_000, seed=42).price_gbm(100, 100, 0.05, 0.0, 0.2, 1.0, True)
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
        >>> pricer.price_gbm(100, 100, 0.05, 0.0, 0.2, 1.0, True).mean.amount > 0
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
        >>> pricer.price_gbm(100, 100, 0.05, 0.0, 0.2, 1.0, True).stderr >= 0
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
        >>> r = EuropeanPricer(2000, seed=42).price_gbm(100, 100, 0.05, 0.0, 0.2, 1.0, True)
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
        >>> r = EuropeanPricer(2000, seed=42).price_gbm(100, 100, 0.05, 0.0, 0.2, 1.0, True)
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
        >>> EuropeanPricer(1234, seed=42).price_gbm(100, 100, 0.05, 0.0, 0.2, 1.0, True).num_paths
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
        >>> pricer.price_gbm(100, 100, 0.05, 0.0, 0.2, 1.0, True).relative_stderr() >= 0
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Reconstruct this value for ``pickle`` through its canonical JSON form.

        Returns
        -------
        tuple[Any, tuple[str]]
            The bound ``from_json`` reconstruction callable and its JSON payload.

        Raises
        ------
        ValueError
            If serialization fails or the JSON payload cannot reconstruct the
            stored value, including non-finite floats unsupported by its schema.
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

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Reconstruct this value for ``pickle`` through its canonical JSON form.

        Returns
        -------
        tuple[Any, tuple[str]]
            The bound ``from_json`` reconstruction callable and its JSON payload.

        Raises
        ------
        ValueError
            If serialization fails or the JSON payload cannot reconstruct the
            stored value, including non-finite floats unsupported by its schema.
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

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Reconstruct this value for ``pickle`` through its canonical JSON form.

        Returns
        -------
        tuple[Any, tuple[str]]
            The bound ``from_json`` reconstruction callable and its JSON payload.

        Raises
        ------
        ValueError
            If serialization fails or the JSON payload cannot reconstruct the
            stored value, including non-finite floats unsupported by its schema.
        """
        ...

class PathSummary:
    """
    Simulated paths of one built-in process on a shared time grid.

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

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Reconstruct this value for ``pickle`` through its canonical JSON form.

        Returns
        -------
        tuple[Any, tuple[str]]
            The bound ``from_json`` reconstruction callable and its JSON payload.

        Raises
        ------
        ValueError
            If serialization fails or the JSON payload cannot reconstruct the
            stored value, including non-finite floats unsupported by its schema.
        """
        ...
